//! The commands the UI calls (through `ui/src/api.ts`, its only way in).
//!
//! Errors are returned as messages to show as they are.

use netfoo_core::system::{self, SystemInfo};
use netfoo_core::{Request, State};

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

/// Asks the helper to make a change. The system shows a password prompt if
/// one is needed.
#[tauri::command]
pub async fn apply(request: Request) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return crate::helper::apply(&request).await;

    #[cfg(not(target_os = "linux"))]
    {
        let _ = request;
        Err("netfoo can't change shares on this system yet.".into())
    }
}
