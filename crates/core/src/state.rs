//! Everything netfoo manages, as one file.
//!
//! The state file is the source of truth: the helper changes it, then
//! regenerates the Samba and NFS config files from it. Those files are never
//! parsed back, so hand edits to them are overwritten (they say so at the top),
//! and anything netfoo didn't write (the rest of smb.conf, /etc/exports) is
//! left alone.
//!
//! It's readable by everyone and holds no secrets, so the app can show it
//! without asking for a password.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::fileserver::FileServer;
use crate::mount::{
    ManagedMount, Mount, MountWhen, Owner, RemoteShare, SmbAddress, SmbLogin, SmbVersion,
};
use crate::names::AbsolutePath;
use crate::share::{NfsExport, SmbShare};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub smb_shares: Vec<SmbShare>,
    pub nfs_exports: Vec<NfsExport>,
    /// Shares on other machines mounted here, SMB and NFS.
    pub mounts: Vec<ManagedMount>,
    /// The file servers netfoo let through the firewall (so it only ever
    /// takes back what it opened).
    pub firewall_opened: Vec<FileServer>,
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("couldn't read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("{path} isn't a netfoo state file netfoo can read: {source}")]
    Parse {
        path: String,
        source: serde_json::Error,
    },
}

/// A mount as the first builds saved it, before NFS mounts: SMB only, under
/// `smbMounts`. [`State::load`] converts them, and the helper's next save
/// writes the new form. Remove once no such state files are left (they're
/// all from unreleased 0.1.0 builds).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySmbMount {
    address: SmbAddress,
    mount_point: AbsolutePath,
    login: SmbLogin,
    when: MountWhen,
    read_only: bool,
    version: SmbVersion,
    owner: Owner,
}

impl From<LegacySmbMount> for ManagedMount {
    fn from(old: LegacySmbMount) -> Self {
        ManagedMount {
            mount: Mount {
                share: RemoteShare::Smb {
                    address: old.address,
                    login: old.login,
                    version: old.version,
                },
                mount_point: old.mount_point,
                when: old.when,
                read_only: old.read_only,
            },
            owner: old.owner,
        }
    }
}

impl State {
    /// Reads the state file, or an empty state if there isn't one yet.
    pub fn load(path: &Path) -> Result<Self, StateError> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(StateError::Read {
                    path: path.display().to_string(),
                    source,
                });
            }
        };
        let parse_error = |source| StateError::Parse {
            path: path.display().to_string(),
            source,
        };

        let mut value: serde_json::Value = serde_json::from_str(&text).map_err(parse_error)?;
        let legacy = value
            .as_object_mut()
            .and_then(|fields| fields.remove("smbMounts"));
        let mut state: State = serde_json::from_value(value).map_err(parse_error)?;
        if let Some(legacy) = legacy {
            let old: Vec<LegacySmbMount> = serde_json::from_value(legacy).map_err(parse_error)?;
            state.mounts.extend(old.into_iter().map(ManagedMount::from));
        }
        Ok(state)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("the state is always serializable") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbsolutePath, ShareName, SmbAccess};

    #[test]
    fn missing_file_is_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            State::load(&dir.path().join("state.json")).unwrap(),
            State::default()
        );
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let state = State {
            smb_shares: vec![SmbShare {
                name: ShareName::new("Media").unwrap(),
                path: AbsolutePath::new("/srv/media").unwrap(),
                read_only: true,
                access: SmbAccess::Guest,
            }],
            nfs_exports: vec![],
            mounts: vec![],
            firewall_opened: vec![],
        };
        std::fs::write(&path, state.to_json()).unwrap();
        assert_eq!(State::load(&path).unwrap(), state);
    }

    #[test]
    fn mounts_saved_before_nfs_are_converted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let old = r#"{
            "smbShares": [],
            "nfsExports": [],
            "smbMounts": [{
                "address": "//nas.lan/media",
                "mountPoint": "/home/alice/Network/media",
                "login": { "kind": "user", "username": "alice", "domain": null },
                "when": "firstAccess",
                "readOnly": false,
                "version": "automatic",
                "owner": { "uid": 1000, "gid": 1000 }
            }]
        }"#;
        std::fs::write(&path, old).unwrap();

        let state = State::load(&path).unwrap();
        assert_eq!(state.mounts.len(), 1);
        assert_eq!(state.mounts[0].mount.share.address(), "//nas.lan/media");
        assert!(!state.to_json().contains("smbMounts"));
    }
}
