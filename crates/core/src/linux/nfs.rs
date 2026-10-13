//! NFS exports, in the format of exports(5).

use crate::linux::GENERATED_HEADER;
use crate::share::NfsExport;

/// The contents of lanwell's exports file (`/etc/exports.d/lanwell.exports`):
/// one line per export.
///
/// `drive_of` says which drive an export is on: its mount point, or `None` if
/// it's on the system's own (`/`). An export on another drive is only made
/// while that drive is mounted (`mp=`); otherwise NFS would serve the empty
/// folder it's mounted on.
pub fn render_exports(
    exports: &[NfsExport],
    drive_of: impl Fn(&NfsExport) -> Option<String>,
) -> String {
    let mut out = String::from(GENERATED_HEADER);
    for export in exports {
        out.push_str(&render_export(export, drive_of(export)));
        out.push('\n');
    }
    out
}

fn render_export(export: &NfsExport, drive: Option<String>) -> String {
    // sync: a write is on disk before the client is told it is (the safe default).
    // no_subtree_check: the recommended setting since nfs-utils 1.1; checking
    // breaks when files are renamed while open.
    let mut options = String::from(if export.read_only {
        "ro,sync,no_subtree_check"
    } else {
        "rw,sync,no_subtree_check"
    });
    if let Some(drive) = drive {
        options.push_str(&format!(",mp={}", escape(&drive)));
    }
    let clients: Vec<String> = export
        .clients
        .iter()
        .map(|client| format!("{client}({options})"))
        .collect();
    format!("{} {}", escape(export.path.as_str()), clients.join(" "))
}

/// exports(5) splits on whitespace and treats `#` and `"` specially; any of
/// those in a path are written as a backslash and three octal digits.
fn escape(path: &str) -> String {
    path.chars()
        .map(|c| match c {
            ' ' | '\t' | '\\' | '#' | '"' => format!("\\{:03o}", c as u32),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbsolutePath, NfsClient};

    #[test]
    fn renders_exports() {
        let export = NfsExport {
            path: AbsolutePath::new("/srv/My Media").unwrap(),
            read_only: true,
            clients: vec![
                NfsClient::new("192.168.1.0/24").unwrap(),
                NfsClient::new("laptop.lan").unwrap(),
            ],
        };
        assert_eq!(
            render_exports(&[export], |_| None),
            format!(
                "{GENERATED_HEADER}/srv/My\\040Media 192.168.1.0/24(ro,sync,no_subtree_check) laptop.lan(ro,sync,no_subtree_check)\n"
            )
        );
    }

    #[test]
    fn exports_on_other_drives_wait_for_them() {
        let export = NfsExport {
            path: AbsolutePath::new("/run/media/alice/My Drive/films").unwrap(),
            read_only: true,
            clients: vec![NfsClient::new("*").unwrap()],
        };
        let text = render_exports(&[export], |_| Some("/run/media/alice/My Drive".into()));
        assert!(text.ends_with(
            "/run/media/alice/My\\040Drive/films *(ro,sync,no_subtree_check,mp=/run/media/alice/My\\040Drive)\n"
        ));
    }
}
