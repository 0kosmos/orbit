//! Sound capture: game / PC sound and microphone.
//!
//! FFmpeg on Windows can't record "what you hear" by itself, so Orbit does it with
//! Windows' own audio system (WASAPI):
//! - PC sound: "loopback" capture records whatever is sent to a playback device
//!   (speakers, headphones, Voicemeeter...). Same method as OBS; never touches the game.
//! - Microphone: normal capture from the chosen mic.
//!
//! The sound is passed to FFmpeg through a named pipe, a small in-memory connection
//! between two programs, which FFmpeg reads like a file.

use serde::Serialize;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Write};
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use wasapi::{Device, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_OUTBOUND;
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT,
};

/// All sound is converted to 48 kHz, 32-bit float.
pub const SAMPLE_RATE: usize = 48_000;
/// When nothing arrives, fill in silence only up to this far behind "now", so
/// the normal small delay of sound arriving never adds extra silence.
const LATENCY_FRAMES: u64 = (SAMPLE_RATE / 20) as u64; // 50 ms

/// Where to capture sound from. The device id is None for the Windows default.
pub enum Source {
    /// Whatever plays on a playback device (stereo).
    System(Option<String>),
    /// A microphone (mono).
    Mic(Option<String>),
}

impl Source {
    pub fn channels(&self) -> usize {
        match self {
            Source::System(_) => 2,
            Source::Mic(_) => 1,
        }
    }

    /// The chosen device, or the Windows default if none is chosen or it's unplugged.
    fn device(&self, enumerator: &DeviceEnumerator) -> Result<Device, wasapi::WasapiError> {
        let (id, direction) = match self {
            // "Render" = a playback device; opening it for capture = loopback.
            Source::System(id) => (id, Direction::Render),
            Source::Mic(id) => (id, Direction::Capture),
        };
        if let Some(id) = id {
            if let Ok(device) = enumerator.get_device(id) {
                return Ok(device);
            }
        }
        enumerator.get_default_device(direction)
    }
}

// ---------------------------------------------------------------------------
// Sending sound to FFmpeg
// ---------------------------------------------------------------------------

/// A sound source that is being sent into a pipe for FFmpeg.
#[derive(Clone)]
pub struct Pipe {
    pub name: String,
    pub channels: usize,
}

static NEXT_PIPE: AtomicU64 = AtomicU64::new(0);

/// Measures how much later the video started than the sound, so it can be corrected
/// when the file is saved (see `common::align_audio_args`).
///
/// FFmpeg opens the sound pipes well before its first video frame (setting up screen
/// capture and the NVIDIA encoder takes ~0.5 s idle, ~1 s in a heavy game). Sound and
/// video both count from 0, so without correction the sound is that much too late.
/// The sound is NOT held back (that would stall FFmpeg: with game sound + mic the
/// screen capture shares a filter graph with the audio mixing); instead the start of
/// the sound is cut off when saving.
#[derive(Default)]
pub struct AvSync {
    audio_start: Mutex<Option<Instant>>,
    video_start: Mutex<Option<Instant>>,
}

impl AvSync {
    /// Sound time 0 (the first sound source to start wins).
    fn set_audio_start(&self, at: Instant) {
        self.audio_start.lock().unwrap().get_or_insert(at);
    }

    /// Video time 0 (worked out from FFmpeg's progress report).
    pub fn set_video_start(&self, at: Instant) {
        self.video_start.lock().unwrap().get_or_insert(at);
    }

    /// Seconds of sound to cut from the start so it lines up with the video
    /// (0 if unknown, e.g. a very short recording).
    pub fn offset(&self) -> f64 {
        let audio = *self.audio_start.lock().unwrap();
        let video = *self.video_start.lock().unwrap();
        match (audio, video) {
            (Some(a), Some(v)) if v > a => (v - a).as_secs_f64(),
            _ => 0.0,
        }
    }
}

/// Creates a pipe and starts a thread that streams the sound into it once FFmpeg
/// connects (its start time goes to `sync`). The thread ends by itself when FFmpeg stops.
pub fn start(source: Source, sync: Arc<AvSync>) -> Result<Pipe, String> {
    let name = format!(
        r"\\.\pipe\orbit_audio_{}_{}",
        std::process::id(),
        NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
    );
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: `wide` is a valid zero-terminated string; other arguments are plain values.
    let handle = unsafe {
        CreateNamedPipeW(
            wide.as_ptr(),
            PIPE_ACCESS_OUTBOUND,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1,
            1 << 20, // 1 MB buffer (a few seconds of sound)
            0,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err("Could not set up audio capture".into());
    }
    let channels = source.channels();

    // Raw handles can't be sent to another thread directly, so pass it as a number.
    let raw = handle as usize;
    std::thread::spawn(move || {
        let handle = raw as HANDLE;
        // Sleep until FFmpeg opens the pipe.
        // SAFETY: `handle` is the pipe we just created.
        let connected = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if !connected {
            unsafe { CloseHandle(handle) };
            return;
        }
        // SAFETY: we own the handle; File closes it when dropped.
        let mut pipe = unsafe { File::from_raw_handle(handle as _) };
        // This ends with an error when FFmpeg stops and closes the pipe; that's normal.
        let _ = capture(&source, Some(&sync), &mut |bytes| pipe.write_all(bytes));
    });
    Ok(Pipe { name, channels })
}

/// If FFmpeg failed before opening the pipe, the thread above would wait forever.
/// Connecting to the pipe ourselves wakes it up so it can finish.
pub fn release(pipes: &[Pipe]) {
    for pipe in pipes {
        let _ = File::open(&pipe.name);
    }
}

/// Captures sound and hands it to `sink` (raw 32-bit float samples) until `sink`
/// returns an error. Runs on its own thread.
fn capture(
    source: &Source,
    sync: Option<&AvSync>,
    sink: &mut dyn FnMut(&[u8]) -> io::Result<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let _ = wasapi::initialize_mta();

    let device = source.device(&DeviceEnumerator::new()?)?;
    let channels = source.channels();
    let bytes_per_frame = channels * 4;

    let mut client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, SampleType::Float, SAMPLE_RATE, channels, None);
    client.initialize_client(
        &format,
        Direction::Capture,
        StreamMode::EventsShared {
            // Let Windows convert to our format, whatever the device uses.
            autoconvert: true,
            // 200 ms of buffer inside Windows, so short hiccups never lose sound.
            buffer_duration_hns: 2_000_000,
        },
        None,
    )?;
    let event = client.set_get_eventhandle()?;
    let capture = client.get_audiocaptureclient()?;

    let mut data: VecDeque<u8> = VecDeque::new();
    let mut frames_written: u64 = 0;
    let silence = vec![0u8; bytes_per_frame * SAMPLE_RATE / 10];
    // Write `frames` of silence.
    let write_silence = |sink: &mut dyn FnMut(&[u8]) -> io::Result<()>, mut frames: usize| {
        while frames > 0 {
            let n = frames.min(silence.len() / bytes_per_frame);
            sink(&silence[..n * bytes_per_frame])?;
            frames -= n;
        }
        io::Result::Ok(())
    };

    client.start_stream()?;
    // Sound time 0 = this moment.
    let started = Instant::now();
    if let Some(sync) = sync {
        sync.set_audio_start(started);
    }

    loop {
        // Windows wakes us when sound is available. For PC sound it sends nothing at
        // all while nothing is playing, so we also wake every 20 ms to fill in silence.
        let _ = event.wait_for_event(20);

        while capture.get_next_packet_size()?.unwrap_or(0) > 0 {
            capture.read_from_device_to_deque(&mut data)?;
        }

        if !data.is_empty() {
            let (a, b) = data.as_slices();
            sink(a)?;
            sink(b)?;
            frames_written += (data.len() / bytes_per_frame) as u64;
            data.clear();
        } else {
            // Nothing arrived: write silence up to (almost) "now", so the sound stays
            // in sync with the video instead of the quiet parts being skipped.
            let now = (started.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
            let target = now.saturating_sub(LATENCY_FRAMES);
            let missing = target.saturating_sub(frames_written) as usize;
            write_silence(sink, missing)?;
            frames_written += missing as u64;
        }
    }
}

// ---------------------------------------------------------------------------
// Device lists for Settings
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct AudioDevice {
    id: String,
    name: String,
}

#[derive(Serialize)]
pub struct DeviceList {
    /// Name of the current Windows default, shown as "Windows default (…)".
    default_name: Option<String>,
    devices: Vec<AudioDevice>,
}

/// All active playback devices (`outputs = true`) or microphones (`false`).
pub fn list_devices(outputs: bool) -> Result<DeviceList, String> {
    let _ = wasapi::initialize_mta();
    let direction = if outputs { Direction::Render } else { Direction::Capture };
    let enumerator = DeviceEnumerator::new().map_err(|e| e.to_string())?;
    let collection = enumerator
        .get_device_collection(direction)
        .map_err(|e| e.to_string())?;
    let mut devices: Vec<AudioDevice> = (&collection)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            Some(AudioDevice {
                id: d.get_id().ok()?,
                name: d.get_friendlyname().ok()?,
            })
        })
        .collect();
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    let default_name = enumerator
        .get_default_device(direction)
        .ok()
        .and_then(|d| d.get_friendlyname().ok());
    Ok(DeviceList { default_name, devices })
}

// ---------------------------------------------------------------------------
// Level test (the "Test" buttons in Settings)
// ---------------------------------------------------------------------------

/// The running level test, if any. Setting the flag stops it.
#[derive(Default)]
pub struct LevelTest(Mutex<Option<Arc<AtomicBool>>>);

/// Captures from `source` and sends "audio-level" events (0.0 – 1.0) about 20 times
/// a second, so Settings can show a live level bar. Stops after 60 seconds at most.
pub fn start_test(app: &AppHandle, source: Source) {
    stop_test(app);
    let stop = Arc::new(AtomicBool::new(false));
    *app.state::<LevelTest>().0.lock().unwrap() = Some(stop.clone());

    let app = app.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut last = Instant::now();
        let mut peak = 0f32;
        let mut timed_out = false;
        let result = capture(&source, None, &mut |bytes| {
            if stop.load(Ordering::Relaxed) {
                return Err(io::Error::other("stopped"));
            }
            if started.elapsed() > Duration::from_secs(60) {
                timed_out = true;
                return Err(io::Error::other("timeout"));
            }
            for sample in bytes.chunks_exact(4) {
                let v = f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
                peak = peak.max(v.abs());
            }
            if last.elapsed() >= Duration::from_millis(50) {
                let _ = app.emit("audio-level", peak.min(1.0));
                peak = 0.0;
                last = Instant::now();
            }
            Ok(())
        });
        // A real problem (not "stopped" by the user or the 60 s limit): tell the UI.
        if let Err(e) = result {
            if !stop.load(Ordering::Relaxed) && !timed_out {
                let _ = app.emit("audio-level-error", e.to_string());
            }
        }
        // Tell the UI the test ended, unless a newer test already replaced this one.
        let state = app.state::<LevelTest>();
        let mut current = state.0.lock().unwrap();
        let replaced = current.as_ref().is_some_and(|c| !Arc::ptr_eq(c, &stop));
        if !replaced {
            *current = None;
            let _ = app.emit("audio-level-stopped", ());
        }
    });
}

pub fn stop_test(app: &AppHandle) {
    if let Some(stop) = app.state::<LevelTest>().0.lock().unwrap().take() {
        stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod sync_test {
    // A/V sync measurement: flashes the screen (inverts colours) and beeps at the same
    // moment while recording through Orbit's normal pipeline (gate + Background).
    // Off by default because it flashes the screen. Run it on purpose with:
    //   ORBIT_TEST_OUT=C:/path/out.mp4 cargo test --lib av_sync -- --ignored --nocapture
    // then compare the flash (video brightness jump) and the beep (audio) times.
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(h: isize) -> isize;
        fn ReleaseDC(h: isize, dc: isize) -> i32;
        fn GetSystemMetrics(i: i32) -> i32;
        fn InvalidateRect(h: isize, r: *const u8, erase: i32) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn PatBlt(dc: isize, x: i32, y: i32, w: i32, h: i32, rop: u32) -> i32;
    }

    #[test]
    #[ignore]
    fn av_sync() {
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY};
        static BEEP: &[u8] = include_bytes!("../sounds/saved.wav");
        let out = std::env::var("ORBIT_TEST_OUT").unwrap();
        let wait: u64 = std::env::var("ORBIT_TEST_WAIT").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
        // Exactly like a real recording with game sound + mic: AvSync + Background.
        let av = std::sync::Arc::new(super::AvSync::default());
        let pipes = vec![
            super::start(super::Source::System(None), av.clone()).unwrap(),
            super::start(super::Source::Mic(None), av.clone()).unwrap(),
        ];
        let mut cmd = crate::common::ffmpeg();
        cmd.arg("-y");
        crate::common::add_capture_args(&mut cmd, &pipes, &crate::settings::Settings::default());
        let raw = format!("{out}.raw.mkv");
        cmd.args(["-t", "8"]).arg(&raw);
        let bg = crate::common::Background::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::time::Instant::now();
        bg.start(cmd, Some((av.clone(), 60)), move |r| { let _ = tx.send(r); }).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(wait));
        unsafe {
            let dc = GetDC(0);
            PlaySoundW(BEEP.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC);
            PatBlt(dc, 0, 0, GetSystemMetrics(0), GetSystemMetrics(1), 0x0055_0009); // DSTINVERT
            ReleaseDC(0, dc);
        }
        println!("FLASH+BEEP at {:.3}s after spawn", spawned.elapsed().as_secs_f64());
        std::thread::sleep(std::time::Duration::from_millis(350));
        unsafe { InvalidateRect(0, std::ptr::null(), 1) };
        let r = rx.recv().unwrap();
        assert!(r.is_ok(), "{r:?}");
        // Save like Orbit does: sound lined up with the video.
        println!("OFFSET {:.3}s", av.offset());
        let mut save = crate::common::ffmpeg();
        save.arg("-y");
        crate::common::add_aligned_inputs(&mut save, &["-i".as_ref(), raw.as_ref()], av.offset());
        let o = save.args(["-c", "copy"]).arg(&out).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    }
}
