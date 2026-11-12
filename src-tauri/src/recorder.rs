use crate::audio;
use crate::common::{self, Background};
use std::path::{Path, PathBuf};
use crate::sound::{self, Sound};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
pub struct Recorder(Background);

pub fn is_recording(app: &AppHandle) -> bool {
    app.state::<Recorder>().0.is_running()
}

/// Start if stopped, stop if recording.
pub fn toggle(app: &AppHandle) {
    if is_recording(app) {
        stop(app);
    } else if let Err(err) = start(app) {
        sound::play(app, Sound::Error);
        let _ = app.emit("recording-failed", err);
    }
}

pub fn stop(app: &AppHandle) {
    app.state::<Recorder>().0.stop();
}

fn start(app: &AppHandle) -> Result<(), String> {
    let path = common::orbit_dir(app, "Recordings")?.join(common::media_name("mkv"));

    let pipes = common::start_audio(app)?;

    let mut cmd = common::ffmpeg();
    cmd.arg("-y");
    let tracks = common::add_capture_args(&mut cmd, &pipes);
    cmd.arg(&path);

    let done_app = app.clone();
    let done_pipes = pipes.clone();
    let started = app.state::<Recorder>().0.start(cmd, move |result| {
        audio::release(&done_pipes);
        let _ = match result {
            Ok(()) => {
                let saved = to_mp4(&path, &tracks);
                sound::play(&done_app, Sound::Saved);
                done_app.emit("recording-saved", saved.to_string_lossy().into_owned())
            }
            Err(err) => {
                sound::play(&done_app, Sound::Error);
                done_app.emit("recording-failed", err)
            }
        };
    });
    if let Err(err) = started {
        audio::release(&pipes);
        return Err(err);
    }
    sound::play(app, Sound::Start);
    let _ = app.emit("recording-started", ());
    Ok(())
}

/// Recordings are written as MKV (still playable if the PC crashes mid-recording).
/// Afterwards they're copied into an MP4 (no re-encoding, about 1 s per GB), which
/// plays everywhere, including Orbit's own viewer. If that fails, the MKV is kept.
fn to_mp4(mkv: &Path, tracks: &[&str]) -> PathBuf {
    let mp4 = mkv.with_extension("mp4");
    let mut cmd = common::ffmpeg();
    cmd.args(["-y", "-i"]).arg(mkv).args(["-map", "0", "-c", "copy"]);
    common::add_track_names(&mut cmd, tracks);
    let ok = cmd
        .arg(&mp4)
        .output()
        .is_ok_and(|out| out.status.success());
    if ok && std::fs::remove_file(mkv).is_ok() {
        mp4
    } else {
        let _ = std::fs::remove_file(&mp4);
        mkv.to_path_buf()
    }
}
