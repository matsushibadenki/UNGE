use std::sync::Arc;
use unge_core::*;
use unge_executor::math_registry;
use unge_tauri::*;
fn setup(large: bool) -> (Engine, Id) {
    let registry = Arc::new(math_registry());
    let mut node = registry.definition("math.number").unwrap().instantiate();
    if large {
        node.properties
            .insert("extra".into(), "x".repeat(256 * 1024).into());
    }
    let id = node.id;
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node,
            rect: Rect::default(),
        })
        .unwrap();
    let engine = Engine::from_registry(editor.document().clone(), 20, registry).unwrap();
    for name in ["a", "b"] {
        engine
            .register_view(
                name,
                Viewport {
                    origin: [0., 0.],
                    zoom: 1.,
                    size: [800., 600.],
                },
            )
            .unwrap();
    }
    (engine, id)
}
#[test]
fn properties_share_validation_revision_and_atomic_undo() {
    let (engine, id) = setup(false);
    let p = engine.node_properties("a", id, 0).unwrap();
    assert_eq!(p.name.ja, "数値");
    assert!(p.schema.fields["value"].required);
    assert_eq!(p.properties["value"], 0);
    let command = |value| Command::SetProperty {
        id,
        key: "value".into(),
        value: Some(value),
    };
    engine
        .dispatch(
            "a",
            Request::Apply {
                expected_revision: 0,
                command: Command::Batch {
                    commands: vec![command(12.into()), command(42.into())],
                },
            },
        )
        .unwrap();
    assert_eq!(
        engine.node_properties("b", id, 1).unwrap().properties["value"],
        42
    );
    assert_eq!(
        engine.node_properties("a", id, 0).unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(
        engine
            .dispatch(
                "a",
                Request::Apply {
                    expected_revision: 1,
                    command: Command::Batch {
                        commands: vec![command(15.into()), command("invalid".into())]
                    }
                }
            )
            .unwrap_err()
            .code,
        "invalid_properties"
    );
    assert_eq!(
        engine.node_properties("a", id, 1).unwrap().properties["value"],
        42
    );
    engine
        .dispatch(
            "b",
            Request::Undo {
                expected_revision: 1,
            },
        )
        .unwrap();
    assert_eq!(
        engine.node_properties("a", id, 2).unwrap().properties["value"],
        0
    );
}
#[test]
fn properties_are_scoped_bounded_and_require_registry() {
    let (engine, id) = setup(false);
    assert_eq!(
        engine.node_properties("unknown", id, 0).unwrap_err().code,
        "unknown_view"
    );
    assert_eq!(
        engine
            .node_properties("a", Id::new_v4(), 0)
            .unwrap_err()
            .code,
        "missing_node"
    );
    let (engine, id) = setup(true);
    assert_eq!(
        engine.node_properties("a", id, 0).unwrap_err().code,
        "limit_exceeded"
    );
    let basic = Engine::new(Document::default()).unwrap();
    basic
        .register_view(
            "a",
            Viewport {
                origin: [0., 0.],
                zoom: 1.,
                size: [800., 600.],
            },
        )
        .unwrap();
    assert_eq!(
        basic
            .node_properties("a", Id::new_v4(), 0)
            .unwrap_err()
            .code,
        "properties_unavailable"
    );
}
