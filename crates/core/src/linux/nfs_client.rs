//! Making sense of the NFS client tools' output: why a mount failed, and
//! which folders a server exports.

/// A plain explanation of why mounting failed, from what mount.nfs logged
/// (the unit's journal). `None` if there's nothing lanwell recognises.
pub fn explain_mount_failure(log: &str) -> Option<String> {
    const EXPLANATIONS: [(&str, &str); 9] = [
        (
            "access denied by server",
            "The server doesn't let this computer mount that folder. Add this computer to its export list.",
        ),
        (
            "No such file or directory",
            "The server doesn't export a folder by that name.",
        ),
        (
            "Failed to resolve server",
            "Couldn't find a server by that name on the network.",
        ),
        (
            "Connection refused",
            "The server isn't accepting NFS connections. Is its NFS server running?",
        ),
        (
            "Connection timed out",
            "Couldn't reach the server: it didn't answer in time.",
        ),
        (
            "No route to host",
            "Couldn't reach the server: there's no route to it from here.",
        ),
        (
            "requested NFS version or transport protocol is not supported",
            "The server doesn't speak the NFS version that was chosen. Try Automatic, or version 3.",
        ),
        (
            "Protocol not supported",
            "The server doesn't speak the NFS version that was chosen. Try Automatic, or version 3.",
        ),
        (
            "bad option",
            "This system can't mount NFS: install nfs-utils (nfs-common on Debian and Ubuntu).",
        ),
    ];

    EXPLANATIONS
        .iter()
        .find(|(needle, _)| log.contains(needle))
        .map(|(_, explanation)| explanation.to_string())
        // Something mount.nfs said, even if lanwell can't explain it.
        .or_else(|| {
            log.lines()
                .rev()
                .find(|line| line.contains("mount.nfs"))
                .map(|line| line.trim().to_string())
        })
}

/// A plain explanation of why `showmount -e` failed.
pub fn explain_list_failure(output: &str) -> String {
    const EXPLANATIONS: [(&str, &str); 4] = [
        (
            "Unknown host",
            "Couldn't find a server by that name on the network.",
        ),
        (
            "Program not registered",
            "The server isn't running an NFS server, or only speaks NFS 4, which can't list its folders. Type the folder's path instead.",
        ),
        (
            "Timed out",
            "Couldn't reach an NFS server there. If it only speaks NFS 4, it can't list its folders; type the path instead.",
        ),
        (
            "Unable to receive",
            "Couldn't reach an NFS server there. If it only speaks NFS 4, it can't list its folders; type the path instead.",
        ),
    ];
    EXPLANATIONS
        .iter()
        .find(|(needle, _)| output.contains(needle))
        .map(|(_, explanation)| explanation.to_string())
        .unwrap_or_else(|| {
            let last = output.lines().rev().find(|line| !line.trim().is_empty());
            last.unwrap_or("Couldn't list the server's folders.")
                .trim()
                .to_string()
        })
}

/// The folders a server exports, from `showmount --exports` output:
///
/// ```text
/// Export list for nas.lan:
/// /srv/media     192.168.1.0/24
/// /srv/backup    (everyone)
/// ```
///
/// Each line is a path, then who may mount it (one word, no spaces), so the
/// path is everything before the last word.
pub fn parse_export_list(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|line| !line.starts_with("Export list for"))
        .skip(1)
        .filter_map(|line| line.trim_end().rsplit_once(char::is_whitespace))
        .map(|(path, _clients)| path.trim_end().to_string())
        .filter(|path| path.starts_with('/'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_common_failures() {
        let log = "Mounting home-alice-Network-media.mount...\nmount.nfs: access denied by server while mounting nas.lan:/srv/media";
        assert!(
            explain_mount_failure(log)
                .unwrap()
                .starts_with("The server doesn't let this computer")
        );
        assert_eq!(
            explain_mount_failure("mount.nfs: something new").unwrap(),
            "mount.nfs: something new"
        );
        assert_eq!(explain_mount_failure("nothing useful"), None);
    }

    #[test]
    fn lists_exports() {
        let output = "Export list for nas.lan:\n/srv/media      192.168.1.0/24\n/srv/My Films   (everyone)\n";
        assert_eq!(parse_export_list(output), ["/srv/media", "/srv/My Films"]);
        assert!(parse_export_list("Export list for nas.lan:\n").is_empty());
    }

    #[test]
    fn explains_list_failures() {
        assert!(
            explain_list_failure("clnt_create: RPC: Unknown host").starts_with("Couldn't find")
        );
    }
}
