//! Writing files safely.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Replaces `path` with `contents` so that anything reading it sees either the
/// old file or the new one, never half of each: the new file is written
/// beside it, then renamed over it. Creates the directory if it's missing.
///
/// `mode` is the file's permissions: 0o644 (anyone may read it, root may
/// write it) for config, like the rest of /etc; 0o755 for programs; 0o600
/// for secrets.
pub fn write_atomically(path: &Path, contents: &[u8], mode: u32) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("a file to write needs a directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("a file to write needs a name"))?;
    fs::create_dir_all(dir)?;

    let temporary = dir.join(format!(".{}.lanwell-new", name.to_string_lossy()));
    let mut file = fs::File::create(&temporary)?;
    // Before writing, so a secret is never readable by others, even briefly.
    file.set_permissions(fs::Permissions::from_mode(mode))?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&temporary, path)
}

/// The first part of `path` that's a symbolic link, if any, checking only
/// the parts that exist. Root follows links, so a folder that looks like
/// `~/Network/media` could otherwise lead anywhere.
pub fn find_symlink(path: &Path) -> io::Result<Option<PathBuf>> {
    let mut so_far = PathBuf::new();
    for part in path.components() {
        so_far.push(part);
        match fs::symlink_metadata(&so_far) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(Some(so_far)),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_symlinks_on_the_way() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink("/etc", dir.path().join("link")).unwrap();

        assert_eq!(find_symlink(&real.join("missing/folder")).unwrap(), None);
        assert_eq!(
            find_symlink(&dir.path().join("link/missing")).unwrap(),
            Some(dir.path().join("link"))
        );
    }

    #[test]
    fn writes_and_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("etc/samba/lanwell.conf");

        write_atomically(&path, b"first", 0o644).unwrap();
        write_atomically(&path, b"second", 0o644).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        // No temporary file left behind.
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }
}
