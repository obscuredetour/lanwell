//! `netfoo-helper install` and `uninstall`: putting the helper where D-Bus
//! can start it, for a netfoo that didn't come from a package.
//!
//! The app's Install button runs `pkexec netfoo-helper install`, so the
//! person sees the desktop's own password prompt; in development,
//! `scripts/install-helper-dev.sh` runs it with sudo. A package installs the
//! same files itself and never needs this.
//!
//! The files are built into the binary, so the helper installs itself from
//! wherever it is, with nothing else alongside it.

use std::fs;
use std::io;
use std::path::Path;

use netfoo_core::linux::HELPER_PATH;

use crate::files::write_atomically;
use crate::services::run;

/// What's installed besides the helper itself, and where. Packages put the
/// same files in the same places (see `linux/README.md`).
const FILES: [(&str, &str); 4] = [
    (
        "/usr/share/dbus-1/system-services/io.netfoo.Helper1.service",
        include_str!("../linux/io.netfoo.Helper1.service"),
    ),
    (
        "/usr/share/dbus-1/system.d/io.netfoo.Helper1.conf",
        include_str!("../linux/io.netfoo.Helper1.conf"),
    ),
    (
        "/usr/lib/systemd/system/netfoo-helper.service",
        include_str!("../linux/netfoo-helper.service"),
    ),
    (
        "/usr/share/polkit-1/actions/io.netfoo.policy",
        include_str!("../linux/io.netfoo.policy"),
    ),
];

const UNIT: &str = "netfoo-helper.service";

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("this needs to run as root (try pkexec or sudo)")]
    NotRoot,
    #[error("couldn't {action} {path}: {source}")]
    File {
        action: &'static str,
        path: String,
        source: io::Error,
    },
    #[error(transparent)]
    Command(#[from] crate::services::CommandError),
}

/// Copies this binary to [`HELPER_PATH`] and writes the D-Bus, systemd and
/// polkit files. Running it again updates them.
pub fn install() -> Result<(), InstallError> {
    require_root()?;
    stop_running_helper();

    let this =
        std::env::current_exe().map_err(|source| file_error("find", "this program", source))?;
    let binary = fs::read(&this)
        .map_err(|source| file_error("read", &this.display().to_string(), source))?;
    write(HELPER_PATH, &binary, 0o755)?;
    for (path, contents) in FILES {
        write(path, contents.as_bytes(), 0o644)?;
    }

    reload_system()?;
    println!("Installed {HELPER_PATH}. D-Bus starts it when netfoo needs it.");
    Ok(())
}

/// Removes everything [`install`] put in place. Shares the helper set up, and
/// /etc/netfoo, are left as they are.
pub fn uninstall() -> Result<(), InstallError> {
    require_root()?;
    stop_running_helper();

    for path in FILES.iter().map(|(path, _)| *path).chain([HELPER_PATH]) {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(file_error("remove", path, source)),
        }
    }
    // Only if it's empty; anything else in it isn't ours to remove.
    let _ = fs::remove_dir(
        Path::new(HELPER_PATH)
            .parent()
            .expect("HELPER_PATH has a directory"),
    );

    reload_system()?;
    println!("Removed netfoo-helper. Shares it set up, and /etc/netfoo, are left as they are.");
    Ok(())
}

fn require_root() -> Result<(), InstallError> {
    // SAFETY: geteuid has no preconditions and can't fail.
    if unsafe { libc::geteuid() } == 0 {
        Ok(())
    } else {
        Err(InstallError::NotRoot)
    }
}

/// A running helper would carry on with the old binary until it exited.
fn stop_running_helper() {
    // Fails if it was never installed, which is fine.
    let _ = run("systemctl", &["stop", UNIT], None);
}

/// Makes systemd and D-Bus read their files again.
fn reload_system() -> Result<(), InstallError> {
    run("systemctl", &["daemon-reload"], None)?;
    // dbus.service is dbus-broker or dbus-daemon, whichever the system uses.
    run("systemctl", &["reload", "dbus.service"], None)?;
    Ok(())
}

fn write(path: &str, contents: &[u8], mode: u32) -> Result<(), InstallError> {
    write_atomically(Path::new(path), contents, mode)
        .map_err(|source| file_error("write", path, source))
}

fn file_error(action: &'static str, path: &str, source: io::Error) -> InstallError {
    InstallError::File {
        action,
        path: path.to_string(),
        source,
    }
}
