//! In-game overlay: a see-through, always-on-top window over the game (no injection,
//! so anti-cheat safe). It's created only when opened and destroyed when closed,
//! so it costs nothing while hidden.

use crate::{screenshot, stats, tray};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

const LABEL: &str = "overlay";

/// Hotkey: open the overlay, or close it if it's open.
pub fn toggle(app: &AppHandle) {
    if app.get_webview_window(LABEL).is_some() {
        close(app);
    } else {
        // Creating a window must not run on the thread that handles the hotkey.
        let app = app.clone();
        std::thread::spawn(move || open(&app));
    }
}

pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.destroy();
    }
}

fn open(app: &AppHandle) {
    // Cover the monitor the mouse is on (normally the one with the game).
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let pos = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);

    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("overlay".into()))
        .title("Orbit overlay")
        .transparent(true)
        .decorations(false)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .position(pos.x, pos.y)
        .inner_size(size.width, size.height)
        // Invisible until the page is drawn (no half-built frame over the game); see `ready`.
        .visible(false)
        // An invisible window can't keep focus; it gets focus when it's shown.
        .focused(false)
        .build();
    let window = match built {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Could not open the overlay: {e}");
            return;
        }
    };

    // Switching to another app (Alt+Tab, clicking the game) closes the overlay.
    // Only once it's VISIBLE and has had focus: while it's still invisible, Windows
    // can take focus away, and that must not close it before it ever appears.
    let had_focus = Arc::new(AtomicBool::new(false));
    let handle = app.clone();
    let me = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(focused) = event {
            let visible = me.is_visible().unwrap_or(false);
            if *focused && visible {
                had_focus.store(true, Ordering::Relaxed);
            } else if !*focused && visible && had_focus.load(Ordering::Relaxed) {
                close(&handle);
            }
        }
    });
    // Safety net: show it anyway if the page never reports "ready".
    let w = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2500));
        if !w.is_visible().unwrap_or(true) {
            let _ = w.show();
            let _ = w.set_focus();
        }
    });
    stats::ensure_running(app);
}

/// The overlay page is drawn: show it.
pub fn ready(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// "All settings" in the overlay: open Orbit on the Settings page.
pub fn open_settings(app: &AppHandle) {
    *app.state::<PendingTab>().0.lock().unwrap() = Some("settings".into());
    open_main(app);
    // If the main window was already open, switch it right away.
    let _ = app.emit("open-tab", "settings");
}

/// A page the main window should open next time it loads (e.g. "settings").
#[derive(Default)]
pub struct PendingTab(pub std::sync::Mutex<Option<String>>);

/// Screenshot button in the overlay: hide the overlay first so it isn't in the picture.
pub fn screenshot_without_overlay(app: &AppHandle) {
    close(app);
    let app = app.clone();
    std::thread::spawn(move || {
        // Give Windows a moment to remove the overlay from the screen.
        std::thread::sleep(Duration::from_millis(250));
        screenshot::take(app);
    });
}

/// "Open Orbit" in the overlay.
pub fn open_main(app: &AppHandle) {
    close(app);
    tray::show_main_window(app);
}
