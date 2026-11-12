use crate::common;
use serde::{Deserialize, Serialize};
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

impl MediaItem {
    pub fn size(&self) -> u64 {
        self.size
    }
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
        // canonicalize() returns Windows' special long form "\\?\C:\...". Some Windows
        // functions (like the Recycle Bin) reject that, so turn it back into "C:\...".
        let text = file.to_string_lossy();
        Ok(match text.strip_prefix(r"\\?\") {
            Some(normal) if !normal.starts_with("UNC\\") => PathBuf::from(normal),
            _ => file,
        })
    } else {
        Err("Not an Orbit file".into())
    }
}

/// What the gallery card needs: the small preview image, and for videos their length.
#[derive(Serialize)]
pub struct Preview {
    thumb: String,
    /// Length in seconds (videos only).
    duration: Option<f64>,
}

fn thumbs_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("thumbs");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// A small 320px-wide JPG preview (+ the video length). Made once with FFmpeg,
/// then reused from the cache: "<file>.jpg" and "<file>.len" (length in seconds).
pub fn preview(app: &AppHandle, path: &str) -> Result<Preview, String> {
    let file = checked(app, path)?;
    let name = file
        .file_name()
        .ok_or("Bad file name")?
        .to_string_lossy()
        .into_owned();
    let lower = name.to_lowercase();
    let is_image = lower.ends_with(".png") || lower.ends_with(".jpg");

    let dir = thumbs_dir(app)?;
    let thumb = dir.join(format!("{name}.jpg"));
    let len_file = dir.join(format!("{name}.len"));
    let cached_len = std::fs::read_to_string(&len_file)
        .ok()
        .and_then(|s| s.trim().parse::<f64>().ok());
    if thumb.exists() && (is_image || cached_len.is_some()) {
        return Ok(Preview {
            thumb: thumb.to_string_lossy().into_owned(),
            duration: cached_len,
        });
    }

    // Write to a temp name first, so a failed attempt never leaves a broken thumbnail.
    let tmp = dir.join(format!("{name}.tmp.jpg"));
    let mut cmd = common::ffmpeg();
    // "info" makes FFmpeg print the video's length ("Duration: 00:01:02.34"),
    // so one FFmpeg run gives us both the preview and the length.
    cmd.args(["-loglevel", "info", "-y"]);
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

    let duration = if is_image {
        None
    } else {
        let d = parse_duration(&String::from_utf8_lossy(&output.stderr));
        if let Some(d) = d {
            let _ = std::fs::write(&len_file, d.to_string());
        }
        d
    };
    Ok(Preview {
        thumb: thumb.to_string_lossy().into_owned(),
        duration,
    })
}

/// Finds "Duration: 00:01:02.34" in FFmpeg's output and returns 62.34.
fn parse_duration(text: &str) -> Option<f64> {
    let start = text.find("Duration: ")? + "Duration: ".len();
    let value = text[start..].split(',').next()?.trim();
    let mut seconds = 0.0;
    for part in value.split(':') {
        seconds = seconds * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(seconds)
}

/// Move a file to the Recycle Bin (so a misclick can be undone) and drop its cached preview.
pub fn delete(app: &AppHandle, path: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{
        FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
        SHFILEOPSTRUCTW, SHFileOperationW,
    };

    let file = checked(app, path)?;
    // Windows wants this path to end with two zero characters.
    let mut from: Vec<u16> = file.as_os_str().to_string_lossy().encode_utf16().collect();
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: from.as_ptr(),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as u16,
        ..Default::default()
    };
    // SAFETY: `op` and `from` are valid for the duration of the call.
    let result = unsafe { SHFileOperationW(&mut op) };
    if result != 0 || op.fAnyOperationsAborted != 0 {
        return Err(format!("Could not delete the file (error {result}). Is it open somewhere?"));
    }

    forget_preview(app, &file);
    Ok(())
}

/// Open a file in its default Windows app (Photos, Media Player, ...).
pub fn open(app: &AppHandle, path: &str) -> Result<(), String> {
    let file = checked(app, path)?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}

/// Move several files to the Recycle Bin. Returns the ones that were deleted;
/// files that couldn't be (e.g. open in a player) are skipped, the rest still go.
pub fn delete_many(app: &AppHandle, paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter(|path| delete(app, path).is_ok())
        .cloned()
        .collect()
}

/// A crop rectangle in image pixels.
#[derive(Deserialize)]
pub struct Crop {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

/// Crop and/or resize a screenshot with FFmpeg.
/// - `crop`: the part to keep (None = whole image)
/// - `width`: final width in pixels, height follows (None = keep size)
/// - `format`: "png" or "jpg"
/// - `replace`: overwrite the original instead of saving a copy
/// Returns the path of the saved image.
pub fn edit_image(
    app: &AppHandle,
    path: &str,
    crop: Option<Crop>,
    width: Option<u32>,
    format: &str,
    replace: bool,
) -> Result<String, String> {
    let file = checked(app, path)?;
    let dir = file.parent().ok_or("Bad path")?.to_path_buf();
    let ext = if format == "jpg" { "jpg" } else { "png" };

    // The copy keeps the game in its name, with the current time.
    let game = game_from_name(&file);
    let game = if game == "Other" { "Desktop".to_string() } else { game };
    let new_name = format!(
        "{game}_{}.{ext}",
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f")
    );
    let out = if replace {
        file.with_extension(ext)
    } else {
        dir.join(new_name)
    };
    // Write to a temp file first; only replace/rename when FFmpeg succeeded.
    let tmp = dir.join(format!("orbit-edit-tmp.{ext}"));

    let mut filters = Vec::new();
    if let Some(c) = crop {
        filters.push(format!("crop={}:{}:{}:{}", c.w.max(1), c.h.max(1), c.x, c.y));
    }
    if let Some(w) = width {
        filters.push(format!("scale={}:-1:flags=lanczos", w.max(1)));
    }
    let mut cmd = common::ffmpeg();
    cmd.args(["-y", "-i"]).arg(&file);
    if !filters.is_empty() {
        cmd.arg("-vf").arg(filters.join(","));
    }
    if ext == "jpg" {
        // Full color detail (yuvj444p) and very high quality.
        cmd.args(["-pix_fmt", "yuvj444p", "-q:v", "2"]);
    }
    let output = cmd
        .args(["-frames:v", "1"])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    if replace {
        // Drop the old preview, and the old file if the format changed.
        forget_preview(app, &file);
        if out != file {
            let _ = std::fs::remove_file(&file);
        }
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}

/// Keep only `start`..`end` (seconds) of a clip or recording.
/// Copies the video and sound without re-encoding: instant, no quality loss, no GPU
/// work. The cut snaps to the nearest keyframe (Orbit records one every second).
/// Returns the path of the saved video.
pub fn trim_video(
    app: &AppHandle,
    path: &str,
    start: f64,
    end: f64,
    replace: bool,
) -> Result<String, String> {
    let file = checked(app, path)?;
    if !(end > start && start >= 0.0) {
        return Err("The end must be after the start.".into());
    }
    let dir = file.parent().ok_or("Bad path")?.to_path_buf();

    let game = game_from_name(&file);
    let game = if game == "Other" { "Desktop".to_string() } else { game };
    let out = if replace {
        file.with_extension("mp4")
    } else {
        dir.join(format!(
            "{game}_{}.mp4",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f")
        ))
    };
    let tmp = dir.join("orbit-trim-tmp.mp4");

    // -ss before -i: jump straight to the keyframe at/before `start` (fast).
    // -map 0 keeps every track (video, Game + Mic, Game, Mic) with their names.
    let output = common::ffmpeg()
        .args(["-y", "-ss"])
        .arg(format!("{start:.3}"))
        .arg("-i")
        .arg(&file)
        .arg("-t")
        .arg(format!("{:.3}", end - start))
        .args([
            "-map",
            "0",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
            "-movflags",
            "+faststart",
        ])
        .arg(&tmp)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&tmp);
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    if replace {
        forget_preview(app, &file);
        if let Err(e) = std::fs::remove_file(&file) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Could not replace the original ({e}). Is it open somewhere?"));
        }
    }
    std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    Ok(out.to_string_lossy().into_owned())
}

/// Remove a file's cached thumbnail and length.
fn forget_preview(app: &AppHandle, file: &Path) {
    if let (Ok(dir), Some(name)) = (thumbs_dir(app), file.file_name()) {
        let name = name.to_string_lossy();
        let _ = std::fs::remove_file(dir.join(format!("{name}.jpg")));
        let _ = std::fs::remove_file(dir.join(format!("{name}.len")));
    }
}

/// Open Explorer with this file selected.
pub fn show_in_folder(app: &AppHandle, path: &str) -> Result<(), String> {
    let file = checked(app, path)?;
    app.opener()
        .reveal_item_in_dir(&file)
        .map_err(|e| e.to_string())
}

/// Open Videos\Orbit in Explorer.
pub fn open_folder(app: &AppHandle) -> Result<(), String> {
    let root = common::orbit_dir(app, "")?;
    app.opener()
        .open_path(root.to_string_lossy(), None::<&str>)
        .map_err(|e| e.to_string())
}
