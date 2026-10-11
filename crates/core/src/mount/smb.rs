//! What mounting an SMB (CIFS) share takes: its address, how to sign in, and
//! which version of the protocol to speak.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::mount::check_host;
use crate::names::{InvalidValue, RemoteName, has_control_chars, invalid};

/// How to sign in to the share. The password isn't part of this: it's sent
/// with the request and kept only in a file only root can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SmbLogin {
    Guest,
    User {
        username: RemoteName,
        /// The Windows domain or workgroup, if the server wants one.
        domain: Option<RemoteName>,
    },
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

/// Where an SMB share is: a server, a share on it, and optionally a folder
/// inside the share, written `//nas.lan/media` or `//nas.lan/media/films`.
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
        let mut parts = rest.split('/').filter(|part| !part.is_empty());
        let host = parts.next().unwrap_or_default();
        check_host(host)?;
        let share = parts.next().ok_or_else(|| {
            invalid("Add the share's name after the server, like //nas.lan/media.")
        })?;
        check_name(share, "share name")?;
        let mut address = format!("//{host}/{share}");
        for folder in parts {
            check_name(folder, "folder name")?;
            address.push('/');
            address.push_str(folder);
        }
        Ok(Self(address))
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

    /// The folder inside the share, if there is one, like `films` or `films/old`.
    pub fn folder(&self) -> Option<&str> {
        self.parts().2
    }

    fn parts(&self) -> (&str, &str, Option<&str>) {
        let rest = &self.0[2..];
        let (host, rest) = rest.split_once('/').expect("checked when it was made");
        match rest.split_once('/') {
            Some((share, folder)) => (host, share, Some(folder)),
            None => (host, rest, None),
        }
    }
}

/// Checks a share's name, or a folder's in the path after it: what mount.cifs
/// and the unit file can take, and not `.` or `..`.
fn check_name(name: &str, what: &str) -> Result<(), InvalidValue> {
    if name == "." || name == ".." || has_control_chars(name) || name.chars().count() > 255 {
        return Err(invalid(format!(
            "\"{name}\" isn't a {what} netfoo can use."
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
    fn folders_inside_a_share() {
        for input in [
            "//nas.lan/media/films/old",
            "\\\\nas.lan\\media\\films\\old",
            "smb://nas.lan/media/films/old/",
        ] {
            let address = SmbAddress::new(input).unwrap();
            assert_eq!(address.as_str(), "//nas.lan/media/films/old", "{input}");
            assert_eq!(address.share(), "media");
            assert_eq!(address.folder(), Some("films/old"));
        }
        assert_eq!(SmbAddress::new("//nas.lan/media").unwrap().folder(), None);
    }

    #[test]
    fn bad_addresses() {
        for input in [
            "nas.lan/media",
            "//nas.lan",
            "//nas.lan/",
            "//nas lan/media",
            "//nas.lan/media/../secret",
        ] {
            assert!(SmbAddress::new(input).is_err(), "{input}");
        }
    }
}
