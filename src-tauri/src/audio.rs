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

/// Creates a pipe and starts a thread that streams the sound into it once FFmpeg
/// connects. The thread ends by itself when FFmpeg stops.
pub fn start(source: Source) -> Result<Pipe, String> {
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
        let _ = capture(&source, &mut |bytes| pipe.write_all(bytes));
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

    client.start_stream()?;
    let started = Instant::now();

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
            let mut missing = target.saturating_sub(frames_written) as usize;
            while missing > 0 {
                let frames = missing.min(silence.len() / bytes_per_frame);
                sink(&silence[..frames * bytes_per_frame])?;
                frames_written += frames as u64;
                missing -= frames;
            }
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
        let result = capture(&source, &mut |bytes| {
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
