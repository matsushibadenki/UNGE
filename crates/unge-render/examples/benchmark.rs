//! CPU geometry benchmark. No GPU, window, IPC or execution backend is measured.
use std::{collections::BTreeSet, hint::black_box, time::Instant};
use unge_core::*;
use unge_render::{SceneChanges, SceneIndex, SceneUpdate};

fn measure(mut operation: impl FnMut(), iterations: usize) -> serde_json::Value {
    operation(); // warm-up, excluded
    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        operation();
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    serde_json::json!({ "median_ms": samples[iterations / 2],
        "p95_ms": samples[(iterations * 95).div_ceil(100).saturating_sub(1)],
        "min_ms": samples[0], "max_ms": samples[iterations - 1] })
}
fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map(|v| v.parse::<usize>().expect("iteration count"))
        .unwrap_or(20);
    assert!(
        (1..=1000).contains(&iterations),
        "iterations must be 1..1000"
    );
    let ids: Vec<_> = (1..=10_000).map(Id::from_u128).collect();
    let port = Port {
        name: "value".into(),
        data_type: DataType::Float,
        cardinality: Cardinality::Multiple,
        required: false,
    };
    let mut commands: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| Command::AddNode {
            node: Node {
                id: *id,
                type_id: "benchmark".into(),
                inputs: vec![port.clone()],
                outputs: vec![port.clone()],
                properties: Properties::new(),
            },
            rect: Rect {
                x: (i % 100) as f32 * 240.0,
                y: (i / 100) as f32 * 140.0,
                width: 180.0,
                height: 90.0,
            },
        })
        .collect();
    let mut edges = 0;
    'offsets: for offset in 1..ids.len() {
        for i in 0..ids.len() - offset {
            commands.push(Command::Connect {
                edge: Edge {
                    id: Id::from_u128(100_000 + edges),
                    from: Endpoint {
                        node: ids[i],
                        port: "value".into(),
                    },
                    to: Endpoint {
                        node: ids[i + offset],
                        port: "value".into(),
                    },
                },
            });
            edges += 1;
            if edges == 30_000 {
                break 'offsets;
            }
        }
    }
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    let start = Instant::now();
    editor.execute(Command::Batch { commands }).unwrap();
    let fixture_ms = start.elapsed().as_secs_f64() * 1000.0;
    let doc = editor.document();
    assert_eq!(doc.graph().nodes().len(), 10_000);
    assert_eq!(doc.graph().edges().len(), 30_000);
    let index = SceneIndex::new(doc);
    let selection = BTreeSet::new();
    let view = Viewport {
        origin: [0.0, 0.0],
        zoom: 1.0,
        size: [1400.0, 900.0],
    };
    let overview = Viewport { zoom: 0.02, ..view };
    let normal = index.scene(view, &selection).unwrap();
    let distant = index.scene(overview, &selection).unwrap();
    let node_build = measure(
        || {
            black_box(SpatialIndex::new(doc));
        },
        iterations,
    );
    let scene_build = measure(
        || {
            black_box(SceneIndex::new(doc));
        },
        iterations,
    );
    let node_query = measure(
        || {
            black_box(index.spatial_index().query(view.world_rect()));
        },
        iterations,
    );
    let normal_scene = measure(
        || {
            black_box(index.scene(view, &selection).unwrap());
        },
        iterations,
    );
    let overview_scene = measure(
        || {
            black_box(index.scene(overview, &selection).unwrap());
        },
        iterations,
    );
    let validation = measure(
        || {
            doc.validate().unwrap();
        },
        iterations,
    );
    let move_command = Command::MoveNode {
        id: ids[0],
        rect: Rect {
            x: 1.0,
            y: 1.0,
            ..Rect::default()
        },
    };
    let changes = SceneChanges::from_command(&move_command);
    let mut changed = Editor::new(doc.clone(), 0).unwrap();
    changed.execute(move_command).unwrap();
    let changed = changed.document();
    let mut delta_index = SceneIndex::new(doc);
    let delta_update = measure(
        || {
            assert_eq!(
                delta_index.update(changed, &changes),
                SceneUpdate::Incremental
            );
            black_box(&delta_index);
        },
        iterations,
    );
    let full_move_rebuild = measure(
        || {
            black_box(SceneIndex::new(changed));
        },
        iterations,
    );
    let delta_normal_scene = measure(
        || {
            black_box(delta_index.scene(view, &selection).unwrap());
        },
        iterations,
    );
    let delta_overview_scene = measure(
        || {
            black_box(delta_index.scene(overview, &selection).unwrap());
        },
        iterations,
    );
    let move_edit = measure(
        || {
            editor
                .execute(Command::MoveNode {
                    id: ids[0],
                    rect: Rect {
                        x: 1.0,
                        y: 1.0,
                        ..Rect::default()
                    },
                })
                .unwrap();
        },
        iterations,
    );
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "benchmark_version": 2, "scope": "CPU geometry only; no GPU/frame-rate claim",
        "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
        "iterations": iterations, "nodes": 10000, "edges": 30000,
        "fixture_batch_ms": fixture_ms, "document_validation": validation,
        "node_index_build": node_build, "scene_index_build": scene_build,
        "node_query": node_query, "normal_scene": normal_scene, "overview_scene": overview_scene,
        "move_command_validation": move_edit,
        "move_index_delta": delta_update, "move_index_full_rebuild": full_move_rebuild,
        "delta_normal_scene": delta_normal_scene, "delta_overview_scene": delta_overview_scene,
        "normal_counts": { "nodes": normal.visible_nodes, "edges": normal.visible_edges, "quads": normal.quads.len(), "labels": normal.labels.len() },
        "overview_counts": { "nodes": distant.visible_nodes, "edges": distant.visible_edges, "quads": distant.quads.len(), "labels": distant.labels.len() }
    })).unwrap());
}
