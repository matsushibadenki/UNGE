use std::{io, sync::Arc};
use unge_acx::{MemoryHost, Policy, Provider, serve};
use unge_core::{Document, Editor};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut document_path = None;
    let mut journal_path = None;
    let mut rotate = false;
    let mut journal_limit = false;
    let mut options = unge_acx::JournalOptions::default();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--document" => document_path = Some(args.next().ok_or("--document needs a path")?),
            "--journal" => journal_path = Some(args.next().ok_or("--journal needs a path")?),
            "--journal-max-bytes" => { journal_limit = true; options.max_bytes = args.next().ok_or("--journal-max-bytes needs a size")?.parse()?; },
            "--rotate-session" => rotate = true,
            _ => return Err("usage: unge-acx-provider [--document graph.json] [--journal replay.jsonl [--journal-max-bytes N] [--rotate-session]]".into()),
        }
    }
    if journal_path.is_some() && document_path.is_none() {
        return Err("--journal requires --document with a stable Document ID; restore Document state in the host".into());
    }
    if (rotate || journal_limit) && journal_path.is_none() {
        return Err("journal options require --journal".into());
    }
    let document = match document_path {
        Some(path) => Document::from_json(&std::fs::read(path)?)?,
        None => Document::default(),
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
    if let Some(path) = journal_path {
        provider = provider.with_journal(path, options)?;
    }
    if rotate {
        provider.rotate_session()?;
    }
    serve(
        &mut provider,
        io::stdin().lock(),
        io::stdout().lock(),
        || {},
    )?;
    Ok(())
}
