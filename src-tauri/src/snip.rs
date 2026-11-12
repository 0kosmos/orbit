//! Region screenshot ("snip"), like Windows' Snipping Tool:
//! 1. take a full screenshot right away (the moment is frozen),
//! 2. show it full-screen in a window where you drag the part you want,
//! 3. save only that part (as a normal Orbit screenshot) and copy it to the clipboard.
//! The window only exists while snipping.

use crate::sound::{self, Sound};
use crate::{common, game, media::Crop, settings};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;

const LABEL: &str = "snip";

/// The frozen screenshot and the game it was taken in.
#[derive(Default)]
pub struct SnipState(Mutex<Option<(PathBuf, String)>>);

#[derive(Serialize)]
pub struct SnipInfo {
    path: String,
}

fn work_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("snip");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Hotkey / overlay button: freeze the screen and open the snip window.
pub fn start(app: &AppHandle) {
    if app.get_webview_window(LABEL).is_some() {
        return; // already snipping
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = try_start(&app) {
            sound::play(&app, Sound::Error);
            let _ = app.emit("screenshot-failed", e);
        }
    });
}

fn try_start(app: &AppHandle) -> Result<(), String> {
    // Which game is in front, before our window covers it.
    let game = game::foreground_name();

    // 1. Full screenshot of the main monitor, without the mouse cursor.
    let frozen = work_dir(app)?.join("frozen.png");
    let output = common::ffmpeg()
        .args([
            "-y",
            "-filter_complex",
            "ddagrab=framerate=60:draw_mouse=0,hwdownload,format=bgra",
            "-frames:v",
            "1",
        ])
        .arg(&frozen)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    *app.state::<SnipState>().0.lock().unwrap() = Some((frozen, game));

    // 2. A borderless, always-on-top window exactly over the main monitor.
    let monitor = app
        .primary_monitor()
        .ok()
        .flatten()
        .ok_or("No monitor found")?;
    let scale = monitor.scale_factor();
    let pos = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("snip".into()))
        .title("Orbit snip")
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .position(pos.x, pos.y)
        .inner_size(size.width, size.height)
        // See-through + invisible until the frozen picture is drawn (see `ready`), so
        // there's never a black frame: anything early just shows the real screen.
        .transparent(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    // Safety net: show it anyway if the page never reports "ready".
    let w = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(2500));
        if !w.is_visible().unwrap_or(true) {
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
    Ok(())
}

/// The snip page has drawn the frozen picture: now show the window.
pub fn ready(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// For the snip window: the frozen screenshot to show.
pub fn info(app: &AppHandle) -> Option<SnipInfo> {
    app.state::<SnipState>()
        .0
        .lock()
        .unwrap()
        .as_ref()
        .map(|(p, _)| SnipInfo { path: p.to_string_lossy().into_owned() })
}

/// Close the snip window and forget the frozen screenshot.
pub fn cancel(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
    if let Some((frozen, _)) = app.state::<SnipState>().0.lock().unwrap().take() {
        let _ = std::fs::remove_file(frozen);
    }
}

/// Copy the chosen part (None = everything) to the clipboard, and save it as a
/// screenshot unless `keep` is false ("Copy" only).
pub fn save(app: &AppHandle, crop: Option<Crop>, keep: bool) -> Result<String, String> {
    let (frozen, game) = app
        .state::<SnipState>()
        .0
        .lock()
        .unwrap()
        .clone()
        .ok_or("Nothing to save")?;
    // Close the window right away; the saving happens in the background.
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }

    let dir = work_dir(app)?;
    let cut = dir.join("cut.png");
    let mut cmd = common::ffmpeg();
    cmd.args(["-y", "-i"]).arg(&frozen);
    if let Some(c) = &crop {
        cmd.arg("-vf").arg(c.ffmpeg_filter());
    }
    let output = cmd
        .args(["-frames:v", "1"])
        .arg(&cut)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    // Copy to the clipboard (paste it straight into Discord etc.).
    if let Ok(image) = tauri::image::Image::from_path(&cut) {
        let _ = app.clipboard().write_image(&image);
    }

    if !keep {
        let _ = std::fs::remove_file(&cut);
        cancel(app);
        sound::play(app, Sound::Saved);
        let _ = app.emit("snip-copied", ());
        return Ok(String::new());
    }

    // Save it like any other screenshot (PNG or JPG, as set in Settings).
    let jpg = settings::get(app).screenshot_format == "jpg";
    let name = format!(
        "{game}_{}.{}",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f"),
        if jpg { "jpg" } else { "png" }
    );
    let out = common::orbit_dir(app, "Screenshots")?.join(name);
    if jpg {
        let ok = common::ffmpeg()
            .args(["-y", "-i"])
            .arg(&cut)
            .args(["-pix_fmt", "yuvj444p", "-q:v", "2"])
            .arg(&out)
            .output()
            .is_ok_and(|o| o.status.success());
        if !ok {
            return Err("Could not save the JPG".into());
        }
    } else {
        std::fs::copy(&cut, &out).map_err(|e| e.to_string())?;
    }
    let _ = std::fs::remove_file(&cut);
    cancel(app);

    sound::play(app, Sound::Saved);
    let path = out.to_string_lossy().into_owned();
    let _ = app.emit("screenshot-saved", path.clone());
    Ok(path)
}
