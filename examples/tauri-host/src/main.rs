use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};
use unge_core::*;
use unge_render::SurfaceRenderer;
use unge_tauri::Engine;

fn initial_document() -> Document {
    let registry = unge_executor::math_registry();
    let mut editor = Editor::new(Document::default(), 100).unwrap();
    let mut commands = Vec::new();
    for i in 0..12 {
        let mut node = registry.definition("math.number").unwrap().instantiate();
        node.properties.insert("value".into(), i.into());
        commands.push(Command::AddNode {
            node,
            rect: Rect {
                x: 40. + (i % 3) as f32 * 240.,
                y: 40. + (i / 3) as f32 * 140.,
                ..Rect::default()
            },
        });
    }
    editor.execute(Command::Batch { commands }).unwrap();
    editor.document().clone()
}
fn redraw(app: &tauri::AppHandle) {
    let Some(engine) = app.try_state::<Engine>() else {
        return;
    };
    if let Some(canvas) = app.get_window("canvas")
        && let Ok(size) = canvas.inner_size()
        && let Err(error) = engine.draw("controls", [size.width, size.height])
    {
        eprintln!("{}: {}", error.code, error.message);
    }
}
fn main() {
    tauri::Builder::default()
        .invoke_handler(unge_tauri::handler())
        .setup(|app| {
            let engine = Engine::new(initial_document())?;
            engine
                .register_view(
                    "controls",
                    Viewport {
                        origin: [0., 0.],
                        zoom: 1.,
                        size: [960., 640.],
                    },
                )
                .map_err(|e| e.message)?;
            // A native window without a WebView is the portable baseline surface.
            let canvas = tauri::WindowBuilder::new(app, "canvas")
                .title("UNGE · Rust / wgpu")
                .inner_size(960., 640.)
                .build()?;
            let size = canvas.inner_size()?;
            let renderer = pollster::block_on(SurfaceRenderer::new(
                Arc::new(canvas.clone()),
                [size.width, size.height],
            ))?;
            engine
                .attach_renderer("controls", renderer)
                .map_err(|e| e.message)?;
            app.manage(engine);
            redraw(app.handle());
            let handle = app.handle().clone();
            app.listen("unge://changed", move |_| {
                let main = handle.clone();
                let _ = handle.run_on_main_thread(move || redraw(&main));
            });
            if std::env::args().any(|arg| arg == "--acx-stdio") {
                let engine = app.state::<Engine>().inner().clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let mut provider = unge_acx::Provider::new(
                        Arc::new(engine.clone()),
                        Arc::new(unge_executor::math_registry()),
                        unge_acx::Policy::math_demo(),
                    );
                    let result = unge_acx::serve(
                        &mut provider,
                        std::io::stdin().lock(),
                        std::io::stdout().lock(),
                        || {
                            if let Ok(summary) =
                                engine.dispatch("controls", unge_tauri::Request::Summary)
                            {
                                let _ = handle.emit("unge://changed", summary);
                            }
                        },
                    );
                    if let Err(error) = result {
                        eprintln!("ACX transport: {error}");
                    }
                    // In pipe mode the parent owns the example process lifetime.
                    handle.exit(0);
                });
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "canvas"
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_)
                        | tauri::WindowEvent::ScaleFactorChanged { .. }
                        | tauri::WindowEvent::Focused(true)
                )
            {
                redraw(window.app_handle());
            }
            if window.label() == "controls" && matches!(event, tauri::WindowEvent::Destroyed) {
                let _ = window.state::<Engine>().remove_view("controls");
                if let Some(canvas) = window.app_handle().get_window("canvas") {
                    let _ = canvas.close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Tauri host failed");
}
