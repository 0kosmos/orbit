use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// All user settings. Saved as settings.json in Orbit's config folder.
/// `#[serde(default)]` means missing values (e.g. from an older version) use the defaults.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Length of the replay buffer in seconds.
    pub replay_seconds: u32,
    /// Turn the replay buffer on automatically when Orbit opens.
    pub replay_autostart: bool,
    /// Play a short sound when something is saved, recording starts, or on errors.
    pub sounds: bool,
    /// Record game / PC sound (what plays on the default speakers or headphones).
    pub system_audio: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            replay_seconds: 120,
            replay_autostart: false,
            sounds: true,
            system_audio: true,
        }
    }
}

#[derive(Default)]
pub struct SettingsState(Mutex<Settings>);

fn file(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_config_dir().ok()?.join("settings.json"))
}

/// Read settings.json once at startup (defaults if it's missing or broken).
pub fn load(app: &AppHandle) {
    let loaded = file(app)
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    *app.state::<SettingsState>().0.lock().unwrap() = loaded;
}

pub fn get(app: &AppHandle) -> Settings {
    app.state::<SettingsState>().0.lock().unwrap().clone()
}

pub fn set(app: &AppHandle, new: Settings) -> Result<(), String> {
    let f = file(app).ok_or("No config folder")?;
    if let Some(dir) = f.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&new).map_err(|e| e.to_string())?;
    std::fs::write(f, text).map_err(|e| e.to_string())?;
    *app.state::<SettingsState>().0.lock().unwrap() = new;
    Ok(())
}
