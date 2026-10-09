//! Standalone native diagnostic. No WebView, editing, execution or IPC traffic.
#[path = "../../../crates/unge-render/examples/support/large_graph.rs"]
mod large_graph;
use std::{collections::BTreeSet, sync::Arc, time::Instant};
use unge_core::{Locale, Viewport};
use unge_render::{
    LabelCatalog, LabelText, NodeLabels, NodeTone, SceneIndex, SurfaceRenderer, Theme,
};

fn stats(mut values: Vec<f64>) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    values.sort_by(f64::total_cmp);
    serde_json::json!({"samples":values.len(),"median_ms":values[values.len()/2],"p95_ms":values[(values.len()*95).div_ceil(100)-1],"min_ms":values[0],"max_ms":values[values.len()-1]})
}
fn measure(window: tauri::Window) -> Result<serde_json::Value, String> {
    let document = large_graph::document();
    let index = SceneIndex::new(&document);
    let labels = LabelCatalog::from([(
        "benchmark".into(),
        NodeLabels {
            tone: NodeTone::Blue,
            symbol: "#".into(),
            title: LabelText {
                en: "Value".into(),
                ja: "数値".into(),
                zh_cn: "数值".into(),
            },
            ..Default::default()
        },
    )]);
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let physical = [size.width, size.height];
    let mut renderer =
        pollster::block_on(SurfaceRenderer::new_profiled(Arc::new(window), physical))?;
    let adapter = renderer.adapter_info().clone();
    let mode = format!("{:?}", renderer.present_mode());
    renderer
        .device
        .push_error_scope(unge_render::wgpu::ErrorFilter::Validation);
    let mut cases = Vec::new();
    for theme in [Theme::Dark, Theme::Light] {
        for locale in [Locale::En, Locale::Ja, Locale::ZhCn] {
            for (name, zoom) in [("near", 1.), ("overview", 0.02)] {
                let viewport = Viewport {
                    origin: [0., 0.],
                    zoom,
                    size: physical.map(|n| (f64::from(n) / scale) as f32),
                };
                let mut samples = Vec::new();
                let mut scene_ms = Vec::new();
                let mut total_ms = Vec::new();
                let mut skipped = 0;
                let mut successful = 0;
                let mut cold = None;
                let mut counts = serde_json::Value::Null;
                // Five successful warm-up frames; at most ten skipped attempts.
                for _ in 0..45 {
                    let total = Instant::now();
                    let scene = index
                        .scene_with_theme(
                            viewport,
                            &BTreeSet::new(),
                            &Default::default(),
                            &labels,
                            locale,
                            theme,
                        )
                        .map_err(|e| e.to_string())?;
                    let scene_time = total.elapsed().as_secs_f64() * 1000.;
                    let Some(timing) = renderer.draw_profiled(&scene, viewport)? else {
                        skipped += 1;
                        continue;
                    };
                    let elapsed = total.elapsed().as_secs_f64() * 1000.;
                    if cold.is_none() {
                        cold = Some(
                            serde_json::json!({"scene_ms":scene_time,"surface":timing,"total_ms":elapsed}),
                        );
                    }
                    successful += 1;
                    if successful > 5 {
                        counts = serde_json::json!({"nodes":scene.visible_nodes,"edges":scene.visible_edges,"quads":scene.quads.len(),"labels":scene.labels.len()});
                        scene_ms.push(scene_time);
                        total_ms.push(elapsed);
                        samples.push(timing);
                    }
                    if samples.len() == 30 {
                        break;
                    }
                }
                if samples.len() != 30 {
                    return Err(format!(
                        "insufficient successful frames: {name} {successful}, skipped {skipped}"
                    ));
                }
                if renderer.text_stats().missing_glyphs != 0 {
                    return Err("missing benchmark glyphs".into());
                }
                cases.push(serde_json::json!({
                    "theme":theme,"locale":locale,"view":name,"zoom":zoom,"counts":counts,"skipped":skipped,"first_frame":cold,
                    "scene":stats(scene_ms),"acquire":stats(samples.iter().map(|t|t.acquire_ms).collect()),
                    "prepare":stats(samples.iter().map(|t|t.frame.prepare_ms).collect()),
                    "encode_submit":stats(samples.iter().map(|t|t.frame.encode_submit_ms).collect()),
                    "completion_wait":stats(samples.iter().map(|t|t.frame.completion_wait_ms).collect()),
                    "gpu_pass":stats(samples.iter().filter_map(|t|t.frame.gpu_pass_ms).collect()),
                    "present_call":stats(samples.iter().map(|t|t.present_call_ms).collect()),"total":stats(total_ms),
                    "gpu_pass_unavailable":samples.iter().filter(|t|t.frame.gpu_pass_ms.is_none()).count(),
                    "raw":samples,
                }));
            }
        }
    }
    if let Some(error) = pollster::block_on(renderer.device.pop_error_scope()) {
        return Err(error.to_string());
    }
    Ok(
        serde_json::json!({"version":1,"scope":"Serialized native diagnostic; not animation FPS or display scanout latency",
        "adapter":adapter.name,"backend":format!("{:?}",adapter.backend),"present_mode":mode,
        "os":std::env::consts::OS,"arch":std::env::consts::ARCH,"physical_size":physical,"scale_factor":scale,
        "nodes":document.graph().nodes().len(),"edges":document.graph().edges().len(),"warmup":5,"samples_per_case":30,"cases":cases}),
    )
}
pub fn run(mut context: tauri::Context<tauri::Wry>) {
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .build(context)
        .expect("benchmark app");
    app.run(|app, event| {
        if let tauri::RunEvent::Ready = event {
            let result = tauri::WindowBuilder::new(app, "benchmark")
                .title("UNGE · GPU / Surface")
                .inner_size(1280., 720.)
                .resizable(false)
                .build()
                .map_err(|e| e.to_string())
                .and_then(measure);
            match result {
                Ok(report) => {
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                    app.exit(0);
                }
                Err(error) => {
                    eprintln!("surface benchmark: {error}");
                    app.exit(1);
                }
            }
        }
    });
}
