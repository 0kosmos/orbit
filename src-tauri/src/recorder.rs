use crate::common;
use std::io::Write;
use std::process::{ChildStdin, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter, Manager};

/// Remembers the FFmpeg recording that is running, if any.
/// We keep FFmpeg's input (stdin) so we can send it "q", which makes FFmpeg
/// finish the file properly and quit.
#[derive(Default)]
pub struct Recorder {
    current: Mutex<Option<(u64, ChildStdin)>>,
    next_id: AtomicU64,
}

pub fn is_recording(app: &AppHandle) -> bool {
    app.state::<Recorder>().current.lock().unwrap().is_some()
}

/// Start if stopped, stop if recording.
pub fn toggle(app: &AppHandle) {
    if is_recording(app) {
        stop(app);
    } else if let Err(err) = start(app) {
        let _ = app.emit("recording-failed", err);
    }
}

/// Ask FFmpeg to finish the file and quit. Does nothing if not recording.
pub fn stop(app: &AppHandle) {
    let taken = app.state::<Recorder>().current.lock().unwrap().take();
    if let Some((_, mut stdin)) = taken {
        let _ = stdin.write_all(b"q");
    }
}

fn start(app: &AppHandle) -> Result<(), String> {
    let path = common::orbit_dir(app, "Recordings")?.join(common::timestamp_name("mkv"));

    // Same capture you tested: Desktop Duplication + NVIDIA GPU encoder.
    let mut child = common::ffmpeg()
        .args([
            "-y",
            "-filter_complex",
            "ddagrab=framerate=60",
            "-c:v",
            "h264_nvenc",
            "-cq",
            "23",
        ])
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start FFmpeg: {e}"))?;

    let stdin = child.stdin.take().ok_or("Could not talk to FFmpeg")?;
    let state = app.state::<Recorder>();
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    *state.current.lock().unwrap() = Some((id, stdin));
    let _ = app.emit("recording-started", ());

    // A thread that just sleeps until FFmpeg exits (no polling), then reports back.
    let app = app.clone();
    let path = path.to_string_lossy().into_owned();
    std::thread::spawn(move || {
        let result = child.wait_with_output();

        // If FFmpeg quit on its own (an error), mark us as not recording anymore.
        {
            let state = app.state::<Recorder>();
            let mut current = state.current.lock().unwrap();
            if current.as_ref().is_some_and(|(cur, _)| *cur == id) {
                *current = None;
            }
        }

        match result {
            Ok(out) if out.status.success() => {
                let _ = app.emit("recording-saved", path);
            }
            Ok(out) => {
                let _ = app.emit(
                    "recording-failed",
                    String::from_utf8_lossy(&out.stderr).trim().to_string(),
                );
            }
            Err(e) => {
                let _ = app.emit("recording-failed", e.to_string());
            }
        }
    });
    Ok(())
}
