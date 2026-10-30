use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use unge_acx::*;
use unge_core::{Document, Id};
use unge_executor::math_registry;

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("unge-journal-{}", Id::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn journal(&self) -> PathBuf {
        self.0.join("replay.jsonl")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn provider(host: Arc<MemoryHost>, path: &Path) -> Provider {
    Provider::new(
        host,
        Arc::new(math_registry()),
        Policy {
            max_sessions: 1,
            ..Policy::math_demo()
        },
    )
    .with_journal(path, JournalOptions::default())
    .unwrap()
}
fn plan(provider: &mut Provider, host: &MemoryHost) -> (String, Value) {
    let snapshot = host.snapshot().unwrap();
    let input = json!({"kind":"edit","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision,"operations":[{"kind":"create_node","id":Id::new_v4(),"type_id":"math.number","properties":{"value":42},"rect":{"x":0,"y":0,"width":180,"height":90}}]});
    let pf = provider
        .dispatch("preflight", json!({"input":input}))
        .unwrap();
    let grant = provider
        .dispatch(
            "authorize",
            json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]}),
        )
        .unwrap();
    let commit = provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":grant["authorization"],"input":pf["input"]})).unwrap();
    (commit["commitId"].as_str().unwrap().into(), grant)
}
fn execute(provider: &mut Provider, commit: &str) -> Value {
    provider
        .dispatch("execute", json!({"commitId":commit}))
        .unwrap()
}
fn receipt(provider: &mut Provider, execution: &Value) -> Value {
    provider
        .dispatch("receipt", json!({"receiptId":execution["receiptId"]}))
        .unwrap()
}

#[test]
fn completed_execution_and_original_receipt_survive_restart_and_rotation() {
    let temp = Temp::new();
    let host = Arc::new(MemoryHost::new(Document::default()).unwrap());
    let mut p = provider(host.clone(), &temp.journal());
    let original_id = p.manifest()["provider"]["id"].clone();
    let (commit, grant) = plan(&mut p, &host);
    let result = execute(&mut p, &commit);
    let evidence = receipt(&mut p, &result);
    drop(p);
    let mut p = provider(host.clone(), &temp.journal());
    assert_eq!(p.manifest()["provider"]["id"], original_id);
    assert_eq!(execute(&mut p, &commit), result);
    assert_eq!(receipt(&mut p, &result), evidence);
    assert_eq!(host.snapshot().unwrap().revision, 1);
    assert_eq!(
        p.dispatch(
            "recover",
            json!({"commitId":commit,"authorization":grant["authorization"],"expectedRevision":1})
        )
        .unwrap_err()
        .code,
        "recovery_unavailable"
    );
    let next = p.rotate_session().unwrap();
    assert_ne!(next, original_id);
    assert_eq!(p.retention_usage().sessions, 0);
    assert_eq!(execute(&mut p, &commit), result);
    let (second, _) = plan(&mut p, &host);
    let second_result = execute(&mut p, &second);
    assert_eq!(p.journal_usage().unwrap().commits, 2);
    drop(p);
    let mut p = provider(host.clone(), &temp.journal());
    assert_eq!(p.manifest()["provider"]["id"], next);
    assert_eq!(execute(&mut p, &commit), result);
    assert_eq!(execute(&mut p, &second), second_result);
    assert_eq!(receipt(&mut p, &result), evidence);
    assert_eq!(host.snapshot().unwrap().revision, 2);
}

#[test]
fn recovered_result_and_both_receipts_replay_without_a_second_undo() {
    let temp = Temp::new();
    let host = Arc::new(MemoryHost::new(Document::default()).unwrap());
    let mut p = provider(host.clone(), &temp.journal());
    let (commit, grant) = plan(&mut p, &host);
    let result = execute(&mut p, &commit);
    let original = receipt(&mut p, &result);
    let args =
        json!({"commitId":commit,"authorization":grant["authorization"],"expectedRevision":1});
    let recovered = p.dispatch("recover", args.clone()).unwrap();
    let recovery_receipt = receipt(&mut p, &recovered);
    p.rotate_session().unwrap();
    drop(p);
    let mut p = provider(host.clone(), &temp.journal());
    assert_eq!(p.dispatch("recover", args).unwrap(), recovered);
    assert_eq!(receipt(&mut p, &recovered), recovery_receipt);
    assert_eq!(receipt(&mut p, &result), original);
    assert_eq!(execute(&mut p, &commit), result);
    assert_eq!(host.snapshot().unwrap().revision, 2);
    assert_eq!(
        p.dispatch(
            "recover",
            json!({"commitId":commit,"authorization":"wrong","expectedRevision":2})
        )
        .unwrap_err()
        .code,
        "unauthorized"
    );
}

#[test]
fn exclusive_writer_wrong_document_and_invalid_storage_fail_closed() {
    let temp = Temp::new();
    let host = Arc::new(MemoryHost::new(Document::default()).unwrap());
    let mut p = provider(host.clone(), &temp.journal());
    fs::write(
        temp.0.join("document.json"),
        host.snapshot().unwrap().document.to_json().unwrap(),
    )
    .unwrap();
    let contender = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "lock_contender", "--ignored"])
        .env("UNGE_TEST_JOURNAL_DIR", &temp.0)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(contender.success());
    let second = Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo())
        .with_journal(temp.journal(), JournalOptions::default());
    assert_eq!(second.err().unwrap().code, "storage_locked");
    let (commit, _) = plan(&mut p, &host);
    let result = execute(&mut p, &commit);
    let before = fs::read(temp.journal()).unwrap();
    drop(p);
    let wrong = Arc::new(MemoryHost::new(Document::default()).unwrap());
    assert_eq!(
        Provider::new(wrong, Arc::new(math_registry()), Policy::math_demo())
            .with_journal(temp.journal(), JournalOptions::default())
            .err()
            .unwrap()
            .code,
        "document_mismatch"
    );
    assert_eq!(fs::read(temp.journal()).unwrap(), before);
    for bytes in [
        before[..before.len() - 1].to_vec(),
        b"corrupted\n".to_vec(),
        Vec::new(),
    ] {
        fs::write(temp.journal(), &bytes).unwrap();
        assert_eq!(
            Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo())
                .with_journal(temp.journal(), JournalOptions::default())
                .err()
                .unwrap()
                .code,
            "storage_corrupt"
        );
        assert_eq!(fs::read(temp.journal()).unwrap(), bytes);
    }
    fs::write(temp.journal(), before).unwrap();
    let mut p = provider(host, &temp.journal());
    assert_eq!(execute(&mut p, &commit), result);
}

#[test]
fn capacity_is_reserved_before_effects_and_volatile_rotation_is_refused() {
    let temp = Temp::new();
    let host = Arc::new(MemoryHost::new(Document::default()).unwrap());
    let mut volatile = Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo());
    assert_eq!(
        volatile.rotate_session().unwrap_err().code,
        "persistence_required"
    );
    let mut p = Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo())
        .with_journal(temp.journal(), JournalOptions { max_bytes: 4096 })
        .unwrap();
    let (commit, _) = plan(&mut p, &host);
    assert_eq!(
        p.dispatch("execute", json!({"commitId":commit}))
            .unwrap_err()
            .code,
        "storage_full"
    );
    assert_eq!(host.snapshot().unwrap().revision, 0);
    assert_eq!(p.journal_usage().unwrap().commits, 0);
    drop(p);
    let mut p = provider(host, &temp.journal());
    assert_eq!(
        p.dispatch("execute", json!({"commitId":commit}))
            .unwrap_err()
            .code,
        "unauthorized"
    );
}

// A separate process exits without Drop after persisting the execution start.
struct ExitHost {
    host: Arc<MemoryHost>,
    directory: PathBuf,
    mode: String,
}
impl GraphHost for ExitHost {
    fn snapshot(&self) -> Result<Snapshot> {
        self.host.snapshot()
    }
    fn apply(&self, document: Id, revision: u64, command: unge_core::Command) -> Result<Snapshot> {
        if self.mode == "before_effect" {
            std::process::exit(23);
        }
        let after = self.host.apply(document, revision, command)?;
        use std::io::Write;
        let mut marker = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.directory.join("effects"))
            .unwrap();
        marker.write_all(b"applied\n").unwrap();
        marker.sync_all().unwrap();
        if self.mode == "after_effect" {
            std::process::exit(23);
        }
        Ok(after)
    }
    fn undo(&self, document: Id, revision: u64) -> Result<Snapshot> {
        let after = self.host.undo(document, revision)?;
        if self.mode == "during_recovery" {
            use std::io::Write;
            let mut marker = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.directory.join("undo-effects"))
                .unwrap();
            marker.write_all(b"undo\n").unwrap();
            marker.sync_all().unwrap();
            std::process::exit(24);
        }
        Ok(after)
    }
}
#[test]
#[ignore = "subprocess fixture; invoked by crash_boundary_reopen_never_repeats_effects"]
fn crash_worker() {
    let Some(directory) = std::env::var_os("UNGE_TEST_JOURNAL_DIR") else {
        return;
    };
    let dir = PathBuf::from(directory);
    let mode = std::env::var("UNGE_TEST_JOURNAL_MODE").unwrap();
    let document = Document::from_json(&fs::read(dir.join("document.json")).unwrap()).unwrap();
    let host = Arc::new(MemoryHost::new(document).unwrap());
    let registry = Arc::new(math_registry());
    let service = unge_executor::RunService::new(
        registry.clone(),
        unge_executor::Scheduler::new(1, 10),
        unge_executor::RunLimits::default(),
    );
    let mut p = Provider::new(
        Arc::new(ExitHost {
            host: host.clone(),
            directory: dir.clone(),
            mode,
        }),
        registry,
        Policy::math_demo(),
    )
    .with_execution(service, |_| {})
    .unwrap()
    .with_journal(dir.join("replay.jsonl"), JournalOptions::default())
    .unwrap();
    let (commit, grant) = plan(&mut p, &host);
    let mut file = fs::File::create(dir.join("commit.json")).unwrap();
    use std::io::Write;
    file.write_all(serde_json::to_string(&commit).unwrap().as_bytes())
        .unwrap();
    file.sync_all().unwrap();
    let result = execute(&mut p, &commit);
    let mut file = fs::File::create(dir.join("reply.json")).unwrap();
    file.write_all(serde_json::to_string(&result).unwrap().as_bytes())
        .unwrap();
    file.sync_all().unwrap();
    if std::env::var("UNGE_TEST_JOURNAL_MODE").unwrap() == "async_start" {
        let snapshot = host.snapshot().unwrap();
        let pf=p.dispatch("preflight",json!({"input":{"kind":"run","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision}})).unwrap();
        let bound =
            json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]});
        let grant = p.dispatch("authorize", bound).unwrap();
        let next=p.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":grant["authorization"],"input":pf["input"]})).unwrap();
        let id = next["commitId"].as_str().unwrap();
        let mut file = fs::File::create(dir.join("commit.json")).unwrap();
        file.write_all(serde_json::to_string(id).unwrap().as_bytes())
            .unwrap();
        file.sync_all().unwrap();
        p.dispatch("run_start", json!({"commitId":id})).unwrap();
        std::process::exit(23);
    }
    if std::env::var("UNGE_TEST_JOURNAL_MODE").unwrap() == "during_recovery" {
        let mut auth = fs::File::create(dir.join("grant.json")).unwrap();
        auth.write_all(serde_json::to_string(&grant).unwrap().as_bytes())
            .unwrap();
        auth.sync_all().unwrap();
        p.dispatch(
            "recover",
            json!({"commitId":commit,"authorization":grant["authorization"],"expectedRevision":1}),
        )
        .unwrap();
        panic!("recovery crash fixture did not exit");
    }
    std::process::exit(23);
}
#[test]
fn crash_boundary_reopen_never_repeats_effects() {
    for mode in [
        "before_effect",
        "after_effect",
        "after_finish",
        "async_start",
    ] {
        let temp = Temp::new();
        let document = Document::default();
        fs::write(temp.0.join("document.json"), document.to_json().unwrap()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "crash_worker", "--ignored", "--nocapture"])
            .env("UNGE_TEST_JOURNAL_DIR", &temp.0)
            .env("UNGE_TEST_JOURNAL_MODE", mode)
            .status()
            .unwrap();
        assert_eq!(status.code(), Some(23));
        let commit: String =
            serde_json::from_slice(&fs::read(temp.0.join("commit.json")).unwrap()).unwrap();
        let host = Arc::new(MemoryHost::new(document).unwrap());
        let mut p = provider(host.clone(), &temp.journal());
        if mode == "after_finish" {
            let reply: Value =
                serde_json::from_slice(&fs::read(temp.0.join("reply.json")).unwrap()).unwrap();
            assert_eq!(execute(&mut p, &commit), reply);
            receipt(&mut p, &reply);
        } else {
            assert_eq!(
                p.dispatch("execute", json!({"commitId":commit}))
                    .unwrap_err()
                    .code,
                "execution_uncertain"
            );
        }
        if mode == "async_start" {
            for method in ["run_start", "run_status", "run_cancel"] {
                assert_eq!(
                    p.dispatch(method, json!({"commitId":commit}))
                        .unwrap_err()
                        .code,
                    "execution_uncertain"
                );
            }
        }
        p.rotate_session().unwrap();
        if mode != "after_finish" {
            assert_eq!(
                p.dispatch("execute", json!({"commitId":commit}))
                    .unwrap_err()
                    .code,
                "execution_uncertain"
            );
        }
        assert_eq!(host.snapshot().unwrap().revision, 0);
        let count = fs::read_to_string(temp.0.join("effects"))
            .unwrap_or_default()
            .lines()
            .count();
        assert_eq!(count, usize::from(mode != "before_effect"));
    }
}

#[test]
fn interrupted_recovery_cannot_undo_again_after_restart_or_rotation() {
    let temp = Temp::new();
    let document = Document::default();
    fs::write(temp.0.join("document.json"), document.to_json().unwrap()).unwrap();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "crash_worker", "--ignored"])
        .env("UNGE_TEST_JOURNAL_DIR", &temp.0)
        .env("UNGE_TEST_JOURNAL_MODE", "during_recovery")
        .status()
        .unwrap();
    assert_eq!(status.code(), Some(24));
    let commit: String =
        serde_json::from_slice(&fs::read(temp.0.join("commit.json")).unwrap()).unwrap();
    let grant: Value =
        serde_json::from_slice(&fs::read(temp.0.join("grant.json")).unwrap()).unwrap();
    let reply: Value =
        serde_json::from_slice(&fs::read(temp.0.join("reply.json")).unwrap()).unwrap();
    let host = Arc::new(MemoryHost::new(document).unwrap());
    let mut p = provider(host.clone(), &temp.journal());
    for _ in 0..2 {
        assert_eq!(p.dispatch("recover",json!({"commitId":commit,"authorization":grant["authorization"],"expectedRevision":1})).unwrap_err().code,"execution_uncertain");
        assert_eq!(execute(&mut p, &commit), reply);
        p.rotate_session().unwrap();
    }
    assert_eq!(host.snapshot().unwrap().revision, 0);
    assert_eq!(
        fs::read_to_string(temp.0.join("undo-effects"))
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn failed_executor_result_is_durable_and_not_reexecuted() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use unge_executor::{ExecutionContext, Inputs, NodeExecutor, Outputs, Registry};
    struct Fails(Arc<AtomicUsize>);
    impl NodeExecutor for Fails {
        fn execute(
            &self,
            _: ExecutionContext,
            _: Inputs,
        ) -> futures::future::BoxFuture<'_, std::result::Result<Outputs, String>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err("failed after possible effects".into()) })
        }
    }
    let temp = Temp::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = Registry::default();
    registry
        .register(
            math_registry().definition("math.number").unwrap().clone(),
            Arc::new(Fails(calls.clone())),
        )
        .unwrap();
    let mut editor = unge_core::Editor::new(Document::default(), 10).unwrap();
    editor
        .execute(unge_core::Command::AddNode {
            node: registry.definition("math.number").unwrap().instantiate(),
            rect: unge_core::Rect::default(),
        })
        .unwrap();
    let registry = Arc::new(registry);
    let host = Arc::new(MemoryHost::from_editor(editor));
    let mut p = Provider::new(host.clone(), registry.clone(), Policy::math_demo())
        .with_journal(temp.journal(), JournalOptions::default())
        .unwrap();
    let snap = host.snapshot().unwrap();
    let input = json!({"kind":"run","document_id":snap.document.graph().id,"expected_revision":snap.revision});
    let pf = p.dispatch("preflight", json!({"input":input})).unwrap();
    let auth = p
        .dispatch(
            "authorize",
            json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]}),
        )
        .unwrap();
    let commit=p.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":pf["input"]})).unwrap()["commitId"].as_str().unwrap().to_string();
    let result = execute(&mut p, &commit);
    let evidence = receipt(&mut p, &result);
    assert_eq!(evidence["status"], "failed");
    drop(p);
    let mut p = Provider::new(host, registry, Policy::default())
        .with_journal(temp.journal(), JournalOptions::default())
        .unwrap();
    assert_eq!(execute(&mut p, &commit), result);
    p.rotate_session().unwrap();
    assert_eq!(execute(&mut p, &commit), result);
    assert_eq!(receipt(&mut p, &result), evidence);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
#[ignore = "subprocess fixture; invoked by exclusive writer test"]
fn lock_contender() {
    let Some(directory) = std::env::var_os("UNGE_TEST_JOURNAL_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory);
    let document =
        Document::from_json(&fs::read(directory.join("document.json")).unwrap()).unwrap();
    let host = Arc::new(MemoryHost::new(document).unwrap());
    let result = Provider::new(host, Arc::new(math_registry()), Policy::math_demo())
        .with_journal(directory.join("replay.jsonl"), JournalOptions::default());
    assert_eq!(result.err().unwrap().code, "storage_locked");
}
