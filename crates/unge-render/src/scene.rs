use std::collections::{BTreeMap, BTreeSet};
use unge_core::{Document, Id, Rect, SpatialIndex, Viewport};

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
    fn line(a: [f32; 2], b: [f32; 2]) -> Self {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        Self {
            rect: [
                (a[0] + b[0]) / 2.0,
                (a[1] + b[1]) / 2.0,
                dx.hypot(dy).max(0.001),
                2.0,
            ],
            color: [0.32, 0.65, 0.74, 1.0],
            params: [dy.atan2(dx), 0.8, 0.0, 0.0],
        }
    }
}
struct RenderNode {
    rect: Rect,
    inputs: usize,
    outputs: usize,
}
struct RenderEdge {
    points: [[f32; 2]; 4],
}
/// Build once per document revision, not once per animation frame.
pub struct SceneIndex {
    nodes: BTreeMap<Id, RenderNode>,
    edges: BTreeMap<Id, RenderEdge>,
    node_index: SpatialIndex,
    edge_index: SpatialIndex,
}
#[derive(Debug, Default)]
pub struct Scene {
    pub quads: Vec<Quad>,
    pub visible_nodes: usize,
    pub visible_edges: usize,
}
fn anchor(rect: Rect, index: usize, count: usize, output: bool) -> [f32; 2] {
    [
        rect.x + if output { rect.width } else { 0.0 },
        rect.y + rect.height * ((index + 1) as f32 / (count + 1) as f32),
    ]
}
impl SceneIndex {
    pub fn new(doc: &Document) -> Self {
        let nodes: BTreeMap<_, _> = doc
            .graph()
            .nodes()
            .values()
            .map(|node| {
                (
                    node.id,
                    RenderNode {
                        rect: doc.placement().get(&node.id).copied().unwrap_or_default(),
                        inputs: node.inputs.len(),
                        outputs: node.outputs.len(),
                    },
                )
            })
            .collect();
        let mut edges = BTreeMap::new();
        let mut bounds = Vec::new();
        for edge in doc.graph().edges().values() {
            let Some(from) = nodes.get(&edge.from.node) else {
                continue;
            };
            let Some(to) = nodes.get(&edge.to.node) else {
                continue;
            };
            let Some(i) = doc.graph().nodes()[&edge.from.node]
                .outputs
                .iter()
                .position(|p| p.name == edge.from.port)
            else {
                continue;
            };
            let Some(j) = doc.graph().nodes()[&edge.to.node]
                .inputs
                .iter()
                .position(|p| p.name == edge.to.port)
            else {
                continue;
            };
            let a = anchor(from.rect, i, from.outputs, true);
            let b = anchor(to.rect, j, to.inputs, false);
            let dx = ((b[0] - a[0]).abs() * 0.5).max(50.0);
            let points = [a, [a[0] + dx, a[1]], [b[0] - dx, b[1]], b];
            let min_x = points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min) - 2.0;
            let max_x = points
                .iter()
                .map(|p| p[0])
                .fold(f32::NEG_INFINITY, f32::max)
                + 2.0;
            let min_y = a[1].min(b[1]) - 2.0;
            let max_y = a[1].max(b[1]) + 2.0;
            bounds.push((
                edge.id,
                Rect {
                    x: min_x,
                    y: min_y,
                    width: max_x - min_x,
                    height: max_y - min_y,
                },
            ));
            edges.insert(edge.id, RenderEdge { points });
        }
        Self {
            nodes,
            edges,
            node_index: SpatialIndex::new(doc),
            edge_index: SpatialIndex::from_rects(bounds),
        }
    }
    pub fn scene(&self, viewport: Viewport, selection: &BTreeSet<Id>) -> unge_core::Result<Scene> {
        viewport.validate()?;
        let area = viewport.world_rect();
        let mut scene = Scene::default();
        let mut grid = Quad::rectangle(area, [0.04, 0.052, 0.075, 1.0], 0.0);
        grid.params[2] = 1.0;
        scene.quads.push(grid);
        for id in self.edge_index.query(area) {
            let points = self.edges[&id].points;
            let segments = if viewport.zoom < 0.3 { 8 } else { 24 };
            let mut previous = points[0];
            for i in 1..=segments {
                let t = i as f32 / segments as f32;
                let u = 1.0 - t;
                let next = [0, 1].map(|axis| {
                    u * u * u * points[0][axis]
                        + 3.0 * u * u * t * points[1][axis]
                        + 3.0 * u * t * t * points[2][axis]
                        + t * t * t * points[3][axis]
                });
                scene.quads.push(Quad::line(previous, next));
                previous = next;
            }
            scene.visible_edges += 1;
        }
        // Include ports/selection borders protruding from the node bounds.
        let padded = Rect {
            x: area.x - 8.0,
            y: area.y - 8.0,
            width: area.width + 16.0,
            height: area.height + 16.0,
        };
        for id in self.node_index.query(padded) {
            let node = &self.nodes[&id];
            let rect = node.rect;
            let selected = selection.contains(&id);
            let border = if selected {
                [0.2, 0.8, 0.72, 1.0]
            } else {
                [0.24, 0.29, 0.37, 1.0]
            };
            scene.quads.push(Quad::rectangle(rect, border, 8.0));
            scene.quads.push(Quad::rectangle(
                Rect {
                    x: rect.x + 1.5,
                    y: rect.y + 1.5,
                    width: (rect.width - 3.0).max(0.1),
                    height: (rect.height - 3.0).max(0.1),
                },
                [0.09, 0.115, 0.16, 1.0],
                7.0,
            ));
            if viewport.zoom >= 0.3 {
                for (count, output) in [(node.inputs, false), (node.outputs, true)] {
                    for i in 0..count {
                        let p = anchor(rect, i, count, output);
                        scene.quads.push(Quad::rectangle(
                            Rect {
                                x: p[0] - 4.0,
                                y: p[1] - 4.0,
                                width: 8.0,
                                height: 8.0,
                            },
                            [0.35, 0.78, 0.72, 1.0],
                            4.0,
                        ));
                    }
                }
            }
            scene.visible_nodes += 1;
        }
        Ok(scene)
    }
}
