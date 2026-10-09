//! Small Tauri 2 boundary. Document, views, indexes and native renderers live in Rust.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use unge_core::*;
use unge_executor::{PreparedRun, RunError, RunService, RunSummary};
use unge_interaction::{Interaction, InteractionError, PointerEvent};
use unge_render::{LabelCatalog, SceneIndex, SurfaceRenderer, Theme};

mod properties;
pub use properties::NodeProperties;

mod groups;
pub use groups::{GroupAction, GroupPage, GroupSummary};

struct ViewState {
    theme: Theme,
    locale: Locale,
    viewport: Viewport,
    selection: BTreeSet<Id>,
    interaction: Interaction,
}
struct State {
    labels: LabelCatalog,
    editor: Editor,
    scene: SceneIndex,
    views: BTreeMap<String, ViewState>,
}
/// Manage one Engine per application document service. No window owns a document.
#[derive(Clone)]
pub struct Engine {
    state: Arc<Mutex<State>>,
    execution: Option<RunService>,
    registry: Option<Arc<unge_executor::Registry>>,
    renderers: Arc<Mutex<BTreeMap<String, SurfaceRenderer>>>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub revision: u64,
    pub nodes: usize,
    pub edges: usize,
}
/// Constant-size selection metadata for the calling view, including off-page nodes.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionSummary {
    pub revision: u64,
    pub count: usize,
    /// Present only when exactly one node is selected. No arbitrary multi-selection target.
    pub single: Option<Id>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}
impl ApiError {
    fn new(code: &str, message: impl ToString) -> Self {
        Self {
            code: code.into(),
            message: message.to_string(),
        }
    }
}
impl From<RunError> for ApiError {
    fn from(error: RunError) -> Self {
        Self::new(&error.code, error.message)
    }
}
impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        let code = if matches!(&error, Error::Properties(_)) {
            "invalid_properties"
        } else {
            "invalid_command"
        };
        Self::new(code, error)
    }
}
impl From<InteractionError> for ApiError {
    fn from(error: InteractionError) -> Self {
        let code = match error {
            InteractionError::Conflict => "revision_conflict",
            InteractionError::PointerBusy => "pointer_busy",
            InteractionError::Invalid => "invalid_pointer",
            InteractionError::Connection => "invalid_connection",
        };
        Self::new(code, error)
    }
}
#[derive(Debug, Clone)]
pub struct ViewSnapshot {
    pub viewport: Viewport,
    pub selection: BTreeSet<Id>,
    pub interacting: bool,
    pub locale: Locale,
    pub theme: Theme,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Appearance {
    pub theme: Theme,
    pub colors: BTreeMap<String, String>,
}
/// Bounded semantic metadata for a keyboard/screen-reader companion to a GPU view.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibleNode {
    pub id: Id,
    pub title: String,
    pub rect: Rect,
    pub selected: bool,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessiblePage {
    pub revision: u64,
    pub total: usize,
    pub nodes: Vec<AccessibleNode>,
    pub next: Option<Id>,
}
type ApiResult<T> = std::result::Result<T, ApiError>;
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Group {
        expected_revision: u64,
        action: GroupAction,
    },
    SetTheme {
        theme: Theme,
    },
    SetLocale {
        locale: Locale,
    },
    Pointer {
        expected_revision: u64,
        event: PointerEvent,
    },
    Apply {
        expected_revision: u64,
        command: Command,
    },
    Undo {
        expected_revision: u64,
    },
    Redo {
        expected_revision: u64,
    },
    SetViewport {
        viewport: Viewport,
    },
    Select {
        ids: BTreeSet<Id>,
    },
    Summary,
}
fn summary(state: &State) -> Summary {
    Summary {
        revision: state.editor.revision(),
        nodes: state.editor.document().graph().nodes().len(),
        edges: state.editor.document().graph().edges().len(),
    }
}
fn refresh_scene(state: &mut State, changes: Option<&unge_render::SceneChanges>) {
    if let Some(changes) = changes {
        state.scene.update(state.editor.document(), changes);
    } else {
        state.scene = SceneIndex::new(state.editor.document());
    }
    let existing: BTreeSet<_> = state
        .editor
        .document()
        .graph()
        .nodes()
        .keys()
        .copied()
        .collect();
    for view in state.views.values_mut() {
        view.interaction.invalidate_preview();
        view.selection.retain(|id| existing.contains(id));
    }
}
fn check_command(command: &Command, depth: usize, budget: &mut usize) -> ApiResult<()> {
    if depth > 16 || *budget == 0 {
        return Err(ApiError::new(
            "limit_exceeded",
            "maximum command depth 16 / count 4096",
        ));
    }
    *budget -= 1;
    if let Command::Batch { commands } = command {
        for command in commands {
            check_command(command, depth + 1, budget)?;
        }
    }
    Ok(())
}
impl Engine {
    pub fn new(document: Document) -> Result<Self> {
        Ok(Self::from_editor(Editor::new(document, 256)?))
    }
    /// Reuse a host-configured validator and history budget for all UI/AI edits.
    pub fn from_editor(editor: Editor) -> Self {
        let scene = SceneIndex::new(editor.document());
        Self {
            execution: None,
            registry: None,
            state: Arc::new(Mutex::new(State {
                labels: LabelCatalog::new(),
                editor,
                scene,
                views: BTreeMap::new(),
            })),
            renderers: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
    /// Configure before sharing Engine clones. Registry/executors are trusted host code.
    pub fn with_execution(mut self, service: RunService) -> Self {
        self.execution = Some(service);
        self
    }
    pub fn prepare_run(&self, view: &str, expected_revision: u64) -> ApiResult<PreparedRun> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        if state.editor.revision() != expected_revision {
            return Err(ApiError::new("revision_conflict", state.editor.revision()));
        }
        let service = self.execution.as_ref().ok_or_else(|| {
            ApiError::new("execution_unavailable", "host did not configure execution")
        })?;
        Ok(service.prepare(state.editor.document(), expected_revision)?)
    }
    pub fn current_execution(&self, view: &str) -> ApiResult<Option<RunSummary>> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        let service = self.execution.as_ref().ok_or_else(|| {
            ApiError::new("execution_unavailable", "host did not configure execution")
        })?;
        let current = service.current()?;
        if current
            .as_ref()
            .is_some_and(|run| run.document_id != state.editor.document().graph().id)
        {
            return Err(ApiError::new(
                "document_mismatch",
                "run belongs to another document",
            ));
        }
        Ok(current)
    }
    pub fn execution_status(&self, view: &str, id: Id) -> ApiResult<RunSummary> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        let service = self.execution.as_ref().ok_or_else(|| {
            ApiError::new("execution_unavailable", "host did not configure execution")
        })?;
        let summary = service.inspect(id)?;
        if summary.document_id != state.editor.document().graph().id {
            return Err(ApiError::new(
                "document_mismatch",
                "run belongs to another document",
            ));
        }
        Ok(summary)
    }
    pub fn cancel_execution(&self, view: &str, id: Id) -> ApiResult<RunSummary> {
        self.execution_status(view, id)?;
        Ok(self
            .execution
            .as_ref()
            .expect("checked execution")
            .cancel(id)?)
    }
    /// Host presentation metadata is shared; each view selects its own language.
    pub fn set_labels(&self, labels: LabelCatalog) -> ApiResult<()> {
        self.state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .labels = labels;
        Ok(())
    }
    /// Called by trusted Rust host setup. WebViews cannot create arbitrary views.
    pub fn register_view(&self, label: impl Into<String>, viewport: Viewport) -> ApiResult<()> {
        viewport.validate()?;
        let mut state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        state.views.insert(
            label.into(),
            ViewState {
                theme: Theme::Dark,
                locale: Locale::En,
                viewport,
                selection: BTreeSet::new(),
                interaction: Interaction::default(),
            },
        );
        Ok(())
    }
    pub fn remove_view(&self, label: &str) -> ApiResult<()> {
        self.state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .views
            .remove(label);
        self.renderers
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .remove(label);
        Ok(())
    }
    /// Attach a native surface to a registered control view. Renderer ownership stays here.
    pub fn attach_renderer(&self, view: &str, renderer: SurfaceRenderer) -> ApiResult<()> {
        if !self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .views
            .contains_key(view)
        {
            return Err(ApiError::new("unknown_view", view));
        }
        self.renderers
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .insert(view.into(), renderer);
        Ok(())
    }
    /// Call on the host's render thread/main-thread callback after edits and resize.
    pub fn draw(&self, view: &str, physical_size: [u32; 2]) -> ApiResult<bool> {
        self.draw_scaled(view, physical_size, 1.0)
    }
    /// Surface size is physical; gesture positions and viewport.size are logical.
    pub fn draw_scaled(
        &self,
        view: &str,
        physical_size: [u32; 2],
        scale_factor: f64,
    ) -> ApiResult<bool> {
        self.draw_scaled_region(view, physical_size, physical_size, scale_factor)
    }
    /// Render into a top-left region of a larger native window. Region and
    /// Surface sizes are physical; pointer positions remain graph-local logical pixels.
    pub fn draw_scaled_region(
        &self,
        view: &str,
        physical_size: [u32; 2],
        graph_size: [u32; 2],
        scale_factor: f64,
    ) -> ApiResult<bool> {
        if graph_size[0] > physical_size[0] || graph_size[1] > physical_size[1] {
            return Err(ApiError::new(
                "invalid_pointer",
                "graph region exceeds surface size",
            ));
        }
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(ApiError::new("invalid_pointer", "invalid scale factor"));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let view_state = state
            .views
            .get_mut(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        let mut viewport = view_state.viewport;
        viewport.size = graph_size.map(|v| (f64::from(v.max(1)) / scale_factor) as f32);
        viewport.validate()?;
        if view_state.viewport.size != viewport.size {
            view_state
                .interaction
                .cancel(&mut view_state.viewport, &mut view_state.selection);
            // Preserve a finished pan; cancel an active gesture before changing metrics.
            viewport.origin = view_state.viewport.origin;
            viewport.zoom = view_state.viewport.zoom;
            view_state.viewport = viewport;
        }
        let view_state = &state.views[view];
        let scene = state.scene.scene_with_theme(
            viewport,
            &view_state.selection,
            view_state.interaction.preview(),
            &state.labels,
            view_state.locale,
            view_state.theme,
        )?;
        drop(state);
        let mut renderers = self
            .renderers
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let renderer = renderers
            .get_mut(view)
            .ok_or_else(|| ApiError::new("renderer_unavailable", view))?;
        renderer
            .resize(physical_size)
            .map_err(|e| ApiError::new("gpu_error", e))?;
        renderer
            .draw_region(&scene, viewport, graph_size)
            .map_err(|e| ApiError::new("gpu_error", e))
    }
    pub fn dispatch(&self, view: &str, request: Request) -> ApiResult<Summary> {
        // Transport is for controls/metadata. Large assets belong in Rust resource stores.
        if serde_json::to_vec(&request)
            .map_err(|e| ApiError::new("invalid_request", e))?
            .len()
            > 256 * 1024
        {
            return Err(ApiError::new("limit_exceeded", "request exceeds 256 KiB"));
        }
        if let Request::Apply { command, .. } = &request {
            check_command(command, 0, &mut 4096)?;
        }
        let mut state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        let expected = match &request {
            Request::Apply {
                expected_revision, ..
            }
            | Request::Pointer {
                expected_revision, ..
            }
            | Request::Group {
                expected_revision, ..
            }
            | Request::Undo { expected_revision }
            | Request::Redo { expected_revision } => Some(*expected_revision),
            _ => None,
        };
        if expected.is_some_and(|r| r != state.editor.revision()) {
            if matches!(request, Request::Pointer { .. }) {
                let v = state.views.get_mut(view).unwrap();
                v.interaction.cancel(&mut v.viewport, &mut v.selection);
                let existing: BTreeSet<_> = state
                    .editor
                    .document()
                    .graph()
                    .nodes()
                    .keys()
                    .copied()
                    .collect();
                state
                    .views
                    .get_mut(view)
                    .unwrap()
                    .selection
                    .retain(|id| existing.contains(id));
            }
            return Err(ApiError::new("revision_conflict", state.editor.revision()));
        }
        let before = state.editor.revision();
        let mut changes = None;
        match request {
            Request::Group { action, .. } => {
                changes = groups::apply_group(&mut state, view, action)?;
            }
            Request::SetTheme { theme } => {
                state.views.get_mut(view).unwrap().theme = theme;
            }
            Request::SetLocale { locale } => {
                state.views.get_mut(view).unwrap().locale = locale;
            }
            Request::Pointer { event, .. } => {
                let State {
                    editor,
                    scene,
                    views,
                    ..
                } = &mut *state;
                let v = views.get_mut(view).unwrap();
                let result = v.interaction.handle(
                    editor.document(),
                    editor.revision(),
                    scene.spatial_index(),
                    &mut v.viewport,
                    &mut v.selection,
                    event,
                );
                v.selection
                    .retain(|id| editor.document().graph().nodes().contains_key(id));
                if let Some(command) = result? {
                    changes = Some(unge_render::SceneChanges::from_command(&command));
                    editor.execute(command)?;
                }
            }
            Request::Apply { command, .. } => {
                changes = Some(unge_render::SceneChanges::from_command(&command));
                state.editor.execute(command)?;
            }
            Request::Undo { .. } => {
                changes = state
                    .editor
                    .undo_command()
                    .map(unge_render::SceneChanges::from_command);
                state.editor.undo()?;
            }
            Request::Redo { .. } => {
                changes = state
                    .editor
                    .redo_command()
                    .map(unge_render::SceneChanges::from_command);
                state.editor.redo()?;
            }
            Request::SetViewport { viewport } => {
                viewport.validate()?;
                let v = state.views.get_mut(view).unwrap();
                v.interaction.cancel(&mut v.viewport, &mut v.selection);
                v.viewport = viewport;
            }
            Request::Select { ids } => {
                if ids.len() > 10_000
                    || ids
                        .iter()
                        .any(|id| !state.editor.document().graph().nodes().contains_key(id))
                {
                    return Err(ApiError::new(
                        "invalid_selection",
                        "selection includes missing nodes or exceeds 10000",
                    ));
                }
                let v = state.views.get_mut(view).unwrap();
                v.interaction.cancel(&mut v.viewport, &mut v.selection);
                v.selection = ids;
            }
            Request::Summary => {}
        }
        if state.editor.revision() != before {
            refresh_scene(&mut state, changes.as_ref());
        }
        Ok(summary(&state))
    }
    /// Rust host metadata; contains no document clone or frame data.
    pub fn selection_summary(&self, view: &str) -> ApiResult<SelectionSummary> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        Ok(SelectionSummary {
            revision: state.editor.revision(),
            count: v.selection.len(),
            single: if v.selection.len() == 1 {
                v.selection.first().copied()
            } else {
                None
            },
        })
    }
    /// Rust host metadata; contains no document clone or frame data.
    pub fn view_state(&self, view: &str) -> ApiResult<ViewSnapshot> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        Ok(ViewSnapshot {
            viewport: v.viewport,
            selection: v.selection.clone(),
            interacting: v.interaction.is_active(),
            locale: v.locale,
            theme: v.theme,
        })
    }
    /// Read the authoritative view theme and its CSS palette, without a graph clone.
    pub fn appearance(&self, view: &str) -> ApiResult<Appearance> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        Ok(Appearance {
            theme: v.theme,
            colors: v.theme.palette().css_variables(),
        })
    }
    /// Stable ID order, exclusive cursor, at most 100 nodes. Reject stale pages.
    /// No properties, resources, document mirror or frame data cross IPC.
    pub fn accessible_nodes(
        &self,
        view: &str,
        expected_revision: u64,
        after: Option<Id>,
        limit: usize,
    ) -> ApiResult<AccessiblePage> {
        if !(1..=100).contains(&limit) {
            return Err(ApiError::new(
                "invalid_request",
                "page limit must be 1..100",
            ));
        }
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let v = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        if state.editor.revision() != expected_revision {
            return Err(ApiError::new(
                "revision_conflict",
                "semantic page revision changed",
            ));
        }
        let doc = state.editor.document();
        let mut iter = doc.graph().nodes().range((
            after.map_or(std::ops::Bound::Unbounded, std::ops::Bound::Excluded),
            std::ops::Bound::Unbounded,
        ));
        let nodes: Vec<_> = iter
            .by_ref()
            .take(limit)
            .map(|(id, node)| {
                let title = state
                    .labels
                    .get(&node.type_id)
                    .map(|labels| labels.title.get(v.locale))
                    .filter(|text| !text.is_empty())
                    .unwrap_or(&node.type_id);
                AccessibleNode {
                    id: *id,
                    title: title
                        .chars()
                        .take(256)
                        .map(|c| if c.is_control() { ' ' } else { c })
                        .collect(),
                    rect: doc.placement()[id],
                    selected: v.selection.contains(id),
                }
            })
            .collect();
        let next = if iter.next().is_some() {
            nodes.last().map(|node| node.id)
        } else {
            None
        };
        Ok(AccessiblePage {
            revision: state.editor.revision(),
            total: doc.graph().nodes().len(),
            nodes,
            next,
        })
    }
    pub fn inspect(&self, view: &str, id: Id) -> ApiResult<Node> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        if !state.views.contains_key(view) {
            return Err(ApiError::new("unknown_view", view));
        }
        let node = state
            .editor
            .document()
            .graph()
            .nodes()
            .get(&id)
            .ok_or_else(|| ApiError::new("missing_node", id))?;
        if serde_json::to_vec(node)
            .map_err(|e| ApiError::new("invalid_node", e))?
            .len()
            > 256 * 1024
        {
            return Err(ApiError::new(
                "limit_exceeded",
                "inspector metadata exceeds 256 KiB",
            ));
        }
        Ok(node.clone())
    }
    /// Rust-only persistence/execution snapshot; never send the full graph per frame.
    pub fn snapshot(&self) -> ApiResult<Document> {
        Ok(self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?
            .editor
            .document()
            .clone())
    }
}
mod commands {
    use super::*;
    #[tauri::command]
    pub async fn dispatch<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        request: Request,
    ) -> ApiResult<Summary> {
        use tauri::{Emitter, Manager};
        let changed = !matches!(request, Request::Summary);
        let appearance_changed = matches!(request, Request::SetTheme { .. });
        let engine = engine.inner().clone();
        let service = engine.clone();
        let label = window.label().to_owned();
        let result = tauri::async_runtime::spawn_blocking(move || engine.dispatch(&label, request))
            .await
            .map_err(|e| ApiError::new("worker_error", e))??;
        // Events are advisory; clients still check revisions on every edit.
        if changed {
            let _ = window.app_handle().emit("unge://changed", &result);
        }
        if appearance_changed {
            let _ = window.emit(
                "unge://appearance-changed",
                service.appearance(window.label())?,
            );
        }
        Ok(result)
    }
    /// Aggregate counters only. Terminal notification bypasses the 100 ms rate limit.
    pub fn execution_observer<R: tauri::Runtime>(
        window: impl tauri::Emitter<R> + Send + Sync,
    ) -> impl Fn(RunSummary) + Send + Sync {
        let last = Mutex::new(None::<std::time::Instant>);
        move |summary| {
            let terminal = matches!(
                summary.state,
                unge_executor::RunState::Finished | unge_executor::RunState::Failed
            );
            let mut sent = last.lock().expect("notification clock");
            if terminal
                || sent.is_none_or(|at| at.elapsed() >= std::time::Duration::from_millis(100))
            {
                *sent = Some(std::time::Instant::now());
                let _ = window.emit("unge://execution", summary);
            }
        }
    }
    #[tauri::command]
    pub async fn start_execution<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        expected_revision: u64,
    ) -> ApiResult<RunSummary> {
        let service = engine.inner().clone();
        let label = window.label().to_owned();
        let prepared = tauri::async_runtime::spawn_blocking(move || {
            service.prepare_run(&label, expected_revision)
        })
        .await
        .map_err(|e| ApiError::new("worker_error", e))??;
        let queued = prepared.summary()?;
        tauri::async_runtime::spawn_blocking(move || {
            let _ = prepared.execute(execution_observer(window));
        });
        Ok(queued)
    }
    #[tauri::command]
    pub fn current_execution<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
    ) -> ApiResult<Option<RunSummary>> {
        engine.current_execution(window.label())
    }
    #[tauri::command]
    pub fn execution_status<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        id: Id,
    ) -> ApiResult<RunSummary> {
        engine.execution_status(window.label(), id)
    }
    #[tauri::command]
    pub fn cancel_execution<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        id: Id,
    ) -> ApiResult<RunSummary> {
        engine.cancel_execution(window.label(), id)
    }
    #[tauri::command]
    pub fn appearance<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
    ) -> ApiResult<Appearance> {
        engine.appearance(window.label())
    }
    #[tauri::command]
    pub fn groups<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        expected_revision: u64,
        after: Option<Id>,
        limit: usize,
    ) -> ApiResult<GroupPage> {
        engine.groups(window.label(), expected_revision, after, limit)
    }
    #[tauri::command]
    pub fn accessible_nodes<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        expected_revision: u64,
        after: Option<Id>,
        limit: usize,
    ) -> ApiResult<AccessiblePage> {
        engine.accessible_nodes(window.label(), expected_revision, after, limit)
    }
    #[tauri::command]
    pub fn selection_summary<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
    ) -> ApiResult<SelectionSummary> {
        engine.selection_summary(window.label())
    }
    #[tauri::command]
    pub fn node_properties<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        id: Id,
        expected_revision: u64,
    ) -> ApiResult<NodeProperties> {
        engine.node_properties(window.label(), id, expected_revision)
    }
    #[tauri::command]
    pub fn inspect<R: tauri::Runtime>(
        window: tauri::Webview<R>,
        engine: tauri::State<'_, Engine>,
        id: Id,
    ) -> ApiResult<Node> {
        engine.inspect(window.label(), id)
    }
}
pub use commands::{
    accessible_nodes, appearance, cancel_execution, current_execution, dispatch,
    execution_observer, execution_status, groups, inspect, node_properties, selection_summary,
    start_execution,
};
/// Combine with your host handler using generate_handler!. Include start_execution,
/// current_execution, execution_status and cancel_execution when configuring RunService.
pub fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![
        dispatch,
        accessible_nodes,
        groups,
        inspect,
        node_properties,
        selection_summary,
        appearance,
        start_execution,
        current_execution,
        execution_status,
        cancel_execution
    ]
}

#[cfg(feature = "acx")]
impl unge_acx::GraphHost for Engine {
    fn snapshot(&self) -> unge_acx::Result<unge_acx::Snapshot> {
        let state = self
            .state
            .lock()
            .map_err(|e| unge_acx::AcxError::new("host_unavailable", e))?;
        Ok(unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        })
    }
    fn apply(
        &self,
        document: Id,
        expected_revision: u64,
        command: Command,
    ) -> unge_acx::Result<unge_acx::Snapshot> {
        check_command(&command, 0, &mut 20_000)
            .map_err(|e| unge_acx::AcxError::new(&e.code, e.message))?;
        let mut state = self
            .state
            .lock()
            .map_err(|e| unge_acx::AcxError::new("host_unavailable", e))?;
        let before = unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        };
        unge_acx::check_snapshot(&before, document, expected_revision)?;
        let changes = unge_render::SceneChanges::from_command(&command);
        state.editor.execute(command)?;
        refresh_scene(&mut state, Some(&changes));
        Ok(unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        })
    }
    fn undo(&self, document: Id, expected_revision: u64) -> unge_acx::Result<unge_acx::Snapshot> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| unge_acx::AcxError::new("host_unavailable", e))?;
        let before = unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        };
        unge_acx::check_snapshot(&before, document, expected_revision)?;
        let changes = state
            .editor
            .undo_command()
            .map(unge_render::SceneChanges::from_command);
        if !state.editor.undo()? {
            return Err(unge_acx::AcxError::new(
                "recovery_unavailable",
                "undo history unavailable",
            ));
        }
        refresh_scene(&mut state, changes.as_ref());
        Ok(unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        })
    }
}

#[cfg(test)]
mod scene_update_tests {
    use super::*;
    #[test]
    fn shared_engine_updates_geometry_and_preserves_conflict_and_history_rules() {
        let engine = Engine::new(Document::default()).unwrap();
        let viewport = Viewport {
            origin: [0.0, 0.0],
            zoom: 1.0,
            size: [800.0, 600.0],
        };
        engine.register_view("main", viewport).unwrap();
        engine.register_view("second", viewport).unwrap();
        let id = Id::new_v4();
        engine
            .dispatch(
                "main",
                Request::Apply {
                    expected_revision: 0,
                    command: Command::AddNode {
                        node: Node {
                            id,
                            type_id: "test".into(),
                            inputs: vec![],
                            outputs: vec![],
                            properties: Default::default(),
                        },
                        rect: unge_core::Rect::default(),
                    },
                },
            )
            .unwrap();
        let move_node = || Command::MoveNode {
            id,
            rect: unge_core::Rect {
                x: 5000.0,
                ..Default::default()
            },
        };
        engine
            .dispatch(
                "second",
                Request::Apply {
                    expected_revision: 1,
                    command: move_node(),
                },
            )
            .unwrap();
        assert!(
            engine
                .state
                .lock()
                .unwrap()
                .scene
                .spatial_index()
                .query(viewport.world_rect())
                .is_empty()
        );
        assert_eq!(
            engine
                .dispatch(
                    "main",
                    Request::Apply {
                        expected_revision: 1,
                        command: move_node()
                    }
                )
                .unwrap_err()
                .code,
            "revision_conflict"
        );
        engine
            .dispatch(
                "main",
                Request::Undo {
                    expected_revision: 2,
                },
            )
            .unwrap();
        assert_eq!(
            engine
                .state
                .lock()
                .unwrap()
                .scene
                .spatial_index()
                .query(viewport.world_rect()),
            vec![id]
        );
        engine
            .dispatch(
                "main",
                Request::Redo {
                    expected_revision: 3,
                },
            )
            .unwrap();
        assert!(
            engine
                .state
                .lock()
                .unwrap()
                .scene
                .spatial_index()
                .query(viewport.world_rect())
                .is_empty()
        );
        let invalid = Command::Batch {
            commands: vec![
                Command::MoveNode {
                    id,
                    rect: Default::default(),
                },
                Command::MoveNode {
                    id: Id::new_v4(),
                    rect: Default::default(),
                },
            ],
        };
        assert!(
            engine
                .dispatch(
                    "main",
                    Request::Apply {
                        expected_revision: 4,
                        command: invalid
                    }
                )
                .is_err()
        );
        assert!(
            engine
                .state
                .lock()
                .unwrap()
                .scene
                .spatial_index()
                .query(viewport.world_rect())
                .is_empty()
        );
        #[cfg(feature = "acx")]
        {
            let snapshot = unge_acx::GraphHost::snapshot(&engine).unwrap();
            unge_acx::GraphHost::apply(
                &engine,
                snapshot.document.graph().id,
                4,
                Command::MoveNode {
                    id,
                    rect: Default::default(),
                },
            )
            .unwrap();
            assert_eq!(
                engine
                    .state
                    .lock()
                    .unwrap()
                    .scene
                    .spatial_index()
                    .query(viewport.world_rect()),
                vec![id]
            );
            unge_acx::GraphHost::undo(&engine, snapshot.document.graph().id, 5).unwrap();
            assert!(
                engine
                    .state
                    .lock()
                    .unwrap()
                    .scene
                    .spatial_index()
                    .query(viewport.world_rect())
                    .is_empty()
            );
        }
    }
}
