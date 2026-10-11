//! netfoo-helper: the only part of netfoo that runs as root.
//!
//! The app (running as the user) sends it a [`netfoo_core::Request`] over the
//! system bus. The helper asks polkit whether the caller may make that kind
//! of change (polkit shows the password prompt), then applies it.
//!
//! It isn't started by hand: D-Bus starts it the first time the app calls it
//! (see `linux/io.netfoo.Helper1.service`), and it exits after a couple of
//! minutes with nothing to do. `scripts/install-helper-dev.sh` installs a
//! development build.

mod apply;
mod files;
mod services;

#[cfg(target_os = "linux")]
mod bus;
#[cfg(target_os = "linux")]
mod polkit;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    match bus::serve_until_idle() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("netfoo-helper: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn main() -> std::process::ExitCode {
    eprintln!("netfoo-helper only runs on Linux so far.");
    std::process::ExitCode::FAILURE
}
