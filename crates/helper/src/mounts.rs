//! Mounting other machines' shares: writing their systemd units and
//! credentials, and starting them.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use lanwell_core::linux::Paths;
use lanwell_core::linux::systemd::{
    UnitNames, render_automount_unit, render_credentials, render_mount_unit,
};
use lanwell_core::mount::check_mount_point;
use lanwell_core::{
    AbsolutePath, InvalidValue, ManagedMount, Mount, MountWhen, Owner, RemoteShare, Secret,
    SmbLogin, State,
};

use crate::apply::{ApplyError, write};
use crate::files::write_atomically;
use crate::services::Services;

/// What else a [`set`] request says, besides the mount itself.
pub struct Edit<'a> {
    /// An SMB user login's password; `None` keeps the one already saved.
    pub password: Option<&'a Secret>,
    /// The folder of the mount being edited, if the edit moves it elsewhere.
    pub replaces: Option<&'a AbsolutePath>,
}

/// Mounts `mount` for `owner`, replacing any lanwell mount on the same folder
/// (or on `edit.replaces`), and checks it connects. A folder with a mount
/// lanwell didn't set up (its own unit, an fstab line, anything mounted there
/// now) is refused and left alone.
pub fn set(
    mount: &Mount,
    edit: Edit<'_>,
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

    let mut state = State::load(&paths.state)?;
    let is_ours = |folder: &AbsolutePath| {
        state
            .mounts
            .iter()
            .any(|other| &other.mount.mount_point == folder)
    };
    let moved_from = edit.replaces.filter(|old| *old != &mount.mount_point);
    if let Some(old) = moved_from
        && !is_ours(old)
    {
        return Err(InvalidValue(format!("{old} isn't one of Lanwell's mounts.")).into());
    }
    let ours = is_ours(&mount.mount_point);
    if ours {
        // An earlier version of this mount: unmounted first, so the folder can be checked.
        services.disable_now(&names.automount);
        services.disable_now(&names.mount);
    } else if services.unit_in_use(&names.mount) || services.unit_in_use(&names.automount) {
        return Err(ApplyError::MountPoint(format!(
            "{} already has a mount that Lanwell didn't set up, so Lanwell leaves it alone.",
            mount.mount_point
        )));
    }
    services
        .prepare_mount_point(&mount.mount_point)
        .map_err(ApplyError::MountPoint)?;

    // The new folder is ready, so the mount can leave the old one. Its saved
    // password comes along, unless a new one was given.
    if let Some(old) = moved_from {
        let old_credentials = paths.credentials.join(UnitNames::for_mount_point(old).stem);
        if edit.password.is_none() && old_credentials.exists() {
            fs::rename(&old_credentials, &credentials).map_err(|source| ApplyError::Write {
                path: credentials.display().to_string(),
                source,
            })?;
        }
        remove(old, paths, services)?;
        state = State::load(&paths.state)?;
    }

    save_credentials(
        &mount.share,
        edit.password,
        &paths.credentials,
        &credentials,
    )?;

    write(
        &mount_unit,
        &render_mount_unit(&managed, &credentials.to_string_lossy()),
    )?;
    match mount.when {
        MountWhen::FirstAccess => write(&automount_unit, &render_automount_unit(&managed))?,
        MountWhen::Startup => remove_if_present(&automount_unit)?,
    }

    state
        .mounts
        .retain(|other| other.mount.mount_point != mount.mount_point);
    state.mounts.push(managed);
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
/// credentials. The folder itself stays. Only lanwell's own mounts can be removed.
pub fn remove(
    mount_point: &AbsolutePath,
    paths: &Paths,
    services: &impl Services,
) -> Result<(), ApplyError> {
    let mut state = State::load(&paths.state)?;
    if !state
        .mounts
        .iter()
        .any(|other| &other.mount.mount_point == mount_point)
    {
        return Err(InvalidValue(format!("{mount_point} isn't one of Lanwell's mounts.")).into());
    }
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

    state
        .mounts
        .retain(|other| &other.mount.mount_point != mount_point);
    write(&paths.state, &state.to_json())?;

    services.reload_units()?;
    Ok(())
}

/// Keeps an SMB user login's password where mount.cifs reads it. Any other
/// mount (a guest, or NFS) needs none, so one left from before is removed.
fn save_credentials(
    share: &RemoteShare,
    password: Option<&Secret>,
    dir: &Path,
    path: &Path,
) -> Result<(), ApplyError> {
    let login = match share {
        RemoteShare::Smb {
            login: login @ SmbLogin::User { .. },
            ..
        } => login,
        _ => return remove_if_present(path),
    };
    match password {
        Some(password) => {
            if password.expose().chars().any(char::is_control) {
                return Err(InvalidValue("A password can't contain line breaks.".into()).into());
            }
            let text = render_credentials(login, password).expect("a user login has credentials");
            write_secret(dir, path, &text)
        }
        None if !path.exists() => {
            Err(InvalidValue("Enter the password for this share.".into()).into())
        }
        // Keeps the password saved for this folder.
        None => Ok(()),
    }
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
