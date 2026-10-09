//! Tao input bridge for a native Tauri window. Keep this unstable Wry API at
//! the host boundary; unge-interaction itself has no window/runtime dependency.
use tauri::{Emitter, EventLoopMessage, Manager};
use tauri_runtime_wry::tao::{
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopProxy, EventLoopWindowTarget},
    keyboard::{Key, ModifiersState},
};
use tauri_runtime_wry::{
    Context, EventLoopIterationContext, Message, Plugin, PluginBuilder, WebContextStore,
};
use unge_interaction::{PointerButton, PointerEvent};
use unge_tauri::{Engine, Request};

pub struct InputBridge {
    pub app: tauri::AppHandle,
    pub engine: Engine,
}
pub struct Input {
    app: tauri::AppHandle,
    engine: Engine,
    position: Option<[f32; 2]>,
    modifiers: ModifiersState,
    button: Option<MouseButton>,
    revision: u64,
    pending_move: bool,
    dirty: bool,
    selection: Option<unge_tauri::SelectionSummary>,
}
impl PluginBuilder<EventLoopMessage> for InputBridge {
    type Plugin = Input;
    fn build(self, _: Context<EventLoopMessage>) -> Input {
        Input {
            app: self.app,
            engine: self.engine,
            position: None,
            modifiers: ModifiersState::default(),
            button: None,
            revision: 0,
            pending_move: false,
            dirty: false,
            selection: None,
        }
    }
}
impl Input {
    fn send(&mut self, event: PointerEvent) {
        match self.engine.dispatch(
            "controls",
            Request::Pointer {
                expected_revision: self.revision,
                event,
            },
        ) {
            Ok(summary) => {
                if summary.revision != self.revision {
                    let _ = self.app.emit("unge://changed", &summary);
                }
            }
            Err(error) => {
                eprintln!("pointer: {}: {}", error.code, error.message);
                let _ = self.app.emit("unge://interaction-error", &error);
                self.button = None;
            }
        }
        self.dirty = true;
    }
    fn cancel(&mut self) {
        if let Ok(summary) = self.engine.dispatch("controls", Request::Summary) {
            self.revision = summary.revision;
        }
        self.send(PointerEvent::Cancel);
        self.button = None;
        self.pending_move = false;
    }
    fn flush_move(&mut self) {
        if self.pending_move {
            self.pending_move = false;
            if self.button.is_some()
                && let Some(position) = self.position
            {
                self.send(PointerEvent::Move {
                    pointer: 0,
                    position,
                });
            }
        }
    }
}
impl Plugin<EventLoopMessage> for Input {
    fn on_event(
        &mut self,
        event: &Event<Message<EventLoopMessage>>,
        _: &EventLoopWindowTarget<Message<EventLoopMessage>>,
        _: &EventLoopProxy<Message<EventLoopMessage>>,
        _: &mut ControlFlow,
        context: EventLoopIterationContext<'_, EventLoopMessage>,
        _: &WebContextStore,
    ) -> bool {
        if matches!(event, Event::MainEventsCleared) {
            self.flush_move();
            if std::mem::take(&mut self.dirty) {
                super::redraw(&self.app);
                if let Ok(selection) = self.engine.selection_summary("controls")
                    && self.selection.as_ref() != Some(&selection)
                {
                    if let Some(view) = self.app.get_webview("controls") {
                        let _ = view.emit("unge://selection-changed", &selection);
                    }
                    self.selection = Some(selection);
                }
            }
            return false;
        }
        let Event::WindowEvent {
            window_id, event, ..
        } = event
        else {
            return false;
        };
        let composition = self.app.state::<super::workspace::Composition>();
        let label = composition.canvas_label();
        let canvas = context.window_id_map.get(window_id).is_some_and(|id| {
            context
                .windows
                .0
                .borrow()
                .get(&id)
                .is_some_and(|w| w.label() == label)
        });
        if !canvas {
            return false;
        }
        let Some(window) = self.app.get_window(label) else {
            return false;
        };
        let scale = window.scale_factor().unwrap_or(1.0);
        let size = window.inner_size().unwrap_or_default();
        // Compute hit boundaries from the same physical split used by rendering.
        let position = if let WindowEvent::CursorMoved { position, .. } = event {
            composition.graph_position([size.width, size.height], scale, [position.x, position.y])
        } else {
            self.position
        };
        match event {
            WindowEvent::CursorMoved { .. } => {
                if position.is_none() && self.button.is_some() {
                    self.cancel();
                }
                self.position = position;
                self.pending_move = self.button.is_some();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = *modifiers,
            WindowEvent::MouseInput { state, button, .. } => {
                if !matches!(
                    button,
                    MouseButton::Left | MouseButton::Middle | MouseButton::Right
                ) {
                    return false;
                }
                self.flush_move();
                let Some(position) = self.position else {
                    return false;
                };
                if *state == ElementState::Pressed && self.button.is_none() {
                    if let Ok(summary) = self.engine.dispatch("controls", Request::Summary) {
                        self.revision = summary.revision;
                    }
                    self.button = Some(*button);
                    self.send(PointerEvent::Down {
                        pointer: 0,
                        position,
                        button: if *button == MouseButton::Left {
                            PointerButton::Primary
                        } else {
                            PointerButton::Pan
                        },
                        additive: self.modifiers.shift_key(),
                    });
                } else if *state == ElementState::Released && self.button == Some(*button) {
                    self.send(PointerEvent::Up {
                        pointer: 0,
                        position,
                    });
                    self.button = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.button.is_none() => {
                if let (Some(position), Ok(state)) =
                    (self.position, self.engine.view_state("controls"))
                {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => f64::from(*y) * 40.,
                        MouseScrollDelta::PixelDelta(p) => p.y / scale,
                        _ => return false,
                    };
                    let mut viewport = state.viewport;
                    viewport.zoom_at(position, (dy.clamp(-400., 400.) as f32 * 0.002).exp());
                    if let Err(error) = self
                        .engine
                        .dispatch("controls", Request::SetViewport { viewport })
                    {
                        let _ = self.app.emit("unge://interaction-error", error);
                    }
                    self.dirty = true;
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Escape =>
            {
                if self.position.is_some() || self.button.is_some() {
                    self.cancel();
                }
            }
            WindowEvent::Focused(false)
            | WindowEvent::Resized(_)
            | WindowEvent::ScaleFactorChanged { .. } => {
                self.cancel();
                self.position = None;
                self.modifiers = ModifiersState::default();
            }
            WindowEvent::CursorLeft { .. } => {
                // Cancel explicitly if the host cannot guarantee pointer capture.
                self.cancel();
                self.position = None;
            }
            _ => {}
        }
        false // Tauri still handles its normal window lifecycle.
    }
}
