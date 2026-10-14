//! Telling people a new lanwell is out. The UI asks at launch and once a day;
//! this fetches the latest release's `latest.json` (see lanwell_core::updates),
//! unless this copy isn't install.sh's or checking is turned off
//! (`updates.json` in the app's config folder). Nothing is downloaded or
//! installed: the banner says to run install.sh again.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use lanwell_core::updates::{self, UpdateStatus};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    check: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self { check: true }
    }
}

/// Whether a newer release is out, if this copy is install.sh's and checking
/// is on.
#[tauri::command]
pub async fn update_status(app: AppHandle) -> UpdateStatus {
    if !managed() {
        return UpdateStatus::Unmanaged;
    }
    if !load_settings(&app).check {
        return UpdateStatus::Off;
    }
    tauri::async_runtime::spawn_blocking(|| fetch(updates::LATEST_URL))
        .await
        .map_err(|e| e.to_string())
        .flatten()
        .and_then(|latest| UpdateStatus::from_latest(&latest, env!("CARGO_PKG_VERSION")))
        .unwrap_or_else(|message| UpdateStatus::Unreachable { message })
}

/// Turns checking for updates on or off.
#[tauri::command]
pub fn set_update_checks(app: AppHandle, on: bool) -> Result<(), String> {
    let path = settings_path(&app)?;
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    }
    let settings = serde_json::to_vec_pretty(&Settings { check: on }).map_err(|e| e.to_string())?;
    fs::write(&path, settings).map_err(|e| format!("Couldn't save {}: {e}", path.display()))
}

/// Opens the latest release's notes in the browser.
#[tauri::command]
pub fn open_release_notes(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url(updates::RELEASE_NOTES_URL, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Whether install.sh installed this copy, so running it again updates it.
fn managed() -> bool {
    let (Ok(exe), Some(home)) = (std::env::current_exe(), std::env::home_dir()) else {
        return false;
    };
    updates::installed_by_script(&exe, &home)
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("updates.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    settings_path(app)
        .ok()
        .and_then(|path| fs::read(path).ok())
        .and_then(|json| serde_json::from_slice(&json).ok())
        .unwrap_or_default()
}

fn fetch(url: &str) -> Result<String, String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(concat!("lanwell/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent();
    agent
        .get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_string())
        .map_err(|e| format!("Couldn't check for updates: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fetches the real latest.json (needs the network, and a published
    /// release): `cargo test -p lanwell -- --ignored latest_release`.
    #[test]
    #[ignore]
    fn latest_release_is_readable() {
        let latest = fetch(updates::LATEST_URL).unwrap();
        println!("{latest}");
        UpdateStatus::from_latest(&latest, "0.0.0").unwrap();
    }
}
