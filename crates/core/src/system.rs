//! What's installed on this machine. Everything here is read without root.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Serialize;

use crate::fileserver::{FileServer, FileServerStatus, Firewall, FirewallAccess};
use crate::linux::firewall;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Os {
    Linux,
    Macos,
    Other,
}

/// What lanwell can do here, for the UI to explain what's missing.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: Os,
    /// Samba's server (`smbd`) is installed.
    pub samba_installed: bool,
    /// The NFS server's tools (`exportfs`) are installed.
    pub nfs_server_installed: bool,
    /// cifs-utils (`mount.cifs`) is installed, which mounting SMB shares needs.
    pub cifs_installed: bool,
    /// Samba's client (`smbclient`) is installed, which listing a server's shares needs.
    pub smbclient_installed: bool,
    /// The NFS client (`mount.nfs`) is installed, which mounting NFS exports needs.
    pub nfs_client_installed: bool,
    /// `showmount` is installed, which listing a server's NFS exports needs.
    pub showmount_installed: bool,
    /// This user's ID. An NFS mount's files are theirs only where the server
    /// gives them the same one.
    pub user_id: Option<u32>,
    /// The user's home folder, where mounts go by default (`~/Network/<share>`).
    pub home_dir: Option<String>,
}

impl SystemInfo {
    pub fn detect() -> Self {
        let os = if cfg!(target_os = "linux") {
            Os::Linux
        } else if cfg!(target_os = "macos") {
            Os::Macos
        } else {
            Os::Other
        };
        Self {
            os,
            samba_installed: find_program("smbd").is_some(),
            nfs_server_installed: find_program("exportfs").is_some(),
            cifs_installed: find_program("mount.cifs").is_some(),
            smbclient_installed: find_program("smbclient").is_some(),
            nfs_client_installed: find_program("mount.nfs").is_some(),
            showmount_installed: find_program("showmount").is_some(),
            user_id: user_id(),
            home_dir: std::env::var("HOME").ok(),
        }
    }
}

/// Whether the privileged helper is installed, and whether it's the version
/// this app was built with (the two are released together).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HelperStatus {
    Ready {
        version: String,
    },
    Missing,
    /// Installed, but a different version (`installed`).
    Outdated {
        installed: String,
    },
    /// lanwell can't install a helper on this system yet.
    Unsupported,
}

impl HelperStatus {
    /// Asks the installed helper its version (`lanwell-helper --version`, which
    /// needs no root) and compares it with `expected`.
    ///
    /// `bundled` is the helper that came with the app, if there is one beside
    /// it. Builds of the same version can differ (in development they always
    /// share one), so if it isn't byte for byte the installed one, the
    /// installed one is outdated too.
    pub fn detect(expected: &str, bundled: Option<&Path>) -> Self {
        if !cfg!(target_os = "linux") {
            return Self::Unsupported;
        }
        let Ok(output) = std::process::Command::new(crate::linux::HELPER_PATH)
            .arg("--version")
            .output()
        else {
            return Self::Missing;
        };
        // "lanwell-helper 0.1.0"
        let stdout = String::from_utf8_lossy(&output.stdout);
        let installed = stdout.split_whitespace().last().unwrap_or_default();
        let same_build = bundled
            .is_none_or(|bundled| same_contents(bundled, Path::new(crate::linux::HELPER_PATH)));
        if installed == expected && same_build {
            Self::Ready {
                version: installed.to_string(),
            }
        } else {
            Self::Outdated {
                installed: installed.to_string(),
            }
        }
    }
}

/// Whether two files hold the same bytes (comparing sizes first, which
/// usually settles it).
fn same_contents(a: &Path, b: &Path) -> bool {
    let size = |path: &Path| std::fs::metadata(path).map(|m| m.len()).ok();
    if size(a).is_none() || size(a) != size(b) {
        return false;
    }
    matches!((std::fs::read(a), std::fs::read(b)), (Ok(a), Ok(b)) if a == b)
}

/// The user this process runs as (who owns /proc/self).
fn user_id() -> Option<u32> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata("/proc/self")
            .ok()
            .map(|metadata| metadata.uid())
    }
    #[cfg(not(unix))]
    None
}

/// The systemd unit that runs `server` here, if it's installed.
pub fn installed_unit(server: FileServer) -> Option<&'static str> {
    server
        .units()
        .iter()
        .copied()
        .find(|unit| systemctl_property(unit, "LoadState").as_deref() == Some("loaded"))
}

/// One property of a unit, as `systemctl show` reports it (no root needed).
fn systemctl_property(unit: &str, property: &str) -> Option<String> {
    let property = format!("--property={property}");
    let output = Command::new("systemctl")
        .args(["show", &property, "--value", "--", unit])
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// The firewall in charge: firewalld if it's running, otherwise ufw if it's
/// switched on.
pub fn detect_firewall() -> Firewall {
    if systemctl_property("firewalld.service", "ActiveState").as_deref() == Some("active") {
        return Firewall::Firewalld;
    }
    let ufw_on = find_program("ufw").is_some()
        && fs::read_to_string("/etc/ufw/ufw.conf").is_ok_and(|conf| firewall::ufw_enabled(&conf));
    if ufw_on {
        Firewall::Ufw
    } else {
        Firewall::None
    }
}

impl FileServerStatus {
    /// What `server` is doing now. `opened_by_lanwell` is whether lanwell's state
    /// says it let it through the firewall.
    pub fn detect(server: FileServer, opened_by_lanwell: bool) -> Self {
        let unit = installed_unit(server);
        let property = |name| unit.and_then(|unit| systemctl_property(unit, name));
        let firewall = detect_firewall();
        let access = match firewall {
            // Nothing in the way.
            Firewall::None => FirewallAccess::Allowed { by_lanwell: false },
            Firewall::Ufw => ufw_access(server, opened_by_lanwell),
            Firewall::Firewalld => firewalld_access(server, opened_by_lanwell),
        };
        Self {
            installed: unit.is_some(),
            running: property("ActiveState").as_deref() == Some("active"),
            enabled: property("UnitFileState").as_deref() == Some("enabled"),
            firewall,
            access,
        }
    }
}

/// From ufw's saved rules, where they can be read (some distributions keep
/// them from everyone but root).
fn ufw_access(server: FileServer, by_lanwell: bool) -> FirewallAccess {
    let rules: Vec<String> = ["/etc/ufw/user.rules", "/etc/ufw/user6.rules"]
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect();
    if rules.is_empty() {
        FirewallAccess::Unknown
    } else if rules
        .iter()
        .any(|rules| firewall::ufw_allows(rules, server.port()))
    {
        FirewallAccess::Allowed { by_lanwell }
    } else {
        FirewallAccess::Blocked
    }
}

/// From `firewall-cmd --query-service`, which says yes (0) or no (1).
fn firewalld_access(server: FileServer, by_lanwell: bool) -> FirewallAccess {
    let Some(firewall_cmd) = find_program("firewall-cmd") else {
        return FirewallAccess::Unknown;
    };
    let query = format!("--query-service={}", server.firewalld_service());
    match Command::new(firewall_cmd)
        .arg(query)
        .output()
        .map(|output| output.status.code())
    {
        Ok(Some(0)) => FirewallAccess::Allowed { by_lanwell },
        Ok(Some(1)) => FirewallAccess::Blocked,
        _ => FirewallAccess::Unknown,
    }
}

/// Where a system program is installed. Looks in the standard directories
/// rather than `PATH`, since `sbin` often isn't on a user's `PATH`.
pub fn find_program(name: &str) -> Option<std::path::PathBuf> {
    [
        "/usr/bin",
        "/usr/sbin",
        "/bin",
        "/sbin",
        "/usr/local/bin",
        "/usr/local/sbin",
    ]
    .iter()
    .map(|dir| Path::new(dir).join(name))
    .find(|path| path.is_file())
}

/// People's accounts on this machine (not system accounts), from /etc/passwd:
/// the users a share can be opened to.
pub fn local_users() -> Vec<String> {
    std::fs::read_to_string("/etc/passwd")
        .map(|passwd| parse_local_users(&passwd))
        .unwrap_or_default()
}

fn parse_local_users(passwd: &str) -> Vec<String> {
    // Distributions number people's accounts from 1000; 65534 is "nobody".
    const FIRST_UID: u32 = 1000;
    const NOBODY: u32 = 65534;

    passwd
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let uid: u32 = fields.nth(1)?.parse().ok()?;
            (FIRST_UID..NOBODY).contains(&uid).then(|| name.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_users_skip_system_accounts() {
        let passwd = "root:x:0:0::/root:/bin/bash\n\
                      alice:x:1000:1000::/home/alice:/bin/zsh\n\
                      nobody:x:65534:65534::/:/usr/bin/nologin\n\
                      bob:x:1001:1001::/home/bob:/bin/bash\n";
        assert_eq!(parse_local_users(passwd), ["alice", "bob"]);
    }

    /// Prints what this machine's file servers and firewall are doing. Reads
    /// the real system, so it only runs when asked:
    /// `cargo test -p lanwell-core -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn file_servers_on_this_machine() {
        for server in FileServer::ALL {
            println!("{server:?}: {:?}", FileServerStatus::detect(server, false));
        }
    }
}
