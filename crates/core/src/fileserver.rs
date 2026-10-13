//! The file servers lanwell shares through (Samba and the NFS server): which
//! systemd unit runs each, and what the firewall has to let through.

use serde::{Deserialize, Serialize};

/// A file server this machine can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum FileServer {
    Samba,
    Nfs,
}

impl FileServer {
    pub const ALL: [FileServer; 2] = [FileServer::Samba, FileServer::Nfs];

    /// The systemd units that run it, by distribution: Samba is `smb` on Arch
    /// and Fedora and `smbd` on Debian and Ubuntu; the NFS server is
    /// `nfs-server` almost everywhere (`nfs-kernel-server` is Debian's old name).
    pub fn units(self) -> &'static [&'static str] {
        match self {
            FileServer::Samba => &["smb.service", "smbd.service"],
            FileServer::Nfs => &["nfs-server.service", "nfs-kernel-server.service"],
        }
    }

    /// The TCP port clients connect to: SMB 2 and 3, and NFS 4. (SMB 1's
    /// NetBIOS ports and NFS 3's extra services aren't opened.)
    pub fn port(self) -> u16 {
        match self {
            FileServer::Samba => 445,
            FileServer::Nfs => 2049,
        }
    }

    /// The service firewalld knows it by.
    pub fn firewalld_service(self) -> &'static str {
        match self {
            FileServer::Samba => "samba",
            FileServer::Nfs => "nfs",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            FileServer::Samba => "Samba",
            FileServer::Nfs => "the NFS server",
        }
    }
}

/// Whether a file server is installed, running, and let through the firewall.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct FileServerStatus {
    pub installed: bool,
    pub running: bool,
    /// Starts with the computer.
    pub enabled: bool,
    pub firewall: Firewall,
    pub access: FirewallAccess,
}

/// The firewall in charge, if one is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Firewall {
    /// None is on: nothing to open.
    None,
    Ufw,
    Firewalld,
}

/// Whether the firewall lets other computers reach a file server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FirewallAccess {
    /// Let through: by lanwell (which can take it back), or by some other rule.
    Allowed {
        by_lanwell: bool,
    },
    Blocked,
    /// The firewall's rules couldn't be read without root.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shape() {
        let access = FirewallAccess::Allowed { by_lanwell: true };
        assert_eq!(
            serde_json::to_string(&access).unwrap(),
            r#"{"kind":"allowed","byLanwell":true}"#
        );
    }
}
