mod input;
mod panel_preferences;
mod property_demo;
mod render_benchmark;
mod workspace;
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};
use unge_core::*;
use unge_render::{LabelCatalog, LabelText, NodeLabels, SurfaceRenderer};
use unge_tauri::Engine;

fn labels(registry: &unge_executor::Registry) -> LabelCatalog {
    let mut catalog = LabelCatalog::new();
    for type_id in ["math.number", "math.add", "example.adjust"] {
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
                symbol: match type_id {
                    "math.number" => "#",
                    "example.adjust" => "ƒ",
                    _ => "+",
                }
                .into(),
                caption: LabelText {
                    en: definition.description.en.clone(),
                    ja: definition.description.ja.clone(),
                    zh_cn: definition.description.zh_cn.clone(),
                },
                tone: if type_id == "math.number" {
                    unge_render::NodeTone::Mint
                } else if type_id == "example.adjust" {
                    unge_render::NodeTone::Violet
                } else {
                    unge_render::NodeTone::Blue
                },
                title: LabelText {
                    en: title.en.clone(),
                    ja: title.ja.clone(),
                    zh_cn: title.zh_cn.clone(),
                },
                inputs: [
                    ("value".into(), value.clone()),
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
fn initial_document(registry: &unge_executor::Registry) -> Document {
    let mut editor = Editor::new(Document::default(), 100).unwrap();
    let mut commands = Vec::new();
    let mut ids = Vec::new();
    // A connected composition leaves room to read names and follow each cable.
    let positions = [
        (64., 104.),
        (64., 324.),
        (380., 434.),
        (380., 170.),
        (696., 270.),
        (1012., 270.),
    ];
    for (i, (x, y)) in positions.into_iter().enumerate() {
        let mut node = registry
            .definition(if i == 5 {
                "example.adjust"
            } else if i >= 3 {
                "math.add"
            } else {
                "math.number"
            })
            .unwrap()
            .instantiate();
        if i < 3 {
            node.properties
                .insert("value".into(), [20, 22, 8][i].into());
        }
        ids.push(node.id);
        commands.push(Command::AddNode {
            node,
            rect: Rect {
                x,
                y,
                width: 224.,
                height: 128.,
            },
        });
    }
    for (source, target, port) in [
        (0, 3, "a"),
        (1, 3, "b"),
        (3, 4, "a"),
        (2, 4, "b"),
        (4, 5, "value"),
    ] {
        commands.push(Command::Connect {
            edge: Edge {
                id: Id::new_v4(),
                from: Endpoint {
                    node: ids[source],
                    port: "value".into(),
                },
                to: Endpoint {
                    node: ids[target],
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
            label: "20 + 22 = 42".into(),
            nodes: [ids[0], ids[1], ids[3]].into(),
        }),
    });
    editor.execute(Command::Batch { commands }).unwrap();
    editor.document().clone()
}
fn redraw(app: &tauri::AppHandle) {
    let Some(engine) = app.try_state::<Engine>() else {
        return;
    };
    let composition = app.state::<workspace::Composition>();
    if composition.is_transitioning() {
        return;
    }
    if let Ok(appearance) = engine.appearance("controls") {
        let native = match appearance.theme {
            unge_render::Theme::Dark => tauri::Theme::Dark,
            unge_render::Theme::Light => tauri::Theme::Light,
        };
        if let Some(canvas) = app.get_window(composition.canvas_label())
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
    if let Some(panel) = app.get_window("inspector")
        && let Ok(view) = engine.view_state("controls")
        && panel.title().ok().as_deref() != Some(workspace::panel_title(view.locale))
    {
        let _ = panel.set_title(workspace::panel_title(view.locale));
    }
    if let Some(canvas) = app.get_window(composition.canvas_label())
        && let Ok(size) = canvas.inner_size()
    {
        let physical = [size.width, size.height];
        let scale = canvas.scale_factor().unwrap_or(1.0);
        if let Err(error) = composition.resize_panel(app, physical, scale) {
            eprintln!("panel layout: {error}");
        }
        if let Err(error) = engine.draw_scaled_region(
            "controls",
            physical,
            composition.layout(physical, scale).graph,
            scale,
        ) {
            eprintln!("{}: {}", error.code, error.message);
        }
    }
}
fn main() {
    let mut context = tauri::generate_context!();
    if std::env::args().any(|arg| arg == "--benchmark-surface") {
        render_benchmark::run(context);
        return;
    }
    context.config_mut().app.windows.clear();
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            unge_tauri::dispatch,
            unge_tauri::inspect,
            unge_tauri::node_properties,
            unge_tauri::selection_summary,
            unge_tauri::appearance,
            unge_tauri::accessible_nodes,
            unge_tauri::groups,
            unge_tauri::start_execution,
            unge_tauri::current_execution,
            unge_tauri::execution_status,
            unge_tauri::cancel_execution,
            workspace::panel_layout,
            workspace::set_panel_floating,
        ])
        .setup(|app| {
            let registry = Arc::new(property_demo::registry());
            let execution = unge_executor::RunService::new(
                registry.clone(),
                unge_executor::Scheduler::new(4, 128),
                unge_executor::RunLimits::default(),
            );
            let engine = Engine::from_registry(initial_document(&registry), 256, registry.clone())?
                .with_execution(execution.clone());
            engine
                .set_labels(labels(&registry))
                .map_err(|e| e.message)?;
            engine
                .register_view(
                    "controls",
                    Viewport {
                        origin: [0., 0.],
                        zoom: 1.,
                        size: [1280., 640.],
                    },
                )
                .map_err(|e| e.message)?;
            let unified = cfg!(target_os = "macos")
                && !std::env::args().any(|arg| arg == "--separate-windows");
            let composition = workspace::Composition::new(unified);
            let preferences_path = if unified {
                match app.path().app_config_dir() {
                    Ok(path) => Some(path.join("panel-layout.json")),
                    Err(error) => {
                        eprintln!("panel preferences path: {error}");
                        None
                    }
                }
            } else {
                None
            };
            app.manage(panel_preferences::Store::load(preferences_path));
            let canvas = tauri::WindowBuilder::new(app, composition.canvas_label())
                .title("UNGE")
                .inner_size(if unified { 1440. } else { 1280. }, 800.)
                .min_inner_size(720., 400.)
                .visible(false)
                .build()?;
            if unified
                && let Err(error) = app
                    .state::<panel_preferences::Store>()
                    .restore(&canvas, [720., 400.])
            {
                eprintln!("workspace restore: {error}");
            }
            app.manage(composition);
            let size = canvas.inner_size()?;
            let renderer = pollster::block_on(SurfaceRenderer::new(
                Arc::new(canvas.clone()),
                [size.width, size.height],
            ))?;
            engine
                .attach_renderer("controls", renderer)
                .map_err(|e| e.message)?;
            if unified {
                let composition = app.state::<workspace::Composition>();
                let layout = composition.layout([size.width, size.height], canvas.scale_factor()?);
                canvas.add_child(
                    tauri::webview::WebviewBuilder::new(
                        "controls",
                        tauri::WebviewUrl::App("index.html".into()),
                    ),
                    tauri::PhysicalPosition::new(layout.graph[0] as i32, 0),
                    tauri::PhysicalSize::new(layout.panel[0], layout.panel[1]),
                )?;
            } else {
                tauri::WebviewWindowBuilder::new(
                    app,
                    "controls",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("UNGE")
                .inner_size(380., 720.)
                .min_inner_size(320., 400.)
                .build()?;
            }
            app.wry_plugin(input::InputBridge {
                app: app.handle().clone(),
                engine: engine.clone(),
            });
            app.manage(engine);
            redraw(app.handle());
            canvas.show()?;
            let restore = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || panel_preferences::initialize(&restore));
            let handle = app.handle().clone();
            app.listen("unge://changed", move |_| {
                let main = handle.clone();
                let _ = handle.run_on_main_thread(move || redraw(&main));
            });
            if std::env::args().any(|arg| arg == "--acx-stdio") {
                let engine = app.state::<Engine>().inner().clone();
                let handle = app.handle().clone();
                let controls = app
                    .get_webview("controls")
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
            let Some(composition) = window.app_handle().try_state::<workspace::Composition>()
            else {
                return;
            };
            if matches!(
                event,
                tauri::WindowEvent::Moved(_)
                    | tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
                    | tauri::WindowEvent::CloseRequested { .. }
            ) {
                window.state::<panel_preferences::Store>().capture(window);
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "inspector" {
                    api.prevent_close();
                    if !composition.is_transitioning() && composition.is_floating() {
                        let app = window.app_handle().clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            if let Err(error) = workspace::move_panel(&app, false) {
                                eprintln!("{}: {}", error.code, error.message);
                            }
                        });
                    }
                } else if window.label() == "workspace" && composition.is_transitioning() {
                    api.prevent_close();
                }
            }
            if (window.label() == composition.canvas_label() || window.label() == "inspector")
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_)
                        | tauri::WindowEvent::ScaleFactorChanged { .. }
                        | tauri::WindowEvent::Focused(true)
                )
            {
                redraw(window.app_handle());
            }
            if (window.label() == "controls" || window.label() == "workspace")
                && matches!(event, tauri::WindowEvent::Destroyed)
            {
                if let Some(inspector) = window.app_handle().get_window("inspector") {
                    let _ = inspector.destroy();
                }
                let _ = window.state::<Engine>().remove_view("controls");
                if let Some(canvas) = window.app_handle().get_window("canvas") {
                    let _ = canvas.close();
                }
            }
        })
        .build(context)
        .expect("Tauri host failed")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                // All geometry is already in Rust memory, even after window destruction.
                if let Some(store) = app.try_state::<panel_preferences::Store>() {
                    store.save();
                }
            }
        });
}
