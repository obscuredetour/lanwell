//! The names the helper is reached by. They must match the files in
//! `crates/helper/linux/`, which install it.

/// The helper's well-known name on the system bus.
pub const BUS_NAME: &str = "io.netfoo.Helper1";
/// The interface the helper's methods are on.
pub const INTERFACE: &str = "io.netfoo.Helper1";
/// The object the helper serves.
pub const OBJECT_PATH: &str = "/io/netfoo/Helper1";

/// polkit action ids, one per kind of change. Their defaults (ask for an
/// administrator's password, and remember it briefly) are set in
/// `io.netfoo.policy`.
pub mod actions {
    /// Add, change or remove shared folders.
    pub const MANAGE_SHARES: &str = "io.netfoo.manage-shares";
    /// Mount other machines' shares here, or stop mounting them.
    pub const MANAGE_MOUNTS: &str = "io.netfoo.manage-mounts";
    /// Set a user's Samba password.
    pub const SET_PASSWORD: &str = "io.netfoo.set-password";
}
