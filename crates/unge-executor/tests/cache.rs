use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use unge_core::*;
use unge_executor::*;

struct Emit {
    calls: Arc<AtomicUsize>,
    mode: u8,
}
impl NodeExecutor for Emit {
    fn execute(
        &self,
        ctx: ExecutionContext,
        _: Inputs,
    ) -> futures::future::BoxFuture<'_, std::result::Result<Outputs, String>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            match self.mode {
                1 => return Err("failure".into()),
                2 => ctx.cancellation.cancel(),
                3 => return Ok(Outputs::new()),
                _ => (),
            }
            Ok(Outputs::from([(
                "value".into(),
                Value::String(ctx.properties["text"].as_str().unwrap().into()),
            )]))
        })
    }
}
fn setup(pure: bool, mode: u8) -> (Registry, Arc<AtomicUsize>) {
    let text = LocalizedText {
        en: "Emit".into(),
        ja: "出力".into(),
        zh_cn: "输出".into(),
    };
    let definition = Definition {
        type_id: "emit".into(),
        version: "1".into(),
        name: text.clone(),
        description: text,
        inputs: vec![],
        outputs: vec![Port {
            name: "value".into(),
            data_type: DataType::String,
            cardinality: Cardinality::Single,
            required: true,
        }],
        pure,
        property_schema: PropertySchema::default(),
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = Registry::default();
    registry
        .register(
            definition,
            Arc::new(Emit {
                calls: calls.clone(),
                mode,
            }),
        )
        .unwrap();
    (registry, calls)
}
fn document(registry: &Registry, values: &[&str]) -> Document {
    let mut editor = Editor::new(Document::default(), 0).unwrap();
    for value in values {
        let mut node = registry.definition("emit").unwrap().instantiate();
        node.properties.insert("text".into(), (*value).into());
        editor
            .execute(Command::AddNode {
                node,
                rect: Rect::default(),
            })
            .unwrap();
    }
    editor.document().clone()
}
fn run(scheduler: &mut Scheduler, registry: &Registry, values: &[&str]) -> Vec<Status> {
    futures::executor::block_on(scheduler.run(
        document(registry, values).graph(),
        registry,
        Cancellation::default(),
    ))
    .unwrap()
    .nodes
    .into_values()
    .map(|node| node.status)
    .collect()
}
#[test]
fn hits_do_not_refresh_fifo_and_duplicates_take_one_slot() {
    let (registry, calls) = setup(true, 0);
    let mut scheduler = Scheduler::new(4, 2);
    assert_eq!(
        run(&mut scheduler, &registry, &["a", "a"]),
        vec![Status::Completed; 2]
    );
    assert_eq!(scheduler.cache_usage().entries, 1);
    run(&mut scheduler, &registry, &["b"]);
    assert_eq!(run(&mut scheduler, &registry, &["a"]), vec![Status::Cached]);
    run(&mut scheduler, &registry, &["c"]);
    assert_eq!(run(&mut scheduler, &registry, &["b"]), vec![Status::Cached]);
    assert_eq!(
        run(&mut scheduler, &registry, &["a"]),
        vec![Status::Completed]
    );
    assert_eq!(calls.load(Ordering::SeqCst), 5);
}
#[test]
fn exact_byte_boundary_oversized_outputs_and_keys_preserve_existing_entries() {
    let (registry, _) = setup(true, 0);
    let mut scheduler = Scheduler::new(1, 10);
    run(&mut scheduler, &registry, &["a"]);
    let bytes = scheduler.cache_usage().bytes;
    scheduler.set_cache_limits(CacheLimits {
        max_entries: 10,
        max_bytes: bytes,
    });
    assert_eq!(run(&mut scheduler, &registry, &["a"]), vec![Status::Cached]);
    // Key fits but key + outputs exceed the exact one-entry budget.
    assert_eq!(
        run(&mut scheduler, &registry, &["aa"]),
        vec![Status::Completed]
    );
    assert_eq!(scheduler.cache_usage().bytes, bytes);
    run(&mut scheduler, &registry, &[&"x".repeat(10_000)]);
    assert_eq!(run(&mut scheduler, &registry, &["a"]), vec![Status::Cached]);
    run(&mut scheduler, &registry, &["b"]);
    assert_eq!(scheduler.cache_usage().entries, 1);
    assert_eq!(run(&mut scheduler, &registry, &["b"]), vec![Status::Cached]);
    assert_eq!(
        run(&mut scheduler, &registry, &["a"]),
        vec![Status::Completed]
    );
    scheduler.set_cache_limits(CacheLimits {
        max_entries: 10,
        max_bytes: bytes - 1,
    });
    assert_eq!(scheduler.cache_usage().bytes, 0);
}
#[test]
fn limits_shrink_immediately_disable_and_clear_release_payloads() {
    let (registry, _) = setup(true, 0);
    let mut scheduler = Scheduler::new(1, 10);
    for value in ["a", "b", "c"] {
        run(&mut scheduler, &registry, &[value]);
    }
    scheduler.set_cache_limits(CacheLimits {
        max_entries: 1,
        max_bytes: usize::MAX,
    });
    assert_eq!(run(&mut scheduler, &registry, &["c"]), vec![Status::Cached]);
    for limits in [
        CacheLimits {
            max_entries: 0,
            max_bytes: usize::MAX,
        },
        CacheLimits {
            max_entries: 10,
            max_bytes: 0,
        },
    ] {
        scheduler.set_cache_limits(limits);
        assert_eq!(scheduler.cache_usage().entries, 0);
        assert_eq!(scheduler.cache_usage().bytes, 0);
        assert_eq!(
            run(&mut scheduler, &registry, &["c"]),
            vec![Status::Completed]
        );
    }
    scheduler.set_cache_limits(CacheLimits::default());
    run(&mut scheduler, &registry, &["c"]);
    scheduler.clear_cache();
    assert_eq!(scheduler.cache_usage().bytes, 0);
    assert_eq!(
        run(&mut scheduler, &registry, &["c"]),
        vec![Status::Completed]
    );
}
#[test]
fn registry_executor_identity_isolated_and_cancelled_run_ignores_warm_cache() {
    let (registry, _) = setup(true, 0);
    let mut scheduler = Scheduler::new(1, 10);
    run(&mut scheduler, &registry, &["a"]);
    let cancel = Cancellation::default();
    cancel.cancel();
    let report = futures::executor::block_on(scheduler.run(
        document(&registry, &["a"]).graph(),
        &registry,
        cancel,
    ))
    .unwrap();
    assert!(
        report
            .nodes
            .values()
            .all(|node| node.status == Status::Cancelled)
    );
    let (other, calls) = setup(true, 0);
    assert_eq!(run(&mut scheduler, &other, &["a"]), vec![Status::Completed]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(run(&mut scheduler, &other, &["a"]), vec![Status::Cached]);
}
#[test]
fn impure_failed_cancelled_and_invalid_outputs_are_never_cached() {
    for (pure, mode, status) in [
        (false, 0, Status::Completed),
        (true, 1, Status::Failed),
        (true, 2, Status::Cancelled),
        (true, 3, Status::Failed),
    ] {
        let (registry, calls) = setup(pure, mode);
        let mut scheduler = Scheduler::new(1, 10);
        for _ in 0..2 {
            assert_eq!(run(&mut scheduler, &registry, &["a"]), vec![status.clone()]);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(scheduler.cache_usage().bytes, 0);
    }
}

#[test]
fn shared_executor_versions_are_distinct_and_clear_releases_retained_executor() {
    let (source, _) = setup(true, 0);
    let executor: Arc<dyn NodeExecutor> = Arc::new(Emit {
        calls: Arc::new(AtomicUsize::new(0)),
        mode: 0,
    });
    let weak = Arc::downgrade(&executor);
    let mut old = Registry::default();
    let mut new = Registry::default();
    let definition = source.definition("emit").unwrap().clone();
    old.register(definition.clone(), executor.clone()).unwrap();
    let mut next = definition;
    next.version = "2".into();
    new.register(next, executor.clone()).unwrap();
    let mut scheduler = Scheduler::new(1, 10);
    run(&mut scheduler, &old, &["a"]);
    assert_eq!(run(&mut scheduler, &new, &["a"]), vec![Status::Completed]);
    assert_eq!(run(&mut scheduler, &new, &["a"]), vec![Status::Cached]);
    drop(old);
    drop(new);
    drop(executor);
    assert!(weak.upgrade().is_some());
    scheduler.clear_cache();
    assert!(weak.upgrade().is_none());
}
