//! netfoo-helper: the only part of netfoo that runs as root.
//!
//! The app (running as the user) sends it a [`netfoo_core::Request`] over the
//! system bus. The helper asks polkit whether the caller may make that kind
//! of change (polkit shows the password prompt), then applies it.
//!
//! It isn't started by hand: D-Bus starts it the first time the app calls it
//! (see `linux/io.netfoo.Helper1.service`), and it exits after a couple of
//! minutes with nothing to do. It's put in place by a package, or by its own
//! `install` command (see `install.rs`).
//!
//! ```text
//! netfoo-helper              serve requests (what D-Bus runs)
//! netfoo-helper install      install this binary and its D-Bus, systemd and polkit files (root)
//! netfoo-helper uninstall    remove them again (root)
//! netfoo-helper --version    print the version (no root needed; the app checks it)
//! ```

mod apply;
mod files;
mod mounts;
mod services;
mod users;

#[cfg(target_os = "linux")]
mod bus;
#[cfg(target_os = "linux")]
mod install;
#[cfg(target_os = "linux")]
mod polkit;

use std::process::ExitCode;

#[cfg(target_os = "linux")]
fn main() -> ExitCode {
    let result = match std::env::args().nth(1).as_deref() {
        None => bus::serve_until_idle().map_err(|e| e.to_string()),
        Some("install") => install::install().map_err(|e| e.to_string()),
        Some("uninstall") => install::uninstall().map_err(|e| e.to_string()),
        Some("--version") => {
            println!("netfoo-helper {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some(other) => Err(format!(
            "unknown command \"{other}\" (try install, uninstall or --version)"
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("netfoo-helper: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> ExitCode {
    eprintln!("netfoo-helper only runs on Linux so far.");
    ExitCode::FAILURE
}
