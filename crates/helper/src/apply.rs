//! Carrying out a request, once polkit has allowed it.
//!
//! Each request updates the state file, regenerates the config files it
//! affects from it, and tells the service to reload. Config files are written
//! before the state, so a failure part-way leaves the state describing what
//! was there before. Mounts are in `mounts.rs`.

use std::collections::HashMap;
use std::io;
use std::path::Path;

use lanwell_core::linux::mountinfo::mount_point_of;
use lanwell_core::linux::{Paths, nfs, samba};
use lanwell_core::state::StateError;
use lanwell_core::{
    AbsolutePath, FileServer, InvalidValue, Owner, Request, State, check_nfs_exports,
    check_shareable, check_smb_shares,
};

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
            for share in shares {
                real_path(&share.path)?;
            }
            let mut state = State::load(&paths.state)?;
            state.smb_shares = shares.clone();

            write(&paths.samba_shares, &samba::render_shares(shares))?;
            include_from_smb_conf(paths)?;
            write(&paths.state, &state.to_json())?;
            if !shares.is_empty() {
                services.set_running(FileServer::Samba, true)?;
            }
            services.reload_samba()?;
        }
        Request::SetNfsExports { exports } => {
            check_nfs_exports(exports)?;
            let mount_table = services.mount_table();
            let mut drives = HashMap::new();
            for export in exports {
                let drive = drive_of(&export.path, &mount_table)?;
                drives.insert(export.path.clone(), drive);
            }
            let mut state = State::load(&paths.state)?;
            state.nfs_exports = exports.clone();

            let text = nfs::render_exports(exports, |export| drives[&export.path].clone());
            write(&paths.nfs_exports, &text)?;
            write(&paths.state, &state.to_json())?;
            if !exports.is_empty() {
                services.set_running(FileServer::Nfs, true)?;
            }
            services.reload_nfs_exports()?;
        }
        Request::SetSmbPassword { user, password } => {
            if !services.is_person(user) {
                return Err(InvalidValue(format!(
                    "Samba passwords are for people's accounts, and {user} isn't one."
                ))
                .into());
            }
            services.set_smb_password(user, password)?;
        }
        Request::SetMount {
            mount,
            password,
            replaces,
        } => {
            let edit = mounts::Edit {
                password: password.as_ref(),
                replaces: replaces.as_ref(),
            };
            mounts::set(mount, edit, caller, paths, services)?;
        }
        Request::RemoveMount { mount_point } => {
            mounts::remove(mount_point, paths, services)?;
        }
        Request::SetServerRunning { server, running } => {
            if *server == FileServer::Samba && *running {
                ensure_samba_config(paths)?;
            }
            services.set_running(*server, *running)?;
        }
        Request::SetFirewall { server, allow } => {
            let mut state = State::load(&paths.state)?;
            if !allow && !state.firewall_opened.contains(server) {
                return Err(InvalidValue(format!(
                    "Lanwell didn't let {} through the firewall, so it leaves the firewall's rules alone.",
                    server.name()
                ))
                .into());
            }
            services.set_firewall(*server, *allow)?;
            state.firewall_opened.retain(|other| other != server);
            if *allow {
                state.firewall_opened.push(*server);
            }
            write(&paths.state, &state.to_json())?;
        }
    }
    Ok(())
}

/// Where a shared folder really is, following symbolic links, after
/// checking it isn't a system folder (so a link to one can't be shared). A
/// folder that doesn't exist yet has only its name to go on, which
/// `check_shareable` already checked.
fn real_path(path: &AbsolutePath) -> Result<String, ApplyError> {
    let Ok(real) = std::fs::canonicalize(path.as_str()) else {
        return Ok(path.as_str().to_string());
    };
    let real = AbsolutePath::new(real.to_string_lossy())?;
    check_shareable(&real)?;
    Ok(real.as_str().to_string())
}

/// The drive an export is on (its mount point), or `None` if it's on the
/// system's own. A folder in /run/media whose drive isn't mounted is refused:
/// exporting it would export the empty folder the drive is mounted on.
fn drive_of(path: &AbsolutePath, mount_table: &str) -> Result<Option<String>, ApplyError> {
    let real = real_path(path)?;
    let drive = mount_point_of(mount_table, &real);
    if real.starts_with("/run/media/") && !drive.starts_with("/run/media/") {
        return Err(InvalidValue(format!(
            "{path} isn't on a mounted drive right now. Mount the drive, then export it."
        ))
        .into());
    }
    Ok((drive != "/").then_some(drive))
}

/// Makes sure Samba has a config to start with. Some distributions (Arch)
/// install it without an smb.conf, and it won't start until there is one:
/// until the first share was added, Start just failed there.
fn ensure_samba_config(paths: &Paths) -> Result<(), ApplyError> {
    if !paths.samba_shares.exists() {
        let state = State::load(&paths.state)?;
        write(
            &paths.samba_shares,
            &samba::render_shares(&state.smb_shares),
        )?;
    }
    include_from_smb_conf(paths)
}

/// Makes sure smb.conf includes lanwell's shares, creating smb.conf if the
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

    use lanwell_core::{
        AbsolutePath, Mount, MountWhen, NfsAddress, NfsClient, NfsExport, NfsVersion, RemoteName,
        RemoteShare, Secret, ShareName, SmbAccess, SmbAddress, SmbLogin, SmbShare, SmbVersion,
        UserName,
    };

    use super::*;

    const ALICE: Owner = Owner {
        uid: 1000,
        gid: 1000,
    };

    /// Records what would have been run.
    #[derive(Default)]
    struct Recorded {
        log: RefCell<Vec<String>>,
        /// Units that exist without lanwell (an admin's, or mounted now).
        units_in_use: Vec<String>,
        /// /proc/self/mountinfo, as far as the test needs it.
        mount_table: String,
    }

    impl Recorded {
        fn record(&self, what: String) {
            self.log.borrow_mut().push(what);
        }

        fn log(&self) -> Vec<String> {
            self.log.borrow().clone()
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
        fn unit_in_use(&self, unit: &str) -> bool {
            self.units_in_use.iter().any(|other| other == unit)
        }
        fn prepare_mount_point(&self, path: &AbsolutePath) -> Result<(), String> {
            self.record(format!("prepare {path}"));
            Ok(())
        }
        fn is_person(&self, user: &UserName) -> bool {
            user.as_str() != "root"
        }
        fn mount_table(&self) -> String {
            self.mount_table.clone()
        }
        fn set_running(&self, server: FileServer, running: bool) -> Result<(), CommandError> {
            let verb = if running { "start" } else { "stop" };
            self.record(format!("{verb} {server:?}"));
            Ok(())
        }
        fn set_firewall(&self, server: FileServer, allow: bool) -> Result<(), CommandError> {
            let verb = if allow { "allow" } else { "disallow" };
            self.record(format!("{verb} {server:?}"));
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

    fn nas_media(when: MountWhen) -> Mount {
        Mount {
            share: RemoteShare::Smb {
                address: SmbAddress::new("//nas.lan/media").unwrap(),
                login: SmbLogin::User {
                    username: RemoteName::new("alice").unwrap(),
                    domain: None,
                },
                version: SmbVersion::Automatic,
            },
            mount_point: AbsolutePath::new("/home/alice/Network/media").unwrap(),
            when,
            read_only: false,
        }
    }

    fn nfs_media() -> Mount {
        Mount {
            share: RemoteShare::Nfs {
                address: NfsAddress::new("nas.lan:/srv/media").unwrap(),
                version: NfsVersion::Automatic,
            },
            ..nas_media(MountWhen::FirstAccess)
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
        assert_eq!(services.log(), ["start Samba", "reload samba"]);
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
        assert_eq!(
            services.log(),
            ["start Samba", "reload samba", "start Nfs", "reload nfs"]
        );
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
        let request = Request::SetMount {
            mount: mount.clone(),
            password: password(),
            replaces: None,
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
        assert_eq!(state.mounts.len(), 1);
        assert_eq!(state.mounts[0].mount, mount);
        assert_eq!(state.mounts[0].owner, ALICE);
        // No password in the state file, which everyone can read.
        assert!(
            !fs::read_to_string(&paths.state)
                .unwrap()
                .contains("hunter2")
        );

        assert_eq!(
            services.log(),
            [
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
        let request = Request::SetMount {
            mount: first_access,
            password: password(),
            replaces: None,
        };
        run(request, &paths, &services).unwrap();

        // No password: the saved one is kept.
        let startup = nas_media(MountWhen::Startup);
        run(
            Request::SetMount {
                mount: startup,
                password: None,
                replaces: None,
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
        assert_eq!(State::load(&paths.state).unwrap().mounts.len(), 1);
        assert_eq!(
            services.log().last().unwrap(),
            "enable home-alice-Network-media.mount"
        );
    }

    #[test]
    fn a_new_user_mount_needs_a_password() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let mount = nas_media(MountWhen::FirstAccess);
        let result = run(
            Request::SetMount {
                mount,
                password: None,
                replaces: None,
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
            Request::SetMount {
                mount,
                password: password(),
                replaces: None,
            },
            &paths,
            &services,
        );
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
        assert!(services.log().is_empty());
    }

    #[test]
    fn removing_a_mount_cleans_up() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let mount = nas_media(MountWhen::FirstAccess);
        let mount_point = mount.mount_point.clone();
        run(
            Request::SetMount {
                mount,
                password: password(),
                replaces: None,
            },
            &paths,
            &services,
        )
        .unwrap();

        run(Request::RemoveMount { mount_point }, &paths, &services).unwrap();

        assert_eq!(fs::read_dir(&paths.systemd_units).unwrap().count(), 0);
        assert!(!paths.credentials.join("home-alice-Network-media").exists());
        assert!(State::load(&paths.state).unwrap().mounts.is_empty());
    }

    #[test]
    fn mounts_lanwell_did_not_make_are_left_alone() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded {
            units_in_use: vec!["home-alice-Network-media.mount".into()],
            ..Recorded::default()
        };

        let mount = nas_media(MountWhen::FirstAccess);
        let mount_point = mount.mount_point.clone();
        let request = Request::SetMount {
            mount,
            password: password(),
            replaces: None,
        };
        assert!(matches!(
            run(request, &paths, &services),
            Err(ApplyError::MountPoint(_))
        ));

        let result = run(Request::RemoveMount { mount_point }, &paths, &services);
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
        // Nothing stopped, nothing written.
        assert!(services.log().is_empty());
        assert!(!paths.systemd_units.exists());
    }

    #[test]
    fn samba_passwords_are_for_people() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let user = UserName::new("root").unwrap();
        let request = Request::SetSmbPassword {
            user,
            password: Secret("pw".into()),
        };
        assert!(matches!(
            run(request, &paths, &services),
            Err(ApplyError::Invalid(_))
        ));
        assert!(services.log().is_empty());
    }

    #[test]
    fn links_to_system_folders_are_not_shared() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let link = root.path().join("media");
        std::os::unix::fs::symlink("/etc", &link).unwrap();

        let mut share = media_share();
        share.path = AbsolutePath::new(link.to_string_lossy()).unwrap();
        let result = run(
            Request::SetSmbShares {
                shares: vec![share],
            },
            &paths,
            &Recorded::default(),
        );
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
    }

    #[test]
    fn mounting_nfs_needs_no_credentials() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();

        // First SMB, with a saved password; then NFS on the same folder.
        let smb = nas_media(MountWhen::FirstAccess);
        run(
            Request::SetMount {
                mount: smb,
                password: password(),
                replaces: None,
            },
            &paths,
            &services,
        )
        .unwrap();
        let nfs = nfs_media();
        run(
            Request::SetMount {
                mount: nfs.clone(),
                password: None,
                replaces: None,
            },
            &paths,
            &services,
        )
        .unwrap();

        let unit = fs::read_to_string(paths.systemd_units.join("home-alice-Network-media.mount"));
        assert!(unit.unwrap().contains("Type=nfs\n"));
        // The SMB password isn't kept once nothing needs it.
        assert!(!paths.credentials.join("home-alice-Network-media").exists());
        let state = State::load(&paths.state).unwrap();
        assert_eq!(state.mounts.len(), 1);
        assert_eq!(state.mounts[0].mount, nfs);
    }

    #[test]
    fn exports_on_a_drive_wait_for_it() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let films = NfsExport {
            path: AbsolutePath::new("/run/media/main/films").unwrap(),
            read_only: true,
            clients: vec![NfsClient::new("*").unwrap()],
        };

        // The drive isn't mounted: refused.
        let unmounted = Recorded::default();
        let exports = vec![films.clone()];
        let result = run(Request::SetNfsExports { exports }, &paths, &unmounted);
        assert!(matches!(result, Err(ApplyError::Invalid(_))));

        // Mounted: exported only while it is.
        let mounted = Recorded {
            mount_table: "62 51 259:0 / /run/media/main rw - ext4 /dev/nvme0n1 rw\n".into(),
            ..Recorded::default()
        };
        run(
            Request::SetNfsExports {
                exports: vec![films],
            },
            &paths,
            &mounted,
        )
        .unwrap();
        let text = fs::read_to_string(&paths.nfs_exports).unwrap();
        assert!(
            text.contains("/run/media/main/films *(ro,sync,no_subtree_check,mp=/run/media/main)")
        );
    }

    #[test]
    fn moving_a_mount_keeps_its_password() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let before = nas_media(MountWhen::FirstAccess);
        let request = Request::SetMount {
            mount: before.clone(),
            password: password(),
            replaces: None,
        };
        run(request, &paths, &services).unwrap();

        let mut after = before.clone();
        after.mount_point = AbsolutePath::new("/home/alice/NAS/media").unwrap();
        let request = Request::SetMount {
            mount: after.clone(),
            password: None,
            replaces: Some(before.mount_point.clone()),
        };
        run(request, &paths, &services).unwrap();

        // The old folder's units and password are gone; the new one has both.
        assert!(
            !paths
                .systemd_units
                .join("home-alice-Network-media.mount")
                .exists()
        );
        assert!(!paths.credentials.join("home-alice-Network-media").exists());
        assert!(
            paths
                .systemd_units
                .join("home-alice-NAS-media.mount")
                .exists()
        );
        let credentials = paths.credentials.join("home-alice-NAS-media");
        assert!(
            fs::read_to_string(credentials)
                .unwrap()
                .contains("password=hunter2")
        );

        let state = State::load(&paths.state).unwrap();
        assert_eq!(state.mounts.len(), 1);
        assert_eq!(state.mounts[0].mount, after);
    }

    #[test]
    fn only_lanwell_mounts_can_be_moved() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let request = Request::SetMount {
            mount: nas_media(MountWhen::FirstAccess),
            password: password(),
            replaces: Some(AbsolutePath::new("/mnt/data").unwrap()),
        };
        let result = run(request, &paths, &services);
        assert!(matches!(result, Err(ApplyError::Invalid(_))));
        assert!(services.log().is_empty());
    }

    #[test]
    fn starting_samba_gives_it_a_config() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let start = Request::SetServerRunning {
            server: FileServer::Samba,
            running: true,
        };

        // A fresh install with no smb.conf (Arch): Samba won't start without one.
        run(start.clone(), &paths, &services).unwrap();
        let smb_conf = fs::read_to_string(&paths.smb_conf).unwrap();
        assert!(smb_conf.contains("[global]"));
        assert!(smb_conf.contains("include = "));
        assert!(paths.samba_shares.exists());
        assert_eq!(services.log(), ["start Samba"]);

        // Started again later: the files are left as they are.
        fs::write(&paths.smb_conf, "[global]\n   workgroup = HOME\n").unwrap();
        run(start, &paths, &services).unwrap();
        let smb_conf = fs::read_to_string(&paths.smb_conf).unwrap();
        assert!(smb_conf.starts_with("[global]\n   workgroup = HOME\n"));
        assert_eq!(smb_conf.matches("include = ").count(), 1);

        // Stopping, or NFS, doesn't touch Samba's files.
        let stop = Request::SetServerRunning {
            server: FileServer::Samba,
            running: false,
        };
        let nfs = Request::SetServerRunning {
            server: FileServer::Nfs,
            running: true,
        };
        let other_root = tempfile::tempdir().unwrap();
        let other = Paths::under(other_root.path());
        run(stop, &other, &services).unwrap();
        run(nfs, &other, &services).unwrap();
        assert!(!other.smb_conf.exists());
    }

    #[test]
    fn removing_every_share_leaves_samba_running() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        run(Request::SetSmbShares { shares: vec![] }, &paths, &services).unwrap();
        // Other shares (in smb.conf) may still need it; stopping it is a choice.
        assert_eq!(services.log(), ["reload samba"]);
    }

    #[test]
    fn lanwell_remembers_what_it_let_through() {
        let root = tempfile::tempdir().unwrap();
        let paths = Paths::under(root.path());
        let services = Recorded::default();
        let set = |allow| Request::SetFirewall {
            server: FileServer::Nfs,
            allow,
        };

        // Taking back what lanwell didn't open is refused.
        assert!(matches!(
            run(set(false), &paths, &services),
            Err(ApplyError::Invalid(_))
        ));

        run(set(true), &paths, &services).unwrap();
        assert_eq!(
            State::load(&paths.state).unwrap().firewall_opened,
            [FileServer::Nfs]
        );
        run(set(false), &paths, &services).unwrap();
        assert!(
            State::load(&paths.state)
                .unwrap()
                .firewall_opened
                .is_empty()
        );
        assert_eq!(services.log(), ["allow Nfs", "disallow Nfs"]);
    }
}
