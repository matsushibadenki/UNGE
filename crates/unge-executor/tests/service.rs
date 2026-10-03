use std::sync::{Arc, Mutex};
use unge_core::*;
use unge_executor::*;
fn setup(limits: RunLimits) -> (RunService, Editor) {
    let registry = Arc::new(math_registry());
    let mut editor = Editor::new(Document::default(), 10).unwrap();
    editor
        .execute(Command::AddNode {
            node: registry.definition("math.number").unwrap().instantiate(),
            rect: Rect::default(),
        })
        .unwrap();
    (
        RunService::new(registry, Scheduler::new(1, 10), limits),
        editor,
    )
}
#[test]
fn frozen_revision_results_counters_and_cache_are_shared() {
    let (service, mut editor) = setup(RunLimits::default());
    let pending = service
        .prepare(editor.document(), editor.revision())
        .unwrap();
    let id = pending.id();
    assert_eq!(service.current().unwrap().unwrap().id, id);
    assert_eq!(
        service.prepare(editor.document(), 1).err().unwrap().code,
        "execution_busy"
    );
    let node = *editor.document().graph().nodes().keys().next().unwrap();
    editor
        .execute(Command::SetProperty {
            id: node,
            key: "value".into(),
            value: Some(99.into()),
        })
        .unwrap();
    let events = Mutex::new(Vec::new());
    let outcome = pending.execute(|s| events.lock().unwrap().push(s)).unwrap();
    assert_eq!(
        outcome.report.nodes[&node].outputs["value"],
        Value::Float(0.)
    );
    let final_state = service.inspect(id).unwrap();
    assert_eq!(final_state.revision, 1);
    assert_eq!(final_state.state, RunState::Finished);
    assert_eq!(final_state.finished, 1);
    assert_eq!(final_state.completed, 1);
    assert!(service.current().unwrap().is_none());
    assert!(
        events
            .lock()
            .unwrap()
            .windows(2)
            .all(|s| s[0].sequence < s[1].sequence)
    );
    let run = service
        .prepare(editor.document(), editor.revision())
        .unwrap();
    let changed = run.execute(|_| {}).unwrap();
    assert_eq!(
        changed.report.nodes[&node].outputs["value"],
        Value::Float(99.)
    );
    let run = service
        .prepare(editor.document(), editor.revision())
        .unwrap();
    let cached = run.execute(|_| {}).unwrap();
    assert_eq!(cached.report.nodes[&node].status, Status::Cached);
}
#[test]
fn reservation_drop_retention_and_cancel_are_bounded_and_idempotent() {
    let (service, editor) = setup(RunLimits {
        retained_runs: 1,
        ..RunLimits::default()
    });
    let a = service.prepare(editor.document(), 1).unwrap();
    let aid = a.id();
    service.cancel(aid).unwrap();
    service.cancel(aid).unwrap();
    let out = a.execute(|_| {}).unwrap();
    assert_eq!(out.reason, StopReason::Cancelled);
    let before = service.inspect(aid).unwrap().sequence;
    assert_eq!(service.cancel(aid).unwrap().sequence, before);
    let b = service.prepare(editor.document(), 1).unwrap();
    let bid = b.id();
    drop(b);
    assert_eq!(
        service.inspect(bid).unwrap().error_code.as_deref(),
        Some("execution_aborted")
    );
    assert_eq!(service.inspect(aid).unwrap_err().code, "unknown_run");
    assert_eq!(
        service.cancel(Id::new_v4()).unwrap_err().code,
        "unknown_run"
    );
    assert!(service.prepare(editor.document(), 1).is_ok());
}
#[test]
fn admission_limits_and_validation_do_not_reserve_a_slot() {
    for limits in [
        RunLimits {
            max_nodes: 0,
            ..RunLimits::default()
        },
        RunLimits {
            max_snapshot_bytes: 1,
            ..RunLimits::default()
        },
    ] {
        let (service, editor) = setup(limits);
        assert_eq!(
            service.prepare(editor.document(), 1).err().unwrap().code,
            "limit_exceeded"
        );
        assert!(service.current().unwrap().is_none());
    }
    let registry = Arc::new(math_registry());
    let service = RunService::new(registry.clone(), Scheduler::new(1, 0), RunLimits::default());
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node: registry.definition("math.add").unwrap().instantiate(),
            rect: Rect::default(),
        })
        .unwrap();
    assert_eq!(
        service.prepare(editor.document(), 1).err().unwrap().code,
        "invalid_graph"
    );
    assert!(service.current().unwrap().is_none());
}
#[test]
fn panic_releases_slot_and_preserves_service_for_next_run() {
    let (service, editor) = setup(RunLimits::default());
    let pending = service.prepare(editor.document(), 1).unwrap();
    let id = pending.id();
    assert_eq!(
        pending
            .execute(|_| panic!("observer panic"))
            .unwrap_err()
            .code,
        "execution_panicked"
    );
    assert_eq!(service.inspect(id).unwrap().state, RunState::Failed);
    assert!(service.current().unwrap().is_none());
    assert!(
        service
            .prepare(editor.document(), 1)
            .unwrap()
            .execute(|_| {})
            .is_ok()
    );
}
#[test]
fn worker_waiting_execution_can_be_cancelled_from_another_thread() {
    struct Wait;
    impl NodeExecutor for Wait {
        fn execute(
            &self,
            _: ExecutionContext,
            _: Inputs,
        ) -> futures::future::BoxFuture<'_, std::result::Result<Outputs, String>> {
            Box::pin(futures::future::pending())
        }
    }
    let registry = math_registry();
    let mut definition = registry.definition("math.number").unwrap().clone();
    definition.pure = false;
    let mut custom = Registry::default();
    custom.register(definition.clone(), Arc::new(Wait)).unwrap();
    let service = RunService::new(Arc::new(custom), Scheduler::new(1, 0), RunLimits::default());
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node: definition.instantiate(),
            rect: Rect::default(),
        })
        .unwrap();
    let prepared = service.prepare(editor.document(), 1).unwrap();
    let id = prepared.id();
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        prepared.execute(|summary| {
            if summary.started == 1 && summary.finished == 0 {
                tx.send(()).unwrap();
            }
        })
    });
    rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
    assert_eq!(service.inspect(id).unwrap().state, RunState::Running);
    service.cancel(id).unwrap();
    assert_eq!(
        thread.join().unwrap().unwrap().reason,
        StopReason::Cancelled
    );
    assert_eq!(service.inspect(id).unwrap().cancelled, 1);
}
