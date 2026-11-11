//! LAN addresses and mDNS. The token never goes in a discovery record.
//! If multicast is blocked, the person types the address from the pairing code.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use socket2::{Domain, SockAddr, Socket, Type};

use crate::tls::LanError;

pub const SERVICE_TYPE: &str = "_holdover._tcp.local.";

/// Stable listen port for pairing and for sync. A busy port falls back, and
/// the port that was actually bound is remembered by the app.
pub const DEFAULT_PORT: u16 = 43720;

pub fn local_hosts() -> Vec<String> {
    let mut hosts = Vec::new();
    if let Ok(list) = if_addrs::get_if_addrs() {
        for iface in list {
            if iface.is_loopback() {
                continue;
            }
            if let IpAddr::V4(addr) = iface.ip() {
                if addr.is_link_local() || addr.is_unspecified() || addr.is_broadcast() {
                    continue;
                }
                let text = addr.to_string();
                if !hosts.contains(&text) {
                    hosts.push(text);
                }
            }
        }
    }
    if hosts.is_empty() {
        if let Some(addr) = default_route_v4() {
            hosts.push(addr.to_string());
        }
    }
    if hosts.is_empty() {
        hosts.push("127.0.0.1".into());
    }
    sort_hosts(&mut hosts);
    hosts
}

/// RFC1918 Wi-Fi addresses first, then Tailscale (100.64/10), then the rest.
pub fn sort_hosts(hosts: &mut [String]) {
    hosts.sort_by(|left, right| {
        host_rank(left)
            .cmp(&host_rank(right))
            .then_with(|| left.cmp(right))
    });
}

pub fn host_rank(host: &str) -> u8 {
    let Ok(addr) = host.parse::<Ipv4Addr>() else {
        return 9;
    };
    if is_rfc1918(addr) {
        0
    } else if is_cgnat(addr) {
        1
    } else if addr.is_loopback() {
        3
    } else {
        2
    }
}

pub fn host_kind(host: &str) -> &'static str {
    match host_rank(host) {
        0 => "lan",
        1 => "tailscale",
        3 => "loopback",
        9 => "name",
        _ => "other",
    }
}

fn is_rfc1918(addr: Ipv4Addr) -> bool {
    let [a, b, _, _] = addr.octets();
    a == 10 || (a == 172 && (16..=31).contains(&b)) || (a == 192 && b == 168)
}

fn is_cgnat(addr: Ipv4Addr) -> bool {
    let [a, b, _, _] = addr.octets();
    a == 100 && (64..=127).contains(&b)
}

fn default_route_v4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(addr) if !addr.is_unspecified() => Some(addr),
        _ => None,
    }
}

pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    pub fn stop(self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

pub fn advertise(
    port: u16,
    device_id: &str,
    name: &str,
    spki_hex: &str,
    host: &str,
) -> Result<Advertisement, LanError> {
    let daemon = ServiceDaemon::new().map_err(|_| LanError::new("mdns-unavailable"))?;
    let instance = format!("holdover-{}", &device_id[..device_id.len().min(8)]);
    let host_name = format!("{instance}.local.");
    let ip: IpAddr = host
        .parse()
        .map_err(|_| LanError::new("mdns-unavailable"))?;
    let props = [
        ("id", device_id),
        ("fp", spki_hex),
        ("name", name),
        ("schema", "1"),
    ];
    let mut info = ServiceInfo::new(SERVICE_TYPE, &instance, &host_name, ip, port, &props[..])
        .map_err(|_| LanError::new("mdns-unavailable"))?;
    // The name is the device id. Probing waits for a multicast reply that a
    // blocked network will never send, so the service would stay unpublished.
    info.set_requires_probe(false);
    let fullname = info.get_fullname().to_string();
    daemon
        .register(info)
        .map_err(|_| LanError::new("mdns-unavailable"))?;
    Ok(Advertisement { daemon, fullname })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub device_id: String,
    pub spki_hex: String,
}

pub fn browse(wait: Duration) -> Result<Vec<Found>, LanError> {
    let daemon = ServiceDaemon::new().map_err(|_| LanError::new("mdns-unavailable"))?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|_| LanError::new("mdns-unavailable"))?;
    let deadline = std::time::Instant::now() + wait;
    let mut found = Vec::new();
    while std::time::Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match receiver.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let props = info.get_properties();
                let value = |key: &str| {
                    props
                        .get(key)
                        .map(|item| item.val_str().to_string())
                        .unwrap_or_default()
                };
                let host = info
                    .get_addresses()
                    .iter()
                    .find(|addr| addr.is_ipv4())
                    .map(|addr| addr.to_string())
                    .unwrap_or_default();
                if host.is_empty() || value("fp").len() != 64 {
                    continue;
                }
                let item = Found {
                    name: value("name"),
                    host,
                    port: info.get_port(),
                    device_id: value("id"),
                    spki_hex: value("fp"),
                };
                if !found
                    .iter()
                    .any(|have: &Found| have.spki_hex == item.spki_hex && have.port == item.port)
                {
                    found.push(item);
                }
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    let _ = daemon.shutdown();
    Ok(found)
}

/// `preferred` of 0 asks the system for a port. A preferred port that is busy
/// falls back to a system port so Ready for a sync still starts. The socket
/// allows reuse so a listener that has just closed does not leave the port
/// stuck. Two listeners still cannot hold the same port at once.
pub fn bind_listener(preferred: u16) -> Result<(TcpListener, u16), LanError> {
    let mut ports = Vec::new();
    if preferred != 0 {
        ports.push(preferred);
    }
    ports.push(0);
    for port in ports {
        if let Some(listener) = bind_one(port) {
            let bound = listener
                .local_addr()
                .map_err(|_| LanError::new("disconnected"))?
                .port();
            return Ok((listener, bound));
        }
    }
    Err(LanError::new("disconnected"))
}

fn bind_one(port: u16) -> Option<TcpListener> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).ok()?;
    socket.set_reuse_address(true).ok()?;
    let addr = SockAddr::from(SocketAddr::from(([0, 0, 0, 0], port)));
    socket.bind(&addr).ok()?;
    socket.listen(128).ok()?;
    Some(TcpListener::from(socket))
}

#[cfg(test)]
mod tests {
    use super::{bind_listener, host_rank, sort_hosts};

    #[test]
    fn lan_addresses_sort_before_tailscale() {
        let mut hosts = vec![
            "100.64.0.8".to_string(),
            "8.8.8.8".to_string(),
            "192.168.1.20".to_string(),
            "172.16.0.4".to_string(),
        ];
        sort_hosts(&mut hosts);
        let lan = hosts
            .iter()
            .position(|host| host.starts_with("192."))
            .unwrap();
        let tail = hosts
            .iter()
            .position(|host| host.starts_with("100."))
            .unwrap();
        let other = hosts
            .iter()
            .position(|host| host.starts_with("8."))
            .unwrap();
        assert!(lan < tail);
        assert!(tail < other);
        assert_eq!(host_rank("10.1.2.3"), 0);
        assert_eq!(host_rank("100.127.0.1"), 1);
        assert_eq!(host_rank("100.63.0.1"), 2);
    }

    #[test]
    fn closing_the_listener_frees_the_same_port_and_reuse_is_set() {
        let (first, port) = bind_listener(0).unwrap();
        let sock = socket2::SockRef::from(&first);
        assert!(sock.reuse_address().unwrap());
        drop(first);
        let (second, again) = bind_listener(port).unwrap();
        assert_eq!(again, port, "the closed listener still held the port");
        drop(second);
    }

    #[test]
    fn a_busy_preferred_port_falls_back_and_a_free_one_is_kept() {
        let (held, port) = bind_listener(0).unwrap();
        let (_busy, fallback) = bind_listener(port).unwrap();
        assert_ne!(fallback, port);
        drop(held);
        let (_again, reused) = bind_listener(port).unwrap();
        assert_eq!(reused, port);
    }
}
