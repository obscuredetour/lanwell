//! What the app can ask the privileged helper to do.
//!
//! Requests describe outcomes ("these are the shares"), not file edits, so the
//! helper decides what gets written where and nothing the app sends can name a
//! file. Each kind of request needs its own polkit permission ([`Request::action`]).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::bus::actions;
use crate::mount::Mount;
use crate::names::{AbsolutePath, UserName};
use crate::share::{NfsExport, SmbShare};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Request {
    /// Replace every Samba share netfoo manages with these.
    SetSmbShares { shares: Vec<SmbShare> },
    /// Replace every NFS export netfoo manages with these.
    SetNfsExports { exports: Vec<NfsExport> },
    /// Set a user's Samba password, adding them to Samba if they're new to it.
    /// The user must already have an account on this machine.
    SetSmbPassword { user: UserName, password: Secret },
    /// Mount a share from another machine, replacing any netfoo mount on the
    /// same folder. `password` is for an SMB user login: needed for a new
    /// one, or left out to keep the one already saved.
    ///
    /// `replaces` is the folder of the mount this edits, if the edit moves it
    /// to another one: that mount goes, and its saved password comes along.
    SetMount {
        mount: Mount,
        password: Option<Secret>,
        replaces: Option<AbsolutePath>,
    },
    /// Unmount the share on this folder and stop mounting it. The folder stays.
    RemoveMount { mount_point: AbsolutePath },
}

impl Request {
    /// The polkit action that allows this request (see `crates/helper/linux/io.netfoo.policy`).
    pub fn action(&self) -> &'static str {
        match self {
            Request::SetSmbShares { .. } | Request::SetNfsExports { .. } => actions::MANAGE_SHARES,
            Request::SetSmbPassword { .. } => actions::SET_PASSWORD,
            Request::SetMount { .. } | Request::RemoveMount { .. } => actions::MANAGE_MOUNTS,
        }
    }
}

/// A password. It never shows up in `Debug` output, so requests can be logged.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Secret(pub String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_stay_out_of_logs() {
        let request = Request::SetSmbPassword {
            user: UserName::new("alice").unwrap(),
            password: Secret("hunter2".into()),
        };
        assert!(!format!("{request:?}").contains("hunter2"));
    }

    #[test]
    fn json_shape() {
        // The UI builds these by hand from the generated bindings; keep the shape stable.
        let json = r#"{"kind":"setSmbPassword","user":"alice","password":"pw"}"#;
        let request: Request = serde_json::from_str(json).unwrap();
        assert_eq!(request.action(), actions::SET_PASSWORD);
    }
}
