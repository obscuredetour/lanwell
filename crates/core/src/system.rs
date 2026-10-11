//! What's installed on this machine. Everything here is read without root.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub enum Os {
    Linux,
    Macos,
    Other,
}

/// What netfoo can do here, for the UI to explain what's missing.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: Os,
    /// Samba's server (`smbd`) is installed.
    pub samba_installed: bool,
    /// The NFS server's tools (`exportfs`) are installed.
    pub nfs_server_installed: bool,
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
    /// netfoo can't install a helper on this system yet.
    Unsupported,
}

impl HelperStatus {
    /// Asks the installed helper its version (`netfoo-helper --version`, which
    /// needs no root) and compares it with `expected`.
    pub fn detect(expected: &str) -> Self {
        if !cfg!(target_os = "linux") {
            return Self::Unsupported;
        }
        let Ok(output) = std::process::Command::new(crate::linux::HELPER_PATH)
            .arg("--version")
            .output()
        else {
            return Self::Missing;
        };
        // "netfoo-helper 0.1.0"
        let stdout = String::from_utf8_lossy(&output.stdout);
        let installed = stdout.split_whitespace().last().unwrap_or_default();
        if installed == expected {
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
}
