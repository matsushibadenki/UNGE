use futures::{channel::oneshot, future::BoxFuture};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use unge_core::*;
use unge_executor::*;

fn numbers() -> (Registry, Document) {
    let registry = math_registry();
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    for _ in 0..3 {
        editor
            .execute(Command::AddNode {
                node: registry.definition("math.number").unwrap().instantiate(),
                rect: Rect::default(),
            })
            .unwrap();
    }
    (registry, editor.document().clone())
}
fn collect(events: &Mutex<Vec<ProgressEvent>>, event: ProgressEvent) {
    events.lock().unwrap().push(event);
}
fn assert_terminals(events: &[ProgressEvent], report: &Report) {
    assert_eq!(
        events.first(),
        Some(&ProgressEvent::Started {
            total: report.nodes.len()
        })
    );
    let terminals: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            ProgressEvent::NodeFinished {
                node,
                status,
                finished,
                total,
            } => Some((*node, status, *finished, *total)),
            _ => None,
        })
        .collect();
    assert_eq!(terminals.len(), report.nodes.len());
    for (index, (id, status, finished, total)) in terminals.iter().enumerate() {
        assert_eq!(*status, &report.nodes[id].status);
        assert_eq!(*finished, index + 1);
        assert_eq!(*total, report.nodes.len());
    }
    let ids: std::collections::BTreeSet<_> = terminals.iter().map(|e| e.0).collect();
    assert_eq!(ids.len(), report.nodes.len());
}
#[test]
fn terminal_events_match_reports_and_warm_cache_has_no_started_nodes() {
    let (registry, document) = numbers();
    let mut scheduler = Scheduler::new(2, 10);
    for cached in [false, true] {
        let events = Mutex::new(Vec::new());
        let outcome = futures::executor::block_on(scheduler.run_with_progress(
            document.graph(),
            &registry,
            Cancellation::default(),
            |e| collect(&events, e),
        ))
        .unwrap();
        let events = events.into_inner().unwrap();
        assert_terminals(&events, &outcome.report);
        assert_eq!(outcome.reason, StopReason::Completed);
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, ProgressEvent::NodeStarted { .. }))
                .count(),
            if cached { 0 } else { 3 }
        );
        assert_eq!(
            events.last(),
            Some(&ProgressEvent::Finished {
                reason: StopReason::Completed,
                finished: 3,
                total: 3
            })
        );
    }
}
struct Waiting {
    started: Mutex<Option<oneshot::Sender<()>>>,
    dropped_cancelled: Arc<AtomicBool>,
}
struct Guard {
    token: Cancellation,
    dropped_cancelled: Arc<AtomicBool>,
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.dropped_cancelled
            .store(self.token.is_cancelled(), Ordering::SeqCst);
    }
}
impl NodeExecutor for Waiting {
    fn execute(
        &self,
        ctx: ExecutionContext,
        _: Inputs,
    ) -> BoxFuture<'_, std::result::Result<Outputs, String>> {
        Box::pin(async move {
            let _guard = Guard {
                token: ctx.cancellation,
                dropped_cancelled: self.dropped_cancelled.clone(),
            };
            if let Some(sender) = self.started.lock().unwrap().take() {
                let _ = sender.send(());
            }
            futures::future::pending().await
        })
    }
}
fn waiting_graph() -> (
    Registry,
    Document,
    oneshot::Receiver<()>,
    Arc<AtomicBool>,
    Id,
) {
    let mut registry = math_registry();
    let (tx, rx) = oneshot::channel();
    let dropped = Arc::new(AtomicBool::new(false));
    let mut definition = registry.definition("math.add").unwrap().clone();
    definition.type_id = "wait".into();
    definition.inputs.truncate(1);
    definition.pure = false;
    registry
        .register(
            definition.clone(),
            Arc::new(Waiting {
                started: Mutex::new(Some(tx)),
                dropped_cancelled: dropped.clone(),
            }),
        )
        .unwrap();
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    let source = registry.definition("math.number").unwrap().instantiate();
    let source_id = source.id;
    let wait = definition.instantiate();
    let downstream = definition.instantiate();
    let mut commands: Vec<_> = [source.clone(), wait.clone(), downstream.clone()]
        .into_iter()
        .map(|node| Command::AddNode {
            node,
            rect: Rect::default(),
        })
        .collect();
    for (from, to) in [(source.id, wait.id), (wait.id, downstream.id)] {
        commands.push(Command::Connect {
            edge: Edge {
                id: Id::new_v4(),
                from: Endpoint {
                    node: from,
                    port: "value".into(),
                },
                to: Endpoint {
                    node: to,
                    port: "a".into(),
                },
            },
        });
    }
    editor.execute(Command::Batch { commands }).unwrap();
    (registry, editor.document().clone(), rx, dropped, source_id)
}
#[test]
fn deadline_drops_pending_work_keeps_completed_results_and_cache() {
    let (registry, document, started, dropped, source) = waiting_graph();
    let events = Mutex::new(Vec::new());
    let token = Cancellation::default();
    let mut scheduler = Scheduler::new(1, 10);
    let outcome = futures::executor::block_on(scheduler.run_with_deadline(
        document.graph(),
        &registry,
        token.clone(),
        async {
            started.await.unwrap();
        },
        |e| collect(&events, e),
    ))
    .unwrap();
    assert_eq!(outcome.reason, StopReason::DeadlineExceeded);
    assert!(token.is_cancelled());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(outcome.report.nodes[&source].status, Status::Completed);
    assert_eq!(
        outcome
            .report
            .nodes
            .values()
            .filter(|n| n.status == Status::Cancelled)
            .count(),
        2
    );
    assert_eq!(scheduler.cache_usage().entries, 1);
    let events = events.into_inner().unwrap();
    assert_terminals(&events, &outcome.report);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ProgressEvent::NodeStarted { .. }))
            .count(),
        2
    );
}
#[test]
fn external_cancellation_wakes_scheduler_while_executor_never_wakes() {
    let (registry, document, started, dropped, _) = waiting_graph();
    let mut scheduler = Scheduler::new(1, 10);
    let token = Cancellation::default();
    let outcome = futures::executor::block_on(async {
        futures::join!(
            scheduler.run_with_progress(document.graph(), &registry, token.clone(), |_| {}),
            async {
                started.await.unwrap();
                token.cancel();
            }
        )
        .0
        .unwrap()
    });
    assert_eq!(outcome.reason, StopReason::Cancelled);
    assert!(dropped.load(Ordering::SeqCst));
}
#[test]
fn immediate_deadline_and_pre_cancel_do_not_poll_executors_or_hit_cache() {
    let (registry, document) = numbers();
    let mut scheduler = Scheduler::new(1, 10);
    futures::executor::block_on(scheduler.run(
        document.graph(),
        &registry,
        Cancellation::default(),
    ))
    .unwrap();
    for pre_cancel in [false, true] {
        let events = Mutex::new(Vec::new());
        let token = Cancellation::default();
        if pre_cancel {
            token.cancel();
        }
        let outcome = futures::executor::block_on(scheduler.run_with_deadline(
            document.graph(),
            &registry,
            token,
            futures::future::ready(()),
            |e| collect(&events, e),
        ))
        .unwrap();
        assert_eq!(
            outcome.reason,
            if pre_cancel {
                StopReason::Cancelled
            } else {
                StopReason::DeadlineExceeded
            }
        );
        assert!(
            outcome
                .report
                .nodes
                .values()
                .all(|n| n.status == Status::Cancelled)
        );
        let events = events.into_inner().unwrap();
        assert_terminals(&events, &outcome.report);
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, ProgressEvent::NodeStarted { .. }))
        );
    }
}
#[test]
fn observer_can_cancel_without_polling_queued_nodes_and_progress_has_no_values() {
    let (registry, document) = numbers();
    let mut scheduler = Scheduler::new(1, 0);
    let token = Cancellation::default();
    let events = Mutex::new(Vec::new());
    let outcome = futures::executor::block_on(scheduler.run_with_progress(
        document.graph(),
        &registry,
        token.clone(),
        |event| {
            if matches!(event, ProgressEvent::NodeStarted { .. }) {
                token.cancel();
            }
            collect(&events, event);
        },
    ))
    .unwrap();
    assert_eq!(outcome.reason, StopReason::Cancelled);
    let events = events.into_inner().unwrap();
    assert_terminals(&events, &outcome.report);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ProgressEvent::NodeStarted { .. }))
            .count(),
        1
    );
    for event in &events {
        let value = serde_json::to_value(event).unwrap();
        assert!(value.get("outputs").is_none());
        assert_eq!(
            serde_json::from_value::<ProgressEvent>(value).unwrap(),
            *event
        );
    }
}
#[test]
fn invalid_graph_emits_nothing_and_empty_graph_has_start_and_finish() {
    let registry = math_registry();
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node: registry.definition("math.add").unwrap().instantiate(),
            rect: Rect::default(),
        })
        .unwrap();
    let events = Mutex::new(Vec::new());
    let mut scheduler = Scheduler::new(1, 1);
    assert!(
        futures::executor::block_on(scheduler.run_with_progress(
            editor.document().graph(),
            &registry,
            Cancellation::default(),
            |e| collect(&events, e)
        ))
        .is_err()
    );
    assert!(events.lock().unwrap().is_empty());
    let outcome = futures::executor::block_on(scheduler.run_with_progress(
        Document::default().graph(),
        &registry,
        Cancellation::default(),
        |e| collect(&events, e),
    ))
    .unwrap();
    assert!(outcome.report.nodes.is_empty());
    assert_eq!(
        events.into_inner().unwrap(),
        vec![
            ProgressEvent::Started { total: 0 },
            ProgressEvent::Finished {
                reason: StopReason::Completed,
                finished: 0,
                total: 0
            }
        ]
    );
}
#[test]
fn cancellation_supports_multiple_waiters_and_send_scheduler_future() {
    let token = Cancellation::default();
    futures::executor::block_on(async {
        let a = token.cancelled();
        let b = token.cancelled();
        futures::join!(a, b, async {
            token.cancel();
        });
    });
    fn require_send<T: Send>(_: T) {}
    let (registry, document) = numbers();
    let mut scheduler = Scheduler::new(1, 0);
    require_send(scheduler.run(document.graph(), &registry, Cancellation::default()));
}

#[test]
fn failed_and_blocked_events_complete_traversal_without_reporting_success() {
    struct Fail;
    impl NodeExecutor for Fail {
        fn execute(
            &self,
            _: ExecutionContext,
            _: Inputs,
        ) -> BoxFuture<'_, std::result::Result<Outputs, String>> {
            Box::pin(async { Err("host failure".into()) })
        }
    }
    let (source, document, _, _, _) = waiting_graph();
    let mut registry = Registry::default();
    for definition in source.definitions() {
        registry
            .register(definition.clone(), Arc::new(Fail))
            .unwrap();
    }
    let events = Mutex::new(Vec::new());
    let outcome = futures::executor::block_on(Scheduler::new(1, 10).run_with_progress(
        document.graph(),
        &registry,
        Cancellation::default(),
        |e| collect(&events, e),
    ))
    .unwrap();
    assert_eq!(outcome.reason, StopReason::Completed);
    assert_eq!(
        outcome
            .report
            .nodes
            .values()
            .filter(|n| n.status == Status::Failed)
            .count(),
        1
    );
    assert_eq!(
        outcome
            .report
            .nodes
            .values()
            .filter(|n| n.status == Status::Blocked)
            .count(),
        2
    );
    let events = events.into_inner().unwrap();
    assert_terminals(&events, &outcome.report);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, ProgressEvent::NodeStarted { .. }))
            .count(),
        1
    );
}

#[test]
fn cancellation_wakes_distinct_tasks_and_unregisters_dropped_waiters() {
    use futures::task::{ArcWake, waker};
    use std::{future::Future, sync::atomic::AtomicUsize, task::Context};
    struct Counter(AtomicUsize);
    impl ArcWake for Counter {
        fn wake_by_ref(this: &Arc<Self>) {
            this.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let token = Cancellation::default();
    let counters: Vec<_> = (0..3)
        .map(|_| Arc::new(Counter(AtomicUsize::new(0))))
        .collect();
    let mut a = Box::pin(token.cancelled());
    let mut b = Box::pin(token.cancelled());
    let mut dropped = Box::pin(token.cancelled());
    for (wait, counter) in [
        (&mut a, &counters[0]),
        (&mut b, &counters[1]),
        (&mut dropped, &counters[2]),
    ] {
        let wake = waker(counter.clone());
        assert!(
            wait.as_mut()
                .poll(&mut Context::from_waker(&wake))
                .is_pending()
        );
    }
    drop(dropped);
    token.cancel();
    token.cancel();
    assert_eq!(
        counters
            .iter()
            .map(|c| c.0.load(Ordering::SeqCst))
            .collect::<Vec<_>>(),
        vec![1, 1, 0]
    );
    futures::executor::block_on(async {
        futures::join!(a, b);
    });
}
