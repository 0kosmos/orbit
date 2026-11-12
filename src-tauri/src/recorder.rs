use crate::audio;
use crate::common::{self, Background};
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
    common::add_capture_args(&mut cmd, &pipes);
    cmd.arg(&path);

    let done_app = app.clone();
    let path = path.to_string_lossy().into_owned();
    let done_pipes = pipes.clone();
    let started = app.state::<Recorder>().0.start(cmd, move |result| {
        audio::release(&done_pipes);
        let _ = match result {
            Ok(()) => {
                sound::play(&done_app, Sound::Saved);
                done_app.emit("recording-saved", path)
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
