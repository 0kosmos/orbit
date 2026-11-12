//! System tray icon, and opening the window again after it was closed.
//!
//! Closing Orbit's window destroys it completely (the WebView2 processes go away, so
//! the window costs no memory while you play). Orbit keeps running in the tray with
//! hotkeys, recording and the replay buffer still working.

use crate::{recorder, replay, settings};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewWindowBuilder};

/// Show the window: bring it to the front, or create it again if it was closed.
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    // Same size/title/background as in tauri.conf.json.
    let Some(config) = app.config().app.windows.first().cloned() else { return };
    match WebviewWindowBuilder::from_config(app, &config).and_then(|b| b.build()) {
        Ok(window) => {
            crate::apply_window_theme(app);
            let _ = window.set_focus();
            crate::stats::ensure_running(app);
        }
        Err(e) => eprintln!("Could not open the window: {e}"),
    }
}

/// The icon next to the clock, with its right-click menu.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Orbit", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Start / stop recording", true, None::<&str>)?;
    let replay_toggle =
        MenuItem::with_id(app, "replay", "Replay buffer on / off", true, None::<&str>)?;
    let save = MenuItem::with_id(app, "save", "Save replay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Orbit", true, None::<&str>)?;
    let separator = || PredefinedMenuItem::separator(app);
    let menu = Menu::with_items(
        app,
        &[&open, &separator()?, &record, &replay_toggle, &save, &separator()?, &quit],
    )?;

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Orbit")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            let app = app.clone();
            match event.id.as_ref() {
                "open" => show_main_window(&app),
                // Off the main thread, like the hotkeys.
                "record" => {
                    std::thread::spawn(move || recorder::toggle(&app));
                }
                "replay" => {
                    std::thread::spawn(move || replay::toggle(&app));
                }
                "save" => replay::save(app),
                "quit" => app.exit(0),
                _ => {}
            }
        })
        // Left click on the icon opens the window.
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Turn "Start with Windows" on or off (a normal entry in Windows' startup apps).
pub fn set_autostart(app: &AppHandle, on: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    let result = if on { launcher.enable() } else { launcher.disable() };
    result.map_err(|e| format!("Could not change \"Start with Windows\": {e}"))
}

/// True when Windows started Orbit at login (then it stays in the tray, no window).
pub fn started_minimized() -> bool {
    std::env::args().any(|a| a == "--minimized")
}

/// Whether closing the window should keep Orbit running.
pub fn keep_running(app: &AppHandle) -> bool {
    settings::get(app).close_to_tray
}
