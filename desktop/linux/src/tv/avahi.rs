//! Finding TVs with Avahi, the system's mDNS, over its D-Bus API on the system bus: a service
//! browser for `_anistream._tcp` while the picker is up, each TV it sees resolved to its
//! addresses, port and TXT record, and handed to the engine's sink. The engine reads what a TV
//! says; this only asks the system what it has seen. With no Avahi running nothing is found,
//! and the picker's typed address still reaches a TV.

use std::{
    cell::RefCell,
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    rc::Rc,
    sync::Arc,
};

use anistream_engine::remote::protocol::discovery::{
    Browser, DiscoverySink, FoundTv, SERVICE_TYPE,
};
use gtk::{gio, glib, prelude::*};

const AVAHI: &str = "org.freedesktop.Avahi";
const SERVER: &str = "org.freedesktop.Avahi.Server";
/// Avahi 0.8's, whose browsers wait for Start: none of their signals can go before they are
/// listened for.
const SERVER2: &str = "org.freedesktop.Avahi.Server2";
const SERVICE_BROWSER: &str = "org.freedesktop.Avahi.ServiceBrowser";
/// Any interface, any protocol: `AVAHI_IF_UNSPEC`, `AVAHI_PROTO_UNSPEC`.
const UNSPEC: i32 = -1;
const TIMEOUT_MS: i32 = 5_000;

/// The engine's browser: it is asked on any thread, and the browse runs on GTK's main loop.
pub struct Avahi {
    asks: async_channel::Sender<Ask>,
}

enum Ask {
    Start(DiscoverySink),
    Stop,
}

impl Avahi {
    pub fn start() -> Arc<Self> {
        let (asks, rx) = async_channel::unbounded();
        glib::spawn_future_local(serve(rx));
        Arc::new(Self { asks })
    }
}

impl Browser for Avahi {
    fn start(&self, sink: DiscoverySink) {
        let _ = self.asks.send_blocking(Ask::Start(sink));
    }

    fn stop(&self) {
        let _ = self.asks.send_blocking(Ask::Stop);
    }
}

async fn serve(asks: async_channel::Receiver<Ask>) {
    let mut browse: Option<Browse> = None;
    while let Ok(ask) = asks.recv().await {
        if let Some(old) = browse.take() {
            old.stop();
        }
        if let Ask::Start(sink) = ask {
            match Browse::start(sink).await {
                Ok(started) => browse = Some(started),
                Err(e) => {
                    tracing::info!(error = %e, "no Avahi to find TVs with: only a typed address")
                }
            }
        }
    }
}

/// What a TV resolved to on one interface and protocol.
#[derive(Debug, Clone)]
struct Resolved {
    address: Option<IpAddr>,
    port: u16,
    txt: Vec<(String, String)>,
}

/// A TV as each interface and protocol resolved it.
type Each = BTreeMap<(i32, i32), Resolved>;

/// A browse under way.
struct Browse {
    connection: gio::DBusConnection,
    state: Rc<State>,
    _signals: gio::SignalSubscription,
}

struct State {
    sink: DiscoverySink,
    connection: gio::DBusConnection,
    /// The browser's object, once Avahi has said it; signals before that wait in `early`.
    path: RefCell<Option<String>>,
    early: RefCell<Vec<(String, String, glib::Variant)>>,
    /// Each TV by its instance name, as each interface and protocol resolved it.
    seen: RefCell<BTreeMap<String, Each>>,
}

impl Browse {
    async fn start(sink: DiscoverySink) -> Result<Self, glib::Error> {
        let connection = gio::bus_get_future(gio::BusType::System).await?;
        let state = Rc::new(State {
            sink,
            connection: connection.clone(),
            path: RefCell::default(),
            early: RefCell::default(),
            seen: RefCell::default(),
        });
        let signals = connection.subscribe_to_signal(
            Some(AVAHI),
            Some(SERVICE_BROWSER),
            None,
            None,
            None,
            gio::DBusSignalFlags::NONE,
            {
                let state = Rc::downgrade(&state);
                move |signal| {
                    if let Some(state) = state.upgrade() {
                        state.signal(signal.object_path, signal.signal_name, signal.parameters);
                    }
                }
            },
        );
        let args = (UNSPEC, UNSPEC, SERVICE_TYPE, "", 0u32).to_variant();
        let reply_type = glib::VariantTy::new("(o)").ok();
        let call = |interface: &'static str, method: &'static str| {
            connection.call_future(
                Some(AVAHI),
                "/",
                interface,
                method,
                Some(&args),
                reply_type,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
            )
        };
        const PREPARE: &str = "ServiceBrowserPrepare"; // i18n-skip
        const NEW: &str = "ServiceBrowserNew"; // i18n-skip
        let (path, prepared) = match call(SERVER2, PREPARE).await {
            Ok(reply) => (object_path(&reply), true),
            // An Avahi before 0.8 starts its browser at once.
            Err(_) => (object_path(&call(SERVER, NEW).await?), false),
        };
        let path = path.ok_or_else(|| {
            glib::Error::new(gio::IOErrorEnum::InvalidData, "Avahi named no browser") // i18n-skip
        })?;
        state.path.replace(Some(path.clone()));
        let early = state.early.take();
        for (from, name, parameters) in early {
            state.signal(&from, &name, &parameters);
        }
        if prepared {
            connection
                .call_future(
                    Some(AVAHI),
                    &path,
                    SERVICE_BROWSER,
                    "Start", // i18n-skip
                    None,
                    None,
                    gio::DBusCallFlags::NONE,
                    TIMEOUT_MS,
                )
                .await?;
        }
        Ok(Self { connection, state, _signals: signals })
    }

    /// Let the browser go. What it found stays the engine's to forget.
    fn stop(self) {
        if let Some(path) = self.state.path.take() {
            self.connection.call(
                Some(AVAHI),
                &path,
                SERVICE_BROWSER,
                "Free", // i18n-skip
                None,
                None,
                gio::DBusCallFlags::NONE,
                TIMEOUT_MS,
                gio::Cancellable::NONE,
                |_| {},
            );
        }
    }
}

impl State {
    fn signal(self: &Rc<Self>, from: &str, name: &str, parameters: &glib::Variant) {
        let path = self.path.borrow().clone();
        let Some(path) = path else {
            self.early.borrow_mut().push((
                from.to_owned(),
                name.to_owned(),
                parameters.clone(),
            ));
            return;
        };
        if from != path {
            return;
        }
        match name {
            "ItemNew" => {
                if let Some(item) = parameters.get::<(i32, i32, String, String, String, u32)>()
                {
                    self.resolve(item.0, item.1, item.2, item.3, item.4);
                }
            }
            "ItemRemove" => {
                if let Some((interface, protocol, instance, ..)) =
                    parameters.get::<(i32, i32, String, String, String, u32)>()
                {
                    self.removed(&instance, (interface, protocol));
                }
            }
            "Failure" => {
                let why = parameters.child_value(0);
                tracing::warn!(error = why.str().unwrap_or_default(), "Avahi's browse failed");
            }
            _ => {}
        }
    }

    /// Ask Avahi where a TV it saw is, and what its TXT record says.
    fn resolve(
        self: &Rc<Self>,
        interface: i32,
        protocol: i32,
        instance: String,
        kind: String,
        domain: String,
    ) {
        let args = (
            interface,
            protocol,
            instance.as_str(),
            kind.as_str(),
            domain.as_str(),
            UNSPEC,
            0u32,
        )
            .to_variant();
        let state = Rc::downgrade(self);
        self.connection.call(
            Some(AVAHI),
            "/",
            SERVER,
            "ResolveService", // i18n-skip
            Some(&args),
            glib::VariantTy::new("(iissssisqaayu)").ok(),
            gio::DBusCallFlags::NONE,
            TIMEOUT_MS,
            gio::Cancellable::NONE,
            move |reply| {
                let Some(state) = state.upgrade() else { return };
                // Gone again, or on a network that cannot reach it: nothing to list.
                let Ok(reply) = reply else { return };
                let Some(read) = read_resolved(&reply) else { return };
                if state.path.borrow().is_none() {
                    return;
                }
                state
                    .seen
                    .borrow_mut()
                    .entry(instance.clone())
                    .or_default()
                    .insert((interface, protocol), read);
                state.tell(&instance);
            },
        );
    }

    fn removed(&self, instance: &str, on: (i32, i32)) {
        let gone = {
            let mut seen = self.seen.borrow_mut();
            let Some(each) = seen.get_mut(instance) else { return };
            each.remove(&on);
            let gone = each.is_empty();
            if gone {
                seen.remove(instance);
            }
            gone
        };
        if gone {
            self.sink.lost(instance);
        } else {
            self.tell(instance);
        }
    }

    /// The TV as every interface resolved it: one entry, with each address it can be reached on.
    fn tell(&self, instance: &str) {
        let seen = self.seen.borrow();
        let Some(each) = seen.get(instance) else { return };
        let Some(first) = each.values().next() else { return };
        let addresses = reachable(
            each.values().filter_map(|r| r.address.map(|a| SocketAddr::new(a, r.port))),
        );
        self.sink.found(FoundTv::from_txt(instance, addresses, &first.txt));
    }
}

fn object_path(reply: &glib::Variant) -> Option<String> {
    reply.try_child_value(0)?.str().map(str::to_owned)
}

/// ResolveService's answer: interface, protocol, name, type, domain, host, address protocol,
/// address, port, TXT and flags.
fn read_resolved(reply: &glib::Variant) -> Option<Resolved> {
    type Answer =
        (i32, i32, String, String, String, String, i32, String, u16, Vec<Vec<u8>>, u32);
    let (_, _, _, _, _, _, _, address, port, txt, _) = reply.get::<Answer>()?;
    Some(Resolved { address: address.parse().ok(), port, txt: txt_pairs(&txt) })
}

/// A TXT record's strings, `key=value` each.
fn txt_pairs(txt: &[Vec<u8>]) -> Vec<(String, String)> {
    txt.iter()
        .filter_map(|item| {
            let text = String::from_utf8_lossy(item);
            let (key, value) = text.split_once('=')?;
            Some((key.to_owned(), value.to_owned()))
        })
        .collect()
}

/// The addresses a TV can be reached on, IPv4 first: a link-local IPv6 address needs the
/// interface it was seen on, which a socket address here cannot carry, so it is left out.
fn reachable(addresses: impl Iterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let mut kept: Vec<SocketAddr> = addresses
        .filter(|a| match a.ip() {
            IpAddr::V4(_) => true,
            IpAddr::V6(ip) => !ip.is_unicast_link_local(),
        })
        .collect();
    kept.sort_by_key(|a| (a.is_ipv6(), *a));
    kept.dedup();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_txt_record_reads_as_its_pairs() {
        let txt = vec![b"v=1".to_vec(), b"id=0123456789abcdef".to_vec(), b"flag".to_vec()];
        assert_eq!(
            txt_pairs(&txt),
            vec![("v".into(), "1".into()), ("id".into(), "0123456789abcdef".into())]
        );
    }

    #[test]
    fn addresses_lead_with_ipv4_and_leave_out_link_local() {
        let addresses = [
            "[fe80::1]:47811",
            "[2001:db8::5]:47811",
            "192.168.1.20:47811",
            "192.168.1.20:47811",
        ]
        .map(|a| a.parse::<SocketAddr>().expect("an address"));
        let kept = reachable(addresses.into_iter());
        assert_eq!(
            kept,
            vec![
                "192.168.1.20:47811".parse().expect("an address"),
                "[2001:db8::5]:47811".parse().expect("an address"),
            ]
        );
    }

    #[test]
    fn a_resolved_answer_reads_its_address_port_and_record() {
        let answer = (
            2i32,
            0i32,
            "Living room",
            SERVICE_TYPE,
            "local",
            "tv.local",
            0i32,
            "192.168.1.20",
            47_811u16,
            vec![b"v=1".to_vec()],
            0u32,
        )
            .to_variant();
        let read = read_resolved(&answer).expect("it reads");
        assert_eq!(read.address, Some("192.168.1.20".parse().expect("an address")));
        assert_eq!(read.port, 47_811);
        assert_eq!(read.txt, vec![("v".into(), "1".into())]);
    }
}
