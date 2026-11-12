mod screenshot;

use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Default screenshot hotkey: Alt+F12 (configurable in Settings later).
    let screenshot_key = Shortcut::new(Some(Modifiers::ALT), Code::F12);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed && *shortcut == screenshot_key {
                        screenshot::take(app.clone());
                    }
                })
                .build(),
        )
        .setup(move |app| {
            app.global_shortcut().register(screenshot_key)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
