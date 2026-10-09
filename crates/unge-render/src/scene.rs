use crate::{LabelCatalog, TextLabel, Theme, labels::label_text};
use std::collections::{BTreeMap, BTreeSet};
use unge_core::{Command, Document, Id, Locale, Rect, SpatialIndex, Viewport, port_anchor};
use unge_interaction::{MAX_SELECTION, PORT_LOD_ZOOM, Preview};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Quad {
    pub rect: [f32; 4],
    pub color: [f32; 4],
    pub params: [f32; 4],
}
impl Quad {
    fn rectangle(rect: Rect, color: [f32; 4], radius: f32) -> Self {
        Self {
            rect: [
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
                rect.width,
                rect.height,
            ],
            color,
            params: [0.0, radius, 0.0, 0.0],
        }
    }
    fn line(a: [f32; 2], b: [f32; 2], color: [f32; 4]) -> Self {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        Self {
            rect: [
                (a[0] + b[0]) / 2.0,
                (a[1] + b[1]) / 2.0,
                dx.hypot(dy).max(0.001) + 3.0,
                3.0,
            ],
            color,
            params: [dy.atan2(dx), 1.5, 0.0, 0.0],
        }
    }
}
struct RenderNode {
    rect: Rect,
    inputs: usize,
    outputs: usize,
    type_id: String,
    input_names: Vec<String>,
    output_names: Vec<String>,
}
struct RenderEdge {
    points: [[f32; 2]; 4],
    from: (Id, usize),
    to: (Id, usize),
}
impl RenderNode {
    fn from_document(doc: &Document, id: Id) -> Option<Self> {
        let node = doc.graph().nodes().get(&id)?;
        Some(Self {
            rect: doc.placement().get(&id).copied().unwrap_or_default(),
            inputs: node.inputs.len(),
            outputs: node.outputs.len(),
            type_id: node.type_id.clone(),
            input_names: node.inputs.iter().map(|p| p.name.clone()).collect(),
            output_names: node.outputs.iter().map(|p| p.name.clone()).collect(),
        })
    }
}
impl RenderEdge {
    fn from_document(doc: &Document, id: Id) -> Option<Self> {
        let edge = doc.graph().edges().get(&id)?;
        let from = doc.graph().nodes().get(&edge.from.node)?;
        let to = doc.graph().nodes().get(&edge.to.node)?;
        let i = from.outputs.iter().position(|p| p.name == edge.from.port)?;
        let j = to.inputs.iter().position(|p| p.name == edge.to.port)?;
        Some(Self {
            points: curve(
                port_anchor(
                    doc.placement().get(&from.id).copied().unwrap_or_default(),
                    i,
                    from.outputs.len(),
                    true,
                ),
                port_anchor(
                    doc.placement().get(&to.id).copied().unwrap_or_default(),
                    j,
                    to.inputs.len(),
                    false,
                ),
            ),
            from: (from.id, i),
            to: (to.id, j),
        })
    }
}
/// Retained geometry. Small committed edits update bounded BVH overlays.
pub struct SceneIndex {
    nodes: BTreeMap<Id, RenderNode>,
    edges: BTreeMap<Id, RenderEdge>,
    node_index: SpatialIndex,
    edge_index: SpatialIndex,
    incident: BTreeMap<Id, BTreeSet<Id>>,
    groups: crate::groups::GroupIndex,
}
/// Capture before consuming a Command; apply only after that edit commits.
#[derive(Debug, Default)]
pub struct SceneChanges {
    nodes: BTreeSet<Id>,
    edges: BTreeSet<Id>,
    rebuild: bool,
    groups_changed: bool,
}
impl SceneChanges {
    pub fn from_command(command: &Command) -> Self {
        fn collect(command: &Command, changes: &mut SceneChanges) {
            if changes.rebuild {
                return;
            }
            match command {
                Command::MoveNode { id, .. } | Command::RemoveNode { id } => {
                    changes.nodes.insert(*id);
                }
                Command::AddNode { node, .. } => {
                    changes.nodes.insert(node.id);
                }
                Command::Connect { edge } => {
                    changes.edges.insert(edge.id);
                }
                Command::Disconnect { id } => {
                    changes.edges.insert(*id);
                }
                Command::SetProperty { .. } => {}
                Command::SetGroup { .. } => changes.groups_changed = true,
                Command::Batch { commands } => {
                    for command in commands {
                        collect(command, changes);
                        if changes.rebuild {
                            break;
                        }
                    }
                }
            }
            if changes.nodes.len() > 128 || changes.edges.len() > 128 {
                changes.rebuild = true;
                changes.nodes.clear();
                changes.edges.clear();
            }
        }
        let mut changes = Self::default();
        collect(command, &mut changes);
        changes
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneUpdate {
    Unchanged,
    Incremental,
    Rebuilt,
}

#[derive(Debug)]
pub struct Scene {
    /// Linear RGBA, matching the target's GPU colour space.
    pub background: [f32; 4],
    pub grid_color: [f32; 4],
    pub quads: Vec<Quad>,
    pub labels: Vec<TextLabel>,
    pub visible_nodes: usize,
    pub visible_edges: usize,
    pub visible_groups: usize,
}
impl Default for Scene {
    fn default() -> Self {
        let palette = Theme::Dark.palette();
        Self {
            background: palette.background.linear(),
            grid_color: palette.grid.linear(),
            quads: Vec::new(),
            labels: Vec::new(),
            visible_nodes: 0,
            visible_edges: 0,
            visible_groups: 0,
        }
    }
}
fn curve(a: [f32; 2], b: [f32; 2]) -> [[f32; 2]; 4] {
    let dx = ((b[0] - a[0]).abs() * 0.5).max(50.0);
    [a, [a[0] + dx, a[1]], [b[0] - dx, b[1]], b]
}
fn curve_bounds(points: [[f32; 2]; 4]) -> Rect {
    let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - 2.0;
    let max_x = points
        .iter()
        .map(|p| p[0])
        .fold(f32::NEG_INFINITY, f32::max)
        + 2.0;
    let min_y = points[0][1].min(points[3][1]) - 2.0;
    let max_y = points[0][1].max(points[3][1]) + 2.0;
    Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}
fn push_curve(scene: &mut Scene, points: [[f32; 2]; 4], zoom: f32, color: [f32; 4]) {
    crate::curves::for_each_segment(points, zoom, |a, b| {
        scene.quads.push(Quad::line(a, b, color));
    });
}
impl SceneIndex {
    pub fn new(doc: &Document) -> Self {
        let nodes = doc
            .graph()
            .nodes()
            .keys()
            .filter_map(|id| RenderNode::from_document(doc, *id).map(|n| (*id, n)))
            .collect();
        let mut edges = BTreeMap::new();
        let mut bounds = Vec::new();
        let mut incident: BTreeMap<Id, BTreeSet<Id>> = BTreeMap::new();
        for id in doc.graph().edges().keys() {
            let Some(edge) = RenderEdge::from_document(doc, *id) else {
                continue;
            };
            bounds.push((*id, curve_bounds(edge.points)));
            for node in [edge.from.0, edge.to.0] {
                incident.entry(node).or_default().insert(*id);
            }
            edges.insert(*id, edge);
        }

        Self {
            nodes,
            edges,
            incident,
            node_index: SpatialIndex::new(doc),
            edge_index: SpatialIndex::from_rects(bounds),
            groups: crate::groups::GroupIndex::new(doc),
        }
    }
    /// The index must correspond to the state immediately before the captured
    /// command. Document validation and revision checks remain the host's job.
    pub fn update(&mut self, doc: &Document, changes: &SceneChanges) -> SceneUpdate {
        let mut edge_ids = changes.edges.clone();
        if !changes.rebuild {
            for id in &changes.nodes {
                for edge in self.incident.get(id).into_iter().flatten() {
                    edge_ids.insert(*edge);
                    if edge_ids.len() > 128 {
                        break;
                    }
                }
                if edge_ids.len() > 128 {
                    break;
                }
            }
        }
        if changes.rebuild || edge_ids.len() > 128 {
            *self = Self::new(doc);
            return SceneUpdate::Rebuilt;
        }
        if changes.nodes.is_empty() && edge_ids.is_empty() {
            if changes.groups_changed {
                self.groups = crate::groups::GroupIndex::new(doc);
                return SceneUpdate::Incremental;
            }
            return SceneUpdate::Unchanged;
        }
        // Read final committed geometry. This handles removal/recreation of the
        // same ID in a Batch, including changed ports and implicit edge deletion.
        let nodes: BTreeMap<_, _> = changes
            .nodes
            .iter()
            .map(|id| (*id, RenderNode::from_document(doc, *id)))
            .collect();
        let edges: BTreeMap<_, _> = edge_ids
            .iter()
            .map(|id| (*id, RenderEdge::from_document(doc, *id)))
            .collect();
        let rects = nodes
            .iter()
            .map(|(id, node)| (*id, node.as_ref().map(|n| n.rect)))
            .collect();
        let bounds = edges
            .iter()
            .map(|(id, edge)| (*id, edge.as_ref().map(|e| curve_bounds(e.points))))
            .collect();
        if !self.node_index.update_entries(&rects) || !self.edge_index.update_entries(&bounds) {
            *self = Self::new(doc);
            return SceneUpdate::Rebuilt;
        }
        // Remove old incidence before installing final endpoints: rewiring an ID
        // must not leave stale references that break a later move or deletion.
        for id in &edge_ids {
            if let Some(old) = self.edges.remove(id) {
                for node in [old.from.0, old.to.0] {
                    if let Some(set) = self.incident.get_mut(&node) {
                        set.remove(id);
                        if set.is_empty() {
                            self.incident.remove(&node);
                        }
                    }
                }
            }
        }
        for (id, node) in nodes {
            if let Some(node) = node {
                self.nodes.insert(id, node);
            } else {
                self.nodes.remove(&id);
                self.incident.remove(&id);
            }
        }
        for (id, edge) in edges {
            if let Some(edge) = edge {
                for node in [edge.from.0, edge.to.0] {
                    self.incident.entry(node).or_default().insert(id);
                }
                self.edges.insert(id, edge);
            }
        }
        if !changes.nodes.is_empty() || changes.groups_changed {
            self.groups = crate::groups::GroupIndex::new(doc);
        }
        SceneUpdate::Incremental
    }
    pub fn scene(&self, viewport: Viewport, selection: &BTreeSet<Id>) -> unge_core::Result<Scene> {
        self.scene_with_preview(viewport, selection, &Preview::default())
    }
    pub fn spatial_index(&self) -> &SpatialIndex {
        &self.node_index
    }
    /// Reuse committed BVHs; only moved nodes and incident edges bypass old bounds.
    pub fn scene_with_preview(
        &self,
        viewport: Viewport,
        selection: &BTreeSet<Id>,
        preview: &Preview,
    ) -> unge_core::Result<Scene> {
        self.scene_with_labels(
            viewport,
            selection,
            preview,
            &LabelCatalog::new(),
            Locale::En,
        )
    }
    pub fn scene_with_labels(
        &self,
        viewport: Viewport,
        selection: &BTreeSet<Id>,
        preview: &Preview,
        catalog: &LabelCatalog,
        locale: Locale,
    ) -> unge_core::Result<Scene> {
        self.scene_with_theme(viewport, selection, preview, catalog, locale, Theme::Dark)
    }
    /// Reuse geometry indexes while resolving view-local presentation colours.
    pub fn scene_with_theme(
        &self,
        viewport: Viewport,
        selection: &BTreeSet<Id>,
        preview: &Preview,
        catalog: &LabelCatalog,
        locale: Locale,
        theme: Theme,
    ) -> unge_core::Result<Scene> {
        viewport.validate()?;
        if preview.placement.len() > MAX_SELECTION
            || preview
                .placement
                .iter()
                .any(|(id, rect)| !self.nodes.contains_key(id) || !rect.valid())
            || preview.marquee.is_some_and(|rect| !rect.valid())
            || preview.cable.as_ref().is_some_and(|c| {
                c.from
                    .iter()
                    .chain(c.to.iter())
                    .any(|v| !v.is_finite() || v.abs() > 1.0e9)
            })
        {
            return Err(unge_core::Error::Invalid(
                "invalid interaction preview".into(),
            ));
        }
        let area = viewport.world_rect();
        let palette = theme.palette();
        let mut scene = Scene {
            background: palette.background.linear(),
            grid_color: palette.grid.linear(),
            ..Scene::default()
        };
        let mut grid = Quad::rectangle(area, scene.background, 0.0);
        grid.params[2] = 1.0;
        scene.quads.push(grid);
        for (rect, label) in self.groups.visible(area, preview, |id| {
            self.nodes.get(&id).map(|node| node.rect)
        }) {
            scene
                .quads
                .push(Quad::rectangle(rect, palette.border.linear(), 10.0));
            let background = [0, 1, 2, 3]
                .map(|i| (palette.background.linear()[i] + palette.node.linear()[i]) * 0.5);
            scene.quads.push(Quad::rectangle(
                Rect {
                    x: rect.x + 1.5,
                    y: rect.y + 1.5,
                    width: rect.width - 3.0,
                    height: rect.height - 3.0,
                },
                background,
                8.5,
            ));
            if viewport.zoom >= 0.6 && !label.is_empty() {
                scene.labels.push(TextLabel {
                    text: label.to_owned(),
                    rect: Rect {
                        x: rect.x + 12.0,
                        y: rect.y + 6.0,
                        width: rect.width - 24.0,
                        height: 22.0,
                    },
                    font_size: if rect.height >= 120.0 { 15.0 } else { 14.0 },
                    right_aligned: false,
                    color: palette.text.linear(),
                    after_quad: scene.quads.len(),
                });
            }
            scene.visible_groups += 1;
        }
        let mut edges: BTreeSet<_> = self.edge_index.query(area).into_iter().collect();
        for id in preview.placement.keys() {
            edges.extend(self.incident.get(id).into_iter().flatten());
        }
        for id in edges {
            let edge = &self.edges[&id];
            let points = if preview.placement.contains_key(&edge.from.0)
                || preview.placement.contains_key(&edge.to.0)
            {
                let from = &self.nodes[&edge.from.0];
                let to = &self.nodes[&edge.to.0];
                curve(
                    port_anchor(
                        preview
                            .placement
                            .get(&edge.from.0)
                            .copied()
                            .unwrap_or(from.rect),
                        edge.from.1,
                        from.outputs,
                        true,
                    ),
                    port_anchor(
                        preview
                            .placement
                            .get(&edge.to.0)
                            .copied()
                            .unwrap_or(to.rect),
                        edge.to.1,
                        to.inputs,
                        false,
                    ),
                )
            } else {
                edge.points
            };
            if !curve_bounds(points).intersects(area) {
                continue;
            }
            let color = catalog
                .get(&self.nodes[&edge.from.0].type_id)
                .filter(|labels| labels.tone != crate::NodeTone::Neutral)
                .map_or(palette.edge, |labels| palette.tone(labels.tone));
            push_curve(&mut scene, points, viewport.zoom, color.linear());
            scene.visible_edges += 1;
        }
        // Include ports/selection borders protruding from the node bounds.
        let padded = Rect {
            x: area.x - 12.0,
            y: area.y - 12.0,
            width: area.width + 24.0,
            height: area.height + 24.0,
        };
        let mut nodes: BTreeSet<_> = self.node_index.query(padded).into_iter().collect();
        nodes.extend(preview.placement.keys());
        for id in nodes {
            let node = &self.nodes[&id];
            let rect = preview.placement.get(&id).copied().unwrap_or(node.rect);
            if !rect.intersects(padded) {
                continue;
            }
            let selected = selection.contains(&id);
            let labels = catalog.get(&node.type_id);
            let tone = palette
                .tone(labels.map_or(crate::NodeTone::Neutral, |l| l.tone))
                .linear();
            // A single soft SDF instance for depth; skip ornament at overview zoom.
            if viewport.zoom >= 0.6 {
                let mut shadow = palette.shadow.linear();
                shadow[3] = 0.24;
                let mut q = Quad::rectangle(
                    Rect {
                        x: rect.x - 6.0,
                        y: rect.y - 2.0,
                        width: rect.width + 12.0,
                        height: rect.height + 12.0,
                    },
                    shadow,
                    16.0,
                );
                q.params[3] = 7.0;
                scene.quads.push(q);
            }
            if selected {
                let mut halo = palette.accent.linear();
                halo[3] = 0.22;
                scene.quads.push(Quad::rectangle(
                    Rect {
                        x: rect.x - 4.0,
                        y: rect.y - 4.0,
                        width: rect.width + 8.0,
                        height: rect.height + 8.0,
                    },
                    halo,
                    14.0,
                ));
            }
            let border = if selected {
                palette.accent.linear()
            } else {
                palette.border.linear()
            };
            scene.quads.push(Quad::rectangle(rect, border, 10.0));
            let inset = if selected { 2.0 } else { 1.0 };
            scene.quads.push(Quad::rectangle(
                Rect {
                    x: rect.x + inset,
                    y: rect.y + inset,
                    width: (rect.width - 2.0 * inset).max(0.1),
                    height: (rect.height - 2.0 * inset).max(0.1),
                },
                palette.node.linear(),
                9.0,
            ));
            if viewport.zoom >= 0.6 && rect.width >= 48.0 && rect.height >= 40.0 {
                // Opaque tint in linear space: inexpensive header separation in both themes.
                // Keep the existing port anchors and dense-node label clearance unchanged.
                let mut header = palette.node.linear();
                for channel in 0..3 {
                    header[channel] = header[channel] * 0.88 + tone[channel] * 0.12;
                }
                let height = if rect.height >= 120.0 { 34.0 } else { 23.0 };
                for (y, height, radius) in
                    [(inset, height - inset, 9.0), (12.0, height - 12.0, 0.0)]
                {
                    scene.quads.push(Quad::rectangle(
                        Rect {
                            x: rect.x + inset,
                            y: rect.y + y,
                            width: rect.width - inset * 2.0,
                            height,
                        },
                        header,
                        radius,
                    ));
                }
                scene.quads.push(Quad::rectangle(
                    Rect {
                        x: rect.x + 12.0,
                        y: rect.y + 1.5,
                        width: rect.width - 24.0,
                        height: 2.0,
                    },
                    tone,
                    1.0,
                ));
                if rect.height >= 120.0
                    && labels.is_some_and(|l| !l.symbol.is_empty())
                    && rect.width >= 96.0
                {
                    let mut badge = tone;
                    badge[3] = 0.14;
                    scene.quads.push(Quad::rectangle(
                        Rect {
                            x: rect.x + 10.0,
                            y: rect.y + 8.0,
                            width: 22.0,
                            height: 22.0,
                        },
                        badge,
                        5.0,
                    ));
                }
                if rect.height >= 120.0 {
                    let mut divider = palette.border.linear();
                    divider[3] = 0.25;
                    scene.quads.push(Quad::rectangle(
                        Rect {
                            x: rect.x + 12.0,
                            y: rect.y + 33.0,
                            width: rect.width - 24.0,
                            height: 1.0,
                        },
                        divider,
                        0.0,
                    ));
                }
            }
            if viewport.zoom >= PORT_LOD_ZOOM {
                for (count, output) in [(node.inputs, false), (node.outputs, true)] {
                    for i in 0..count {
                        let p = port_anchor(rect, i, count, output);
                        // Ring centres use the exact same anchor as hit testing and cables.
                        for (radius, color) in
                            [(6.0, tone), (4.5, palette.node.linear()), (2.5, tone)]
                        {
                            scene.quads.push(Quad::rectangle(
                                Rect {
                                    x: p[0] - radius,
                                    y: p[1] - radius,
                                    width: radius * 2.0,
                                    height: radius * 2.0,
                                },
                                color,
                                radius,
                            ));
                        }
                    }
                }
            }
            if viewport.zoom >= 0.6 && rect.width >= 48.0 && rect.height >= 40.0 {
                let labels = catalog.get(&node.type_id);
                let after_quad = scene.quads.len();
                let symbol = labels
                    .map(|l| label_text(None, &l.symbol, locale))
                    .unwrap_or_default();
                let has_symbol = !symbol.is_empty() && rect.height >= 120.0 && rect.width >= 96.0;
                if has_symbol {
                    scene.labels.push(TextLabel {
                        text: symbol.chars().take(2).collect(),
                        rect: Rect {
                            x: rect.x + 15.0,
                            y: rect.y + 10.0,
                            width: 16.0,
                            height: 18.0,
                        },
                        font_size: 13.0,
                        right_aligned: false,
                        color: tone,
                        after_quad,
                    });
                }
                let title_inset = if has_symbol { 40.0 } else { 12.0 };
                scene.labels.push(TextLabel {
                    text: label_text(labels.map(|v| &v.title), &node.type_id, locale),
                    rect: Rect {
                        x: rect.x + title_inset,
                        y: rect.y + if rect.height >= 120.0 { 10.0 } else { 4.0 },
                        width: rect.width - title_inset - 12.0,
                        height: 18.0,
                    },
                    font_size: if rect.height >= 120.0 { 15.0 } else { 14.0 },
                    right_aligned: false,
                    color: palette.text.linear(),
                    after_quad,
                });
                let caption = labels
                    .map(|l| label_text(Some(&l.caption), "", locale))
                    .unwrap_or_default();
                let rows = node.inputs.max(node.outputs);
                let bottom_port = rect.height * rows as f32 / (rows + 1) as f32;
                if viewport.zoom >= 0.75
                    && rect.height >= 90.0
                    && !caption.is_empty()
                    && bottom_port + 10.0 <= rect.height - 28.0
                {
                    scene.labels.push(TextLabel {
                        text: caption,
                        rect: Rect {
                            x: rect.x + 12.0,
                            y: rect.y + rect.height - 26.0,
                            width: rect.width - 24.0,
                            height: 16.0,
                        },
                        font_size: 11.0,
                        right_aligned: false,
                        color: palette.muted.linear(),
                        after_quad,
                    });
                }
                if viewport.zoom >= 0.75 {
                    for (names, output) in [(&node.input_names, false), (&node.output_names, true)]
                    {
                        // Dense port rows would overlap; retain circles and omit labels.
                        if rect.height / ((names.len() + 1) as f32) < 16.0 {
                            continue;
                        }
                        for (i, name) in names.iter().enumerate() {
                            let y = port_anchor(rect, i, names.len(), output)[1] - 7.0;
                            let header_bottom =
                                rect.y + if rect.height >= 120.0 { 34.0 } else { 23.0 };
                            if y < header_bottom || y + 14.0 > rect.y + rect.height - 5.0 {
                                continue;
                            }
                            let localized = labels.and_then(|v| {
                                if output {
                                    v.outputs.get(name)
                                } else {
                                    v.inputs.get(name)
                                }
                            });
                            scene.labels.push(TextLabel {
                                text: label_text(localized, name, locale),
                                rect: Rect {
                                    x: rect.x + if output { rect.width / 2.0 + 4.0 } else { 12.0 },
                                    y,
                                    width: rect.width / 2.0 - 16.0,
                                    height: 14.0,
                                },
                                font_size: 11.0,
                                right_aligned: output,
                                color: palette.muted.linear(),
                                after_quad,
                            });
                        }
                    }
                }
            }
            scene.visible_nodes += 1;
        }
        if let Some(cable) = &preview.cable {
            push_curve(
                &mut scene,
                curve(cable.from, cable.to),
                viewport.zoom,
                if cable.valid {
                    palette.cable_valid.linear()
                } else {
                    palette.cable_invalid.linear()
                },
            );
        }
        if let Some(rect) = preview.marquee {
            let mut fill = palette.accent.linear();
            fill[3] = 0.15;
            scene.quads.push(Quad::rectangle(rect, fill, 0.0));
            let width = (1.0 / viewport.zoom).min(rect.width.min(rect.height));
            for border in [
                Rect {
                    height: width,
                    ..rect
                },
                Rect {
                    y: rect.y + rect.height - width,
                    height: width,
                    ..rect
                },
                Rect { width, ..rect },
                Rect {
                    x: rect.x + rect.width - width,
                    width,
                    ..rect
                },
            ] {
                scene
                    .quads
                    .push(Quad::rectangle(border, palette.accent.linear(), 0.0));
            }
        }
        Ok(scene)
    }
}
