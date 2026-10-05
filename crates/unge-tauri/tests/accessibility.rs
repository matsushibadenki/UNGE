use unge_core::*;
use unge_render::{LabelCatalog, LabelText, NodeLabels};
use unge_tauri::*;

fn engine() -> Engine {
    let mut editor = Editor::new(Document::default(), 256).unwrap();
    for n in 1..=105 {
        editor
            .execute(Command::AddNode {
                node: Node {
                    id: Id::from_u128(n),
                    type_id: "test".into(),
                    inputs: vec![],
                    outputs: vec![],
                    properties: Properties::new(),
                },
                rect: Rect {
                    x: n as f32,
                    ..Rect::default()
                },
            })
            .unwrap();
    }
    let engine = Engine::from_editor(editor);
    for view in ["a", "b"] {
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
    engine
        .set_labels(LabelCatalog::from([(
            "test".into(),
            NodeLabels {
                title: LabelText {
                    en: "Number".into(),
                    ja: "数値".into(),
                    zh_cn: "数值".into(),
                },
                ..Default::default()
            },
        )]))
        .unwrap();
    engine
}
#[test]
fn semantic_pages_are_bounded_localized_and_view_specific() {
    let engine = engine();
    let rev = engine.dispatch("a", Request::Summary).unwrap().revision;
    for (locale, name) in [
        (Locale::En, "Number"),
        (Locale::Ja, "数値"),
        (Locale::ZhCn, "数值"),
    ] {
        engine.dispatch("a", Request::SetLocale { locale }).unwrap();
        let page = engine.accessible_nodes("a", rev, None, 100).unwrap();
        assert_eq!(page.total, 105);
        assert_eq!(page.nodes.len(), 100);
        assert_eq!(page.nodes[0].title, name);
        assert_eq!(page.nodes[0].rect.x, 1.);
        assert_eq!(page.next, Some(Id::from_u128(100)));
        let tail = engine.accessible_nodes("a", rev, page.next, 100).unwrap();
        assert_eq!(tail.nodes.len(), 5);
        assert_eq!(tail.nodes[0].id, Id::from_u128(101));
        assert!(tail.next.is_none());
    }
    engine
        .dispatch(
            "a",
            Request::Select {
                ids: [Id::from_u128(1)].into(),
            },
        )
        .unwrap();
    assert!(engine.accessible_nodes("a", rev, None, 1).unwrap().nodes[0].selected);
    assert!(!engine.accessible_nodes("b", rev, None, 1).unwrap().nodes[0].selected);
    for limit in [0, 101, usize::MAX] {
        assert_eq!(
            engine
                .accessible_nodes("a", rev, None, limit)
                .unwrap_err()
                .code,
            "invalid_request"
        );
    }
    assert_eq!(
        engine
            .accessible_nodes("missing", rev, None, 1)
            .unwrap_err()
            .code,
        "unknown_view"
    );
}
#[test]
fn stale_pages_cannot_edit_and_refresh_observes_move_delete_undo() {
    let engine = engine();
    let rev = engine.dispatch("a", Request::Summary).unwrap().revision;
    engine
        .dispatch(
            "b",
            Request::Apply {
                expected_revision: rev,
                command: Command::MoveNode {
                    id: Id::from_u128(1),
                    rect: Rect {
                        x: 500.,
                        ..Rect::default()
                    },
                },
            },
        )
        .unwrap();
    assert_eq!(
        engine.accessible_nodes("a", rev, None, 1).unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(
        engine
            .dispatch(
                "a",
                Request::Apply {
                    expected_revision: rev,
                    command: Command::RemoveNode {
                        id: Id::from_u128(1)
                    }
                }
            )
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    let moved = engine.accessible_nodes("a", rev + 1, None, 1).unwrap();
    assert_eq!(moved.nodes[0].rect.x, 500.);
    engine
        .dispatch(
            "a",
            Request::Apply {
                expected_revision: moved.revision,
                command: Command::RemoveNode {
                    id: Id::from_u128(1),
                },
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .accessible_nodes("a", rev + 2, None, 1)
            .unwrap()
            .nodes[0]
            .id,
        Id::from_u128(2)
    );
    engine
        .dispatch(
            "b",
            Request::Undo {
                expected_revision: rev + 2,
            },
        )
        .unwrap();
    assert_eq!(
        engine
            .accessible_nodes("a", rev + 3, None, 1)
            .unwrap()
            .nodes[0]
            .rect
            .x,
        500.
    );
    engine
        .set_labels(LabelCatalog::from([(
            "test".into(),
            NodeLabels {
                title: LabelText {
                    en: "\n".repeat(1000),
                    ..Default::default()
                },
                ..Default::default()
            },
        )]))
        .unwrap();
    let title = engine
        .accessible_nodes("a", rev + 3, None, 1)
        .unwrap()
        .nodes
        .remove(0)
        .title;
    assert_eq!(title.chars().count(), 256);
    assert!(title.chars().all(|c| c == ' '));
    engine
        .dispatch("a", Request::SetLocale { locale: Locale::Ja })
        .unwrap();
    assert_eq!(
        engine
            .accessible_nodes("a", rev + 3, None, 1)
            .unwrap()
            .nodes[0]
            .title,
        "test"
    );
}
