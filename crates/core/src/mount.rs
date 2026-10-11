//! Shares on other machines, mounted here.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::names::{AbsolutePath, InvalidValue, RemoteName, has_control_chars, invalid};

/// A share on another machine, mounted here over SMB (CIFS).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SmbMount {
    /// Where the share is, like `//nas.lan/media`.
    pub address: SmbAddress,
    /// The folder it appears in here. netfoo suggests `~/Network/<share>`.
    pub mount_point: AbsolutePath,
    pub login: MountLogin,
    pub when: MountWhen,
    pub read_only: bool,
    pub version: SmbVersion,
}

/// How to sign in to the share. The password isn't part of this: it's sent
/// with the request and kept only in a file only root can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MountLogin {
    Guest,
    User {
        username: RemoteName,
        /// The Windows domain or workgroup, if the server wants one.
        domain: Option<RemoteName>,
    },
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

/// Which version of SMB to speak. Automatic lets the two sides agree on the
/// newest they share, which is almost always right; the others are for
/// servers that negotiate badly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum SmbVersion {
    Automatic,
    #[serde(rename = "3.1.1")]
    V3_1_1,
    #[serde(rename = "3.0")]
    V3_0,
    #[serde(rename = "2.1")]
    V2_1,
    #[serde(rename = "2.0")]
    V2_0,
    /// SMB1: insecure, and off by default on modern systems. Only for very old devices.
    #[serde(rename = "1.0")]
    V1_0,
}

impl SmbVersion {
    /// The value of mount.cifs's `vers=` option, if one is needed.
    pub fn option(self) -> Option<&'static str> {
        match self {
            SmbVersion::Automatic => None,
            SmbVersion::V3_1_1 => Some("3.1.1"),
            SmbVersion::V3_0 => Some("3.0"),
            SmbVersion::V2_1 => Some("2.1"),
            SmbVersion::V2_0 => Some("2.0"),
            SmbVersion::V1_0 => Some("1.0"),
        }
    }
}

/// The account a mount's files appear to belong to (SMB has no Unix owners).
/// The helper sets it to whoever asked for the mount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Owner {
    pub uid: u32,
    pub gid: u32,
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

/// A mount as netfoo keeps it: what was asked for, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ManagedMount {
    #[serde(flatten)]
    pub mount: SmbMount,
    pub owner: Owner,
}

/// Where an SMB share is: a server and a share on it, written `//nas.lan/media`.
///
/// Also accepts `\\nas.lan\media` (as Windows writes it) and
/// `smb://nas.lan/media` (as file managers do), and keeps the first form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct SmbAddress(String);

impl SmbAddress {
    pub fn new(input: impl AsRef<str>) -> Result<Self, InvalidValue> {
        let input = input.as_ref().trim();
        let rest = input
            .strip_prefix("smb://")
            .or_else(|| input.strip_prefix("//"))
            .or_else(|| input.strip_prefix("\\\\"))
            .ok_or_else(|| invalid("Write the address like //nas.lan/media."))?;
        let rest = rest.replace('\\', "/");
        let (host, share) = rest.trim_end_matches('/').split_once('/').ok_or_else(|| {
            invalid("Add the share's name after the server, like //nas.lan/media.")
        })?;
        check_host(host)?;
        check_remote_share(share)?;
        Ok(Self(format!("//{host}/{share}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The server, like `nas.lan`.
    pub fn host(&self) -> &str {
        self.parts().0
    }

    /// The share, like `media`.
    pub fn share(&self) -> &str {
        self.parts().1
    }

    fn parts(&self) -> (&str, &str) {
        self.0[2..]
            .split_once('/')
            .expect("checked when it was made")
    }
}

/// Checks a server's name or address, like `nas.lan` or `192.168.1.5`.
pub fn check_host(host: &str) -> Result<(), InvalidValue> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || ".-:[]".contains(c);
    if host.is_empty() || !host.chars().all(allowed) {
        return Err(invalid(format!(
            "\"{host}\" isn't a server name or address netfoo understands."
        )));
    }
    Ok(())
}

fn check_remote_share(share: &str) -> Result<(), InvalidValue> {
    if share.is_empty() {
        return Err(invalid(
            "Add the share's name after the server, like //nas.lan/media.",
        ));
    }
    if share.contains('/') {
        return Err(invalid(
            "netfoo mounts a whole share; folders inside one aren't supported yet.",
        ));
    }
    if has_control_chars(share) || share.chars().count() > 80 {
        return Err(invalid(format!(
            "\"{share}\" isn't a share name netfoo can use."
        )));
    }
    Ok(())
}

impl fmt::Display for SmbAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for SmbAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SmbAddress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Folders a share must never be mounted on, or in: the system's own.
const SYSTEM_FOLDERS: [&str; 13] = [
    "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/root", "/run", "/sbin", "/sys",
    "/usr", "/var",
];

/// Checks a folder can take a mount: not the root or a system folder, and
/// written plainly (no `//`, no trailing `/`), since its systemd unit is
/// named after it.
pub fn check_mount_point(path: &AbsolutePath) -> Result<(), InvalidValue> {
    let path = path.as_str();
    if path == "/" {
        return Err(invalid("A share can't be mounted over the whole system."));
    }
    if path.ends_with('/') || path.contains("//") || path.split('/').any(|part| part == ".") {
        return Err(invalid(format!(
            "Write {path} without extra slashes or dots."
        )));
    }
    let inside = |folder: &str| path == folder || path.starts_with(&format!("{folder}/"));
    if let Some(folder) = SYSTEM_FOLDERS.iter().find(|folder| inside(folder)) {
        return Err(invalid(format!(
            "{folder} belongs to the system; mount the share somewhere else."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_in_every_spelling() {
        for input in [
            "//nas.lan/media",
            "\\\\nas.lan\\media",
            "smb://nas.lan/media",
            " //nas.lan/media/ ",
        ] {
            let address = SmbAddress::new(input).unwrap();
            assert_eq!(address.as_str(), "//nas.lan/media", "{input}");
            assert_eq!((address.host(), address.share()), ("nas.lan", "media"));
        }
        assert_eq!(
            SmbAddress::new("//192.168.1.5/My Media").unwrap().share(),
            "My Media"
        );
    }

    #[test]
    fn bad_addresses() {
        for input in [
            "nas.lan/media",
            "//nas.lan",
            "//nas.lan/",
            "//nas lan/media",
            "//nas.lan/media/movies",
        ] {
            assert!(SmbAddress::new(input).is_err(), "{input}");
        }
    }

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
        let json = r#"{
            "address": "//nas.lan/media",
            "mountPoint": "/home/alice/Network/media",
            "login": { "kind": "user", "username": "alice", "domain": null },
            "when": "firstAccess",
            "readOnly": false,
            "version": "automatic",
            "owner": { "uid": 1000, "gid": 1000 }
        }"#;
        let mount: ManagedMount = serde_json::from_str(json).unwrap();
        assert_eq!(mount.mount.when, MountWhen::FirstAccess);
        assert_eq!(mount.owner.uid, 1000);
    }
}
