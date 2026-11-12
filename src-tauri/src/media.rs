use crate::common;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

/// One screenshot, clip or recording, as shown in the gallery.
#[derive(Serialize)]
pub struct MediaItem {
    path: String,
    name: String,
    kind: &'static str,
    /// The game it was taken in (from the file name), e.g. "Rocket League".
    game: String,
    /// Last changed, in milliseconds since 1970 (JavaScript's Date understands this).
    modified: u64,
    size: u64,
}

/// (folder inside Videos\Orbit, kind sent to the UI)
const FOLDERS: [(&str, &str); 3] = [
    ("Screenshots", "screenshot"),
    ("Clips", "clip"),
    ("Recordings", "recording"),
];

/// Everything in Videos\Orbit's folders, newest first.
pub fn list(app: &AppHandle) -> Result<Vec<MediaItem>, String> {
    let mut items = Vec::new();
    for (folder, kind) in FOLDERS {
        let dir = common::orbit_dir(app, folder)?;
        for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_millis() as u64);
            let path = entry.path();
            items.push(MediaItem {
                game: game_from_name(&path),
                path: path.to_string_lossy().into_owned(),
                name: entry.file_name().to_string_lossy().into_owned(),
                kind,
                modified,
                size: meta.len(),
            });
        }
    }
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(items)
}

/// "Rocket League_2026-11-12_20-56-03-123.mp4" → "Rocket League".
/// Files from before game detection ("Orbit_...") or renamed files → "Other".
fn game_from_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let chars: Vec<char> = stem.chars().collect();
    let n = chars.len();
    if n > common::TIMESTAMP_LEN + 1 && chars[n - common::TIMESTAMP_LEN - 1] == '_' {
        let game: String = chars[..n - common::TIMESTAMP_LEN - 1].iter().collect();
        if game != "Orbit" && !game.is_empty() {
            return game;
        }
    }
    "Other".into()
}

/// Only allow files inside Videos\Orbit, so the UI can't be used to open anything else.
fn checked(app: &AppHandle, path: &str) -> Result<PathBuf, String> {
    let root = common::orbit_dir(app, "")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let file = Path::new(path).canonicalize().map_err(|e| e.to_string())?;
    if file.starts_with(&root) {
        Ok(file)
    } else {
        Err("Not an Orbit file".into())
    }
}

/// A small 320px-wide JPG preview. Made once with FFmpeg, then reused from the cache.
pub fn thumbnail(app: &AppHandle, path: &str) -> Result<String, String> {
    let file = checked(app, path)?;
    let name = file
        .file_name()
        .ok_or("Bad file name")?
        .to_string_lossy()
        .into_owned();

    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("thumbs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let thumb = dir.join(format!("{name}.jpg"));
    if thumb.exists() {
        return Ok(thumb.to_string_lossy().into_owned());
    }

    // Write to a temp name first, so a failed attempt never leaves a broken thumbnail.
    let tmp = dir.join(format!("{name}.tmp.jpg"));
    let is_image = name.to_lowercase().ends_with(".png");
    let mut cmd = common::ffmpeg();
    cmd.arg("-y");
    if !is_image {
        // Half a second in, so we don't get a black first frame.
        cmd.args(["-ss", "0.5"]);
    }
    let output = cmd
        .arg("-i")
        .arg(&file)
        .args(["-frames:v", "1", "-vf", "scale=320:-2", "-q:v", "5"])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    std::fs::rename(&tmp, &thumb).map_err(|e| e.to_string())?;
    Ok(thumb.to_string_lossy().into_owned())
}

/// Open a file in its default Windows app (Photos, Media Player, ...).
pub fn open(app: &AppHandle, path: &str) -> Result<(), String> {
    let file = checked(app, path)?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Open Videos\Orbit in Explorer.
pub fn open_folder(app: &AppHandle) -> Result<(), String> {
    let root = common::orbit_dir(app, "")?;
    app.opener()
        .open_path(root.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}
