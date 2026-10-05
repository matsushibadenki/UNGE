use std::collections::BTreeSet;
use unge_core::*;
use unge_render::*;
fn editor() -> Editor {
    let port = Port {
        name: "p".into(),
        data_type: DataType::Float,
        cardinality: Cardinality::Multiple,
        required: false,
    };
    let mut commands: Vec<_> = (1..=180)
        .map(|n| Command::AddNode {
            node: Node {
                id: Id::from_u128(n),
                type_id: "test".into(),
                inputs: vec![port.clone()],
                outputs: vec![port.clone()],
                properties: Properties::new(),
            },
            rect: Rect {
                x: (n % 10) as f32 * 250.0,
                y: (n / 10) as f32 * 150.0,
                ..Rect::default()
            },
        })
        .collect();
    for n in 1..180 {
        commands.push(Command::Connect {
            edge: Edge {
                id: Id::from_u128(1000 + n),
                from: Endpoint {
                    node: Id::from_u128(n),
                    port: "p".into(),
                },
                to: Endpoint {
                    node: Id::from_u128(n + 1),
                    port: "p".into(),
                },
            },
        });
    }
    let mut editor = Editor::new(Document::default(), 256).unwrap();
    editor.execute(Command::Batch { commands }).unwrap();
    editor
}
fn compare(index: &SceneIndex, document: &Document) {
    let rebuilt = SceneIndex::new(document);
    for (origin, zoom) in [
        ([0.0, 0.0], 1.0),
        ([4000.0, -1000.0], 1.0),
        ([0.0, 0.0], 0.02),
    ] {
        let view = Viewport {
            origin,
            zoom,
            size: [800.0, 600.0],
        };
        let selection = BTreeSet::from([Id::from_u128(1)]);
        let a = index.scene(view, &selection).unwrap();
        let b = rebuilt.scene(view, &selection).unwrap();
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&a.quads),
            bytemuck::cast_slice::<_, u8>(&b.quads)
        );
        assert_eq!(format!("{:?}", a.labels), format!("{:?}", b.labels));
        assert_eq!(
            (a.visible_nodes, a.visible_edges),
            (b.visible_nodes, b.visible_edges)
        );
        let preview = unge_interaction::Preview {
            placement: [(
                Id::from_u128(2),
                Rect {
                    x: 4100.0,
                    y: -500.0,
                    ..Rect::default()
                },
            )]
            .into(),
            ..Default::default()
        };
        let a = index
            .scene_with_preview(view, &selection, &preview)
            .unwrap();
        let b = rebuilt
            .scene_with_preview(view, &selection, &preview)
            .unwrap();
        assert_eq!(
            bytemuck::cast_slice::<_, u8>(&a.quads),
            bytemuck::cast_slice::<_, u8>(&b.quads)
        );
        assert_eq!(format!("{:?}", a.labels), format!("{:?}", b.labels));
        assert_eq!(
            index.spatial_index().query(view.world_rect()),
            rebuilt.spatial_index().query(view.world_rect())
        );
    }
}
#[test]
fn repeated_moves_and_overlay_rebuild_match_fresh_indexes() {
    let mut editor = editor();
    let mut index = SceneIndex::new(editor.document());
    let mut rebuilds = 0;
    for n in 1..=150 {
        let command = Command::MoveNode {
            id: Id::from_u128(n),
            rect: Rect {
                x: 5000.0 + n as f32 * 12.0,
                y: -500.0,
                ..Rect::default()
            },
        };
        let changes = SceneChanges::from_command(&command);
        editor.execute(command).unwrap();
        if index.update(editor.document(), &changes) == SceneUpdate::Rebuilt {
            rebuilds += 1;
        }
        if n % 25 == 0 {
            compare(&index, editor.document());
        }
    }
    assert!(rebuilds > 0);
    for x in [0.0, 9000.0, -1000.0, 200.0] {
        let command = Command::MoveNode {
            id: Id::from_u128(1),
            rect: Rect {
                x,
                ..Rect::default()
            },
        };
        let changes = SceneChanges::from_command(&command);
        editor.execute(command).unwrap();
        assert_eq!(
            index.update(editor.document(), &changes),
            SceneUpdate::Incremental
        );
        compare(&index, editor.document());
    }
}
#[test]
fn batch_final_placement_properties_groups_and_topology_match_rebuild() {
    let mut editor = editor();
    let mut index = SceneIndex::new(editor.document());
    let id = Id::from_u128(1);
    let edit = Command::Batch {
        commands: vec![
            Command::MoveNode {
                id,
                rect: Rect {
                    x: -700.0,
                    ..Rect::default()
                },
            },
            Command::MoveNode {
                id,
                rect: Rect {
                    x: 4500.0,
                    ..Rect::default()
                },
            },
            Command::SetProperty {
                id,
                key: "value".into(),
                value: Some(serde_json::json!(42)),
            },
        ],
    };
    let changes = SceneChanges::from_command(&edit);
    editor.execute(edit).unwrap();
    assert_eq!(
        index.update(editor.document(), &changes),
        SceneUpdate::Incremental
    );
    compare(&index, editor.document());
    let property = Command::SetProperty {
        id,
        key: "value".into(),
        value: None,
    };
    let changes = SceneChanges::from_command(&property);
    editor.execute(property).unwrap();
    assert_eq!(
        index.update(editor.document(), &changes),
        SceneUpdate::Unchanged
    );
    compare(&index, editor.document());
    let group_id = Id::from_u128(5000);
    let group = Command::SetGroup {
        id: group_id,
        group: Some(Group {
            id: group_id,
            label: "Group".into(),
            nodes: BTreeSet::from([id]),
        }),
    };
    let changes = SceneChanges::from_command(&group);
    editor.execute(group).unwrap();
    assert_eq!(
        index.update(editor.document(), &changes),
        SceneUpdate::Incremental
    );
    compare(&index, editor.document());
    for command in [
        Command::Disconnect {
            id: Id::from_u128(1001),
        },
        Command::RemoveNode { id },
    ] {
        let changes = SceneChanges::from_command(&command);
        editor.execute(command).unwrap();
        assert_eq!(
            index.update(editor.document(), &changes),
            SceneUpdate::Rebuilt
        );
        compare(&index, editor.document());
    }
}

#[test]
fn high_degree_moves_rebuild_without_truncating_incident_edges() {
    let mut editor = editor();
    let commands = (1..180)
        .flat_map(|n| {
            [
                Command::Disconnect {
                    id: Id::from_u128(1000 + n),
                },
                Command::Connect {
                    edge: Edge {
                        id: Id::from_u128(1000 + n),
                        from: Endpoint {
                            node: Id::from_u128(1),
                            port: "p".into(),
                        },
                        to: Endpoint {
                            node: Id::from_u128(n + 1),
                            port: "p".into(),
                        },
                    },
                },
            ]
        })
        .collect();
    editor.execute(Command::Batch { commands }).unwrap();
    let mut index = SceneIndex::new(editor.document());
    let command = Command::MoveNode {
        id: Id::from_u128(1),
        rect: Rect {
            x: 5000.0,
            y: -500.0,
            ..Default::default()
        },
    };
    let changes = SceneChanges::from_command(&command);
    editor.execute(command).unwrap();
    assert_eq!(
        index.update(editor.document(), &changes),
        SceneUpdate::Rebuilt
    );
    compare(&index, editor.document());
}
