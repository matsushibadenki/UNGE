//! Compare legacy full host validation with Editor-aware Registry validation.
#[path = "../../unge-render/examples/support/large_graph.rs"]
mod large_graph;
use std::{sync::Arc, time::Instant};
use unge_core::*;
use unge_executor::*;

struct Never;
impl NodeExecutor for Never {
    fn execute(
        &self,
        _: ExecutionContext,
        _: Inputs,
    ) -> futures::future::BoxFuture<'_, std::result::Result<Outputs, String>> {
        panic!("CPU validation benchmark must not execute nodes")
    }
}
struct Legacy(Arc<Registry>);
impl DocumentValidator for Legacy {
    fn validate(&self, doc: &Document) -> unge_core::Result<()> {
        self.0.validate_edit(doc.graph())
    }
}
fn stats(mut v: Vec<f64>) -> serde_json::Value {
    v.sort_by(f64::total_cmp);
    serde_json::json!({"median_ms":v[v.len()/2],"p95_ms":v[(v.len()*95).div_ceil(100)-1]})
}
fn main() {
    let doc = large_graph::document();
    let mut registry = Registry::default();
    let mut definition = math_registry().definition("math.number").unwrap().clone();
    definition.type_id = "benchmark".into();
    definition.inputs = doc.graph().nodes().values().next().unwrap().inputs.clone();
    definition.outputs = doc.graph().nodes().values().next().unwrap().outputs.clone();
    definition
        .property_schema
        .fields
        .get_mut("value")
        .unwrap()
        .required = false;
    registry.register(definition, Arc::new(Never)).unwrap();
    let registry = Arc::new(registry);
    let mut cases = Vec::new();
    for (operation, count) in [("move", 1), ("move", 100), ("property", 1)] {
        for legacy in [true, false] {
            let validator: Arc<dyn DocumentValidator> = if legacy {
                Arc::new(Legacy(registry.clone()))
            } else {
                registry.clone()
            };
            let mut editor = Editor::new(doc.clone(), 64)
                .unwrap()
                .with_validator(validator)
                .unwrap();
            let mut raw = Vec::new();
            for step in 0..35 {
                let command = if operation == "move" {
                    Command::Batch {
                        commands: (1..=count)
                            .map(|id| {
                                let id = Id::from_u128(id);
                                let rect = doc.placement()[&id];
                                Command::MoveNode {
                                    id,
                                    rect: Rect {
                                        x: rect.x + 96.,
                                        y: rect.y + 36.,
                                        ..rect
                                    },
                                }
                            })
                            .collect(),
                    }
                } else {
                    Command::SetProperty {
                        id: Id::from_u128(1),
                        key: "value".into(),
                        value: Some(42.into()),
                    }
                };
                let (_, timing) = editor.execute_timed(command).unwrap();
                // Independent full validators outside measured intervals.
                editor.document().validate().unwrap();
                registry.validate_edit(editor.document().graph()).unwrap();
                let start = Instant::now();
                assert!(editor.undo().unwrap());
                let undo_ms = start.elapsed().as_secs_f64() * 1000.;
                assert_eq!(editor.document(), &doc);
                registry.validate_edit(editor.document().graph()).unwrap();
                if step >= 5 {
                    raw.push((timing, undo_ms));
                }
            }
            cases.push(serde_json::json!({"operation":operation,"nodes_changed":count,"legacy_full_validation":legacy,
                "execute":stats(raw.iter().map(|s|s.0.total_ms).collect()),
                "core_validation":stats(raw.iter().map(|s|s.0.core_validation_ms).collect()),
                "host_validation":stats(raw.iter().map(|s|s.0.host_validation_ms).collect()),
                "undo":stats(raw.iter().map(|s|s.1).collect()),"raw":raw}));
        }
    }
    println!("{}",serde_json::to_string_pretty(&serde_json::json!({
        "version":1,"scope":"Same binary: full-validation host wrapper vs Editor-aware Registry; CPU only, no Index/GPU/IPC",
        "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
        "nodes":10000,"edges":30000,"warmup":5,"samples":30,"cases":cases,
    })).unwrap());
}
