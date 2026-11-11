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

#[test]
fn group_header_and_card_move_members_as_one_undo_step() {
    use unge_interaction::{PointerButton, PointerEvent};
    let e = engine();
    let id = Id::from_u128(10);
    select(&e, "a", &[1, 2]);
    action(
        &e,
        "a",
        GroupAction::Create {
            id,
            label: "Before".into(),
        },
    );
    action(
        &e,
        "a",
        GroupAction::Rename {
            id,
            label: "計算グループ".into(),
        },
    );
    let drag = |from, to| {
        let expected_revision = revision(&e);
        for event in [
            PointerEvent::Down {
                pointer: 0,
                position: from,
                button: PointerButton::Primary,
                additive: false,
            },
            PointerEvent::Move {
                pointer: 0,
                position: to,
            },
        ] {
            e.dispatch(
                "a",
                Request::Pointer {
                    expected_revision,
                    event,
                },
            )
            .unwrap();
            assert_eq!(revision(&e), expected_revision);
        }
        e.dispatch(
            "a",
            Request::Pointer {
                expected_revision,
                event: PointerEvent::Up {
                    pointer: 0,
                    position: to,
                },
            },
        )
        .unwrap();
        assert_eq!(revision(&e), expected_revision + 1);
    };
    // Shift the group's top edge into the viewport, as a panned canvas would.
    e.dispatch(
        "a",
        Request::SetViewport {
            viewport: Viewport {
                origin: [0., -100.],
                zoom: 1.,
                size: [800., 600.],
            },
        },
    )
    .unwrap();
    drag([195., 65.], [225., 85.]);
    let moved = e.snapshot().unwrap();
    assert_eq!(moved.placement()[&Id::from_u128(1)].x, 230.);
    assert_eq!(moved.placement()[&Id::from_u128(2)].x, 430.);
    assert_eq!(moved.placement()[&Id::from_u128(3)].x, 600.);
    let rev = revision(&e);
    action(
        &e,
        "a",
        GroupAction::SetCollapsed {
            id,
            collapsed: true,
        },
    );
    assert_eq!(revision(&e), rev);
    assert!(e.groups("a", rev, None, 10).unwrap().groups[0].collapsed);
    assert!(!e.groups("b", rev, None, 10).unwrap().groups[0].collapsed);
    drag([225., 85.], [275., 95.]);
    e.dispatch(
        "a",
        Request::Undo {
            expected_revision: revision(&e),
        },
    )
    .unwrap();
    assert_eq!(e.snapshot().unwrap(), moved);
    action(
        &e,
        "a",
        GroupAction::SetCollapsed {
            id,
            collapsed: false,
        },
    );
    assert!(!e.groups("a", revision(&e), None, 10).unwrap().groups[0].collapsed);
    assert_eq!(
        e.groups("a", revision(&e), None, 10).unwrap().groups[0].label,
        "計算グループ"
    );
}

#[test]
fn folded_ports_connect_to_original_endpoints_in_both_directions() {
    use unge_interaction::{PointerButton, PointerEvent};
    let mut editor = Editor::new(Document::default(), 256).unwrap();
    let port = |name: &str| Port {
        name: name.into(),
        data_type: DataType::Float,
        cardinality: Cardinality::Single,
        required: false,
    };
    for n in 1..=3 {
        editor
            .execute(Command::AddNode {
                node: Node {
                    id: Id::from_u128(n),
                    type_id: "test".into(),
                    inputs: vec![port("in")],
                    outputs: vec![port("out")],
                    properties: Properties::new(),
                },
                rect: Rect {
                    x: (n as f32) * 240. - 120.,
                    y: 100.,
                    ..Rect::default()
                },
            })
            .unwrap();
    }
    let e = Engine::from_editor(editor);
    e.register_view(
        "a",
        Viewport {
            origin: [0., 0.],
            zoom: 1.,
            size: [1100., 600.],
        },
    )
    .unwrap();
    let group = Id::from_u128(10);
    select(&e, "a", &[1, 2]);
    action(
        &e,
        "a",
        GroupAction::Create {
            id: group,
            label: "Unit".into(),
        },
    );
    action(
        &e,
        "a",
        GroupAction::SetCollapsed {
            id: group,
            collapsed: true,
        },
    );
    let external = Id::from_u128(3);
    let geometry = |node, output| {
        let doc = e.snapshot().unwrap();
        unge_render::SceneIndex::new(&doc)
            .group_input(None, &[group].into_iter().collect())
            .ports
            .into_iter()
            .find(|p| p.hit.endpoint.node == node && p.hit.output == output)
            .unwrap()
            .anchor
    };
    let drag = |start, end| {
        let expected_revision = revision(&e);
        for event in [
            PointerEvent::Down {
                pointer: 0,
                position: start,
                button: PointerButton::Primary,
                additive: false,
            },
            PointerEvent::Move {
                pointer: 0,
                position: end,
            },
            PointerEvent::Up {
                pointer: 0,
                position: end,
            },
        ] {
            e.dispatch(
                "a",
                Request::Pointer {
                    expected_revision,
                    event,
                },
            )
            .unwrap();
        }
        assert_eq!(revision(&e), expected_revision + 1);
    };
    let rect = e.snapshot().unwrap().placement()[&external];
    let hit = |point| e.canvas_context("a", point).unwrap().target;
    let from = geometry(Id::from_u128(2), true);
    let to = port_anchor(rect, 0, 1, false);
    assert!(matches!(hit([110., 64.]), ContextTarget::Group(g) if g.id == group && g.collapsed));
    assert!(matches!(hit([650., 122.]), ContextTarget::Node { id, .. } if id == external));
    assert!(matches!(hit([900., 500.]), ContextTarget::Stage));
    assert!(
        matches!(hit(from), ContextTarget::Port { endpoint, output: true, edges, .. }
        if endpoint.node == Id::from_u128(2) && edges.is_empty())
    );
    assert!(
        matches!(hit(to), ContextTarget::Port { endpoint, output: false, .. }
        if endpoint.node == external)
    );
    assert!(e.canvas_context("a", [f32::NAN, 0.]).is_err());
    drag(
        geometry(Id::from_u128(2), true),
        port_anchor(rect, 0, 1, false),
    );
    // The displayed cable starts on the folded card, not on the hidden node's rect.
    let midpoint = std::array::from_fn(|i| (from[i] + to[i]) / 2.);
    let ContextTarget::Edge {
        id: edge,
        from: a,
        to: b,
        ..
    } = hit(midpoint)
    else {
        panic!("visible folded cable was not hit");
    };
    assert_eq!((a.node, b.node), (Id::from_u128(2), external));
    assert!(matches!(hit(from), ContextTarget::Port { edges, .. } if edges == vec![edge]));
    // Start a cable on the group's input; the original connection is still output -> input.
    drag(
        geometry(Id::from_u128(1), false),
        port_anchor(rect, 0, 1, true),
    );
    let doc = e.snapshot().unwrap();
    let edges: std::collections::BTreeSet<_> = doc
        .graph()
        .edges()
        .values()
        .map(|edge| {
            (
                edge.from.node,
                edge.from.port.as_str(),
                edge.to.node,
                edge.to.port.as_str(),
            )
        })
        .collect();
    assert_eq!(
        edges,
        [
            (Id::from_u128(2), "out", external, "in"),
            (external, "out", Id::from_u128(1), "in"),
        ]
        .into_iter()
        .collect()
    );
    e.dispatch(
        "a",
        Request::Undo {
            expected_revision: revision(&e),
        },
    )
    .unwrap();
    assert_eq!(e.snapshot().unwrap().graph().edges().len(), 1);
}
