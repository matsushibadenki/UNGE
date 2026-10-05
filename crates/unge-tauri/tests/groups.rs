use unge_core::*;
use unge_tauri::*;
fn engine() -> Engine {
    let mut editor = Editor::new(Document::default(), 256).unwrap();
    for n in 1..=3 {
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
                    x: n as f32 * 200.,
                    ..Rect::default()
                },
            })
            .unwrap();
    }
    let e = Engine::from_editor(editor);
    for v in ["a", "b"] {
        e.register_view(
            v,
            Viewport {
                origin: [0., 0.],
                zoom: 1.,
                size: [800., 600.],
            },
        )
        .unwrap();
    }
    e
}
fn revision(e: &Engine) -> u64 {
    e.dispatch("a", Request::Summary).unwrap().revision
}
fn action(e: &Engine, view: &str, a: GroupAction) -> Summary {
    e.dispatch(
        view,
        Request::Group {
            expected_revision: revision(e),
            action: a,
        },
    )
    .unwrap()
}
fn select(e: &Engine, view: &str, ids: &[u128]) {
    e.dispatch(
        view,
        Request::Select {
            ids: ids.iter().map(|id| Id::from_u128(*id)).collect(),
        },
    )
    .unwrap();
}
#[test]
fn group_membership_operations_share_history_and_preserve_nodes() {
    let e = engine();
    let id = Id::from_u128(10);
    select(&e, "a", &[1, 2]);
    let start = revision(&e);
    action(
        &e,
        "a",
        GroupAction::Create {
            id,
            label: "計算 组 Group".into(),
        },
    );
    let p = e.groups("a", revision(&e), None, 50).unwrap();
    assert_eq!(p.groups[0].members, 2);
    assert_eq!(p.selected_nodes, 2);
    // Duplicate/no-op membership and rename do not consume a revision or Undo.
    let r = revision(&e);
    action(&e, "a", GroupAction::AddSelection { id });
    action(
        &e,
        "a",
        GroupAction::Rename {
            id,
            label: "計算 组 Group".into(),
        },
    );
    assert_eq!(revision(&e), r);
    select(&e, "b", &[3]);
    action(&e, "b", GroupAction::AddSelection { id });
    action(
        &e,
        "a",
        GroupAction::Rename {
            id,
            label: "改名".into(),
        },
    );
    action(&e, "a", GroupAction::RemoveSelection { id });
    assert_eq!(
        e.groups("a", revision(&e), None, 1).unwrap().groups[0].members,
        1
    );
    let r = revision(&e);
    action(&e, "a", GroupAction::SelectMembers { id });
    assert_eq!(revision(&e), r);
    assert_eq!(
        e.view_state("a").unwrap().selection,
        [Id::from_u128(3)].into()
    );
    assert_eq!(
        e.view_state("b").unwrap().selection,
        [Id::from_u128(3)].into()
    );
    select(&e, "b", &[]);
    assert_eq!(e.view_state("a").unwrap().selection.len(), 1);
    action(&e, "a", GroupAction::Delete { id });
    assert_eq!(e.groups("a", revision(&e), None, 1).unwrap().total, 0);
    assert_eq!(e.dispatch("a", Request::Summary).unwrap().nodes, 3);
    for _ in 0..5 {
        e.dispatch(
            "b",
            Request::Undo {
                expected_revision: revision(&e),
            },
        )
        .unwrap();
    }
    assert_eq!(e.groups("a", revision(&e), None, 1).unwrap().total, 0);
    assert_eq!(revision(&e), start + 10);
    for _ in 0..5 {
        e.dispatch(
            "a",
            Request::Redo {
                expected_revision: revision(&e),
            },
        )
        .unwrap();
    }
    assert_eq!(e.groups("a", revision(&e), None, 1).unwrap().total, 0);
}
#[test]
fn errors_and_stale_intents_leave_group_and_selection_unchanged() {
    let e = engine();
    let id = Id::from_u128(10);
    let r = revision(&e);
    let create = |label: &str| Request::Group {
        expected_revision: r,
        action: GroupAction::Create {
            id,
            label: label.into(),
        },
    };
    assert_eq!(
        e.dispatch("a", create("Group")).unwrap_err().code,
        "invalid_selection"
    );
    select(&e, "a", &[1]);
    for label in [" ".to_string(), "字".repeat(257)] {
        assert_eq!(
            e.dispatch("a", create(&label)).unwrap_err().code,
            "invalid_group_label"
        );
    }
    e.dispatch("a", create("Group")).unwrap();
    assert_eq!(
        e.dispatch(
            "a",
            Request::Group {
                expected_revision: r,
                action: GroupAction::SelectMembers { id }
            }
        )
        .unwrap_err()
        .code,
        "revision_conflict"
    );
    let r = revision(&e);
    assert_eq!(
        e.dispatch(
            "a",
            Request::Group {
                expected_revision: r,
                action: GroupAction::Create {
                    id,
                    label: "overwrite".into()
                }
            }
        )
        .unwrap_err()
        .code,
        "group_exists"
    );
    assert_eq!(
        e.dispatch(
            "a",
            Request::Group {
                expected_revision: r,
                action: GroupAction::Delete {
                    id: Id::from_u128(11)
                }
            }
        )
        .unwrap_err()
        .code,
        "missing_group"
    );
    assert_eq!(
        e.dispatch(
            "unknown",
            Request::Group {
                expected_revision: r,
                action: GroupAction::Delete { id }
            }
        )
        .unwrap_err()
        .code,
        "unknown_view"
    );
    assert_eq!(e.groups("a", r, None, 1).unwrap().groups[0].label, "Group");
    assert_eq!(revision(&e), r);
}
#[test]
fn group_pages_are_bounded_and_empty_groups_can_be_selected() {
    let e = engine();
    for n in 10..115 {
        e.dispatch(
            "a",
            Request::Apply {
                expected_revision: revision(&e),
                command: Command::SetGroup {
                    id: Id::from_u128(n),
                    group: Some(Group {
                        id: Id::from_u128(n),
                        label: "\n".repeat(1000),
                        nodes: Default::default(),
                    }),
                },
            },
        )
        .unwrap();
    }
    let r = revision(&e);
    let p = e.groups("a", r, None, 100).unwrap();
    assert_eq!(p.total, 105);
    assert_eq!(p.groups.len(), 100);
    assert_eq!(p.next, Some(Id::from_u128(109)));
    assert_eq!(p.groups[0].label, " ".repeat(256));
    let tail = e.groups("a", r, p.next, 100).unwrap();
    assert_eq!(tail.groups.len(), 5);
    assert!(tail.next.is_none());
    for limit in [0, 101] {
        assert_eq!(
            e.groups("a", r, None, limit).unwrap_err().code,
            "invalid_request"
        );
    }
    assert_eq!(
        e.groups("unknown", r, None, 1).unwrap_err().code,
        "unknown_view"
    );
    assert_eq!(
        e.groups("a", r - 1, None, 1).unwrap_err().code,
        "revision_conflict"
    );
    select(&e, "a", &[1]);
    action(
        &e,
        "a",
        GroupAction::SelectMembers {
            id: Id::from_u128(10),
        },
    );
    assert!(e.view_state("a").unwrap().selection.is_empty());
    assert_eq!(revision(&e), r);
}
