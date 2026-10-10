//! Normal queued rendering, one synthetic input/frame per native event-loop turn.
//! No per-frame GPU wait, timestamp query, readback, WebView or IPC.
#[path = "animation_workload.rs"]
mod workload;
use super::render_benchmark::{labels, large_graph, stats};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tauri::EventLoopMessage;
use tauri_runtime_wry::{
    Context, EventLoopIterationContext, Message, Plugin, PluginBuilder, WebContextStore,
    tao::{
        event::Event,
        event_loop::{ControlFlow, EventLoopProxy, EventLoopWindowTarget},
    },
};
use unge_core::{Document, Locale};
use unge_render::{CpuSurfaceFrameTimings, LabelCatalog, SurfaceRenderer, Theme};
use workload::{Motion, Workload};

const WARMUP: usize = 5;
const SAMPLES: usize = 60;
const MAX_ATTEMPTS: usize = WARMUP + SAMPLES + 10;

#[derive(serde::Serialize)]
struct Sample {
    step: usize,
    interval_ms: f64,
    update_ms: f64,
    scene_ms: f64,
    surface: CpuSurfaceFrameTimings,
    total_ms: f64,
    visible_nodes: usize,
    visible_edges: usize,
    quads: usize,
    labels: usize,
    zoom: f32,
}

struct Session {
    renderer: SurfaceRenderer,
    fixture: Document,
    workload: Workload,
    labels: LabelCatalog,
    physical: [u32; 2],
    scale: f64,
    case: usize,
    attempt: usize,
    successful: usize,
    previous_start: Option<Instant>,
    samples: Vec<Sample>,
    reports: Vec<serde_json::Value>,
    deadline: Instant,
}
impl Session {
    fn new(window: tauri::Window) -> Result<Self, String> {
        let fixture = large_graph::document();
        let size = window.inner_size().map_err(|e| e.to_string())?;
        let scale = window.scale_factor().map_err(|e| e.to_string())?;
        let physical = [size.width, size.height];
        let workload = Workload::new(
            fixture.clone(),
            physical.map(|n| (f64::from(n) / scale) as f32),
        )?;
        let mut renderer = pollster::block_on(SurfaceRenderer::new(Arc::new(window), physical))?;
        renderer
            .device
            .push_error_scope(unge_render::wgpu::ErrorFilter::Validation);
        // Verify skip/error semantics against a real Surface before collecting.
        let scene = workload
            .index
            .scene(workload.viewport, &workload.selection)
            .map_err(|e| e.to_string())?;
        renderer.resize([0, 0])?;
        if renderer.draw_timed(&scene, workload.viewport)?.is_some()
            || renderer.draw(&scene, workload.viewport)?
        {
            return Err("suspended Surface returned a timing sample".into());
        }
        renderer.resize(physical)?;
        if renderer
            .draw_region_timed(&scene, workload.viewport, [0, physical[1]])?
            .is_some()
        {
            return Err("empty graph region returned a timing sample".into());
        }
        if renderer
            .draw_region_timed(&scene, workload.viewport, [physical[0] + 1, physical[1]])
            .is_ok()
        {
            return Err("oversized region was accepted".into());
        }
        Ok(Self {
            renderer,
            fixture,
            workload,
            labels: labels(),
            physical,
            scale,
            case: 0,
            attempt: 0,
            successful: 0,
            previous_start: None,
            samples: Vec::with_capacity(SAMPLES),
            reports: Vec::with_capacity(18),
            deadline: Instant::now() + Duration::from_secs(180),
        })
    }
    fn tick(&mut self) -> Result<Option<serde_json::Value>, String> {
        if Instant::now() > self.deadline || self.attempt >= MAX_ATTEMPTS {
            return Err("animation benchmark exceeded time or Surface retry limit".into());
        }
        let theme = [Theme::Dark, Theme::Light][self.case / 9];
        let locale = [Locale::En, Locale::Ja, Locale::ZhCn][self.case / 3 % 3];
        let motion = [Motion::Pan, Motion::Zoom, Motion::Drag][self.case % 3];
        let step = self.attempt;
        self.attempt += 1;
        let start = Instant::now();
        self.workload.advance(motion, step)?;
        let update_ms = start.elapsed().as_secs_f64() * 1000.;
        let scene_start = Instant::now();
        let scene = self
            .workload
            .index
            .scene_with_theme(
                self.workload.viewport,
                &self.workload.selection,
                self.workload.interaction.preview(),
                &self.labels,
                locale,
                theme,
            )
            .map_err(|e| e.to_string())?;
        let scene_ms = scene_start.elapsed().as_secs_f64() * 1000.;
        let Some(surface) = self.renderer.draw_timed(&scene, self.workload.viewport)? else {
            // Keep the previous successful start: the next interval includes
            // event-loop delays and skipped attempts instead of hiding them.
            return Ok(None);
        };
        let total_ms = start.elapsed().as_secs_f64() * 1000.;
        let interval = self.previous_start.replace(start);
        self.successful += 1;
        if self.renderer.text_stats().missing_glyphs != 0 {
            return Err("missing animation benchmark glyphs".into());
        }
        if self.successful > WARMUP {
            self.samples.push(Sample {
                step,
                interval_ms: start
                    .duration_since(interval.expect("warmup precedes samples"))
                    .as_secs_f64()
                    * 1000.,
                update_ms,
                scene_ms,
                surface,
                total_ms,
                visible_nodes: scene.visible_nodes,
                visible_edges: scene.visible_edges,
                quads: scene.quads.len(),
                labels: scene.labels.len(),
                zoom: self.workload.viewport.zoom,
            });
        }
        if self.samples.len() != SAMPLES {
            return Ok(None);
        }
        let interval_sum: f64 = self.samples.iter().map(|s| s.interval_ms).sum();
        self.reports.push(serde_json::json!({
            "theme":theme,"locale":locale,"motion":motion,
            "attempts":self.attempt,"skipped":self.attempt-self.successful,
            "committed_edits":self.workload.editor.revision(),
            "interval":stats(self.samples.iter().map(|s|s.interval_ms).collect()),
            "update":stats(self.samples.iter().map(|s|s.update_ms).collect()),
            "scene":stats(self.samples.iter().map(|s|s.scene_ms).collect()),
            "acquire":stats(self.samples.iter().map(|s|s.surface.acquire_ms).collect()),
            "prepare":stats(self.samples.iter().map(|s|s.surface.prepare_ms).collect()),
            "encode_submit":stats(self.samples.iter().map(|s|s.surface.encode_submit_ms).collect()),
            "present_call":stats(self.samples.iter().map(|s|s.surface.present_call_ms).collect()),
            "total":stats(self.samples.iter().map(|s|s.total_ms).collect()),
            "start_rate_hz":1000.*SAMPLES as f64/interval_sum,
            "intervals_over_16_667_ms":self.samples.iter().filter(|s|s.interval_ms>1000./60.).count(),
            "raw":self.samples,
        }));
        self.case += 1;
        if self.case == 18 {
            if let Some(error) = pollster::block_on(self.renderer.device.pop_error_scope()) {
                return Err(error.to_string());
            }
            let adapter = self.renderer.adapter_info();
            return Ok(Some(serde_json::json!({
                "version":1,"scope":"Event-loop CPU submission cadence; no explicit GPU completion wait; not display FPS or input-to-display latency",
                "adapter":adapter.name,"backend":format!("{:?}",adapter.backend),
                "present_mode":format!("{:?}",self.renderer.present_mode()),
                "os":std::env::consts::OS,"arch":std::env::consts::ARCH,
                "physical_size":self.physical,"scale_factor":self.scale,
                "nodes":self.fixture.graph().nodes().len(),"edges":self.fixture.graph().edges().len(),
                "warmup":WARMUP,"samples_per_case":SAMPLES,"max_attempts_per_case":MAX_ATTEMPTS,
                "surface_contract_checks":"suspended skips; oversized region rejects; subsequent rendering recovers",
                "cases":self.reports,
            })));
        }
        // Reset each case outside its measured span; do not mix that gap into
        // the next case. Renderer/font caches remain warm across cases.
        self.workload = Workload::new(self.fixture.clone(), self.workload.viewport.size)?;
        self.attempt = 0;
        self.successful = 0;
        self.previous_start = None;
        self.samples.clear();
        Ok(None)
    }
}

struct Bridge {
    app: tauri::AppHandle,
    window: tauri::Window,
}
struct Animation {
    app: tauri::AppHandle,
    session: Result<Session, String>,
    finished: bool,
}
impl PluginBuilder<EventLoopMessage> for Bridge {
    type Plugin = Animation;
    fn build(self, _: Context<EventLoopMessage>) -> Animation {
        Animation {
            app: self.app,
            session: Session::new(self.window),
            finished: false,
        }
    }
}
impl Plugin<EventLoopMessage> for Animation {
    fn on_event(
        &mut self,
        event: &Event<Message<EventLoopMessage>>,
        _: &EventLoopWindowTarget<Message<EventLoopMessage>>,
        proxy: &EventLoopProxy<Message<EventLoopMessage>>,
        _: &mut ControlFlow,
        _: EventLoopIterationContext<'_, EventLoopMessage>,
        _: &WebContextStore,
    ) -> bool {
        if self.finished || !matches!(event, Event::MainEventsCleared) {
            return false;
        }
        let result = match &mut self.session {
            Ok(session) => session.tick(),
            Err(error) => Err(error.clone()),
        };
        match result {
            Ok(Some(report)) => {
                self.finished = true;
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
                self.app.exit(0);
            }
            Err(error) => {
                self.finished = true;
                eprintln!("animation benchmark: {error}");
                self.app.exit(1);
            }
            Ok(None) => {
                // Wry resets ControlFlow to Wait after plugin callbacks. Queue
                // one wake event so the next native turn runs without overriding
                // normal lifecycle handling or accumulating background work.
                if proxy.send_event(Message::Task(Box::new(|| {}))).is_err() {
                    self.finished = true;
                    eprintln!("animation benchmark: event loop closed");
                    self.app.exit(1);
                }
            }
        }
        false
    }
}

pub fn run(mut context: tauri::Context<tauri::Wry>) {
    context.config_mut().app.windows.clear();
    tauri::Builder::default()
        .setup(|app| {
            let window = tauri::WindowBuilder::new(app, "animation-benchmark")
                .title("UNGE · Animation / 連続操作 / 连续操作")
                .inner_size(1280., 720.)
                .resizable(false)
                .build()?;
            app.wry_plugin(Bridge {
                app: app.handle().clone(),
                window,
            });
            Ok(())
        })
        .run(context)
        .expect("animation benchmark app");
}
