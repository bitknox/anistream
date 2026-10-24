//! Finding a TV: DNS-SD over multicast DNS where the network allows it, and a pairing link
//! where it does not.
//!
//! The engine does no multicast of its own. Each platform already runs an mDNS responder that
//! the system manages, shares and wakes for its apps (Android's NsdManager, Apple's Bonjour,
//! Avahi on Linux), and a second one in-process would fight it for port 5353, need its own
//! multicast permission (a restricted entitlement on iOS) and keep a radio awake. So
//! [`Advertiser`] and [`Browser`] are implemented by the app, through its host, and this module
//! only says what to advertise and how to read what was found. [`MemoryNetwork`] stands in for
//! the network in tests.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::sync::mpsc;

use crate::{identity::Fingerprint, wire::PROTOCOL};

/// The DNS-SD service type a TV registers. At most 15 characters before `._tcp`, as RFC 6335
/// asks.
pub const SERVICE_TYPE: &str = "_anistream._tcp";

/// The port a TV listens on when it is free, so an address typed by hand rarely needs one. When
/// it is taken the TV takes any port and says which, in its advert and on its screen.
pub const DEFAULT_PORT: u16 = 47_811;

/// What a TV advertises: its name as the instance, the port, and a TXT record of
/// `v` (the protocol's major version) and `id` (the short form of its key's fingerprint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advert {
    pub instance: String,
    pub port: u16,
    pub txt: Vec<(String, String)>,
}

impl Advert {
    pub fn new(tv_name: &str, port: u16, fingerprint: &Fingerprint) -> Self {
        Self {
            // An instance name is at most 63 bytes (RFC 6763, section 4.1.1).
            instance: truncate(tv_name, 63),
            port,
            txt: vec![("v".into(), PROTOCOL.to_string()), ("id".into(), fingerprint.short())],
        }
    }
}

/// A TV a phone found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundTv {
    pub instance: String,
    pub addresses: Vec<SocketAddr>,
    /// The short fingerprint from its TXT record: which known TV this is, if any.
    pub id: Option<String>,
    /// Its protocol's major version, when it said.
    pub protocol: Option<u32>,
}

impl FoundTv {
    pub fn from_txt(
        instance: &str,
        addresses: Vec<SocketAddr>,
        txt: &[(String, String)],
    ) -> Self {
        let get = |key: &str| {
            txt.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.clone())
        };
        Self {
            instance: instance.to_owned(),
            addresses,
            id: get("id"),
            protocol: get("v").and_then(|v| v.parse().ok()),
        }
    }

    /// Whether this is the TV pinned as `fingerprint`. A hint for the list; the connection
    /// checks the whole key.
    pub fn is(&self, fingerprint: &Fingerprint) -> bool {
        self.id.as_deref() == Some(fingerprint.short().as_str())
    }

    /// Whether this phone can talk to it.
    pub fn compatible(&self) -> bool {
        self.protocol.is_none_or(|v| v == PROTOCOL)
    }
}

/// What a browse reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    Found(FoundTv),
    Lost { instance: String },
}

/// Where a platform's browser reports what it finds. Cheap to clone.
#[derive(Debug, Clone)]
pub struct DiscoverySink(mpsc::UnboundedSender<Discovery>);

impl DiscoverySink {
    pub fn channel() -> (Self, mpsc::UnboundedReceiver<Discovery>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self(tx), rx)
    }

    pub fn found(&self, tv: FoundTv) {
        let _ = self.0.send(Discovery::Found(tv));
    }

    pub fn lost(&self, instance: &str) {
        let _ = self.0.send(Discovery::Lost { instance: instance.to_owned() });
    }
}

/// The TV's side: register the service while the remote is on, withdraw it when it is off.
pub trait Advertiser: Send + Sync {
    fn advertise(&self, advert: &Advert);
    fn withdraw(&self, instance: &str);
}

/// The phone's side: browse only while the TV picker is open, and stop when it closes.
pub trait Browser: Send + Sync {
    fn start(&self, sink: DiscoverySink);
    fn stop(&self);
}

/// A network in memory, for tests: what one side advertises the other finds, at the address
/// the test says the TV listens on.
#[derive(Debug, Default, Clone)]
pub struct MemoryNetwork {
    inner: Arc<Mutex<Network>>,
}

#[derive(Debug, Default)]
struct Network {
    adverts: BTreeMap<String, Advert>,
    host: Option<std::net::IpAddr>,
    browsers: Vec<DiscoverySink>,
}

impl MemoryNetwork {
    /// TVs on this network are found at `host`.
    pub fn new(host: std::net::IpAddr) -> Self {
        let network = Self::default();
        network.lock().host = Some(host);
        network
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Network> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn found(network: &Network, advert: &Advert) -> FoundTv {
        let host = network.host.unwrap_or(std::net::Ipv4Addr::LOCALHOST.into());
        FoundTv::from_txt(
            &advert.instance,
            vec![SocketAddr::new(host, advert.port)],
            &advert.txt,
        )
    }
}

impl Advertiser for MemoryNetwork {
    fn advertise(&self, advert: &Advert) {
        let mut network = self.lock();
        network.adverts.insert(advert.instance.clone(), advert.clone());
        let found = Self::found(&network, advert);
        for sink in &network.browsers {
            sink.found(found.clone());
        }
    }

    fn withdraw(&self, instance: &str) {
        let mut network = self.lock();
        network.adverts.remove(instance);
        for sink in &network.browsers {
            sink.lost(instance);
        }
    }
}

impl Browser for MemoryNetwork {
    fn start(&self, sink: DiscoverySink) {
        let mut network = self.lock();
        for advert in network.adverts.values() {
            sink.found(Self::found(&network, advert));
        }
        network.browsers.push(sink);
    }

    fn stop(&self) {
        self.lock().browsers.clear();
    }
}

/// What the TV shows as a QR code beside its pairing code, and what a phone takes typed by hand
/// when multicast is blocked: where the TV is, its key, and the code.
///
/// `anistream-remote://pair?addr=192.168.1.20:47811&tv=<fingerprint>&code=123456`
///
/// Scanned, it carries the TV's whole fingerprint, so the phone pins the key before it connects
/// and pairing has nothing to take on trust. Typed, it is the address alone, and pairing proves
/// the key as it does after a browse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairLink {
    pub addr: SocketAddr,
    pub tv: Option<Fingerprint>,
    pub code: Option<String>,
}

impl PairLink {
    pub const SCHEME: &str = "anistream-remote://pair";
}

impl std::fmt::Display for PairLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}?addr={}", Self::SCHEME, self.addr)?;
        if let Some(tv) = &self.tv {
            write!(f, "&tv={tv}")?;
        }
        if let Some(code) = &self.code {
            write!(f, "&code={code}")?;
        }
        Ok(())
    }
}

impl FromStr for PairLink {
    type Err = &'static str;

    /// A link, or an address typed by hand: `192.168.1.20` or `192.168.1.20:47811`.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        let Some(query) = text.strip_prefix(Self::SCHEME) else {
            let addr = SocketAddr::from_str(text)
                .or_else(|_| {
                    std::net::IpAddr::from_str(text).map(|ip| SocketAddr::new(ip, DEFAULT_PORT))
                })
                .map_err(|_| "not an address")?;
            return Ok(Self { addr, tv: None, code: None });
        };
        let query = query.strip_prefix('?').ok_or("no parameters")?;
        let (mut addr, mut tv, mut code) = (None, None, None);
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').ok_or("a parameter without a value")?;
            match key {
                "addr" => {
                    addr = Some(SocketAddr::from_str(value).map_err(|_| "not an address")?)
                }
                "tv" => tv = Some(Fingerprint::from_hex(value).ok_or("not a fingerprint")?),
                "code" if value.len() == 6 && value.bytes().all(|b| b.is_ascii_digit()) => {
                    code = Some(value.to_owned());
                }
                "code" => return Err("not a code"),
                _ => {}
            }
        }
        Ok(Self { addr: addr.ok_or("no address")?, tv, code })
    }
}

fn truncate(text: &str, max: usize) -> String {
    let mut end = text.len().min(max);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    #[test]
    fn a_found_tv_reads_its_txt_record() {
        let fp = Identity::generate().unwrap().fingerprint();
        let advert = Advert::new("Living room", 47_811, &fp);
        let found = FoundTv::from_txt(&advert.instance, vec![], &advert.txt);
        assert!(found.is(&fp));
        assert!(found.compatible());
        assert_eq!(found.protocol, Some(PROTOCOL));
    }

    #[test]
    fn a_pair_link_round_trips_and_a_bare_address_takes_the_default_port() {
        let link = PairLink {
            addr: "192.168.1.20:47811".parse().unwrap(),
            tv: Some(Identity::generate().unwrap().fingerprint()),
            code: Some("042917".into()),
        };
        assert_eq!(link.to_string().parse::<PairLink>(), Ok(link));
        let typed: PairLink = "10.0.0.7".parse().unwrap();
        assert_eq!(typed.addr.port(), DEFAULT_PORT);
        assert!("anistream-remote://pair?addr=10.0.0.7:1&code=12".parse::<PairLink>().is_err());
    }

    #[tokio::test]
    async fn a_browser_finds_what_was_advertised_and_hears_it_go() {
        let network = MemoryNetwork::new([10, 0, 0, 7].into());
        let fp = Identity::generate().unwrap().fingerprint();
        network.advertise(&Advert::new("Den", 5000, &fp));
        let (sink, mut found) = DiscoverySink::channel();
        network.start(sink);
        let Some(Discovery::Found(tv)) = found.recv().await else { panic!("nothing found") };
        assert_eq!(tv.addresses, vec!["10.0.0.7:5000".parse().unwrap()]);
        network.withdraw("Den");
        assert_eq!(found.recv().await, Some(Discovery::Lost { instance: "Den".into() }));
    }

    #[test]
    fn a_long_name_is_cut_on_a_character() {
        let name = "é".repeat(40);
        assert!(
            Advert::new(&name, 1, &Identity::generate().unwrap().fingerprint()).instance.len()
                <= 63
        );
    }
}
