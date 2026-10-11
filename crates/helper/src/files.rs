//! Writing config files safely.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

/// Replaces `path` with `contents` so that anything reading it sees either the
/// old file or the new one, never half of each: the new file is written
/// beside it, then renamed over it. Creates the directory if it's missing.
///
/// The file is readable by everyone and writable by root (0644), like the
/// rest of /etc.
pub fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a file to write needs a directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("a file to write needs a name"))?;
    fs::create_dir_all(dir)?;

    let temporary = dir.join(format!(".{}.netfoo-new", name.to_string_lossy()));
    let mut file = fs::File::create(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.set_permissions(fs::Permissions::from_mode(0o644))?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("etc/samba/netfoo.conf");

        write_atomically(&path, "first").unwrap();
        write_atomically(&path, "second").unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        // No temporary file left behind.
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}
