mod common;
mod recorder;
mod screenshot;

use tauri::RunEvent;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[tauri::command]
fn toggle_recording(app: tauri::AppHandle) {
    recorder::toggle(&app);
}

#[tauri::command]
fn is_recording(app: tauri::AppHandle) -> bool {
    recorder::is_recording(&app)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Default hotkeys (configurable in Settings later).
    let screenshot_key = Shortcut::new(Some(Modifiers::ALT), Code::F12);
    let record_key = Shortcut::new(Some(Modifiers::ALT), Code::F9);

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
                    }
                })
                .build(),
        )
        .manage(recorder::Recorder::default())
        .invoke_handler(tauri::generate_handler![toggle_recording, is_recording])
        .setup(move |app| {
            // If another app already uses a hotkey, don't crash; just report it.
            for key in [screenshot_key, record_key] {
                if let Err(e) = app.global_shortcut().register(key) {
                    eprintln!("Could not register hotkey {key}: {e}");
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Closing Orbit while recording: finish the file instead of leaving FFmpeg running.
            if let RunEvent::Exit = event {
                recorder::stop(app);
            }
        });
}
