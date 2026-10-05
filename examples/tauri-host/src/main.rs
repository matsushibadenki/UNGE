mod input;
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};
use unge_core::*;
use unge_render::{LabelCatalog, LabelText, NodeLabels, SurfaceRenderer};
use unge_tauri::Engine;

fn labels(registry: &unge_executor::Registry) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for type_id in ["math.number", "math.add"] {
        let definition = registry.definition(type_id).unwrap();
        let title = &definition.name;
        let value = LabelText {
            en: "Value".into(),
            ja: "値".into(),
            zh_cn: "数值".into(),
        };
        catalog.insert(
            type_id.into(),
            NodeLabels {
                title: LabelText {
                    en: title.en.clone(),
                    ja: title.ja.clone(),
                    zh_cn: title.zh_cn.clone(),
                },
                inputs: [
                    (
                        "a".into(),
                        LabelText {
                            en: "A".into(),
                            ja: "入力 A".into(),
                            zh_cn: "输入 A".into(),
                        },
                    ),
                    (
                        "b".into(),
                        LabelText {
                            en: "B".into(),
                            ja: "入力 B".into(),
                            zh_cn: "输入 B".into(),
                        },
                    ),
                ]
                .into(),
                outputs: [("value".into(), value)].into(),
            },
        );
    }
    catalog
}
fn initial_document() -> Document {
    let registry = unge_executor::math_registry();
    let mut editor = Editor::new(Document::default(), 100).unwrap();
    let mut commands = Vec::new();
    let mut ids = Vec::new();
    for i in 0..12 {
        let mut node = registry
            .definition(if i == 11 { "math.add" } else { "math.number" })
            .unwrap()
            .instantiate();
        if i != 11 {
            node.properties.insert("value".into(), i.into());
        }
        ids.push(node.id);
        commands.push(Command::AddNode {
            node,
            rect: Rect {
                x: 40. + (i % 3) as f32 * 240.,
                y: 40. + (i / 3) as f32 * 140.,
                ..Rect::default()
            },
        });
    }
    for (source, port) in [(9, "a"), (10, "b")] {
        commands.push(Command::Connect {
            edge: Edge {
                id: Id::new_v4(),
                from: Endpoint {
                    node: ids[source],
                    port: "value".into(),
                },
                to: Endpoint {
                    node: ids[11],
                    port: port.into(),
                },
            },
        });
    }
    let group_id = Id::new_v4();
    commands.push(Command::SetGroup {
        id: group_id,
        group: Some(unge_core::Group {
            id: group_id,
            label: "9 + 10 = 19".into(),
            nodes: ids[9..12].iter().copied().collect(),
        }),
    });
    editor.execute(Command::Batch { commands }).unwrap();
    editor.document().clone()
}
fn redraw(app: &tauri::AppHandle) {
    let Some(engine) = app.try_state::<Engine>() else {
        return;
    };
    if let Ok(appearance) = engine.appearance("controls") {
        let native = match appearance.theme {
            unge_render::Theme::Dark => tauri::Theme::Dark,
            unge_render::Theme::Light => tauri::Theme::Light,
        };
        if let Some(canvas) = app.get_window("canvas")
            && canvas.theme().ok() != Some(native)
            && let Err(error) = canvas.set_theme(Some(native))
        {
            eprintln!("native theme: {error}");
        }
        if let Some(controls) = app.get_webview_window("controls")
            && controls.theme().ok() != Some(native)
            && let Err(error) = controls.set_theme(Some(native))
        {
            eprintln!("control theme: {error}");
        }
    }
    if let Some(canvas) = app.get_window("canvas")
        && let Ok(size) = canvas.inner_size()
        && let Err(error) = engine.draw_scaled(
            "controls",
            [size.width, size.height],
            canvas.scale_factor().unwrap_or(1.0),
        )
    {
        eprintln!("{}: {}", error.code, error.message);
    }
}
fn main() {
    tauri::Builder::default()
        .invoke_handler(unge_tauri::handler())
        .setup(|app| {
            let registry = Arc::new(unge_executor::math_registry());
            let editor = Editor::new(initial_document(), 256)?.with_validator(registry.clone())?;
            let execution = unge_executor::RunService::new(
                registry.clone(),
                unge_executor::Scheduler::new(4, 128),
                unge_executor::RunLimits::default(),
            );
            let engine = Engine::from_editor(editor).with_execution(execution.clone());
            engine
                .set_labels(labels(&registry))
                .map_err(|e| e.message)?;
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
            app.wry_plugin(input::InputBridge {
                app: app.handle().clone(),
                engine: engine.clone(),
            });
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
                let controls = app
                    .get_webview_window("controls")
                    .ok_or("controls window missing")?;
                std::thread::spawn(move || {
                    let mut provider = unge_acx::Provider::new(
                        Arc::new(engine.clone()),
                        registry,
                        unge_acx::Policy::math_demo(),
                    )
                    .with_execution(execution, unge_tauri::execution_observer(controls))
                    .expect("same host registry");
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
