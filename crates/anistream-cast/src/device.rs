//! A Cast receiver as a browse for `_googlecast._tcp` reports it, and where to serve it from.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

/// The `ca` capability bit for video out. A speaker or a speaker group leaves it clear.
const VIDEO_OUT: u64 = 1;

/// What the model of a speaker group says, which plays audio across its members only.
const GROUP_MODEL: &str = "Google Cast Group";

/// A Cast receiver, as its DNS-SD record describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastDevice {
    /// The service instance name, `Chromecast-…`.
    pub instance: String,
    /// The TXT `id`: stable across addresses and names, and what the engine remembers it by.
    pub id: String,
    /// The friendly name the owner gave it (`fn`).
    pub name: String,
    /// The model (`md`). Every Chromecast generation says "Chromecast".
    pub model: String,
    pub address: SocketAddr,
    /// It has a screen: the `ca` video-out bit.
    pub video: bool,
    /// A speaker group.
    pub group: bool,
}

impl CastDevice {
    /// A receiver from the TXT record as it came, or `None` when the record has no `id`.
    pub fn from_txt(
        instance: &str,
        address: SocketAddr,
        txt: &[(String, String)],
    ) -> Option<Self> {
        let field = |key: &str| {
            txt.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v.trim().to_owned())
        };
        let id = field("id").filter(|id| !id.is_empty())?;
        let model = field("md").unwrap_or_default();
        let capabilities = field("ca").and_then(|ca| ca.parse::<u64>().ok()).unwrap_or(0);
        Some(Self {
            instance: instance.to_owned(),
            name: field("fn").filter(|n| !n.is_empty()).unwrap_or_else(|| instance.to_owned()),
            group: model == GROUP_MODEL,
            model,
            id,
            address,
            video: capabilities & VIDEO_OUT != 0,
        })
    }

    /// Whether the picker shows it: a screen, and not a group of speakers.
    pub fn shown(&self) -> bool {
        self.video && !self.group
    }
}

/// The local IPv4 address whose interface reaches `destination`: the one to bind the relay to,
/// so it listens on that interface alone. A UDP connect sends nothing; it only routes.
pub fn local_address_towards(destination: Ipv4Addr) -> std::io::Result<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect((destination, 9))?;
    match socket.local_addr()?.ip() {
        IpAddr::V4(ip) => Ok(ip),
        IpAddr::V6(_) => Err(std::io::Error::other("no IPv4 route")),
    }
}

/// The private block an address is in: 10/8, 172.16/12, 192.168/16 or link-local 169.254/16.
fn private_block(ip: Ipv4Addr) -> Option<(u32, u32)> {
    let bits = u32::from(ip);
    [
        (u32::from(Ipv4Addr::new(10, 0, 0, 0)), 8),
        (u32::from(Ipv4Addr::new(172, 16, 0, 0)), 12),
        (u32::from(Ipv4Addr::new(192, 168, 0, 0)), 16),
        (u32::from(Ipv4Addr::new(169, 254, 0, 0)), 16),
    ]
    .into_iter()
    .find(|(base, prefix)| bits >> (32 - prefix) == base >> (32 - prefix))
}

/// The same-subnet check the relay binds under: both addresses in the same private block, and
/// the local one not the wildcard or loopback. A route that leaves through a VPN's public exit,
/// or a receiver on the internet, is refused. The netmask itself is the system's and is not read,
/// so two private networks of one block behind a router pass; the relay's allow-list still
/// serves the receiver's address alone.
pub fn same_network(local: Ipv4Addr, receiver: Ipv4Addr) -> bool {
    if local.is_unspecified() || local.is_loopback() || receiver.is_loopback() {
        return false;
    }
    matches!((private_block(local), private_block(receiver)), (Some(a), Some(b)) if a == b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txt(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn a_tv_is_shown_and_a_speaker_or_group_is_not() {
        let at: SocketAddr = "192.168.1.20:8009".parse().unwrap();
        let tv = CastDevice::from_txt(
            "Chromecast-1",
            at,
            &txt(&[
                ("id", "abc"),
                ("fn", "Living room"),
                ("md", "Chromecast"),
                ("ca", "465413"),
            ]),
        )
        .unwrap();
        assert_eq!((tv.name.as_str(), tv.id.as_str()), ("Living room", "abc"));
        assert!(tv.shown());
        let speaker =
            CastDevice::from_txt("Speaker-1", at, &txt(&[("id", "s"), ("ca", "199172")]))
                .unwrap();
        assert!(!speaker.shown());
        let group = CastDevice::from_txt(
            "Group-1",
            at,
            &txt(&[("id", "g"), ("md", "Google Cast Group"), ("ca", "199173")]),
        )
        .unwrap();
        assert!(!group.shown());
        assert_eq!(CastDevice::from_txt("No-id", at, &txt(&[("fn", "x")])), None);
    }

    #[test]
    fn the_relay_binds_only_beside_the_receiver() {
        let ip = |s: &str| s.parse::<Ipv4Addr>().unwrap();
        assert!(same_network(ip("192.168.1.5"), ip("192.168.1.20")));
        assert!(same_network(ip("10.0.0.5"), ip("10.20.0.9")));
        assert!(!same_network(ip("10.64.0.2"), ip("192.168.1.20")), "a VPN's tunnel address");
        assert!(!same_network(ip("0.0.0.0"), ip("192.168.1.20")));
        assert!(!same_network(ip("127.0.0.1"), ip("127.0.0.1")));
        assert!(!same_network(ip("203.0.113.4"), ip("203.0.113.5")), "public addresses");
    }
}
