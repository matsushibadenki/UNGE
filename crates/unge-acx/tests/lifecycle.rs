use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use unge_acx::*;
use unge_core::{Command, Document, Id, Rect};
use unge_executor::math_registry;

struct Harness {
    host: Arc<MemoryHost>,
    provider: Provider,
    now: Arc<AtomicU64>,
}
impl Harness {
    fn new() -> Self {
        Self::policy(Policy::math_demo())
    }
    fn policy(policy: Policy) -> Self {
        let host = Arc::new(MemoryHost::new(Document::default()).unwrap());
        let now = Arc::new(AtomicU64::new(1_000));
        let clock = now.clone();
        let provider = Provider::with_clock(
            host.clone(),
            Arc::new(math_registry()),
            policy,
            Arc::new(move || clock.load(Ordering::SeqCst)),
        );
        Self {
            host,
            provider,
            now,
        }
    }
    fn input(&self) -> Value {
        let snapshot = self.host.snapshot().unwrap();
        json!({"kind":"edit","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision,"operations":[{"kind":"create_node","id":Id::new_v4(),"type_id":"math.number","properties":{"value":42},"rect":{"x":10.25,"y":-20.5,"width":180,"height":90}}]})
    }
    fn preflight(&mut self, input: Value) -> Value {
        self.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap()
    }
    fn authorize(&mut self, pf: &Value) -> Value {
        self.provider
            .dispatch(
                "authorize",
                json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]}),
            )
            .unwrap()
    }
    fn commit(&mut self, pf: &Value, auth: &Value) -> Value {
        self.provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":pf["input"]})).unwrap()
    }
    fn execute(&mut self, commit: &Value) -> Value {
        self.provider
            .dispatch("execute", json!({"commitId":commit["commitId"]}))
            .unwrap()
    }
    fn plan(&mut self) -> (Value, Value, Value) {
        let input = self.input();
        let pf = self.preflight(input);
        let auth = self.authorize(&pf);
        let commit = self.commit(&pf, &auth);
        (pf, auth, commit)
    }
}
#[test]
fn edit_lifecycle_receipt_recovery_and_replay() {
    let mut h = Harness::new();
    let before = h.host.snapshot().unwrap().document;
    let (pf, auth, commit) = h.plan();
    assert_eq!(h.host.snapshot().unwrap().revision, 0);
    assert_eq!(
        hash_bytes(pf["requestJson"].as_str().unwrap().as_bytes()),
        pf["requestHash"]
    );
    assert_eq!(
        hash_bytes(pf["preflightJson"].as_str().unwrap().as_bytes()),
        pf["preflightDigest"]
    );
    let result = h.execute(&commit);
    assert_eq!(result["result"]["summary"]["nodes"], 1);
    let receipt = h
        .provider
        .dispatch("receipt", json!({"receiptId":result["receiptId"]}))
        .unwrap();
    assert_eq!(receipt["status"], "succeeded");
    assert_eq!(receipt["requestHash"], pf["requestHash"]);
    assert_eq!(
        receipt["resultHash"],
        hash_bytes(result["resultJson"].as_str().unwrap().as_bytes())
    );
    assert_eq!(h.execute(&commit), result);
    assert_eq!(h.host.snapshot().unwrap().revision, 1);
    let args = json!({"commitId":commit["commitId"],"authorization":auth["authorization"],"expectedRevision":1});
    let recovered = h.provider.dispatch("recover", args.clone()).unwrap();
    assert_eq!(h.host.snapshot().unwrap().document, before);
    assert_eq!(h.host.snapshot().unwrap().revision, 2);
    assert_eq!(h.provider.dispatch("recover", args).unwrap(), recovered);
    assert_eq!(h.host.snapshot().unwrap().revision, 2);
    assert_eq!(
        h.provider
            .dispatch("receipt", json!({"receiptId":result["receiptId"]}))
            .unwrap(),
        receipt
    );
    let recovery_receipt = h
        .provider
        .dispatch("receipt", json!({"receiptId":recovered["receiptId"]}))
        .unwrap();
    assert_eq!(recovery_receipt["recovery"]["state"], "recovered");
}
#[test]
fn tampered_digest_input_and_unrelated_authorization_are_rejected() {
    let mut h = Harness::new();
    let input = h.input();
    let pf = h.preflight(input);
    let auth = h.authorize(&pf);
    let mut args = json!({"preflightId":pf["preflightId"],"preflightDigest":"wrong","authorization":auth["authorization"],"input":pf["input"]});
    assert_eq!(
        h.provider
            .dispatch("commit", args.clone())
            .unwrap_err()
            .code,
        "digest_mismatch"
    );
    args["preflightDigest"] = pf["preflightDigest"].clone();
    args["input"]["operations"][0]["properties"]["value"] = json!(9);
    assert_eq!(
        h.provider
            .dispatch("commit", args.clone())
            .unwrap_err()
            .code,
        "digest_mismatch"
    );
    args["input"] = pf["input"].clone();
    let other_input = h.input();
    let other_pf = h.preflight(other_input);
    let other_auth = h.authorize(&other_pf);
    args["authorization"] = other_auth["authorization"].clone();
    assert_eq!(
        h.provider.dispatch("commit", args).unwrap_err().code,
        "unauthorized"
    );
    assert_eq!(h.host.snapshot().unwrap().revision, 0);
}
#[test]
fn duplicate_commit_is_rejected() {
    let mut h = Harness::new();
    let (pf, auth, _) = h.plan();
    assert_eq!(h.provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":pf["input"]})).unwrap_err().code,"already_committed");
}
#[test]
fn expiry_boundary_and_successful_retry_after_expiry() {
    let mut h = Harness::new();
    let input = h.input();
    let pf = h.preflight(input);
    h.now.store(1120, Ordering::SeqCst);
    assert_eq!(
        h.provider
            .dispatch(
                "authorize",
                json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]})
            )
            .unwrap_err()
            .code,
        "expired"
    );
    let (_, _, commit) = h.plan();
    let result = h.execute(&commit);
    h.now.store(9999, Ordering::SeqCst);
    assert_eq!(h.execute(&commit), result);
}
#[test]
fn expired_new_execution_is_refused() {
    let mut h = Harness::new();
    let (_, _, commit) = h.plan();
    h.now.store(1120, Ordering::SeqCst);
    assert_eq!(
        h.provider
            .dispatch("execute", json!({"commitId":commit["commitId"]}))
            .unwrap_err()
            .code,
        "expired"
    );
    assert_eq!(h.host.snapshot().unwrap().revision, 0);
}
fn user_edit(h: &Harness) {
    let snapshot = h.host.snapshot().unwrap();
    let node = math_registry()
        .definition("math.number")
        .unwrap()
        .instantiate();
    h.host
        .apply(
            snapshot.document.graph().id,
            snapshot.revision,
            Command::AddNode {
                node,
                rect: Rect::default(),
            },
        )
        .unwrap();
}
#[test]
fn stale_preflight_is_rejected_at_authorize_commit_and_execute() {
    for stage in ["authorize", "commit", "execute"] {
        let mut h = Harness::new();
        let input = h.input();
        let pf = h.preflight(input);
        let auth = if stage != "authorize" {
            h.authorize(&pf)
        } else {
            Value::Null
        };
        let commit = if stage == "execute" {
            h.commit(&pf, &auth)
        } else {
            Value::Null
        };
        user_edit(&h);
        let args = match stage {
            "authorize" => {
                json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]})
            }
            "commit" => {
                json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":pf["input"]})
            }
            _ => json!({"commitId":commit["commitId"]}),
        };
        assert_eq!(
            h.provider.dispatch(stage, args).unwrap_err().code,
            "revision_conflict"
        );
        assert_eq!(h.host.snapshot().unwrap().document.graph().nodes().len(), 1);
    }
}
#[test]
fn recovery_never_undoes_intervening_user_edit() {
    let mut h = Harness::new();
    let (_, auth, commit) = h.plan();
    h.execute(&commit);
    user_edit(&h);
    let args = json!({"commitId":commit["commitId"],"authorization":auth["authorization"],"expectedRevision":1});
    assert_eq!(
        h.provider.dispatch("recover", args).unwrap_err().code,
        "revision_conflict"
    );
    assert_eq!(h.host.snapshot().unwrap().document.graph().nodes().len(), 2);
}
#[test]
fn policy_defaults_to_read_only_and_creation_is_allowlisted() {
    let mut h = Harness::policy(Policy::default());
    let input = h.input();
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "policy_denied"
    );
    assert!(
        h.provider
            .dispatch("observe", json!({"query":"summary"}))
            .is_ok()
    );
    let mut h = Harness::new();
    let mut input = h.input();
    input["operations"][0]["type_id"] = json!("shell.execute");
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "policy_denied"
    );
}
#[test]
fn forged_ports_unknown_operations_and_partial_batches_do_not_mutate() {
    let mut h = Harness::new();
    let mut input = h.input();
    input["operations"][0]["outputs"] = json!([]);
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "invalid_request"
    );
    let mut input = h.input();
    let bad = json!({"kind":"delete_node","id":Id::new_v4()});
    input["operations"].as_array_mut().unwrap().push(bad);
    assert!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .is_err()
    );
    assert_eq!(h.host.snapshot().unwrap().revision, 0);
}
#[test]
fn document_identity_page_revision_and_limits_are_checked() {
    let mut h = Harness::new();
    let mut input = h.input();
    input["document_id"] = json!(Id::new_v4());
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "document_mismatch"
    );
    assert_eq!(
        h.provider
            .dispatch("observe", json!({"query":"nodes","limit":101}))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
    assert_eq!(
        h.provider
            .dispatch("observe", json!({"query":"nodes","expected_revision":99}))
            .unwrap_err()
            .code,
        "revision_conflict"
    );
    let definitions = h
        .provider
        .dispatch("observe", json!({"query":"definitions","limit":1}))
        .unwrap();
    assert_eq!(definitions["items"].as_array().unwrap().len(), 1);
    assert!(definitions["nextCursor"].is_string());
    let next = h
        .provider
        .dispatch(
            "observe",
            json!({"query":"definitions","limit":1,"after":definitions["nextCursor"]}),
        )
        .unwrap();
    assert_ne!(
        next["items"][0]["type_id"],
        definitions["items"][0]["type_id"]
    );
    assert!(next["nextCursor"].is_null());
}
#[test]
fn retention_limit_preserves_existing_receipts() {
    let mut h = Harness::policy(Policy {
        max_sessions: 1,
        ..Policy::math_demo()
    });
    let (_, _, commit) = h.plan();
    let result = h.execute(&commit);
    let input = h.input();
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "capacity_exceeded"
    );
    assert_eq!(h.execute(&commit), result);
}
#[test]
fn malformed_and_oversized_lines_are_bounded_and_next_line_still_works() {
    let mut h = Harness::new();
    let mut bytes = vec![b'x'; MAX_MESSAGE_BYTES + 20];
    bytes.extend_from_slice(
        b"\n{bad}\n{\"id\":\"ok\",\"method\":\"observe\",\"params\":{\"query\":\"summary\"}}\n",
    );
    let mut output = Vec::new();
    serve(
        &mut h.provider,
        std::io::Cursor::new(bytes),
        &mut output,
        || {},
    )
    .unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["error"]["code"], "limit_exceeded");
    assert_eq!(lines[1]["error"]["code"], "invalid_request");
    assert_eq!(lines[2]["result"]["revision"], 0);
}
#[test]
fn failed_run_has_failed_receipt_and_is_not_reexecuted() {
    let mut h = Harness::new();
    user_edit(&h); // Missing math.number.value causes a node failure.
    let snapshot = h.host.snapshot().unwrap();
    let input = json!({"kind":"run","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision});
    let pf = h.preflight(input);
    let auth = h.authorize(&pf);
    let commit = h.commit(&pf, &auth);
    let result = h.execute(&commit);
    assert_eq!(result["result"]["succeeded"], false);
    assert_eq!(h.execute(&commit), result);
    let receipt = h
        .provider
        .dispatch("receipt", json!({"receiptId":result["receiptId"]}))
        .unwrap();
    assert_eq!(receipt["status"], "failed");
    assert_eq!(receipt["recovery"]["reversible"], false);
}
#[test]
fn denied_run_type_cannot_hide_behind_pure_flag() {
    let mut policy = Policy::math_demo();
    policy.execute_types.clear();
    let mut h = Harness::policy(policy);
    user_edit(&h);
    let snapshot = h.host.snapshot().unwrap();
    let input = json!({"kind":"run","document_id":snapshot.document.graph().id,"expected_revision":snapshot.revision});
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "policy_denied"
    );
}
#[test]
fn integer_and_float_geometry_normalize_to_same_commit() {
    let mut h = Harness::new();
    let mut input = h.input();
    input["operations"][0]["rect"]["x"] = json!(10);
    let pf = h.preflight(input.clone());
    let auth = h.authorize(&pf);
    input["operations"][0]["rect"]["x"] = json!(10.0);
    assert!(h.provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":auth["authorization"],"input":input})).is_ok());
}
#[test]
fn batch_count_is_checked() {
    let mut h = Harness::new();
    let mut input = h.input();
    input["operations"] = json!(vec![json!({"kind":"auto_layout","gap":[40,20]}); 257]);
    assert_eq!(
        h.provider
            .dispatch("preflight", json!({"input":input}))
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}

#[test]
fn non_binary_fraction_has_identical_structured_and_hashed_input() {
    let mut h = Harness::new();
    let mut input = h.input();
    input["operations"][0]["rect"]["x"] = json!(0.1);
    let pf = h.preflight(input);
    let decoded: Value = serde_json::from_str(pf["requestJson"].as_str().unwrap()).unwrap();
    assert_eq!(decoded, pf["input"]);
    let auth = h.authorize(&pf);
    let commit = h.commit(&pf, &auth);
    h.execute(&commit);
    assert_eq!(h.host.snapshot().unwrap().revision, 1);
}

#[test]
fn host_race_failure_is_receipted_without_claiming_mutation_or_retrying() {
    use std::sync::atomic::AtomicUsize;
    struct RacingHost {
        inner: MemoryHost,
        attempts: AtomicUsize,
    }
    impl GraphHost for RacingHost {
        fn snapshot(&self) -> Result<Snapshot> {
            self.inner.snapshot()
        }
        fn apply(&self, _: Id, _: u64, _: Command) -> Result<Snapshot> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Err(AcxError::new(
                "revision_conflict",
                "a UI edit won the atomic host check",
            ))
        }
        fn undo(&self, id: Id, revision: u64) -> Result<Snapshot> {
            self.inner.undo(id, revision)
        }
    }
    let host = Arc::new(RacingHost {
        inner: MemoryHost::new(Document::default()).unwrap(),
        attempts: AtomicUsize::new(0),
    });
    let mut provider = Provider::new(host.clone(), Arc::new(math_registry()), Policy::math_demo());
    let input = json!({"kind":"edit","document_id":host.snapshot().unwrap().document.graph().id,"expected_revision":0,"operations":[{"kind":"auto_layout","gap":[10,10]}]});
    let pf = provider
        .dispatch("preflight", json!({"input":input}))
        .unwrap();
    let grant = provider
        .dispatch(
            "authorize",
            json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"]}),
        )
        .unwrap();
    let commit=provider.dispatch("commit",json!({"preflightId":pf["preflightId"],"preflightDigest":pf["preflightDigest"],"authorization":grant["authorization"],"input":pf["input"]})).unwrap();
    let args = json!({"commitId":commit["commitId"]});
    let result = provider.dispatch("execute", args.clone()).unwrap();
    assert_eq!(provider.dispatch("execute", args).unwrap(), result);
    assert_eq!(host.attempts.load(Ordering::SeqCst), 1);
    let receipt = provider
        .dispatch("receipt", json!({"receiptId":result["receiptId"]}))
        .unwrap();
    assert_eq!(receipt["status"], "failed");
    assert_eq!(receipt["effects"], json!([]));
}
