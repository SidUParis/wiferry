use if_addrs::{IfAddr, get_if_addrs};
use std::collections::BTreeSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

#[derive(Debug, Clone)]
pub struct LocalNetwork {
    pub address: Ipv4Addr,
    pub network: u32,
    pub mask: u32,
}

impl LocalNetwork {
    pub fn contains(&self, address: Ipv4Addr) -> bool {
        (u32::from(address) & self.mask) == self.network
    }
}

pub fn interfaces() -> io::Result<Vec<LocalNetwork>> {
    let mut result = Vec::new();
    for interface in get_if_addrs()? {
        let IfAddr::V4(v4) = interface.addr else {
            continue;
        };
        if v4.ip.is_loopback() || v4.ip.is_link_local() || v4.ip.is_unspecified() {
            continue;
        }
        let mask = u32::from(v4.netmask);
        result.push(LocalNetwork {
            address: v4.ip,
            network: u32::from(v4.ip) & mask,
            mask,
        });
    }
    Ok(result)
}

pub fn candidates() -> Vec<Ipv4Addr> {
    let routed = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(1, 1, 1, 1), 80))?;
            match socket.local_addr()?.ip() {
                IpAddr::V4(address) => Ok(Some(address)),
                _ => Ok(None),
            }
        })
        .ok()
        .flatten();
    let all = interfaces().unwrap_or_default();
    let mut unique = BTreeSet::new();
    for item in &all {
        unique.insert(item.address);
    }
    let mut result = Vec::new();
    if let Some(address) = routed
        && unique.remove(&address)
    {
        result.push(address);
    }
    result.extend(unique);
    result
}

pub fn guest_allowed(peer: SocketAddr, networks: &[LocalNetwork]) -> bool {
    match peer.ip() {
        IpAddr::V4(address) => {
            address.is_loopback() || networks.iter().any(|net| net.contains(address))
        }
        IpAddr::V6(address) => address.is_loopback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subnet_guard_accepts_only_local_network() {
        let network = LocalNetwork {
            address: Ipv4Addr::new(192, 168, 1, 10),
            network: u32::from(Ipv4Addr::new(192, 168, 1, 0)),
            mask: u32::from(Ipv4Addr::new(255, 255, 255, 0)),
        };
        assert!(guest_allowed(
            "192.168.1.22:8".parse().unwrap(),
            std::slice::from_ref(&network)
        ));
        assert!(guest_allowed(
            "127.0.0.1:8".parse().unwrap(),
            std::slice::from_ref(&network)
        ));
        assert!(!guest_allowed(
            "192.168.2.22:8".parse().unwrap(),
            &[network]
        ));
    }
}
