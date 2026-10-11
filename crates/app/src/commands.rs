//! The commands the UI calls (through `ui/src/api.ts`, its only way in).
//!
//! Errors are returned as messages to show as they are.

use netfoo_core::system::{self, HelperStatus, SystemInfo};
use netfoo_core::{MountStatus, Request, State};
use tauri_plugin_opener::OpenerExt;

use crate::mounts;

/// What's installed, so the UI can explain what's missing.
#[tauri::command]
pub fn system_info() -> SystemInfo {
    SystemInfo::detect()
}

/// Everything netfoo manages. Read directly (it's world-readable), so showing
/// it never asks for a password.
#[tauri::command]
pub fn load_state() -> Result<State, String> {
    #[cfg(target_os = "linux")]
    return State::load(&netfoo_core::linux::Paths::system().state).map_err(|e| e.to_string());

    #[cfg(not(target_os = "linux"))]
    Ok(State::default())
}

/// The people with accounts here, who shares can be opened to.
#[tauri::command]
pub fn local_users() -> Vec<String> {
    system::local_users()
}

/// Whether the privileged helper is installed and up to date. Asks the
/// installed helper its version; doesn't start it as root.
#[tauri::command]
pub fn helper_status() -> HelperStatus {
    let bundled = std::env::current_exe()
        .map(|app| app.with_file_name("netfoo-helper"))
        .ok()
        .filter(|helper| helper.is_file());
    HelperStatus::detect(env!("CARGO_PKG_VERSION"), bundled.as_deref())
}

/// Installs or updates the helper, after the system asks for an
/// administrator's password.
#[tauri::command]
pub async fn install_helper() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return crate::helper::install().await;

    #[cfg(not(target_os = "linux"))]
    Err("netfoo can't install its helper on this system yet.".into())
}

/// Removes the helper, after the system asks for an administrator's password.
/// Shares stay as they are; netfoo just can't change them until it's back.
#[tauri::command]
pub async fn uninstall_helper() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return crate::helper::uninstall().await;

    #[cfg(not(target_os = "linux"))]
    Err("netfoo has no helper on this system to remove.".into())
}

/// Asks the helper to make a change. The system shows a password prompt if
/// one is needed.
#[tauri::command]
pub async fn apply(request: Request) -> Result<(), String> {
    create_mount_point_at_home(&request)?;

    #[cfg(target_os = "linux")]
    return crate::helper::apply(&request).await;

    #[cfg(not(target_os = "linux"))]
    {
        let _ = request;
        Err("netfoo can't change shares on this system yet.".into())
    }
}

/// A mount in the user's home folder gets its folder made here, as the user,
/// so it's theirs (the helper, as root, would make it root's). Elsewhere the
/// helper makes it.
fn create_mount_point_at_home(request: &Request) -> Result<(), String> {
    let Request::SetMount { mount, .. } = request else {
        return Ok(());
    };
    let Ok(home) = std::env::var("HOME") else {
        return Ok(());
    };
    let path = mount.mount_point.as_str();
    if path.starts_with(&format!("{}/", home.trim_end_matches('/'))) {
        std::fs::create_dir_all(path).map_err(|e| format!("Couldn't create {path}: {e}"))?;
    }
    Ok(())
}

/// What each mount is doing now (mounted, waiting to be opened, failed).
#[tauri::command]
pub fn mount_statuses() -> Result<Vec<MountStatus>, String> {
    let state = load_state()?;
    Ok(mounts::statuses(&state.mounts))
}

/// The shares on a server, to choose one to mount. Signs in as `username`
/// if given, or as a guest.
#[tauri::command]
pub async fn list_server_shares(
    host: String,
    username: Option<String>,
    password: Option<String>,
) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        mounts::list_shares(&host, username.as_deref(), password.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The folders an NFS server exports, to choose one to mount.
#[tauri::command]
pub async fn list_server_exports(host: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || mounts::list_exports(&host))
        .await
        .map_err(|e| e.to_string())?
}

/// Opens a mount's folder in the file manager (which mounts it, if it waits
/// to be opened). Only netfoo's own mounts can be opened this way.
#[tauri::command]
pub fn open_mount(app: tauri::AppHandle, mount_point: String) -> Result<(), String> {
    let state = load_state()?;
    if !state
        .mounts
        .iter()
        .any(|m| m.mount.mount_point.as_str() == mount_point)
    {
        return Err(format!("{mount_point} isn't one of netfoo's mounts."));
    }
    app.opener()
        .open_path(mount_point, None::<&str>)
        .map_err(|e| e.to_string())
}
