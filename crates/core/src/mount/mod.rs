//! Shares on other machines, mounted here: over SMB ([`smb`]) or NFS ([`nfs`]).

pub mod nfs;
pub mod smb;

use serde::{Deserialize, Serialize};

use crate::names::{AbsolutePath, InvalidValue, invalid};

pub use nfs::{NfsAddress, NfsVersion};
pub use smb::{SmbAddress, SmbLogin, SmbVersion};

/// A share on another machine, and the folder it appears in here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct Mount {
    pub share: RemoteShare,
    /// The folder it appears in here. netfoo suggests `~/Network/<share>`.
    pub mount_point: AbsolutePath,
    pub when: MountWhen,
    pub read_only: bool,
}

/// Where the share is, and what speaking to it takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "protocol", rename_all = "camelCase")]
pub enum RemoteShare {
    /// The protocol Windows, macOS and most NAS boxes speak. Has user names
    /// and passwords.
    Smb {
        address: SmbAddress,
        login: SmbLogin,
        version: SmbVersion,
    },
    /// The protocol Linux and other Unix systems speak natively. Has no
    /// passwords: the server decides by which machine is asking.
    Nfs {
        address: NfsAddress,
        version: NfsVersion,
    },
}

impl RemoteShare {
    /// The address as it's written, like `//nas.lan/media` or `nas.lan:/srv/media`.
    pub fn address(&self) -> &str {
        match self {
            RemoteShare::Smb { address, .. } => address.as_str(),
            RemoteShare::Nfs { address, .. } => address.as_str(),
        }
    }
}

/// When the share is mounted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MountWhen {
    /// The first time something opens the folder. Starting up never waits for
    /// the server, and an unreachable server only matters when it's used.
    FirstAccess,
    /// While the computer starts up. Startup waits for the server (up to a
    /// time limit), so it's slower whenever the server is off.
    Startup,
}

/// The account that asked for a mount. An SMB mount's files appear to
/// belong to it (SMB has no Unix owners); an NFS mount's keep the server's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
}

/// A mount as netfoo keeps it: what was asked for, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ManagedMount {
    #[serde(flatten)]
    pub mount: Mount,
    pub owner: Owner,
}

/// What a mount is doing now, as systemd reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum MountActivity {
    Mounted,
    /// Not mounted yet; it will be when something opens the folder.
    Waiting,
    /// The last attempt to mount it failed.
    Failed,
    /// Neither mounted nor waiting (stopped, or being set up).
    Off,
}

impl MountActivity {
    /// From the `ActiveState` of a mount's `.mount` and `.automount` units.
    pub fn from_unit_states(mount: &str, automount: &str) -> Self {
        match (mount.trim(), automount.trim()) {
            ("active", _) => Self::Mounted,
            ("failed", _) => Self::Failed,
            (_, "active") => Self::Waiting,
            _ => Self::Off,
        }
    }
}

/// A mount and what it's doing now, for the UI.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct MountStatus {
    pub mount_point: AbsolutePath,
    pub activity: MountActivity,
}

/// Checks a server's name or address, like `nas.lan`, `192.168.1.5` or `[fe80::1]`.
pub fn check_host(host: &str) -> Result<(), InvalidValue> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || ".-:[]".contains(c);
    if host.is_empty() || !host.chars().all(allowed) {
        return Err(invalid(format!(
            "\"{host}\" isn't a server name or address netfoo understands."
        )));
    }
    Ok(())
}

/// Checks a folder can take a mount: not a system folder, and written
/// plainly (no `//`, no trailing `/`), since its systemd unit is named after it.
pub fn check_mount_point(path: &AbsolutePath) -> Result<(), InvalidValue> {
    if let Some(folder) = path.system_folder() {
        return Err(invalid(format!(
            "{folder} belongs to the system; mount the share somewhere else."
        )));
    }
    let text = path.as_str();
    if text.ends_with('/') || text.contains("//") || text.split('/').any(|part| part == ".") {
        return Err(invalid(format!(
            "Write {text} without extra slashes or dots."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_points() {
        let check = |path: &str| check_mount_point(&AbsolutePath::new(path).unwrap());
        assert!(check("/home/alice/Network/media").is_ok());
        assert!(check("/mnt/media").is_ok());
        assert!(check("/").is_err());
        assert!(check("/etc").is_err());
        assert!(check("/usr/local/media").is_err());
        assert!(check("/home/alice/Network/").is_err());
        assert!(check("/home//alice").is_err());
        // Only the folder itself, not every name that starts the same.
        assert!(check("/variety").is_ok());
    }

    #[test]
    fn json_shape() {
        // The UI builds these from the generated bindings; keep the shape stable.
        let json = r#"{
            "share": { "protocol": "nfs", "address": "nas.lan:/srv/media", "version": "automatic" },
            "mountPoint": "/home/alice/Network/media",
            "when": "firstAccess",
            "readOnly": true,
            "owner": { "uid": 1000, "gid": 1000 }
        }"#;
        let managed: ManagedMount = serde_json::from_str(json).unwrap();
        assert_eq!(managed.mount.share.address(), "nas.lan:/srv/media");
        assert_eq!(managed.owner.uid, 1000);
    }
}
