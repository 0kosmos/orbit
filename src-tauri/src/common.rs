use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

// Stops Windows from flashing a black console window when FFmpeg starts.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// An FFmpeg command with no console window that only prints real errors.
pub fn ffmpeg() -> Command {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"])
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Videos\Orbit\<sub>, created if it doesn't exist yet.
pub fn orbit_dir(app: &AppHandle, sub: &str) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .video_dir()
        .map_err(|e| e.to_string())?
        .join("Orbit")
        .join(sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// File name like Orbit_2026-11-12_20-56-03-123.png
pub fn timestamp_name(ext: &str) -> String {
    format!(
        "{}.{ext}",
        chrono::Local::now().format("Orbit_%Y-%m-%d_%H-%M-%S-%3f")
    )
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
    pub fn start<F>(&self, mut cmd: Command, on_exit: F) -> Result<(), String>
    where
        F: FnOnce(Result<(), String>) + Send + 'static,
    {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Could not start FFmpeg: {e}"))?;
        let stdin = child.stdin.take().ok_or("Could not talk to FFmpeg")?;
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
                Ok(out) => Err(String::from_utf8_lossy(&out.stderr).trim().to_string()),
                Err(e) => Err(e.to_string()),
            });
        });
        Ok(())
    }
}
