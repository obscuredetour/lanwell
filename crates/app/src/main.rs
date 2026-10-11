//! netfoo's desktop app. The commands here are thin: what a share is lives in
//! netfoo-core, and every change is made by the privileged helper
//! (netfoo-helper), which this process asks over D-Bus.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
#[cfg(target_os = "linux")]
mod helper;
mod mounts;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::load_state,
            commands::local_users,
            commands::helper_status,
            commands::install_helper,
            commands::uninstall_helper,
            commands::apply,
            commands::mount_statuses,
            commands::list_server_shares,
            commands::list_server_exports,
            commands::open_mount,
        ])
        .run(tauri::generate_context!())
        .expect("netfoo couldn't start");
}
