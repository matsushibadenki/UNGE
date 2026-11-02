use std::{io, sync::Arc};
use unge_acx::{
    GraphHost, MemoryHost, PersistentHost, PersistentHostOptions, Policy, Provider, serve,
};
use unge_core::{Document, Editor};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut document_path = None;
    let mut journal_path = None;
    let mut state_path = None;
    let mut state_limit = false;
    let mut state_options = PersistentHostOptions::default();
    let mut rotate = false;
    let mut journal_limit = false;
    let mut options = unge_acx::JournalOptions::default();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--document" => document_path = Some(args.next().ok_or("--document needs a path")?),
            "--state-log" => state_path = Some(args.next().ok_or("--state-log needs a path")?),
            "--journal" => journal_path = Some(args.next().ok_or("--journal needs a path")?),
            "--state-max-bytes" => { state_limit = true; state_options.max_bytes = args.next().ok_or("--state-max-bytes needs a size")?.parse()?; },
            "--journal-max-bytes" => { journal_limit = true; options.max_bytes = args.next().ok_or("--journal-max-bytes needs a size")?.parse()?; },
            "--rotate-session" => rotate = true,
            _ => return Err("usage: unge-acx-provider [--document graph.json] [--journal replay.jsonl [--state-log editor.jsonl [--state-max-bytes N]] [--journal-max-bytes N] [--rotate-session]]".into()),
        }
    }
    if state_limit && state_path.is_none() {
        return Err("--state-max-bytes requires --state-log".into());
    }
    if state_path.is_some() && journal_path.is_none() {
        return Err("--state-log requires --journal to preserve commit replay guards".into());
    }
    if journal_path.is_some() && document_path.is_none() {
        return Err("--journal requires --document with a stable Document ID; restore Document state in the host".into());
    }
    if (rotate || journal_limit) && journal_path.is_none() {
        return Err("journal options require --journal".into());
    }
    if let (Some(state), Some(journal)) = (&state_path, &journal_path)
        && std::path::Path::new(state).try_exists()?
            != std::path::Path::new(journal).try_exists()?
    {
        return Err(
            "state/replay logs must both exist or both be new; restore the pair together".into(),
        );
    }
    let document = match document_path {
        Some(path) => Document::from_json(&std::fs::read(path)?)?,
        None => Document::default(),
    };
    let registry = Arc::new(unge_executor::math_registry());
    let host: Arc<dyn GraphHost> = if let Some(path) = state_path {
        Arc::new(PersistentHost::open(
            path,
            document,
            registry.clone(),
            state_options,
        )?)
    } else {
        let editor = Editor::new(document, 256)?.with_validator(registry.clone())?;
        Arc::new(MemoryHost::from_editor(editor))
    };
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
