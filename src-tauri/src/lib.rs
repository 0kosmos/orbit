mod audio;
mod common;
mod game;
mod hotkeys;
mod media;
mod recorder;
mod replay;
mod screenshot;
mod settings;
mod sound;

use tauri::{AppHandle, Emitter, RunEvent};
use tauri_plugin_global_shortcut::ShortcutState;

#[tauri::command]
fn toggle_recording(app: AppHandle) {
    recorder::toggle(&app);
}

#[tauri::command]
fn is_recording(app: AppHandle) -> bool {
    recorder::is_recording(&app)
}

#[tauri::command]
fn toggle_replay(app: AppHandle) {
    replay::toggle(&app);
}

#[tauri::command]
fn is_replay_running(app: AppHandle) -> bool {
    replay::is_running(&app)
}

#[tauri::command]
fn save_replay(app: AppHandle) {
    replay::save(app);
}

#[tauri::command]
fn get_settings(app: AppHandle) -> settings::Settings {
    settings::get(&app)
}

#[tauri::command]
fn set_settings(app: AppHandle, new: settings::Settings) -> Result<(), String> {
    let old = settings::get(&app);
    // Anything that changes how video/sound is captured restarts a running replay buffer.
    let capture_changed = new.capture_differs(&old);
    let hotkeys_changed = new.hotkey_screenshot != old.hotkey_screenshot
        || new.hotkey_record != old.hotkey_record
        || new.hotkey_replay != old.hotkey_replay;

    // Try new hotkeys first: if one can't be used, nothing is saved.
    if hotkeys_changed {
        hotkeys::apply(&app, &new)?;
    }
    settings::set(&app, new)?;
    if capture_changed {
        replay::restart(&app);
    }
    // Tell the window (e.g. the hotkey hints in the sidebar).
    let _ = app.emit("settings-changed", settings::get(&app));
    Ok(())
}

/// Hotkeys are off while choosing a new one in Settings.
#[tauri::command]
fn pause_hotkeys(app: AppHandle, paused: bool) {
    if paused {
        hotkeys::pause(&app);
    } else {
        hotkeys::resume(&app);
    }
}

#[tauri::command]
fn list_media(app: AppHandle) -> Result<Vec<media::MediaItem>, String> {
    media::list(&app)
}

/// `async` + `spawn_blocking`: runs FFmpeg on a worker thread so the window never freezes.
#[tauri::command]
async fn preview(app: AppHandle, path: String) -> Result<media::Preview, String> {
    tauri::async_runtime::spawn_blocking(move || media::preview(&app, &path))
        .await
        .map_err(|e| e.to_string())?
}

/// Moves the file to the Recycle Bin (on a worker thread; Windows can take a moment).
#[tauri::command]
async fn delete_media(app: AppHandle, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || media::delete(&app, &path))
        .await
        .map_err(|e| e.to_string())?
}

/// Playback devices (`outputs = true`) or microphones. Runs on a worker thread,
/// because Windows' audio system needs its own thread setup.
#[tauri::command]
async fn list_audio_devices(outputs: bool) -> Result<audio::DeviceList, String> {
    tauri::async_runtime::spawn_blocking(move || audio::list_devices(outputs))
        .await
        .map_err(|e| e.to_string())?
}

/// Start the live level bar in Settings. `kind` is "game" or "mic".
#[tauri::command]
fn start_audio_test(app: AppHandle, kind: String, device: Option<String>) {
    let source = match kind.as_str() {
        "mic" => audio::Source::Mic(device),
        _ => audio::Source::System(device),
    };
    audio::start_test(&app, source);
}

#[tauri::command]
fn stop_audio_test(app: AppHandle) {
    audio::stop_test(&app);
}

#[tauri::command]
fn open_media(app: AppHandle, path: String) -> Result<(), String> {
    media::open(&app, &path)
}

#[tauri::command]
fn show_in_folder(app: AppHandle, path: String) -> Result<(), String> {
    media::show_in_folder(&app, &path)
}

#[tauri::command]
fn open_media_folder(app: AppHandle) -> Result<(), String> {
    media::open_folder(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    match hotkeys::action_for(app, shortcut) {
                        Some(hotkeys::Action::Screenshot) => screenshot::take(app.clone()),
                        Some(hotkeys::Action::Record) => recorder::toggle(app),
                        Some(hotkeys::Action::SaveReplay) => replay::save(app.clone()),
                        None => {}
                    }
                })
                .build(),
        )
        .manage(settings::SettingsState::default())
        .manage(hotkeys::Hotkeys::default())
        .manage(recorder::Recorder::default())
        .manage(replay::Replay::default())
        .manage(audio::LevelTest::default())
        .invoke_handler(tauri::generate_handler![
            toggle_recording,
            is_recording,
            toggle_replay,
            is_replay_running,
            save_replay,
            get_settings,
            set_settings,
            list_media,
            preview,
            delete_media,
            open_media,
            open_media_folder,
            show_in_folder,
            list_audio_devices,
            start_audio_test,
            stop_audio_test,
            pause_hotkeys
        ])
        .setup(|app| {
            let handle = app.handle();
            settings::load(handle);
            hotkeys::load(handle, &settings::get(handle));

            if settings::get(handle).replay_autostart {
                replay::start(handle);
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Closing Orbit: finish files instead of leaving FFmpeg running.
            if let RunEvent::Exit = event {
                recorder::stop(app);
                replay::stop(app);
            }
        });
}
