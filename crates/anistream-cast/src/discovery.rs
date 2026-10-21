//! Finding Cast devices: one mDNS question for `_googlecast._tcp.local`, and the answers that
//! come back within a wait.
//!
//! The question is a *legacy unicast* one, sent from an ephemeral port rather than 5353, so the
//! devices answer straight back to that port and nothing has to share 5353 with the system's own
//! responder. It goes out on the interface the multicast route takes, which is the LAN even when
//! the default route is a VPN. A phone does not use this: Android's NSD and iOS's Bonjour browse
//! for the app, and iOS forbids raw multicast without an entitlement Apple grants on request.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    time::{Duration, Instant},
};

use socket2::{Domain, Protocol, Socket, Type};

const MDNS: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(224, 0, 0, 251), 5353);
pub const SERVICE: &str = "_googlecast._tcp.local";

/// A Cast device, as its mDNS record describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastDevice {
    /// The instance name, `Google-Nest-Hub-…`.
    pub instance: String,
    /// The friendly name the owner gave it (`fn`).
    pub name: String,
    /// The model (`md`).
    pub model: String,
    /// Its id (`id`), stable across addresses.
    pub id: String,
    pub address: SocketAddr,
    pub txt: HashMap<String, String>,
}

/// The local IPv4 address whose interface reaches `destination`: the one to bind a LAN server
/// to, so it listens on that interface alone. A UDP connect sends nothing; it only routes.
pub fn local_address_towards(destination: Ipv4Addr) -> std::io::Result<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
    socket.connect((destination, 9))?;
    match socket.local_addr()?.ip() {
        std::net::IpAddr::V4(ip) => Ok(ip),
        std::net::IpAddr::V6(_) => Err(std::io::Error::other("no IPv4 route")),
    }
}

/// Ask once, and collect every device that answers within `wait`.
pub fn discover(wait: Duration) -> std::io::Result<Vec<CastDevice>> {
    let interface = local_address_towards(*MDNS.ip())?;
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.bind(&SocketAddr::from((interface, 0)).into())?;
    socket.set_multicast_if_v4(&interface)?;
    socket.set_multicast_ttl_v4(255)?;
    let socket: UdpSocket = socket.into();
    socket.set_read_timeout(Some(Duration::from_millis(200)))?;
    let id = rand::random::<u16>() | 1;
    socket.send_to(&query(id, SERVICE, 12), MDNS)?;

    let mut records = Records::default();
    let deadline = Instant::now() + wait;
    let mut buffer = [0u8; 9000];
    let mut asked_again = false;
    while Instant::now() < deadline {
        match socket.recv_from(&mut buffer) {
            Ok((n, _)) => {
                if let Some(message) = buffer.get(..n) {
                    records.read(message);
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(e) => return Err(e),
        }
        // A device that answered without its address in the additional records: ask for it.
        if !asked_again && Instant::now() + wait / 2 > deadline {
            asked_again = true;
            for target in records.missing_addresses() {
                socket.send_to(&query(id, &target, 1), MDNS)?;
            }
        }
    }
    Ok(records.devices())
}

/// A DNS query for `name`'s `kind` records, asking for a unicast answer.
fn query(id: u16, name: &str, kind: u16) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.extend(id.to_be_bytes());
    out.extend([0, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
    for label in name.trim_end_matches('.').split('.') {
        out.push(label.len() as u8);
        out.extend(label.as_bytes());
    }
    out.push(0);
    out.extend(kind.to_be_bytes());
    out.extend(0x8001u16.to_be_bytes());
    out
}

#[derive(Default)]
struct Records {
    instances: Vec<String>,
    services: HashMap<String, (String, u16)>,
    texts: HashMap<String, HashMap<String, String>>,
    addresses: HashMap<String, Ipv4Addr>,
}

impl Records {
    fn read(&mut self, message: &[u8]) {
        let _ = self.try_read(message);
    }

    fn try_read(&mut self, message: &[u8]) -> Option<()> {
        let count =
            |at: usize| Some(u16::from_be_bytes(message.get(at..at + 2)?.try_into().ok()?));
        let questions = count(4)?;
        let records = count(6)? as usize + count(8)? as usize + count(10)? as usize;
        let mut at = 12;
        for _ in 0..questions {
            at = read_name(message, at)?.1 + 4;
        }
        for _ in 0..records {
            let (name, next) = read_name(message, at)?;
            let kind = u16::from_be_bytes(message.get(next..next + 2)?.try_into().ok()?);
            let len = count(next + 8)? as usize;
            let data_at = next + 10;
            let data = message.get(data_at..data_at + len)?;
            match kind {
                12 if name.eq_ignore_ascii_case(SERVICE) => {
                    let (instance, _) = read_name(message, data_at)?;
                    if !self.instances.contains(&instance) {
                        self.instances.push(instance);
                    }
                }
                33 => {
                    let port = u16::from_be_bytes(data.get(4..6)?.try_into().ok()?);
                    let (target, _) = read_name(message, data_at + 6)?;
                    self.services.insert(name, (target, port));
                }
                16 => {
                    let mut entries = HashMap::new();
                    let mut rest = data;
                    while let Some((&len, tail)) = rest.split_first() {
                        let entry = tail.get(..len as usize)?;
                        rest = &tail[len as usize..];
                        let entry = String::from_utf8_lossy(entry);
                        if let Some((key, value)) = entry.split_once('=') {
                            entries.insert(key.to_owned(), value.to_owned());
                        }
                    }
                    self.texts.insert(name, entries);
                }
                1 if len == 4 => {
                    self.addresses
                        .insert(name, Ipv4Addr::new(data[0], data[1], data[2], data[3]));
                }
                _ => {}
            }
            at = data_at + len;
        }
        Some(())
    }

    fn missing_addresses(&self) -> Vec<String> {
        self.services
            .values()
            .filter(|(target, _)| !self.addresses.contains_key(target))
            .map(|(target, _)| target.clone())
            .collect()
    }

    fn devices(&self) -> Vec<CastDevice> {
        self.instances
            .iter()
            .filter_map(|instance| {
                let (target, port) = self.services.get(instance)?;
                let ip = self.addresses.get(target)?;
                let txt = self.texts.get(instance).cloned().unwrap_or_default();
                let field = |key: &str| txt.get(key).cloned().unwrap_or_default();
                Some(CastDevice {
                    instance: instance.trim_end_matches(&format!(".{SERVICE}")).to_owned(),
                    name: field("fn"),
                    model: field("md"),
                    id: field("id"),
                    address: SocketAddr::from((*ip, *port)),
                    txt,
                })
            })
            .collect()
    }
}

/// A name at `at`, following compression pointers, and where the record goes on after it.
fn read_name(message: &[u8], mut at: usize) -> Option<(String, usize)> {
    let mut labels = Vec::new();
    let mut end = None;
    for _ in 0..64 {
        let len = *message.get(at)?;
        if len == 0 {
            return Some((labels.join("."), end.unwrap_or(at + 1)));
        }
        if len & 0xc0 == 0xc0 {
            let pointer = (usize::from(len & 0x3f) << 8) | usize::from(*message.get(at + 1)?);
            end.get_or_insert(at + 2);
            at = pointer;
            continue;
        }
        let label = message.get(at + 1..at + 1 + len as usize)?;
        labels.push(String::from_utf8_lossy(label).into_owned());
        at += 1 + len as usize;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_query_asks_for_the_service_by_unicast() {
        let q = query(0x1234, SERVICE, 12);
        assert_eq!(&q[..2], &[0x12, 0x34]);
        assert_eq!(&q[12..24], b"\x0b_googlecast");
        assert_eq!(&q[q.len() - 4..], &[0, 12, 0x80, 1]);
    }

    #[test]
    fn a_compressed_answer_reads_whole() {
        // The question's name at 12, and an answer whose name points back at it.
        let mut message = query(1, SERVICE, 12);
        message[6..8].copy_from_slice(&1u16.to_be_bytes());
        message.extend([0xc0, 12, 0, 12, 0, 1, 0, 0, 0, 120]);
        let mut data = vec![3];
        data.extend(b"Hub");
        data.extend([0xc0, 12]);
        message.extend((data.len() as u16).to_be_bytes());
        message.extend(data);
        let mut records = Records::default();
        records.read(&message);
        assert_eq!(records.instances, [format!("Hub.{SERVICE}")]);
    }
}
