//! Opt-in release benchmark; synthetic data only. No GPU, IPC or Provider receipts.
use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
use unge_acx::{PersistentEditor, PersistentHostOptions};
use unge_core::{Command, Document, Editor, Id, Rect};
use unge_executor::{Registry, math_registry};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("unge-persistence-bench-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn timed<T>(action: impl FnOnce() -> T) -> (T, f64) {
    let start = Instant::now();
    let value = action();
    (value, start.elapsed().as_secs_f64() * 1000.)
}
fn stats(raw: &[f64]) -> Value {
    assert!(!raw.is_empty() && raw.iter().all(|v| v.is_finite() && *v >= 0.));
    let mut sorted = raw.to_vec();
    sorted.sort_by(f64::total_cmp);
    json!({"median_ms":sorted[sorted.len()/2], "p95_ms":sorted[(sorted.len()*95).div_ceil(100)-1], "raw_ms":raw})
}
fn fixture(nodes: usize, registry: &Registry) -> Document {
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    let commands = (0..nodes)
        .map(|i| {
            let mut node = registry.definition("math.number").unwrap().instantiate();
            node.id = Id::from_u128(i as u128 + 1);
            Command::AddNode {
                node,
                rect: Rect {
                    x: (i % 100) as f32 * 240.,
                    y: (i / 100) as f32 * 140.,
                    width: 200.,
                    height: 100.,
                },
            }
        })
        .collect();
    editor.execute(Command::Batch { commands }).unwrap();
    editor.document().clone()
}
fn equivalent(memory: &Editor, durable: &PersistentEditor, registry: &Registry) {
    durable.check().unwrap();
    assert_eq!(memory.document(), durable.editor().document());
    assert_eq!(memory.revision(), durable.editor().revision());
    assert_eq!(memory.history_stats(), durable.editor().history_stats());
    memory.document().validate().unwrap();
    unge_core::DocumentValidator::validate(registry, memory.document()).unwrap();
}
fn case(nodes: usize, prime: usize, kind: &str, samples: usize) -> Value {
    let temp = Temp::new();
    let path = temp.0.join("state.jsonl");
    let registry = Arc::new(math_registry());
    let seed = fixture(nodes, &registry);
    let options = PersistentHostOptions::default();
    let mut memory = Editor::with_history_limits(seed.clone(), options.history)
        .unwrap()
        .with_validator(registry.clone())
        .unwrap();
    let (opened, create_ms) =
        timed(|| PersistentEditor::open(&path, seed.clone(), registry.clone(), options).unwrap());
    let mut durable = opened;
    for i in 0..prime {
        let id = Id::from_u128((i % nodes) as u128 + 1);
        let mut rect = memory.document().placement()[&id];
        rect.x += 1.;
        let command = Command::MoveNode { id, rect };
        memory.execute(command.clone()).unwrap();
        durable.execute(command).unwrap();
    }
    equivalent(&memory, &durable, &registry);
    let baseline = memory.document().clone();
    let id = Id::from_u128(1);
    let mut rect = baseline.placement()[&id];
    rect.x += 1.;
    rect.y += 1.;
    let command = match kind {
        "move" => Command::MoveNode { id, rect },
        "property" => Command::SetProperty {
            id,
            key: "value".into(),
            value: Some(1.0.into()),
        },
        _ => unreachable!(),
    };
    let mut raw = Vec::new();
    for i in 0..samples + 5 {
        let m = command.clone();
        let d = command.clone();
        // Alternate order to reduce fixed-order thermal/cache bias.
        let (memory_edit, durable_edit) = if i % 2 == 0 {
            let (_, m) = timed(|| memory.execute(m).unwrap());
            let (_, d) = timed(|| durable.execute(d).unwrap());
            (m, d)
        } else {
            let (_, d) = timed(|| durable.execute(d).unwrap());
            let (_, m) = timed(|| memory.execute(m).unwrap());
            (m, d)
        };
        equivalent(&memory, &durable, &registry);
        let (m, memory_undo) = timed(|| memory.undo().unwrap());
        let (d, durable_undo) = timed(|| durable.undo().unwrap());
        assert!(m && d);
        equivalent(&memory, &durable, &registry);
        assert_eq!(memory.document(), &baseline);
        let (m, memory_redo) = timed(|| memory.redo().unwrap());
        let (d, durable_redo) = timed(|| durable.redo().unwrap());
        assert!(m && d);
        equivalent(&memory, &durable, &registry);
        assert!(memory.undo().unwrap());
        assert!(durable.undo().unwrap());
        equivalent(&memory, &durable, &registry);
        assert_eq!(memory.document(), &baseline);
        if i >= 5 {
            raw.push(json!({"memory_edit":memory_edit,"durable_edit":durable_edit,"memory_undo":memory_undo,"durable_undo":durable_undo,"memory_redo":memory_redo,"durable_redo":durable_redo}));
        }
    }
    drop(durable);
    // History must remain usable after reopening, not just produce the same Document.
    let mut reopened =
        PersistentEditor::open(&path, seed.clone(), registry.clone(), options).unwrap();
    equivalent(&memory, &reopened, &registry);
    assert!(memory.redo().unwrap());
    assert!(reopened.redo().unwrap());
    equivalent(&memory, &reopened, &registry);
    assert!(memory.undo().unwrap());
    assert!(reopened.undo().unwrap());
    equivalent(&memory, &reopened, &registry);
    let usage = reopened.storage_usage().unwrap();
    drop(reopened);
    assert_eq!(usage.bytes, fs::metadata(&path).unwrap().len());
    let mut reopen_raw = Vec::new();
    for _ in 0..5 {
        let (restored, ms) = timed(|| {
            PersistentEditor::open(&path, seed.clone(), registry.clone(), options).unwrap()
        });
        equivalent(&memory, &restored, &registry);
        assert_eq!(restored.storage_usage().unwrap(), usage);
        drop(restored);
        reopen_raw.push(ms);
    }
    let mut metrics = serde_json::Map::new();
    for key in [
        "memory_edit",
        "durable_edit",
        "memory_undo",
        "durable_undo",
        "memory_redo",
        "durable_redo",
    ] {
        let values: Vec<_> = raw.iter().map(|s| s[key].as_f64().unwrap()).collect();
        metrics.insert(key.into(), stats(&values));
    }
    json!({"nodes":nodes,"edges":0,"kind":kind,"primed_history_steps":prime,"history_limit_steps":options.history.max_steps,
        "history_limit_bytes":options.history.max_bytes,"history_bytes":usage.history_bytes,"undo_steps":usage.undo_steps,"redo_steps":usage.redo_steps,
        "create_ms":create_ms,"state_bytes":usage.bytes,"state_records":usage.records,"revision":usage.revision,"metrics":metrics,"reopen":stats(&reopen_raw),"correctness":"document/revision/history/validation match after every transition; reopened redo/undo verified"})
}
fn main() {
    let samples = match std::env::args().nth(1) {
        None => 30,
        Some(s) => s.parse::<usize>().expect("sample count must be an integer"),
    };
    assert!((3..=100).contains(&samples), "sample count must be 3..100");
    if cfg!(debug_assertions) {
        eprintln!("run with --release");
        std::process::exit(2);
    }
    let mut cases = Vec::new();
    for nodes in [100, 1000, 10000] {
        for prime in [0, 128] {
            for kind in ["move", "property"] {
                eprintln!("persistence benchmark: nodes={nodes}, prime={prime}, kind={kind}");
                cases.push(case(nodes, prime, kind, samples));
            }
        }
    }
    println!("{}",serde_json::to_string_pretty(&json!({"version":1,"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"warmup":5,"samples":samples,"reopen_samples":5,
        "scope":"PersistentEditor candidate cloning/validation/append/sync/publication vs memory Editor; warm filesystem cache; no Provider receipts, Engine mutex contention, SceneIndex, GPU, IPC or external resources; history bytes are serialized budget, not heap/RSS",
        "cases":cases})).unwrap());
}
