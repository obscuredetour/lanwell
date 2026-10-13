//! Finding file servers on the local network, from what they announce over
//! mDNS (Bonjour). Macs, Linux machines running Avahi with Samba, and most NAS
//! boxes announce their SMB shares this way. Windows PCs don't (they use
//! WS-Discovery), and few servers announce NFS.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use lanwell_core::{FoundServer, Protocol};
use mdns_sd::{ServiceDaemon, ServiceEvent};

/// The service types servers announce, and what each means.
const SERVICES: [(&str, Protocol); 2] = [
    ("_smb._tcp.local.", Protocol::Smb),
    ("_nfs._tcp.local.", Protocol::Nfs),
];

/// The servers that answer within `wait`, by name.
pub fn find_servers(wait: Duration) -> Result<Vec<FoundServer>, String> {
    let daemon = ServiceDaemon::new().map_err(|e| format!("Couldn't look for servers: {e}"))?;
    let browsers = SERVICES
        .iter()
        .map(|(service, protocol)| daemon.browse(service).map(|receiver| (receiver, *protocol)))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Couldn't look for servers: {e}"))?;

    // By host, so a server announcing both protocols is listed once.
    let mut found: BTreeMap<String, FoundServer> = BTreeMap::new();
    let deadline = Instant::now() + wait;
    while Instant::now() < deadline {
        for (receiver, protocol) in &browsers {
            while let Ok(event) = receiver.try_recv() {
                if let ServiceEvent::ServiceResolved(service) = event {
                    let host = service.get_hostname().trim_end_matches('.').to_string();
                    let name = instance_name(service.get_fullname());
                    let server = found.entry(host.clone()).or_insert_with(|| FoundServer {
                        name,
                        host,
                        protocols: Vec::new(),
                    });
                    if !server.protocols.contains(protocol) {
                        server.protocols.push(*protocol);
                        server.protocols.sort();
                    }
                }
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = daemon.shutdown();

    let mut servers: Vec<FoundServer> = found.into_values().collect();
    servers.sort_by_key(|server| server.name.to_lowercase());
    Ok(servers)
}

/// `Prometheus._smb._tcp.local.` → `Prometheus`. Names may contain dots, so
/// it's everything before the service type.
fn instance_name(fullname: &str) -> String {
    let name = SERVICES
        .iter()
        .find_map(|(service, _)| fullname.strip_suffix(service))
        .unwrap_or(fullname);
    name.trim_end_matches('.').replace("\\032", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names() {
        assert_eq!(instance_name("Prometheus._smb._tcp.local."), "Prometheus");
        assert_eq!(instance_name("nas.home._nfs._tcp.local."), "nas.home");
    }

    /// Lists what's on the network this runs on. Needs a real network, so it
    /// only runs when asked: `cargo test -p lanwell -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn finds_servers_on_this_network() {
        let servers = find_servers(Duration::from_secs(3)).unwrap();
        println!("{servers:#?}");
    }
}
