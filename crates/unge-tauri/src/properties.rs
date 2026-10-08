//! One-node metadata snapshots. Property writes use the existing revision-checked Batch command.
use super::*;
use unge_executor::{LocalizedText, PropertySchema, Registry};

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeProperties {
    pub id: Id,
    pub revision: u64,
    pub type_id: String,
    pub version: String,
    pub name: LocalizedText,
    pub description: LocalizedText,
    pub schema: PropertySchema,
    pub properties: Properties,
}
impl Engine {
    /// Install the same immutable Registry for inspector metadata and edit validation.
    /// Configure before sharing clones. Registry contains trusted host code only.
    pub fn from_registry(
        document: Document,
        history_capacity: usize,
        registry: Arc<Registry>,
    ) -> Result<Self> {
        let editor = Editor::new(document, history_capacity)?.with_validator(registry.clone())?;
        let mut engine = Self::from_editor(editor);
        engine.registry = Some(registry);
        Ok(engine)
    }
    pub fn node_properties(
        &self,
        view: &str,
        id: Id,
        expected_revision: u64,
    ) -> ApiResult<NodeProperties> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        if state.editor.revision() != expected_revision {
            return Err(ApiError::new("revision_conflict", state.editor.revision()));
        }
        let registry = self.registry.as_ref().ok_or_else(|| {
            ApiError::new("properties_unavailable", "configure Engine::from_registry")
        })?;
        let node = state
            .editor
            .document()
            .graph()
            .nodes()
            .get(&id)
            .ok_or_else(|| ApiError::new("missing_node", id))?;
        let definition = registry
            .definition(&node.type_id)
            .ok_or_else(|| ApiError::new("properties_unavailable", &node.type_id))?;
        if definition.property_schema.fields.len() > 128 {
            return Err(ApiError::new(
                "limit_exceeded",
                "inspector supports up to 128 fields",
            ));
        }
        let result = NodeProperties {
            id,
            revision: state.editor.revision(),
            type_id: node.type_id.clone(),
            version: definition.version.clone(),
            name: definition.name.clone(),
            description: definition.description.clone(),
            schema: definition.property_schema.clone(),
            properties: node.properties.clone(),
        };
        if serde_json::to_vec(&result)
            .map_err(|e| ApiError::new("invalid_node", e))?
            .len()
            > 256 * 1024
        {
            return Err(ApiError::new(
                "limit_exceeded",
                "inspector metadata exceeds 256 KiB",
            ));
        }
        Ok(result)
    }
}
