//! The system programs the helper runs after changing config files.
//!
//! [`Services`] is a trait so `apply`'s tests can check what would be run
//! without running anything.

use std::io::Write;
use std::process::{Command, Stdio};

use netfoo_core::system::find_program;
use netfoo_core::{Secret, UserName};

pub trait Services {
    /// Makes Samba pick up changed shares, if it's running.
    fn reload_samba(&self) -> Result<(), CommandError>;
    /// Makes the NFS server pick up changed exports.
    fn reload_nfs_exports(&self) -> Result<(), CommandError>;
    /// Sets `user`'s Samba password, adding them to Samba if needed.
    fn set_smb_password(&self, user: &UserName, password: &Secret) -> Result<(), CommandError>;
}

#[derive(Debug, thiserror::Error)]
#[error("{program}: {message}")]
pub struct CommandError {
    program: String,
    message: String,
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
}

/// Runs a system program with a plain environment and returns its output.
fn run(program: &str, args: &[&str], stdin: Option<&str>) -> Result<String, CommandError> {
    let error = |message: String| CommandError {
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
