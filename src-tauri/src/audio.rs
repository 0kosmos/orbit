//! Game / PC sound capture.
//!
//! FFmpeg on Windows can't record "what you hear" by itself, so Orbit does it:
//! Windows "loopback" capture (WASAPI) records whatever is sent to your default
//! speakers or headphones (the same method OBS uses; it never touches the game).
//! The sound is passed to FFmpeg through a named pipe, a small in-memory connection
//! between two programs, which FFmpeg reads like a file.

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::os::windows::io::FromRawHandle;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use wasapi::{DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_OUTBOUND;
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT,
};

/// The format FFmpeg is told to expect: 48 kHz, stereo, 32-bit float.
pub const SAMPLE_RATE: usize = 48_000;
pub const CHANNELS: usize = 2;
const BYTES_PER_FRAME: usize = CHANNELS * 4;

static NEXT_PIPE: AtomicU64 = AtomicU64::new(0);

/// Creates a pipe and starts a thread that streams PC sound into it once FFmpeg
/// connects. Returns the pipe's name, to pass to FFmpeg as an input.
/// The thread ends by itself when FFmpeg stops.
pub fn start_loopback() -> Result<String, String> {
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
            1 << 20, // 1 MB buffer (about 2.7 seconds of sound)
            0,
            0,
            std::ptr::null(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err("Could not set up audio capture".into());
    }

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
        // An error here normally just means FFmpeg stopped and closed the pipe.
        let _ = capture(&mut pipe);
    });
    Ok(name)
}

/// If FFmpeg failed before opening the pipe, the thread above would wait forever.
/// Connecting to the pipe ourselves wakes it up so it can finish.
pub fn release(name: &str) {
    let _ = File::open(name);
}

fn capture(pipe: &mut File) -> Result<(), Box<dyn std::error::Error>> {
    let _ = wasapi::initialize_mta();

    // "Render" = the default playback device; opening it for capture = loopback.
    let device = DeviceEnumerator::new()?.get_default_device(Direction::Render)?;
    let mut client = device.get_iaudioclient()?;
    let format = WaveFormat::new(32, 32, SampleType::Float, SAMPLE_RATE, CHANNELS, None);
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
    let silence = vec![0u8; BYTES_PER_FRAME * SAMPLE_RATE / 10];

    client.start_stream()?;
    let started = Instant::now();

    loop {
        // Windows wakes us when sound is available. When nothing is playing it sends
        // nothing at all, so we also wake every 20 ms to fill in silence.
        let _ = event.wait_for_event(20);

        while capture.get_next_packet_size()?.unwrap_or(0) > 0 {
            capture.read_from_device_to_deque(&mut data)?;
        }

        if !data.is_empty() {
            let (a, b) = data.as_slices();
            pipe.write_all(a)?;
            pipe.write_all(b)?;
            frames_written += (data.len() / BYTES_PER_FRAME) as u64;
            data.clear();
        } else {
            // Nothing playing: write silence up to "now", so the sound stays in sync
            // with the video instead of the quiet parts being skipped.
            let expected = (started.elapsed().as_secs_f64() * SAMPLE_RATE as f64) as u64;
            let mut missing = expected.saturating_sub(frames_written) as usize;
            while missing > 0 {
                let frames = missing.min(silence.len() / BYTES_PER_FRAME);
                pipe.write_all(&silence[..frames * BYTES_PER_FRAME])?;
                frames_written += frames as u64;
                missing -= frames;
            }
        }
    }
}
