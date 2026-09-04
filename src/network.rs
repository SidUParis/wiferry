use if_addrs::{IfAddr, get_if_addrs};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const TAILSCALE_PROBE_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    Lan,
    Tailscale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkCandidate {
    pub address: Ipv4Addr,
    pub kind: TransportKind,
    pub label: String,
    #[serde(skip)]
    network: u32,
    #[serde(skip)]
    mask: u32,
}

impl NetworkCandidate {
    pub fn lan(address: Ipv4Addr, netmask: Ipv4Addr, label: impl Into<String>) -> Self {
        let mask = u32::from(netmask);
        Self {
            address,
            kind: TransportKind::Lan,
            label: label.into(),
            network: u32::from(address) & mask,
            mask,
        }
    }

    pub fn tailscale(address: Ipv4Addr) -> Self {
        Self {
            address,
            kind: TransportKind::Tailscale,
            label: "Tailscale".into(),
            network: 0,
            mask: 0,
        }
    }

    pub fn loopback() -> Self {
        Self::lan(Ipv4Addr::LOCALHOST, Ipv4Addr::new(255, 0, 0, 0), "Loopback")
    }

    fn contains_lan_peer(&self, address: Ipv4Addr) -> bool {
        (u32::from(address) & self.mask) == self.network
    }
}

#[derive(Debug, Clone)]
struct InterfaceAddress {
    name: String,
    address: Ipv4Addr,
    netmask: Ipv4Addr,
}

pub fn candidates(probe_tailscale_cli: bool) -> io::Result<Vec<NetworkCandidate>> {
    let interfaces = interface_addresses()?;
    let routed = routed_ipv4();
    let tailscale_ips = if probe_tailscale_cli {
        tailscale_cli_ipv4s()
    } else {
        BTreeSet::new()
    };
    Ok(classify_candidates(&interfaces, routed, &tailscale_ips))
}

pub fn select_candidate(
    candidates: &[NetworkCandidate],
    requested_address: Option<Ipv4Addr>,
    requested_transport: Option<TransportKind>,
) -> Result<NetworkCandidate, String> {
    if requested_address.is_some_and(|address| address.is_loopback()) {
        if requested_transport == Some(TransportKind::Tailscale) {
            return Err("a loopback --host-ip cannot use Tailscale transport".into());
        }
        return Ok(NetworkCandidate::loopback());
    }

    if let Some(address) = requested_address {
        let Some(candidate) = candidates.iter().find(|item| item.address == address) else {
            return Err(format!(
                "--host-ip must be assigned to this computer: {address}"
            ));
        };
        if requested_transport.is_some_and(|kind| kind != candidate.kind) {
            return Err(format!(
                "--host-ip {address} is not a confirmed {} address on this computer",
                transport_name(requested_transport.unwrap())
            ));
        }
        return Ok(candidate.clone());
    }

    if let Some(kind) = requested_transport {
        return candidates
            .iter()
            .find(|candidate| candidate.kind == kind)
            .cloned()
            .ok_or_else(|| match kind {
                TransportKind::Lan => {
                    "LAN transport requested, but no local LAN IPv4 address was found".into()
                }
                TransportKind::Tailscale => concat!(
                    "Tailscale transport requested, but no confirmed local Tailscale IPv4 ",
                    "address was found; make sure Tailscale is installed and connected"
                )
                .into(),
            });
    }

    Ok(candidates
        .first()
        .cloned()
        .unwrap_or_else(NetworkCandidate::loopback))
}

pub fn guest_allowed(peer: SocketAddr, selected: &NetworkCandidate) -> bool {
    match peer.ip() {
        IpAddr::V4(address) if address.is_loopback() => true,
        IpAddr::V4(address) => match selected.kind {
            TransportKind::Lan => selected.contains_lan_peer(address),
            TransportKind::Tailscale => is_tailscale_ipv4(address),
        },
        IpAddr::V6(address) => address.is_loopback(),
    }
}

fn interface_addresses() -> io::Result<Vec<InterfaceAddress>> {
    let mut result = Vec::new();
    for interface in get_if_addrs()? {
        let IfAddr::V4(v4) = interface.addr else {
            continue;
        };
        if v4.ip.is_loopback() || v4.ip.is_link_local() || v4.ip.is_unspecified() {
            continue;
        }
        result.push(InterfaceAddress {
            name: interface.name,
            address: v4.ip,
            netmask: v4.netmask,
        });
    }
    Ok(result)
}

fn routed_ipv4() -> Option<Ipv4Addr> {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|socket| {
            socket.connect((Ipv4Addr::new(1, 1, 1, 1), 80))?;
            match socket.local_addr()?.ip() {
                IpAddr::V4(address) => Ok(Some(address)),
                _ => Ok(None),
            }
        })
        .ok()
        .flatten()
}

fn tailscale_cli_ipv4s() -> BTreeSet<Ipv4Addr> {
    let Ok(mut child) = Command::new("tailscale")
        .args(["ip", "-4"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return BTreeSet::new();
    };
    let deadline = Instant::now() + TAILSCALE_PROBE_TIMEOUT;
    let output = loop {
        match child.try_wait() {
            Ok(Some(_)) => break child.wait_with_output().ok(),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let Some(output) = output else {
        return BTreeSet::new();
    };
    if !output.status.success() {
        return BTreeSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<Ipv4Addr>().ok())
        .filter(|address| is_tailscale_ipv4(*address))
        .collect()
}

fn classify_candidates(
    interfaces: &[InterfaceAddress],
    routed: Option<Ipv4Addr>,
    tailscale_cli_ips: &BTreeSet<Ipv4Addr>,
) -> Vec<NetworkCandidate> {
    let assigned: BTreeSet<Ipv4Addr> = interfaces.iter().map(|item| item.address).collect();
    let verified_cli_ips: BTreeSet<Ipv4Addr> =
        tailscale_cli_ips.intersection(&assigned).copied().collect();
    let mut by_address = BTreeMap::new();

    for interface in interfaces {
        let explicit_tailscale_interface =
            interface.name.to_ascii_lowercase().contains("tailscale");
        let is_tailscale = is_tailscale_ipv4(interface.address)
            && (explicit_tailscale_interface || verified_cli_ips.contains(&interface.address));
        let candidate = if is_tailscale {
            NetworkCandidate::tailscale(interface.address)
        } else {
            NetworkCandidate::lan(
                interface.address,
                interface.netmask,
                format!("LAN · {}", interface.name),
            )
        };
        by_address
            .entry(interface.address)
            .and_modify(|existing: &mut NetworkCandidate| {
                if candidate.kind == TransportKind::Tailscale {
                    *existing = candidate.clone();
                }
            })
            .or_insert(candidate);
    }

    let mut result: Vec<NetworkCandidate> = by_address.into_values().collect();
    result.sort_by_key(|candidate| {
        let priority = if Some(candidate.address) == routed {
            0
        } else if candidate.kind == TransportKind::Lan {
            1
        } else {
            2
        };
        (priority, candidate.address)
    });
    result
}

fn is_tailscale_ipv4(address: Ipv4Addr) -> bool {
    const CGNAT_MASK: u32 = 0xffc0_0000;
    const TAILSCALE_NETWORK: u32 = 0x6440_0000;
    (u32::from(address) & CGNAT_MASK) == TAILSCALE_NETWORK
}

fn transport_name(kind: TransportKind) -> &'static str {
    match kind {
        TransportKind::Lan => "LAN",
        TransportKind::Tailscale => "Tailscale",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interface(name: &str, address: &str, netmask: &str) -> InterfaceAddress {
        InterfaceAddress {
            name: name.into(),
            address: address.parse().unwrap(),
            netmask: netmask.parse().unwrap(),
        }
    }

    #[test]
    fn classifies_only_confirmed_tailscale_addresses() {
        let interfaces = vec![
            interface("wlan0", "192.168.1.10", "255.255.255.0"),
            interface("tailscale0", "100.123.32.112", "255.255.255.255"),
            interface("carrier", "100.70.1.2", "255.255.255.0"),
            interface("utun8", "100.99.8.7", "255.255.255.255"),
        ];
        let verified = BTreeSet::from(["100.99.8.7".parse().unwrap()]);
        let candidates = classify_candidates(
            &interfaces,
            Some("192.168.1.10".parse().unwrap()),
            &verified,
        );

        assert_eq!(
            candidates[0].address,
            "192.168.1.10".parse::<Ipv4Addr>().unwrap()
        );
        assert_eq!(candidates[0].kind, TransportKind::Lan);
        assert_eq!(
            candidates
                .iter()
                .find(|item| item.address == "100.123.32.112".parse::<Ipv4Addr>().unwrap())
                .unwrap()
                .kind,
            TransportKind::Tailscale
        );
        assert_eq!(
            candidates
                .iter()
                .find(|item| item.address == "100.99.8.7".parse::<Ipv4Addr>().unwrap())
                .unwrap()
                .kind,
            TransportKind::Tailscale
        );
        assert_eq!(
            candidates
                .iter()
                .find(|item| item.address == "100.70.1.2".parse::<Ipv4Addr>().unwrap())
                .unwrap()
                .kind,
            TransportKind::Lan
        );
    }

    #[test]
    fn lan_policy_accepts_only_the_selected_subnet() {
        let selected = NetworkCandidate::lan(
            "192.168.1.10".parse().unwrap(),
            "255.255.255.0".parse().unwrap(),
            "LAN",
        );
        assert!(guest_allowed("192.168.1.22:8".parse().unwrap(), &selected));
        assert!(guest_allowed("127.0.0.1:8".parse().unwrap(), &selected));
        assert!(!guest_allowed("192.168.2.22:8".parse().unwrap(), &selected));
        assert!(!guest_allowed("100.90.1.2:8".parse().unwrap(), &selected));
    }

    #[test]
    fn tailscale_policy_accepts_only_tailnet_ipv4_and_loopback() {
        let selected = NetworkCandidate::tailscale("100.123.32.112".parse().unwrap());
        assert!(guest_allowed("100.64.0.1:8".parse().unwrap(), &selected));
        assert!(guest_allowed(
            "100.127.255.254:8".parse().unwrap(),
            &selected
        ));
        assert!(guest_allowed("127.0.0.1:8".parse().unwrap(), &selected));
        assert!(!guest_allowed(
            "100.63.255.255:8".parse().unwrap(),
            &selected
        ));
        assert!(!guest_allowed("100.128.0.1:8".parse().unwrap(), &selected));
        assert!(!guest_allowed("192.168.1.22:8".parse().unwrap(), &selected));
    }

    #[test]
    fn explicit_tailscale_selection_fails_closed_without_candidate() {
        let candidates = vec![NetworkCandidate::lan(
            "192.168.1.10".parse().unwrap(),
            "255.255.255.0".parse().unwrap(),
            "LAN",
        )];
        let error =
            select_candidate(&candidates, None, Some(TransportKind::Tailscale)).unwrap_err();
        assert!(error.contains("no confirmed local Tailscale"));
    }
}
