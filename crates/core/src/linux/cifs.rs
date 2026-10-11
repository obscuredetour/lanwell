//! Making sense of the CIFS (SMB) client tools' output: why a mount failed,
//! and which shares a server has.

/// A plain explanation of why mounting failed, from what mount.cifs logged
/// (the unit's journal). `None` if there's nothing netfoo recognises.
pub fn explain_mount_failure(log: &str) -> Option<String> {
    // mount.cifs reports "mount error(13): Permission denied"; the number is errno.
    const EXPLANATIONS: [(&str, &str); 10] = [
        (
            "mount error(13)",
            "The server didn't accept the user name or password.",
        ),
        (
            "mount error(2)",
            "The server doesn't have a share by that name.",
        ),
        (
            "mount error(6)",
            "The server doesn't have a share by that name.",
        ),
        (
            "mount error(112)",
            "Couldn't reach the server: it seems to be off.",
        ),
        (
            "mount error(113)",
            "Couldn't reach the server: there's no route to it from here.",
        ),
        (
            "mount error(110)",
            "Couldn't reach the server: it didn't answer in time.",
        ),
        (
            "mount error(115)",
            "Couldn't reach the server: it didn't answer in time.",
        ),
        (
            "could not resolve address",
            "Couldn't find a server by that name on the network.",
        ),
        (
            "mount error(95)",
            "The server doesn't speak the SMB version that was chosen. Try Automatic, or an older version.",
        ),
        (
            "cifs filesystem not supported",
            "This system's kernel can't mount SMB shares (its cifs module is missing).",
        ),
    ];

    EXPLANATIONS
        .iter()
        .find(|(needle, _)| log.contains(needle))
        .map(|(_, explanation)| explanation.to_string())
        // Something mount.cifs said, even if netfoo can't explain it.
        .or_else(|| {
            log.lines()
                .rev()
                .find(|line| line.contains("mount error"))
                .map(|line| line.trim().to_string())
        })
}

/// A plain explanation of why `smbclient --list` failed, from its output
/// (it reports NT status codes, like `NT_STATUS_LOGON_FAILURE`).
pub fn explain_list_failure(output: &str) -> String {
    const EXPLANATIONS: [(&str, &str); 6] = [
        (
            "NT_STATUS_LOGON_FAILURE",
            "The server didn't accept the user name or password.",
        ),
        (
            "NT_STATUS_ACCESS_DENIED",
            "The server wants a user name and password to list its shares.",
        ),
        ("NT_STATUS_HOST_UNREACHABLE", "Couldn't reach the server."),
        (
            "NT_STATUS_IO_TIMEOUT",
            "Couldn't reach the server: it didn't answer in time.",
        ),
        (
            "NT_STATUS_CONNECTION_REFUSED",
            "The server isn't accepting SMB connections.",
        ),
        (
            "NT_STATUS_UNSUCCESSFUL",
            "Couldn't find a server by that name on the network.",
        ),
    ];
    EXPLANATIONS
        .iter()
        .find(|(status, _)| output.contains(status))
        .map(|(_, explanation)| explanation.to_string())
        .unwrap_or_else(|| {
            let last = output.lines().rev().find(|line| !line.trim().is_empty());
            last.unwrap_or("Couldn't list the server's shares.")
                .trim()
                .to_string()
        })
}

/// The shares a server offers, from `smbclient --grepable --list` output
/// (`Disk|media|Films and music`). Only folders, and not the hidden
/// administrative ones (`C$`, `IPC$`).
pub fn parse_share_list(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| line.strip_prefix("Disk|"))
        .filter_map(|rest| rest.split('|').next())
        .filter(|name| !name.ends_with('$'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_common_failures() {
        let log = "Mounting home-alice-Network-media.mount...\nmount error(13): Permission denied\nRefer to the mount.cifs(8) manual page";
        assert_eq!(
            explain_mount_failure(log).unwrap(),
            "The server didn't accept the user name or password."
        );
        // error(2) isn't error(22).
        assert_eq!(
            explain_mount_failure("mount error(22): Invalid argument").unwrap(),
            "mount error(22): Invalid argument"
        );
        assert_eq!(explain_mount_failure("nothing useful"), None);
    }

    #[test]
    fn explains_list_failures() {
        let output = "session setup failed: NT_STATUS_LOGON_FAILURE\n";
        assert_eq!(
            explain_list_failure(output),
            "The server didn't accept the user name or password."
        );
        assert_eq!(explain_list_failure("something new\n\n"), "something new");
    }

    #[test]
    fn lists_folder_shares() {
        let output = "Disk|media|Films and music\nDisk|backup|\nIPC|IPC$|IPC Service\nDisk|C$|Default share\nPrinter|laser|\n";
        assert_eq!(parse_share_list(output), ["media", "backup"]);
    }
}
