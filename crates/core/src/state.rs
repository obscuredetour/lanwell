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

use crate::mount::ManagedMount;
use crate::share::{NfsExport, SmbShare};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub smb_shares: Vec<SmbShare>,
    pub nfs_exports: Vec<NfsExport>,
    pub smb_mounts: Vec<ManagedMount>,
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
        serde_json::from_str(&text).map_err(|source| StateError::Parse {
            path: path.display().to_string(),
            source,
        })
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
            smb_mounts: vec![],
        };
        std::fs::write(&path, state.to_json()).unwrap();
        assert_eq!(State::load(&path).unwrap(), state);
    }
}
