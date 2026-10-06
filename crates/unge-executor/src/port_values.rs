//! Definition-specific executor value-map validation and optional Draft 7 export.
use crate::{Definition, Inputs, Outputs};
use std::collections::BTreeSet;
use unge_core::{Cardinality, Port};
fn check_ports(ports: &[Port]) -> Result<(), String> {
    let mut names = BTreeSet::new();
    if ports
        .iter()
        .any(|p| p.name.is_empty() || !names.insert(&p.name))
    {
        return Err("empty or duplicate port name".into());
    }
    Ok(())
}
impl Definition {
    /// Inputs contain arrays even for Single ports. Missing optional ports are allowed.
    pub fn validate_inputs(&self, inputs: &Inputs) -> Result<(), String> {
        check_ports(&self.inputs)?;
        for port in &self.inputs {
            if port.required && inputs.get(&port.name).is_none_or(Vec::is_empty) {
                return Err(format!("missing required input: {}", port.name));
            }
        }
        for (name, values) in inputs {
            let port = self
                .inputs
                .iter()
                .find(|p| p.name == *name)
                .ok_or_else(|| format!("unknown input: {name}"))?;
            if port.cardinality == Cardinality::Single && values.len() > 1 {
                return Err(format!("too many input values: {name}"));
            }
            if values
                .iter()
                .any(|v| !v.valid() || !port.data_type.accepts(&v.data_type()))
            {
                return Err(format!("invalid input type: {name}"));
            }
        }
        Ok(())
    }
    pub fn validate_outputs(&self, outputs: &Outputs) -> Result<(), String> {
        check_ports(&self.outputs)?;
        crate::validate_outputs(&self.outputs, outputs)
    }
    #[cfg(feature = "schema")]
    pub fn input_values_schema(&self) -> Result<serde_json::Value, String> {
        schema(&self.inputs, true)
    }
    #[cfg(feature = "schema")]
    pub fn output_values_schema(&self) -> Result<serde_json::Value, String> {
        schema(&self.outputs, false)
    }
}
#[cfg(feature = "schema")]
fn schema(ports: &[Port], inputs: bool) -> Result<serde_json::Value, String> {
    use crate::Value;
    use serde_json::{Value as Json, json};
    use unge_core::DataType;
    check_ports(ports)?;
    // Derive the wire shape, including resource IDs, from the actual serde enum.
    let wire = serde_json::to_value(schemars::schema_for!(Value)).map_err(|e| e.to_string())?;
    let variants = wire["oneOf"]
        .as_array()
        .ok_or("Value schema must be a tagged union")?;
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();
    for port in ports {
        let mut accepted = Vec::new();
        for variant in variants {
            let kind = variant["properties"]["kind"]["enum"][0]
                .as_str()
                .ok_or("missing Value tag")?;
            let primitive = match kind {
                "bool" => Some(DataType::Bool),
                "int" => Some(DataType::Int),
                "float" => Some(DataType::Float),
                "string" => Some(DataType::String),
                "json" => Some(DataType::Json),
                "resource" => None,
                _ => return Err("unknown Value variant".into()),
            };
            if primitive
                .as_ref()
                .is_some_and(|t| !port.data_type.accepts(t))
            {
                continue;
            }
            let mut variant = variant.clone();
            if kind == "resource" && port.data_type != DataType::Any {
                let encoded = serde_json::to_value(&port.data_type).map_err(|e| e.to_string())?;
                let mut data_properties = serde_json::Map::new();
                let mut data_required = Vec::new();
                for (key, value) in encoded.as_object().ok_or("DataType must be an object")? {
                    data_properties.insert(key.clone(), json!({"const":value}));
                    data_required.push(key);
                }
                variant["properties"]["value"]["properties"]["data_type"] =
                    json!({"type":"object","properties":data_properties,"required":data_required});
            }
            if kind == "int" {
                variant["properties"]["value"]["minimum"] = json!(i64::MIN);
                variant["properties"]["value"]["maximum"] = json!(i64::MAX);
            }
            if kind == "float" {
                variant["properties"]["value"]["minimum"] = json!(-f64::MAX);
                variant["properties"]["value"]["maximum"] = json!(f64::MAX);
            }
            accepted.push(variant);
        }
        let mut value = json!({"oneOf":accepted});
        if inputs {
            value = json!({"type":"array","items":value});
            if port.required {
                value["minItems"] = json!(1);
            }
            if port.cardinality == Cardinality::Single {
                value["maxItems"] = json!(1);
            }
        }
        properties.insert(port.name.clone(), value);
        if port.required {
            required.push(&port.name);
        }
    }
    Ok(
        json!({"$schema":"http://json-schema.org/draft-07/schema#","type":"object","properties":properties,"required":required,"additionalProperties":false,"definitions":wire.get("definitions").cloned().unwrap_or(Json::Object(Default::default()))}),
    )
}
