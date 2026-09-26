use unge_core::*;
use unge_tauri::*;
fn engine() -> Engine {
    let engine = Engine::new(Document::default()).unwrap();
    for label in ["main", "second"] {
        engine
            .register_view(
                label,
                Viewport {
                    origin: [0., 0.],
                    zoom: 1.,
                    size: [800., 600.],
                },
            )
            .unwrap();
    }
    engine
}
fn command() -> Command {
    Command::AddNode {
        node: Node {
            id: Id::new_v4(),
            type_id: "test".into(),
            inputs: vec![],
            outputs: vec![],
            properties: Properties::new(),
        },
        rect: Rect::default(),
    }
}
#[test]
fn all_windows_share_document_and_reject_stale_edits() {
    let engine = engine();
    engine
        .dispatch(
            "main",
            Request::Apply {
                expected_revision: 0,
                command: command(),
            },
        )
        .unwrap();
    let summary = engine.dispatch("second", Request::Summary).unwrap();
    assert_eq!(summary.nodes, 1);
    let error = engine
        .dispatch(
            "second",
            Request::Apply {
                expected_revision: 0,
                command: command(),
            },
        )
        .unwrap_err();
    assert_eq!(error.code, "revision_conflict");
    assert_eq!(engine.dispatch("main", Request::Summary).unwrap().nodes, 1);
    engine
        .dispatch(
            "second",
            Request::Undo {
                expected_revision: 1,
            },
        )
        .unwrap();
    assert_eq!(engine.dispatch("main", Request::Summary).unwrap().nodes, 0);
}
#[test]
fn unknown_windows_and_deep_commands_are_rejected() {
    let engine = engine();
    assert_eq!(
        engine
            .dispatch("unknown", Request::Summary)
            .unwrap_err()
            .code,
        "unknown_view"
    );
    let mut cmd = command();
    for _ in 0..20 {
        cmd = Command::Batch {
            commands: vec![cmd],
        };
    }
    assert_eq!(
        engine
            .dispatch(
                "main",
                Request::Apply {
                    expected_revision: 0,
                    command: cmd
                }
            )
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(
        engine.dispatch("main", Request::Summary).unwrap().revision,
        0
    );
}
#[test]
fn views_do_not_change_document_revision() {
    let engine = engine();
    engine
        .dispatch(
            "main",
            Request::SetViewport {
                viewport: Viewport {
                    origin: [10., 20.],
                    zoom: 2.,
                    size: [800., 600.],
                },
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .dispatch("second", Request::Summary)
            .unwrap()
            .revision,
        0
    );
    engine.remove_view("main").unwrap();
    assert!(engine.dispatch("main", Request::Summary).is_err());
    assert!(engine.dispatch("second", Request::Summary).is_ok());
}

#[cfg(feature = "acx")]
#[test]
fn ai_and_windows_share_revision_history_and_document() {
    use serde_json::json;
    use std::sync::Arc;
    use unge_acx::{GraphHost, Policy, Provider};
    let engine = engine();
    let before = GraphHost::snapshot(&engine).unwrap();
    let mut provider = Provider::new(
        Arc::new(engine.clone()),
        Arc::new(unge_executor::math_registry()),
        Policy::math_demo(),
    );
    let input = json!({"kind":"edit","document_id":before.document.graph().id,"expected_revision":0,"operations":[{"kind":"create_node","id":Id::new_v4(),"type_id":"math.number","properties":{"value":42},"rect":{"x":0,"y":0,"width":180,"height":90}}]});
    let pf = provider
        .dispatch("preflight", json!({"input":input}))
        .unwrap();
    let bound = json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]});
    let auth = provider.dispatch("authorize", bound).unwrap();
    let commit=provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":pf["input"]})).unwrap();
    provider
        .dispatch("execute", json!({"commitId":commit["commitId"]}))
        .unwrap();
    let ui = engine.dispatch("main", Request::Summary).unwrap();
    assert_eq!(ui.nodes, 1);
    assert_eq!(ui.revision, 1);
    engine
        .dispatch(
            "second",
            Request::Undo {
                expected_revision: 1,
            },
        )
        .unwrap();
    assert_eq!(
        provider
            .dispatch("observe", json!({"query":"summary"}))
            .unwrap()["nodes"],
        0
    );
    assert_eq!(provider.dispatch("recover",json!({"commitId":commit["commitId"],"authorization":auth["authorization"],"expectedRevision":1})).unwrap_err().code,"revision_conflict");
}
