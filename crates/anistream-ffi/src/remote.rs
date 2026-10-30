//! The remote a TV runs beside its engine, for phones on the local network to pair with and drive
//! it. The protocol is `anistream-remote`'s, written up in `docs/remote-protocol.md`.
//!
//! [`FfiEngine::start_remote`] starts it. From then on every event `next_events` returns, and
//! every player report `send` takes, is handed to the remote too, so its state follows the
//! engine's, and what the remote asks of the front end comes out of `next_events` as
//! [`Event::Remote`] among the engine's own. The app changes nothing in how it reads events.
//!
//! Built with the `remote` feature, which Android's builds turn on. Without it, the engine is
//! held as before and none of this is in the bindings.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
};

use anistream_core::ids::AnilistId;
use anistream_engine::{
    cast::CastHub,
    engine::EventStream,
    remote::{
        TvRemote,
        protocol::{
            Error as RemoteError,
            discovery::PairLink,
            identity::{Fingerprint, Identity},
            server::ServerOptions,
            state::Screen,
            trust::FileTrust,
        },
    },
};
use anistream_model::{Msg, event::Event, hosted::PlayerReport, remote::FrontIntent};
use tokio::sync::mpsc;

use crate::{FfiEngine, engine::RemoteHome, start::FfiError};

/// Where the TV's key is kept: under this id, in the host's keychain.
const IDENTITY_SECRET: &str = "remote-identity";

/// The remote as the engine holds it: the one running, if any, and the intents it sent the front
/// end, waiting to go out with the next events.
pub(crate) struct Attached {
    home: RemoteHome,
    /// Casting's hub, which reads the engine's events for a cast's card.
    casts: Arc<CastHub>,
    running: Arc<Mutex<Option<TvRemote>>>,
    intents: mpsc::UnboundedSender<FrontIntent>,
    waiting: tokio::sync::Mutex<mpsc::UnboundedReceiver<FrontIntent>>,
}

impl Attached {
    pub(crate) fn new(home: RemoteHome, casts: Arc<CastHub>) -> Self {
        let (intents, waiting) = mpsc::unbounded_channel();
        Self {
            home,
            casts,
            running: Arc::new(Mutex::new(None)),
            intents,
            waiting: tokio::sync::Mutex::new(waiting),
        }
    }

    /// The next batch of events: the engine's, or what the remote asks of the front end,
    /// whichever comes first, and what else is waiting of the remote's. Every event is handed
    /// to the remote on its way out. Cancel-safe, as both reads are.
    pub(crate) async fn read(&self, events: &tokio::sync::Mutex<EventStream>) -> Vec<Event> {
        let mut stream = events.lock().await;
        let mut waiting = self.waiting.lock().await;
        let mut batch = tokio::select! {
            batch = stream.next_events() => batch,
            Some(intent) = waiting.recv() => vec![Event::Remote(intent)],
        };
        if batch.is_empty() {
            // The engine stopped, which ends the app's loop.
            return batch;
        }
        while let Ok(intent) = waiting.try_recv() {
            batch.push(Event::Remote(intent));
        }
        if let Some(remote) = lock(&self.running).as_ref() {
            for event in &batch {
                remote.observe(event);
            }
        }
        for event in &batch {
            self.casts.observe(event);
        }
        batch
    }

    /// Casting's hub, for the sender side.
    pub(crate) fn casts(&self) -> &Arc<CastHub> {
        &self.casts
    }

    /// Where the remote's keys are kept and its files written, for the sender side too.
    pub(crate) fn home(&self) -> &RemoteHome {
        &self.home
    }

    /// A hosted player's report, on its way to the engine.
    pub(crate) fn report(&self, report: &PlayerReport) {
        if let Some(remote) = lock(&self.running).as_ref() {
            remote.observe_report(report);
        }
    }

    /// The TV's key from the host's keychain, made and kept there the first time.
    fn identity(&self) -> Result<Identity, FfiError> {
        let fail =
            |e: RemoteError| FfiError::remote(Msg::StartFailedRemote { reason: e.to_string() });
        let Some(secrets) = &self.home.secrets else {
            return Identity::generate().map_err(fail);
        };
        if let Ok(Some(kept)) = secrets.get(IDENTITY_SECRET)
            && let Some(identity) =
                from_hex(&kept).and_then(|der| Identity::from_pkcs8(&der).ok())
        {
            return Ok(identity);
        }
        let identity = Identity::generate().map_err(fail)?;
        // A key that cannot be kept still serves this session; phones paired with it pair again
        // after the next launch.
        let _ = secrets.set(IDENTITY_SECRET, &to_hex(&identity.to_pkcs8()));
        Ok(identity)
    }
}

/// An address the remote is reached at, as `pair_link` takes it, and the port it listens on.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiAdvert {
    /// The instance name: the TV's name, cut to 63 bytes.
    pub instance: String,
    pub port: u16,
    /// The TXT record, in order.
    pub txt: Vec<FfiTxtPair>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTxtPair {
    pub key: String,
    pub value: String,
}

/// A phone paired with this TV, for its settings.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPairedPhone {
    /// The phone's key's whole fingerprint, in hex: what `revoke` takes.
    pub fingerprint: String,
    pub name: String,
    /// When it paired, in Unix seconds.
    pub paired_at: i64,
    /// When it last connected, in Unix seconds.
    pub last_seen: Option<i64>,
}

/// What the TV has on screen, for a phone's remote to follow.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScreen {
    Home,
    /// A search, with the text in its field: what a phone's field mirrors.
    Search {
        query: String,
    },
    Title {
        id: AnilistId,
    },
    Player,
    Settings,
    Other,
}

impl From<FfiScreen> for Screen {
    fn from(screen: FfiScreen) -> Self {
        match screen {
            FfiScreen::Home => Self::Home,
            FfiScreen::Search { query } => Self::Search { query },
            FfiScreen::Title { id } => Self::Title { id },
            FfiScreen::Player => Self::Player,
            FfiScreen::Settings => Self::Settings,
            FfiScreen::Other => Self::Other,
        }
    }
}

/// A TV's remote, listening. Stopped by [`Self::stop`], or by a later `start_remote`.
#[derive(uniffi::Object)]
pub struct FfiTvRemote {
    running: Arc<Mutex<Option<TvRemote>>>,
    /// The engine's runtime, which stopping runs on.
    handle: tokio::runtime::Handle,
    fingerprint: Fingerprint,
    port: u16,
    advert: FfiAdvert,
}

#[uniffi::export]
impl FfiEngine {
    /// Listen for phones on `port`, `47811` as a rule, or on any port when that one is taken or
    /// is 0, under `name`: what phones call this TV. The TV's key comes from the host's keychain, made the first time,
    /// and its paired phones from the data directory. A remote already running is stopped
    /// first.
    ///
    /// What the remote asks of the front end comes out of `next_events` as `Event::Remote`.
    pub async fn start_remote(
        &self,
        name: String,
        port: u16,
    ) -> Result<Arc<FfiTvRemote>, FfiError> {
        let fail =
            |e: RemoteError| FfiError::remote(Msg::StartFailedRemote { reason: e.to_string() });
        let Some(sender) = self.remote_sender() else {
            return Err(FfiError::remote(Msg::StartFailedRemote {
                reason: "the engine has stopped".into(),
            }));
        };
        let attached = self.remote();
        let identity = attached.identity()?;
        let fingerprint = identity.fingerprint();
        let remote_dir = attached.home.data.join("remote");
        std::fs::create_dir_all(&remote_dir).map_err(|e| {
            let reason = format!("{}: {e}", remote_dir.display());
            FfiError::remote(Msg::StartFailedRemote { reason })
        })?;
        let trust = Arc::new(FileTrust::open(&remote_dir).map_err(fail)?);
        let options = ServerOptions { tv_name: name, ..ServerOptions::default() };
        let running = Arc::clone(&attached.running);
        let intents = attached.intents.clone();
        let started = self
            .on_runtime(async move {
                let old = lock(&running).take();
                if let Some(old) = old {
                    old.stop().await;
                }
                let bind = |port| SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
                let first = TvRemote::start_with(
                    &sender,
                    bind(port),
                    identity.clone(),
                    trust.clone(),
                    options.clone(),
                )
                .await;
                let (remote, mut front) = match first {
                    Ok(started) => started,
                    // Taken: any port, which the advert and the screen say.
                    Err(RemoteError::Io(e)) if e.kind() == std::io::ErrorKind::AddrInUse => {
                        TvRemote::start_with(&sender, bind(0), identity, trust, options).await?
                    }
                    Err(e) => return Err(e),
                };
                tokio::spawn(async move {
                    while let Some(intent) = front.recv().await {
                        if intents.send(intent).is_err() {
                            break;
                        }
                    }
                });
                let advert = remote.advert();
                let port = remote.local_addr().port();
                *lock(&running) = Some(remote);
                Ok((advert, port))
            })
            .await
            .ok_or_else(|| {
                FfiError::remote(Msg::StartFailedRemote {
                    reason: "the engine has stopped".into(),
                })
            })?;
        let (advert, port) = started.map_err(fail)?;
        Ok(Arc::new(FfiTvRemote {
            running: Arc::clone(&attached.running),
            handle: self.runtime_handle(),
            fingerprint,
            port,
            advert: FfiAdvert {
                instance: advert.instance,
                port: advert.port,
                txt: advert
                    .txt
                    .into_iter()
                    .map(|(key, value)| FfiTxtPair { key, value })
                    .collect(),
            },
        }))
    }
}

#[uniffi::export]
impl FfiTvRemote {
    /// What to register with NsdManager: the service type is `_anistream._tcp`.
    pub fn advert(&self) -> FfiAdvert {
        self.advert.clone()
    }

    /// The port it listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// The TV's key's short fingerprint, the four groups a phone shows to tell TVs apart.
    pub fn short_fingerprint(&self) -> String {
        self.fingerprint.short()
    }

    /// The link a QR code carries, for a phone to reach this TV at `address` (its IP on the
    /// local network) with the TV's key pinned, and the pairing code on screen when there is
    /// one. `None` when `address` is not an IP address.
    pub fn pair_link(&self, address: String, code: Option<String>) -> Option<String> {
        let ip = address.parse().ok()?;
        let link =
            PairLink { addr: SocketAddr::new(ip, self.port), tv: Some(self.fingerprint), code };
        Some(link.to_string())
    }

    /// Open a pairing code from the TV's side, for its Pair a phone screen and the QR code it
    /// shows: any phone may pair with it, typed or scanned, for five minutes or until one pairs,
    /// when `Event::Remote` brings `HidePairingCode` and then `Paired`. `None` once stopped.
    pub fn open_pairing(&self) -> Option<String> {
        lock(&self.running).as_ref().map(TvRemote::open_code)
    }

    /// Close the code on screen, as its screen or dialog closes: the one `open_pairing` opened,
    /// or one a phone asked for, which is then abandoned.
    pub fn close_pairing(&self) {
        if let Some(remote) = lock(&self.running).as_ref() {
            remote.close_code();
        }
    }

    /// What the TV has on screen now.
    pub fn set_screen(&self, screen: FfiScreen) {
        if let Some(remote) = lock(&self.running).as_ref() {
            remote.set_screen(screen.into());
        }
    }

    /// The TV's own volume, out of 100, as the system has it now: what phones show.
    pub fn set_volume(&self, percent: f64) {
        if let Some(remote) = lock(&self.running).as_ref() {
            remote.set_volume(percent);
        }
    }

    /// The phones paired with this TV, newest first.
    pub fn paired(&self) -> Vec<FfiPairedPhone> {
        let Some(remote) = lock(&self.running).as_ref().map(TvRemote::paired) else {
            return Vec::new();
        };
        let mut phones: Vec<FfiPairedPhone> = remote
            .into_iter()
            .map(|device| FfiPairedPhone {
                fingerprint: device.fingerprint.to_hex(),
                name: device.name,
                paired_at: device.paired_at,
                last_seen: device.last_seen,
            })
            .collect();
        phones.sort_by_key(|phone| std::cmp::Reverse(phone.paired_at));
        phones
    }

    /// Forget a phone and close its connection now. True when it was paired.
    pub fn revoke(&self, fingerprint: String) -> Result<bool, FfiError> {
        let Some(phone) = Fingerprint::from_hex(&fingerprint) else { return Ok(false) };
        let running = lock(&self.running);
        let Some(remote) = running.as_ref() else { return Ok(false) };
        remote
            .revoke(&phone)
            .map_err(|e| FfiError::remote(Msg::StartFailedRemote { reason: e.to_string() }))
    }

    /// Whether it is listening: false once stopped.
    pub fn is_running(&self) -> bool {
        lock(&self.running).is_some()
    }

    /// Close the listener and every phone's connection. The app withdraws the advert itself.
    pub async fn stop(&self) {
        let running = lock(&self.running).take();
        if let Some(remote) = running {
            let _ = self.handle.spawn(remote.stop()).await;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_key_reads_back_as_the_same_key() {
        let identity = Identity::generate().unwrap();
        let kept = to_hex(&identity.to_pkcs8());
        let read = Identity::from_pkcs8(&from_hex(&kept).unwrap()).unwrap();
        assert_eq!(read.fingerprint(), identity.fingerprint());
        assert_eq!(from_hex("abc"), None);
        assert_eq!(from_hex("zz"), None);
    }
}
