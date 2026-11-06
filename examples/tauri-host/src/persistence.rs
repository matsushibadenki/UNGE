//! Host-only opt-in. Validate the log pair before exposing Engine to any view.
use std::{path::PathBuf, sync::Arc};
use unge_core::Document;
use unge_executor::Registry;
use unge_tauri::Engine;
#[derive(Default)]
pub struct Options {
    pub document: Option<PathBuf>,
    pub state: Option<PathBuf>,
    pub replay: Option<PathBuf>,
    pub stdio: bool,
}
impl Options {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut options = Self::default();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--acx-stdio" => options.stdio = true,
                "--document" | "--state-log" | "--journal" => {
                    let path = args
                        .next()
                        .filter(|s| !s.is_empty() && !s.starts_with("--"))
                        .ok_or_else(|| format!("{flag} needs a path"))?;
                    let slot = match flag.as_str() {
                        "--document" => &mut options.document,
                        "--state-log" => &mut options.state,
                        _ => &mut options.replay,
                    };
                    if slot.replace(path.into()).is_some() {
                        return Err(format!("duplicate {flag}"));
                    }
                }
                _ => return Err(format!("unknown option: {flag}")),
            }
        }
        let count = usize::from(options.document.is_some())
            + usize::from(options.state.is_some())
            + usize::from(options.replay.is_some());
        if count != 0 && (count != 3 || !options.stdio) {
            return Err("persistent mode requires --acx-stdio --document SEED --state-log STATE --journal REPLAY".into());
        }
        Ok(options)
    }
    pub fn engine(
        &self,
        seed: Document,
        registry: Arc<Registry>,
    ) -> Result<Engine, Box<dyn std::error::Error>> {
        match (&self.document, &self.state, &self.replay) {
            (Some(document), Some(state), Some(replay)) => {
                if state.try_exists()? != replay.try_exists()? {
                    return Err("restore both state/replay logs together".into());
                }
                let seed = Document::from_json(&std::fs::read(document)?)?;
                Engine::open_persistent(state, seed, registry, Default::default())
                    .map_err(|e| format!("{}: {}", e.code, e.message).into())
            }
            _ => Ok(Engine::from_registry(seed, 256, registry)?),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_require_complete_explicit_paths_and_stdio() {
        let parse = |s: &str| Options::parse(s.split_whitespace().map(str::to_owned));
        assert!(parse("").is_ok());
        assert!(parse("--acx-stdio").is_ok());
        assert!(parse("--acx-stdio --document seed --state-log state --journal replay").is_ok());
        for bad in [
            "--document seed",
            "--state-log",
            "--document --journal replay",
            "--unknown",
            "--document seed --state-log state --journal replay",
            "--acx-stdio --document seed --document seed --state-log state --journal replay",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
