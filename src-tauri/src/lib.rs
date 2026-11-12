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
mod stats;
mod tray;

use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Theme};
use tauri_plugin_global_shortcut::ShortcutState;

/// Commands that start/stop FFmpeg run on a worker thread, never on the main
/// thread that draws the window (so the window can never freeze while they work).
#[tauri::command]
async fn toggle_recording(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || recorder::toggle(&app)).await;
}

#[tauri::command]
fn is_recording(app: AppHandle) -> bool {
    recorder::is_recording(&app)
}

/// When the running recording started (ms since 1970), or null.
#[tauri::command]
fn recording_since(app: AppHandle) -> Option<u64> {
    recorder::recording_since(&app)
}

/// Total size and count of everything in the save folder (for the Home page).
#[tauri::command]
async fn storage_summary(app: AppHandle) -> Result<(u64, usize), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let items = media::list(&app)?;
        Ok((items.iter().map(|i| i.size()).sum(), items.len()))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn toggle_replay(app: AppHandle) {
    let _ = tauri::async_runtime::spawn_blocking(move || replay::toggle(&app)).await;
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
    let theme_changed = new.theme != old.theme;
    if new.start_with_windows != old.start_with_windows {
        tray::set_autostart(&app, new.start_with_windows)?;
    }
    let folder_changed = new.save_dir != old.save_dir;
    if let (true, Some(dir)) = (folder_changed, &new.save_dir) {
        common::check_writable(dir)?;
    }
    settings::set(&app, new)?;
    if folder_changed {
        common::allow_in_window(&app);
    }
    if theme_changed {
        apply_window_theme(&app);
    }
    if capture_changed {
        replay::restart(&app);
    }
    // Status bar switched on: start measuring.
    stats::ensure_running(&app);
    // Tell the window (e.g. the hotkey hints in the sidebar).
    let _ = app.emit("settings-changed", settings::get(&app));
    Ok(())
}

/// Light or dark Windows title bar (and window background), to match the theme.
pub(crate) fn apply_window_theme(app: &AppHandle) {
    let light = settings::get(app).theme == "light";
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_theme(Some(if light { Theme::Light } else { Theme::Dark }));
        let bg = if light { (244, 244, 245) } else { (0, 0, 0) };
        let _ = window.set_background_color(Some(Color(bg.0, bg.1, bg.2, 255)));
    }
}

/// The save folder in use, and the default one (for Settings).
#[tauri::command]
fn save_folders(app: AppHandle) -> Result<(String, String), String> {
    Ok((
        common::save_root(&app)?.to_string_lossy().into_owned(),
        common::default_root(&app)?.to_string_lossy().into_owned(),
    ))
}

/// Windows' "choose a folder" dialog. None if cancelled.
#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Choose where Orbit saves your screenshots, clips and recordings")
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
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
async fn list_media(app: AppHandle) -> Result<Vec<media::MediaItem>, String> {
    tauri::async_runtime::spawn_blocking(move || media::list(&app))
        .await
        .map_err(|e| e.to_string())?
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

/// Bulk delete (to the Recycle Bin). Returns the paths that were deleted.
#[tauri::command]
async fn delete_media_many(app: AppHandle, paths: Vec<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || media::delete_many(&app, &paths))
        .await
        .map_err(|e| e.to_string())
}

/// Crop / resize a screenshot (on a worker thread). Tells the gallery and the
/// notification about the new image with the usual "screenshot-saved" event.
#[tauri::command]
async fn edit_image(
    app: AppHandle,
    path: String,
    crop: Option<media::Crop>,
    width: Option<u32>,
    format: String,
    replace: bool,
) -> Result<String, String> {
    let saved = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || media::edit_image(&app, &path, crop, width, &format, replace)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("screenshot-saved", saved.clone());
    Ok(saved)
}

/// Trim a clip / recording (on a worker thread). The gallery and notification hear
/// about it through the "replay-saved" event (shown as "Clip saved").
#[tauri::command]
async fn trim_video(
    app: AppHandle,
    path: String,
    start: f64,
    end: f64,
    replace: bool,
) -> Result<String, String> {
    let saved = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || media::trim_video(&app, &path, start, end, replace)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit("replay-saved", saved.clone());
    Ok(saved)
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
        // Must be first: starting Orbit again just opens the running one's window.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        // "Start with Windows" starts Orbit with --minimized (tray only, no window).
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    match hotkeys::action_for(app, shortcut) {
                        Some(hotkeys::Action::Screenshot) => screenshot::take(app.clone()),
                        Some(hotkeys::Action::Record) => {
                            // Off the main thread, so the window keeps drawing.
                            let app = app.clone();
                            std::thread::spawn(move || recorder::toggle(&app));
                        }
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
            recording_since,
            storage_summary,
            toggle_replay,
            is_replay_running,
            save_replay,
            get_settings,
            set_settings,
            list_media,
            preview,
            delete_media,
            delete_media_many,
            edit_image,
            trim_video,
            open_media,
            open_media_folder,
            show_in_folder,
            list_audio_devices,
            start_audio_test,
            stop_audio_test,
            pause_hotkeys,
            save_folders,
            pick_folder
        ])
        .setup(|app| {
            let handle = app.handle();
            settings::load(handle);
            hotkeys::load(handle, &settings::get(handle));
            common::allow_in_window(handle);
            tray::create(handle)?;
            // Started by Windows at login: stay in the tray. Otherwise open the window.
            if !tray::started_minimized() {
                tray::show_main_window(handle);
            }

            if settings::get(handle).replay_autostart {
                let handle = handle.clone();
                std::thread::spawn(move || replay::start(&handle));
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // The window was closed (no exit code): keep running in the tray if wanted.
            // "Quit Orbit" in the tray menu gives an exit code, so that really quits.
            RunEvent::ExitRequested { code: None, api, .. } if tray::keep_running(app) => {
                api.prevent_exit();
            }
            // Quitting: finish files instead of leaving FFmpeg running.
            RunEvent::Exit => {
                recorder::stop(app);
                replay::stop(app);
            }
            _ => {}
        });
}
