use crate::common;
use tauri::{AppHandle, Emitter};

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
    let path = common::orbit_dir(app, "Screenshots")?.join(common::timestamp_name("png"));

    // Grab one frame with Windows Desktop Duplication (ddagrab), copy it from the GPU,
    // and save it as PNG. FFmpeg then exits, so there's no cost when idle.
    let output = common::ffmpeg()
        .args([
            "-y",
            "-filter_complex",
            "ddagrab=framerate=60,hwdownload,format=bgra",
            "-frames:v",
            "1",
        ])
        .arg(&path)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(path.to_string_lossy().into_owned())
}
