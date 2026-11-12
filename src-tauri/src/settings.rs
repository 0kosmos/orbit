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
    /// Which playback device to record (Windows device id). None = the Windows default.
    pub system_device: Option<String>,
    /// Record the microphone as a separate track.
    pub mic_enabled: bool,
    /// Which mic (Windows device id). None = the Windows default mic.
    pub mic_device: Option<String>,
    /// Video frames per second (30, 60, 90, 120, 144).
    pub video_fps: u32,
    /// "low" | "medium" | "high" | "very_high" (NVENC constant quality 30/26/23/19).
    pub video_quality: String,
    /// "fastest" | "balanced" | "quality" (NVENC presets p1/p4/p6: less or more GPU work).
    pub encoder_preset: String,
    /// Show the mouse cursor in recordings and screenshots.
    pub draw_mouse: bool,
    /// Game / mixed sound bitrate in kbit/s (128, 160, 192, 256).
    pub audio_bitrate: u32,
    /// "png" (lossless, bigger) or "jpg" (smaller).
    pub screenshot_format: String,
    /// Look: "black" | "grey" | "light".
    pub theme: String,
    /// Short animations when you click or open things (off = everything instant).
    pub animations: bool,
    /// Blur / see-through effects behind the viewer, notifications and filter bar.
    pub blur: bool,
    /// Closing the window keeps Orbit running in the system tray (hotkeys keep working).
    pub close_to_tray: bool,
    /// Start Orbit (in the tray, no window) when Windows starts.
    pub start_with_windows: bool,
    /// Hotkeys, like "Alt+F12" or "Ctrl+Shift+KeyS".
    pub hotkey_screenshot: String,
    pub hotkey_record: String,
    pub hotkey_replay: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            replay_seconds: 120,
            replay_autostart: false,
            sounds: true,
            system_audio: true,
            system_device: None,
            mic_enabled: false,
            mic_device: None,
            video_fps: 60,
            video_quality: "high".into(),
            encoder_preset: "balanced".into(),
            draw_mouse: true,
            audio_bitrate: 160,
            screenshot_format: "png".into(),
            theme: "black".into(),
            animations: true,
            blur: true,
            close_to_tray: true,
            start_with_windows: false,
            hotkey_screenshot: "Alt+F12".into(),
            hotkey_record: "Alt+F9".into(),
            hotkey_replay: "Alt+F10".into(),
        }
    }
}

impl Settings {
    /// NVENC constant-quality value: lower = better quality, bigger files.
    pub fn cq(&self) -> &'static str {
        match self.video_quality.as_str() {
            "low" => "30",
            "medium" => "26",
            "very_high" => "19",
            _ => "23",
        }
    }

    /// NVENC preset: p1 = least GPU work, p7 = most.
    pub fn nvenc_preset(&self) -> &'static str {
        match self.encoder_preset.as_str() {
            "fastest" => "p1",
            "quality" => "p6",
            _ => "p4",
        }
    }

    pub fn fps(&self) -> u32 {
        self.video_fps.clamp(15, 240)
    }

    /// True if `other` would record differently (so a running replay buffer must restart).
    pub fn capture_differs(&self, other: &Settings) -> bool {
        self.replay_seconds != other.replay_seconds
            || self.video_fps != other.video_fps
            || self.video_quality != other.video_quality
            || self.encoder_preset != other.encoder_preset
            || self.draw_mouse != other.draw_mouse
            || self.audio_bitrate != other.audio_bitrate
            || self.system_audio != other.system_audio
            || self.system_device != other.system_device
            || self.mic_enabled != other.mic_enabled
            || self.mic_device != other.mic_device
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
