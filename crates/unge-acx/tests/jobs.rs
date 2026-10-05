use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
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
    let provider = Provider::new(host.clone(), registry, Policy::math_demo())
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
