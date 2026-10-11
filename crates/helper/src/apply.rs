//! Carrying out a request, once polkit has allowed it.
//!
//! Each request updates the state file, regenerates the config file it
//! affects from it, and tells the service to reload. Config files are written
//! before the state, so a failure part-way leaves the state describing what
//! was there before.

use std::io;
use std::path::Path;

use netfoo_core::linux::{Paths, nfs, samba};
use netfoo_core::state::StateError;
use netfoo_core::{InvalidValue, Request, State, check_nfs_exports, check_smb_shares};

use crate::files::write_atomically;
use crate::services::{CommandError, Services};

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("{0}")]
    Invalid(#[from] InvalidValue),
    #[error(transparent)]
    State(#[from] StateError),
    #[error("couldn't write {path}: {source}")]
    Write { path: String, source: io::Error },
    #[error(transparent)]
    Command(#[from] CommandError),
}

pub fn apply(request: &Request, paths: &Paths, services: &impl Services) -> Result<(), ApplyError> {
    match request {
        Request::SetSmbShares { shares } => {
            check_smb_shares(shares)?;
            let mut state = State::load(&paths.state)?;
            state.smb_shares = shares.clone();

            write(&paths.samba_shares, &samba::render_shares(shares))?;
            include_from_smb_conf(paths)?;
            write(&paths.state, &state.to_json())?;
            services.reload_samba()?;
        }
        Request::SetNfsExports { exports } => {
            check_nfs_exports(exports)?;
            let mut state = State::load(&paths.state)?;
            state.nfs_exports = exports.clone();

            write(&paths.nfs_exports, &nfs::render_exports(exports))?;
            write(&paths.state, &state.to_json())?;
            services.reload_nfs_exports()?;
        }
        Request::SetSmbPassword { user, password } => {
            services.set_smb_password(user, password)?;
        }
    }
    Ok(())
}

/// Makes sure smb.conf includes netfoo's shares, creating smb.conf if the
/// system doesn't have one.
fn include_from_smb_conf(paths: &Paths) -> Result<(), ApplyError> {
    let include_path = paths.samba_shares.to_string_lossy();
    let updated = match std::fs::read_to_string(&paths.smb_conf) {
        Ok(current) => samba::with_include(&current, &include_path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Some(samba::new_smb_conf(&include_path)),
        Err(source) => {
            return Err(ApplyError::Write {
                path: paths.smb_conf.display().to_string(),
                source,
            });
        }
    };
    match updated {
        Some(text) => write(&paths.smb_conf, &text),
        None => Ok(()),
    }
}

fn write(path: &Path, contents: &str) -> Result<(), ApplyError> {
    write_atomically(path, contents).map_err(|source| ApplyError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;

    use netfoo_core::{
        AbsolutePath, NfsClient, NfsExport, Secret, ShareName, SmbAccess, SmbShare, UserName,
    };

    use super::*;

    /// Records what would have been run.
    #[derive(Default)]
    struct Recorded(RefCell<Vec<String>>);

    impl Services for Recorded {
        fn reload_samba(&self) -> Result<(), CommandError> {
            self.0.borrow_mut().push("reload samba".into());
            Ok(())
        }
        fn reload_nfs_exports(&self) -> Result<(), CommandError> {
            self.0.borrow_mut().push("reload nfs".into());
            Ok(())
        }
        fn set_smb_password(&self, user: &UserName, _: &Secret) -> Result<(), CommandError> {
            self.0.borrow_mut().push(format!("password for {user}"));
            Ok(())
        }
    }

    fn media_share() -> SmbShare {
        SmbShare {
            name: ShareName::new("Media").unwrap(),
            path: AbsolutePath::new("/srv/media").unwrap(),
            read_only: true,
            access: SmbAccess::Guest,
        }
    }

    #[test]
    fn sharing_over_samba_writes_config_and_reloads() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();

        apply(
            &Request::SetSmbShares {
                shares: vec![media_share()],
            },
            &paths,
            &services,
        )
        .unwrap();

        assert!(
            fs::read_to_string(&paths.samba_shares)
                .unwrap()
                .contains("[Media]")
        );
        assert!(
            fs::read_to_string(&paths.smb_conf)
                .unwrap()
                .contains("include = ")
        );
        assert_eq!(
            State::load(&paths.state).unwrap().smb_shares,
            [media_share()]
        );
        assert_eq!(*services.0.borrow(), ["reload samba"]);
    }

    #[test]
    fn an_existing_smb_conf_is_kept() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        fs::create_dir_all(paths.smb_conf.parent().unwrap()).unwrap();
        fs::write(&paths.smb_conf, "[global]\n   workgroup = HOME\n").unwrap();

        apply(
            &Request::SetSmbShares {
                shares: vec![media_share()],
            },
            &paths,
            &Recorded::default(),
        )
        .unwrap();
        apply(
            &Request::SetSmbShares { shares: vec![] },
            &paths,
            &Recorded::default(),
        )
        .unwrap();

        let smb_conf = fs::read_to_string(&paths.smb_conf).unwrap();
        assert!(smb_conf.starts_with("[global]\n   workgroup = HOME\n"));
        assert_eq!(smb_conf.matches("include = ").count(), 1);
    }

    #[test]
    fn exporting_over_nfs_leaves_samba_alone() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        apply(
            &Request::SetSmbShares {
                shares: vec![media_share()],
            },
            &paths,
            &services,
        )
        .unwrap();

        let export = NfsExport {
            path: AbsolutePath::new("/srv/media").unwrap(),
            read_only: false,
            clients: vec![NfsClient::new("*").unwrap()],
        };
        apply(
            &Request::SetNfsExports {
                exports: vec![export.clone()],
            },
            &paths,
            &services,
        )
        .unwrap();

        let state = State::load(&paths.state).unwrap();
        assert_eq!(state.smb_shares, [media_share()]);
        assert_eq!(state.nfs_exports, [export]);
        assert!(
            fs::read_to_string(&paths.nfs_exports)
                .unwrap()
                .contains("/srv/media *(rw,")
        );
        assert_eq!(*services.0.borrow(), ["reload samba", "reload nfs"]);
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());

        let twice = Request::SetSmbShares {
            shares: vec![media_share(), media_share()],
        };
        assert!(matches!(
            apply(&twice, &paths, &Recorded::default()),
            Err(ApplyError::Invalid(_))
        ));
        assert!(!root.path().join("etc").exists());
    }
}
