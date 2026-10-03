use std::{io, sync::Arc};
use unge_acx::{MemoryHost, Policy, Provider, serve};
use unge_core::{Document, Editor};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let document = match args.as_slice() {
        [] => Document::default(),
        [flag, path] if flag == "--document" => Document::from_json(&std::fs::read(path)?)?,
        _ => return Err("usage: unge-acx-provider [--document path.graph.json]".into()),
    };
    let registry = Arc::new(unge_executor::math_registry());
    let editor = Editor::new(document, 256)?.with_validator(registry.clone())?;
    let host = Arc::new(MemoryHost::from_editor(editor));
    let execution = unge_executor::RunService::new(
        registry.clone(),
        unge_executor::Scheduler::new(4, 128),
        unge_executor::RunLimits::default(),
    );
    let mut provider =
        Provider::new(host, registry, Policy::math_demo()).with_execution(execution, |_| {})?;
    serve(
        &mut provider,
        io::stdin().lock(),
        io::stdout().lock(),
        || {},
    )?;
    Ok(())
}
