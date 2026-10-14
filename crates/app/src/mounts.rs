//! Mounts, from the app's side: what they're doing now, which shares a server
//! has, and opening one. All of it runs as the user; changing mounts goes
//! through the helper.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use lanwell_core::linux::cifs::{self, parse_share_list};
use lanwell_core::linux::nfs_client::{self, parse_export_list};
use lanwell_core::linux::systemd::UnitNames;
use lanwell_core::mount::check_host;
use lanwell_core::system::find_program;
use lanwell_core::{ManagedMount, MountActivity, MountStatus};

/// What each mount is doing now, from systemd (no root needed).
pub fn statuses(mounts: &[ManagedMount]) -> Vec<MountStatus> {
    mounts
        .iter()
        .map(|managed| {
            let mount_point = managed.mount.mount_point.clone();
            let names = UnitNames::for_mount_point(&mount_point);
            let output = Command::new("systemctl")
                .args([
                    "show",
                    "--property=ActiveState",
                    "--value",
                    "--",
                    &names.mount,
                    &names.automount,
                ])
                .output();
            // One line per unit, in the order asked.
            let text = output
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default();
            let mut states = text.lines();
            let activity = MountActivity::from_unit_states(
                states.next().unwrap_or_default(),
                states.next().unwrap_or_default(),
            );
            MountStatus {
                mount_point,
                activity,
            }
        })
        .collect()
}

/// How long to wait for a server to list its shares.
const LIST_TIMEOUT: Duration = Duration::from_secs(15);

/// The shares on `host`, signing in as `username` (or as a guest without one).
pub fn list_shares(
    host: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<String>, String> {
    check_host(host).map_err(|e| e.to_string())?;
    let smbclient = find_program("smbclient")
        .ok_or("Listing a server's shares needs smbclient (part of Samba's client tools).")?;

    let mut command = Command::new(smbclient);
    command.args(["--grepable", "--list", &format!("//{host}")]);
    match username {
        Some(username) => {
            command.args(["--user", username]);
            // smbclient reads the password from $PASSWD, which (unlike its
            // arguments) other users can't see in the process list.
            command.env("PASSWD", password.unwrap_or_default());
        }
        None => {
            command.arg("--no-pass");
        }
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (succeeded, output) = run_with_timeout(command, LIST_TIMEOUT)?;
    if succeeded {
        Ok(parse_share_list(&output))
    } else {
        Err(cifs::explain_list_failure(&output))
    }
}

/// The folders an NFS server exports. Asks its mount service (NFS 3's), so a
/// server that only speaks NFS 4 can't answer; the error says to type the path.
pub fn list_exports(host: &str) -> Result<Vec<String>, String> {
    check_host(host).map_err(|e| e.to_string())?;
    let showmount = find_program("showmount")
        .ok_or("Listing a server's NFS folders needs showmount (part of nfs-utils).")?;

    let mut command = Command::new(showmount);
    command.args(["--exports", host]);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (succeeded, output) = run_with_timeout(command, LIST_TIMEOUT)?;
    if succeeded {
        Ok(parse_export_list(&output))
    } else {
        Err(nfs_client::explain_list_failure(&output))
    }
}

/// Runs `command`, giving up after `timeout`. Returns whether it succeeded,
/// and its output (stdout then stderr).
fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<(bool, String), String> {
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Couldn't reach the server: it didn't answer in time.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    let mut output = String::new();
    // The listing is small, so it fits in the pipes' buffers until it's read here.
    if let Some(mut stdout) = child.stdout.take() {
        let _ = stdout.read_to_string(&mut output);
    }
    if let Some(mut stderr) = child.stderr.take() {
        let _ = stderr.read_to_string(&mut output);
    }
    Ok((status.success(), output))
}
