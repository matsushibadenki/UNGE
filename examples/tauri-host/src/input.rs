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
#[cfg(target_os = "macos")]
#[path = "input_macos.rs"]
mod macos;

use unge_core::Viewport;
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
    right_start: Option<[f32; 2]>,
    pending_move: bool,
    dirty: bool,
    selection: Option<unge_tauri::SelectionSummary>,
    #[cfg(target_os = "macos")]
    pinches: macos::Pending,
}
impl PluginBuilder<EventLoopMessage> for InputBridge {
    type Plugin = Input;
    fn build(self, _context: Context<EventLoopMessage>) -> Input {
        #[cfg(target_os = "macos")]
        let pinches = {
            let pending = macos::Pending::default();
            let composition = self.app.state::<super::workspace::Composition>();
            if let Some(canvas) = self.app.get_window(composition.canvas_label())
                && !macos::install(&canvas, pending.clone(), _context.proxy)
            {
                eprintln!("AppKit magnification monitor unavailable");
            }
            pending
        };
        Input {
            app: self.app,
            engine: self.engine,
            position: None,
            modifiers: ModifiersState::default(),
            button: None,
            revision: 0,
            right_start: None,
            pending_move: false,
            dirty: false,
            selection: None,
            #[cfg(target_os = "macos")]
            pinches,
        }
    }
}
impl Input {
    fn set_viewport(&mut self, viewport: Viewport) {
        if let Err(error) = self
            .engine
            .dispatch("controls", Request::SetViewport { viewport })
        {
            let _ = self.app.emit("unge://interaction-error", error);
        }
        self.dirty = true;
    }

    #[cfg(target_os = "macos")]
    fn flush_pinches(&mut self) {
        let events = self
            .pinches
            .lock()
            .map(|mut p| std::mem::take(&mut *p))
            .unwrap_or_default();
        if events.is_empty() || self.button.is_some() {
            return;
        }
        let composition = self.app.state::<super::workspace::Composition>();
        if composition.is_transitioning() {
            return;
        }
        let Some(canvas) = self.app.get_window(composition.canvas_label()) else {
            return;
        };
        let (Ok(size), Ok(scale), Ok(state)) = (
            canvas.inner_size(),
            canvas.scale_factor(),
            self.engine.view_state("controls"),
        ) else {
            return;
        };
        if state.interacting {
            return;
        }
        let mut viewport = state.viewport;
        for event in events {
            if let Some(anchor) = composition.graph_position(
                [size.width, size.height],
                scale,
                event.position.map(|v| v * scale),
            ) && let Some(next) = pinch_viewport(viewport, anchor, event.magnification)
            {
                viewport = next;
            }
        }
        if viewport.zoom != state.viewport.zoom || viewport.origin != state.viewport.origin {
            self.set_viewport(viewport);
        }
    }
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
        self.right_start = None;
    }
    fn begin(&mut self, button: MouseButton, position: [f32; 2]) {
        if let Ok(summary) = self.engine.dispatch("controls", Request::Summary) {
            self.revision = summary.revision;
        }
        self.button = Some(button);
        self.send(PointerEvent::Down {
            pointer: 0,
            position,
            button: if button == MouseButton::Left {
                PointerButton::Primary
            } else {
                PointerButton::Pan
            },
            additive: self.modifiers.shift_key(),
        });
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
        #[cfg(target_os = "macos")]
        if matches!(event, Event::LoopDestroyed) {
            macos::remove();
        }
        if matches!(event, Event::MainEventsCleared) {
            #[cfg(target_os = "macos")]
            self.flush_pinches();
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
        let transitioning = composition.is_transitioning();
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
                if let (Some(start), Some(point)) = (self.right_start, position)
                    && (point[0] - start[0]).hypot(point[1] - start[1]) >= 5.
                {
                    self.right_start = None;
                    self.begin(MouseButton::Right, start);
                }
                if position.is_none() {
                    self.right_start = None;
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
                if *state == ElementState::Pressed
                    && self.button.is_none()
                    && self.right_start.is_none()
                {
                    if *button == MouseButton::Right {
                        self.right_start = Some(position);
                    } else {
                        self.begin(*button, position);
                    }
                } else if *state == ElementState::Released
                    && *button == MouseButton::Right
                    && self.right_start.take().is_some()
                {
                    if !transitioning
                        && let Ok(context) = self.engine.canvas_context("controls", position)
                    {
                        let _ = window.run_on_main_thread({
                            let window = window.clone();
                            move || {
                                if let Err(error) =
                                    super::context_menu::show(&window, context, position)
                                {
                                    eprintln!("context menu: {error}");
                                }
                            }
                        });
                    }
                } else if *state == ElementState::Released && self.button == Some(*button) {
                    self.send(PointerEvent::Up {
                        pointer: 0,
                        position,
                    });
                    self.button = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.button.is_none() => {
                if !composition.is_transitioning()
                    && let (Some(position), Ok(state)) =
                        (self.position, self.engine.view_state("controls"))
                    && !state.interacting
                    && let Some(viewport) = scroll_viewport(state.viewport, position, *delta, scale)
                {
                    self.set_viewport(viewport);
                }
            }
            WindowEvent::KeyboardInput { event, .. }
                if event.state == ElementState::Pressed && event.logical_key == Key::Escape =>
            {
                if self.position.is_some() || self.button.is_some() || self.right_start.is_some() {
                    self.cancel();
                    let _ = self.app.emit("unge://canvas-cancel", ());
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

fn scroll_viewport(
    mut viewport: Viewport,
    anchor: [f32; 2],
    delta: MouseScrollDelta,
    scale: f64,
) -> Option<Viewport> {
    match delta {
        MouseScrollDelta::LineDelta(_, y) if y.is_finite() && y != 0. => {
            viewport.zoom_at(anchor, ((y * 40.).clamp(-400., 400.) * 0.002).exp());
        }
        MouseScrollDelta::PixelDelta(p)
            if scale.is_finite()
                && scale > 0.
                && p.x.is_finite()
                && p.y.is_finite()
                && (p.x != 0. || p.y != 0.) =>
        {
            // Tao inverts AppKit's horizontal scroll sign. Pan follows content motion.
            let x = if cfg!(target_os = "macos") { -p.x } else { p.x };
            viewport.pan([(x / scale) as f32, (p.y / scale) as f32]);
        }
        _ => return None,
    }
    viewport.validate().ok().map(|()| viewport)
}

#[cfg(any(target_os = "macos", test))]
fn pinch_viewport(
    mut viewport: Viewport,
    anchor: [f32; 2],
    magnification: f64,
) -> Option<Viewport> {
    let factor = (1. + magnification) as f32;
    if !factor.is_finite() || factor <= 0. || factor == 1. {
        return None;
    }
    viewport.zoom_at(anchor, factor);
    viewport.validate().ok().map(|()| viewport)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_runtime_wry::tao::dpi::PhysicalPosition;

    fn viewport() -> Viewport {
        Viewport {
            origin: [10., -20.],
            size: [800., 600.],
            zoom: 2.,
        }
    }

    #[test]
    fn precise_scroll_moves_both_axes_in_logical_points() {
        let view = viewport();
        let dx = if cfg!(target_os = "macos") { -24. } else { 24. };
        for sign in [1., -1.] {
            let next = scroll_viewport(
                view,
                [200., 150.],
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(sign * dx, sign * -16.)),
                2.,
            )
            .unwrap();
            assert_eq!(
                next.origin,
                [10. - sign as f32 * 6., -20. + sign as f32 * 4.]
            );
            assert_eq!(next.zoom, view.zoom);
            assert_eq!(next.size, view.size);
        }
        assert!(
            scroll_viewport(
                view,
                [0., 0.],
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(1., 1.)),
                0.
            )
            .is_none()
        );
        assert!(
            scroll_viewport(
                view,
                [0., 0.],
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(f64::NAN, 1.)),
                2.
            )
            .is_none()
        );
    }

    #[test]
    fn pinch_and_mouse_wheel_keep_the_anchor_stationary() {
        let view = viewport();
        let anchor = [200., 100.];
        let pinched_in = pinch_viewport(view, anchor, 0.5).unwrap();
        let pinched_out = pinch_viewport(view, anchor, -0.5).unwrap();
        let wheel = scroll_viewport(view, anchor, MouseScrollDelta::LineDelta(0., 1.), 2.).unwrap();
        assert_eq!(pinched_in.zoom, 3.);
        assert_eq!(pinched_out.zoom, 1.);
        assert!(wheel.zoom > view.zoom);
        for next in [pinched_in, pinched_out, wheel] {
            let before = view.to_world(anchor);
            let after = next.to_world(anchor);
            assert!((before[0] - after[0]).abs() < 0.001);
            assert!((before[1] - after[1]).abs() < 0.001);
        }
    }

    #[test]
    fn pinch_respects_zoom_limits_and_rejects_invalid_input() {
        let view = viewport();
        assert_eq!(pinch_viewport(view, [0., 0.], 100.).unwrap().zoom, 16.);
        assert_eq!(pinch_viewport(view, [0., 0.], -0.999).unwrap().zoom, 0.02);
        for invalid in [0., -1., -2., f64::NAN, f64::INFINITY] {
            assert!(pinch_viewport(view, [0., 0.], invalid).is_none());
        }
    }
}
