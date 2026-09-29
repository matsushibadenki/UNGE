use unge_core::*;
use unge_executor::*;
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let registry = std::sync::Arc::new(math_registry());
    let mut editor = Editor::new(Document::default(), 100)?.with_validator(registry.clone())?;
    let mut a = registry.definition("math.number").unwrap().instantiate();
    let mut b = registry.definition("math.number").unwrap().instantiate();
    let sum = registry.definition("math.add").unwrap().instantiate();
    a.properties.insert("value".into(), 20.into());
    b.properties.insert("value".into(), 22.into());
    let (a_id, b_id, sum_id) = (a.id, b.id, sum.id);
    let mut commands: Vec<_> = [a, b, sum]
        .into_iter()
        .map(|node| Command::AddNode {
            node,
            rect: Rect::default(),
        })
        .collect();
    for (source, port) in [(a_id, "a"), (b_id, "b")] {
        commands.push(Command::Connect {
            edge: Edge {
                id: Id::new_v4(),
                from: Endpoint {
                    node: source,
                    port: "value".into(),
                },
                to: Endpoint {
                    node: sum_id,
                    port: port.into(),
                },
            },
        });
    }
    editor.execute(Command::Batch { commands })?;
    editor.execute(auto_layout(editor.document(), [80.0, 40.0])?)?;
    let mut scheduler = Scheduler::new(4, 128);
    let report = futures::executor::block_on(scheduler.run(
        editor.document().graph(),
        &registry,
        Cancellation::default(),
    ))?;
    assert_eq!(report.nodes[&sum_id].outputs["value"], Value::Float(42.0));
    println!("{}", serde_json::to_string_pretty(&report)?);
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(path, editor.document().to_json()?)?;
    }
    Ok(())
}
