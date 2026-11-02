use serde_json::{Value, json};
use std::{fs, path::PathBuf, sync::Arc};
use unge_acx::*;
use unge_core::{Command, Document, Id, Rect};
use unge_executor::math_registry;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("unge-editor-{}", Id::new_v4()));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn state(&self) -> PathBuf {
        self.0.join("state.jsonl")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn open(temp: &Temp, doc: &Document) -> PersistentHost {
    PersistentHost::open(
        temp.state(),
        doc.clone(),
        Arc::new(math_registry()),
        PersistentHostOptions::default(),
    )
    .unwrap()
}
fn command() -> Command {
    Command::AddNode {
        node: math_registry()
            .definition("math.number")
            .unwrap()
            .instantiate(),
        rect: Rect::default(),
    }
}
#[test]
fn restart_restores_document_revision_undo_redo_and_history_barriers() {
    let t = Temp::new();
    let doc = Document::default();
    let id = doc.graph().id;
    let h = open(&t, &doc);
    let edited = h.apply(id, 0, command()).unwrap();
    assert_eq!(
        h.apply(id, 0, command()).unwrap_err().code,
        "revision_conflict"
    );
    drop(h);
    let h = open(&t, &doc);
    assert_eq!(h.snapshot().unwrap().document, edited.document);
    assert_eq!(h.snapshot().unwrap().revision, 1);
    assert_eq!(h.undo(id, 1).unwrap().document, doc);
    drop(h);
    let h = open(&t, &doc);
    assert_eq!(h.snapshot().unwrap().revision, 2);
    assert_eq!(h.redo(id, 2).unwrap().document, edited.document);
    drop(h);
    let h = open(&t, &doc);
    assert_eq!(h.snapshot().unwrap().revision, 3);
    assert_eq!(h.undo(id, 3).unwrap().document, doc);
    h.apply(id, 4, command()).unwrap();
    assert_eq!(h.redo(id, 5).unwrap_err().code, "recovery_unavailable");
    drop(h);
    let options = PersistentHostOptions {
        history: unge_core::HistoryLimits {
            max_steps: 1,
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(
        PersistentHost::open(t.state(), doc, Arc::new(math_registry()), options)
            .err()
            .unwrap()
            .code,
        "invalid_policy"
    );
}
#[test]
fn invalid_edit_capacity_and_corruption_never_publish_or_repair() {
    let t = Temp::new();
    let doc = Document::default();
    let id = doc.graph().id;
    let h = open(&t, &doc);
    let bytes = fs::read(t.state()).unwrap();
    let mut node = math_registry()
        .definition("math.number")
        .unwrap()
        .instantiate();
    node.properties.insert("value".into(), json!("bad"));
    assert_eq!(
        h.apply(
            id,
            0,
            Command::AddNode {
                node,
                rect: Rect::default()
            }
        )
        .unwrap_err()
        .code,
        "invalid_properties"
    );
    assert_eq!(h.snapshot().unwrap().revision, 0);
    assert_eq!(fs::read(t.state()).unwrap(), bytes);
    assert_eq!(
        PersistentHost::open(
            t.state(),
            doc.clone(),
            Arc::new(math_registry()),
            Default::default()
        )
        .err()
        .unwrap()
        .code,
        "storage_locked"
    );
    drop(h);
    let options = PersistentHostOptions {
        max_bytes: 1024,
        ..Default::default()
    };
    let h =
        PersistentHost::open(t.state(), doc.clone(), Arc::new(math_registry()), options).unwrap();
    let oversized = Command::Batch {
        commands: (0..20).map(|_| command()).collect(),
    };
    assert_eq!(h.apply(id, 0, oversized).unwrap_err().code, "storage_full");
    assert_eq!(h.snapshot().unwrap().revision, 0);
    drop(h);
    assert_eq!(fs::read(t.state()).unwrap(), bytes);
    assert_eq!(
        PersistentHost::open(
            t.state(),
            Document::default(),
            Arc::new(math_registry()),
            Default::default()
        )
        .err()
        .unwrap()
        .code,
        "document_mismatch"
    );
    let mut broken = bytes.clone();
    broken.push(b'{');
    fs::write(t.state(), &broken).unwrap();
    assert_eq!(
        PersistentHost::open(
            t.state(),
            doc,
            Arc::new(math_registry()),
            Default::default()
        )
        .err()
        .unwrap()
        .code,
        "storage_corrupt"
    );
    assert_eq!(fs::read(t.state()).unwrap(), broken);
}
fn plan(p: &mut Provider, h: &dyn GraphHost) -> String {
    let s = h.snapshot().unwrap();
    let input = json!({"kind":"edit","document_id":s.document.graph().id,"expected_revision":s.revision,"operations":[{"kind":"create_node","id":Id::new_v4(),"type_id":"math.number","properties":{"value":42},"rect":{"x":0,"y":0,"width":180,"height":90}}]});
    let pf = p.dispatch("preflight", json!({"input":input})).unwrap();
    let bound = json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]});
    let a = p.dispatch("authorize", bound.clone()).unwrap();
    let c = p.dispatch("commit", json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"input":pf["input"],"authorization":a["authorization"]})).unwrap();
    c["commitId"].as_str().unwrap().into()
}
fn provider(t: &Temp, h: Arc<dyn GraphHost>) -> Provider {
    Provider::new(h, Arc::new(math_registry()), Policy::math_demo())
        .with_journal(t.0.join("replay.jsonl"), Default::default())
        .unwrap()
}
#[test]
fn receipts_and_saved_state_survive_restart_rotation_and_trusted_undo() {
    let t = Temp::new();
    let doc = Document::default();
    let id = doc.graph().id;
    let h = Arc::new(open(&t, &doc));
    let mut p = provider(&t, h.clone());
    let commit = plan(&mut p, h.as_ref());
    let args = json!({"commitId":commit});
    let result = p.dispatch("execute", args.clone()).unwrap();
    let receipt_args = json!({"receiptId":result["receiptId"]});
    let evidence = p.dispatch("receipt", receipt_args.clone()).unwrap();
    let saved = h.snapshot().unwrap();
    drop(p);
    drop(h);
    let h = Arc::new(open(&t, &doc));
    let mut p = provider(&t, h.clone());
    p.rotate_session().unwrap();
    assert_eq!(p.dispatch("execute", args.clone()).unwrap(), result);
    assert_eq!(
        p.dispatch("receipt", receipt_args.clone()).unwrap(),
        evidence
    );
    assert_eq!(h.snapshot().unwrap().document, saved.document);
    assert_eq!(h.snapshot().unwrap().revision, 1);
    assert_eq!(h.undo(id, 1).unwrap().document, doc);
    drop(p);
    drop(h);
    let h = Arc::new(open(&t, &doc));
    let mut p = provider(&t, h.clone());
    assert_eq!(p.dispatch("execute", args).unwrap(), result);
    assert_eq!(p.dispatch("receipt", receipt_args).unwrap(), evidence);
    assert_eq!(h.snapshot().unwrap().revision, 2);
    assert_eq!(h.snapshot().unwrap().document, doc);
}
struct CrashAfterSave(PersistentHost);
impl GraphHost for CrashAfterSave {
    fn persistence(&self) -> Result<Option<HostPersistence>> {
        self.0.persistence()
    }
    fn verify_persistence(&self, checkpoint: &LogCheckpoint) -> Result<()> {
        self.0.verify_persistence(checkpoint)
    }
    fn acknowledge_replay(&self, checkpoint: LogCheckpoint) -> Result<()> {
        self.0.acknowledge_replay(checkpoint)
    }

    fn snapshot(&self) -> Result<Snapshot> {
        self.0.snapshot()
    }
    fn apply(&self, id: Id, revision: u64, command: Command) -> Result<Snapshot> {
        self.0.apply(id, revision, command)?;
        std::process::exit(42)
    }
    fn undo(&self, id: Id, revision: u64) -> Result<Snapshot> {
        self.0.undo(id, revision)
    }
}
#[test]
fn process_crash_after_saved_effect_before_receipt_never_repeats_edit() {
    let t = Temp::new();
    let doc = Document::default();
    fs::write(t.0.join("seed.json"), doc.to_json().unwrap()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "saved_effect_crash_worker", "--ignored"])
        .env("UNGE_STATE_CRASH_DIR", &t.0)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(42));
    let h = Arc::new(open(&t, &doc));
    let mut p = provider(&t, h.clone());
    let commit: Value =
        serde_json::from_slice(&fs::read(t.0.join("commit.json")).unwrap()).unwrap();
    for _ in 0..3 {
        assert_eq!(
            p.dispatch("execute", commit.clone()).unwrap_err().code,
            "execution_uncertain"
        );
    }
    assert_eq!(h.snapshot().unwrap().revision, 1);
    assert_eq!(h.snapshot().unwrap().document.graph().nodes().len(), 1);
    assert_eq!(h.undo(doc.graph().id, 1).unwrap().document, doc);
}
#[test]
#[ignore = "subprocess entry invoked by parent regression"]
fn saved_effect_crash_worker() {
    let Some(dir) = std::env::var_os("UNGE_STATE_CRASH_DIR") else {
        return;
    };
    let t = Temp(PathBuf::from(dir));
    let doc = Document::from_json(&fs::read(t.0.join("seed.json")).unwrap()).unwrap();
    let h = Arc::new(CrashAfterSave(open(&t, &doc)));
    let mut p = provider(&t, h.clone());
    let commit = plan(&mut p, h.as_ref());
    let args = json!({"commitId":commit});
    let file = fs::File::create(t.0.join("commit.json")).unwrap();
    serde_json::to_writer(&file, &args).unwrap();
    file.sync_all().unwrap();
    p.dispatch("execute", args).unwrap();
    panic!("worker did not exit");
}

#[test]
fn restored_history_limits_preserve_eviction_and_oversized_step_barriers() {
    for (max_steps, max_bytes) in [(1, 1024 * 1024), (256, 1)] {
        let t = Temp::new();
        let doc = Document::default();
        let id = doc.graph().id;
        let options = PersistentHostOptions {
            history: unge_core::HistoryLimits {
                max_steps,
                max_bytes,
            },
            ..Default::default()
        };
        let h = PersistentHost::open(t.state(), doc.clone(), Arc::new(math_registry()), options)
            .unwrap();
        h.apply(id, 0, command()).unwrap();
        h.apply(id, 1, command()).unwrap();
        drop(h);
        let h = PersistentHost::open(t.state(), doc, Arc::new(math_registry()), options).unwrap();
        if max_bytes > 1 {
            assert_eq!(h.undo(id, 2).unwrap().document.graph().nodes().len(), 1);
            assert_eq!(h.undo(id, 3).unwrap_err().code, "recovery_unavailable");
        } else {
            assert_eq!(h.undo(id, 2).unwrap_err().code, "recovery_unavailable");
        }
    }
}
#[test]
fn concurrent_editors_cannot_commit_the_same_revision() {
    let t = Temp::new();
    let doc = Document::default();
    let id = doc.graph().id;
    let host = Arc::new(open(&t, &doc));
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let host = host.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                host.apply(id, 0, command())
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers.into_iter().map(|w| w.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results.iter().find_map(|r| r.as_ref().err()).unwrap().code,
        "revision_conflict"
    );
    drop(host);
    let host = open(&t, &doc);
    assert_eq!(host.snapshot().unwrap().revision, 1);
    assert_eq!(host.snapshot().unwrap().document.graph().nodes().len(), 1);
}

#[test]
fn either_log_rollback_and_cross_pair_swap_fail_without_modifying_evidence() {
    for mode in ["state", "replay", "other_pair"] {
        let t = Temp::new();
        let doc = Document::default();
        let host = Arc::new(open(&t, &doc));
        let mut p = provider(&t, host.clone());
        let old_state = fs::read(t.state()).unwrap();
        let old_replay = fs::read(t.0.join("replay.jsonl")).unwrap();
        let commit = plan(&mut p, host.as_ref());
        p.dispatch("execute", json!({"commitId":commit})).unwrap();
        drop(p);
        drop(host);
        match mode {
            "state" => fs::write(t.state(), old_state).unwrap(),
            "replay" => fs::write(t.0.join("replay.jsonl"), old_replay).unwrap(),
            _ => {
                let other = Temp::new();
                let h = Arc::new(open(&other, &doc));
                let mut p = provider(&other, h.clone());
                let commit = plan(&mut p, h.as_ref());
                p.dispatch("execute", json!({"commitId":commit})).unwrap();
                drop(p);
                drop(h);
                fs::copy(other.state(), t.state()).unwrap();
            }
        }
        let state_bytes = fs::read(t.state()).unwrap();
        let replay_bytes = fs::read(t.0.join("replay.jsonl")).unwrap();
        let host = Arc::new(open(&t, &doc));
        let result = Provider::new(host, Arc::new(math_registry()), Policy::math_demo())
            .with_journal(t.0.join("replay.jsonl"), Default::default());
        assert_eq!(result.err().unwrap().code, "storage_corrupt", "{mode}");
        assert_eq!(fs::read(t.state()).unwrap(), state_bytes);
        assert_eq!(fs::read(t.0.join("replay.jsonl")).unwrap(), replay_bytes);
    }
}
#[test]
fn legacy_receipts_cannot_be_silently_enrolled_and_paired_receipts_need_the_host() {
    let t = Temp::new();
    let doc = Document::default();
    let registry = Arc::new(math_registry());
    let memory = Arc::new(MemoryHost::new(doc.clone()).unwrap());
    let mut p = provider(&t, memory.clone());
    let commit = plan(&mut p, memory.as_ref());
    p.dispatch("execute", json!({"commitId":commit})).unwrap();
    drop(p);
    let host = Arc::new(open(&t, &doc));
    let bytes = fs::read(t.state()).unwrap();
    assert_eq!(
        Provider::new(host, registry.clone(), Policy::math_demo())
            .with_journal(t.0.join("replay.jsonl"), Default::default())
            .err()
            .unwrap()
            .code,
        "persistence_required"
    );
    assert_eq!(fs::read(t.state()).unwrap(), bytes);
    // A paired journal must never silently fall back to an unpaired memory host.
    let paired = Temp::new();
    let host = Arc::new(open(&paired, &doc));
    drop(provider(&paired, host));
    assert_eq!(
        Provider::new(memory, registry, Policy::math_demo())
            .with_journal(paired.0.join("replay.jsonl"), Default::default())
            .err()
            .unwrap()
            .code,
        "persistence_required"
    );
}
#[test]
fn whole_pair_backup_restores_replay_and_rotation_is_mirrored() {
    let t = Temp::new();
    let doc = Document::default();
    let host = Arc::new(open(&t, &doc));
    let mut p = provider(&t, host.clone());
    let commit = plan(&mut p, host.as_ref());
    let args = json!({"commitId":commit});
    let result = p.dispatch("execute", args.clone()).unwrap();
    drop(p);
    drop(host);
    let state = fs::read(t.state()).unwrap();
    let replay = fs::read(t.0.join("replay.jsonl")).unwrap();
    let host = Arc::new(open(&t, &doc));
    let mut p = provider(&t, host.clone());
    p.rotate_session().unwrap();
    drop(p);
    drop(host);
    fs::write(t.0.join("replay.jsonl"), &replay).unwrap();
    let host = Arc::new(open(&t, &doc));
    assert_eq!(
        Provider::new(host, Arc::new(math_registry()), Policy::math_demo())
            .with_journal(t.0.join("replay.jsonl"), Default::default())
            .err()
            .unwrap()
            .code,
        "storage_corrupt"
    );
    fs::write(t.state(), state).unwrap();
    let host = Arc::new(open(&t, &doc));
    let mut p = provider(&t, host.clone());
    assert_eq!(p.dispatch("execute", args).unwrap(), result);
    assert_eq!(host.snapshot().unwrap().revision, 1);
}

struct AckFault {
    host: PersistentHost,
    blocked: std::sync::atomic::AtomicBool,
}
impl GraphHost for AckFault {
    fn snapshot(&self) -> Result<Snapshot> {
        self.host.snapshot()
    }
    fn apply(&self, id: Id, rev: u64, command: Command) -> Result<Snapshot> {
        self.host.apply(id, rev, command)
    }
    fn undo(&self, id: Id, rev: u64) -> Result<Snapshot> {
        self.host.undo(id, rev)
    }
    fn persistence(&self) -> Result<Option<HostPersistence>> {
        self.host.persistence()
    }
    fn verify_persistence(&self, checkpoint: &LogCheckpoint) -> Result<()> {
        self.host.verify_persistence(checkpoint)
    }
    fn acknowledge_replay(&self, checkpoint: LogCheckpoint) -> Result<()> {
        if self.host.snapshot()?.revision > 0
            && self.blocked.load(std::sync::atomic::Ordering::SeqCst)
        {
            return Err(AcxError::new(
                "storage_full",
                "injected acknowledgement capacity failure",
            ));
        }
        self.host.acknowledge_replay(checkpoint)
    }
}
#[test]
fn failed_completion_ack_blocks_replay_until_mirrored_and_then_detects_receipt_rollback() {
    let t = Temp::new();
    let doc = Document::default();
    let host = Arc::new(AckFault {
        host: open(&t, &doc),
        blocked: true.into(),
    });
    let mut p = provider(&t, host.clone());
    let commit = plan(&mut p, host.as_ref());
    let args = json!({"commitId":commit});
    assert_eq!(
        p.dispatch("execute", args.clone()).unwrap_err().code,
        "storage_full"
    );
    assert_eq!(
        p.dispatch("execute", args.clone()).unwrap_err().code,
        "storage_full"
    );
    host.blocked
        .store(false, std::sync::atomic::Ordering::SeqCst);
    let result = p.dispatch("execute", args).unwrap();
    assert_eq!(host.snapshot().unwrap().revision, 1);
    assert!(result["receiptId"].is_string());
    drop(p);
    drop(host);
    let bytes = fs::read(t.0.join("replay.jsonl")).unwrap();
    let mut prefix = Vec::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        let frame: Value = serde_json::from_slice(line).unwrap();
        let event: Value = serde_json::from_str(frame["payload"].as_str().unwrap()).unwrap();
        if event["kind"] == "finished" {
            break;
        }
        prefix.extend_from_slice(line);
    }
    fs::write(t.0.join("replay.jsonl"), prefix).unwrap();
    let host = Arc::new(open(&t, &doc));
    assert_eq!(
        Provider::new(host, Arc::new(math_registry()), Policy::math_demo())
            .with_journal(t.0.join("replay.jsonl"), Default::default())
            .err()
            .unwrap()
            .code,
        "storage_corrupt"
    );
}

#[test]
fn persistent_provider_cannot_serve_without_replay_storage() {
    let t = Temp::new();
    let doc = Document::default();
    let host = Arc::new(open(&t, &doc));
    let before = fs::read(t.state()).unwrap();
    let mut p = Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo());
    assert_eq!(
        p.dispatch("discover", json!({})).unwrap_err().code,
        "persistence_required"
    );
    assert_eq!(fs::read(t.state()).unwrap(), before);
    let mut p = p
        .with_journal(t.0.join("replay.jsonl"), Default::default())
        .unwrap();
    assert!(p.dispatch("discover", json!({})).is_ok());
}
