//! The remote as a phone, a tablet or the Mac app holds it: find TVs on the local network, pair
//! with one, keep the TVs paired with, and drive one from its remote card. The rules are the
//! engine's (`anistream_engine::remote::sender`); this is their shape for Swift and Kotlin.
//!
//! The app browses the network itself, through the platform's own DNS-SD (Bonjour's NWBrowser,
//! Android's NsdManager), as [`FfiTvBrowser`], and reports what it finds to the
//! [`FfiDiscoverySink`] it is handed. Everything else is here: the picker's rows, pairing with a
//! code or a scanned link, the link to a paired TV and the card it shows.

use std::{
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, PoisonError},
};

use anistream_core::ids::AnilistId;
use anistream_engine::remote::{
    protocol::{
        client::Hello,
        command::{RemoteCommand, Track},
        discovery::{Browser, DiscoverySink, FoundTv},
        identity::{Fingerprint, Identity},
        known::{KnownTv, KnownTvs},
        state::{Screen, TrackInfo},
    },
    sender::{
        LinkPhase, NowCard, Pairing, Problem, RemoteCard, TvLink, TvRow, TvRowKind, TvSender,
    },
};
use tokio::{runtime::Handle, sync::watch};

use crate::{FfiEngine, remote::FfiScreen, remote::FfiTxtPair, start::FfiError};

/// Where the phone's key is kept: under this id, in the host's keychain. Not the TV's
/// `remote-identity`, so one install that is both keeps them apart.
const IDENTITY_SECRET: &str = "remote-sender-identity";

/// The platform's DNS-SD browse for `_anistream._tcp`, implemented by the app.
///
/// `start` begins browsing and reports to `sink` until `stop`. Called on the engine's threads.
#[uniffi::export(with_foreign)]
pub trait FfiTvBrowser: Send + Sync {
    fn start(&self, sink: Arc<FfiDiscoverySink>);
    fn stop(&self);
}

/// Where the app's browse reports what it finds.
#[derive(uniffi::Object)]
pub struct FfiDiscoverySink {
    sink: DiscoverySink,
}

#[uniffi::export]
impl FfiDiscoverySink {
    /// A TV was found, or found again with new details: its instance name, the IP addresses it
    /// resolved to, its port and its TXT record. Link-local IPv6 addresses are skipped.
    pub fn found(
        &self,
        instance: String,
        addresses: Vec<String>,
        port: u16,
        txt: Vec<FfiTxtPair>,
    ) {
        let mut ips: Vec<IpAddr> = addresses
            .iter()
            .filter_map(|text| text.split('%').next()?.parse().ok())
            .filter(|ip: &IpAddr| match ip {
                IpAddr::V4(_) => true,
                IpAddr::V6(v6) => !v6.is_unicast_link_local(),
            })
            .collect();
        ips.sort_by_key(|ip| u8::from(ip.is_ipv6()));
        ips.dedup();
        let addresses = ips.into_iter().map(|ip| SocketAddr::new(ip, port)).collect();
        let txt: Vec<(String, String)> = txt.into_iter().map(|p| (p.key, p.value)).collect();
        self.sink.found(FoundTv::from_txt(&instance, addresses, &txt));
    }

    pub fn lost(&self, instance: String) {
        self.sink.lost(&instance);
    }
}

/// The app's browser, as the engine's.
struct AppBrowser(Arc<dyn FfiTvBrowser>);

impl Browser for AppBrowser {
    fn start(&self, sink: DiscoverySink) {
        self.0.start(Arc::new(FfiDiscoverySink { sink }));
    }

    fn stop(&self) {
        self.0.stop();
    }
}

/// What went wrong, as [`FfiTvError`] names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTvProblem {
    WrongCode,
    NoCode,
    Full,
    Busy,
    Version,
    NotNow,
    Unpaired,
    KeyChanged,
    Unreachable,
    Gone,
    NotALink,
    Other,
}

/// A remote call failed, with the words to show for it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum FfiTvError {
    #[error("{message}")]
    Failed { problem: FfiTvProblem, message: String },
}

impl From<Problem> for FfiTvError {
    fn from(problem: Problem) -> Self {
        let kind = match problem {
            Problem::WrongCode => FfiTvProblem::WrongCode,
            Problem::NoCode => FfiTvProblem::NoCode,
            Problem::Full => FfiTvProblem::Full,
            Problem::Busy => FfiTvProblem::Busy,
            Problem::Version => FfiTvProblem::Version,
            Problem::NotNow => FfiTvProblem::NotNow,
            Problem::Unpaired => FfiTvProblem::Unpaired,
            Problem::KeyChanged => FfiTvProblem::KeyChanged,
            Problem::Unreachable => FfiTvProblem::Unreachable,
            Problem::Gone => FfiTvProblem::Gone,
            Problem::NotALink => FfiTvProblem::NotALink,
            Problem::Other => FfiTvProblem::Other,
        };
        Self::Failed { problem: kind, message: problem.message().to_owned() }
    }
}

/// A row of the TV picker.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiTvRow {
    /// What `pair` and, for a paired TV, `link_row` take.
    pub key: String,
    pub name: String,
    /// The TV's fingerprint, when paired: what `link` and `forget` take.
    pub fingerprint: Option<String>,
    /// "192.168.1.20:47811".
    pub address: Option<String>,
    pub kind: FfiTvRowKind,
    /// What the row says under the name.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTvRowKind {
    Paired,
    PairedAway,
    New,
    NeedsUpdate,
}

impl From<TvRow> for FfiTvRow {
    fn from(row: TvRow) -> Self {
        Self {
            key: row.key,
            name: row.name,
            fingerprint: row.fingerprint.map(|f| f.to_hex()),
            address: row.address.map(|a| a.to_string()),
            detail: row.kind.detail().to_owned(),
            kind: match row.kind {
                TvRowKind::Paired => FfiTvRowKind::Paired,
                TvRowKind::PairedAway => FfiTvRowKind::PairedAway,
                TvRowKind::New => FfiTvRowKind::New,
                TvRowKind::NeedsUpdate => FfiTvRowKind::NeedsUpdate,
            },
        }
    }
}

/// A TV this device paired with, for Settings.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPairedTv {
    pub fingerprint: String,
    /// The first 16 hex digits, which the TV's own settings show for it.
    pub short_fingerprint: String,
    pub name: String,
    pub address: String,
    /// Unix seconds.
    pub paired_at: i64,
    pub last_seen: Option<i64>,
}

impl From<KnownTv> for FfiPairedTv {
    fn from(tv: KnownTv) -> Self {
        Self {
            fingerprint: tv.fingerprint.to_hex(),
            short_fingerprint: tv.fingerprint.short(),
            name: tv.name,
            address: tv.address.to_string(),
            paired_at: tv.paired_at,
            last_seen: tv.last_seen,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLinkPhase {
    Connecting,
    Connected,
    Retrying,
    Unpaired,
    KeyChanged,
}

/// The now-playing card.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiRemoteCard {
    pub tv_name: String,
    pub phase: FfiLinkPhase,
    /// What the card says about the link when it is not simply connected.
    pub phase_line: Option<String>,
    pub screen: FfiScreen,
    pub now: Option<FfiNowCard>,
    /// Counts up with every state the TV sends.
    pub rev: u64,
    /// The session is over: the TV forgot this device, has played nothing for a while after
    /// playing something, or has been out of reach for minutes. A remote kept in the
    /// background (a notification, a Live Activity) lets the link go.
    #[uniffi(default)]
    pub over: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiNowCard {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    /// "Episode 3 · Low Tide".
    pub episode_line: String,
    /// The TV is still finding the stream.
    pub loading: bool,
    /// Where the TV's playhead was when the card was read: see `tv_playhead`.
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    /// The TV's own volume, out of 100, once it said.
    pub volume: Option<f64>,
    pub audio: Vec<FfiTvTrack>,
    pub subtitles: Vec<FfiTvTrack>,
    pub subtitles_off: bool,
    pub skip: Option<FfiSkipCard>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiTvTrack {
    pub id: i64,
    /// The language code the file names, for the app to name in the viewer's language.
    pub language: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiSkipCard {
    pub to: f64,
    pub label: String,
}

fn screen(screen: &Screen) -> FfiScreen {
    match screen {
        Screen::Home => FfiScreen::Home,
        Screen::Search { query } => FfiScreen::Search { query: query.clone() },
        Screen::Title { id } => FfiScreen::Title { id: *id },
        Screen::Player => FfiScreen::Player,
        Screen::Settings => FfiScreen::Settings,
        Screen::Other => FfiScreen::Other,
    }
}

fn track(track: &TrackInfo) -> FfiTvTrack {
    FfiTvTrack {
        id: track.id,
        language: track.language.clone(),
        title: track.title.clone(),
        selected: track.selected,
    }
}

impl From<&NowCard> for FfiNowCard {
    fn from(now: &NowCard) -> Self {
        Self {
            id: now.id,
            title: now.title.clone(),
            episode: now.episode.clone(),
            episode_line: now.episode_line.clone(),
            loading: now.loading,
            position: now.position,
            duration: now.duration,
            paused: now.paused,
            speed: now.speed,
            volume: now.volume,
            audio: now.audio.iter().map(track).collect(),
            subtitles: now.subtitles.iter().map(track).collect(),
            subtitles_off: now.subtitles_off,
            skip: now.skip.as_ref().map(|s| FfiSkipCard { to: s.to, label: s.label.clone() }),
        }
    }
}

impl From<&RemoteCard> for FfiRemoteCard {
    fn from(card: &RemoteCard) -> Self {
        Self {
            tv_name: card.tv_name.clone(),
            phase: match card.phase {
                LinkPhase::Connecting => FfiLinkPhase::Connecting,
                LinkPhase::Connected => FfiLinkPhase::Connected,
                LinkPhase::Retrying => FfiLinkPhase::Retrying,
                LinkPhase::Unpaired => FfiLinkPhase::Unpaired,
                LinkPhase::KeyChanged => FfiLinkPhase::KeyChanged,
            },
            phase_line: card.phase_line.clone(),
            screen: screen(&card.screen),
            now: card.now.as_ref().map(FfiNowCard::from),
            rev: card.rev,
            over: card.over,
        }
    }
}

/// Where the TV's playhead is `elapsed` seconds after `now` was read: moved on this device's
/// clock while the TV plays, since the TV says it only when it jumps.
#[uniffi::export]
pub fn tv_playhead(now: FfiNowCard, elapsed: f64) -> f64 {
    if now.paused || now.loading {
        return now.position;
    }
    let moved = now.position + elapsed.max(0.0) * now.speed;
    now.duration.map_or(moved, |duration| moved.min(duration))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiTvTrackKind {
    Audio,
    Subtitle,
}

/// What the remote card sends. Play on TV is `FfiTvLink::play`.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiTvCommand {
    TogglePause,
    /// By seconds; negative goes back.
    Seek {
        by: f64,
    },
    SeekTo {
        position: f64,
    },
    Next,
    Previous,
    /// The skip on offer.
    Skip,
    Stop,
    /// A step of the TV's own volume, out of 100.
    Volume {
        step: f64,
    },
    Speed {
        speed: f64,
    },
    /// A track by its id, or no id to turn subtitles off.
    Track {
        kind: FfiTvTrackKind,
        id: Option<i64>,
    },
    /// The whole text of the TV's search field, and whether to search now.
    Text {
        text: String,
        submit: bool,
    },
    /// Open a title's page on the TV.
    Open {
        id: AnilistId,
    },
}

impl From<FfiTvCommand> for RemoteCommand {
    fn from(command: FfiTvCommand) -> Self {
        match command {
            FfiTvCommand::TogglePause => Self::TogglePause,
            FfiTvCommand::Seek { by } => Self::Seek { by },
            FfiTvCommand::SeekTo { position } => Self::SeekTo { position },
            FfiTvCommand::Next => Self::Next,
            FfiTvCommand::Previous => Self::Previous,
            FfiTvCommand::Skip => Self::Skip,
            FfiTvCommand::Stop => Self::Stop,
            FfiTvCommand::Volume { step } => Self::Volume { step },
            FfiTvCommand::Speed { speed } => Self::Speed { speed },
            FfiTvCommand::Track { kind, id } => Self::Track {
                kind: match kind {
                    FfiTvTrackKind::Audio => Track::Audio,
                    FfiTvTrackKind::Subtitle => Track::Subtitle,
                },
                id,
            },
            FfiTvCommand::Text { text, submit } => Self::Text { text, submit },
            FfiTvCommand::Open { id } => Self::Open { id },
        }
    }
}

#[uniffi::export]
impl FfiEngine {
    /// The remote's sender side, for Play on TV: `device_name` is what TVs list this device as,
    /// and `app` the app and its version, `anistream-ios/2.1.0`. Its key comes from the host's
    /// keychain, made the first time, and its paired TVs from the data directory. One per app.
    pub fn tv_sender(
        &self,
        device_name: String,
        app: String,
    ) -> Result<Arc<FfiTvSender>, FfiError> {
        let fail = |reason: String| FfiError::Remote { reason };
        let home = self.remote().home();
        let identity = identity_from(home.secrets.as_ref(), IDENTITY_SECRET)
            .map_err(|e| fail(e.to_string()))?;
        let dir = home.data.join("remote");
        std::fs::create_dir_all(&dir).map_err(|e| fail(format!("{}: {e}", dir.display())))?;
        let known = KnownTvs::open(&dir).map_err(|e| fail(e.to_string()))?;
        let sender = TvSender::new(identity, Hello { name: device_name, app }, known);
        let changes = sender.changes();
        Ok(Arc::new(FfiTvSender {
            sender: Arc::new(sender),
            handle: self.runtime_handle(),
            changes: tokio::sync::Mutex::new(changes),
        }))
    }
}

/// The remote's sender side.
#[derive(uniffi::Object)]
pub struct FfiTvSender {
    sender: Arc<TvSender>,
    handle: Handle,
    changes: tokio::sync::Mutex<watch::Receiver<u64>>,
}

fn tv_error(problem: Problem) -> FfiTvError {
    problem.into()
}

fn stopped() -> FfiTvError {
    Problem::Other.into()
}

#[uniffi::export]
impl FfiTvSender {
    /// Browse with the app's browser while the TV picker is open.
    pub fn browse(&self, browser: Arc<dyn FfiTvBrowser>) {
        let _entered = self.handle.enter();
        self.sender.browse(Arc::new(AppBrowser(browser)));
    }

    /// Stop browsing, as the picker closes.
    pub fn stop_browsing(&self) {
        let _entered = self.handle.enter();
        self.sender.stop_browsing();
    }

    /// The picker's rows, in the order to show them.
    pub fn rows(&self) -> Vec<FfiTvRow> {
        self.sender.rows().into_iter().map(FfiTvRow::from).collect()
    }

    /// The paired TVs, the most recently paired first.
    pub fn paired(&self) -> Vec<FfiPairedTv> {
        self.sender.paired().into_iter().map(FfiPairedTv::from).collect()
    }

    /// This device's short fingerprint, as a TV's settings list it.
    pub fn short_fingerprint(&self) -> String {
        self.sender.short_fingerprint()
    }

    /// Wait until the rows or the paired TVs may read differently. Cancel-safe.
    pub async fn changed(&self) {
        let mut changes = self.changes.lock().await;
        let _ = changes.changed().await;
    }

    /// Start pairing with a picker row's `key`, a scanned link or an address typed by hand.
    pub async fn pair(&self, target: String) -> Result<Arc<FfiTvPairing>, FfiTvError> {
        let sender = Arc::clone(&self.sender);
        let pairing = self
            .handle
            .spawn(async move { sender.pair(&target).await })
            .await
            .map_err(|_| stopped())?
            .map_err(tv_error)?;
        Ok(Arc::new(FfiTvPairing {
            tv_name: pairing.tv_name().to_owned(),
            needs_code: pairing.needs_code(),
            pairing: Arc::new(tokio::sync::Mutex::new(pairing)),
            handle: self.handle.clone(),
        }))
    }

    /// Connect to a paired TV, by its fingerprint, for as long as the link is held: until
    /// `close`, or the last reference goes. `None` when it is not paired.
    pub fn link(&self, fingerprint: String) -> Option<Arc<FfiTvLink>> {
        let fingerprint = Fingerprint::from_hex(&fingerprint)?;
        let _entered = self.handle.enter();
        let link = self.sender.link(&fingerprint)?;
        let card = link.card();
        Some(Arc::new(FfiTvLink {
            link: Mutex::new(Some(Arc::new(link))),
            latest: card.clone(),
            card: tokio::sync::Mutex::new(card),
            handle: self.handle.clone(),
        }))
    }

    /// Forget a paired TV, telling it to forget this device when it answers. True when it was
    /// paired.
    pub async fn forget(&self, fingerprint: String) -> bool {
        let Some(fingerprint) = Fingerprint::from_hex(&fingerprint) else { return false };
        let sender = Arc::clone(&self.sender);
        self.handle
            .spawn(async move { sender.forget(&fingerprint).await.unwrap_or(false) })
            .await
            .unwrap_or(false)
    }
}

/// A connection made to pair with a TV.
#[derive(uniffi::Object)]
pub struct FfiTvPairing {
    tv_name: String,
    needs_code: bool,
    pairing: Arc<tokio::sync::Mutex<Pairing>>,
    handle: Handle,
}

#[uniffi::export]
impl FfiTvPairing {
    pub fn tv_name(&self) -> String {
        self.tv_name.clone()
    }

    /// Whether the viewer types a code: not for a scanned link that carried one.
    pub fn needs_code(&self) -> bool {
        self.needs_code
    }

    /// Ask the TV to show a code for this device.
    pub async fn request_code(&self) -> Result<(), FfiTvError> {
        let pairing = Arc::clone(&self.pairing);
        self.handle
            .spawn(async move { pairing.lock().await.request_code().await })
            .await
            .map_err(|_| stopped())?
            .map_err(tv_error)
    }

    /// Pair with the code typed, or the one a scanned link carried, and keep the TV. A wrong
    /// code can be typed again on the same pairing.
    pub async fn pair(&self, code: Option<String>) -> Result<FfiPairedTv, FfiTvError> {
        let pairing = Arc::clone(&self.pairing);
        self.handle
            .spawn(async move { pairing.lock().await.pair(code).await })
            .await
            .map_err(|_| stopped())?
            .map(FfiPairedTv::from)
            .map_err(tv_error)
    }
}

/// A paired TV, connected while the remote card is open.
#[derive(uniffi::Object)]
pub struct FfiTvLink {
    link: Mutex<Option<Arc<TvLink>>>,
    /// For `card`, which reads without waiting.
    latest: watch::Receiver<RemoteCard>,
    card: tokio::sync::Mutex<watch::Receiver<RemoteCard>>,
    handle: Handle,
}

impl FfiTvLink {
    fn held(&self) -> Result<Arc<TvLink>, FfiTvError> {
        self.link
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| Problem::Unreachable.into())
    }
}

#[uniffi::export]
impl FfiTvLink {
    /// The card as it stands.
    pub fn card(&self) -> FfiRemoteCard {
        FfiRemoteCard::from(&*self.latest.borrow())
    }

    /// The card once it changes. `None` once the link is closed. Cancel-safe.
    pub async fn next_card(&self) -> Option<FfiRemoteCard> {
        let mut card = self.card.lock().await;
        card.changed().await.ok()?;
        Some(FfiRemoteCard::from(&*card.borrow_and_update()))
    }

    pub async fn send(&self, command: FfiTvCommand) -> Result<(), FfiTvError> {
        let link = self.held()?;
        self.handle
            .spawn(async move { link.send(command.into()).await })
            .await
            .map_err(|_| stopped())?
            .map_err(tv_error)
    }

    /// Play on TV: the episode, what this device calls the title (the card shows it by that
    /// name), and the position this device was at in it, carried over once the TV has started
    /// it.
    pub async fn play(
        &self,
        id: AnilistId,
        title: String,
        episode: String,
        from: Option<f64>,
    ) -> Result<(), FfiTvError> {
        let link = self.held()?;
        self.handle
            .spawn(async move { link.play(id, title, episode, from).await })
            .await
            .map_err(|_| stopped())?
            .map_err(tv_error)
    }

    /// Try the TV again now rather than after the wait: when the network comes back, or the app
    /// comes to the front. A connected link stays as it is.
    pub fn retry_now(&self) {
        if let Some(link) = self.link.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            link.retry_now();
        }
    }

    /// Close the connection, as the remote card closes or the app leaves the screen.
    pub fn close(&self) {
        let link = self.link.lock().unwrap_or_else(PoisonError::into_inner).take();
        drop(link);
    }
}

#[cfg(feature = "testing")]
pub use fake::FfiFakeTv;

/// A TV on the loopback interface for the Fixtures apps and tests: a real remote server with a
/// scripted player, playing the fixture titles.
#[cfg(feature = "testing")]
mod fake {
    use anistream_engine::remote::protocol::fake::{FakeTitle, FakeTv};
    use anistream_testkit::{fixtures, sources};

    use super::*;
    use crate::remote::FfiAdvert;

    #[derive(uniffi::Object)]
    pub struct FfiFakeTv {
        tv: Mutex<Option<FakeTv>>,
        handle: Handle,
    }

    #[uniffi::export]
    impl FfiEngine {
        /// A TV named `name` on 127.0.0.1, for the Fixtures app's Play on TV: it pairs, plays
        /// the fixture titles to a script and sends state as a TV does. Testing builds only.
        pub fn start_fake_tv(&self, name: String) -> Result<Arc<FfiFakeTv>, FfiError> {
            let titles = fixtures::ALL
                .iter()
                .map(|id| FakeTitle {
                    id: *id,
                    title: fixtures::title(*id),
                    episodes: sources::episodes(*id)
                        .into_iter()
                        .map(|e| e.title.unwrap_or_default())
                        .collect(),
                    duration: fixtures::runtime_secs(*id),
                })
                .collect();
            let handle = self.runtime_handle();
            let tv = handle
                .block_on(handle.spawn(async move { FakeTv::start(&name, titles).await }))
                .map_err(|e| FfiError::Remote { reason: e.to_string() })?
                .map_err(|e| FfiError::Remote { reason: e.to_string() })?;
            Ok(Arc::new(FfiFakeTv { tv: Mutex::new(Some(tv)), handle }))
        }
    }

    impl FfiFakeTv {
        fn with<T>(&self, read: impl FnOnce(&FakeTv) -> T) -> Option<T> {
            self.tv.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(read)
        }
    }

    #[uniffi::export]
    impl FfiFakeTv {
        /// "127.0.0.1".
        pub fn host(&self) -> String {
            self.with(|tv| tv.addr().ip().to_string()).unwrap_or_default()
        }

        pub fn port(&self) -> u16 {
            self.with(|tv| tv.addr().port()).unwrap_or_default()
        }

        /// What it would register with DNS-SD.
        pub fn advert(&self) -> FfiAdvert {
            self.with(|tv| {
                let advert = tv.advert();
                FfiAdvert {
                    instance: advert.instance,
                    port: advert.port,
                    txt: advert
                        .txt
                        .into_iter()
                        .map(|(key, value)| FfiTxtPair { key, value })
                        .collect(),
                }
            })
            .unwrap_or(FfiAdvert {
                instance: String::new(),
                port: 0,
                txt: Vec::new(),
            })
        }

        /// A code opened from the TV's side, as its Pair a phone screen opens one.
        pub fn open_code(&self) -> String {
            self.with(FakeTv::open_code).unwrap_or_default()
        }

        /// The code it shows for a device that asked.
        pub fn shown_code(&self) -> Option<String> {
            self.with(FakeTv::shown_code).flatten()
        }

        /// The link its QR code carries.
        pub fn pair_link(&self, code: Option<String>) -> String {
            self.with(|tv| tv.pair_link(code)).unwrap_or_default()
        }

        /// What it was sent, oldest first.
        pub fn commands(&self) -> Vec<String> {
            self.with(FakeTv::commands).unwrap_or_default()
        }

        pub fn volume(&self) -> f64 {
            self.with(FakeTv::volume).unwrap_or_default()
        }

        /// Forget every device, as the TV's settings would.
        pub fn revoke_all(&self) {
            self.with(FakeTv::revoke_all);
        }

        pub fn stop(&self) {
            let tv = self.tv.lock().unwrap_or_else(PoisonError::into_inner).take();
            let _entered = self.handle.enter();
            drop(tv);
        }
    }
}

impl Drop for FfiTvLink {
    fn drop(&mut self) {
        let link = self.link.lock().unwrap_or_else(PoisonError::into_inner).take();
        let _entered = self.handle.enter();
        drop(link);
    }
}

/// Make or read this install's key, kept under `id`.
pub(crate) fn identity_from(
    secrets: Option<&Arc<dyn anistream_engine::host::SecretStore>>,
    id: &str,
) -> Result<Identity, anistream_engine::remote::protocol::Error> {
    let Some(secrets) = secrets else { return Identity::generate() };
    if let Ok(Some(kept)) = secrets.get(id)
        && let Some(identity) =
            hex_decode(&kept).and_then(|der| Identity::from_pkcs8(&der).ok())
    {
        return Ok(identity);
    }
    let identity = Identity::generate()?;
    let _ = secrets.set(id, &hex_encode(&identity.to_pkcs8()));
    Ok(identity)
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}
