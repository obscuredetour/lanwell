//! Samba's config: the share sections lanwell generates, and the line that
//! includes them from smb.conf.

use crate::linux::GENERATED_HEADER;
use crate::share::{SmbAccess, SmbShare};

/// The contents of lanwell's Samba file (`/etc/samba/lanwell.conf`): one section per share.
///
/// It holds only share sections, never `[global]` settings, so it can be
/// included from anywhere in smb.conf (see [`with_include`]).
pub fn render_shares(shares: &[SmbShare]) -> String {
    let mut out = String::from(GENERATED_HEADER);
    for share in shares {
        out.push('\n');
        out.push_str(&render_share(share));
    }
    out
}

fn render_share(share: &SmbShare) -> String {
    let yes_no = |value: bool| if value { "yes" } else { "no" };
    let mut lines = vec![
        format!("[{}]", share.name),
        format!("   path = {}", share.path),
        format!("   read only = {}", yes_no(share.read_only)),
    ];
    match &share.access {
        SmbAccess::Guest => {
            // Also needs `map to guest = bad user` in [global], which the
            // smb.conf lanwell creates has (see [`new_smb_conf`]).
            lines.push("   guest ok = yes".into());
        }
        SmbAccess::Users { users } => {
            let users: Vec<&str> = users.iter().map(|user| user.as_str()).collect();
            lines.push("   guest ok = no".into());
            lines.push(format!("   valid users = {}", users.join(" ")));
        }
    }
    lines.join("\n") + "\n"
}

/// smb.conf with a line including lanwell's file added at the end, or `None`
/// if it's already there.
///
/// `include` reads the file in place, so at the end of smb.conf its sections
/// simply follow the others'.
pub fn with_include(smb_conf: &str, include_path: &str) -> Option<String> {
    let line = include_line(include_path);
    if smb_conf.lines().any(|existing| existing.trim() == line) {
        return None;
    }
    let mut out = smb_conf.to_string();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n# Shares managed by lanwell\n");
    out.push_str(&line);
    out.push('\n');
    Some(out)
}

fn include_line(include_path: &str) -> String {
    format!("include = {include_path}")
}

/// smb.conf for a system that doesn't have one yet (Arch, for example, ships
/// without it). It's the user's file from then on: lanwell only ever adds the
/// include line.
pub fn new_smb_conf(include_path: &str) -> String {
    format!(
        "# Created by lanwell, because there was no smb.conf. It's yours to edit:\n\
         # lanwell only needs the include line at the end.\n\
         \n\
         [global]\n   \
            server role = standalone server\n   \
            # Lets shares marked as open to guests work without a password.\n   \
            map to guest = bad user\n\
         \n\
         # Shares managed by lanwell\n\
         {}\n",
        include_line(include_path)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbsolutePath, ShareName, UserName};

    fn share(name: &str, access: SmbAccess) -> SmbShare {
        SmbShare {
            name: ShareName::new(name).unwrap(),
            path: AbsolutePath::new(format!("/srv/{}", name.to_lowercase())).unwrap(),
            read_only: false,
            access,
        }
    }

    #[test]
    fn renders_shares() {
        let users = vec![
            UserName::new("alice").unwrap(),
            UserName::new("bob").unwrap(),
        ];
        let text = render_shares(&[
            share("Media", SmbAccess::Guest),
            share("Private", SmbAccess::Users { users }),
        ]);
        let expected = format!(
            "{GENERATED_HEADER}\n\
             [Media]\n   path = /srv/media\n   read only = no\n   guest ok = yes\n\
             \n\
             [Private]\n   path = /srv/private\n   read only = no\n   guest ok = no\n   valid users = alice bob\n"
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn adds_the_include_once() {
        let original = "[global]\n   workgroup = HOME";
        let included = with_include(original, "/etc/samba/lanwell.conf").unwrap();
        assert!(included.starts_with(original));
        assert!(included.ends_with("include = /etc/samba/lanwell.conf\n"));
        assert_eq!(with_include(&included, "/etc/samba/lanwell.conf"), None);
    }

    #[test]
    fn a_new_smb_conf_already_includes() {
        assert_eq!(
            with_include(
                &new_smb_conf("/etc/samba/lanwell.conf"),
                "/etc/samba/lanwell.conf"
            ),
            None
        );
    }
}
