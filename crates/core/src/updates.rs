//! Whether a newer lanwell is out. Each release has a `latest.json`
//! (`{"version": "0.2.0"}`, written by scripts/package.sh); the app fetches the
//! latest release's and this decides what it means.
//!
//! Only copies install.sh put in place are told about updates: install.sh is
//! how they update. Development builds, and any copy a package manager
//! installed, are left alone.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The latest release's `latest.json`. GitHub redirects this to the newest
/// release that isn't a draft or a prerelease.
pub const LATEST_URL: &str =
    "https://github.com/obscuredetour/lanwell/releases/latest/download/latest.json";

/// The latest release's page, with its notes.
pub const RELEASE_NOTES_URL: &str = "https://github.com/obscuredetour/lanwell/releases/latest";

/// Where install.sh puts the app and its helper, under the user's home.
pub const INSTALL_DIR: &str = ".local/lib/lanwell";

/// What the app knows about updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// Not installed by install.sh, so not lanwell's to update.
    Unmanaged,
    /// The user turned checking off.
    Off,
    Current,
    Available {
        version: String,
    },
    /// Checking is on, but the latest release couldn't be read (`message` says why).
    Unreachable {
        message: String,
    },
}

#[derive(Deserialize)]
struct Latest {
    version: String,
}

impl UpdateStatus {
    /// What a release's `latest.json` says, for a copy at `current`.
    pub fn from_latest(latest_json: &str, current: &str) -> Result<Self, String> {
        let latest: Latest = serde_json::from_str(latest_json)
            .map_err(|e| format!("The latest release's latest.json isn't readable: {e}"))?;
        Ok(if is_newer(&latest.version, current) {
            Self::Available {
                version: latest.version,
            }
        } else {
            Self::Current
        })
    }
}

/// Whether `exe` is the app as install.sh installs it, in `home`.
pub fn installed_by_script(exe: &Path, home: &Path) -> bool {
    exe == home.join(INSTALL_DIR).join("lanwell")
}

/// Whether version `a` comes after `b`. Both are `major.minor.patch`; one
/// that isn't is never newer, so a malformed release offers nothing.
fn is_newer(a: &str, b: &str) -> bool {
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

fn parse(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.split('.').map(|part| part.parse().ok());
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_compare_as_numbers() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
    }

    #[test]
    fn malformed_versions_are_never_newer() {
        for version in ["", "1", "1.2", "1.2.3.4", "v1.2.3", "1.2.3-beta", "a.b.c"] {
            assert!(!is_newer(version, "0.1.0"), "{version}");
        }
    }

    #[test]
    fn latest_json_says_whether_theres_an_update() {
        assert_eq!(
            UpdateStatus::from_latest(r#"{"version": "0.2.0"}"#, "0.1.0"),
            Ok(UpdateStatus::Available {
                version: "0.2.0".into()
            })
        );
        assert_eq!(
            UpdateStatus::from_latest(r#"{"version": "0.1.0"}"#, "0.1.0"),
            Ok(UpdateStatus::Current)
        );
        // Tauri's updater writes more than the version; the rest is ignored.
        assert_eq!(
            UpdateStatus::from_latest(r#"{"version": "0.1.0", "platforms": {}}"#, "0.2.0"),
            Ok(UpdateStatus::Current)
        );
        assert!(UpdateStatus::from_latest("<html>Not Found</html>", "0.1.0").is_err());
    }

    #[test]
    fn only_install_sh_copies_are_managed() {
        let home = Path::new("/home/alice");
        assert!(installed_by_script(
            Path::new("/home/alice/.local/lib/lanwell/lanwell"),
            home
        ));
        for exe in [
            "/home/alice/repos/lanwell/target/debug/lanwell",
            "/usr/bin/lanwell",
            "/home/bob/.local/lib/lanwell/lanwell",
        ] {
            assert!(!installed_by_script(Path::new(exe), home), "{exe}");
        }
    }
}
