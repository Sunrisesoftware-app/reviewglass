//! Config store (rg.config-store).
//!
//! JSON under the user profile, written atomically, containing no secrets. A corrupt
//! file resets to defaults with a visible notice rather than failing to start.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

pub use crate::notifier::AlertConfig;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlassConfig {
    /// Outer position of the glass window in physical pixels (virtual desktop).
    pub x: Option<i32>,
    pub y: Option<i32>,
    /// Inner size of the glass window in physical pixels.
    pub width: u32,
    pub height: u32,
    pub zoom: f32,
    pub frozen: bool,
    /// Lens mode: the window rides on the cursor.
    pub lens: bool,
    /// The lens has its own, smaller size, remembered separately from the parked glass.
    pub lens_width: u32,
    pub lens_height: u32,
    /// Origin of the frozen source rectangle, kept only while `frozen`.
    pub frozen_x: i32,
    pub frozen_y: i32,
    pub visible: bool,
}

impl Default for GlassConfig {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: 900,
            height: 340,
            zoom: 2.0,
            frozen: false,
            lens: false,
            lens_width: 640,
            lens_height: 360,
            frozen_x: 0,
            frozen_y: 0,
            visible: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub toggle_glass: String,
    pub toggle_freeze: String,
    pub toggle_lens: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            toggle_glass: "Ctrl+Alt+G".into(),
            toggle_freeze: "Ctrl+Alt+F".into(),
            toggle_lens: "Ctrl+Alt+L".into(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub glass: GlassConfig,
    pub hotkeys: Hotkeys,
    pub alerts: AlertConfig,
}

/// What happened when the file was loaded, for the panel to show once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LoadOutcome {
    Loaded,
    /// No file yet: defaults, nothing to say.
    Fresh,
    /// The file could not be parsed; it was kept beside as `.bak` and defaults apply.
    ResetCorrupt,
}

pub struct Store {
    path: PathBuf,
    config: Mutex<Config>,
    outcome: LoadOutcome,
}

impl Store {
    /// Load from `dir/config.json`, creating the directory on first save.
    pub fn open(dir: &Path) -> Self {
        let path = dir.join("config.json");
        let (config, outcome) = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Config>(&bytes) {
                Ok(c) => (c, LoadOutcome::Loaded),
                Err(_) => {
                    let _ = fs::rename(&path, path.with_extension("json.bak"));
                    (Config::default(), LoadOutcome::ResetCorrupt)
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                (Config::default(), LoadOutcome::Fresh)
            }
            // Unreadable for another reason: run on defaults, do not overwrite until a
            // save is asked for.
            Err(_) => (Config::default(), LoadOutcome::ResetCorrupt),
        };
        Self {
            path,
            config: Mutex::new(config),
            outcome,
        }
    }

    pub fn outcome(&self) -> LoadOutcome {
        self.outcome
    }

    pub fn get(&self) -> Config {
        self.config.lock().clone()
    }

    /// Apply a change and persist it. The write is atomic: temp file plus rename.
    pub fn update(&self, f: impl FnOnce(&mut Config)) -> io::Result<()> {
        let snapshot = {
            let mut c = self.config.lock();
            f(&mut c);
            c.clone()
        };
        write_atomic(&self.path, &snapshot)
    }
}

fn write_atomic(path: &Path, config: &Config) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_vec_pretty(config).map_err(io::Error::other)?;
    fs::write(&tmp, body)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("reviewglass-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn fresh_store_is_defaults_and_persists() {
        let d = tmpdir("fresh");
        let s = Store::open(&d);
        assert_eq!(s.outcome(), LoadOutcome::Fresh);
        assert_eq!(s.get(), Config::default());
        s.update(|c| c.glass.zoom = 3.0).unwrap();
        let s2 = Store::open(&d);
        assert_eq!(s2.outcome(), LoadOutcome::Loaded);
        assert_eq!(s2.get().glass.zoom, 3.0);
        assert!(!d.join("config.json.tmp").exists());
    }

    #[test]
    fn corrupt_file_resets_to_defaults_and_is_kept_beside() {
        let d = tmpdir("corrupt");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("config.json"), b"{ not json").unwrap();
        let s = Store::open(&d);
        assert_eq!(s.outcome(), LoadOutcome::ResetCorrupt);
        assert_eq!(s.get(), Config::default());
        assert!(d.join("config.json.bak").exists());
    }

    #[test]
    fn unknown_and_missing_fields_are_tolerated() {
        let d = tmpdir("partial");
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("config.json"),
            br#"{"glass":{"zoom":2.5,"future":1}}"#,
        )
        .unwrap();
        let s = Store::open(&d);
        assert_eq!(s.outcome(), LoadOutcome::Loaded);
        assert_eq!(s.get().glass.zoom, 2.5);
        assert_eq!(s.get().glass.width, 900);
        assert_eq!(s.get().hotkeys, Hotkeys::default());
    }
}
