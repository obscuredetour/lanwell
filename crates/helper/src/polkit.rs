//! Asking polkit whether a caller may do something.
//!
//! polkit decides from the action's defaults (in `linux/io.github.obscuredetour.lanwell.policy`) and
//! any local rules, and shows the caller a password prompt when the answer is
//! "only with an administrator's password".

use std::collections::HashMap;

use zbus::fdo;
use zbus::names::UniqueName;
use zbus::zvariant::Value;

#[zbus::proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
trait Authority {
    /// Returns (authorized, challenge, details). See polkit's
    /// org.freedesktop.PolicyKit1.Authority documentation.
    fn check_authorization(
        &self,
        subject: &(&str, HashMap<&str, Value<'_>>),
        action_id: &str,
        details: &HashMap<&str, &str>,
        flags: u32,
        cancellation_id: &str,
    ) -> zbus::Result<(bool, bool, HashMap<String, String>)>;
}

/// Lets polkit show a password prompt if one is needed.
const ALLOW_USER_INTERACTION: u32 = 1;

/// Succeeds if `caller` (the bus name a request came from) may perform `action`.
pub async fn authorize(
    connection: &zbus::Connection,
    caller: &UniqueName<'_>,
    action: &str,
) -> fdo::Result<()> {
    let authority = AuthorityProxy::new(connection).await?;

    // Identifying the caller by its bus name (rather than a process id) is
    // what polkit recommends for D-Bus services: it can't be raced.
    let subject = (
        "system-bus-name",
        HashMap::from([("name", Value::from(caller.as_str()))]),
    );
    let (authorized, _challenge, _details) = authority
        .check_authorization(
            &subject,
            action,
            &HashMap::new(),
            ALLOW_USER_INTERACTION,
            "",
        )
        .await?;

    if authorized {
        Ok(())
    } else {
        Err(fdo::Error::AccessDenied(format!("Not allowed ({action}).")))
    }
}
