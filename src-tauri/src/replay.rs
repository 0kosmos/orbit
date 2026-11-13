use crate::audio;
use crate::common::{self, Background};
use crate::settings;
use crate::sound::{self, Sound};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;
use tauri::{AppHandle, Emitter, Manager};

/// Each piece of the buffer is about this many seconds long.
const SEGMENT_SECONDS: u32 = 5;

/// The replay buffer: FFmpeg keeps writing short pieces (buf_000.ts, buf_001.ts, ...)
/// into a temp folder and overwrites the oldest one in a circle, so disk use stays fixed.
/// Saving joins the newest pieces into one MP4.
#[derive(Default)]
pub struct Replay {
    ffmpeg: Background,
    /// Folder of the current buffer session.
    dir: Mutex<Option<PathBuf>>,
    /// Set when the length changed: start again as soon as FFmpeg has quit.
    restart: AtomicBool,
    /// Names of the audio tracks in the buffer ("Game + Mic", "Game", "Mic").
    tracks: Mutex<Vec<&'static str>>,
    /// Sound/video start difference of the running buffer (cut from the sound on save).
    av: Mutex<Option<std::sync::Arc<audio::AvSync>>>,
}

pub fn is_running(app: &AppHandle) -> bool {
    app.state::<Replay>().ffmpeg.is_running()
}

pub fn toggle(app: &AppHandle) {
    if is_running(app) {
        stop(app);
    } else {
        start(app);
    }
}

pub fn stop(app: &AppHandle) {
    app.state::<Replay>().ffmpeg.stop();
}

/// Called when the length setting changes: restart so the new length applies.
pub fn restart(app: &AppHandle) {
    let state = app.state::<Replay>();
    if state.ffmpeg.is_running() {
        state.restart.store(true, Ordering::Relaxed);
        state.ffmpeg.stop();
    }
}

pub fn start(app: &AppHandle) {
    if let Err(err) = try_start(app) {
        sound::play(app, Sound::Error);
        let _ = app.emit("replay-failed", err);
    }
}

fn try_start(app: &AppHandle) -> Result<(), String> {
    if is_running(app) {
        return Ok(());
    }
    let state = app.state::<Replay>();

    // Clean out old buffer folders, then make a fresh one for this session.
    let root = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("replay");
    let _ = std::fs::remove_dir_all(&root);
    let dir = root.join(common::timestamp_name("d"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Keep a few more pieces than needed, so the ones we join are never being overwritten.
    let pieces = settings::get(app).replay_seconds / SEGMENT_SECONDS + 3;

    let (pipes, av) = common::start_audio(app)?;
    // With sound: measure when the video really starts, to line the sound up on save.
    let sync = (!pipes.is_empty()).then(|| (av.clone(), crate::settings::get(app).fps()));
    *state.av.lock().unwrap() = Some(av);

    let mut cmd = common::ffmpeg();
    cmd.arg("-y");
    // (The keyframe every second lets pieces be cut cleanly every 5 seconds.)
    let tracks = common::add_capture_args(&mut cmd, &pipes, &crate::settings::get(app));
    cmd.args([
        "-f",
        "segment",
        "-segment_time",
        &SEGMENT_SECONDS.to_string(),
        "-segment_wrap",
        &pieces.to_string(),
        "-reset_timestamps",
        "1",
    ])
    .arg(dir.join("buf_%03d.ts"));

    let done_app = app.clone();
    let done_pipes = pipes.clone();
    let started = state.ffmpeg.start(cmd, sync, move |result| {
        audio::release(&done_pipes);
        let state = done_app.state::<Replay>();
        if state.restart.swap(false, Ordering::Relaxed) {
            start(&done_app);
            return;
        }
        let _ = match result {
            Ok(()) => done_app.emit("replay-stopped", ()),
            Err(err) => {
                sound::play(&done_app, Sound::Error);
                done_app.emit("replay-failed", err)
            }
        };
    });
    if let Err(err) = started {
        audio::release(&pipes);
        return Err(err);
    }
    *state.dir.lock().unwrap() = Some(dir);
    *state.tracks.lock().unwrap() = tracks;
    let _ = app.emit("replay-started", ());
    Ok(())
}

/// Save the last X seconds as an MP4 (on a background thread).
pub fn save(app: AppHandle) {
    std::thread::spawn(move || {
        let _ = match try_save(&app) {
            Ok(path) => {
                sound::play(&app, Sound::Saved);
                app.emit("replay-saved", path)
            }
            Err(err) => {
                sound::play(&app, Sound::Error);
                app.emit("replay-save-failed", err)
            }
        };
    });
}

fn try_save(app: &AppHandle) -> Result<String, String> {
    if !is_running(app) {
        return Err("Replay buffer is off. Turn it on first.".into());
    }
    let dir = app
        .state::<Replay>()
        .dir
        .lock()
        .unwrap()
        .clone()
        .ok_or("Replay buffer is off")?;

    // All pieces, newest first (by when they were last written).
    let mut pieces: Vec<(SystemTime, String)> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".ts") {
                return None;
            }
            Some((entry.metadata().ok()?.modified().ok()?, name))
        })
        .collect();
    pieces.sort_by(|a, b| b.0.cmp(&a.0));

    // The newest piece is still being written (it holds the last few seconds), plus
    // enough full pieces before it to cover the chosen length. Then oldest first.
    let wanted = (settings::get(app).replay_seconds / SEGMENT_SECONDS + 1) as usize;
    pieces.truncate(wanted);
    pieces.reverse();
    if pieces.is_empty() {
        return Err("Nothing in the buffer yet, try again in a few seconds.".into());
    }

    // A list file for FFmpeg's "concat" (join) feature.
    let list: String = pieces
        .iter()
        .map(|(_, name)| format!("file '{name}'\n"))
        .collect();
    let list_path = dir.join("list.txt");
    std::fs::write(&list_path, list).map_err(|e| e.to_string())?;

    // Join without re-encoding (-c copy): fast and no quality loss.
    let out = common::orbit_dir(app, "Clips")?.join(common::media_name("mp4"));
    // Every track (video, game sound, mic), with the sound lined up with the video.
    let offset = app
        .state::<Replay>()
        .av
        .lock()
        .unwrap()
        .as_ref()
        .map_or(0.0, |av| av.offset());
    let mut cmd = common::ffmpeg();
    cmd.arg("-y");
    common::add_aligned_inputs(
        &mut cmd,
        &["-f".as_ref(), "concat".as_ref(), "-safe".as_ref(), "0".as_ref(), "-i".as_ref(), list_path.as_os_str()],
        offset,
    );
    cmd.args(["-c", "copy", "-movflags", "+faststart"]);
    // The .ts pieces can't store track names, so add them again.
    common::add_track_names(&mut cmd, &app.state::<Replay>().tracks.lock().unwrap());
    let output = cmd
        .arg(&out)
        .output()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
    if !output.status.success() {
        return Err(common::friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(out.to_string_lossy().into_owned())
}
