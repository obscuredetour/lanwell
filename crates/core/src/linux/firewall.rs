//! Letting a file server through the firewall: ufw's rules (as commands to
//! run, and as it saves them) and firewalld's services.
//!
//! netfoo only ever opens a server's one port, and with ufw only to the local
//! network: the private address ranges, never the whole internet.

use crate::fileserver::FileServer;

/// The local network: the private IPv4 ranges, and IPv6's unique local and
/// link-local ones.
pub const LOCAL_NETWORKS_V4: [&str; 3] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];
pub const LOCAL_NETWORKS_V6: [&str; 2] = ["fc00::/7", "fe80::/10"];

/// Marks netfoo's ufw rules, so people (and netfoo) can tell them apart.
pub const UFW_COMMENT: &str = "netfoo";

/// The ufw commands (arguments after `ufw`) that let `server` in from the
/// local network, or (`allow: false`) take those rules out again. `ipv6` is
/// whether ufw handles IPv6 (`IPV6=yes` in /etc/default/ufw); if it doesn't,
/// IPv6 rules would be refused.
pub fn ufw_commands(server: FileServer, allow: bool, ipv6: bool) -> Vec<Vec<String>> {
    let port = server.port().to_string();
    let networks = LOCAL_NETWORKS_V4
        .iter()
        .chain(if ipv6 { &LOCAL_NETWORKS_V6[..] } else { &[] });
    networks
        .map(|network| {
            let mut args: Vec<String> = Vec::new();
            if !allow {
                args.push("delete".into());
            }
            for word in [
                "allow", "from", network, "to", "any", "port", &port, "proto", "tcp",
            ] {
                args.push(word.into());
            }
            if allow {
                args.extend(["comment".into(), UFW_COMMENT.into()]);
            }
            args
        })
        .collect()
}

/// Whether ufw is switched on, from /etc/ufw/ufw.conf.
pub fn ufw_enabled(conf: &str) -> bool {
    setting(conf, "ENABLED").is_some_and(|value| value == "yes")
}

/// Whether ufw handles IPv6, from /etc/default/ufw.
pub fn ufw_ipv6(defaults: &str) -> bool {
    setting(defaults, "IPV6").is_some_and(|value| value == "yes")
}

/// A `KEY=value` line's value, without quotes. The last one wins, as in a
/// shell file.
fn setting<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .rev()
        .find_map(|line| line.trim().strip_prefix(key)?.strip_prefix('='))
        .map(|value| value.trim().trim_matches('"'))
}

/// Whether ufw's saved rules (/etc/ufw/user.rules and user6.rules) let in
/// TCP connections to `port`, by any rule. ufw saves each rule as a line like
///
/// ```text
/// ### tuple ### allow tcp 445 0.0.0.0/0 any 10.0.0.0/8 in comment=6e6574666f6f
/// ```
///
/// (action, protocol, destination ports, destination, source port, source, ...).
pub fn ufw_allows(rules: &str, port: u16) -> bool {
    rules
        .lines()
        .filter_map(|line| line.strip_prefix("### tuple ###"))
        .map(|tuple| tuple.split_whitespace().collect::<Vec<_>>())
        .any(|fields| match fields[..] {
            [action, protocol, ports, ..] => {
                action == "allow"
                    && (protocol == "tcp" || protocol == "any")
                    && includes(ports, port)
            }
            _ => false,
        })
}

/// Whether a ufw port list (`445`, `139,445`, `137:139`, `any`) includes `port`.
fn includes(ports: &str, port: u16) -> bool {
    ports == "any"
        || ports.split(',').any(|part| match part.split_once(':') {
            Some((low, high)) => matches!(
                (low.parse::<u16>(), high.parse::<u16>()),
                (Ok(low), Ok(high)) if (low..=high).contains(&port)
            ),
            None => part.parse() == Ok(port),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_open_only_the_local_network() {
        let commands = ufw_commands(FileServer::Samba, true, false);
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[0].join(" "),
            "allow from 10.0.0.0/8 to any port 445 proto tcp comment netfoo"
        );
        assert_eq!(ufw_commands(FileServer::Nfs, true, true).len(), 5);
        assert_eq!(
            ufw_commands(FileServer::Nfs, false, false)[2].join(" "),
            "delete allow from 192.168.0.0/16 to any port 2049 proto tcp"
        );
    }

    #[test]
    fn reads_settings() {
        assert!(ufw_enabled("# comment\nENABLED=yes\nLOGLEVEL=low\n"));
        assert!(!ufw_enabled("ENABLED=no\n"));
        assert!(ufw_ipv6("IPV6=\"yes\"\n"));
        assert!(!ufw_ipv6("IPV6=no\n"));
    }

    #[test]
    fn finds_allowed_ports() {
        let rules = "\
*filter
### tuple ### allow tcp 445 0.0.0.0/0 any 10.0.0.0/8 in comment=6e6574666f6f
-A ufw-user-input -p tcp --dport 445 -s 10.0.0.0/8 -j ACCEPT
### tuple ### deny tcp 2049 0.0.0.0/0 any 0.0.0.0/0 in
### tuple ### allow udp 137:139 0.0.0.0/0 any 0.0.0.0/0 in
";
        assert!(ufw_allows(rules, 445));
        assert!(!ufw_allows(rules, 2049));
        assert!(!ufw_allows(rules, 138)); // UDP only
        assert!(ufw_allows(
            "### tuple ### allow any 137:445 0.0.0.0/0 any 0.0.0.0/0 in",
            445
        ));
        assert!(ufw_allows(
            "### tuple ### allow tcp any 0.0.0.0/0 any 10.0.0.0/8 in",
            2049
        ));
        assert!(!ufw_allows("", 445));
    }
}
