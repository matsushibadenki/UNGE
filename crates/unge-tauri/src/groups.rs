use super::*;

/// UI operations derive membership from the invoking Rust view, never a JS graph mirror.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GroupAction {
    Create { id: Id, label: String },
    Rename { id: Id, label: String },
    AddSelection { id: Id },
    RemoveSelection { id: Id },
    Delete { id: Id },
    SelectMembers { id: Id },
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSummary {
    pub id: Id,
    pub label: String,
    pub members: usize,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPage {
    pub revision: u64,
    pub total: usize,
    pub selected_nodes: usize,
    pub groups: Vec<GroupSummary>,
    pub next: Option<Id>,
}
fn check_label(label: &str) -> ApiResult<()> {
    if label.trim().is_empty() || label.chars().count() > 256 {
        return Err(ApiError::new(
            "invalid_group_label",
            "group label must contain 1..256 characters",
        ));
    }
    Ok(())
}
pub(super) fn apply_group(
    state: &mut State,
    view: &str,
    action: GroupAction,
) -> ApiResult<Option<unge_render::SceneChanges>> {
    let id = match &action {
        GroupAction::Create { id, .. }
        | GroupAction::Rename { id, .. }
        | GroupAction::AddSelection { id }
        | GroupAction::RemoveSelection { id }
        | GroupAction::Delete { id }
        | GroupAction::SelectMembers { id } => *id,
    };
    let old = state.editor.document().graph().groups().get(&id);
    if matches!(action, GroupAction::Create { .. }) {
        if old.is_some() {
            return Err(ApiError::new("group_exists", id));
        }
    } else if old.is_none() {
        return Err(ApiError::new("missing_group", id));
    }
    if matches!(action, GroupAction::SelectMembers { .. }) {
        let members = &old.unwrap().nodes;
        if members.len() > 10_000 {
            return Err(ApiError::new(
                "invalid_selection",
                "group selection exceeds 10000",
            ));
        }
        let members = members.clone();
        let v = state.views.get_mut(view).unwrap();
        v.interaction.cancel(&mut v.viewport, &mut v.selection);
        v.selection = members;
        return Ok(None);
    }
    let selection = &state.views[view].selection;
    let group = match action {
        GroupAction::Create { label, .. } => {
            check_label(&label)?;
            if selection.is_empty() {
                return Err(ApiError::new(
                    "invalid_selection",
                    "select nodes before creating a group",
                ));
            }
            Some(Group {
                id,
                label,
                nodes: selection.clone(),
            })
        }
        GroupAction::Rename { label, .. } => {
            check_label(&label)?;
            let mut group = old.unwrap().clone();
            group.label = label;
            Some(group)
        }
        GroupAction::AddSelection { .. } | GroupAction::RemoveSelection { .. } => {
            let add = matches!(action, GroupAction::AddSelection { .. });
            let mut group = old.unwrap().clone();
            if add {
                group.nodes.extend(selection);
            } else {
                group.nodes.retain(|id| !selection.contains(id));
            }
            if group.nodes.len() > 10_000 {
                return Err(ApiError::new(
                    "invalid_selection",
                    "group membership edit exceeds 10000",
                ));
            }
            Some(group)
        }
        GroupAction::Delete { .. } => None,
        GroupAction::SelectMembers { .. } => unreachable!(),
    };
    if old == group.as_ref() {
        return Ok(None);
    }
    let command = Command::SetGroup { id, group };
    let changes = unge_render::SceneChanges::from_command(&command);
    state.editor.execute(command)?;
    Ok(Some(changes))
}
impl Engine {
    /// Bounded group metadata; memberships remain in Rust. Same exclusive cursor contract as nodes.
    pub fn groups(
        &self,
        view: &str,
        expected_revision: u64,
        after: Option<Id>,
        limit: usize,
    ) -> ApiResult<GroupPage> {
        if !(1..=100).contains(&limit) {
            return Err(ApiError::new(
                "invalid_request",
                "page limit must be 1..100",
            ));
        }
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        if expected_revision != state.editor.revision() {
            return Err(ApiError::new("revision_conflict", state.editor.revision()));
        }
        let all = state.editor.document().graph().groups();
        let mut iter = all.range((
            after.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded),
            std::ops::Bound::Unbounded,
        ));
        let groups: Vec<_> = iter
            .by_ref()
            .take(limit)
            .map(|(id, g)| GroupSummary {
                id: *id,
                label: g
                    .label
                    .chars()
                    .take(256)
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect(),
                members: g.nodes.len(),
            })
            .collect();
        let next = if iter.next().is_some() {
            groups.last().map(|g| g.id)
        } else {
            None
        };
        Ok(GroupPage {
            revision: state.editor.revision(),
            total: all.len(),
            selected_nodes: v.selection.len(),
            groups,
            next,
        })
    }
}
