//! Build-time exporter; never invoked through IPC.
use schemars::{JsonSchema, schema::RootSchema};
use std::collections::BTreeMap;
fn add<T: JsonSchema>(schemas: &mut BTreeMap<String, RootSchema>) {
    schemas.insert(T::schema_name(), schemars::schema_for!(T));
}
fn fixtures() -> serde_json::Value {
    use unge_core::{Command, Document, Id, Rect};
    use unge_executor::{Definition, math_registry};
    let registry = math_registry();
    let definition: &Definition = registry.definition("math.number").expect("number");
    let node = definition.instantiate();
    let command = Command::Batch {
        commands: vec![
            Command::AddNode {
                node,
                rect: Rect::default(),
            },
            Command::SetProperty {
                id: Id::new_v4(),
                key: "value".into(),
                value: None,
            },
            Command::SetGroup {
                id: Id::new_v4(),
                group: None,
            },
        ],
    };
    serde_json::json!({
        "Document": Document::default(), "Definition": definition,
        "Command": command, "Request": unge_tauri::Request::Apply { expected_revision: 0, command },
        "property_values_schema": definition.property_schema.json_schema().expect("property schema"),
        "property_cases": property_cases(), "port_cases": port_cases()
    })
}
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--port-schemas") {
        println!("{}", port_schemas());
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("--fixtures") {
        println!("{}", fixtures());
        return;
    }
    let mut schemas = BTreeMap::new();
    add::<unge_core::Document>(&mut schemas);
    add::<unge_core::Command>(&mut schemas);
    add::<unge_executor::Definition>(&mut schemas);
    add::<unge_executor::Value>(&mut schemas);
    add::<unge_executor::ProgressEvent>(&mut schemas);
    add::<unge_executor::RunSummary>(&mut schemas);
    add::<unge_tauri::Request>(&mut schemas);
    add::<unge_tauri::Summary>(&mut schemas);
    add::<unge_tauri::SelectionSummary>(&mut schemas);
    add::<unge_tauri::ApiError>(&mut schemas);
    add::<unge_tauri::Appearance>(&mut schemas);
    add::<unge_tauri::AccessiblePage>(&mut schemas);
    add::<unge_tauri::GroupPage>(&mut schemas);
    add::<unge_tauri::NodeProperties>(&mut schemas);
    println!(
        "{}",
        serde_json::to_string(&schemas).expect("serialize contracts")
    );
}

fn property_cases() -> serde_json::Value {
    use unge_executor::{LocalizedText, PropertyDefinition, PropertySchema, PropertyType};
    let text = LocalizedText {
        en: "Value".into(),
        ja: "値".into(),
        zh_cn: "值".into(),
    };
    let mut cases = Vec::new();
    for value_type in [
        PropertyType::Bool,
        PropertyType::Json,
        PropertyType::Int {
            minimum: Some(-2),
            maximum: Some(2),
        },
        PropertyType::Int {
            minimum: None,
            maximum: None,
        },
        PropertyType::Float {
            minimum: Some(-2.0),
            maximum: Some(2.0),
        },
        PropertyType::String {
            min_length: 1,
            max_length: Some(2),
            choices: Some(vec!["日".into(), "中文".into(), "😀".into()]),
        },
    ] {
        let schema = PropertySchema {
            fields: [(
                "value".into(),
                PropertyDefinition {
                    name: text.clone(),
                    description: text.clone(),
                    value_type,
                    required: true,
                    default: None,
                },
            )]
            .into(),
            additional_properties: false,
        };
        let samples = [
            serde_json::json!({}),
            serde_json::json!({"value": null}),
            serde_json::json!({"value": true}),
            serde_json::json!({"value": 2}),
            serde_json::json!({"value": 3}),
            serde_json::json!({"value": 2.5}),
            serde_json::json!({"value": u64::MAX}),
            serde_json::json!({"value": "日"}),
            serde_json::json!({"value": "中文"}),
            serde_json::json!({"value": "😀"}),
            serde_json::json!({"value": "abc"}),
            serde_json::json!({"value": [], "extra": 1}),
        ];
        let values: Vec<_> = samples
            .into_iter()
            .map(|value| {
                let properties = serde_json::from_value(value.clone()).expect("map");
                serde_json::json!({ "value": value, "valid": schema.validate(&properties).is_ok() })
            })
            .collect();
        cases.push(serde_json::json!({ "schema": schema.json_schema().expect("schema"), "samples": values }));
    }
    serde_json::json!(cases)
}

fn port_cases() -> serde_json::Value {
    use unge_core::{Cardinality, DataType, Id, Port};
    use unge_executor::{Inputs, Outputs, Value, math_registry};
    let types = vec![
        DataType::Bool,
        DataType::Int,
        DataType::Float,
        DataType::String,
        DataType::Json,
        DataType::Bytes,
        DataType::Image,
        DataType::Audio,
        DataType::Video,
        DataType::Tensor,
        DataType::Event,
        DataType::Any,
        DataType::Custom("image.x".into()),
    ];
    let mut values = vec![
        Value::Bool(true),
        Value::Int(2),
        Value::Float(2.5),
        Value::String("组".into()),
        Value::Json(serde_json::json!(null)),
    ];
    values.extend(types.iter().cloned().map(|data_type| Value::Resource {
        id: Id::from_u128(1),
        data_type,
    }));
    let base = math_registry();
    let mut cases = Vec::new();
    for data_type in types {
        for cardinality in [Cardinality::Single, Cardinality::Multiple] {
            for required in [false, true] {
                let mut definition = base.definition("math.number").unwrap().clone();
                let port = Port {
                    name: "value / 组".into(),
                    data_type: data_type.clone(),
                    cardinality,
                    required,
                };
                definition.inputs = vec![port.clone()];
                definition.outputs = vec![port];
                let mut inputs: Vec<Inputs> = vec![
                    Inputs::new(),
                    [("value / 组".into(), vec![])].into(),
                    [("unknown".into(), vec![])].into(),
                ];
                let mut outputs: Vec<Outputs> = vec![
                    Outputs::new(),
                    [("unknown".into(), Value::Bool(true))].into(),
                ];
                for value in &values {
                    inputs.push([("value / 组".into(), vec![value.clone()])].into());
                    inputs.push([("value / 组".into(), vec![value.clone(), value.clone()])].into());
                    outputs.push([("value / 组".into(), value.clone())].into());
                }
                cases.push(serde_json::json!({"input_schema":definition.input_values_schema().unwrap(),"output_schema":definition.output_values_schema().unwrap(),"inputs":inputs.into_iter().map(|v|serde_json::json!({"valid":definition.validate_inputs(&v).is_ok(),"value":v})).collect::<Vec<_>>(),"outputs":outputs.into_iter().map(|v|serde_json::json!({"valid":definition.validate_outputs(&v).is_ok(),"value":v})).collect::<Vec<_>>() }));
            }
        }
    }
    serde_json::json!(cases)
}
fn port_schemas() -> serde_json::Value {
    let registry = unge_executor::math_registry();
    let schemas:std::collections::BTreeMap<_,_>=registry.definitions().map(|d|(d.type_id.clone(),serde_json::json!({"version":d.version,"inputs":d.input_values_schema().unwrap(),"outputs":d.output_values_schema().unwrap()}))).collect();
    serde_json::json!(schemas)
}
