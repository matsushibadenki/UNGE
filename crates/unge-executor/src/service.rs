//! Host-owned execution admission, cancellation and bounded metadata retention.
use crate::{
    Cancellation, ExecutionOutcome, ProgressEvent, Registry, Scheduler, Status, StopReason,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    future::Future,
    io::{self, Write},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};
use unge_core::{Document, Id};

#[derive(Debug, Clone, Copy)]
pub struct RunLimits {
    pub retained_runs: usize,
    pub max_nodes: usize,
    pub max_edges: usize,
    pub max_snapshot_bytes: usize,
}
impl Default for RunLimits {
    fn default() -> Self {
        Self {
            retained_runs: 32,
            max_nodes: 10_000,
            max_edges: 30_000,
            max_snapshot_bytes: 16 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Running,
    Finished,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub id: Id,
    pub document_id: Id,
    pub revision: u64,
    pub sequence: u64,
    pub state: RunState,
    pub total: usize,
    pub started: usize,
    pub finished: usize,
    pub completed: usize,
    pub cached: usize,
    pub failed: usize,
    pub blocked: usize,
    pub cancelled: usize,
    pub cancel_requested: bool,
    pub reason: Option<StopReason>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{code}: {message}")]
pub struct RunError {
    pub code: String,
    pub message: String,
}
impl RunError {
    fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }
}
struct Record {
    summary: RunSummary,
    token: Cancellation,
}
#[derive(Default)]
struct State {
    records: BTreeMap<Id, Record>,
    completed: VecDeque<Id>,
    active: Option<Id>,
}
#[derive(Clone)]
pub struct RunService {
    registry: Arc<Registry>,
    scheduler: Arc<Mutex<Scheduler>>,
    state: Arc<Mutex<State>>,
    limits: RunLimits,
}
impl RunService {
    pub fn new(registry: Arc<Registry>, scheduler: Scheduler, limits: RunLimits) -> Self {
        Self {
            registry,
            scheduler: Arc::new(Mutex::new(scheduler)),
            state: Arc::new(Mutex::new(State::default())),
            limits,
        }
    }
    /// Trusted adapters must share the exact registered executor set.
    pub fn uses_registry(&self, registry: &Arc<Registry>) -> bool {
        Arc::ptr_eq(&self.registry, registry)
    }
    /// Validates before reserving the single execution slot. Caller checks the document revision.
    pub fn prepare(&self, document: &Document, revision: u64) -> Result<PreparedRun, RunError> {
        if document.graph().nodes().len() > self.limits.max_nodes
            || document.graph().edges().len() > self.limits.max_edges
        {
            return Err(RunError::new(
                "limit_exceeded",
                "execution graph exceeds host limits",
            ));
        }
        let mut size = Size {
            bytes: 0,
            limit: self.limits.max_snapshot_bytes,
        };
        serde_json::to_writer(&mut size, document)
            .map_err(|e| RunError::new("limit_exceeded", e))?;
        self.registry
            .validate(document.graph())
            .map_err(|e| RunError::new("invalid_graph", e))?;
        let mut state = self
            .state
            .lock()
            .map_err(|e| RunError::new("state_unavailable", e))?;
        if state.active.is_some() {
            return Err(RunError::new("execution_busy", "one run is already active"));
        }
        let id = Id::new_v4();
        let summary = RunSummary {
            id,
            document_id: document.graph().id,
            revision,
            sequence: 0,
            state: RunState::Queued,
            total: document.graph().nodes().len(),
            started: 0,
            finished: 0,
            completed: 0,
            cached: 0,
            failed: 0,
            blocked: 0,
            cancelled: 0,
            cancel_requested: false,
            reason: None,
            error_code: None,
        };
        state.records.insert(
            id,
            Record {
                summary,
                token: Cancellation::default(),
            },
        );
        state.active = Some(id);
        Ok(PreparedRun {
            id,
            document: document.clone(),
            service: self.clone(),
            armed: true,
        })
    }
    pub fn current(&self) -> Result<Option<RunSummary>, RunError> {
        let state = self
            .state
            .lock()
            .map_err(|e| RunError::new("state_unavailable", e))?;
        Ok(state
            .active
            .and_then(|id| state.records.get(&id))
            .map(|record| record.summary.clone()))
    }
    pub fn inspect(&self, id: Id) -> Result<RunSummary, RunError> {
        self.state
            .lock()
            .map_err(|e| RunError::new("state_unavailable", e))?
            .records
            .get(&id)
            .map(|r| r.summary.clone())
            .ok_or_else(|| RunError::new("unknown_run", id))
    }
    /// Idempotent for retained terminal runs. No document mutation or revision increment.
    pub fn cancel(&self, id: Id) -> Result<RunSummary, RunError> {
        let (summary, token) = {
            let mut state = self
                .state
                .lock()
                .map_err(|e| RunError::new("state_unavailable", e))?;
            let record = state
                .records
                .get_mut(&id)
                .ok_or_else(|| RunError::new("unknown_run", id))?;
            let active = matches!(record.summary.state, RunState::Queued | RunState::Running);
            if active && !record.summary.cancel_requested {
                record.summary.cancel_requested = true;
                record.summary.sequence += 1;
            }
            (record.summary.clone(), active.then(|| record.token.clone()))
        };
        if let Some(token) = token {
            token.cancel();
        }
        Ok(summary)
    }
    fn event(&self, id: Id, event: ProgressEvent) -> RunSummary {
        let mut state = self.state.lock().expect("run state lock");
        let summary = &mut state.records.get_mut(&id).expect("active run").summary;
        summary.sequence += 1;
        match event {
            ProgressEvent::Started { .. } => summary.state = RunState::Running,
            ProgressEvent::NodeStarted { .. } => summary.started += 1,
            ProgressEvent::NodeFinished {
                status, finished, ..
            } => {
                summary.finished = finished;
                match status {
                    Status::Completed => summary.completed += 1,
                    Status::Cached => summary.cached += 1,
                    Status::Failed => summary.failed += 1,
                    Status::Blocked => summary.blocked += 1,
                    Status::Cancelled => summary.cancelled += 1,
                }
            }
            // The slot is released only after scheduler cleanup and return.
            ProgressEvent::Finished { reason, .. } => {
                summary.reason = Some(reason);
            }
        }
        summary.clone()
    }
    fn terminal(&self, id: Id, error: Option<&str>) -> RunSummary {
        let mut state = self.state.lock().expect("run state lock");
        let record = state.records.get_mut(&id).expect("active run");
        record.summary.sequence += 1;
        record.summary.state = if error.is_some() {
            RunState::Failed
        } else {
            RunState::Finished
        };
        record.summary.error_code = error.map(str::to_owned);
        let summary = record.summary.clone();
        state.active = None;
        state.completed.push_back(id);
        while state.completed.len() > self.limits.retained_runs.max(1) {
            let oldest = state.completed.pop_front().expect("retention invariant");
            state.records.remove(&oldest);
        }
        summary
    }
}
/// Owns a frozen Rust snapshot and reservation. Dropping unused work releases the slot.
pub struct PreparedRun {
    id: Id,
    document: Document,
    service: RunService,
    armed: bool,
}
impl PreparedRun {
    pub fn id(&self) -> Id {
        self.id
    }
    pub fn summary(&self) -> Result<RunSummary, RunError> {
        self.service.inspect(self.id)
    }
    /// Run on a host worker. This method blocks until async execution finishes.
    pub fn execute(
        self,
        observer: impl Fn(RunSummary) + Sync,
    ) -> Result<ExecutionOutcome, RunError> {
        self.execute_until(futures::future::pending(), observer)
    }
    pub fn execute_until<D: Future<Output = ()>>(
        mut self,
        deadline: D,
        observer: impl Fn(RunSummary) + Sync,
    ) -> Result<ExecutionOutcome, RunError> {
        let token = self
            .service
            .state
            .lock()
            .map_err(|e| RunError::new("state_unavailable", e))?
            .records[&self.id]
            .token
            .clone();
        let result = {
            let mut scheduler = self
                .service
                .scheduler
                .lock()
                .map_err(|e| RunError::new("state_unavailable", e))?;
            match catch_unwind(AssertUnwindSafe(|| {
                futures::executor::block_on(scheduler.run_with_deadline(
                    self.document.graph(),
                    &self.service.registry,
                    token,
                    deadline,
                    |event| observer(self.service.event(self.id, event)),
                ))
            })) {
                Ok(result) => result.map_err(|e| RunError::new("execution_failed", e)),
                Err(_) => {
                    scheduler.clear_cache();
                    Err(RunError::new(
                        "execution_panicked",
                        "host executor or observer panicked; do not retry effects",
                    ))
                }
            }
        };
        if result.is_err() {
            let _ = self.service.cancel(self.id);
        }
        let summary = self
            .service
            .terminal(self.id, result.as_ref().err().map(|e| e.code.as_str()));
        self.armed = false;
        // Terminal notification is advisory; cleanup has already completed.
        let _ = catch_unwind(AssertUnwindSafe(|| observer(summary)));
        result
    }
}
impl Drop for PreparedRun {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.service.cancel(self.id);
            self.service.terminal(self.id, Some("execution_aborted"));
        }
    }
}
struct Size {
    bytes: usize,
    limit: usize,
}
impl Write for Size {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other("snapshot exceeds byte limit"));
        }
        self.bytes += buf.len();
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
