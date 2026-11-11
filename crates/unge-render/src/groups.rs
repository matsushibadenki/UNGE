use std::collections::{BTreeMap, BTreeSet};
use unge_core::{Document, Id, Rect, SpatialIndex};
use unge_interaction::Preview;
const PADDING: f32 = 16.0;
const HEADER: f32 = 28.0;
struct RenderGroup {
    label: String,
    nodes: BTreeSet<Id>,
    bounds: Option<Rect>,
}
#[derive(Default)]
pub(crate) struct GroupIndex {
    groups: BTreeMap<Id, RenderGroup>,
    membership: BTreeMap<Id, BTreeSet<Id>>,
    spatial: SpatialIndex,
}
fn bounds(nodes: &BTreeSet<Id>, rect: impl Fn(Id) -> Option<Rect>) -> Option<Rect> {
    let mut rectangles = nodes.iter().filter_map(|id| rect(*id));
    let first = rectangles.next()?;
    let (mut left, mut top, mut right, mut bottom) = (
        first.x,
        first.y,
        first.x + first.width,
        first.y + first.height,
    );
    for r in rectangles {
        left = left.min(r.x);
        top = top.min(r.y);
        right = right.max(r.x + r.width);
        bottom = bottom.max(r.y + r.height);
    }
    let rect = Rect {
        x: left - PADDING,
        y: top - PADDING - HEADER,
        width: right - left + 2.0 * PADDING,
        height: bottom - top + 2.0 * PADDING + HEADER,
    };
    // Derived frames may exceed the engine's coordinate envelope even when
    // individual nodes are valid. Omit those frames instead of invalid GPU data.
    rect.valid().then_some(rect)
}
impl GroupIndex {
    pub(crate) fn new(doc: &Document) -> Self {
        let mut groups = BTreeMap::new();
        let mut membership: BTreeMap<Id, BTreeSet<Id>> = BTreeMap::new();
        let mut rectangles = Vec::new();
        for group in doc.graph().groups().values() {
            let nodes: BTreeSet<_> = group
                .nodes
                .iter()
                .filter(|id| doc.graph().nodes().contains_key(id))
                .copied()
                .collect();
            let bounds = bounds(&nodes, |id| {
                Some(doc.placement().get(&id).copied().unwrap_or_default())
            });
            if let Some(rect) = bounds {
                rectangles.push((group.id, rect));
            }
            for id in &nodes {
                membership.entry(*id).or_default().insert(group.id);
            }
            groups.insert(
                group.id,
                RenderGroup {
                    label: group.label.clone(),
                    nodes,
                    bounds,
                },
            );
        }
        Self {
            groups,
            membership,
            spatial: SpatialIndex::from_rects(rectangles),
        }
    }
    pub(crate) fn collapsed(
        &self,
        ids: &BTreeSet<Id>,
        preview: &Preview,
        rect: impl Fn(Id) -> Option<Rect>,
    ) -> (BTreeMap<Id, Rect>, BTreeMap<Id, Id>) {
        let mut cards = BTreeMap::new();
        let mut hidden = BTreeMap::new();
        for id in ids {
            let Some(group) = self.groups.get(id) else {
                continue;
            };
            let frame = if group
                .nodes
                .iter()
                .any(|node| preview.placement.contains_key(node))
            {
                bounds(&group.nodes, |node| {
                    preview.placement.get(&node).copied().or_else(|| rect(node))
                })
            } else {
                group.bounds
            };
            if let Some(frame) = frame {
                let card = Rect {
                    width: 184.,
                    height: 44.,
                    ..frame
                };
                if card.valid() {
                    cards.insert(*id, card);
                    for node in &group.nodes {
                        hidden.entry(*node).or_insert(*id);
                    }
                }
            }
        }
        (cards, hidden)
    }
    pub(crate) fn hit_header(&self, point: [f32; 2], cards: &BTreeMap<Id, Rect>) -> Option<Id> {
        let contains = |r: Rect| {
            point[0] >= r.x
                && point[0] < r.x + r.width
                && point[1] >= r.y
                && point[1] < r.y + r.height
        };
        cards
            .iter()
            .rev()
            .find(|(_, r)| contains(**r))
            .map(|(id, _)| *id)
            .or_else(|| {
                let area = Rect {
                    x: point[0],
                    y: point[1],
                    width: 0.001,
                    height: 0.001,
                };
                self.spatial.query(area).into_iter().rev().find(|id| {
                    !cards.contains_key(id)
                        && self.groups[id].bounds.is_some_and(|r| {
                            contains(Rect {
                                height: HEADER,
                                ..r
                            })
                        })
                })
            })
    }
    pub(crate) fn label(&self, id: Id) -> &str {
        &self.groups[&id].label
    }
    pub(crate) fn visible(
        &self,
        area: Rect,
        preview: &Preview,
        rect: impl Fn(Id) -> Option<Rect>,
    ) -> Vec<(Id, Rect, &str)> {
        let affected: BTreeSet<_> = preview
            .placement
            .keys()
            .flat_map(|id| self.membership.get(id).into_iter().flatten())
            .copied()
            .collect();
        let mut candidates: BTreeSet<_> = self.spatial.query(area).into_iter().collect();
        candidates.extend(&affected);
        candidates
            .into_iter()
            .filter_map(|id| {
                let group = &self.groups[&id];
                let bounds = if affected.contains(&id) {
                    bounds(&group.nodes, |id| {
                        preview.placement.get(&id).copied().or_else(|| rect(id))
                    })
                } else {
                    group.bounds
                }?;
                bounds
                    .intersects(area)
                    .then_some((id, bounds, group.label.as_str()))
            })
            .collect()
    }
}
