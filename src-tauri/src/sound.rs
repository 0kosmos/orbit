use crate::settings;
use tauri::AppHandle;
use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

// The sounds are built into orbit.exe, so there are no extra files to ship.
static SAVED: &[u8] = include_bytes!("../sounds/saved.wav");
static START: &[u8] = include_bytes!("../sounds/start.wav");
static ERROR: &[u8] = include_bytes!("../sounds/error.wav");

pub enum Sound {
    Saved,
    Start,
    Error,
}

/// Play a short sound in the background (doesn't wait for it to finish).
/// Uses Windows' own PlaySound, so no audio library and nothing running when silent.
pub fn play(app: &AppHandle, sound: Sound) {
    if !settings::get(app).sounds {
        return;
    }
    let data = match sound {
        Sound::Saved => SAVED,
        Sound::Start => START,
        Sound::Error => ERROR,
    };
    // SAFETY: the sound data is a valid WAV that lives for the whole program.
    unsafe {
        PlaySoundW(
            data.as_ptr() as *const u16,
            std::ptr::null_mut(),
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}
