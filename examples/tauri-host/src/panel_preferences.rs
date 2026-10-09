//! Host-only layout preferences. No document data or WebView drafts are stored here.
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::Manager;

const MAX_BYTES: u64 = 16 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Placement {
    monitor: Option<String>,
    origin: [i32; 2],
    // Logical offset from the monitor work area; logical client size.
    offset: [f64; 2],
    size: [f64; 2],
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Preferences {
    version: u32,
    floating: bool,
    workspace: Option<Placement>,
    inspector: Option<Placement>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            floating: false,
            workspace: None,
            inspector: None,
        }
    }
}
impl Preferences {
    fn validate(&self) -> io::Result<()> {
        if self.version != 1
            || [&self.workspace, &self.inspector]
                .into_iter()
                .flatten()
                .any(|p| {
                    p.monitor.as_ref().is_some_and(|s| s.len() > 1024)
                        || p.offset.iter().any(|v| !v.is_finite() || v.abs() > 1e7)
                        || p.size
                            .iter()
                            .any(|v| !v.is_finite() || *v <= 0. || *v > 100_000.)
                })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported or invalid panel preferences",
            ));
        }
        Ok(())
    }
}

fn read(path: &Path) -> io::Result<Preferences> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Preferences::default()),
        Err(e) => return Err(e),
    };
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "panel preferences too large",
        ));
    }
    let prefs: Preferences = serde_json::from_slice(&bytes)?;
    prefs.validate()?;
    Ok(prefs)
}

fn write(path: &Path, prefs: &Preferences) -> io::Result<()> {
    prefs.validate()?;
    let bytes = serde_json::to_vec_pretty(prefs)?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing preferences directory"))?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".panel-layout-{}.tmp", unge_core::Id::new_v4()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[derive(Clone, Debug)]
struct Screen {
    name: Option<String>,
    origin: [i32; 2],
    size: [u32; 2],
    scale: f64,
}
impl From<tauri::Monitor> for Screen {
    fn from(monitor: tauri::Monitor) -> Self {
        let area = monitor.work_area();
        Self {
            name: monitor.name().cloned(),
            origin: [area.position.x, area.position.y],
            size: [area.size.width, area.size.height],
            scale: monitor.scale_factor(),
        }
    }
}

// Screens are ordered with the primary first. A disconnected display falls back there.
fn fit(
    p: &Placement,
    screens: &[Screen],
    frame: [f64; 2],
    minimum: [f64; 2],
) -> Option<([i32; 2], [f64; 2])> {
    let valid = |s: &&Screen| s.scale.is_finite() && s.scale > 0. && !s.size.contains(&0);
    let screen = screens
        .iter()
        .filter(valid)
        .find(|s| s.origin == p.origin && s.name == p.monitor)
        .or_else(|| {
            screens
                .iter()
                .filter(valid)
                .find(|s| p.monitor.is_some() && s.name == p.monitor)
        })
        .or_else(|| screens.iter().find(valid))?;
    let mut position = [0; 2];
    let mut size = [0.; 2];
    for axis in 0..2 {
        let available = f64::from(screen.size[axis]) / screen.scale;
        let max_content = (available - frame[axis]).max(1.);
        size[axis] = p.size[axis].clamp(minimum[axis].min(max_content), max_content);
        let offset = p.offset[axis].clamp(0., (available - size[axis] - frame[axis]).max(0.));
        position[axis] = (f64::from(screen.origin[axis]) + offset * screen.scale).round() as i32;
    }
    Some((position, size))
}

pub struct Store {
    path: Option<PathBuf>,
    prefs: Mutex<Preferences>,
    recording: AtomicBool,
}
impl Store {
    pub fn load(path: Option<PathBuf>) -> Self {
        let (path, prefs) = match path {
            Some(path) => match read(&path) {
                Ok(prefs) => (Some(path), prefs),
                Err(e) => {
                    // Preserve malformed/newer files; do not silently overwrite them on exit.
                    eprintln!("panel preferences (using defaults, file preserved): {e}");
                    (None, Preferences::default())
                }
            },
            None => (None, Preferences::default()),
        };
        Self {
            path,
            prefs: Mutex::new(prefs),
            recording: AtomicBool::new(false),
        }
    }
    pub fn floating(&self) -> bool {
        self.prefs.lock().expect("panel preferences").floating
    }
    pub fn record_mode(&self, floating: bool) {
        self.prefs.lock().expect("panel preferences").floating = floating;
    }
    pub fn start_recording(&self) {
        self.recording.store(true, Ordering::Release);
    }
    pub fn capture(&self, window: &tauri::Window) {
        if !self.recording.load(Ordering::Acquire)
            || !matches!(window.label(), "workspace" | "inspector")
        {
            return;
        }
        let placement = || -> tauri::Result<Option<Placement>> {
            // Keep the last normal geometry instead of saving minimized/fullscreen bounds.
            if window.is_minimized()? || window.is_maximized()? || window.is_fullscreen()? {
                return Ok(None);
            }
            let Some(monitor) = window.current_monitor()? else {
                return Ok(None);
            };
            let screen = Screen::from(monitor);
            let position = window.outer_position()?;
            let size = window
                .inner_size()?
                .to_logical::<f64>(window.scale_factor()?);
            if size.width <= 0. || size.height <= 0. {
                return Ok(None);
            }
            Ok(Some(Placement {
                monitor: screen.name,
                origin: screen.origin,
                offset: [
                    (f64::from(position.x) - f64::from(screen.origin[0])) / screen.scale,
                    (f64::from(position.y) - f64::from(screen.origin[1])) / screen.scale,
                ],
                size: [size.width, size.height],
            }))
        };
        if let Ok(Some(placement)) = placement() {
            let mut prefs = self.prefs.lock().expect("panel preferences");
            if window.label() == "workspace" {
                prefs.workspace = Some(placement);
            } else {
                prefs.inspector = Some(placement);
            }
        }
    }
    pub fn restore(&self, window: &tauri::Window, minimum: [f64; 2]) -> tauri::Result<()> {
        let placement = {
            let prefs = self.prefs.lock().expect("panel preferences");
            if window.label() == "workspace" {
                prefs.workspace.clone()
            } else {
                prefs.inspector.clone()
            }
        };
        let Some(p) = placement else {
            return Ok(());
        };
        let mut screens: Vec<Screen> = window
            .primary_monitor()?
            .into_iter()
            .map(Screen::from)
            .collect();
        screens.extend(window.available_monitors()?.into_iter().map(Screen::from));
        let scale = window.scale_factor()?;
        let outer = window.outer_size()?;
        let inner = window.inner_size()?;
        let frame = [
            f64::from(outer.width.saturating_sub(inner.width)) / scale,
            f64::from(outer.height.saturating_sub(inner.height)) / scale,
        ];
        if let Some((position, size)) = fit(&p, &screens, frame, minimum) {
            window.set_min_size(Some(tauri::LogicalSize::new(
                minimum[0].min(size[0]),
                minimum[1].min(size[1]),
            )))?;
            window.set_position(tauri::PhysicalPosition::new(position[0], position[1]))?;
            window.set_size(tauri::LogicalSize::new(size[0], size[1]))?;
        }
        Ok(())
    }
    pub fn save(&self) {
        if !self.recording.load(Ordering::Acquire) {
            return;
        }
        if let Some(path) = &self.path {
            let prefs = self.prefs.lock().expect("panel preferences").clone();
            if let Err(e) = write(path, &prefs) {
                eprintln!("panel preferences save: {e}");
            }
        }
    }
}

pub fn initialize(app: &tauri::AppHandle) {
    let c = app.state::<super::workspace::Composition>();
    if !c.unified {
        return;
    }
    let store = app.state::<Store>();
    if store.floating()
        && let Err(e) = super::workspace::move_panel(app, true)
    {
        eprintln!("panel restore: {}: {}", e.code, e.message);
    }
    store.record_mode(c.is_floating());
    store.start_recording();
    for label in ["workspace", "inspector"] {
        if let Some(window) = app.get_window(label) {
            store.capture(&window);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn placement() -> Placement {
        Placement {
            monitor: Some("External".into()),
            origin: [-1920, 0],
            offset: [100., 60.],
            size: [380., 720.],
        }
    }
    #[test]
    fn geometry_uses_current_dpi_and_recovers_disconnected_monitors() {
        let primary = Screen {
            name: Some("Built-in".into()),
            origin: [0, 48],
            size: [2880, 1700],
            scale: 2.,
        };
        let external = Screen {
            name: Some("External".into()),
            origin: [-2560, 0],
            size: [2560, 1440],
            scale: 1.25,
        };
        assert_eq!(
            fit(
                &placement(),
                &[primary.clone(), external],
                [0., 28.],
                [320., 400.]
            ),
            Some(([-2435, 75], [380., 720.]))
        );
        let mut p = placement();
        p.offset = [-9000., 9000.];
        p.size = [9000., 9000.];
        assert_eq!(
            fit(&p, &[primary], [0., 28.], [320., 400.]),
            Some(([0, 48], [1440., 822.]))
        );
        assert_eq!(fit(&p, &[], [0., 28.], [320., 400.]), None);
    }
    #[test]
    fn preferences_roundtrip_replacement_and_invalid_file_preservation() {
        let directory =
            std::env::temp_dir().join(format!("unge-panel-{}", unge_core::Id::new_v4()));
        let path = directory.join("panel-layout.json");
        assert_eq!(read(&path).unwrap(), Preferences::default());
        let prefs = Preferences {
            floating: true,
            inspector: Some(placement()),
            ..Preferences::default()
        };
        write(&path, &prefs).unwrap();
        assert_eq!(read(&path).unwrap(), prefs);
        let not_initialized = Store::load(Some(path.clone()));
        not_initialized.record_mode(false);
        not_initialized.save();
        assert_eq!(read(&path).unwrap(), prefs);
        write(&path, &Preferences::default()).unwrap();
        assert_eq!(read(&path).unwrap(), Preferences::default());
        for bytes in [
            b"{broken".to_vec(),
            b"{\"version\":2,\"floating\":true,\"workspace\":null,\"inspector\":null}".to_vec(),
            vec![b' '; MAX_BYTES as usize + 1],
        ] {
            fs::write(&path, &bytes).unwrap();
            let store = Store::load(Some(path.clone()));
            assert!(!store.floating());
            store.start_recording();
            store.save();
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn geometry_rejects_invalid_saved_numbers_and_ignores_unusable_screens() {
        for invalid in [0., -1., f64::INFINITY, f64::NAN, 100_001.] {
            let mut p = placement();
            p.size[0] = invalid;
            assert!(
                Preferences {
                    inspector: Some(p),
                    ..Preferences::default()
                }
                .validate()
                .is_err()
            );
        }
        let screen = Screen {
            name: None,
            origin: [0, 0],
            size: [0, 0],
            scale: f64::NAN,
        };
        assert_eq!(fit(&placement(), &[screen], [0., 28.], [320., 400.]), None);
    }
}
