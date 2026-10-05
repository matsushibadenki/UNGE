//! Portable property metadata and validation. This is a typed subset, not a JSON
//! Schema interpreter. String lengths count Unicode scalar values (Rust chars).
use crate::LocalizedText;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use unge_core::Properties;

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PropertyType {
    Bool,
    Int {
        minimum: Option<i64>,
        maximum: Option<i64>,
    },
    Float {
        minimum: Option<f64>,
        maximum: Option<f64>,
    },
    String {
        #[serde(default)]
        min_length: usize,
        max_length: Option<usize>,
        choices: Option<Vec<String>>,
    },
    Json,
}
impl PropertyType {
    pub fn validate(&self, value: &Value) -> Result<(), String> {
        let valid = match self {
            Self::Bool => value.is_boolean(),
            Self::Int { minimum, maximum } => value.as_i64().is_some_and(|v| {
                minimum.is_none_or(|min| v >= min) && maximum.is_none_or(|max| v <= max)
            }),
            Self::Float { minimum, maximum } => value.as_f64().is_some_and(|v| {
                v.is_finite()
                    && minimum.is_none_or(|min| v >= min)
                    && maximum.is_none_or(|max| v <= max)
            }),
            Self::String {
                min_length,
                max_length,
                choices,
            } => value.as_str().is_some_and(|v| {
                let len = v.chars().count();
                len >= *min_length
                    && max_length.is_none_or(|max| len <= max)
                    && choices
                        .as_ref()
                        .is_none_or(|items| items.iter().any(|item| item == v))
            }),
            Self::Json => true,
        };
        if valid {
            Ok(())
        } else {
            Err("value does not match property type or constraints".into())
        }
    }
    fn validate_schema(&self) -> Result<(), String> {
        let valid = match self {
            Self::Int { minimum, maximum } => {
                minimum.zip(*maximum).is_none_or(|(min, max)| min <= max)
            }
            Self::Float { minimum, maximum } => {
                minimum.is_none_or(f64::is_finite)
                    && maximum.is_none_or(f64::is_finite)
                    && minimum.zip(*maximum).is_none_or(|(min, max)| min <= max)
            }
            Self::String {
                min_length,
                max_length,
                choices,
            } => {
                max_length.is_none_or(|max| *min_length <= max)
                    && choices.as_ref().is_none_or(|items| {
                        !items.is_empty()
                            && items.iter().all(|item| {
                                let len = item.chars().count();
                                len >= *min_length && max_length.is_none_or(|max| len <= max)
                            })
                            && items
                                .iter()
                                .collect::<std::collections::BTreeSet<_>>()
                                .len()
                                == items.len()
                    })
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err("invalid property constraints".into())
        }
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertyDefinition {
    pub name: LocalizedText,
    pub description: LocalizedText,
    pub value_type: PropertyType,
    #[serde(default)]
    pub required: bool,
    /// Applied only by Definition::instantiate; validation never repairs values.
    /// Null represents no default, matching SetProperty's deletion convention.
    pub default: Option<Value>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropertySchema {
    #[serde(default)]
    pub fields: BTreeMap<String, PropertyDefinition>,
    #[serde(default = "allow_additional")]
    pub additional_properties: bool,
}
fn allow_additional() -> bool {
    true
}
impl Default for PropertySchema {
    fn default() -> Self {
        Self {
            fields: BTreeMap::new(),
            additional_properties: true,
        }
    }
}
impl PropertySchema {
    /// Export value constraints for host tools. Metadata derives describe the
    /// declaration itself; this describes the properties accepted by a node.
    #[cfg(feature = "schema")]
    pub fn json_schema(&self) -> Result<Value, String> {
        self.validate_schema()?;
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        for (key, field) in &self.fields {
            let mut schema = serde_json::Map::new();
            match &field.value_type {
                PropertyType::Bool => {
                    schema.insert("type".into(), "boolean".into());
                }
                PropertyType::Int { minimum, maximum } => {
                    schema.insert("type".into(), "integer".into());
                    // validate() accepts only values representable as i64.
                    schema.insert("minimum".into(), minimum.unwrap_or(i64::MIN).into());
                    schema.insert("maximum".into(), maximum.unwrap_or(i64::MAX).into());
                }
                PropertyType::Float { minimum, maximum } => {
                    schema.insert("type".into(), "number".into());
                    if let Some(min) = minimum {
                        schema.insert("minimum".into(), (*min).into());
                    }
                    if let Some(max) = maximum {
                        schema.insert("maximum".into(), (*max).into());
                    }
                }
                PropertyType::String {
                    min_length,
                    max_length,
                    choices,
                } => {
                    schema.insert("type".into(), "string".into());
                    schema.insert("minLength".into(), (*min_length).into());
                    if let Some(max) = max_length {
                        schema.insert("maxLength".into(), (*max).into());
                    }
                    if let Some(items) = choices {
                        schema.insert("enum".into(), serde_json::json!(items));
                    }
                }
                PropertyType::Json => {}
            }
            // Keep all translations; consumers choose their locale.
            schema.insert("x-unge-name".into(), serde_json::json!(field.name));
            schema.insert(
                "x-unge-description".into(),
                serde_json::json!(field.description),
            );
            if let Some(default) = &field.default {
                schema.insert("default".into(), default.clone());
            }
            properties.insert(key.clone(), schema.into());
            if field.required {
                required.push(key);
            }
        }
        Ok(serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object", "properties": properties,
            "required": required, "additionalProperties": self.additional_properties
        }))
    }
    pub fn defaults(&self) -> Properties {
        self.fields
            .iter()
            .filter_map(|(key, field)| field.default.clone().map(|v| (key.clone(), v)))
            .collect()
    }
    pub fn validate_schema(&self) -> Result<(), String> {
        for (key, field) in &self.fields {
            if key.is_empty() {
                return Err("empty property name".into());
            }
            field
                .value_type
                .validate_schema()
                .map_err(|e| format!("{key}: {e}"))?;
            if let Some(default) = &field.default {
                if default.is_null() {
                    return Err(format!("{key}: null defaults are unsupported"));
                }
                field
                    .value_type
                    .validate(default)
                    .map_err(|e| format!("{key} default: {e}"))?;
            }
        }
        Ok(())
    }
    pub fn validate(&self, properties: &Properties) -> Result<(), String> {
        for (key, field) in &self.fields {
            match properties.get(key) {
                Some(value) => field
                    .value_type
                    .validate(value)
                    .map_err(|e| format!("{key}: {e}"))?,
                None if field.required => return Err(format!("{key}: required property missing")),
                None => {}
            }
        }
        if !self.additional_properties
            && let Some(key) = properties
                .keys()
                .find(|key| !self.fields.contains_key(*key))
        {
            return Err(format!("{key}: unknown property"));
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "schema"))]
mod schema_tests {
    use super::*;
    fn field(value_type: PropertyType, default: Option<Value>) -> PropertyDefinition {
        let text = LocalizedText {
            en: "Value".into(),
            ja: "値".into(),
            zh_cn: "值".into(),
        };
        PropertyDefinition {
            name: text.clone(),
            description: text,
            value_type,
            required: false,
            default,
        }
    }
    #[test]
    fn invalid_declarations_are_not_exported() {
        let schema = PropertySchema {
            fields: [(
                "value".into(),
                field(
                    PropertyType::Int {
                        minimum: Some(3),
                        maximum: Some(2),
                    },
                    None,
                ),
            )]
            .into(),
            additional_properties: false,
        };
        assert!(schema.json_schema().is_err());
        let schema = PropertySchema {
            fields: [("value".into(), field(PropertyType::Bool, Some(Value::Null)))].into(),
            additional_properties: true,
        };
        assert!(schema.json_schema().is_err());
    }
    #[test]
    fn defaults_and_translations_are_annotations() {
        let schema = PropertySchema {
            fields: [(
                "value".into(),
                field(PropertyType::Bool, Some(Value::Bool(true))),
            )]
            .into(),
            additional_properties: false,
        };
        let exported = schema.json_schema().unwrap();
        assert_eq!(exported["properties"]["value"]["default"], true);
        assert_eq!(exported["properties"]["value"]["x-unge-name"]["ja"], "値");
        assert_eq!(
            exported["properties"]["value"]["x-unge-name"]["zh_cn"],
            "值"
        );
        assert_eq!(exported["required"], serde_json::json!([]));
        assert!(schema.validate(&Properties::new()).is_ok());
    }
}
