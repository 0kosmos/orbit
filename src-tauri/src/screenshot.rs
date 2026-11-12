use std::os::windows::process::CommandExt;
use std::process::Command;
use tauri::{AppHandle, Emitter, Manager};

// Stops Windows from flashing a black console window when FFmpeg starts.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Takes a screenshot on a background thread so the app never freezes.
/// Tells the UI the result with a "screenshot-saved" or "screenshot-failed" event.
pub fn take(app: AppHandle) {
    std::thread::spawn(move || match capture(&app) {
        Ok(path) => {
            let _ = app.emit("screenshot-saved", path);
        }
        Err(err) => {
            let _ = app.emit("screenshot-failed", err);
        }
    });
}

fn capture(app: &AppHandle) -> Result<String, String> {
    // Videos\Orbit\Screenshots
    let dir = app
        .path()
        .video_dir()
        .map_err(|e| e.to_string())?
        .join("Orbit")
        .join("Screenshots");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let name = chrono::Local::now()
        .format("Orbit_%Y-%m-%d_%H-%M-%S-%3f.png")
        .to_string();
    let path = dir.join(name);

    // Grab one frame with Windows Desktop Duplication (ddagrab), copy it from the GPU,
    // and save it as PNG. FFmpeg then exits, so there's no cost when idle.
    let output = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-filter_complex",
            "ddagrab=framerate=60,hwdownload,format=bgra",
            "-frames:v",
            "1",
        ])
        .arg(&path)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(path.to_string_lossy().into_owned())
}
