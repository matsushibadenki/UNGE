use std::{io, sync::Arc};
use unge_acx::{MemoryHost, Policy, Provider, serve};
use unge_core::Document;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let document = match args.as_slice() {
        [] => Document::default(),
        [flag, path] if flag == "--document" => Document::from_json(&std::fs::read(path)?)?,
        _ => return Err("usage: unge-acx-provider [--document path.graph.json]".into()),
    };
    let host = Arc::new(MemoryHost::new(document)?);
    let mut provider = Provider::new(
        host,
        Arc::new(unge_executor::math_registry()),
        Policy::math_demo(),
    );
    serve(
        &mut provider,
        io::stdin().lock(),
        io::stdout().lock(),
        || {},
    )?;
    Ok(())
}
