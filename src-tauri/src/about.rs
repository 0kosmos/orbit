//! "About" in Settings: open the GitHub page (for updates) and uninstall Orbit.

use crate::{recorder, replay, tray};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

/// Orbit's GitHub repository (releases = updates).
pub const REPO_URL: &str = "https://github.com/0kosmos/orbit";

/// Open a page of Orbit's GitHub repository in the browser (only that site).
pub fn open_github(app: &AppHandle, url: &str) -> Result<(), String> {
    if !url.starts_with(REPO_URL) {
        return Err("Only Orbit's GitHub page can be opened.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// The uninstaller that Orbit's installer puts next to orbit.exe.
fn uninstaller() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let path = exe.parent()?.join("uninstall.exe");
    path.exists().then_some(path)
}

/// True when Orbit was installed with its installer (so it can be uninstalled).
pub fn is_installed() -> bool {
    uninstaller().is_some()
}

/// Uninstall Orbit: stop recording, remove "Start with Windows", optionally delete
/// settings + caches (never the screenshots / clips / recordings), then start
/// Windows' normal uninstaller and quit.
pub fn uninstall(app: &AppHandle, delete_data: bool) -> Result<(), String> {
    let Some(uninstaller) = uninstaller() else {
        return Err(
            "This copy of Orbit wasn't installed with the installer (e.g. the development \
             version), so there's nothing to uninstall."
                .into(),
        );
    };

    recorder::stop(app);
    replay::stop(app);
    let _ = tray::set_autostart(app, false);

    if delete_data {
        // Settings + thumbnails + replay buffer pieces. Your media folder is NOT touched.
        if let Ok(dir) = app.path().app_config_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
        if let Ok(dir) = app.path().app_cache_dir() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    std::process::Command::new(uninstaller)
        .spawn()
        .map_err(|e| format!("Could not start the uninstaller: {e}"))?;
    app.exit(0);
    Ok(())
}
