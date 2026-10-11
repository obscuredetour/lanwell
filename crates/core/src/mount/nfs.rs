//! What mounting an NFS export takes: its address and which version of the
//! protocol to speak. There's no login: the server decides by which machine
//! is asking (its export list), and files keep the server's user IDs.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::mount::check_host;
use crate::names::{InvalidValue, has_control_chars, invalid};

/// Which version of NFS to speak. Automatic tries the newest first, which is
/// almost always right; a fixed one is for servers that answer badly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum NfsVersion {
    Automatic,
    #[serde(rename = "4.2")]
    V4_2,
    #[serde(rename = "4.1")]
    V4_1,
    #[serde(rename = "4.0")]
    V4_0,
    /// What macOS serves, and older NAS boxes.
    #[serde(rename = "3")]
    V3,
}

impl NfsVersion {
    /// The value of mount.nfs's `vers=` option, if one is needed.
    pub fn option(self) -> Option<&'static str> {
        match self {
            NfsVersion::Automatic => None,
            NfsVersion::V4_2 => Some("4.2"),
            NfsVersion::V4_1 => Some("4.1"),
            NfsVersion::V4_0 => Some("4.0"),
            NfsVersion::V3 => Some("3"),
        }
    }
}

/// Where an NFS export is: a server and a folder on it, written
/// `nas.lan:/srv/media` (as mount and fstab write it).
///
/// Also accepts `nfs://nas.lan/srv/media`, and keeps the first form. An IPv6
/// address goes in brackets: `[fe80::1]:/srv/media`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct NfsAddress(String);

impl NfsAddress {
    pub fn new(input: impl AsRef<str>) -> Result<Self, InvalidValue> {
        let input = input.as_ref().trim();
        let (host, path) = match input.strip_prefix("nfs://") {
            Some(rest) => {
                let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
                (host, format!("/{path}"))
            }
            None => {
                let (host, path) = input
                    .split_once(":/")
                    .ok_or_else(|| invalid("Write the address like nas.lan:/srv/media."))?;
                (host, format!("/{path}"))
            }
        };
        check_host(host)?;
        let path = check_export_path(&path)?;
        Ok(Self(format!("{host}:{path}")))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The server, like `nas.lan`.
    pub fn host(&self) -> &str {
        self.parts().0
    }

    /// The exported folder on the server, like `/srv/media`.
    pub fn path(&self) -> &str {
        self.parts().1
    }

    fn parts(&self) -> (&str, &str) {
        let colon = self.0.find(":/").expect("checked when it was made");
        (&self.0[..colon], &self.0[colon + 1..])
    }
}

/// The folder on the server, without a trailing `/` (except for `/` itself).
fn check_export_path(path: &str) -> Result<String, InvalidValue> {
    let trimmed = match path.trim_end_matches('/') {
        "" => "/",
        trimmed => trimmed,
    };
    if has_control_chars(trimmed) || trimmed.split('/').any(|part| part == "..") {
        return Err(invalid(format!(
            "\"{trimmed}\" isn't a folder path netfoo can use."
        )));
    }
    Ok(trimmed.to_string())
}

impl fmt::Display for NfsAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for NfsAddress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for NfsAddress {
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
            "nas.lan:/srv/media",
            "nfs://nas.lan/srv/media",
            " nas.lan:/srv/media/ ",
        ] {
            let address = NfsAddress::new(input).unwrap();
            assert_eq!(address.as_str(), "nas.lan:/srv/media", "{input}");
            assert_eq!((address.host(), address.path()), ("nas.lan", "/srv/media"));
        }
        let ipv6 = NfsAddress::new("[fe80::1]:/srv/media").unwrap();
        assert_eq!((ipv6.host(), ipv6.path()), ("[fe80::1]", "/srv/media"));
        assert_eq!(NfsAddress::new("localhost:/").unwrap().path(), "/");
    }

    #[test]
    fn bad_addresses() {
        for input in [
            "nas.lan/srv/media",
            "//nas.lan/media",
            ":/srv/media",
            "nas lan:/srv/media",
            "nas.lan:/srv/../etc",
        ] {
            assert!(NfsAddress::new(input).is_err(), "{input}");
        }
    }
}
