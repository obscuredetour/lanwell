//! Mounting other machines' shares: writing their systemd units and
//! credentials, and starting them.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use netfoo_core::linux::Paths;
use netfoo_core::linux::systemd::{
    UnitNames, render_automount_unit, render_credentials, render_mount_unit,
};
use netfoo_core::mount::check_mount_point;
use netfoo_core::{
    AbsolutePath, InvalidValue, ManagedMount, MountLogin, MountWhen, Owner, Secret, SmbMount, State,
};

use crate::apply::{ApplyError, write};
use crate::files::write_atomically;
use crate::services::Services;

/// Mounts `mount` for `owner`, replacing any netfoo mount on the same folder,
/// and checks it connects.
pub fn set(
    mount: &SmbMount,
    password: Option<&Secret>,
    owner: Owner,
    paths: &Paths,
    services: &impl Services,
) -> Result<(), ApplyError> {
    check_mount_point(&mount.mount_point)?;
    let names = UnitNames::for_mount_point(&mount.mount_point);
    let credentials = paths.credentials.join(&names.stem);
    let mount_unit = paths.systemd_units.join(&names.mount);
    let automount_unit = paths.systemd_units.join(&names.automount);
    let managed = ManagedMount {
        mount: mount.clone(),
        owner,
    };

    // Whatever netfoo mounted on this folder before (an earlier version of
    // this mount) is unmounted first, so the folder can be checked.
    services.disable_now(&names.automount);
    services.disable_now(&names.mount);
    services
        .prepare_mount_point(&mount.mount_point)
        .map_err(ApplyError::MountPoint)?;

    match (&mount.login, password) {
        (MountLogin::User { .. }, Some(password)) => {
            if password.expose().chars().any(char::is_control) {
                return Err(InvalidValue("A password can't contain line breaks.".into()).into());
            }
            let text =
                render_credentials(&mount.login, password).expect("a user login has credentials");
            write_secret(&paths.credentials, &credentials, &text)?;
        }
        (MountLogin::User { .. }, None) if !credentials.exists() => {
            return Err(InvalidValue("Enter the password for this share.".into()).into());
        }
        // Keeps the password saved for this folder.
        (MountLogin::User { .. }, None) => {}
        (MountLogin::Guest, _) => remove_if_present(&credentials)?,
    }

    write(
        &mount_unit,
        &render_mount_unit(&managed, &credentials.to_string_lossy()),
    )?;
    match mount.when {
        MountWhen::FirstAccess => write(&automount_unit, &render_automount_unit(&managed))?,
        MountWhen::Startup => remove_if_present(&automount_unit)?,
    }

    let mut state = State::load(&paths.state)?;
    state
        .smb_mounts
        .retain(|other| other.mount.mount_point != mount.mount_point);
    state.smb_mounts.push(managed);
    write(&paths.state, &state.to_json())?;

    services.reload_units()?;
    let connected = match mount.when {
        // Mount it now too, so a wrong password shows up here rather than the
        // first time someone opens the folder.
        MountWhen::FirstAccess => services
            .enable_now(&names.automount)
            .and_then(|()| services.start(&names.mount)),
        MountWhen::Startup => services.enable_now(&names.mount),
    };
    connected.map_err(|e| ApplyError::Connect(e.to_string()))
}

/// Unmounts the share on `mount_point` and removes its units and
/// credentials. The folder itself stays.
pub fn remove(
    mount_point: &AbsolutePath,
    paths: &Paths,
    services: &impl Services,
) -> Result<(), ApplyError> {
    let names = UnitNames::for_mount_point(mount_point);
    services.disable_now(&names.automount);
    services.disable_now(&names.mount);

    for path in [
        paths.systemd_units.join(&names.automount),
        paths.systemd_units.join(&names.mount),
        paths.credentials.join(&names.stem),
    ] {
        remove_if_present(&path)?;
    }

    let mut state = State::load(&paths.state)?;
    state
        .smb_mounts
        .retain(|other| &other.mount.mount_point != mount_point);
    write(&paths.state, &state.to_json())?;

    services.reload_units()?;
    Ok(())
}

/// Writes a file only root can read, in a folder only root can open.
fn write_secret(dir: &Path, path: &Path, contents: &str) -> Result<(), ApplyError> {
    let error = |source: io::Error| ApplyError::Write {
        path: path.display().to_string(),
        source,
    };
    fs::create_dir_all(dir).map_err(error)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(error)?;
    write_atomically(path, contents.as_bytes(), 0o600).map_err(error)
}

fn remove_if_present(path: &Path) -> Result<(), ApplyError> {
    match fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(ApplyError::Write {
            path: path.display().to_string(),
            source: e,
        }),
        _ => Ok(()),
    }
}
