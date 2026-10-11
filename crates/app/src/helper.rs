//! Talking to netfoo-helper over the system bus.

use netfoo_core::Request;
use zbus::fdo;

// The proxy macro needs literal names; the test below keeps them in step with
// netfoo_core::bus, which the helper uses.
#[zbus::proxy(
    interface = "io.netfoo.Helper1",
    default_service = "io.netfoo.Helper1",
    default_path = "/io/netfoo/Helper1"
)]
trait Helper {
    /// Applies a JSON-encoded `netfoo_core::Request` (see the helper's `bus.rs`).
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

/// A message for the person using netfoo.
fn describe(error: zbus::Error) -> String {
    match fdo::Error::from(error) {
        fdo::Error::ServiceUnknown(_) => {
            "netfoo's helper isn't installed, so it can't change anything. \
             (In development, run scripts/install-helper-dev.sh.)"
                .into()
        }
        fdo::Error::AccessDenied(_) => "That change wasn't authorized.".into(),
        fdo::Error::Failed(message) | fdo::Error::InvalidArgs(message) => message,
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use netfoo_core::bus::{BUS_NAME, INTERFACE, OBJECT_PATH};
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
