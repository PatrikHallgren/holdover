//! LAN addresses and mDNS. The token never goes in a discovery record.
//! If multicast is blocked, the person types the address from the pairing code.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, UdpSocket};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::tls::LanError;

pub const SERVICE_TYPE: &str = "_holdover._tcp.local.";

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
    hosts
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

pub fn bind_listener() -> Result<(TcpListener, u16), LanError> {
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], 0)))
        .map_err(|_| LanError::new("disconnected"))?;
    let port = listener
        .local_addr()
        .map_err(|_| LanError::new("disconnected"))?
        .port();
    Ok((listener, port))
}
