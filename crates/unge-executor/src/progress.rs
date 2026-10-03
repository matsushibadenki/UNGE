use crate::{NodeResult, Report, Status};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, Waker},
};
use unge_core::Id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Completed,
    Cancelled,
    DeadlineExceeded,
}

/// Small metadata only: no properties, outputs, resource contents or error strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProgressEvent {
    Started {
        total: usize,
    },
    NodeStarted {
        node: Id,
    },
    NodeFinished {
        node: Id,
        status: Status,
        finished: usize,
        total: usize,
    },
    Finished {
        reason: StopReason,
        finished: usize,
        total: usize,
    },
}
#[derive(Debug)]
pub struct ExecutionOutcome {
    pub report: Report,
    pub reason: StopReason,
}

pub(crate) fn finish(
    report: &mut Report,
    observer: &impl Fn(ProgressEvent),
    total: usize,
    node: Id,
    result: NodeResult,
) {
    let status = result.status.clone();
    report.nodes.insert(node, result);
    observer(ProgressEvent::NodeFinished {
        node,
        status,
        finished: report.nodes.len(),
        total,
    });
}

#[derive(Debug, Default)]
pub(crate) struct CancellationState {
    pub cancelled: AtomicBool,
    waiters: Mutex<BTreeMap<Id, Waker>>,
}
impl CancellationState {
    pub fn cancel(&self) {
        if !self.cancelled.swap(true, Ordering::AcqRel) {
            let waiters = std::mem::take(&mut *self.waiters.lock().expect("cancellation lock"));
            for waker in waiters.into_values() {
                waker.wake();
            }
        }
    }
}
pub(crate) struct CancellationWait {
    state: Arc<CancellationState>,
    id: Id,
}
impl CancellationWait {
    pub fn new(state: Arc<CancellationState>) -> Self {
        Self {
            state,
            id: Id::new_v4(),
        }
    }
}
impl Future for CancellationWait {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let mut waiters = self.state.waiters.lock().expect("cancellation lock");
        if self.state.cancelled.load(Ordering::Acquire) {
            waiters.remove(&self.id);
            Poll::Ready(())
        } else {
            waiters.insert(self.id, cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for CancellationWait {
    fn drop(&mut self) {
        self.state
            .waiters
            .lock()
            .expect("cancellation lock")
            .remove(&self.id);
    }
}
