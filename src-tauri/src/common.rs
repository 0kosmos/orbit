use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use crate::{audio, settings};
use tauri::{AppHandle, Manager};

// Stops Windows from flashing a black console window when FFmpeg starts.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Which FFmpeg to run: the one shipped inside Orbit (installed version), otherwise
/// the one on PATH (e.g. while developing without the bundled copy).
static FFMPEG: OnceLock<PathBuf> = OnceLock::new();

/// Called once at startup.
pub fn init_ffmpeg(app: &AppHandle) {
    let bundled = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("ffmpeg").join("ffmpeg.exe"))
        .filter(|path| path.exists());
    let _ = FFMPEG.set(bundled.unwrap_or_else(|| PathBuf::from("ffmpeg")));
}

/// An FFmpeg command with no console window that only prints real errors.
pub fn ffmpeg() -> Command {
    let path = FFMPEG.get().cloned().unwrap_or_else(|| PathBuf::from("ffmpeg"));
    let mut cmd = Command::new(path);
    cmd.args(["-hide_banner", "-loglevel", "error"])
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Turns FFmpeg's technical errors into something a user understands where we can.
pub fn friendly_error(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("nvenc") || lower.contains("nvcuda") || lower.contains("nvencodeapi") {
        return "Orbit needs an NVIDIA graphics card to record (it uses NVIDIA's video \
                encoder, NVENC). AMD and Intel support is planned."
            .into();
    }
    if lower.contains("ddagrab") || lower.contains("duplicat") {
        return format!(
            "Windows didn't allow screen capture right now (e.g. a protected screen or \
             a fullscreen switch). Try again. Details: {}",
            stderr.trim()
        );
    }
    stderr.trim().to_string()
}

/// Videos\Orbit\<sub>, created if it doesn't exist yet.
pub fn orbit_dir(app: &AppHandle, sub: &str) -> Result<PathBuf, String> {
    let dir = save_root(app)?.join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// Where everything is saved: the folder chosen in Settings, or Videos\Orbit.
pub fn save_root(app: &AppHandle) -> Result<PathBuf, String> {
    match settings::get(app).save_dir {
        Some(dir) if !dir.is_empty() => Ok(PathBuf::from(dir)),
        _ => default_root(app),
    }
}

pub fn default_root(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().video_dir().map_err(|e| e.to_string())?.join("Orbit"))
}

/// Let the window show files from the save folder (thumbnails, viewer, player).
/// Only this folder is allowed, nothing else on the PC.
pub fn allow_in_window(app: &AppHandle) {
    if let Ok(root) = save_root(app) {
        let _ = std::fs::create_dir_all(&root);
        let _ = app.asset_protocol_scope().allow_directory(&root, true);
    }
}

/// Check that Orbit can create and write files in `dir`.
pub fn check_writable(dir: &str) -> Result<(), String> {
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Can't use that folder: {e}"))?;
    let probe = dir.join(".orbit-write-test");
    std::fs::write(&probe, b"ok").map_err(|e| format!("Can't save in that folder: {e}"))?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

/// Name like Orbit_2026-11-12_20-56-03-123.ext (used for temp folders).
pub fn timestamp_name(ext: &str) -> String {
    format!(
        "{}.{ext}",
        chrono::Local::now().format("Orbit_%Y-%m-%d_%H-%M-%S-%3f")
    )
}

/// Starts the sound sources that are turned on in Settings: PC sound first, then the mic.
/// Also returns the `AvSync` that measures the sound/video start difference (pass it
/// to `Background::start`; use its offset with `add_aligned_inputs` when saving).
pub fn start_audio(app: &AppHandle) -> Result<(Vec<audio::Pipe>, Arc<audio::AvSync>), String> {
    let s = settings::get(app);
    let gate = Arc::new(audio::AvSync::default());
    let mut pipes = Vec::new();
    if s.system_audio {
        pipes.push(audio::start(audio::Source::System(s.system_device), gate.clone())?);
    }
    if s.mic_enabled {
        match audio::start(audio::Source::Mic(s.mic_device), gate.clone()) {
            Ok(pipe) => pipes.push(pipe),
            Err(e) => {
                audio::release(&pipes);
                return Err(e);
            }
        }
    }
    Ok((pipes, gate))
}

/// Inputs, filters, encoders and track layout shared by recording and the replay buffer.
/// `pipes`: sound sources from `start_audio` (can be empty for video only).
/// Returns the names of the audio tracks in the file, in order.
///
/// With game sound AND mic:
///   track 1 "Game + Mic" (mixed; the one every player plays),
///   track 2 "Game", track 3 "Mic" (separate, for video editors).
/// With only one of them: a single track with that name.
pub fn add_capture_args(
    cmd: &mut Command,
    pipes: &[audio::Pipe],
    s: &settings::Settings,
) -> Vec<&'static str> {
    for pipe in pipes {
        cmd.args(["-thread_queue_size", "1024", "-f", "f32le", "-ar"])
            .arg(audio::SAMPLE_RATE.to_string())
            .arg("-ac")
            .arg(pipe.channels.to_string())
            .arg("-i")
            .arg(&pipe.name);
    }

    // Video: Desktop Duplication + NVIDIA GPU encoder, a keyframe every second.
    let fps = s.fps();
    let mut graph = format!("ddagrab=framerate={fps}:draw_mouse={}[v]", s.draw_mouse as u8);
    // (output label or input stream, track name, bitrate)
    let mut tracks: Vec<(String, &'static str, String)> = Vec::new();

    let game_rate = format!("{}k", s.audio_bitrate.clamp(64, 320));
    let name_of = |pipe: &audio::Pipe| if pipe.channels == 1 { "Mic" } else { "Game" };
    let bitrate_of = |pipe: &audio::Pipe| {
        if pipe.channels == 1 { "96k".to_string() } else { game_rate.clone() }
    };

    if pipes.len() == 2 {
        // Copy both sources (asplit), turn the mono mic into stereo, and mix one
        // copy of each. normalize=0 keeps both at their own volume (no halving).
        graph.push_str(
            ";[0:a]asplit=2[g1][g2];[1:a]asplit=2[m1][m2]\
             ;[m1]aformat=channel_layouts=stereo[m1s]\
             ;[g1][m1s]amix=inputs=2:duration=longest:normalize=0[mix]",
        );
        tracks.push(("[mix]".into(), "Game + Mic", game_rate.clone()));
        tracks.push(("[g2]".into(), name_of(&pipes[0]), bitrate_of(&pipes[0])));
        tracks.push(("[m2]".into(), name_of(&pipes[1]), bitrate_of(&pipes[1])));
    } else {
        for (i, pipe) in pipes.iter().enumerate() {
            tracks.push((format!("{i}:a"), name_of(pipe), bitrate_of(pipe)));
        }
    }

    // With sound: FFmpeg reports its video frame count 4x a second (one short text
    // line), so Orbit knows when the video really started (audio::AvSync).
    if !pipes.is_empty() {
        cmd.args(["-progress", "pipe:1", "-stats_period", "0.25"]);
    }
    cmd.arg("-filter_complex")
        .arg(graph)
        .args(["-map", "[v]", "-c:v", "h264_nvenc", "-preset", s.nvenc_preset()])
        .args(["-cq", s.cq(), "-g"])
        .arg(fps.to_string());
    // Named tracks, so video editors show "Game + Mic" / "Game" / "Mic".
    for (i, (source, title, bitrate)) in tracks.iter().enumerate() {
        cmd.arg("-map")
            .arg(source)
            .arg(format!("-c:a:{i}"))
            .arg("aac")
            .arg(format!("-b:a:{i}"))
            .arg(bitrate)
            .arg(format!("-metadata:s:a:{i}"))
            .arg(format!("title={title}"));
    }
    tracks.into_iter().map(|(_, title, _)| title).collect()
}

/// One FFmpeg process running in the background (a recording or the replay buffer).
/// We keep FFmpeg's input (stdin) so we can send it "q", which makes FFmpeg
/// finish its file properly and quit.
#[derive(Default)]
pub struct Background {
    current: Arc<Mutex<Option<(u64, ChildStdin)>>>,
    next_id: AtomicU64,
}

impl Background {
    pub fn is_running(&self) -> bool {
        self.current.lock().unwrap().is_some()
    }

    /// Ask FFmpeg to quit. Does nothing if it isn't running.
    pub fn stop(&self) {
        let taken = self.current.lock().unwrap().take();
        if let Some((_, mut stdin)) = taken {
            let _ = stdin.write_all(b"q");
        }
    }

    /// Start FFmpeg. `on_exit` runs once it has quit: Ok if it finished cleanly,
    /// Err with FFmpeg's error message otherwise.
    /// `sync`: the recording's `AvSync` and the video fps, when the command records
    /// sound (it then prints "-progress" lines, see `add_capture_args`).
    pub fn start<F>(
        &self,
        mut cmd: Command,
        sync: Option<(Arc<audio::AvSync>, u32)>,
        on_exit: F,
    ) -> Result<(), String>
    where
        F: FnOnce(Result<(), String>) + Send + 'static,
    {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(if sync.is_some() { Stdio::piped() } else { Stdio::null() })
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
        let stdin = child.stdin.take().ok_or("Could not talk to FFmpeg")?;

        // Read FFmpeg's progress lines. The first "frame=N" with N > 0 tells when the
        // video really started: now minus N frames. The rest is just read (and ignored)
        // so FFmpeg never waits on a full pipe.
        if let (Some((gate, fps)), Some(stdout)) = (sync, child.stdout.take()) {
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(n) = line.strip_prefix("frame=") {
                        let frames: u64 = n.trim().parse().unwrap_or(0);
                        if frames > 0 {
                            // FFmpeg counts a frame ~0.13 s after capturing it (encoding +
                            // reporting), so the video really started that much earlier.
                            // Checked with the flash + beep test in audio.rs (sync_test), game
                            // sound + mic, recording and replay path: +0.024 s after saving
                            // (sound was 0.55–0.7 s late idle, ~1 s in-game, before the fix).
                            const COUNT_DELAY: f64 = 0.13;
                            let secs = frames as f64 / fps.max(1) as f64 + COUNT_DELAY;
                            if let Some(at) = std::time::Instant::now()
                                .checked_sub(std::time::Duration::from_secs_f64(secs))
                            {
                                gate.set_video_start(at);
                            }
                        }
                    }
                }
            });
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        *self.current.lock().unwrap() = Some((id, stdin));

        // A thread that just sleeps until FFmpeg exits (no polling).
        let current = self.current.clone();
        std::thread::spawn(move || {
            let result = child.wait_with_output();
            // If FFmpeg quit on its own (an error), mark it as not running anymore.
            {
                let mut cur = current.lock().unwrap();
                if cur.as_ref().is_some_and(|(c, _)| *c == id) {
                    *cur = None;
                }
            }
            on_exit(match result {
                Ok(out) if out.status.success() => Ok(()),
                Ok(out) => Err(friendly_error(&String::from_utf8_lossy(&out.stderr))),
                Err(e) => Err(e.to_string()),
            });
        });
        Ok(())
    }
}

/// Length of the date/time part of our file names: 2026-11-12_20-56-03-123
pub const TIMESTAMP_LEN: usize = 23;

/// File name for a new screenshot / clip / recording, starting with the game that's
/// in front, like "Rocket League_2026-11-12_20-56-03-123.mp4".
pub fn media_name(ext: &str) -> String {
    format!(
        "{}_{}.{ext}",
        crate::game::foreground_name(),
        chrono::Local::now().format("%Y-%m-%d_%H-%M-%S-%3f")
    )
}

/// Names the audio tracks ("Game + Mic", "Game", "Mic") in an MP4. MP4 keeps track
/// names as "handler name", which is what players and editors show, so set both.
pub fn add_track_names(cmd: &mut Command, tracks: &[&str]) {
    for (i, title) in tracks.iter().enumerate() {
        cmd.arg(format!("-metadata:s:a:{i}"))
            .arg(format!("title={title}"))
            .arg(format!("-metadata:s:a:{i}"))
            .arg(format!("handler_name={title}"));
    }
}

/// Adds the input(s) for saving a recording / clip with the sound lined up with the
/// video: the same file is read twice, the video from the start and the sound with
/// `offset` seconds cut off its start (see `audio::AvSync`). Lossless (`-c copy`).
/// `input` = the input options, e.g. `["-i", "x.mkv"]`.
pub fn add_aligned_inputs(cmd: &mut Command, input: &[&std::ffi::OsStr], offset: f64) {
    cmd.args(input);
    if offset >= 0.01 {
        cmd.arg("-ss").arg(format!("{offset:.3}")).args(input);
        cmd.args(["-map", "0:v", "-map", "1:a"]);
    } else {
        cmd.args(["-map", "0"]);
    }
}
