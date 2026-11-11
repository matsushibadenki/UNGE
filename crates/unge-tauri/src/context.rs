use super::*;
use unge_interaction::{Hit, PointerGeometry, hit_test, proxy_hit};

pub const MAX_CONTEXT_PORT_EDGES: usize = 2048;

/// A read-only hit in the committed canvas, before any menu action changes selection.
#[derive(Debug, Clone)]
pub enum ContextTarget {
    Stage,
    Node {
        id: Id,
        rect: Rect,
        selected: bool,
    },
    Group(GroupSummary),
    Port {
        endpoint: Endpoint,
        output: bool,
        label: String,
        edges: Vec<Id>,
    },
    Edge {
        id: Id,
        from: Endpoint,
        to: Endpoint,
        labels: [String; 2],
    },
}

#[derive(Debug, Clone)]
pub struct CanvasContext {
    pub revision: u64,
    pub locale: Locale,
    pub world: [f32; 2],
    pub viewport: Viewport,
    pub selection_count: usize,
    pub target: ContextTarget,
}

impl Engine {
    pub fn canvas_context(&self, view: &str, position: [f32; 2]) -> ApiResult<CanvasContext> {
        let state = self.lock_state()?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        if v.interaction.is_active()
            || position
                .iter()
                .zip(v.viewport.size)
                .any(|(p, size)| !p.is_finite() || *p < 0. || *p >= size)
        {
            return Err(ApiError::new(
                "invalid_pointer",
                "context outside idle canvas",
            ));
        }
        let world = v.viewport.to_world(position);
        let doc = state.editor.document();
        let port_label = |endpoint: &Endpoint, output| {
            let node = &doc.graph().nodes()[&endpoint.node];
            state
                .labels
                .get(&node.type_id)
                .and_then(|labels| {
                    if output {
                        labels.outputs.get(&endpoint.port)
                    } else {
                        labels.inputs.get(&endpoint.port)
                    }
                })
                .map(|label| label.get(v.locale))
                .filter(|name| !name.is_empty())
                .unwrap_or(&endpoint.port)
                .chars()
                .take(80)
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect()
        };
        let group_input = state.scene.group_input(Some(world), &v.collapsed_groups);
        let geometry = PointerGeometry {
            index: state.scene.spatial_index(),
            group: None,
            hidden: &group_input.hidden,
            ports: &group_input.ports,
        };
        let hit = if let Some(port) = proxy_hit(doc, geometry.ports, v.viewport, world) {
            Hit::Port(port)
        } else if let Some(id) = group_input.group {
            let group = &doc.graph().groups()[&id];
            return Ok(CanvasContext {
                revision: state.editor.revision(),
                locale: v.locale,
                world,
                viewport: v.viewport,
                selection_count: v.selection.len(),
                target: ContextTarget::Group(GroupSummary {
                    id,
                    label: group.label.clone(),
                    members: group.nodes.len(),
                    collapsed: v.collapsed_groups.contains(&id),
                }),
            });
        } else {
            hit_test(doc, &geometry, v.viewport, world)
        };
        let target = match hit {
            Hit::Node(id) => ContextTarget::Node {
                id,
                rect: doc.placement().get(&id).copied().unwrap_or_default(),
                selected: v.selection.contains(&id),
            },
            Hit::Port(port) => {
                let edges = state
                    .scene
                    .incident_edges(port.endpoint.node)
                    .filter(|id| {
                        let edge = &doc.graph().edges()[id];
                        let owner = group_input.hidden_groups.get(&edge.from.node);
                        if owner.is_some() && owner == group_input.hidden_groups.get(&edge.to.node)
                        {
                            return false;
                        }
                        if port.output {
                            edge.from == port.endpoint
                        } else {
                            edge.to == port.endpoint
                        }
                    })
                    .take(MAX_CONTEXT_PORT_EDGES + 1)
                    .collect();
                ContextTarget::Port {
                    label: port_label(&port.endpoint, port.output),
                    endpoint: port.endpoint,
                    output: port.output,
                    edges,
                }
            }
            Hit::Empty => state
                .scene
                .edge_at(world, v.viewport.zoom, &v.collapsed_groups)
                .map_or(ContextTarget::Stage, |id| {
                    let edge = &doc.graph().edges()[&id];
                    ContextTarget::Edge {
                        id,
                        from: edge.from.clone(),
                        to: edge.to.clone(),
                        labels: [port_label(&edge.from, true), port_label(&edge.to, false)],
                    }
                }),
        };
        Ok(CanvasContext {
            revision: state.editor.revision(),
            locale: v.locale,
            world,
            viewport: v.viewport,
            selection_count: v.selection.len(),
            target,
        })
    }
}
