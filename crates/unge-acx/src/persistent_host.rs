//! Durable standalone editor. Receipt replay remains the Provider journal's responsibility.
use crate::{
    AcxError, GraphHost, HostPersistence, LogCheckpoint, Result, Snapshot, check_snapshot,
    hash_bytes,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{Arc, Mutex},
};
use unge_core::{Command, Document, DocumentValidator, Editor, HistoryLimits, Id};

const MAX_FRAME: usize = 16 * 1024 * 1024;

/// Host-owned limits, never supplied through agent requests.
#[derive(Debug, Clone, Copy)]
pub struct PersistentHostOptions {
    pub max_bytes: u64,
    pub history: HistoryLimits,
}
impl Default for PersistentHostOptions {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
            history: HistoryLimits::default(),
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    ReplayAcknowledged {
        checkpoint: LogCheckpoint,
    },
    Opened {
        version: u32,
        document: Document,
        max_steps: usize,
        history_bytes: usize,
    },
    Apply {
        expected_revision: u64,
        command: Command,
    },
    Undo {
        expected_revision: u64,
    },
    Redo {
        expected_revision: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    sequence: u64,
    previous: String,
    payload: String,
    hash: String,
}
struct State {
    file: File,
    editor: Editor,
    bytes: u64,
    sequence: u64,
    previous: String,
    max_bytes: u64,
    poisoned: bool,
    prefixes: Vec<String>,
    replay: Option<LogCheckpoint>,
}
/// One authoritative Rust editor protected by a single lock and a durable command log.
/// Replay executes core Commands only, never NodeExecutors. The log restores revision,
/// Undo/Redo and their original budgets; it does not store external resources or receipts.
pub struct PersistentHost {
    state: Mutex<State>,
}
fn storage(error: impl ToString) -> AcxError {
    AcxError::new("storage_unavailable", error)
}
fn corrupt(error: impl ToString) -> AcxError {
    AcxError::new("storage_corrupt", error)
}
fn snapshot(editor: &Editor) -> Snapshot {
    Snapshot {
        document: editor.document().clone(),
        revision: editor.revision(),
    }
}
fn digest(sequence: u64, previous: &str, payload: &str) -> String {
    hash_bytes(format!("{sequence}:{previous}:{payload}").as_bytes())
}
fn edit(editor: &mut Editor, event: &Event) -> Result<()> {
    let expected = match event {
        Event::Apply {
            expected_revision, ..
        }
        | Event::Undo { expected_revision }
        | Event::Redo { expected_revision } => *expected_revision,
        _ => return Err(corrupt("unexpected editor header")),
    };
    if expected != editor.revision() {
        return Err(AcxError::new(
            "revision_conflict",
            "editor revision changed",
        ));
    }
    match event {
        Event::Apply { command, .. } => {
            editor.execute(command.clone())?;
        }
        Event::Undo { .. } if !editor.undo()? => {
            return Err(AcxError::new(
                "recovery_unavailable",
                "undo history unavailable",
            ));
        }
        Event::Redo { .. } if !editor.redo()? => {
            return Err(AcxError::new(
                "recovery_unavailable",
                "redo history unavailable",
            ));
        }
        _ => {}
    }
    Ok(())
}
impl PersistentHost {
    /// `initial` seeds a new file. Existing files restore their own state and must
    /// have the same Document ID and history limits. Install a deterministic validator.
    pub fn open(
        path: impl AsRef<Path>,
        initial: Document,
        validator: Arc<dyn DocumentValidator>,
        options: PersistentHostOptions,
    ) -> Result<Self> {
        if options.max_bytes < 1024 {
            return Err(AcxError::new(
                "invalid_policy",
                "editor log capacity must be at least 1024 bytes",
            ));
        }
        let path = path.as_ref();
        let mut editor = Editor::with_history_limits(initial.clone(), options.history)?
            .with_validator(validator.clone())?;
        let mut create_options = OpenOptions::new();
        create_options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            create_options.mode(0o600);
        }
        let (file, created) = match create_options.open(path) {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .map_err(storage)?,
                false,
            ),
            Err(e) => return Err(storage(e)),
        };
        file.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => {
                AcxError::new("storage_locked", "another writer owns this editor log")
            }
            std::fs::TryLockError::Error(e) => storage(e),
        })?;
        let bytes = file.metadata().map_err(storage)?.len();
        if bytes > options.max_bytes {
            return Err(AcxError::new(
                "storage_full",
                "editor log exceeds configured capacity",
            ));
        }
        let mut state = State {
            file,
            editor: editor.clone(),
            bytes: 0,
            sequence: 0,
            previous: String::new(),
            max_bytes: options.max_bytes,
            poisoned: false,
            prefixes: Vec::new(),
            replay: None,
        };
        if created {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                state
                    .file
                    .set_permissions(std::fs::Permissions::from_mode(0o600))
                    .map_err(storage)?;
            }
            state.append(&Event::Opened {
                version: 1,
                document: initial,
                max_steps: options.history.max_steps,
                history_bytes: options.history.max_bytes,
            })?;
            #[cfg(unix)]
            {
                let parent = path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .unwrap_or(Path::new("."));
                File::open(parent)
                    .and_then(|f| f.sync_all())
                    .map_err(storage)?;
            }
        } else {
            if bytes == 0 {
                return Err(corrupt("existing editor log is empty"));
            }
            let mut reader = BufReader::new(state.file.try_clone().map_err(storage)?);
            loop {
                let mut line = Vec::new();
                let read = (&mut reader)
                    .take(MAX_FRAME as u64 + 1)
                    .read_until(b'\n', &mut line)
                    .map_err(storage)?;
                if read == 0 {
                    break;
                }
                if read > MAX_FRAME || line.last() != Some(&b'\n') {
                    return Err(corrupt("oversized or incomplete editor frame"));
                }
                let frame: Frame = serde_json::from_slice(&line).map_err(corrupt)?;
                if frame.sequence != state.sequence
                    || frame.previous != state.previous
                    || frame.hash != digest(frame.sequence, &frame.previous, &frame.payload)
                {
                    return Err(corrupt("editor log chain mismatch"));
                }
                let event: Event = serde_json::from_str(&frame.payload).map_err(corrupt)?;
                if state.sequence == 0 {
                    match event {
                        Event::Opened {
                            version: 1,
                            document,
                            max_steps,
                            history_bytes,
                        } => {
                            if document.graph().id != initial.graph().id {
                                return Err(AcxError::new(
                                    "document_mismatch",
                                    "editor log belongs to another Document",
                                ));
                            }
                            if max_steps != options.history.max_steps
                                || history_bytes != options.history.max_bytes
                            {
                                return Err(AcxError::new(
                                    "invalid_policy",
                                    "restore the original editor history limits",
                                ));
                            }
                            editor = Editor::with_history_limits(document, options.history)
                                .and_then(|e| e.with_validator(validator.clone()))
                                .map_err(corrupt)?;
                        }
                        _ => return Err(corrupt("missing or unsupported editor header")),
                    }
                } else if let Event::ReplayAcknowledged { checkpoint } = event {
                    state.validate_replay(&checkpoint)?;
                    state.replay = Some(checkpoint);
                } else {
                    edit(&mut editor, &event).map_err(corrupt)?;
                }
                state.sequence += 1;
                state.prefixes.push(frame.hash.clone());
                state.previous = frame.hash;
                state.bytes += read as u64;
            }
            state.editor = editor;
        }
        state.file.seek(SeekFrom::End(0)).map_err(storage)?;
        state.file.sync_all().map_err(storage)?;
        Ok(Self {
            state: Mutex::new(state),
        })
    }
    fn transact(&self, document: Id, expected_revision: u64, event: Event) -> Result<Snapshot> {
        let mut state = self.state.lock().map_err(storage)?;
        state.check()?;
        check_snapshot(&snapshot(&state.editor), document, expected_revision)?;
        // Validate a private candidate; neither an invalid command nor failed save
        // can publish a new revision/history to readers of the authoritative editor.
        let mut candidate = state.editor.clone();
        edit(&mut candidate, &event)?;
        state.append(&event)?;
        state.editor = candidate;
        Ok(snapshot(&state.editor))
    }
    /// Trusted-host redo, subject to the same revision and durability checks.
    pub fn redo(&self, document: Id, expected_revision: u64) -> Result<Snapshot> {
        self.transact(
            document,
            expected_revision,
            Event::Redo { expected_revision },
        )
    }
}
impl State {
    fn checkpoint(&self) -> LogCheckpoint {
        LogCheckpoint {
            identity: self.prefixes[0].clone(),
            sequence: self.sequence,
            hash: self.previous.clone(),
        }
    }
    fn validate_replay(&self, checkpoint: &LogCheckpoint) -> Result<()> {
        if checkpoint.sequence == 0
            || checkpoint.identity.is_empty()
            || checkpoint.hash.is_empty()
            || self.replay.as_ref().is_some_and(|old| {
                old.identity != checkpoint.identity
                    || old.sequence > checkpoint.sequence
                    || (old.sequence == checkpoint.sequence && old.hash != checkpoint.hash)
            })
        {
            return Err(corrupt("replay identity or prefix moved backwards"));
        }
        Ok(())
    }

    fn check(&self) -> Result<()> {
        if self.poisoned {
            Err(storage("editor save failed; stop and inspect storage"))
        } else {
            Ok(())
        }
    }
    fn append(&mut self, event: &Event) -> Result<()> {
        self.check()?;
        let payload = serde_json::to_string(event).map_err(storage)?;
        let hash = digest(self.sequence, &self.previous, &payload);
        let frame = Frame {
            sequence: self.sequence,
            previous: self.previous.clone(),
            payload,
            hash: hash.clone(),
        };
        let mut bytes = serde_json::to_vec(&frame).map_err(storage)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_FRAME || self.bytes.saturating_add(bytes.len() as u64) > self.max_bytes
        {
            return Err(AcxError::new(
                "storage_full",
                "editor log capacity exhausted; preserve existing history",
            ));
        }
        if let Err(e) = self
            .file
            .write_all(&bytes)
            .and_then(|()| self.file.sync_all())
        {
            self.poisoned = true;
            return Err(storage(e));
        }
        self.bytes += bytes.len() as u64;
        self.sequence += 1;
        self.prefixes.push(hash.clone());
        self.previous = hash;
        Ok(())
    }
}
impl GraphHost for PersistentHost {
    fn persistence(&self) -> Result<Option<HostPersistence>> {
        let state = self.state.lock().map_err(storage)?;
        state.check()?;
        Ok(Some(HostPersistence {
            checkpoint: state.checkpoint(),
            replay: state.replay.clone(),
        }))
    }
    fn verify_persistence(&self, checkpoint: &LogCheckpoint) -> Result<()> {
        let state = self.state.lock().map_err(storage)?;
        state.check()?;
        let index = checkpoint
            .sequence
            .checked_sub(1)
            .and_then(|i| usize::try_from(i).ok());
        if checkpoint.identity != state.prefixes[0]
            || index.and_then(|i| state.prefixes.get(i)) != Some(&checkpoint.hash)
        {
            return Err(corrupt(
                "state log is from another generation or was rolled back",
            ));
        }
        Ok(())
    }
    fn acknowledge_replay(&self, checkpoint: LogCheckpoint) -> Result<()> {
        let mut state = self.state.lock().map_err(storage)?;
        state.check()?;
        state.validate_replay(&checkpoint)?;
        if state.replay.as_ref() == Some(&checkpoint) {
            return Ok(());
        }
        state.append(&Event::ReplayAcknowledged {
            checkpoint: checkpoint.clone(),
        })?;
        state.replay = Some(checkpoint);
        Ok(())
    }

    fn snapshot(&self) -> Result<Snapshot> {
        let state = self.state.lock().map_err(storage)?;
        state.check()?;
        Ok(snapshot(&state.editor))
    }
    fn apply(&self, document: Id, expected_revision: u64, command: Command) -> Result<Snapshot> {
        self.transact(
            document,
            expected_revision,
            Event::Apply {
                expected_revision,
                command,
            },
        )
    }
    fn undo(&self, document: Id, expected_revision: u64) -> Result<Snapshot> {
        self.transact(
            document,
            expected_revision,
            Event::Undo { expected_revision },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(unix)]
    fn failed_write_never_publishes_staged_editor_and_poison_stops_reads() {
        let path = std::env::temp_dir().join(format!("unge-state-fault-{}", Id::new_v4()));
        let doc = Document::default();
        let host = PersistentHost::open(
            &path,
            doc.clone(),
            Arc::new(unge_executor::math_registry()),
            Default::default(),
        )
        .unwrap();
        let before = std::fs::read(&path).unwrap();
        {
            let mut state = host.state.lock().unwrap();
            state.file = File::open(&path).unwrap();
            state.file.try_lock().unwrap();
        }
        let command = Command::AddNode {
            node: unge_executor::math_registry()
                .definition("math.number")
                .unwrap()
                .instantiate(),
            rect: unge_core::Rect::default(),
        };
        assert_eq!(
            host.apply(doc.graph().id, 0, command).unwrap_err().code,
            "storage_unavailable"
        );
        assert_eq!(host.state.lock().unwrap().editor.revision(), 0);
        assert_eq!(host.snapshot().unwrap_err().code, "storage_unavailable");
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(host);
        let host = PersistentHost::open(
            &path,
            doc.clone(),
            Arc::new(unge_executor::math_registry()),
            Default::default(),
        )
        .unwrap();
        assert_eq!(host.snapshot().unwrap().document, doc);
        drop(host);
        std::fs::remove_file(path).unwrap();
    }
}
