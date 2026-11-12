use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Manager};

// Stops Windows from flashing a black console window when FFmpeg starts.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// An FFmpeg command with no console window that only prints real errors.
pub fn ffmpeg() -> Command {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"])
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Videos\Orbit\<sub>, created if it doesn't exist yet.
pub fn orbit_dir(app: &AppHandle, sub: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .video_dir()
        .map_err(|e| e.to_string())?
        .join("Orbit")
        .join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// File name like Orbit_2026-11-12_20-56-03-123.png
pub fn timestamp_name(ext: &str) -> String {
    format!(
        "{}.{ext}",
        chrono::Local::now().format("Orbit_%Y-%m-%d_%H-%M-%S-%3f")
    )
}
