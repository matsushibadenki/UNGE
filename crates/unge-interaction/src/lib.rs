//! Host-independent pointer gestures. Document edits are returned as commands;
//! the host applies them with a revision check under its shared document lock.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use unge_core::*;

pub const MAX_SELECTION: usize = 10_000;
pub const DRAG_THRESHOLD: f32 = 3.0;
pub const PORT_LOD_ZOOM: f32 = 0.3;
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Primary,
    Pan,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PointerEvent {
    Down {
        pointer: u32,
        position: [f32; 2],
        button: PointerButton,
        #[serde(default)]
        additive: bool,
    },
    Move {
        pointer: u32,
        position: [f32; 2],
    },
    Up {
        pointer: u32,
        position: [f32; 2],
    },
    Cancel,
}
#[derive(Debug, thiserror::Error)]
pub enum InteractionError {
    #[error("document changed during pointer gesture")]
    Conflict,
    #[error("another pointer owns the active gesture")]
    PointerBusy,
    #[error("invalid pointer coordinate, viewport, selection or geometry")]
    Invalid,
    #[error("connection is incompatible, occupied, duplicate or cyclic")]
    Connection,
}
pub type InteractionResult<T> = std::result::Result<T, InteractionError>;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortHit {
    pub endpoint: Endpoint,
    pub output: bool,
}
#[derive(Debug, Clone)]
pub struct CablePreview {
    pub from: [f32; 2],
    pub to: [f32; 2],
    pub valid: bool,
}
/// Ephemeral geometry only. Never serialize this as a Document or history entry.
#[derive(Debug, Clone, Default)]
pub struct Preview {
    pub placement: BTreeMap<Id, Rect>,
    pub marquee: Option<Rect>,
    pub cable: Option<CablePreview>,
}
#[derive(Debug)]
enum Action {
    Drag {
        original: BTreeMap<Id, Rect>,
    },
    Box,
    Connect {
        source: PortHit,
        target: Option<PortHit>,
        valid: bool,
    },
    Pan,
    Click,
}
#[derive(Debug)]
struct Gesture {
    pointer: u32,
    revision: u64,
    start: [f32; 2],
    viewport: Viewport,
    selection: BTreeSet<Id>,
    additive: bool,
    action: Action,
}
#[derive(Debug, Default)]
pub struct Interaction {
    gesture: Option<Gesture>,
    preview: Preview,
}
impl Interaction {
    pub fn preview(&self) -> &Preview {
        &self.preview
    }
    pub fn is_active(&self) -> bool {
        self.gesture.is_some()
    }
    /// Clear the overlay immediately after another editor changes the document.
    /// Retain the starting revision so the pending release reports a conflict.
    pub fn invalidate_preview(&mut self) {
        self.preview = Preview::default();
    }
    pub fn cancel(&mut self, viewport: &mut Viewport, selection: &mut BTreeSet<Id>) {
        if let Some(g) = self.gesture.take() {
            *viewport = g.viewport;
            *selection = g.selection;
        }
        self.preview = Preview::default();
    }
    /// Positions are logical surface pixels, not physical pixels or world units.
    /// The host must serialize this call and command execution with other edits.
    pub fn handle(
        &mut self,
        document: &Document,
        revision: u64,
        index: &SpatialIndex,
        viewport: &mut Viewport,
        selection: &mut BTreeSet<Id>,
        event: PointerEvent,
    ) -> InteractionResult<Option<Command>> {
        self.handle_with_groups(
            document,
            revision,
            PointerGeometry {
                index,
                group: None,
                hidden: &BTreeSet::new(),
                ports: &[],
            },
            viewport,
            selection,
            event,
        )
    }
    pub fn handle_with_groups(
        &mut self,
        document: &Document,
        revision: u64,
        geometry: PointerGeometry<'_>,
        viewport: &mut Viewport,
        selection: &mut BTreeSet<Id>,
        event: PointerEvent,
    ) -> InteractionResult<Option<Command>> {
        if matches!(event, PointerEvent::Cancel) {
            self.cancel(viewport, selection);
            return Ok(None);
        }
        let (pointer, position) = match event {
            PointerEvent::Down {
                pointer, position, ..
            }
            | PointerEvent::Move { pointer, position }
            | PointerEvent::Up { pointer, position } => (pointer, position),
            PointerEvent::Cancel => unreachable!(),
        };
        if let Some(g) = &self.gesture {
            if g.pointer != pointer {
                return Err(InteractionError::PointerBusy);
            }
            if g.revision != revision {
                self.cancel(viewport, selection);
                selection.retain(|id| document.graph().nodes().contains_key(id));
                return Err(InteractionError::Conflict);
            }
        }
        if viewport.validate().is_err()
            || position.iter().any(|v| !v.is_finite() || v.abs() > 1.0e9)
            || viewport
                .to_world(position)
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 1.0e9)
        {
            self.cancel(viewport, selection);
            return Err(InteractionError::Invalid);
        }
        if let PointerEvent::Down {
            button, additive, ..
        } = event
        {
            if self.gesture.is_some() {
                return Err(InteractionError::PointerBusy);
            }
            if selection.len() > MAX_SELECTION
                || selection
                    .iter()
                    .any(|id| !document.graph().nodes().contains_key(id))
            {
                return Err(InteractionError::Invalid);
            }
            let original_selection = selection.clone();
            let point = viewport.to_world(position);
            let action = if button == PointerButton::Pan {
                Action::Pan
            } else if let Some(source) = proxy_hit(document, geometry.ports, *viewport, point) {
                Action::Connect {
                    source,
                    target: None,
                    valid: false,
                }
            } else if let Some(group) = geometry.group {
                if group.len() > MAX_SELECTION
                    || group
                        .iter()
                        .any(|id| !document.graph().nodes().contains_key(id))
                {
                    return Err(InteractionError::Invalid);
                }
                *selection = group.clone();
                Action::Drag {
                    original: group
                        .iter()
                        .map(|id| {
                            (
                                *id,
                                document.placement().get(id).copied().unwrap_or_default(),
                            )
                        })
                        .collect(),
                }
            } else {
                match hit_test(document, &geometry, *viewport, point) {
                    Hit::Port(source) => Action::Connect {
                        source,
                        target: None,
                        valid: false,
                    },
                    Hit::Node(id) => {
                        if additive && selection.contains(&id) {
                            selection.remove(&id);
                            Action::Click
                        } else {
                            if !additive && !selection.contains(&id) {
                                selection.clear();
                            }
                            if selection.len() == MAX_SELECTION && !selection.contains(&id) {
                                return Err(InteractionError::Invalid);
                            }
                            selection.insert(id);
                            Action::Drag {
                                original: selection
                                    .iter()
                                    .map(|id| {
                                        (
                                            *id,
                                            document
                                                .placement()
                                                .get(id)
                                                .copied()
                                                .unwrap_or_default(),
                                        )
                                    })
                                    .collect(),
                            }
                        }
                    }
                    Hit::Empty => {
                        if !additive {
                            selection.clear();
                        }
                        Action::Box
                    }
                }
            };
            self.gesture = Some(Gesture {
                pointer,
                revision,
                start: position,
                viewport: *viewport,
                selection: original_selection,
                additive,
                action,
            });
            return Ok(None);
        }
        if self.gesture.is_none() {
            return Ok(None);
        }
        if self
            .update(document, geometry, viewport, selection, position)
            .is_err()
        {
            self.cancel(viewport, selection);
            return Err(InteractionError::Invalid);
        }
        if matches!(event, PointerEvent::Move { .. }) {
            return Ok(None);
        }
        let g = self.gesture.take().unwrap();
        let preview = std::mem::take(&mut self.preview);
        match g.action {
            Action::Drag { original } => {
                let commands: Vec<_> = preview
                    .placement
                    .into_iter()
                    .filter(|(id, rect)| original[id] != *rect)
                    .map(|(id, rect)| Command::MoveNode { id, rect })
                    .collect();
                Ok((!commands.is_empty()).then_some(Command::Batch { commands }))
            }
            Action::Connect {
                source,
                target: Some(target),
                valid: true,
            } => {
                let (from, to) = if source.output {
                    (source.endpoint, target.endpoint)
                } else {
                    (target.endpoint, source.endpoint)
                };
                Ok(Some(Command::Connect {
                    edge: Edge {
                        id: Id::new_v4(),
                        from,
                        to,
                    },
                }))
            }
            Action::Connect {
                target: Some(_),
                valid: false,
                ..
            } => Err(InteractionError::Connection),
            _ => Ok(None),
        }
    }
    fn update(
        &mut self,
        document: &Document,
        geometry: PointerGeometry<'_>,
        viewport: &mut Viewport,
        selection: &mut BTreeSet<Id>,
        position: [f32; 2],
    ) -> InteractionResult<()> {
        let g = self.gesture.as_mut().unwrap();
        let delta = [position[0] - g.start[0], position[1] - g.start[1]];
        let moved = delta[0].hypot(delta[1]) >= DRAG_THRESHOLD;
        let point = g.viewport.to_world(position);
        self.preview = Preview::default();
        match &mut g.action {
            Action::Drag { original } if moved => {
                for (id, rect) in original {
                    let moved = Rect {
                        x: rect.x + delta[0] / g.viewport.zoom,
                        y: rect.y + delta[1] / g.viewport.zoom,
                        ..*rect
                    };
                    if !moved.valid() {
                        return Err(InteractionError::Invalid);
                    }
                    self.preview.placement.insert(*id, moved);
                }
            }
            Action::Box => {
                if moved {
                    let start = g.viewport.to_world(g.start);
                    let rect = Rect {
                        x: start[0].min(point[0]),
                        y: start[1].min(point[1]),
                        width: (start[0] - point[0]).abs().max(0.001),
                        height: (start[1] - point[1]).abs().max(0.001),
                    };
                    if !rect.valid() {
                        return Err(InteractionError::Invalid);
                    }
                    *selection = if g.additive {
                        g.selection.clone()
                    } else {
                        BTreeSet::new()
                    };
                    selection.extend(
                        geometry
                            .index
                            .query(rect)
                            .into_iter()
                            .filter(|id| !geometry.hidden.contains(id)),
                    );
                    if selection.len() > MAX_SELECTION {
                        return Err(InteractionError::Invalid);
                    }
                    self.preview.marquee = Some(rect);
                } else {
                    *selection = if g.additive {
                        g.selection.clone()
                    } else {
                        BTreeSet::new()
                    };
                }
            }
            Action::Connect {
                source,
                target,
                valid,
            } => {
                let next = match hit_test(document, &geometry, g.viewport, point) {
                    Hit::Port(port) if port != *source => Some(port),
                    _ => None,
                };
                if next != *target {
                    *valid = next
                        .as_ref()
                        .is_some_and(|next| connectable(document.graph(), source, next));
                    *target = next;
                }
                let start = visible_port_position(document, geometry.ports, source);
                let end = target.as_ref().map_or(point, |p| {
                    visible_port_position(document, geometry.ports, p)
                });
                let (from, to) = if source.output {
                    (start, end)
                } else {
                    (end, start)
                };
                self.preview.cable = Some(CablePreview {
                    from,
                    to,
                    valid: *valid,
                });
            }
            Action::Pan => {
                let mut next = g.viewport;
                next.pan(delta);
                next.validate().map_err(|_| InteractionError::Invalid)?;
                *viewport = next;
            }
            _ => {}
        }
        Ok(())
    }
}
/// Host presentation filters; the Document remains the source of member placements.
pub struct PointerGeometry<'a> {
    pub index: &'a SpatialIndex,
    pub group: Option<&'a BTreeSet<Id>>,
    pub hidden: &'a BTreeSet<Id>,
    pub ports: &'a [ProxyPort],
}
/// A visible alias for an existing endpoint. Connection commands use the endpoint, not the alias.
#[derive(Debug, Clone)]
pub struct ProxyPort {
    pub hit: PortHit,
    pub anchor: [f32; 2],
}
pub fn proxy_hit(
    doc: &Document,
    ports: &[ProxyPort],
    viewport: Viewport,
    point: [f32; 2],
) -> Option<PortHit> {
    if viewport.zoom < PORT_LOD_ZOOM {
        return None;
    }
    let radius = (8. / viewport.zoom).min(10.);
    ports
        .iter()
        .rev()
        .find(|port| {
            (point[0] - port.anchor[0]).hypot(point[1] - port.anchor[1]) <= radius
                && doc
                    .graph()
                    .nodes()
                    .get(&port.hit.endpoint.node)
                    .is_some_and(|node| {
                        let actual = if port.hit.output {
                            &node.outputs
                        } else {
                            &node.inputs
                        };
                        actual.iter().any(|p| p.name == port.hit.endpoint.port)
                    })
        })
        .map(|port| port.hit.clone())
}
fn visible_port_position(doc: &Document, ports: &[ProxyPort], hit: &PortHit) -> [f32; 2] {
    ports
        .iter()
        .find(|p| &p.hit == hit)
        .map_or_else(|| port_position(doc, hit), |p| p.anchor)
}
#[derive(Debug)]
pub enum Hit {
    Node(Id),
    Port(PortHit),
    Empty,
}
pub fn hit_test(
    doc: &Document,
    geometry: &PointerGeometry<'_>,
    viewport: Viewport,
    point: [f32; 2],
) -> Hit {
    if let Some(port) = proxy_hit(doc, geometry.ports, viewport, point) {
        return Hit::Port(port);
    }
    let radius = 8.0 / viewport.zoom;
    let area = Rect {
        x: point[0] - radius,
        y: point[1] - radius,
        width: radius * 2.,
        height: radius * 2.,
    };
    for id in geometry.index.query(area).into_iter().rev() {
        if geometry.hidden.contains(&id) {
            continue;
        }
        let rect = doc.placement().get(&id).copied().unwrap_or_default();
        let node = &doc.graph().nodes()[&id];
        if viewport.zoom >= PORT_LOD_ZOOM {
            for (ports, output) in [(&node.inputs, false), (&node.outputs, true)] {
                for (i, port) in ports.iter().enumerate() {
                    let anchor = port_anchor(rect, i, ports.len(), output);
                    if (point[0] - anchor[0]).hypot(point[1] - anchor[1]) <= radius {
                        return Hit::Port(PortHit {
                            endpoint: Endpoint {
                                node: id,
                                port: port.name.clone(),
                            },
                            output,
                        });
                    }
                }
            }
        }
        if point[0] >= rect.x
            && point[0] <= rect.x + rect.width
            && point[1] >= rect.y
            && point[1] <= rect.y + rect.height
        {
            return Hit::Node(id);
        }
    }
    Hit::Empty
}
fn port_position(doc: &Document, hit: &PortHit) -> [f32; 2] {
    let node = &doc.graph().nodes()[&hit.endpoint.node];
    let ports = if hit.output {
        &node.outputs
    } else {
        &node.inputs
    };
    let index = ports
        .iter()
        .position(|p| p.name == hit.endpoint.port)
        .expect("gesture port was validated at its starting revision");
    port_anchor(
        doc.placement().get(&node.id).copied().unwrap_or_default(),
        index,
        ports.len(),
        hit.output,
    )
}
fn connectable(graph: &Graph, a: &PortHit, b: &PortHit) -> bool {
    if a.output == b.output || a.endpoint.node == b.endpoint.node {
        return false;
    }
    let (from, to) = if a.output {
        (&a.endpoint, &b.endpoint)
    } else {
        (&b.endpoint, &a.endpoint)
    };
    let output = graph.nodes()[&from.node]
        .outputs
        .iter()
        .find(|p| p.name == from.port)
        .unwrap();
    let input = graph.nodes()[&to.node]
        .inputs
        .iter()
        .find(|p| p.name == to.port)
        .unwrap();
    input.data_type.accepts(&output.data_type)
        && !graph
            .edges()
            .values()
            .any(|e| e.to == *to && (input.cardinality == Cardinality::Single || e.from == *from))
        && !GraphIndex::new(graph)
            .downstream(graph, to.node)
            .contains(&from.node)
}
