use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use unge_acx::*;
use unge_core::*;
use unge_executor::*;
struct Wait(Arc<AtomicUsize>);
impl NodeExecutor for Wait {
    fn execute(
        &self,
        _: ExecutionContext,
        _: Inputs,
    ) -> futures::future::BoxFuture<'_, std::result::Result<Outputs, String>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(futures::future::pending())
    }
}
fn setup(wait: bool) -> (Provider, Arc<MemoryHost>, Arc<AtomicUsize>, RunService) {
    setup_retention(wait, Policy::math_demo(), Arc::new(AtomicU64::new(1_000)))
}
fn setup_retention(
    wait: bool,
    policy: Policy,
    now: Arc<AtomicU64>,
) -> (Provider, Arc<MemoryHost>, Arc<AtomicUsize>, RunService) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = math_registry();
    if wait {
        let def = registry.definition("math.number").unwrap().clone();
        registry = Registry::default();
        registry
            .register(def, Arc::new(Wait(calls.clone())))
            .unwrap();
    }
    let registry = Arc::new(registry);
    let mut editor = Editor::new(Document::default(), 10).unwrap();
    editor
        .execute(Command::AddNode {
            node: registry.definition("math.number").unwrap().instantiate(),
            rect: Rect::default(),
        })
        .unwrap();
    let host = Arc::new(MemoryHost::from_editor(editor));
    let service = RunService::new(
        registry.clone(),
        Scheduler::new(1, 10),
        RunLimits::default(),
    );
    let provider = Provider::with_clock(
        host.clone(),
        registry,
        policy,
        Arc::new(move || now.load(Ordering::SeqCst)),
    )
    .with_execution(service.clone(), |_| {})
    .unwrap();
    (provider, host, calls, service)
}
fn commit(provider: &mut Provider, host: &MemoryHost, kind: &str) -> String {
    let snap = host.snapshot().unwrap();
    let mut input = json!({"kind":kind,"document_id":snap.document.graph().id,"expected_revision":snap.revision});
    if kind == "edit" {
        input["operations"] = json!([{"kind":"auto_layout","gap":[40,20]}]);
    }
    let pf = provider
        .dispatch("preflight", json!({"input":input}))
        .unwrap();
    let bound = json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]});
    let grant = provider.dispatch("authorize", bound).unwrap();
    provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":grant["authorization"],"input":pf["input"]})).unwrap()["commitId"].as_str().unwrap().into()
}
fn terminal(provider: &mut Provider, id: &str) -> Value {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let result = provider
            .dispatch("run_status", json!({"commitId":id}))
            .unwrap();
        if !result["execution"].is_null() {
            return result;
        }
        assert!(std::time::Instant::now() < until, "job did not finish");
        std::thread::yield_now();
    }
}
#[test]
fn active_job_leaves_pipe_responsive_cancel_and_receipt_are_replayed() {
    let (mut provider, host, calls, _) = setup(true);
    let id = commit(&mut provider, &host, "run");
    let before = host.snapshot().unwrap().revision;
    let first = provider
        .dispatch("run_start", json!({"commitId":id}))
        .unwrap();
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":id}))
            .unwrap()["run"]["id"],
        first["run"]["id"]
    );
    assert!(
        provider
            .dispatch("observe", json!({"query":"summary"}))
            .is_ok()
    );
    // Wait only for the executor to begin, then cancel a future that never wakes itself.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while calls.load(Ordering::SeqCst) == 0 {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    let cancelled = provider
        .dispatch("run_cancel", json!({"commitId":id}))
        .unwrap();
    assert!(cancelled["run"]["cancel_requested"].as_bool().unwrap());
    let done = terminal(&mut provider, &id);
    assert_eq!(done["run"]["reason"], "cancelled");
    assert_eq!(done["execution"]["result"]["succeeded"], false);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        provider
            .dispatch("execute", json!({"commitId":id}))
            .unwrap(),
        done["execution"]
    );
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":id}))
            .unwrap(),
        done
    );
    assert_eq!(
        provider
            .dispatch("run_cancel", json!({"commitId":id}))
            .unwrap(),
        done
    );
    let receipt = provider
        .dispatch(
            "receipt",
            json!({"receiptId":done["execution"]["receiptId"]}),
        )
        .unwrap();
    assert_eq!(receipt["status"], "failed");
    assert_eq!(host.snapshot().unwrap().revision, before);
}
#[test]
fn completed_job_is_stable_even_after_run_service_retention_expires() {
    let (mut provider, host, _, service) = setup(false);
    let id = commit(&mut provider, &host, "run");
    provider
        .dispatch("run_start", json!({"commitId":id}))
        .unwrap();
    let done = terminal(&mut provider, &id);
    assert_eq!(done["execution"]["result"]["succeeded"], true);
    for _ in 0..33 {
        let snapshot = host.snapshot().unwrap();
        service
            .prepare(&snapshot.document, snapshot.revision)
            .unwrap()
            .execute(|_| {})
            .unwrap();
    }
    assert_eq!(
        provider
            .dispatch("run_status", json!({"commitId":id}))
            .unwrap(),
        done
    );
    assert_eq!(
        provider
            .dispatch("run_cancel", json!({"commitId":id}))
            .unwrap(),
        done
    );
}
#[test]
fn unknown_unapproved_edit_and_stale_commits_cannot_launch_jobs() {
    let (mut provider, host, _, service) = setup(false);
    assert!(
        provider
            .dispatch("run_start", json!({"commitId":"unknown"}))
            .is_err()
    );
    assert_eq!(
        provider
            .dispatch("run_cancel", json!({"commitId":"unknown"}))
            .unwrap_err()
            .code,
        "unknown_job"
    );
    let edit = commit(&mut provider, &host, "edit");
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":edit}))
            .unwrap_err()
            .code,
        "invalid_request"
    );
    let id = commit(&mut provider, &host, "run");
    let snapshot = host.snapshot().unwrap();
    let node = *snapshot.document.graph().nodes().keys().next().unwrap();
    host.apply(
        snapshot.document.graph().id,
        snapshot.revision,
        Command::SetProperty {
            id: node,
            key: "value".into(),
            value: Some(5.into()),
        },
    )
    .unwrap();
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":id}))
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    assert!(service.current().unwrap().is_none());
}
#[test]
fn provider_drop_cancels_its_owned_job_and_releases_shared_slot() {
    let (mut provider, host, _, service) = setup(true);
    let id = commit(&mut provider, &host, "run");
    provider
        .dispatch("run_start", json!({"commitId":id}))
        .unwrap();
    drop(provider);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while service.current().unwrap().is_some() {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
}

#[test]
fn busy_start_does_not_consume_another_commit_and_params_are_strict() {
    let (mut provider, host, _, _) = setup(true);
    let a = commit(&mut provider, &host, "run");
    let b = commit(&mut provider, &host, "run");
    provider
        .dispatch("run_start", json!({"commitId":a}))
        .unwrap();
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":b}))
            .unwrap_err()
            .code,
        "execution_busy"
    );
    assert_eq!(
        provider
            .dispatch("run_status", json!({"commitId":b}))
            .unwrap_err()
            .code,
        "unknown_job"
    );
    assert_eq!(
        provider
            .dispatch("run_cancel", json!({"commitId":a,"runId":"arbitrary"}))
            .unwrap_err()
            .code,
        "invalid_request"
    );
    provider
        .dispatch("run_cancel", json!({"commitId":a}))
        .unwrap();
    terminal(&mut provider, &a);
    provider
        .dispatch("run_start", json!({"commitId":b}))
        .unwrap();
    provider
        .dispatch("run_cancel", json!({"commitId":b}))
        .unwrap();
    terminal(&mut provider, &b);
}

#[test]
fn expired_active_and_finalized_jobs_keep_their_session_and_replay() {
    let now = Arc::new(AtomicU64::new(1_000));
    let (mut provider, host, calls, _) = setup_retention(
        true,
        Policy {
            max_sessions: 1,
            ttl_seconds: 1,
            ..Policy::math_demo()
        },
        now.clone(),
    );
    let id = commit(&mut provider, &host, "run");
    provider
        .dispatch("run_start", json!({"commitId":id}))
        .unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while calls.load(Ordering::SeqCst) == 0 {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    now.store(1_001, Ordering::SeqCst);
    assert_eq!(provider.prune_expired_preflights(), 0);
    let usage = provider.retention_usage();
    assert_eq!(
        (
            usage.sessions,
            usage.started,
            usage.jobs,
            usage.active_jobs,
            usage.expired_unstarted
        ),
        (1, 1, 1, 1, 0)
    );
    let snapshot = host.snapshot().unwrap();
    assert_eq!(provider.dispatch("preflight",json!({"input":{"kind":"run","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision}})).unwrap_err().code,"capacity_exceeded");
    provider
        .dispatch("run_cancel", json!({"commitId":id}))
        .unwrap();
    let done = terminal(&mut provider, &id);
    assert_eq!(provider.prune_expired_preflights(), 0);
    let usage = provider.retention_usage();
    assert_eq!(
        (
            usage.sessions,
            usage.receipts,
            usage.jobs,
            usage.active_jobs
        ),
        (1, 1, 1, 0)
    );
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":id}))
            .unwrap(),
        done
    );
    assert_eq!(
        provider
            .dispatch("execute", json!({"commitId":id}))
            .unwrap(),
        done["execution"]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(host.snapshot().unwrap().revision, snapshot.revision);
}

#[test]
fn durable_jobs_refuse_active_rotation_and_replay_after_restart() {
    let directory = std::env::temp_dir().join(format!("unge-job-journal-{}", Id::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("journal.jsonl");
    let (provider, host, calls, _) = setup(true);
    let mut provider = provider
        .with_journal(&path, JournalOptions::default())
        .unwrap();
    let id = commit(&mut provider, &host, "run");
    provider
        .dispatch("run_start", json!({"commitId":id}))
        .unwrap();
    assert_eq!(
        provider.rotate_session().unwrap_err().code,
        "execution_busy"
    );
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while calls.load(Ordering::SeqCst) == 0 {
        assert!(std::time::Instant::now() < until);
        std::thread::yield_now();
    }
    provider
        .dispatch("run_cancel", json!({"commitId":id}))
        .unwrap();
    let done = terminal(&mut provider, &id);
    let receipt_id = done["execution"]["receiptId"].clone();
    let receipt = provider
        .dispatch("receipt", json!({"receiptId":receipt_id}))
        .unwrap();
    provider.rotate_session().unwrap();
    assert_eq!(
        provider
            .dispatch("run_start", json!({"commitId":id}))
            .unwrap(),
        done
    );
    drop(provider);
    let mut provider = Provider::new(host, Arc::new(math_registry()), Policy::default())
        .with_journal(&path, JournalOptions::default())
        .unwrap();
    for method in ["run_start", "run_status", "run_cancel"] {
        assert_eq!(
            provider.dispatch(method, json!({"commitId":id})).unwrap(),
            done
        );
    }
    assert_eq!(
        provider
            .dispatch("execute", json!({"commitId":id}))
            .unwrap(),
        done["execution"]
    );
    assert_eq!(
        provider
            .dispatch("receipt", json!({"receiptId":receipt_id}))
            .unwrap(),
        receipt
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    drop(provider);
    std::fs::remove_dir_all(directory).unwrap();
}
