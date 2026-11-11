//! Native composition stays in the host. Only geometry is shared with input/rendering.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager};

type PanelResult<T> = Result<T, unge_tauri::ApiError>;
fn panel_error(code: &str, message: impl ToString) -> unge_tauri::ApiError {
    unge_tauri::ApiError {
        code: code.into(),
        message: message.to_string(),
    }
}

pub fn panel_title(locale: unge_core::Locale) -> &'static str {
    match locale {
        unge_core::Locale::En => "UNGE · Settings",
        unge_core::Locale::Ja => "UNGE · 設定",
        unge_core::Locale::ZhCn => "UNGE · 设置",
    }
}

pub struct Composition {
    pub unified: bool,
    floating: AtomicBool,
    transitioning: AtomicBool,
    applied_size: Mutex<Option<([u32; 2], u64)>>,
    canvas_editor: Mutex<Option<([f32; 2], u32)>>,
}
#[derive(Debug, PartialEq)]
pub struct Layout {
    pub graph: [u32; 2],
    pub panel: [u32; 2],
}
impl Composition {
    pub fn new(unified: bool) -> Self {
        Self {
            unified,
            floating: AtomicBool::new(false),
            transitioning: AtomicBool::new(false),
            applied_size: Mutex::new(None),
            canvas_editor: Mutex::new(None),
        }
    }
    pub fn is_floating(&self) -> bool {
        self.floating.load(Ordering::Acquire)
    }
    pub fn is_transitioning(&self) -> bool {
        self.transitioning.load(Ordering::Acquire)
    }
    fn begin_transition(&self) -> PanelResult<Transition<'_>> {
        self.transitioning
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| panel_error("panel_busy", "panel transition in progress"))?;
        Ok(Transition(&self.transitioning))
    }
    fn status(&self) -> serde_json::Value {
        serde_json::json!({"supported": self.unified, "floating": self.is_floating()})
    }
    pub fn canvas_label(&self) -> &'static str {
        if self.unified { "workspace" } else { "canvas" }
    }
    pub fn layout(&self, size: [u32; 2], scale: f64) -> Layout {
        let panel_width = if self.unified
            && !self.is_floating()
            && self.canvas_editor.lock().expect("canvas editor").is_none()
        {
            ((380. * scale).round() as u32).min(size[0] / 2)
        } else {
            0
        };
        Layout {
            graph: [size[0] - panel_width, size[1]],
            panel: [panel_width, size[1]],
        }
    }
    pub fn graph_position(
        &self,
        size: [u32; 2],
        scale: f64,
        position: [f64; 2],
    ) -> Option<[f32; 2]> {
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        let graph = self.layout(size, scale).graph;
        let overlay = self.editor_bounds(size, scale);
        if overlay.is_some_and(|(origin, extent)| {
            (0..2).all(|i| {
                position[i] >= f64::from(origin[i])
                    && position[i] < f64::from(origin[i] + extent[i])
            })
        }) {
            return None;
        }
        (position.iter().all(|v| v.is_finite() && *v >= 0.)
            && position[0] < f64::from(graph[0])
            && position[1] < f64::from(graph[1]))
        .then(|| position.map(|v| (v / scale) as f32))
    }
    fn editor_bounds(&self, size: [u32; 2], scale: f64) -> Option<([u32; 2], [u32; 2])> {
        if !self.unified || self.is_floating() {
            return None;
        }
        let (anchor, height) = (*self.canvas_editor.lock().ok()?)?;
        let graph = self.layout(size, scale).graph;
        let extent = [
            graph[0].min((380. * scale) as u32),
            graph[1].min((f64::from(height) * scale).round() as u32),
        ];
        let origin: [u32; 2] =
            std::array::from_fn(|i| ((f64::from(anchor[i]) + 8.) * scale) as u32);
        Some((
            std::array::from_fn(|i| origin[i].min(graph[i] - extent[i])),
            extent,
        ))
    }
    pub fn resize_panel(
        &self,
        app: &tauri::AppHandle,
        size: [u32; 2],
        scale: f64,
    ) -> tauri::Result<()> {
        if !self.unified || size.contains(&0) {
            return Ok(());
        }
        let (size, scale) = if self.is_floating() {
            let Some(window) = app.get_window("inspector") else {
                return Ok(());
            };
            let size = window.inner_size()?;
            ([size.width, size.height], window.scale_factor()?)
        } else {
            (size, scale)
        };
        if size.contains(&0) {
            return Ok(());
        }
        let key = (size, scale.to_bits());
        if *self.applied_size.lock().expect("panel layout") == Some(key) {
            return Ok(());
        }
        let layout = if self.is_floating() {
            Layout {
                graph: [0, size[1]],
                panel: size,
            }
        } else {
            self.layout(size, scale)
        };
        if let Some(panel) = app.get_webview("controls") {
            let (origin, extent) = self
                .editor_bounds(size, scale)
                .unwrap_or(([layout.graph[0], 0], layout.panel));
            panel.set_bounds(tauri::Rect {
                position: tauri::PhysicalPosition::new(origin[0] as i32, origin[1] as i32).into(),
                size: tauri::PhysicalSize::new(extent[0], extent[1]).into(),
            })?;
            *self.applied_size.lock().expect("panel layout") = Some(key);
        }
        Ok(())
    }
}
// Only host-native operations run here. The same WebView and draft survive reparenting.
struct Transition<'a>(&'a AtomicBool);
impl Drop for Transition<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command]
pub fn panel_layout(webview: tauri::Webview) -> PanelResult<serde_json::Value> {
    authorize(webview.label())?;
    Ok(webview.state::<Composition>().status())
}
fn authorize(label: &str) -> PanelResult<()> {
    if label != "controls" {
        return Err(panel_error("unknown_view", label));
    }
    Ok(())
}
#[tauri::command]
pub async fn canvas_editor(
    webview: tauri::Webview,
    position: Option<[f32; 2]>,
    height: Option<u32>,
) -> PanelResult<bool> {
    authorize(webview.label())?;
    if position.is_some_and(|p| p.iter().any(|v| !v.is_finite() || *v < 0.)) {
        return Err(panel_error(
            "invalid_pointer",
            "invalid canvas editor anchor",
        ));
    }
    let app = webview.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let c = app.state::<Composition>();
        if !c.unified || c.is_floating() {
            return Ok(false);
        }
        let transition = c.begin_transition()?;
        let window = app
            .get_window(c.canvas_label())
            .ok_or_else(|| panel_error("panel_error", "workspace missing"))?;
        let size = window
            .inner_size()
            .map_err(|e| panel_error("panel_error", e))?;
        let scale = window
            .scale_factor()
            .map_err(|e| panel_error("panel_error", e))?;
        *c.canvas_editor.lock().expect("canvas editor") =
            position.map(|p| (p, height.unwrap_or(440).clamp(160, 520)));
        *c.applied_size.lock().expect("panel layout") = None;
        let result = c.resize_panel(&app, [size.width, size.height], scale);
        if result.is_err() {
            *c.canvas_editor.lock().expect("canvas editor") = None;
            let _ = c.resize_panel(&app, [size.width, size.height], scale);
        }
        // Keep the Surface and input boundaries in sync after the WebView moves.
        drop(transition);
        let main = app.clone();
        app.run_on_main_thread(move || super::redraw(&main))
            .map_err(|e| panel_error("panel_error", e))?;
        result.map_err(|e| panel_error("panel_error", e))?;
        Ok(position.is_some())
    })
    .await
    .map_err(|e| panel_error("panel_error", e))?
}

#[tauri::command]
pub async fn set_panel_floating(
    webview: tauri::Webview,
    floating: bool,
) -> PanelResult<serde_json::Value> {
    authorize(webview.label())?;
    let app = webview.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || move_panel(&app, floating))
        .await
        .map_err(|e| panel_error("panel_error", e))?
}
pub fn move_panel(app: &tauri::AppHandle, floating: bool) -> PanelResult<serde_json::Value> {
    let c = app.state::<Composition>();
    if !c.unified {
        return Err(panel_error("panel_unavailable", "composition unavailable"));
    }
    let _transition = c.begin_transition()?;
    if c.canvas_editor.lock().expect("canvas editor").is_some() {
        return Err(panel_error(
            "panel_busy",
            "close the canvas editor before moving the panel",
        ));
    }
    if floating == c.is_floating() {
        return Ok(c.status());
    }
    let operation = || -> Result<(), String> {
        let panel = app.get_webview("controls").ok_or("controls missing")?;
        let home = app.get_window("workspace").ok_or("workspace missing")?;
        let engine = app.state::<unge_tauri::Engine>();
        let locale = engine.view_state("controls").map_err(|e| e.message)?.locale;
        let detached = match app.get_window("inspector") {
            Some(window) => window,
            None => {
                let window = tauri::WindowBuilder::new(app, "inspector")
                    .title(panel_title(locale))
                    .inner_size(380., 720.)
                    .min_inner_size(320., 400.)
                    .visible(false)
                    .center()
                    .build()
                    .map_err(|e| e.to_string())?;
                if let Err(error) = app
                    .state::<super::panel_preferences::Store>()
                    .restore(&window, [320., 400.])
                {
                    eprintln!("inspector restore: {error}");
                }
                window
            }
        };
        // Cancel graph previews before the viewport changes. No Document command.
        let summary = engine
            .dispatch("controls", unge_tauri::Request::Summary)
            .map_err(|e| e.message)?;
        engine
            .dispatch(
                "controls",
                unge_tauri::Request::Pointer {
                    expected_revision: summary.revision,
                    event: unge_interaction::PointerEvent::Cancel,
                },
            )
            .map_err(|e| e.message)?;
        let source = if floating { &home } else { &detached };
        let target = if floating { &detached } else { &home };
        if let Err(error) = panel.reparent(target) {
            // Tauri updates its Window reference before the runtime call succeeds.
            // Attempt to restore that reference as well as native ownership.
            let _ = panel.reparent(source);
            return Err(error.to_string());
        }
        c.floating.store(floating, Ordering::Release);
        app.state::<super::panel_preferences::Store>()
            .record_mode(floating);
        *c.applied_size.lock().expect("panel layout") = None;
        let size = home.inner_size().map_err(|e| e.to_string())?;
        c.resize_panel(
            app,
            [size.width, size.height],
            home.scale_factor().map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if floating {
            detached.show().map_err(|e| e.to_string())?;
            detached.set_focus().map_err(|e| e.to_string())?;
        } else {
            detached.hide().map_err(|e| e.to_string())?;
            home.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    let result = operation();
    let status = c.status();
    drop(_transition);
    let _ = app.emit("unge://panel-layout", &status);
    let main = app.clone();
    let _ = app.run_on_main_thread(move || super::redraw(&main));
    result.map_err(|e| panel_error("panel_error", e))?;
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canvas_editor_clamps_to_the_graph_and_excludes_its_input_region() {
        let composition = Composition::new(true);
        let size = [2400, 1600];
        let scale = 2.;
        let anchor = [800., 700.];
        *composition.canvas_editor.lock().unwrap() = Some((anchor, 520));
        let (origin, extent) = composition.editor_bounds(size, scale).unwrap();
        assert_eq!(origin, [1616, 560]);
        assert_eq!(composition.layout(size, scale).graph, size);
        assert_eq!(
            composition.graph_position(size, scale, [2350., 200.]),
            Some([1175., 100.])
        );
        assert_eq!(extent, [760, 1040]);
        assert_eq!(
            composition.graph_position(size, scale, [1615., 560.]),
            Some([807.5, 280.])
        );
        assert_eq!(composition.graph_position(size, scale, [1616., 560.]), None);
        *composition.canvas_editor.lock().unwrap() = None;
        assert_eq!(composition.layout(size, scale).graph, [1640, 1600]);
        assert_eq!(
            composition.graph_position(size, scale, [880., 560.]),
            Some([440., 280.])
        );
    }

    #[test]
    fn floating_expands_graph_and_transition_guard_serializes_requests() {
        let c = Composition::new(true);
        let guard = c.begin_transition().unwrap();
        assert!(c.begin_transition().is_err());
        c.floating.store(true, Ordering::Release);
        assert_eq!(c.layout([1200, 800], 1.).graph, [1200, 800]);
        assert_eq!(
            c.graph_position([1200, 800], 1., [1100., 100.]),
            Some([1100., 100.])
        );
        drop(guard);
        assert!(!c.is_transitioning());
        assert!(c.begin_transition().is_ok());
        c.floating.store(false, Ordering::Release);
        assert_eq!(c.layout([1200, 800], 1.).graph, [820, 800]);
        assert!(authorize("workspace").is_err());
        assert!(authorize("controls").is_ok());
    }
    #[test]
    fn split_and_input_share_physical_boundary_at_all_scales() {
        for scale in [1., 1.25, 2.] {
            let c = Composition::new(true);
            for width in [1, 320, 720, 1280, 2560] {
                let size = [width, 900];
                let l = c.layout(size, scale);
                assert_eq!(l.graph[0] + l.panel[0], width);
                assert!(l.graph[0] >= l.panel[0]);
                let edge = f64::from(l.graph[0]);
                assert_eq!(c.graph_position(size, scale, [edge, 20.]), None);
                assert_eq!(
                    c.graph_position(size, scale, [edge - 1., 20.]),
                    Some([((edge - 1.) / scale) as f32, (20. / scale) as f32])
                );
                assert_eq!(c.graph_position(size, scale, [-1., 20.]), None);
                assert_eq!(c.graph_position(size, scale, [0., 900.]), None);
            }
        }
        let c = Composition::new(false);
        assert_eq!(c.layout([1000, 700], 2.).graph, [1000, 700]);
        assert_eq!(
            c.graph_position([1000, 700], 2., [800., 100.]),
            Some([400., 50.])
        );
    }
}
