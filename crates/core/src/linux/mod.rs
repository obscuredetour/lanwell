//! The Linux config files netfoo writes, and where they live.
//!
//! netfoo writes only files of its own, plus one `include` line in smb.conf.
//! Everything else in the system's config is left as it was.

pub mod cifs;
pub mod mountinfo;
pub mod nfs;
pub mod nfs_client;
pub mod samba;
pub mod systemd;

/// A plain explanation of why a mount failed, from what its mount tool
/// (mount.cifs or mount.nfs) logged. `None` if there's nothing to go on.
pub fn explain_mount_failure(log: &str) -> Option<String> {
    cifs::explain_mount_failure(log).or_else(|| nfs_client::explain_mount_failure(log))
}

use std::path::{Path, PathBuf};

/// Where the helper is installed: by a package, or by `netfoo-helper install`
/// (the app's Install button). The D-Bus and systemd files name this path.
pub const HELPER_PATH: &str = "/usr/lib/netfoo/netfoo-helper";

/// Where everything is on a Linux system. The helper uses [`Paths::system`];
/// tests point it at a temporary directory.
#[derive(Debug, Clone)]
pub struct Paths {
    /// netfoo's own record of what it manages (see [`crate::state`]).
    pub state: PathBuf,
    /// The Samba shares netfoo manages, included from smb.conf.
    pub samba_shares: PathBuf,
    /// Samba's main config file, which gets one `include` line.
    pub smb_conf: PathBuf,
    /// The NFS exports netfoo manages. `exportfs` reads every file in /etc/exports.d.
    pub nfs_exports: PathBuf,
    /// Where the mounts' systemd units go (the administrator's units, not a package's).
    pub systemd_units: PathBuf,
    /// The mounts' credentials files, readable only by root.
    pub credentials: PathBuf,
}

impl Paths {
    pub fn system() -> Self {
        Self {
            state: "/etc/netfoo/state.json".into(),
            samba_shares: "/etc/samba/netfoo.conf".into(),
            smb_conf: "/etc/samba/smb.conf".into(),
            nfs_exports: "/etc/exports.d/netfoo.exports".into(),
            systemd_units: "/etc/systemd/system".into(),
            credentials: "/etc/netfoo/credentials".into(),
        }
    }

    /// The same layout under `root`, for tests.
    pub fn under(root: &Path) -> Self {
        let system = Self::system();
        let reroot = |path: PathBuf| root.join(path.strip_prefix("/").unwrap());
        Self {
            state: reroot(system.state),
            samba_shares: reroot(system.samba_shares),
            smb_conf: reroot(system.smb_conf),
            nfs_exports: reroot(system.nfs_exports),
            systemd_units: reroot(system.systemd_units),
            credentials: reroot(system.credentials),
        }
    }
}

/// The first line of every file netfoo generates.
pub const GENERATED_HEADER: &str = "# Written by netfoo from /etc/netfoo/state.json. Changes made here will be overwritten:\n# use netfoo to change these shares.\n";
