//! lanwell-helper: asking it for changes over the system bus, and installing it.

use std::path::PathBuf;
use std::process::Command;

use lanwell_core::Request;
use lanwell_core::linux::HELPER_PATH;
use lanwell_core::system::find_program;
use zbus::fdo;

// The proxy macro needs literal names; the test below keeps them in step with
// lanwell_core::bus, which the helper uses.
#[zbus::proxy(
    interface = "io.github.obscuredetour.lanwell.Helper1",
    default_service = "io.github.obscuredetour.lanwell.Helper1",
    default_path = "/io/github/obscuredetour/lanwell/Helper1"
)]
trait Helper {
    /// Applies a JSON-encoded `lanwell_core::Request` (see the helper's `bus.rs`).
    fn apply(&self, request: &str) -> zbus::Result<()>;

    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}

pub async fn apply(request: &Request) -> Result<(), String> {
    let json = serde_json::to_string(request).map_err(|e| e.to_string())?;
    // A new connection per change: changes are rare, and it keeps no state to go stale.
    let connection = zbus::Connection::system().await.map_err(describe)?;
    let helper = HelperProxy::new(&connection).await.map_err(describe)?;
    helper.apply(&json).await.map_err(describe)
}

/// Installs (or updates) the helper that came with this app, as root.
pub async fn install() -> Result<(), String> {
    as_root(bundled_helper()?, "install").await
}

/// Removes the installed helper (which removes itself), as root.
pub async fn uninstall() -> Result<(), String> {
    as_root(PathBuf::from(HELPER_PATH), "uninstall").await
}

/// Runs `helper command` as root. pkexec shows the desktop's own password
/// prompt for it.
async fn as_root(helper: PathBuf, command: &'static str) -> Result<(), String> {
    let pkexec = find_program("pkexec")
        .ok_or("pkexec isn't installed, so lanwell can't ask for the password it needs.")?;

    // pkexec waits for the password prompt, so it runs off the async runtime.
    let output = tauri::async_runtime::spawn_blocking(move || {
        Command::new(pkexec).arg(helper).arg(command).output()
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Couldn't run pkexec: {e}"))?;

    match output.status.code() {
        Some(0) => Ok(()),
        // pkexec's own codes: the prompt was dismissed, or the password refused
        // (or there's no polkit agent to show one).
        Some(126) => Err("Cancelled.".into()),
        Some(127) => Err(
            "The password wasn't accepted, or there's no password prompt \
                          available on this desktop."
                .into(),
        ),
        _ => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
    }
}

/// The helper built and shipped with this app: beside the app's own binary
/// (target/debug in development; a bundle's sidecar when packaged).
fn bundled_helper() -> Result<PathBuf, String> {
    let app = std::env::current_exe().map_err(|e| e.to_string())?;
    let helper = app.with_file_name("lanwell-helper");
    if helper.is_file() {
        Ok(helper)
    } else {
        Err(format!(
            "lanwell's helper should be at {}, but it isn't there. \
             (In development, `cargo tauri dev` builds it; check its output for errors.)",
            helper.display()
        ))
    }
}

/// A message for the person using lanwell.
fn describe(error: zbus::Error) -> String {
    match fdo::Error::from(error) {
        fdo::Error::ServiceUnknown(_) => {
            "lanwell's helper isn't installed, so it can't change anything. \
             Install it from the banner at the top of the window."
                .into()
        }
        fdo::Error::AccessDenied(_) => "That change wasn't authorized.".into(),
        fdo::Error::Failed(message) | fdo::Error::InvalidArgs(message) => message,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use lanwell_core::bus::{BUS_NAME, INTERFACE, OBJECT_PATH};
    use zbus::proxy::Defaults;

    use super::HelperProxy;

    #[test]
    fn names_match_the_helper() {
        assert_eq!(
            HelperProxy::DESTINATION.as_ref().map(|name| name.as_str()),
            Some(BUS_NAME)
        );
        assert_eq!(
            HelperProxy::PATH.as_ref().map(|path| path.as_str()),
            Some(OBJECT_PATH)
        );
        assert_eq!(
            HelperProxy::INTERFACE.as_ref().map(|name| name.as_str()),
            Some(INTERFACE)
        );
    }
}
