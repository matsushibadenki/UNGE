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
        let panel_width = if self.unified && !self.is_floating() {
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
        (position.iter().all(|v| v.is_finite() && *v >= 0.)
            && position[0] < f64::from(graph[0])
            && position[1] < f64::from(graph[1]))
        .then(|| position.map(|v| (v / scale) as f32))
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
            panel.set_bounds(tauri::Rect {
                position: tauri::PhysicalPosition::new(layout.graph[0] as i32, 0).into(),
                size: tauri::PhysicalSize::new(layout.panel[0], layout.panel[1]).into(),
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
