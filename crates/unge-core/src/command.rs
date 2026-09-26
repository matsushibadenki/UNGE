use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    AddNode {
        node: Node,
        rect: Rect,
    },
    RemoveNode {
        id: Id,
    },
    MoveNode {
        id: Id,
        rect: Rect,
    },
    Connect {
        edge: Edge,
    },
    Disconnect {
        id: Id,
    },
    SetProperty {
        id: Id,
        key: String,
        value: Option<serde_json::Value>,
    },
    SetGroup {
        id: Id,
        group: Option<Group>,
    },
    Batch {
        commands: Vec<Command>,
    },
}
impl Command {
    /// Apply without whole-graph cloning; return the inverse for rollback/history.
    fn apply(self, doc: &mut Document) -> Result<Command> {
        Ok(match self {
            Self::AddNode { node, rect } => {
                let id = node.id;
                if doc.graph.nodes.contains_key(&id) {
                    return Err(Error::Duplicate(id.to_string()));
                }
                if !rect.valid() {
                    return Err(Error::Invalid("invalid rectangle".into()));
                }
                doc.graph.nodes.insert(id, node);
                doc.placement.insert(id, rect);
                Self::RemoveNode { id }
            }
            Self::RemoveNode { id } => {
                let node = doc
                    .graph
                    .nodes
                    .remove(&id)
                    .ok_or_else(|| Error::Missing(id.to_string()))?;
                let rect = doc.placement.remove(&id).unwrap_or_default();
                let mut restore = vec![Self::AddNode { node, rect }];
                let edges: Vec<_> = doc
                    .graph
                    .edges
                    .values()
                    .filter(|e| e.from.node == id || e.to.node == id)
                    .map(|e| e.id)
                    .collect();
                for edge_id in edges {
                    restore.push(Self::Connect {
                        edge: doc.graph.edges.remove(&edge_id).unwrap(),
                    });
                }
                for group in doc.graph.groups.values_mut() {
                    if group.nodes.contains(&id) {
                        restore.push(Self::SetGroup {
                            id: group.id,
                            group: Some(group.clone()),
                        });
                        group.nodes.remove(&id);
                    }
                }
                Self::Batch { commands: restore }
            }
            Self::MoveNode { id, rect } => {
                if !doc.graph.nodes.contains_key(&id) {
                    return Err(Error::Missing(id.to_string()));
                }
                if !rect.valid() {
                    return Err(Error::Invalid("invalid rectangle".into()));
                }
                let old = doc.placement.insert(id, rect).unwrap_or_default();
                Self::MoveNode { id, rect: old }
            }
            Self::Connect { edge } => {
                let id = edge.id;
                if doc.graph.edges.contains_key(&id) {
                    return Err(Error::Duplicate(id.to_string()));
                }
                doc.graph.edges.insert(id, edge);
                Self::Disconnect { id }
            }
            Self::Disconnect { id } => Self::Connect {
                edge: doc
                    .graph
                    .edges
                    .remove(&id)
                    .ok_or_else(|| Error::Missing(id.to_string()))?,
            },
            Self::SetProperty { id, key, value } => {
                let node = doc
                    .graph
                    .nodes
                    .get_mut(&id)
                    .ok_or_else(|| Error::Missing(id.to_string()))?;
                let old = match value {
                    Some(value) => node.properties.insert(key.clone(), value),
                    None => node.properties.remove(&key),
                };
                Self::SetProperty {
                    id,
                    key,
                    value: old,
                }
            }
            Self::SetGroup { id, group } => {
                if group.as_ref().is_some_and(|g| g.id != id) {
                    return Err(Error::Invalid("group id mismatch".into()));
                }
                let old = match group {
                    Some(group) => doc.graph.groups.insert(id, group),
                    None => doc.graph.groups.remove(&id),
                };
                Self::SetGroup { id, group: old }
            }
            Self::Batch { commands } => {
                let mut inverses = Vec::new();
                for command in commands {
                    match command.apply(doc) {
                        Ok(inverse) => inverses.push(inverse),
                        Err(error) => {
                            for inverse in inverses.into_iter().rev() {
                                inverse.apply(doc).expect("internal rollback invariant");
                            }
                            return Err(error);
                        }
                    }
                }
                inverses.reverse();
                Self::Batch { commands: inverses }
            }
        })
    }
}

/// Single source of truth. Share this from Rust app state, never from a WebView.
#[derive(Debug)]
pub struct Editor {
    document: Document,
    undo: VecDeque<Command>,
    redo: Vec<Command>,
    history_limit: usize,
    revision: u64,
}
impl Editor {
    pub fn new(document: Document, history_limit: usize) -> Result<Self> {
        document.validate()?;
        // Normalize missing optional placements before any undoable edits.
        let mut document = document;
        for id in document.graph.nodes.keys() {
            document.placement.entry(*id).or_default();
        }
        Ok(Self {
            document,
            undo: VecDeque::new(),
            redo: Vec::new(),
            history_limit,
            revision: 0,
        })
    }
    pub fn document(&self) -> &Document {
        &self.document
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    fn apply_checked(&mut self, command: Command) -> Result<Command> {
        let inverse = command.apply(&mut self.document)?;
        if let Err(error) = self.document.validate() {
            inverse
                .apply(&mut self.document)
                .expect("internal rollback invariant");
            return Err(error);
        }
        self.revision += 1;
        Ok(inverse)
    }
    fn remember(&mut self, inverse: Command) {
        if self.history_limit > 0 {
            self.undo.push_back(inverse);
            while self.undo.len() > self.history_limit {
                self.undo.pop_front();
            }
        }
    }
    pub fn execute(&mut self, command: Command) -> Result<u64> {
        let inverse = self.apply_checked(command)?;
        self.remember(inverse);
        self.redo.clear();
        Ok(self.revision)
    }
    pub fn undo(&mut self) -> Result<bool> {
        let Some(command) = self.undo.pop_back() else {
            return Ok(false);
        };
        let inverse = self.apply_checked(command)?;
        self.redo.push(inverse);
        Ok(true)
    }
    pub fn redo(&mut self) -> Result<bool> {
        let Some(command) = self.redo.pop() else {
            return Ok(false);
        };
        let inverse = self.apply_checked(command)?;
        self.remember(inverse);
        Ok(true)
    }
}

/// Copy only selected nodes and internal connections. Paste allocates fresh IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fragment {
    nodes: Vec<(Node, Rect)>,
    edges: Vec<Edge>,
}
impl Fragment {
    pub fn copy(doc: &Document, ids: &std::collections::BTreeSet<Id>) -> Self {
        Self {
            nodes: doc
                .graph
                .nodes
                .values()
                .filter(|n| ids.contains(&n.id))
                .map(|n| {
                    (
                        n.clone(),
                        doc.placement.get(&n.id).copied().unwrap_or_default(),
                    )
                })
                .collect(),
            edges: doc
                .graph
                .edges
                .values()
                .filter(|e| ids.contains(&e.from.node) && ids.contains(&e.to.node))
                .cloned()
                .collect(),
        }
    }
    pub fn paste(&self, offset: [f32; 2]) -> Command {
        let mut ids = std::collections::BTreeMap::new();
        let mut commands = Vec::new();
        for (node, rect) in &self.nodes {
            let mut node = node.clone();
            let new = Id::new_v4();
            ids.insert(node.id, new);
            node.id = new;
            commands.push(Command::AddNode {
                node,
                rect: Rect {
                    x: rect.x + offset[0],
                    y: rect.y + offset[1],
                    ..*rect
                },
            });
        }
        for edge in &self.edges {
            // Ignore malformed clipboard edges instead of indexing untrusted IDs.
            if let (Some(from), Some(to)) = (ids.get(&edge.from.node), ids.get(&edge.to.node)) {
                commands.push(Command::Connect {
                    edge: Edge {
                        id: Id::new_v4(),
                        from: Endpoint {
                            node: *from,
                            port: edge.from.port.clone(),
                        },
                        to: Endpoint {
                            node: *to,
                            port: edge.to.port.clone(),
                        },
                    },
                });
            }
        }
        Command::Batch { commands }
    }
}

/// Deterministic left-to-right layout. A whole layout is one undo step.
pub fn auto_layout(doc: &Document, gap: [f32; 2]) -> Result<Command> {
    if gap.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return Err(Error::Invalid("invalid layout gap".into()));
    }
    let mut commands = Vec::new();
    let mut x = 0.0;
    for layer in doc.graph.layers()? {
        let mut y = 0.0;
        let mut width: f32 = 0.0;
        for id in layer {
            let rect = doc.placement.get(&id).copied().unwrap_or_default();
            commands.push(Command::MoveNode {
                id,
                rect: Rect { x, y, ..rect },
            });
            y += rect.height + gap[1];
            width = width.max(rect.width);
        }
        x += width + gap[0];
    }
    Ok(Command::Batch { commands })
}
