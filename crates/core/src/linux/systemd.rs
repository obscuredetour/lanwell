//! Mounts as systemd units, and the credentials file an SMB mount signs in with.
//!
//! Each mount is a `.mount` unit, plus an `.automount` unit when it mounts on
//! first access. They're named after the mount point, the way systemd
//! requires (`/home/alice/Network/media` is `home-alice-Network-media.mount`).
//! Units live in /etc/systemd/system, so they're an administrator's, like
//! fstab entries, without touching fstab.

use crate::linux::GENERATED_HEADER;
use crate::mount::{ManagedMount, MountWhen, NfsVersion, Owner, RemoteShare, SmbLogin, SmbVersion};
use crate::names::AbsolutePath;
use crate::request::Secret;

/// How long mounting may take before systemd gives up, and so how long
/// startup can wait for a server that's off ([`MountWhen::Startup`]).
pub const MOUNT_TIMEOUT_SECONDS: u32 = 30;

/// The unit names for a mount point (`systemd-escape --path`, with a suffix).
pub struct UnitNames {
    /// `home-alice-Network-media.mount`
    pub mount: String,
    /// `home-alice-Network-media.automount`
    pub automount: String,
    /// `home-alice-Network-media`, which also names its credentials file.
    pub stem: String,
}

impl UnitNames {
    pub fn for_mount_point(path: &AbsolutePath) -> Self {
        let stem = escape_path(path.as_str());
        Self {
            mount: format!("{stem}.mount"),
            automount: format!("{stem}.automount"),
            stem,
        }
    }
}

/// systemd's escaping of a path into a unit name: slashes become dashes, and
/// anything but letters, digits, `:`, `_` and (not first) `.` becomes `\xNN`.
fn escape_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    let mut out = String::new();
    for (i, byte) in trimmed.bytes().enumerate() {
        match byte {
            b'/' => out.push('-'),
            b'.' if i > 0 => out.push('.'),
            b if b.is_ascii_alphanumeric() || b == b':' || b == b'_' => out.push(b as char),
            b => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    out
}

/// The `.mount` unit. `credentials` is the path of its credentials file, used
/// when an SMB mount signs in as a user.
pub fn render_mount_unit(managed: &ManagedMount, credentials: &str) -> String {
    let mount = &managed.mount;
    let (fs_type, mut options) = match &mount.share {
        RemoteShare::Smb { login, version, .. } => (
            "cifs",
            smb_options(login, *version, managed.owner, credentials),
        ),
        RemoteShare::Nfs { version, .. } => ("nfs", nfs_options(*version)),
    };
    if mount.read_only {
        options.push("ro".into());
    }
    // The server decides what's on the share, so nothing on it may act as
    // another user (setuid) or as a device.
    options.push("nosuid,nodev".into());

    let install = match mount.when {
        MountWhen::Startup => "\n[Install]\nWantedBy=remote-fs.target\n",
        // The .automount unit is the one that's enabled.
        MountWhen::FirstAccess => "",
    };

    format!(
        "{GENERATED_HEADER}\n\
         [Unit]\n\
         Description=lanwell: {address}\n\
         Wants=network-online.target\n\
         After=network-online.target\n\
         \n\
         [Mount]\n\
         What={address}\n\
         Where={where_}\n\
         Type={fs_type}\n\
         Options={options}\n\
         TimeoutSec={MOUNT_TIMEOUT_SECONDS}\n\
         {install}",
        address = unit_value(mount.share.address()),
        where_ = unit_value(mount.mount_point.as_str()),
        options = unit_value(&options.join(",")),
    )
}

/// mount.cifs's options: how to sign in, whose the files are, and the version.
fn smb_options(
    login: &SmbLogin,
    version: SmbVersion,
    owner: Owner,
    credentials: &str,
) -> Vec<String> {
    let mut options = vec![match login {
        SmbLogin::Guest => "guest".to_string(),
        SmbLogin::User { .. } => format!("credentials={credentials}"),
    }];
    // Every file appears as the person who added the mount, and only theirs to
    // read. force*: even if the server sends its own owners (Unix extensions).
    options.push(format!(
        "uid={},gid={},forceuid,forcegid",
        owner.uid, owner.gid
    ));
    options.push("file_mode=0600,dir_mode=0700,iocharset=utf8".into());
    if let Some(version) = version.option() {
        options.push(format!("vers={version}"));
    }
    options
}

/// mount.nfs's options. NFS has no login or owner options: the server
/// decides who may mount, and files keep the server's user IDs.
fn nfs_options(version: NfsVersion) -> Vec<String> {
    version
        .option()
        .map(|version| format!("vers={version}"))
        .into_iter()
        .collect()
}

/// The `.automount` unit, for [`MountWhen::FirstAccess`]: it mounts the share
/// when something first opens the folder, and keeps it mounted.
pub fn render_automount_unit(managed: &ManagedMount) -> String {
    format!(
        "{GENERATED_HEADER}\n\
         [Unit]\n\
         Description=lanwell: {address} (mounted when first opened)\n\
         \n\
         [Automount]\n\
         Where={where_}\n\
         \n\
         [Install]\n\
         WantedBy=remote-fs.target\n",
        address = unit_value(managed.mount.share.address()),
        where_ = unit_value(managed.mount.mount_point.as_str()),
    )
}

/// `%` starts a specifier in unit files; a literal one is written `%%`.
fn unit_value(value: &str) -> String {
    value.replace('%', "%%")
}

/// The credentials file mount.cifs reads (only root may read it). `None` for
/// a guest login, which needs none.
pub fn render_credentials(login: &SmbLogin, password: &Secret) -> Option<String> {
    let SmbLogin::User { username, domain } = login else {
        return None;
    };
    let mut out = format!("username={username}\npassword={}\n", password.expose());
    if let Some(domain) = domain {
        out.push_str(&format!("domain={domain}\n"));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mount::{Mount, NfsAddress, SmbAddress};
    use crate::names::RemoteName;

    fn smb_mount(when: MountWhen, login: SmbLogin, version: SmbVersion) -> ManagedMount {
        managed(
            when,
            RemoteShare::Smb {
                address: SmbAddress::new("//nas.lan/media").unwrap(),
                login,
                version,
            },
        )
    }

    fn managed(when: MountWhen, share: RemoteShare) -> ManagedMount {
        ManagedMount {
            mount: Mount {
                share,
                mount_point: AbsolutePath::new("/home/alice/Network/media").unwrap(),
                when,
                read_only: false,
            },
            owner: Owner {
                uid: 1000,
                gid: 1000,
            },
        }
    }

    fn alice() -> SmbLogin {
        SmbLogin::User {
            username: RemoteName::new("alice").unwrap(),
            domain: None,
        }
    }

    #[test]
    fn unit_names_match_systemd_escape() {
        // Checked against `systemd-escape --path`.
        let names =
            |path: &str| UnitNames::for_mount_point(&AbsolutePath::new(path).unwrap()).mount;
        assert_eq!(
            names("/home/alice/Network/media"),
            "home-alice-Network-media.mount"
        );
        assert_eq!(names("/mnt/My Media"), "mnt-My\\x20Media.mount");
        assert_eq!(names("/mnt/nas-media"), "mnt-nas\\x2dmedia.mount");
        assert_eq!(names("/mnt/.hidden"), "mnt-.hidden.mount");
        assert_eq!(names("/.hidden"), "\\x2ehidden.mount");
        assert_eq!(names("/mnt/café"), "mnt-caf\\xc3\\xa9.mount");
    }

    #[test]
    fn smb_on_first_access() {
        let managed = smb_mount(MountWhen::FirstAccess, alice(), SmbVersion::Automatic);
        let unit = render_mount_unit(
            &managed,
            "/etc/lanwell/credentials/home-alice-Network-media",
        );
        assert!(unit.contains("What=//nas.lan/media\n"));
        assert!(unit.contains("Where=/home/alice/Network/media\n"));
        assert!(unit.contains("Type=cifs\n"));
        assert!(unit.contains(
            "Options=credentials=/etc/lanwell/credentials/home-alice-Network-media,uid=1000,gid=1000,forceuid,forcegid,file_mode=0600,dir_mode=0700,iocharset=utf8,nosuid,nodev\n"
        ));
        // The automount is what's enabled, not the mount.
        assert!(!unit.contains("[Install]"));
        assert!(
            render_automount_unit(&managed)
                .contains("[Automount]\nWhere=/home/alice/Network/media\n")
        );
    }

    #[test]
    fn smb_at_startup_as_guest() {
        let mut managed = smb_mount(MountWhen::Startup, SmbLogin::Guest, SmbVersion::V3_0);
        managed.mount.read_only = true;
        let unit = render_mount_unit(&managed, "unused");
        assert!(unit.contains("Options=guest,uid=1000,gid=1000,forceuid,forcegid,file_mode=0600,dir_mode=0700,iocharset=utf8,vers=3.0,ro,nosuid,nodev\n"));
        assert!(unit.ends_with("[Install]\nWantedBy=remote-fs.target\n"));
    }

    #[test]
    fn nfs() {
        let share = |version| RemoteShare::Nfs {
            address: NfsAddress::new("nas.lan:/srv/media").unwrap(),
            version,
        };
        let unit = render_mount_unit(
            &managed(MountWhen::FirstAccess, share(NfsVersion::Automatic)),
            "",
        );
        assert!(unit.contains("What=nas.lan:/srv/media\n"));
        assert!(unit.contains("Type=nfs\n"));
        assert!(unit.contains("Options=nosuid,nodev\n"));

        let unit = render_mount_unit(&managed(MountWhen::FirstAccess, share(NfsVersion::V3)), "");
        assert!(unit.contains("Options=vers=3,nosuid,nodev\n"));
    }

    #[test]
    fn percent_signs_are_escaped() {
        let share = RemoteShare::Smb {
            address: SmbAddress::new("//nas.lan/100%").unwrap(),
            login: SmbLogin::Guest,
            version: SmbVersion::Automatic,
        };
        let unit = render_mount_unit(&managed(MountWhen::FirstAccess, share), "");
        assert!(unit.contains("What=//nas.lan/100%%\n"));
    }

    #[test]
    fn credentials() {
        let login = SmbLogin::User {
            username: RemoteName::new("Alice").unwrap(),
            domain: Some(RemoteName::new("HOME").unwrap()),
        };
        assert_eq!(
            render_credentials(&login, &Secret("pw".into())).unwrap(),
            "username=Alice\npassword=pw\ndomain=HOME\n"
        );
        assert_eq!(
            render_credentials(&SmbLogin::Guest, &Secret("pw".into())),
            None
        );
    }
}
