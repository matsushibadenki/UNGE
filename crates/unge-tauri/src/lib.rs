//! Small Tauri 2 boundary. Document, views, indexes and native renderers live in Rust.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use unge_core::*;
use unge_render::{SceneIndex, SurfaceRenderer};

struct ViewState {
    viewport: Viewport,
    selection: BTreeSet<Id>,
}
struct State {
    editor: Editor,
    scene: SceneIndex,
    views: BTreeMap<String, ViewState>,
}
/// Manage one Engine per application document service. No window owns a document.
#[derive(Clone)]
pub struct Engine {
    state: Arc<Mutex<State>>,
    renderers: Arc<Mutex<BTreeMap<String, SurfaceRenderer>>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub revision: u64,
    pub nodes: usize,
    pub edges: usize,
}
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
impl From<Error> for ApiError {
    fn from(error: Error) -> Self {
        Self::new("invalid_command", error)
    }
}
type ApiResult<T> = std::result::Result<T, ApiError>;
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
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
fn refresh_scene(state: &mut State) {
    state.scene = SceneIndex::new(state.editor.document());
    let existing: BTreeSet<_> = state
        .editor
        .document()
        .graph()
        .nodes()
        .keys()
        .copied()
        .collect();
    for view in state.views.values_mut() {
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
        let editor = Editor::new(document, 256)?;
        let scene = SceneIndex::new(editor.document());
        Ok(Self {
            state: Arc::new(Mutex::new(State {
                editor,
                scene,
                views: BTreeMap::new(),
            })),
            renderers: Arc::new(Mutex::new(BTreeMap::new())),
        })
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
                viewport,
                selection: BTreeSet::new(),
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
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        let view_state = state
            .views
            .get(view)
            .ok_or_else(|| ApiError::new("unknown_view", view))?;
        let mut viewport = view_state.viewport;
        viewport.size = [
            physical_size[0].max(1) as f32,
            physical_size[1].max(1) as f32,
        ];
        let scene = state.scene.scene(viewport, &view_state.selection)?;
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
            .draw(&scene, viewport)
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
            | Request::Undo { expected_revision }
            | Request::Redo { expected_revision } => Some(*expected_revision),
            _ => None,
        };
        if expected.is_some_and(|r| r != state.editor.revision()) {
            return Err(ApiError::new("revision_conflict", state.editor.revision()));
        }
        let before = state.editor.revision();
        match request {
            Request::Apply { command, .. } => {
                state.editor.execute(command)?;
            }
            Request::Undo { .. } => {
                state.editor.undo()?;
            }
            Request::Redo { .. } => {
                state.editor.redo()?;
            }
            Request::SetViewport { viewport } => {
                viewport.validate()?;
                state.views.get_mut(view).unwrap().viewport = viewport;
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
                state.views.get_mut(view).unwrap().selection = ids;
            }
            Request::Summary => {}
        }
        if state.editor.revision() != before {
            refresh_scene(&mut state);
        }
        Ok(summary(&state))
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
        window: tauri::WebviewWindow<R>,
        engine: tauri::State<'_, Engine>,
        request: Request,
    ) -> ApiResult<Summary> {
        use tauri::{Emitter, Manager};
        let changed = !matches!(request, Request::Summary);
        let engine = engine.inner().clone();
        let label = window.label().to_owned();
        let result = tauri::async_runtime::spawn_blocking(move || engine.dispatch(&label, request))
            .await
            .map_err(|e| ApiError::new("worker_error", e))??;
        // Events are advisory; clients still check revisions on every edit.
        if changed {
            let _ = window.app_handle().emit("unge://changed", &result);
        }
        Ok(result)
    }
    #[tauri::command]
    pub fn inspect<R: tauri::Runtime>(
        window: tauri::WebviewWindow<R>,
        engine: tauri::State<'_, Engine>,
        id: Id,
    ) -> ApiResult<Node> {
        engine.inspect(window.label(), id)
    }
}
pub use commands::{dispatch, inspect};
/// Combine with your host's invoke handler by using generate_handler![unge_tauri::dispatch, unge_tauri::inspect, ...].
pub fn handler<R: tauri::Runtime>() -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static
{
    tauri::generate_handler![dispatch, inspect]
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
        state.editor.execute(command)?;
        refresh_scene(&mut state);
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
        if !state.editor.undo()? {
            return Err(unge_acx::AcxError::new(
                "recovery_unavailable",
                "undo history unavailable",
            ));
        }
        refresh_scene(&mut state);
        Ok(unge_acx::Snapshot {
            document: state.editor.document().clone(),
            revision: state.editor.revision(),
        })
    }
}
