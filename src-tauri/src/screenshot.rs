use crate::common;
use crate::sound::{self, Sound};
use tauri::{AppHandle, Emitter};

/// Takes a screenshot on a background thread so the app never freezes.
/// Tells the UI the result with a "screenshot-saved" or "screenshot-failed" event.
pub fn take(app: AppHandle) {
    std::thread::spawn(move || match capture(&app) {
        Ok(path) => {
            sound::play(&app, Sound::Saved);
            let _ = app.emit("screenshot-saved", path);
        }
        Err(err) => {
            sound::play(&app, Sound::Error);
            let _ = app.emit("screenshot-failed", err);
        }
    });
}

fn capture(app: &AppHandle) -> Result<String, String> {
    let s = crate::settings::get(app);
    let jpg = s.screenshot_format == "jpg";
    let path = common::orbit_dir(app, "Screenshots")?
        .join(common::media_name(if jpg { "jpg" } else { "png" }));

    // Grab one frame with Windows Desktop Duplication (ddagrab), copy it from the GPU,
    // and save it as PNG or JPG. FFmpeg then exits, so there's no cost when idle.
    let mut cmd = common::ffmpeg();
    cmd.args(["-y", "-filter_complex"])
        .arg(format!(
            "ddagrab=framerate=60:draw_mouse={},hwdownload,format=bgra",
            s.draw_mouse as u8
        ))
        .args(["-frames:v", "1"]);
    if jpg {
        // Very high JPG quality (2 = best normal setting), about 5-10x smaller than PNG.
        cmd.args(["-q:v", "2"]);
    }
    let output = cmd
        .arg(&path)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    if !output.status.success() {
        return Err(common::friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(path.to_string_lossy().into_owned())
}
