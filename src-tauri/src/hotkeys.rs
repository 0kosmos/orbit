//! Global hotkeys (work even while a game has focus).
//! Uses Windows' normal RegisterHotKey, so Windows tells Orbit when a combination is
//! pressed: no keyboard hooks, no polling, anti-cheat safe.

use crate::settings::Settings;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[derive(Clone, Copy)]
pub enum Action {
    Screenshot,
    Record,
    SaveReplay,
    Overlay,
    Snip,
}

/// The hotkeys currently in use, and what each one does.
#[derive(Default)]
pub struct Hotkeys(Mutex<Vec<(Shortcut, Action)>>);

/// What a pressed hotkey should do.
pub fn action_for(app: &AppHandle, shortcut: &Shortcut) -> Option<Action> {
    let keys = app.state::<Hotkeys>();
    let keys = keys.0.lock().unwrap();
    keys.iter().find(|(k, _)| k == shortcut).map(|(_, a)| *a)
}

fn parse(text: &str) -> Result<Shortcut, String> {
    text.parse::<Shortcut>()
        .map_err(|_| format!("\"{text}\" is not a valid hotkey"))
}

/// Switch to the hotkeys in `settings`. If one can't be used (invalid, a duplicate,
/// or taken by another app), nothing changes and the error says why.
pub fn apply(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let mut wanted = vec![
        (parse(&settings.hotkey_screenshot)?, Action::Screenshot),
        (parse(&settings.hotkey_record)?, Action::Record),
        (parse(&settings.hotkey_replay)?, Action::SaveReplay),
        (parse(&settings.hotkey_snip)?, Action::Snip),
    ];
    // The overlay hotkey only exists while the overlay is switched on.
    if settings.overlay_enabled {
        wanted.push((parse(&settings.hotkey_overlay)?, Action::Overlay));
    }
    for (i, (a, _)) in wanted.iter().enumerate() {
        if wanted[..i].iter().any(|(b, _)| a == b) {
            return Err("Two actions can't use the same hotkey.".into());
        }
    }

    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    for (key, _) in &wanted {
        if gs.register(*key).is_err() {
            // Put the old hotkeys back.
            resume(app);
            return Err(
                "That hotkey is already used by another app (or Windows). Try another one.".into(),
            );
        }
    }
    *app.state::<Hotkeys>().0.lock().unwrap() = wanted;
    Ok(())
}

/// At startup: use the saved hotkeys. Any that can't be used are skipped
/// (and reported in the terminal) so the others still work.
pub fn load(app: &AppHandle, settings: &Settings) {
    let defaults = Settings::default();
    let pick = |saved: &str, default: &str| parse(saved).or_else(|_| parse(default)).ok();
    let mut keys = vec![
        (pick(&settings.hotkey_screenshot, &defaults.hotkey_screenshot), Action::Screenshot),
        (pick(&settings.hotkey_record, &defaults.hotkey_record), Action::Record),
        (pick(&settings.hotkey_replay, &defaults.hotkey_replay), Action::SaveReplay),
        (pick(&settings.hotkey_snip, &defaults.hotkey_snip), Action::Snip),
    ];
    if settings.overlay_enabled {
        keys.push((pick(&settings.hotkey_overlay, &defaults.hotkey_overlay), Action::Overlay));
    }
    *app.state::<Hotkeys>().0.lock().unwrap() = keys
        .into_iter()
        .filter_map(|(key, action)| Some((key?, action)))
        .collect();
    resume(app);
}

/// Turn hotkeys off while the user is choosing a new one in Settings,
/// so pressing e.g. Alt+F12 there doesn't also take a screenshot.
pub fn pause(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

/// Turn the current hotkeys (back) on. Safe to call when they're already on.
pub fn resume(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
    let keys: Vec<Shortcut> = app
        .state::<Hotkeys>()
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(k, _)| *k)
        .collect();
    let gs = app.global_shortcut();
    for key in keys {
        if let Err(e) = gs.register(key) {
            eprintln!("Could not register hotkey {key}: {e}");
        }
    }
}
