mod common;
mod media;
mod recorder;
mod replay;
mod screenshot;
mod settings;

use tauri::{AppHandle, RunEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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
    let length_changed = new.replay_seconds != settings::get(&app).replay_seconds;
    settings::set(&app, new)?;
    if length_changed {
        replay::restart(&app);
    }
    Ok(())
}

#[tauri::command]
fn list_media(app: AppHandle) -> Result<Vec<media::MediaItem>, String> {
    media::list(&app)
}

/// `async` + `spawn_blocking`: runs FFmpeg on a worker thread so the window never freezes.
#[tauri::command]
async fn thumbnail(app: AppHandle, path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || media::thumbnail(&app, &path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
fn open_media(app: AppHandle, path: String) -> Result<(), String> {
    media::open(&app, &path)
}

#[tauri::command]
fn open_media_folder(app: AppHandle) -> Result<(), String> {
    media::open_folder(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Default hotkeys (configurable in Settings later).
    let screenshot_key = Shortcut::new(Some(Modifiers::ALT), Code::F12);
    let record_key = Shortcut::new(Some(Modifiers::ALT), Code::F9);
    let replay_key = Shortcut::new(Some(Modifiers::ALT), Code::F10);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if *shortcut == screenshot_key {
                        screenshot::take(app.clone());
                    } else if *shortcut == record_key {
                        recorder::toggle(app);
                    } else if *shortcut == replay_key {
                        replay::save(app.clone());
                    }
                })
                .build(),
        )
        .manage(settings::SettingsState::default())
        .manage(recorder::Recorder::default())
        .manage(replay::Replay::default())
        .invoke_handler(tauri::generate_handler![
            toggle_recording,
            is_recording,
            toggle_replay,
            is_replay_running,
            save_replay,
            get_settings,
            set_settings,
            list_media,
            thumbnail,
            open_media,
            open_media_folder
        ])
        .setup(move |app| {
            let handle = app.handle();
            settings::load(handle);

            // If another app already uses a hotkey, don't crash; just report it.
            for key in [screenshot_key, record_key, replay_key] {
                if let Err(e) = app.global_shortcut().register(key) {
                    eprintln!("Could not register hotkey {key}: {e}");
                }
            }

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
