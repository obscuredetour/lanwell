//! NFS exports, in the format of exports(5).

use crate::linux::GENERATED_HEADER;
use crate::names::AbsolutePath;
use crate::share::NfsExport;

/// The contents of netfoo's exports file (`/etc/exports.d/netfoo.exports`):
/// one line per export.
pub fn render_exports(exports: &[NfsExport]) -> String {
    let mut out = String::from(GENERATED_HEADER);
    for export in exports {
        out.push_str(&render_export(export));
        out.push('\n');
    }
    out
}

fn render_export(export: &NfsExport) -> String {
    // sync: a write is on disk before the client is told it is (the safe default).
    // no_subtree_check: the recommended setting since nfs-utils 1.1; checking
    // breaks when files are renamed while open.
    let options = if export.read_only {
        "ro,sync,no_subtree_check"
    } else {
        "rw,sync,no_subtree_check"
    };
    let clients: Vec<String> = export
        .clients
        .iter()
        .map(|client| format!("{client}({options})"))
        .collect();
    format!("{} {}", escape_path(&export.path), clients.join(" "))
}

/// exports(5) splits on whitespace and treats `#` and `"` specially; any of
/// those in a path are written as a backslash and three octal digits.
fn escape_path(path: &AbsolutePath) -> String {
    path.as_str()
        .chars()
        .map(|c| match c {
            ' ' | '\t' | '\\' | '#' | '"' => format!("\\{:03o}", c as u32),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NfsClient;

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
            render_exports(&[export]),
            format!(
                "{GENERATED_HEADER}/srv/My\\040Media 192.168.1.0/24(ro,sync,no_subtree_check) laptop.lan(ro,sync,no_subtree_check)\n"
            )
        );
    }
}
