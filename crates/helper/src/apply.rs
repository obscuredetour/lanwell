//! Carrying out a request, once polkit has allowed it.
//!
//! Each request updates the state file, regenerates the config files it
//! affects from it, and tells the service to reload. Config files are written
//! before the state, so a failure part-way leaves the state describing what
//! was there before. Mounts are in `mounts.rs`.

use std::io;
use std::path::Path;

use netfoo_core::linux::{Paths, nfs, samba};
use netfoo_core::state::StateError;
use netfoo_core::{InvalidValue, Owner, Request, State, check_nfs_exports, check_smb_shares};

use crate::files::write_atomically;
use crate::mounts;
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
    #[error("{0}")]
    MountPoint(String),
    /// The mount is saved (and will be tried again), but didn't connect now.
    #[error("Saved, but couldn't connect: {0}")]
    Connect(String),
}

/// Carries out `request` for `caller` (whose files a mount's appear as).
pub fn apply(
    request: &Request,
    caller: Owner,
    paths: &Paths,
    services: &impl Services,
) -> Result<(), ApplyError> {
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
        Request::SetSmbMount { mount, password } => {
            mounts::set(mount, password.as_ref(), caller, paths, services)?;
        }
        Request::RemoveSmbMount { mount_point } => {
            mounts::remove(mount_point, paths, services)?;
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

pub(crate) fn write(path: &Path, contents: &str) -> Result<(), ApplyError> {
    write_atomically(path, contents.as_bytes(), 0o644).map_err(|source| ApplyError::Write {
        path: path.display().to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use netfoo_core::{
        AbsolutePath, MountLogin, MountWhen, NfsClient, NfsExport, RemoteName, Secret, ShareName,
        SmbAccess, SmbAddress, SmbMount, SmbShare, SmbVersion, UserName,
    };

    use super::*;

    const ALICE: Owner = Owner {
        uid: 1000,
        gid: 1000,
    };

    /// Records what would have been run.
    #[derive(Default)]
    struct Recorded(RefCell<Vec<String>>);

    impl Recorded {
        fn record(&self, what: String) {
            self.0.borrow_mut().push(what);
        }
    }

    impl Services for Recorded {
        fn reload_samba(&self) -> Result<(), CommandError> {
            self.record("reload samba".into());
            Ok(())
        }
        fn reload_nfs_exports(&self) -> Result<(), CommandError> {
            self.record("reload nfs".into());
            Ok(())
        }
        fn set_smb_password(&self, user: &UserName, _: &Secret) -> Result<(), CommandError> {
            self.record(format!("password for {user}"));
            Ok(())
        }
        fn reload_units(&self) -> Result<(), CommandError> {
            self.record("reload units".into());
            Ok(())
        }
        fn enable_now(&self, unit: &str) -> Result<(), CommandError> {
            self.record(format!("enable {unit}"));
            Ok(())
        }
        fn disable_now(&self, unit: &str) {
            self.record(format!("disable {unit}"));
        }
        fn start(&self, unit: &str) -> Result<(), CommandError> {
            self.record(format!("start {unit}"));
            Ok(())
        }
        fn prepare_mount_point(&self, path: &AbsolutePath) -> Result<(), String> {
            self.record(format!("prepare {path}"));
            Ok(())
        }
    }

    /// Applies `request` as Alice.
    fn run(request: Request, paths: &Paths, services: &Recorded) -> Result<(), ApplyError> {
        apply(&request, ALICE, paths, services)
    }

    fn media_share() -> SmbShare {
        SmbShare {
            name: ShareName::new("Media").unwrap(),
            path: AbsolutePath::new("/srv/media").unwrap(),
            read_only: true,
            access: SmbAccess::Guest,
        }
    }

    fn nas_media(when: MountWhen) -> SmbMount {
        SmbMount {
            address: SmbAddress::new("//nas.lan/media").unwrap(),
            mount_point: AbsolutePath::new("/home/alice/Network/media").unwrap(),
            login: MountLogin::User {
                username: RemoteName::new("alice").unwrap(),
                domain: None,
            },
            when,
            read_only: false,
            version: SmbVersion::Automatic,
        }
    }

    fn password() -> Option<Secret> {
        Some(Secret("hunter2".into()))
    }

    #[test]
    fn sharing_over_samba_writes_config_and_reloads() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();

        let shares = vec![media_share()];
        run(Request::SetSmbShares { shares }, &paths, &services).unwrap();

        let samba_shares = fs::read_to_string(&paths.samba_shares).unwrap();
        assert!(samba_shares.contains("[Media]"));
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

        let services = Recorded::default();
        let shares = vec![media_share()];
        run(Request::SetSmbShares { shares }, &paths, &services).unwrap();
        run(Request::SetSmbShares { shares: vec![] }, &paths, &services).unwrap();

        let smb_conf = fs::read_to_string(&paths.smb_conf).unwrap();
        assert!(smb_conf.starts_with("[global]\n   workgroup = HOME\n"));
        assert_eq!(smb_conf.matches("include = ").count(), 1);
    }

    #[test]
    fn exporting_over_nfs_leaves_samba_alone() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let shares = vec![media_share()];
        run(Request::SetSmbShares { shares }, &paths, &services).unwrap();

        let export = NfsExport {
            path: AbsolutePath::new("/srv/media").unwrap(),
            read_only: false,
            clients: vec![NfsClient::new("*").unwrap()],
        };
        let exports = vec![export.clone()];
        run(Request::SetNfsExports { exports }, &paths, &services).unwrap();

        let state = State::load(&paths.state).unwrap();
        assert_eq!(state.smb_shares, [media_share()]);
        assert_eq!(state.nfs_exports, [export]);
        let nfs_exports = fs::read_to_string(&paths.nfs_exports).unwrap();
        assert!(nfs_exports.contains("/srv/media *(rw,"));
        assert_eq!(*services.0.borrow(), ["reload samba", "reload nfs"]);
    }

    #[test]
    fn invalid_requests_change_nothing() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());

        let shares = vec![media_share(), media_share()];
        let result = run(
            Request::SetSmbShares { shares },
            &paths,
            &Recorded::default(),
        );
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
        assert!(!root.path().join("etc").exists());
    }

    #[test]
    fn mounting_on_first_access() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();

        let mount = nas_media(MountWhen::FirstAccess);
        let request = Request::SetSmbMount {
            mount: mount.clone(),
            password: password(),
        };
        run(request, &paths, &services).unwrap();

        let units = &paths.systemd_units;
        assert!(units.join("home-alice-Network-media.mount").exists());
        assert!(units.join("home-alice-Network-media.automount").exists());

        let credentials = paths.credentials.join("home-alice-Network-media");
        assert!(
            fs::read_to_string(&credentials)
                .unwrap()
                .contains("password=hunter2")
        );
        assert_eq!(
            fs::metadata(&credentials).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&paths.credentials)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let state = State::load(&paths.state).unwrap();
        assert_eq!(state.smb_mounts.len(), 1);
        assert_eq!(state.smb_mounts[0].mount, mount);
        assert_eq!(state.smb_mounts[0].owner, ALICE);
        // No password in the state file, which everyone can read.
        assert!(
            !fs::read_to_string(&paths.state)
                .unwrap()
                .contains("hunter2")
        );

        assert_eq!(
            *services.0.borrow(),
            [
                "disable home-alice-Network-media.automount",
                "disable home-alice-Network-media.mount",
                "prepare /home/alice/Network/media",
                "reload units",
                "enable home-alice-Network-media.automount",
                "start home-alice-Network-media.mount",
            ]
        );
    }

    #[test]
    fn switching_to_startup_drops_the_automount() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let first_access = nas_media(MountWhen::FirstAccess);
        let request = Request::SetSmbMount {
            mount: first_access,
            password: password(),
        };
        run(request, &paths, &services).unwrap();

        // No password: the saved one is kept.
        let startup = nas_media(MountWhen::Startup);
        run(
            Request::SetSmbMount {
                mount: startup,
                password: None,
            },
            &paths,
            &services,
        )
        .unwrap();

        assert!(
            !paths
                .systemd_units
                .join("home-alice-Network-media.automount")
                .exists()
        );
        let credentials = paths.credentials.join("home-alice-Network-media");
        assert!(
            fs::read_to_string(credentials)
                .unwrap()
                .contains("password=hunter2")
        );
        assert_eq!(State::load(&paths.state).unwrap().smb_mounts.len(), 1);
        assert_eq!(
            services.0.borrow().last().unwrap(),
            "enable home-alice-Network-media.mount"
        );
    }

    #[test]
    fn a_new_user_mount_needs_a_password() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let mount = nas_media(MountWhen::FirstAccess);
        let result = run(
            Request::SetSmbMount {
                mount,
                password: None,
            },
            &paths,
            &Recorded::default(),
        );
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
    }

    #[test]
    fn system_folders_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let mut mount = nas_media(MountWhen::FirstAccess);
        mount.mount_point = AbsolutePath::new("/etc").unwrap();
        let services = Recorded::default();
        let result = run(
            Request::SetSmbMount {
                mount,
                password: password(),
            },
            &paths,
            &services,
        );
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
        assert!(services.0.borrow().is_empty());
    }

    #[test]
    fn removing_a_mount_cleans_up() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let mount = nas_media(MountWhen::FirstAccess);
        let mount_point = mount.mount_point.clone();
        run(
            Request::SetSmbMount {
                mount,
                password: password(),
            },
            &paths,
            &services,
        )
        .unwrap();

        run(Request::RemoveSmbMount { mount_point }, &paths, &services).unwrap();

        assert_eq!(fs::read_dir(&paths.systemd_units).unwrap().count(), 0);
        assert!(!paths.credentials.join("home-alice-Network-media").exists());
        assert!(State::load(&paths.state).unwrap().smb_mounts.is_empty());
    }
}
