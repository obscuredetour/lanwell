//! What the helper does to the running system, after changing config files:
//! reloading services, starting units, preparing mount points.
//!
//! [`Services`] is a trait so `apply`'s tests can check what would be done
//! without doing anything.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use netfoo_core::linux::explain_mount_failure;
use netfoo_core::system::find_program;
use netfoo_core::{AbsolutePath, Secret, UserName};

use crate::files::find_symlink;
use crate::users;

pub trait Services {
    /// Makes Samba pick up changed shares, if it's running.
    fn reload_samba(&self) -> Result<(), CommandError>;
    /// Makes the NFS server pick up changed exports.
    fn reload_nfs_exports(&self) -> Result<(), CommandError>;
    /// Sets `user`'s Samba password, adding them to Samba if needed.
    fn set_smb_password(&self, user: &UserName, password: &Secret) -> Result<(), CommandError>;
    /// Makes systemd read changed unit files.
    fn reload_units(&self) -> Result<(), CommandError>;
    /// Enables `unit` and starts it now. If it's a mount that fails, says why.
    fn enable_now(&self, unit: &str) -> Result<(), CommandError>;
    /// Stops and disables `unit`, if there is one.
    fn disable_now(&self, unit: &str);
    /// Starts `unit` now. If it's a mount that fails, says why.
    fn start(&self, unit: &str) -> Result<(), CommandError>;
    /// Whether `unit` is set up or active: a unit file anywhere, or a mount
    /// that's mounted now. Used to leave alone mounts netfoo didn't make.
    fn unit_in_use(&self, unit: &str) -> bool;
    /// Makes sure a share can be mounted on `path`: refuses it if any part of
    /// it is a symbolic link, or if it has files in it (mounting would hide
    /// them), and creates it if it's missing.
    fn prepare_mount_point(&self, path: &AbsolutePath) -> Result<(), String>;
    /// Whether `user` is a person's account, not root or a system account.
    fn is_person(&self, user: &UserName) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{program}: {message}")]
    Failed { program: String, message: String },
    /// A failure netfoo can put in plain words.
    #[error("{0}")]
    Explained(String),
}

/// The real thing: runs the system's own tools.
pub struct SystemServices;

/// Samba's service is `smb` on Arch and Fedora and `smbd` on Debian and Ubuntu.
const SAMBA_UNITS: [&str; 2] = ["smb.service", "smbd.service"];

impl Services for SystemServices {
    fn reload_samba(&self) -> Result<(), CommandError> {
        for unit in SAMBA_UNITS {
            if run(
                "systemctl",
                &["show", "--property=LoadState", "--value", unit],
                None,
            )?
            .trim()
                == "loaded"
            {
                // "try": does nothing if Samba isn't running; it reads the new
                // config when it's next started.
                run("systemctl", &["try-reload-or-restart", unit], None)?;
            }
        }
        Ok(())
    }

    fn reload_nfs_exports(&self) -> Result<(), CommandError> {
        run("exportfs", &["-ra"], None).map(drop)
    }

    fn set_smb_password(&self, user: &UserName, password: &Secret) -> Result<(), CommandError> {
        // -s reads the password (twice, as confirmation) from stdin rather than a terminal.
        let input = format!("{0}\n{0}\n", password.expose());
        run("smbpasswd", &["-s", "-a", user.as_str()], Some(&input)).map(drop)
    }

    fn reload_units(&self) -> Result<(), CommandError> {
        run("systemctl", &["daemon-reload"], None).map(drop)
    }

    // `--` before unit names: a name can't then be read as an option.
    fn enable_now(&self, unit: &str) -> Result<(), CommandError> {
        explained(unit, || {
            run("systemctl", &["enable", "--now", "--", unit], None)
        })
    }

    fn disable_now(&self, unit: &str) {
        // Fails when there's no such unit, which is fine: nothing to stop.
        let _ = run("systemctl", &["disable", "--now", "--", unit], None);
    }

    fn start(&self, unit: &str) -> Result<(), CommandError> {
        explained(unit, || run("systemctl", &["start", "--", unit], None))
    }

    fn unit_in_use(&self, unit: &str) -> bool {
        let args = ["show", "--property=FragmentPath,ActiveState", "--", unit];
        let Ok(output) = run("systemctl", &args, None) else {
            return false;
        };
        output.lines().any(|line| {
            line == "ActiveState=active"
                || line
                    .strip_prefix("FragmentPath=")
                    .is_some_and(|path| !path.is_empty())
        })
    }

    fn prepare_mount_point(&self, path: &AbsolutePath) -> Result<(), String> {
        let folder = Path::new(path.as_str());
        match find_symlink(folder) {
            Ok(None) => {}
            Ok(Some(link)) => {
                return Err(format!(
                    "{} is a symbolic link; choose a folder whose path has none.",
                    link.display()
                ));
            }
            Err(e) => return Err(format!("Couldn't check {path}: {e}")),
        }
        match std::fs::read_dir(folder) {
            Ok(mut entries) => match entries.next() {
                Some(_) => Err(format!(
                    "{path} already has files in it, which the share would hide. Choose an empty folder."
                )),
                None => Ok(()),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(folder).map_err(|e| format!("Couldn't create {path}: {e}"))
            }
            Err(e) => Err(format!("Couldn't open {path}: {e}")),
        }
    }

    fn is_person(&self, user: &UserName) -> bool {
        users::is_person(user.as_str())
    }
}

/// Runs `systemctl`, and if a mount fails, replaces systemd's "see the
/// journal" with what mount.cifs or mount.nfs said there, explained when possible.
fn explained(
    unit: &str,
    systemctl: impl FnOnce() -> Result<String, CommandError>,
) -> Result<(), CommandError> {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let Err(error) = systemctl() else {
        return Ok(());
    };
    if !unit.ends_with(".mount") {
        return Err(error);
    }
    // Only what was logged since this attempt began.
    let since = format!("--since=@{started}");
    let log = run(
        "journalctl",
        &[
            "--output=cat",
            "--no-pager",
            &since,
            &format!("--unit={unit}"),
        ],
        None,
    )
    .unwrap_or_default();
    Err(explain_mount_failure(&log).map_or(error, CommandError::Explained))
}

/// Runs a system program with a plain environment and returns its output.
pub fn run(program: &str, args: &[&str], stdin: Option<&str>) -> Result<String, CommandError> {
    let error = |message: String| CommandError::Failed {
        program: program.to_string(),
        message,
    };

    let path = find_program(program).ok_or_else(|| error("isn't installed".into()))?;
    let mut child = Command::new(path)
        .args(args)
        // Only what these tools need, and messages in a language we can show as-is.
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| error(e.to_string()))?;

    if let Some(input) = stdin {
        // Dropped at the end of this block, which closes stdin.
        let mut pipe = child.stdin.take().expect("stdin was piped");
        pipe.write_all(input.as_bytes())
            .map_err(|e| error(e.to_string()))?;
    }

    let output = child.wait_with_output().map_err(|e| error(e.to_string()))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(error(if stderr.is_empty() {
            output.status.to_string()
        } else {
            stderr
        }))
    }
}
