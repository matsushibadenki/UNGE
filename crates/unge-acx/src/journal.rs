//! Single-writer, fsynced, hash-chained replay journal. Not a Document/Undo store.
use crate::{AcxError, Result, hash_bytes};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::Path,
};
use unge_core::Id;
use unge_executor::RunSummary;

const MAX_PAYLOAD: usize = 4 * crate::MAX_MESSAGE_BYTES;
const MAX_LINE: usize = 2 * MAX_PAYLOAD + 4096;

#[derive(Debug, Clone, Copy)]
pub struct JournalOptions {
    pub max_bytes: u64,
}
impl Default for JournalOptions {
    fn default() -> Self {
        Self {
            max_bytes: 256 * 1024 * 1024,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JournalUsage {
    /// Bytes confirmed by sync, or validated and synced when reopening.
    pub bytes: u64,
    pub max_bytes: u64,
    pub commits: usize,
    pub receipts: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    Opened {
        version: u32,
        document: Id,
        provider: String,
    },
    Rotated {
        provider: String,
    },
    Started {
        commit: String,
        provider: String,
        request_hash: String,
        authorization_hash: String,
        job: bool,
        recovery: bool,
    },
    Finished {
        commit: String,
        execution: Value,
        receipt: Value,
        job: Option<RunSummary>,
        recovery: bool,
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
#[derive(Default)]
pub(super) struct Replay {
    pub execution: Option<Value>,
    pub recovery: Option<Value>,
    pub recovery_started: bool,
    pub job: Option<RunSummary>,
    pub is_job: bool,
    pub authorization_hash: String,
    provider: String,
    request_hash: String,
}
pub(super) struct Journal {
    file: File,
    pub document: Id,
    pub provider: String,
    pub records: BTreeMap<String, Replay>,
    pub receipts: BTreeMap<String, Value>,
    bytes: u64,
    options: JournalOptions,
    sequence: u64,
    previous: String,
    poisoned: bool,
    reserved: BTreeSet<(String, bool)>,
}
fn corrupt(message: impl ToString) -> AcxError {
    AcxError::new("storage_corrupt", message)
}
fn unavailable(message: impl ToString) -> AcxError {
    AcxError::new("storage_unavailable", message)
}
fn digest(sequence: u64, previous: &str, payload: &str) -> String {
    hash_bytes(format!("{sequence}:{previous}:{payload}").as_bytes())
}
impl Journal {
    pub fn open(
        path: &Path,
        document: Id,
        provider: String,
        options: JournalOptions,
    ) -> Result<Self> {
        if options.max_bytes < 1024 {
            return Err(AcxError::new(
                "invalid_policy",
                "journal capacity must be at least 1024 bytes",
            ));
        }
        let (file, created) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (
                OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)
                    .map_err(unavailable)?,
                false,
            ),
            Err(e) => return Err(unavailable(e)),
        };
        file.try_lock().map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => {
                AcxError::new("storage_locked", "another writer owns this journal")
            }
            std::fs::TryLockError::Error(error) => unavailable(error),
        })?;
        let bytes = file.metadata().map_err(unavailable)?.len();
        if bytes > options.max_bytes {
            return Err(AcxError::new(
                "storage_full",
                "journal exceeds configured size",
            ));
        }
        let mut journal = Self {
            file,
            document,
            provider,
            records: BTreeMap::new(),
            receipts: BTreeMap::new(),
            bytes: 0,
            options,
            sequence: 0,
            previous: String::new(),
            poisoned: false,
            reserved: BTreeSet::new(),
        };
        if created {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                journal
                    .file
                    .set_permissions(std::fs::Permissions::from_mode(0o600))
                    .map_err(unavailable)?;
            }
            journal.append(
                Event::Opened {
                    version: 1,
                    document,
                    provider: journal.provider.clone(),
                },
                None,
                None,
            )?;
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            #[cfg(unix)]
            File::open(parent)
                .and_then(|f| f.sync_all())
                .map_err(unavailable)?;
            #[cfg(not(unix))]
            let _ = parent;
        } else {
            if bytes == 0 {
                return Err(corrupt("existing journal is empty"));
            }
            let mut reader = BufReader::new(journal.file.try_clone().map_err(unavailable)?);
            loop {
                let mut line = Vec::new();
                let read = std::io::Read::take(&mut reader, MAX_LINE as u64 + 1)
                    .read_until(b'\n', &mut line)
                    .map_err(unavailable)?;
                if read == 0 {
                    break;
                }
                if read > MAX_LINE || line.last() != Some(&b'\n') {
                    return Err(corrupt("oversized or incomplete journal frame"));
                }
                let frame: Frame = serde_json::from_slice(&line).map_err(corrupt)?;
                if frame.sequence != journal.sequence
                    || frame.previous != journal.previous
                    || frame.hash != digest(frame.sequence, &frame.previous, &frame.payload)
                    || frame.payload.len() > MAX_PAYLOAD
                {
                    return Err(corrupt("journal chain mismatch"));
                }
                let event: Event = serde_json::from_str(&frame.payload).map_err(corrupt)?;
                journal.validate(&event)?;
                journal.apply(event);
                journal.sequence += 1;
                journal.previous = frame.hash;
                journal.bytes += read as u64;
            }
            if journal.document != document {
                return Err(AcxError::new(
                    "document_mismatch",
                    "journal belongs to another Document",
                ));
            }
        }
        journal.file.seek(SeekFrom::End(0)).map_err(unavailable)?;
        // A previous writer may have written a valid frame but failed to sync it.
        // Confirm the validated prefix before serving any restored result.
        journal.file.sync_all().map_err(unavailable)?;
        Ok(journal)
    }
    pub fn check(&self) -> Result<()> {
        if self.poisoned {
            Err(unavailable(
                "journal write failed; stop effects and inspect storage",
            ))
        } else {
            Ok(())
        }
    }
    pub fn usage(&self) -> JournalUsage {
        JournalUsage {
            bytes: self.bytes,
            max_bytes: self.options.max_bytes,
            commits: self.records.len(),
            receipts: self.receipts.len(),
        }
    }
    fn validate(&self, event: &Event) -> Result<()> {
        if self.sequence == 0 {
            return match event {
                Event::Opened { version: 1, .. } => Ok(()),
                _ => Err(corrupt("missing or unsupported journal header")),
            };
        }
        match event {
            Event::Opened { .. } => Err(corrupt("duplicate header")),
            Event::Rotated { provider } if provider.is_empty() || !self.reserved.is_empty() => {
                Err(corrupt("invalid rotation"))
            }
            Event::Rotated { .. } => Ok(()),
            Event::Started {
                commit,
                provider,
                authorization_hash,
                recovery,
                request_hash,
                job,
            } => {
                if provider != &self.provider || authorization_hash.is_empty() {
                    return Err(corrupt("invalid operation identity"));
                }
                match self.records.get(commit) {
                    None if !recovery => Ok(()),
                    Some(r)
                        if *recovery
                            && r.execution.is_some()
                            && !r.recovery_started
                            && !r.is_job
                            && !job
                            && authorization_hash == &r.authorization_hash
                            && request_hash == &r.request_hash =>
                    {
                        Ok(())
                    }
                    _ => Err(corrupt("duplicate or out-of-order start")),
                }
            }
            Event::Finished {
                commit,
                execution,
                receipt,
                recovery,
                job,
            } => {
                let r = self
                    .records
                    .get(commit)
                    .ok_or_else(|| corrupt("finish without start"))?;
                if (*recovery && (!r.recovery_started || r.recovery.is_some()))
                    || (!recovery && r.execution.is_some())
                {
                    return Err(corrupt("duplicate finish"));
                }
                let raw = execution["resultJson"]
                    .as_str()
                    .ok_or_else(|| corrupt("missing result bytes"))?;
                let result: Value = serde_json::from_str(raw).map_err(corrupt)?;
                let id = receipt["receiptId"]
                    .as_str()
                    .ok_or_else(|| corrupt("missing receipt ID"))?;
                if result != execution["result"]
                    || execution["receiptId"] != id
                    || self.receipts.contains_key(id)
                    || receipt["resultHash"] != hash_bytes(raw.as_bytes())
                    || receipt["requestHash"] != r.request_hash
                    || (!recovery && receipt["provider"] != r.provider)
                    || (*recovery && receipt["provider"] != self.provider)
                    || (!recovery && r.is_job != job.is_some())
                    || (*recovery && job.is_some())
                {
                    return Err(corrupt("invalid replay evidence"));
                }
                Ok(())
            }
        }
    }
    fn apply(&mut self, event: Event) {
        match event {
            Event::Opened {
                document, provider, ..
            } => {
                self.document = document;
                self.provider = provider;
            }
            Event::Rotated { provider } => self.provider = provider,
            Event::Started {
                commit,
                provider,
                request_hash,
                authorization_hash,
                job,
                recovery,
            } => {
                if recovery {
                    self.records.get_mut(&commit).unwrap().recovery_started = true;
                } else {
                    self.records.insert(
                        commit,
                        Replay {
                            provider,
                            request_hash,
                            authorization_hash,
                            is_job: job,
                            ..Replay::default()
                        },
                    );
                }
            }
            Event::Finished {
                commit,
                execution,
                receipt,
                job,
                recovery,
            } => {
                let r = self.records.get_mut(&commit).unwrap();
                if recovery {
                    r.recovery = Some(execution);
                } else {
                    r.execution = Some(execution);
                    r.job = job;
                }
                self.receipts
                    .insert(receipt["receiptId"].as_str().unwrap().into(), receipt);
            }
        }
    }
    fn append(
        &mut self,
        event: Event,
        reserve: Option<(String, bool)>,
        release: Option<(String, bool)>,
    ) -> Result<()> {
        self.check()?;
        self.validate(&event)?;
        let payload = serde_json::to_string(&event).map_err(unavailable)?;
        if payload.len() > MAX_PAYLOAD {
            self.poisoned = true;
            return Err(unavailable("record exceeds journal frame limit"));
        }
        let hash = digest(self.sequence, &self.previous, &payload);
        let frame = Frame {
            sequence: self.sequence,
            previous: self.previous.clone(),
            payload,
            hash: hash.clone(),
        };
        let mut bytes = serde_json::to_vec(&frame).map_err(unavailable)?;
        bytes.push(b'\n');
        let held = self.reserved.len() + usize::from(reserve.is_some())
            - usize::from(release.as_ref().is_some_and(|r| self.reserved.contains(r)));
        if self
            .bytes
            .saturating_add(bytes.len() as u64)
            .saturating_add(held as u64 * MAX_LINE as u64)
            > self.options.max_bytes
        {
            return Err(AcxError::new(
                "storage_full",
                "journal is full; preserve history and provision more capacity",
            ));
        }
        if let Err(e) = self
            .file
            .write_all(&bytes)
            .and_then(|()| self.file.sync_all())
        {
            self.poisoned = true;
            return Err(unavailable(e));
        }
        self.bytes += bytes.len() as u64;
        self.sequence += 1;
        self.previous = hash;
        if let Some(key) = reserve {
            self.reserved.insert(key);
        }
        if let Some(key) = release {
            self.reserved.remove(&key);
        }
        self.apply(event);
        Ok(())
    }
    pub fn begin(
        &mut self,
        commit: &str,
        request_hash: &str,
        authorization_hash: &str,
        job: bool,
        recovery: bool,
    ) -> Result<()> {
        self.append(
            Event::Started {
                commit: commit.into(),
                provider: self.provider.clone(),
                request_hash: request_hash.into(),
                authorization_hash: authorization_hash.into(),
                job,
                recovery,
            },
            Some((commit.into(), recovery)),
            None,
        )
    }
    pub fn finish(
        &mut self,
        commit: &str,
        execution: Value,
        receipt: Value,
        job: Option<RunSummary>,
        recovery: bool,
    ) -> Result<()> {
        self.append(
            Event::Finished {
                commit: commit.into(),
                execution,
                receipt,
                job,
                recovery,
            },
            None,
            Some((commit.into(), recovery)),
        )
    }
    pub fn rotate(&mut self, provider: String) -> Result<()> {
        self.check()?;
        if !self.reserved.is_empty() {
            return Err(AcxError::new(
                "execution_busy",
                "durable operation is still pending",
            ));
        }
        self.append(Event::Rotated { provider }, None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("unge-log-{}", Id::new_v4()))
    }
    fn start(j: &mut Journal, id: &str) -> Result<()> {
        j.begin(id, "request", "authorization", false, false)
    }
    #[test]
    #[cfg(unix)]
    fn failed_start_write_poison_stops_further_admission() {
        let path = path();
        let doc = Id::new_v4();
        let mut j =
            Journal::open(&path, doc, "provider".into(), JournalOptions::default()).unwrap();
        let before = std::fs::read(&path).unwrap();
        j.file = File::open(&path).unwrap();
        j.file.try_lock().unwrap();
        assert_eq!(
            start(&mut j, "first").unwrap_err().code,
            "storage_unavailable"
        );
        assert_eq!(
            start(&mut j, "next").unwrap_err().code,
            "storage_unavailable"
        );
        assert!(j.records.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(j);
        let j = Journal::open(&path, doc, "unused".into(), JournalOptions::default()).unwrap();
        assert!(j.records.is_empty());
        drop(j);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    #[cfg(unix)]
    fn failed_finish_write_retains_only_the_durable_start() {
        let path = path();
        let doc = Id::new_v4();
        let mut j =
            Journal::open(&path, doc, "provider".into(), JournalOptions::default()).unwrap();
        start(&mut j, "commit").unwrap();
        j.file = File::open(&path).unwrap();
        j.file.try_lock().unwrap();
        let execution = serde_json::json!({"result":{},"resultJson":"{}","receiptId":"receipt"});
        let receipt = serde_json::json!({"receiptId":"receipt","resultHash":hash_bytes(b"{}"),"requestHash":"request","provider":"provider"});
        assert_eq!(
            j.finish("commit", execution, receipt, None, false)
                .unwrap_err()
                .code,
            "storage_unavailable"
        );
        assert!(j.records["commit"].execution.is_none());
        assert!(j.receipts.is_empty());
        drop(j);
        let j = Journal::open(&path, doc, "unused".into(), JournalOptions::default()).unwrap();
        assert!(j.records["commit"].execution.is_none());
        assert!(j.receipts.is_empty());
        drop(j);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn checksum_and_unknown_version_rejected_without_repairing_history() {
        let path = path();
        let doc = Id::new_v4();
        let j = Journal::open(&path, doc, "provider".into(), JournalOptions::default()).unwrap();
        drop(j);
        let before = std::fs::read(&path).unwrap();
        let mut frame: Frame = serde_json::from_slice(&before).unwrap();
        frame.payload = frame.payload.replace("provider", "changed");
        let mut bytes = serde_json::to_vec(&frame).unwrap();
        bytes.push(b'\n');
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            Journal::open(&path, doc, "unused".into(), JournalOptions::default())
                .err()
                .unwrap()
                .code,
            "storage_corrupt"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let event = Event::Opened {
            version: 2,
            document: doc,
            provider: "provider".into(),
        };
        let payload = serde_json::to_string(&event).unwrap();
        let frame = Frame {
            sequence: 0,
            previous: String::new(),
            hash: digest(0, "", &payload),
            payload,
        };
        let mut bytes = serde_json::to_vec(&frame).unwrap();
        bytes.push(b'\n');
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            Journal::open(&path, doc, "unused".into(), JournalOptions::default())
                .err()
                .unwrap()
                .code,
            "storage_corrupt"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
    }
}
