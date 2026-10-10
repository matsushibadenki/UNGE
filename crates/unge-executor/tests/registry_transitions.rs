use std::sync::Arc;
use unge_core::*;
use unge_executor::*;

struct Full(Arc<Registry>);
impl DocumentValidator for Full {
    fn validate(&self, doc: &Document) -> unge_core::Result<()> {
        self.0.validate_edit(doc.graph())
    }
}
fn same(a: &Editor, b: &Editor) {
    assert_eq!(a.document(), b.document());
    assert_eq!(a.revision(), b.revision());
    assert_eq!(a.history_stats(), b.history_stats());
}
#[test]
fn optimized_registry_matches_full_validation_for_mixed_batches_and_history() {
    let registry = Arc::new(math_registry());
    let mut fast = Editor::new(Document::default(), 8)
        .unwrap()
        .with_validator(registry.clone())
        .unwrap();
    let mut full = Editor::new(fast.document().clone(), 8)
        .unwrap()
        .with_validator(Arc::new(Full(registry.clone())))
        .unwrap();
    let node = registry.definition("math.number").unwrap().instantiate();
    let id = node.id;
    let moved = |x| Command::MoveNode {
        id,
        rect: Rect {
            x,
            ..Rect::default()
        },
    };
    let property = |v| Command::SetProperty {
        id,
        key: "value".into(),
        value: Some(v),
    };
    let mut forged = node.clone();
    forged.id = Id::new_v4();
    forged.outputs.clear();
    let mut unknown = node.clone();
    unknown.id = Id::new_v4();
    unknown.type_id = "unknown".into();
    let operations = vec![
        Command::AddNode {
            node,
            rect: Rect::default(),
        },
        Command::Batch {
            commands: vec![
                moved(10.),
                Command::Batch {
                    commands: vec![moved(20.)],
                },
            ],
        },
        Command::Batch {
            commands: vec![moved(30.), property("bad".into())],
        },
        Command::Batch {
            commands: vec![
                property("temporary".into()),
                moved(40.),
                property(42.into()),
            ],
        },
        Command::AddNode {
            node: forged,
            rect: Rect::default(),
        },
        Command::AddNode {
            node: unknown,
            rect: Rect::default(),
        },
        Command::SetProperty {
            id,
            key: "value".into(),
            value: None,
        },
        Command::Batch { commands: vec![] },
        moved(50.),
    ];
    for command in operations {
        assert_eq!(
            fast.execute(command.clone()).map_err(|e| e.to_string()),
            full.execute(command).map_err(|e| e.to_string())
        );
        same(&fast, &full);
        registry.validate_edit(fast.document().graph()).unwrap();
    }
    while fast.undo().unwrap() {
        assert!(full.undo().unwrap());
        same(&fast, &full);
        registry.validate_edit(fast.document().graph()).unwrap();
    }
    assert!(!full.undo().unwrap());
    while fast.redo().unwrap() {
        assert!(full.redo().unwrap());
        same(&fast, &full);
        registry.validate_edit(fast.document().graph()).unwrap();
    }
    assert!(!full.redo().unwrap());
}
#[test]
fn direct_registry_checks_and_loading_still_reject_untrusted_graphs() {
    let registry = Arc::new(math_registry());
    let mut node = registry.definition("math.number").unwrap().instantiate();
    node.properties.insert("value".into(), "invalid".into());
    let mut raw = Editor::new(Document::default(), 0).unwrap();
    raw.execute(Command::AddNode {
        node,
        rect: Rect::default(),
    })
    .unwrap();
    assert!(registry.validate_edit(raw.document().graph()).is_err());
    assert!(registry.validate(raw.document().graph()).is_err());
    assert!(
        Editor::new(raw.document().clone(), 0)
            .unwrap()
            .with_validator(registry.clone())
            .is_err()
    );
    // Deserialized graphs must still receive structural checks on direct calls.
    let mut e = Editor::new(Document::default(), 0).unwrap();
    let mut a = registry.definition("math.add").unwrap().instantiate();
    a.id = Id::from_u128(1);
    let mut b = a.clone();
    b.id = Id::from_u128(2);
    e.execute(Command::Batch {
        commands: vec![
            Command::AddNode {
                node: a,
                rect: Rect::default(),
            },
            Command::AddNode {
                node: b,
                rect: Rect::default(),
            },
        ],
    })
    .unwrap();
    let mut wire = serde_json::to_value(e.document()).unwrap();
    let edges = serde_json::json!({
        Id::from_u128(10).to_string():{"id":Id::from_u128(10),"from":{"node":Id::from_u128(1),"port":"value"},"to":{"node":Id::from_u128(2),"port":"a"}},
        Id::from_u128(11).to_string():{"id":Id::from_u128(11),"from":{"node":Id::from_u128(2),"port":"value"},"to":{"node":Id::from_u128(1),"port":"a"}}
    });
    wire["graph"]["edges"] = edges;
    let malformed: Document = serde_json::from_value(wire).unwrap();
    assert!(matches!(
        registry.validate_edit(malformed.graph()),
        Err(Error::Cycle)
    ));
    assert_eq!(
        registry.validate(malformed.graph()).unwrap_err(),
        Error::Cycle.to_string()
    );
}
