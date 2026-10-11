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
        check_share(share)?;
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

fn check_share(share: &str) -> Result<(), InvalidValue> {
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
}
