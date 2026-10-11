//! Folders shared from this machine.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::names::{AbsolutePath, InvalidValue, NfsClient, ShareName, UserName};

/// A folder shared over SMB, the protocol Windows, macOS and most NAS boxes speak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SmbShare {
    pub name: ShareName,
    pub path: AbsolutePath,
    pub read_only: bool,
    pub access: SmbAccess,
}

/// Who may connect to a Samba share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SmbAccess {
    /// Anyone on the network, without a password.
    Guest,
    /// Only these users, with their Samba passwords (set separately: Samba
    /// keeps its own, not the login password).
    Users { users: Vec<UserName> },
}

/// A folder shared over NFS, the protocol Linux and other Unix systems speak
/// natively.
///
/// NFS has no user names or passwords: an export trusts the machines listed in
/// `clients`, and those machines' own idea of who their users are. That's why
/// it's modelled by host rather than by user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct NfsExport {
    pub path: AbsolutePath,
    pub read_only: bool,
    pub clients: Vec<NfsClient>,
}

/// Refuses to share a system folder (`/`, `/etc`, ...): their files aren't
/// anyone's to hand out, and some are secret.
pub fn check_shareable(path: &AbsolutePath) -> Result<(), InvalidValue> {
    match path.system_folder() {
        Some(folder) => Err(InvalidValue(format!(
            "{folder} belongs to the system and can't be shared."
        ))),
        None => Ok(()),
    }
}

/// Checks what the individual names and paths can't: that shares don't clash,
/// that each is open to someone, and that none is a system folder.
pub fn check_smb_shares(shares: &[SmbShare]) -> Result<(), InvalidValue> {
    let mut seen = HashSet::new();
    for share in shares {
        check_shareable(&share.path)?;
        // Clients look shares up without regard to case.
        if !seen.insert(share.name.as_str().to_lowercase()) {
            return Err(InvalidValue(format!(
                "There are two shares called \"{}\".",
                share.name
            )));
        }
        if let SmbAccess::Users { users } = &share.access
            && users.is_empty()
        {
            return Err(InvalidValue(format!(
                "Choose who can open \"{}\", or open it to guests.",
                share.name
            )));
        }
    }
    Ok(())
}

/// Checks that no folder is exported twice, that each export names a client,
/// and that none is a system folder.
pub fn check_nfs_exports(exports: &[NfsExport]) -> Result<(), InvalidValue> {
    let mut seen = HashSet::new();
    for export in exports {
        check_shareable(&export.path)?;
        if !seen.insert(export.path.as_str()) {
            return Err(InvalidValue(format!("{} is exported twice.", export.path)));
        }
        if export.clients.is_empty() {
            return Err(InvalidValue(format!(
                "Say which machines may mount {} (use * for any).",
                export.path
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smb(name: &str, access: SmbAccess) -> SmbShare {
        SmbShare {
            name: ShareName::new(name).unwrap(),
            path: AbsolutePath::new("/srv/share").unwrap(),
            read_only: false,
            access,
        }
    }

    #[test]
    fn share_names_clash_regardless_of_case() {
        assert!(
            check_smb_shares(&[
                smb("Media", SmbAccess::Guest),
                smb("Photos", SmbAccess::Guest)
            ])
            .is_ok()
        );
        assert!(
            check_smb_shares(&[
                smb("Media", SmbAccess::Guest),
                smb("media", SmbAccess::Guest)
            ])
            .is_err()
        );
    }

    #[test]
    fn system_folders_are_not_shared() {
        let mut share = smb("Etc", SmbAccess::Guest);
        share.path = AbsolutePath::new("/etc").unwrap();
        assert!(check_smb_shares(&[share]).is_err());
    }

    #[test]
    fn a_share_is_open_to_someone() {
        assert!(check_smb_shares(&[smb("Media", SmbAccess::Users { users: vec![] })]).is_err());
    }

    #[test]
    fn exports_name_clients_once() {
        let export = |clients: &[&str]| NfsExport {
            path: AbsolutePath::new("/srv/media").unwrap(),
            read_only: true,
            clients: clients
                .iter()
                .map(|c| NfsClient::new(*c).unwrap())
                .collect(),
        };
        assert!(check_nfs_exports(&[export(&["*"])]).is_ok());
        assert!(check_nfs_exports(&[export(&[])]).is_err());
        assert!(check_nfs_exports(&[export(&["*"]), export(&["*"])]).is_err());
    }
}
