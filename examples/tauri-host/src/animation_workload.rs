//! Deterministic logical pointer input, independent of the OS and GPU.
use std::collections::BTreeSet;
use unge_core::{Document, Editor, Id, Viewport};
use unge_interaction::{Interaction, PointerButton, PointerEvent};
use unge_render::{SceneChanges, SceneIndex};

#[derive(Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    Pan,
    Zoom,
    Drag,
}

pub struct Workload {
    pub editor: Editor,
    pub index: SceneIndex,
    pub viewport: Viewport,
    pub selection: BTreeSet<Id>,
    pub interaction: Interaction,
    start: [f32; 2],
}
impl Workload {
    pub fn new(document: Document, size: [f32; 2]) -> Result<Self, String> {
        Ok(Self {
            index: SceneIndex::new(&document),
            editor: Editor::new(document, 16).map_err(|e| e.to_string())?,
            viewport: Viewport {
                origin: [0., 0.],
                zoom: 1.,
                size,
            },
            selection: BTreeSet::new(),
            interaction: Interaction::default(),
            start: [0., 0.],
        })
    }
    pub fn advance(&mut self, motion: Motion, step: usize) -> Result<(), String> {
        if matches!(motion, Motion::Zoom) {
            // Logarithmic sweep from near to overview and back, anchored at the
            // same screen position as the native wheel handler.
            let phase = (step % 60) as f32 / 60.;
            let zoom = 0.02_f32.powf((1. - (phase * std::f32::consts::TAU).cos()) * 0.5);
            self.viewport.zoom_at([0., 0.], zoom / self.viewport.zoom);
            return self.viewport.validate().map_err(|e| e.to_string());
        }
        let phase = step % 13;
        let direction = if (step / 13).is_multiple_of(2) {
            1.
        } else {
            -1.
        };
        let event = if phase == 0 {
            self.start = match motion {
                Motion::Pan => [400., 240.],
                Motion::Drag => {
                    let rect = self.editor.document().placement()[&Id::from_u128(1)];
                    [rect.x + 90., rect.y + 20.]
                }
                Motion::Zoom => unreachable!(),
            };
            PointerEvent::Down {
                pointer: 0,
                position: self.start,
                button: if matches!(motion, Motion::Pan) {
                    PointerButton::Pan
                } else {
                    PointerButton::Primary
                },
                additive: false,
            }
        } else {
            let offset = phase as f32 * direction;
            let position = [self.start[0] + offset * 8., self.start[1] + offset * 3.];
            if phase == 12 {
                PointerEvent::Up {
                    pointer: 0,
                    position,
                }
            } else {
                PointerEvent::Move {
                    pointer: 0,
                    position,
                }
            }
        };
        if let Some(command) = self
            .interaction
            .handle(
                self.editor.document(),
                self.editor.revision(),
                self.index.spatial_index(),
                &mut self.viewport,
                &mut self.selection,
                event,
            )
            .map_err(|e| e.to_string())?
        {
            let changes = SceneChanges::from_command(&command);
            self.editor.execute(command).map_err(|e| e.to_string())?;
            self.index.update(self.editor.document(), &changes);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn continuous_input_preserves_preview_commit_and_index_contracts() {
        let document = crate::render_benchmark::large_graph::document();
        for motion in [Motion::Pan, Motion::Zoom, Motion::Drag] {
            let mut state = Workload::new(document.clone(), [1280., 720.]).unwrap();
            let mut zoom_range = [1_f32, 0_f32];
            for step in 0..65 {
                state.advance(motion, step).unwrap();
                state.viewport.validate().unwrap();
                zoom_range[0] = zoom_range[0].min(state.viewport.zoom);
                zoom_range[1] = zoom_range[1].max(state.viewport.zoom);
                if matches!(motion, Motion::Drag) {
                    assert_eq!(state.editor.revision(), ((step + 1) / 13) as u64);
                    if step % 13 == 5 {
                        assert!(!state.interaction.preview().placement.is_empty());
                        assert_ne!(
                            state.interaction.preview().placement[&Id::from_u128(1)],
                            state.editor.document().placement()[&Id::from_u128(1)]
                        );
                    }
                    if step % 13 == 12 {
                        assert!(state.interaction.preview().placement.is_empty());
                        let expected = SceneIndex::new(state.editor.document())
                            .scene(state.viewport, &state.selection)
                            .unwrap();
                        let actual = state.index.scene(state.viewport, &state.selection).unwrap();
                        assert_eq!(quad_fields(&actual), quad_fields(&expected));
                    }
                }
            }
            if !matches!(motion, Motion::Drag) {
                assert_eq!(state.editor.revision(), 0);
                assert_eq!(state.editor.document().placement(), document.placement());
            }
            match motion {
                Motion::Pan => assert_ne!(state.viewport.origin, [0., 0.]),
                Motion::Zoom => {
                    assert!(zoom_range[0] <= 0.02001);
                    assert!(zoom_range[1] >= 0.9999);
                }
                Motion::Drag => assert_eq!(
                    state.editor.document().placement()[&Id::from_u128(1)].x,
                    96.
                ),
            }
        }
    }
    fn quad_fields(scene: &unge_render::Scene) -> Vec<[[f32; 4]; 3]> {
        scene
            .quads
            .iter()
            .map(|q| [q.rect, q.color, q.params])
            .collect()
    }
}
