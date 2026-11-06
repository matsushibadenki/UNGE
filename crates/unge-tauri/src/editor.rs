//! One editor owned by Engine. Persistent storage is opt-in trusted Rust setup.
use super::*;
use std::ops::Deref;

pub(crate) enum EditorStore {
    Memory(Box<Editor>),
    #[cfg(feature = "acx")]
    Persistent(Box<unge_acx::PersistentEditor>),
}
impl Deref for EditorStore {
    type Target = Editor;
    fn deref(&self) -> &Editor {
        match self {
            Self::Memory(editor) => editor,
            #[cfg(feature = "acx")]
            Self::Persistent(editor) => editor.editor(),
        }
    }
}
impl EditorStore {
    pub fn check(&self) -> ApiResult<()> {
        #[cfg(feature = "acx")]
        if let Self::Persistent(editor) = self {
            editor.check()?;
        }
        Ok(())
    }
    pub fn execute(&mut self, command: Command) -> ApiResult<u64> {
        match self {
            Self::Memory(editor) => Ok(editor.execute(command)?),
            #[cfg(feature = "acx")]
            Self::Persistent(editor) => Ok(editor.execute(command).map_err(edit_error)?),
        }
    }
    pub fn undo(&mut self) -> ApiResult<bool> {
        match self {
            Self::Memory(editor) => Ok(editor.undo()?),
            #[cfg(feature = "acx")]
            Self::Persistent(editor) => Ok(editor.undo().map_err(edit_error)?),
        }
    }
    pub fn redo(&mut self) -> ApiResult<bool> {
        match self {
            Self::Memory(editor) => Ok(editor.redo()?),
            #[cfg(feature = "acx")]
            Self::Persistent(editor) => Ok(editor.redo().map_err(edit_error)?),
        }
    }
}
impl Engine {
    pub(super) fn lock_state(&self) -> ApiResult<std::sync::MutexGuard<'_, State>> {
        let state = self
            .state
            .lock()
            .map_err(|e| ApiError::new("state_unavailable", e))?;
        state.editor.check()?;
        Ok(state)
    }
    /// Trusted-host counters only. None denotes the in-memory editor, not a
    /// storage failure. Poisoned durable storage returns an error instead.
    #[cfg(feature = "acx")]
    pub fn storage_usage(&self) -> ApiResult<Option<unge_acx::StateLogUsage>> {
        let state = self.lock_state()?;
        match &state.editor {
            EditorStore::Memory(_) => Ok(None),
            EditorStore::Persistent(editor) => Ok(Some(editor.storage_usage()?)),
        }
    }
    /// Open before exposing views or configuring the Provider journal. Restores
    /// the sole Editor and its history; no mutable Document lives in a window.
    #[cfg(feature = "acx")]
    pub fn open_persistent(
        path: impl AsRef<std::path::Path>,
        initial: Document,
        registry: Arc<unge_executor::Registry>,
        options: unge_acx::PersistentHostOptions,
    ) -> ApiResult<Self> {
        let editor = unge_acx::PersistentEditor::open(path, initial, registry.clone(), options)?;
        let mut engine = Self::from_store(EditorStore::Persistent(Box::new(editor)));
        engine.registry = Some(registry);
        Ok(engine)
    }
}
#[cfg(feature = "acx")]
impl From<unge_acx::AcxError> for ApiError {
    fn from(error: unge_acx::AcxError) -> Self {
        Self::new(&error.code, error.message)
    }
}
#[cfg(feature = "acx")]
impl From<ApiError> for unge_acx::AcxError {
    fn from(error: ApiError) -> Self {
        let code = match error.code.as_str() {
            "invalid_command" => "invalid_graph",
            "state_unavailable" => "host_unavailable",
            code => code,
        };
        Self::new(code, error.message)
    }
}

#[cfg(feature = "acx")]
fn edit_error(error: unge_acx::AcxError) -> ApiError {
    let code = if error.code == "invalid_graph" {
        "invalid_command"
    } else {
        &error.code
    };
    ApiError::new(code, error.message)
}
