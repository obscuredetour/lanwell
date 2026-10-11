//! Validated names and paths.
//!
//! The helper writes these into config files as root, so each type rejects
//! anything that could change the meaning of the file around it: control
//! characters (a newline would start a new setting), and the characters each
//! format treats specially. They're checked when they're made and again when
//! they're deserialized, so a request from the app can't skip the check.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A value that isn't allowed where it was used. The message is meant for the
/// person who typed it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InvalidValue(pub String);

pub(crate) fn invalid(message: impl Into<String>) -> InvalidValue {
    InvalidValue(message.into())
}

pub(crate) fn has_control_chars(value: &str) -> bool {
    value.chars().any(char::is_control)
}

/// Implements what every validated string type shares: `new` (which checks
/// the value), `as_str`, `Display`, and serde as a plain string that's checked
/// when it's read.
///
/// (Written out rather than `#[serde(try_from = "String")]`, which ts-rs
/// doesn't understand.)
macro_rules! validated_string {
    ($name:ident, $check:path) => {
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, InvalidValue> {
                let value = value.into();
                $check(&value)?;
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

/// The name a Samba share is seen by on the network, like `Media`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct ShareName(String);

validated_string!(ShareName, check_share_name);

/// Sections smb.conf gives a meaning of its own.
const RESERVED_SHARE_NAMES: [&str; 4] = ["global", "homes", "printers", "ipc$"];

fn check_share_name(name: &str) -> Result<(), InvalidValue> {
    // Windows allows 80 characters; these are the ones it (or smb.conf) won't take.
    const FORBIDDEN: &str = "\\/[]:|<>+=;,*?\"%";

    if name.trim().is_empty() {
        return Err(invalid("A share needs a name."));
    }
    if name.trim() != name {
        return Err(invalid("A share name can't start or end with a space."));
    }
    if name.chars().count() > 80 {
        return Err(invalid("A share name can be at most 80 characters."));
    }
    if has_control_chars(name) || name.chars().any(|c| FORBIDDEN.contains(c)) {
        return Err(invalid(format!(
            "A share name can't contain any of {FORBIDDEN}"
        )));
    }
    if RESERVED_SHARE_NAMES.contains(&name.to_lowercase().as_str()) {
        return Err(invalid(format!("\"{name}\" is reserved by Samba.")));
    }
    Ok(())
}

/// A user account on this machine, like `alice`.
///
/// Follows the rule most distributions' `useradd` enforces: lowercase letters,
/// digits, `_` and `-`, not starting with a digit or `-`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct UserName(String);

validated_string!(UserName, check_user_name);

fn check_user_name(name: &str) -> Result<(), InvalidValue> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err(invalid("A user needs a name."));
    };
    if name.len() > 32 {
        return Err(invalid("A user name can be at most 32 characters."));
    }
    let valid_first = first.is_ascii_lowercase() || first == '_';
    let valid_rest =
        chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if !valid_first || !valid_rest {
        return Err(invalid(format!(
            "\"{name}\" isn't a valid user name: use lowercase letters, digits, _ and -, starting with a letter."
        )));
    }
    Ok(())
}

/// An absolute path on this machine, like `/srv/media`.
///
/// Kept as a `String` rather than a `PathBuf`: config files are text, and the
/// UI needs it as one too.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct AbsolutePath(String);

validated_string!(AbsolutePath, check_absolute_path);

fn check_absolute_path(path: &str) -> Result<(), InvalidValue> {
    if !path.starts_with('/') {
        return Err(invalid(format!(
            "\"{path}\" isn't a full path (it should start with /)."
        )));
    }
    if path.trim_end() != path {
        return Err(invalid("A path can't end with a space."));
    }
    if has_control_chars(path) {
        return Err(invalid("A path can't contain control characters."));
    }
    if path.split('/').any(|part| part == "..") {
        return Err(invalid("A path can't contain \"..\"."));
    }
    // smb.conf reads a line ending in \ as continuing on the next one.
    if path.ends_with('\\') {
        return Err(invalid("A path can't end with \\."));
    }
    Ok(())
}

/// Folders that belong to the system: nothing is shared from them or
/// mounted on them. (Except drives in /run/media; see [`AbsolutePath::system_folder`].)
const SYSTEM_FOLDERS: [&str; 15] = [
    "/bin", "/boot", "/dev", "/efi", "/etc", "/lib", "/lib32", "/lib64", "/proc", "/root", "/run",
    "/sbin", "/sys", "/usr", "/var",
];

impl AbsolutePath {
    /// The system folder this is, or is inside, if any (`/` counts as one).
    /// Compares the text only; the helper resolves symbolic links first.
    ///
    /// Drives in /run/media (`/run/media/main`, `/run/media/alice/backup`) are
    /// people's, not the system's: it's where many distributions and desktops
    /// mount them. /run/media itself still isn't.
    pub fn system_folder(&self) -> Option<&'static str> {
        let path = self.0.as_str();
        if path.trim_end_matches('/').is_empty() {
            return Some("/");
        }
        if path
            .strip_prefix("/run/media/")
            .is_some_and(|rest| !rest.trim_matches('/').is_empty())
        {
            return None;
        }
        let inside = |folder: &str| path == folder || path.starts_with(&format!("{folder}/"));
        SYSTEM_FOLDERS.iter().copied().find(|folder| inside(folder))
    }
}

/// Which machines may mount an NFS export: a host name (`laptop.lan`), an
/// address (`192.168.1.20`), a network (`192.168.1.0/24`), a wildcard
/// (`*.lan`), or `*` for anyone.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct NfsClient(String);

validated_string!(NfsClient, check_nfs_client);

fn check_nfs_client(client: &str) -> Result<(), InvalidValue> {
    // Anything else (spaces, parentheses, commas) would be read as the start of
    // the next client or of the options.
    let allowed = |c: char| c.is_ascii_alphanumeric() || ".:-_*?/[]".contains(c);

    if client.is_empty() {
        return Err(invalid("Say which machines may connect (use * for any)."));
    }
    if !client.chars().all(allowed) {
        return Err(invalid(format!(
            "\"{client}\" isn't a host name, address or network netfoo understands."
        )));
    }
    Ok(())
}

/// A user name or domain on another machine, for signing in to its shares.
///
/// Other systems allow far more than [`UserName`] does here (capitals,
/// spaces, `DOMAIN\user`), so this only rejects what would break the
/// credentials file it's written to: line breaks and other control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct RemoteName(String);

validated_string!(RemoteName, check_remote_name);

fn check_remote_name(name: &str) -> Result<(), InvalidValue> {
    if name.trim().is_empty() {
        return Err(invalid("A user name can't be empty."));
    }
    if has_control_chars(name) || name.len() > 256 {
        return Err(invalid(format!(
            "\"{name}\" isn't a user name netfoo can use."
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_names() {
        assert!(ShareName::new("Media").is_ok());
        assert!(ShareName::new("Family Photos").is_ok());
        assert!(ShareName::new("").is_err());
        assert!(ShareName::new(" Media").is_err());
        assert!(ShareName::new("Media]\n[global").is_err());
        assert!(ShareName::new("Global").is_err());
        assert!(ShareName::new("a".repeat(81)).is_err());
    }

    #[test]
    fn user_names() {
        assert!(UserName::new("alice").is_ok());
        assert!(UserName::new("_svc-backup2").is_ok());
        assert!(UserName::new("Alice").is_err());
        assert!(UserName::new("2alice").is_err());
        assert!(UserName::new("alice bob").is_err());
        assert!(UserName::new("").is_err());
    }

    #[test]
    fn paths() {
        assert!(AbsolutePath::new("/srv/media").is_ok());
        assert!(AbsolutePath::new("/home/alice/My Music").is_ok());
        assert!(AbsolutePath::new("srv/media").is_err());
        assert!(AbsolutePath::new("/srv/media\n[x]").is_err());
        assert!(AbsolutePath::new("/srv/../etc").is_err());
        assert!(AbsolutePath::new("/srv/media ").is_err());
        assert!(AbsolutePath::new("/srv/media\\").is_err());
    }

    #[test]
    fn system_folders() {
        let folder = |path: &str| AbsolutePath::new(path).unwrap().system_folder();
        assert_eq!(folder("/"), Some("/"));
        assert_eq!(folder("/etc"), Some("/etc"));
        assert_eq!(folder("/usr/local/media"), Some("/usr"));
        assert_eq!(folder("/srv/media"), None);
        // Only the folder itself, not every name that starts the same.
        assert_eq!(folder("/variety"), None);
        // Drives in /run/media are people's; the rest of /run isn't.
        assert_eq!(folder("/run/media/main/films"), None);
        assert_eq!(folder("/run/media/alice/backup"), None);
        assert_eq!(folder("/run/media"), Some("/run"));
        assert_eq!(folder("/run/user/1000"), Some("/run"));
    }

    #[test]
    fn nfs_clients() {
        for ok in ["*", "laptop.lan", "*.lan", "192.168.1.0/24", "fe80::1"] {
            assert!(NfsClient::new(ok).is_ok(), "{ok}");
        }
        for bad in ["", "laptop lan", "host(rw)", "a,b"] {
            assert!(NfsClient::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn deserializing_validates() {
        assert!(serde_json::from_str::<ShareName>("\"Media\"").is_ok());
        assert!(serde_json::from_str::<ShareName>("\"x]\\n[global\"").is_err());
    }
}
