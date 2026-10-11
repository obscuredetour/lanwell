//! Which mounted filesystem a path is on, from the kernel's mount table
//! (/proc/self/mountinfo).

/// The mount point of the filesystem `path` is on: the longest mount point
/// that is `path` or contains it. `/` if nothing else does.
///
/// Read from the mount table rather than by comparing device numbers, which
/// btrfs subvolumes would fool (each has its own, mounted or not).
pub fn mount_point_of(mountinfo: &str, path: &str) -> String {
    let contains = |mount_point: &str| {
        mount_point == "/" || path == mount_point || path.starts_with(&format!("{mount_point}/"))
    };
    mountinfo
        .lines()
        // Fields: id, parent, major:minor, root, mount point, ...
        .filter_map(|line| line.split(' ').nth(4))
        .map(unescape)
        .filter(|mount_point| contains(mount_point))
        .max_by_key(String::len)
        .unwrap_or_else(|| "/".to_string())
}

/// mountinfo writes spaces, tabs, newlines and backslashes as `\` and three
/// octal digits (`\040` for a space).
fn unescape(field: &str) -> String {
    let mut out = String::new();
    let mut rest = field;
    while let Some(slash) = rest.find('\\') {
        out.push_str(&rest[..slash]);
        let code = rest.get(slash + 1..slash + 4);
        match code.and_then(|code| u8::from_str_radix(code, 8).ok()) {
            Some(byte) => {
                out.push(byte as char);
                rest = &rest[slash + 4..];
            }
            None => {
                out.push('\\');
                rest = &rest[slash + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTINFO: &str = "\
40 1 0:33 /@ / rw,noatime shared:1 - btrfs /dev/sdb2 rw,subvol=/@
50 40 0:34 /@home /home rw,noatime shared:2 - btrfs /dev/sdb2 rw,subvol=/@home
51 40 0:28 / /run rw,nosuid,nodev shared:9 - tmpfs tmpfs rw
62 51 259:0 / /run/media/main rw,nosuid,nodev,relatime shared:158 - ext4 /dev/nvme0n1 rw
63 51 259:1 / /run/media/alice/My\\040Drive rw shared:159 - ext4 /dev/sdc1 rw
";

    #[test]
    fn finds_the_containing_mount() {
        assert_eq!(
            mount_point_of(MOUNTINFO, "/run/media/main/films"),
            "/run/media/main"
        );
        assert_eq!(
            mount_point_of(MOUNTINFO, "/run/media/main"),
            "/run/media/main"
        );
        assert_eq!(mount_point_of(MOUNTINFO, "/home/alice/music"), "/home");
        assert_eq!(mount_point_of(MOUNTINFO, "/srv/media"), "/");
        // Not /run/media/main: only whole names count.
        assert_eq!(mount_point_of(MOUNTINFO, "/run/media/mainly"), "/run");
    }

    #[test]
    fn unescapes_spaces() {
        assert_eq!(
            mount_point_of(MOUNTINFO, "/run/media/alice/My Drive/films"),
            "/run/media/alice/My Drive"
        );
    }
}
