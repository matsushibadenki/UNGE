//! Optional async execution of an already-approved run commit. No unsolicited frames.
use super::*;
use std::sync::{Mutex, mpsc};
use unge_executor::RunState;
pub(super) struct Job {
    service: RunService,
    summary: Arc<Mutex<RunSummary>>,
    receiver: Option<mpsc::Receiver<Result<(Value, bool)>>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        let id = self.summary.lock().expect("job summary").id;
        let _ = self.service.cancel(id);
    }
}
impl Provider {
    pub(super) fn start_job(&mut self, args: ExecuteArgs) -> Result<Value> {
        if self.jobs.contains_key(&args.commit_id) {
            return self.job_response(&args.commit_id);
        }
        let session_id = self.session_for_commit(&args.commit_id)?;
        let session = &self.sessions[&session_id];
        if !matches!(session.input, Intent::Run { .. }) {
            return Err(AcxError::new(
                "invalid_request",
                "jobs accept run commits only",
            ));
        }
        if session.execution.is_some() || session.started {
            return Err(AcxError::new(
                "execution_uncertain",
                "commit already executed; retrieve execute result",
            ));
        }
        self.unexpired(session)?;
        if self.jobs.values().any(|job| job.receiver.is_some()) {
            return Err(AcxError::new(
                "execution_busy",
                "one job is still executing or finalizing",
            ));
        }
        let snapshot = self.fresh(session)?;
        let (service, observer) = self.execution.as_ref().ok_or_else(|| {
            AcxError::new("jobs_unavailable", "host must configure shared execution")
        })?;
        let prepared = service
            .prepare(&snapshot.document, snapshot.revision)
            .map_err(|e| AcxError::new(&e.code, e.message))?;
        let summary = Arc::new(Mutex::new(
            prepared
                .summary()
                .map_err(|e| AcxError::new(&e.code, e.message))?,
        ));
        let progress = summary.clone();
        let observer = observer.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let document_id = snapshot.document.graph().id;
        let revision = snapshot.revision;
        std::thread::Builder::new()
            .name("unge-acx-run".into())
            .spawn(move || {
                let result = prepared
                    .execute(|next| {
                        *progress.lock().expect("job summary") = next.clone();
                        observer(next);
                    })
                    .map_err(|e| AcxError::new(&e.code, e.message))
                    .and_then(|outcome| run_result(document_id, revision, &outcome.report));
                let _ = sender.send(result);
            })
            .map_err(|e| AcxError::new("worker_error", e))?;
        self.sessions.get_mut(&session_id).unwrap().started = true;
        self.jobs.insert(
            args.commit_id.clone(),
            Job {
                service: service.clone(),
                summary,
                receiver: Some(receiver),
            },
        );
        self.job_response(&args.commit_id)
    }
    pub(super) fn job_response(&self, commit_id: &str) -> Result<Value> {
        let job = self
            .jobs
            .get(commit_id)
            .ok_or_else(|| AcxError::new("unknown_job", "no job for this commit"))?;
        let session_id = self.session_for_commit(commit_id)?;
        let summary = job
            .summary
            .lock()
            .map_err(|e| AcxError::new("state_unavailable", e))?
            .clone();
        Ok(
            json!({"commitId":commit_id,"run":summary,"execution":self.sessions[&session_id].execution}),
        )
    }
    pub(super) fn cancel_job(&mut self, args: ExecuteArgs) -> Result<Value> {
        let job = self
            .jobs
            .get(&args.commit_id)
            .ok_or_else(|| AcxError::new("unknown_job", "no job for this commit"))?;
        // Commit ownership is scoped to this dedicated provider/pipe, never arbitrary run IDs.
        let id = job
            .summary
            .lock()
            .map_err(|e| AcxError::new("state_unavailable", e))?
            .id;
        let next = job.service.cancel(id);
        if let Ok(next) = next {
            let mut current = job
                .summary
                .lock()
                .map_err(|e| AcxError::new("state_unavailable", e))?;
            if next.sequence > current.sequence {
                *current = next;
            }
        } else if matches!(
            job.summary.lock().expect("job summary").state,
            RunState::Queued | RunState::Running
        ) {
            return Err(AcxError::new(
                "execution_uncertain",
                "active job state unavailable",
            ));
        }
        self.job_response(&args.commit_id)
    }
    pub(super) fn collect_jobs(&mut self) -> Result<()> {
        let ready: Vec<_> = self
            .jobs
            .iter_mut()
            .filter_map(|(id, job)| {
                let result = match job.receiver.as_ref()?.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => return None,
                    Err(mpsc::TryRecvError::Disconnected) => Err(AcxError::new(
                        "execution_failed",
                        "worker exited without a result; do not retry effects",
                    )),
                };
                job.receiver = None;
                Some((id.clone(), result))
            })
            .collect();
        for (commit_id, result) in ready {
            let session_id = self.session_for_commit(&commit_id)?;
            let preflight = self.sessions[&session_id].preflight.clone();
            let (result, status) = match result {
                Ok((result, true)) => (result, "succeeded"),
                Ok((result, false)) => (result, "failed"),
                Err(error) => (json!({"error":error}), "failed"),
            };
            let execution = self.record(&preflight, result, status, json!({"reversible":false}))?;
            self.sessions.get_mut(&session_id).unwrap().execution = Some(execution);
        }
        Ok(())
    }
}
