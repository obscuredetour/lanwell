//! lanwell's desktop app. The commands here are thin: what a share is lives in
//! lanwell-core, and every change is made by the privileged helper
//! (lanwell-helper), which this process asks over D-Bus.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod discovery;
#[cfg(target_os = "linux")]
mod helper;
mod mounts;
mod updates;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::system_info,
            commands::load_state,
            commands::local_users,
            commands::file_server_status,
            commands::helper_status,
            commands::install_helper,
            commands::uninstall_helper,
            commands::apply,
            commands::mount_statuses,
            commands::list_server_shares,
            commands::list_server_exports,
            commands::find_servers,
            commands::open_mount,
            updates::update_status,
            updates::set_update_checks,
            updates::open_release_notes,
        ])
        .run(tauri::generate_context!())
        .expect("lanwell couldn't start");
}
