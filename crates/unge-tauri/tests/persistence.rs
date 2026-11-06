#![cfg(feature = "acx")]
use std::{fs, path::PathBuf, sync::Arc};
use unge_acx::{GraphHost, PersistentHostOptions};
use unge_core::*;
use unge_executor::math_registry;
use unge_interaction::{PointerButton, PointerEvent};
use unge_tauri::{Engine, GroupAction, Request};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("unge-engine-state-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn log(&self) -> PathBuf {
        self.0.join("state.jsonl")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn seed() -> (Document, Id) {
    let node = math_registry()
        .definition("math.number")
        .unwrap()
        .instantiate();
    let id = node.id;
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    editor
        .execute(Command::AddNode {
            node,
            rect: Rect {
                x: 40.,
                y: 40.,
                width: 200.,
                height: 100.,
            },
        })
        .unwrap();
    (editor.document().clone(), id)
}
fn open(temp: &Temp, doc: &Document, options: PersistentHostOptions) -> Engine {
    let engine =
        Engine::open_persistent(temp.log(), doc.clone(), Arc::new(math_registry()), options)
            .unwrap();
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
}
fn property(id: Id, value: i32) -> Command {
    Command::SetProperty {
        id,
        key: "value".into(),
        value: Some(value.into()),
    }
}
#[test]
fn ui_pointer_group_ai_and_history_restore_one_editor() {
    let temp = Temp::new();
    let (doc, id) = seed();
    let engine = open(&temp, &doc, Default::default());
    engine
        .dispatch(
            "a",
            Request::Apply {
                expected_revision: 0,
                command: property(id, 3),
            },
        )
        .unwrap();
    engine
        .dispatch("a", Request::Select { ids: [id].into() })
        .unwrap();
    let group = Id::new_v4();
    engine
        .dispatch(
            "a",
            Request::Group {
                expected_revision: 1,
                action: GroupAction::Create {
                    id: group,
                    label: "saved group".into(),
                },
            },
        )
        .unwrap();
    for event in [
        PointerEvent::Down {
            pointer: 1,
            position: [90., 60.],
            button: PointerButton::Primary,
            additive: false,
        },
        PointerEvent::Move {
            pointer: 1,
            position: [120., 80.],
        },
        PointerEvent::Up {
            pointer: 1,
            position: [120., 80.],
        },
    ] {
        engine
            .dispatch(
                "a",
                Request::Pointer {
                    expected_revision: 2,
                    event,
                },
            )
            .unwrap();
    }
    let dragged = GraphHost::snapshot(&engine).unwrap();
    assert_eq!(dragged.revision, 3);
    assert_eq!(dragged.document.placement()[&id].x, 70.);
    engine.apply(doc.graph().id, 3, property(id, 99)).unwrap();
    engine
        .dispatch(
            "b",
            Request::Undo {
                expected_revision: 4,
            },
        )
        .unwrap();
    engine
        .dispatch(
            "a",
            Request::Redo {
                expected_revision: 5,
            },
        )
        .unwrap();
    engine
        .dispatch(
            "b",
            Request::Undo {
                expected_revision: 6,
            },
        )
        .unwrap();
    let saved = GraphHost::snapshot(&engine).unwrap();
    assert_eq!(saved.revision, 7);
    assert!(saved.document.graph().groups().contains_key(&group));
    drop(engine);
    let engine = open(&temp, &doc, Default::default());
    let restored = GraphHost::snapshot(&engine).unwrap();
    assert_eq!(restored.document, saved.document);
    assert_eq!(restored.revision, 7);
    assert_eq!(engine.selection_summary("a").unwrap().count, 0);
    let page = engine.accessible_nodes("b", 7, None, 10).unwrap();
    assert_eq!(page.nodes[0].rect.x, 70.);
    engine
        .dispatch(
            "a",
            Request::Redo {
                expected_revision: 7,
            },
        )
        .unwrap();
    assert_eq!(
        engine.node_properties("b", id, 8).unwrap().properties["value"],
        99
    );
}
#[test]
fn failures_do_not_append_or_publish_and_invalid_storage_is_preserved() {
    let temp = Temp::new();
    let (doc, id) = seed();
    let options = PersistentHostOptions {
        max_bytes: 4096,
        ..Default::default()
    };
    let engine = open(&temp, &doc, options);
    let original = fs::read(temp.log()).unwrap();
    for (view, revision, command, code) in [
        ("missing", 0, property(id, 3), "unknown_view"),
        ("a", 1, property(id, 3), "revision_conflict"),
        (
            "a",
            0,
            Command::SetProperty {
                id,
                key: "value".into(),
                value: Some("wrong".into()),
            },
            "invalid_properties",
        ),
        (
            "a",
            0,
            Command::SetProperty {
                id,
                key: "extra".into(),
                value: Some("x".repeat(8192).into()),
            },
            "storage_full",
        ),
    ] {
        assert_eq!(
            engine
                .dispatch(
                    view,
                    Request::Apply {
                        expected_revision: revision,
                        command
                    }
                )
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(GraphHost::snapshot(&engine).unwrap().document, doc);
        assert_eq!(fs::read(temp.log()).unwrap(), original);
        assert_eq!(
            engine.accessible_nodes("a", 0, None, 10).unwrap().nodes[0]
                .rect
                .x,
            40.
        );
    }
    engine
        .dispatch(
            "a",
            Request::Undo {
                expected_revision: 0,
            },
        )
        .unwrap();
    assert_eq!(fs::read(temp.log()).unwrap(), original);
    assert_eq!(
        Engine::open_persistent(temp.log(), doc.clone(), Arc::new(math_registry()), options)
            .err()
            .unwrap()
            .code,
        "storage_locked"
    );
    engine
        .dispatch(
            "b",
            Request::Apply {
                expected_revision: 0,
                command: property(id, 5),
            },
        )
        .unwrap();
    drop(engine);
    let bytes = fs::read(temp.log()).unwrap();
    fs::write(temp.log(), &bytes[..bytes.len() - 1]).unwrap();
    assert_eq!(
        Engine::open_persistent(temp.log(), doc, Arc::new(math_registry()), options)
            .err()
            .unwrap()
            .code,
        "storage_corrupt"
    );
    assert_eq!(fs::read(temp.log()).unwrap(), bytes[..bytes.len() - 1]);
}
#[test]
fn provider_pairing_and_original_receipt_survive_engine_restart() {
    let temp = Temp::new();
    let (doc, id) = seed();
    let registry = Arc::new(math_registry());
    let engine = Arc::new(open(&temp, &doc, Default::default()));
    let mut provider = unge_acx::Provider::new(
        engine.clone(),
        registry.clone(),
        unge_acx::Policy::math_demo(),
    )
    .with_journal(temp.0.join("replay.jsonl"), Default::default())
    .unwrap();
    let early_state = fs::read(temp.log()).unwrap();
    use serde_json::json;
    let input = json!({"kind":"edit","document_id":doc.graph().id,"expected_revision":0,"operations":[{"kind":"set_property","id":id,"key":"value","value":42}]});
    let pf = provider
        .dispatch("preflight", json!({"input":input}))
        .unwrap();
    let bound = json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]});
    let grant = provider.dispatch("authorize", bound.clone()).unwrap();
    let commit=provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"input":pf["input"],"authorization":grant["authorization"]})).unwrap();
    let args = json!({"commitId":commit["commitId"]});
    let reply = provider.dispatch("execute", args.clone()).unwrap();
    let receipt = provider
        .dispatch("receipt", json!({"receiptId":reply["receiptId"]}))
        .unwrap();
    assert_eq!(
        engine.node_properties("a", id, 1).unwrap().properties["value"],
        42
    );
    drop(provider);
    drop(engine);
    let engine = Arc::new(open(&temp, &doc, Default::default()));
    let mut provider =
        unge_acx::Provider::new(engine.clone(), registry, unge_acx::Policy::math_demo())
            .with_journal(temp.0.join("replay.jsonl"), Default::default())
            .unwrap();
    assert_eq!(provider.dispatch("execute", args).unwrap(), reply);
    assert_eq!(
        provider
            .dispatch("receipt", json!({"receiptId":reply["receiptId"]}))
            .unwrap(),
        receipt
    );
    assert_eq!(GraphHost::snapshot(engine.as_ref()).unwrap().revision, 1);
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
    drop(provider);
    drop(engine);
    fs::write(temp.log(), &early_state).unwrap();
    let engine = Arc::new(open(&temp, &doc, Default::default()));
    let error = unge_acx::Provider::new(
        engine,
        Arc::new(math_registry()),
        unge_acx::Policy::math_demo(),
    )
    .with_journal(temp.0.join("replay.jsonl"), Default::default())
    .err()
    .unwrap();
    assert_eq!(error.code, "storage_corrupt");
}

#[test]
fn concurrent_views_publish_only_one_revision_and_restore_the_winner() {
    let temp = Temp::new();
    let (doc, id) = seed();
    let engine = open(&temp, &doc, Default::default());
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let threads: Vec<_> = [("a", 7), ("b", 8)]
        .into_iter()
        .map(|(view, value)| {
            let engine = engine.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                engine.dispatch(
                    view,
                    Request::Apply {
                        expected_revision: 0,
                        command: property(id, value),
                    },
                )
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .into_iter()
            .find_map(|result| result.err())
            .unwrap()
            .code,
        "revision_conflict"
    );
    let saved = engine.snapshot().unwrap();
    drop(engine);
    let engine = open(&temp, &doc, Default::default());
    assert_eq!(engine.snapshot().unwrap(), saved);
    assert_eq!(engine.dispatch("a", Request::Summary).unwrap().revision, 1);
}

#[test]
fn storage_usage_distinguishes_memory_and_persistent_engines() {
    let temp = Temp::new();
    let (doc, id) = seed();
    let memory = Engine::new(doc.clone()).unwrap();
    assert_eq!(memory.storage_usage().unwrap(), None);
    let engine = open(&temp, &doc, Default::default());
    let before = engine.storage_usage().unwrap().unwrap();
    assert_eq!(before.bytes, fs::metadata(temp.log()).unwrap().len());
    engine
        .dispatch(
            "a",
            Request::Apply {
                expected_revision: 0,
                command: property(id, 7),
            },
        )
        .unwrap();
    let after = engine.storage_usage().unwrap().unwrap();
    assert_eq!(after.revision, 1);
    assert_eq!(after.records, before.records + 1);
    assert!(after.bytes > before.bytes);
    assert_eq!(after.undo_steps, 1);
    drop(engine);
    let restored = open(&temp, &doc, Default::default());
    assert_eq!(restored.storage_usage().unwrap().unwrap(), after);
}
