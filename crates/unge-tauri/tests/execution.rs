use std::sync::Arc;
use unge_core::*;
use unge_executor::*;
use unge_tauri::{Engine, Request};
fn setup() -> (Engine, RunService, Id) {
    let registry = Arc::new(math_registry());
    let mut editor = Editor::new(Document::default(), 10)
        .unwrap()
        .with_validator(registry.clone())
        .unwrap();
    let node = registry.definition("math.number").unwrap().instantiate();
    let id = node.id;
    editor
        .execute(Command::AddNode {
            node,
            rect: Rect::default(),
        })
        .unwrap();
    let service = RunService::new(registry, Scheduler::new(1, 10), RunLimits::default());
    let engine = Engine::from_editor(editor).with_execution(service.clone());
    for view in ["main", "other"] {
        engine
            .register_view(
                view,
                Viewport {
                    origin: [0., 0.],
                    zoom: 1.,
                    size: [800., 600.],
                },
            )
            .unwrap();
    }
    (engine, service, id)
}
#[test]
fn run_uses_atomic_revision_snapshot_and_edits_do_not_overwrite_execution() {
    let (engine, _, node) = setup();
    assert_eq!(
        engine.prepare_run("unknown", 1).err().unwrap().code,
        "unknown_view"
    );
    assert_eq!(
        engine.prepare_run("main", 0).err().unwrap().code,
        "revision_conflict"
    );
    let run = engine.prepare_run("main", 1).unwrap();
    let id = run.id();
    assert_eq!(engine.current_execution("other").unwrap().unwrap().id, id);
    assert_eq!(
        engine.prepare_run("other", 1).err().unwrap().code,
        "execution_busy"
    );
    engine
        .dispatch(
            "other",
            Request::Apply {
                expected_revision: 1,
                command: Command::SetProperty {
                    id: node,
                    key: "value".into(),
                    value: Some(80.into()),
                },
            },
        )
        .unwrap();
    let outcome = run.execute(|_| {}).unwrap();
    assert_eq!(
        outcome.report.nodes[&node].outputs["value"],
        Value::Float(0.)
    );
    assert_eq!(
        engine.snapshot().unwrap().graph().nodes()[&node].properties["value"],
        serde_json::json!(80)
    );
    let summary = engine.execution_status("other", id).unwrap();
    assert_eq!(summary.revision, 1);
    assert_eq!(summary.state, RunState::Finished);
    assert_eq!(
        engine.dispatch("main", Request::Summary).unwrap().revision,
        2
    );
}
#[test]
fn cancellation_is_shared_by_views_and_never_changes_revision_or_history() {
    let (engine, service, _) = setup();
    let run = engine.prepare_run("main", 1).unwrap();
    let id = run.id();
    assert_eq!(
        engine.cancel_execution("unknown", id).unwrap_err().code,
        "unknown_view"
    );
    assert!(
        engine
            .cancel_execution("other", id)
            .unwrap()
            .cancel_requested
    );
    assert_eq!(run.execute(|_| {}).unwrap().reason, StopReason::Cancelled);
    assert_eq!(service.inspect(id).unwrap().cancelled, 1);
    assert_eq!(
        engine.dispatch("main", Request::Summary).unwrap().revision,
        1
    );
    assert_eq!(
        engine
            .dispatch(
                "main",
                Request::Undo {
                    expected_revision: 1
                }
            )
            .unwrap()
            .nodes,
        0
    );
}
#[test]
fn missing_configuration_and_foreign_document_runs_are_rejected() {
    let engine = Engine::new(Document::default()).unwrap();
    engine
        .register_view(
            "main",
            Viewport {
                origin: [0., 0.],
                zoom: 1.,
                size: [800., 600.],
            },
        )
        .unwrap();
    assert_eq!(
        engine.prepare_run("main", 0).err().unwrap().code,
        "execution_unavailable"
    );
    let (engine, service, _) = setup();
    let foreign = Document::default();
    let run = service.prepare(&foreign, 0).unwrap();
    assert_eq!(
        engine.current_execution("main").unwrap_err().code,
        "document_mismatch"
    );
    assert_eq!(
        engine.execution_status("main", run.id()).unwrap_err().code,
        "document_mismatch"
    );
    assert_eq!(
        engine.cancel_execution("main", run.id()).unwrap_err().code,
        "document_mismatch"
    );
}
