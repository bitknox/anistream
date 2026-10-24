//! The remote, as a phone, a tablet or the Mac app holds it: finding TVs, pairing with one,
//! keeping the TVs it paired with, and driving one while its remote card is open.
//!
//! The rules every app shares live here, so the apps only draw them:
//!
//! - [`TvSender::rows`]: the TV picker's list, the TVs a browse found merged with the ones paired
//!   before, in the order the picker shows them;
//! - [`RemoteCard`]: the now-playing card, read off the TV's state and the link's status;
//! - [`Problem`]: what went wrong, in the words every app shows;
//! - [`TvLink::play`]: Play on TV, carrying the phone's position over to the TV.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    str::FromStr,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use anistream_core::ids::AnilistId;
use anistream_remote::{
    Error,
    client::{Hello, Link, LinkOptions, LinkStatus, RemoteClient},
    command::{RemoteCommand, Track},
    discovery::{Browser, Discovery, DiscoverySink, FoundTv, PairLink},
    identity::{Fingerprint, Identity},
    known::{KnownTv, KnownTvs},
    state::{NowPlaying, RemoteState, Screen, SkipPart, TrackInfo},
    wire::{PairFailure, Rejection},
};
use tokio::{sync::watch, task::JoinHandle};

/// How long Play on TV waits for the TV to start the episode before it gives up carrying the
/// phone's position over.
const START_WAIT: Duration = Duration::from_secs(90);

/// A position closer to the start than this is not carried over: the TV starts from the top.
const CARRY_FROM: f64 = 5.0;

/// The phone's side of the remote.
pub struct TvSender {
    identity: Identity,
    hello: Hello,
    known: Arc<KnownTvs>,
    found: Arc<Mutex<BTreeMap<String, FoundTv>>>,
    changes: Arc<watch::Sender<u64>>,
    browsing: Mutex<Option<Browsing>>,
}

struct Browsing {
    browser: Arc<dyn Browser>,
    task: JoinHandle<()>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A row of the TV picker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TvRow {
    /// What [`TvSender::pair`] and [`TvSender::link`] take for this row.
    pub key: String,
    pub name: String,
    /// The TV's fingerprint, when this phone paired with it.
    pub fingerprint: Option<Fingerprint>,
    pub address: Option<SocketAddr>,
    pub kind: TvRowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TvRowKind {
    /// Paired, and found on the network now.
    Paired,
    /// Paired, but a browse has not found it: its last address is tried.
    PairedAway,
    /// Found, not paired yet: picking it pairs.
    New,
    /// Found, but it speaks another version of the protocol.
    NeedsUpdate,
}

impl TvRowKind {
    /// What the row says under the TV's name.
    pub fn detail(self) -> &'static str {
        match self {
            Self::Paired => "Paired",
            Self::PairedAway => "Paired, not seen on this network",
            Self::New => "Not paired yet",
            Self::NeedsUpdate => "Needs the same anistream version as this device",
        }
    }

    fn order(self) -> u8 {
        match self {
            Self::Paired => 0,
            Self::New => 1,
            Self::PairedAway => 2,
            Self::NeedsUpdate => 3,
        }
    }
}

impl TvSender {
    /// `hello` is what TVs list this device as. Known TVs are kept in `known`.
    pub fn new(identity: Identity, hello: Hello, known: KnownTvs) -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            identity,
            hello,
            known: Arc::new(known),
            found: Arc::new(Mutex::new(BTreeMap::new())),
            changes: Arc::new(changes),
            browsing: Mutex::new(None),
        }
    }

    /// This device's key fingerprint, the short form a TV's settings list.
    pub fn short_fingerprint(&self) -> String {
        self.identity.fingerprint().short()
    }

    /// Browse with the platform's browser while the picker is open. A browse already running
    /// is stopped first. Spawns on the runtime it is called from.
    pub fn browse(&self, browser: Arc<dyn Browser>) {
        self.stop_browsing();
        let (sink, mut rx) = DiscoverySink::channel();
        let found = Arc::clone(&self.found);
        let changes = Arc::clone(&self.changes);
        let task = tokio::spawn(async move {
            while let Some(discovery) = rx.recv().await {
                {
                    let mut found = lock(&found);
                    match discovery {
                        Discovery::Found(tv) => {
                            found.insert(tv.instance.clone(), tv);
                        }
                        Discovery::Lost { instance } => {
                            found.remove(&instance);
                        }
                    }
                }
                changes.send_modify(|n| *n += 1);
            }
        });
        browser.start(sink);
        *lock(&self.browsing) = Some(Browsing { browser, task });
    }

    /// Stop browsing, as the picker closes. What was found is forgotten.
    pub fn stop_browsing(&self) {
        if let Some(browsing) = lock(&self.browsing).take() {
            browsing.browser.stop();
            browsing.task.abort();
        }
        let had = !std::mem::take(&mut *lock(&self.found)).is_empty();
        if had {
            self.bump();
        }
    }

    fn bump(&self) {
        self.changes.send_modify(|n| *n += 1);
    }

    /// Counts up whenever [`Self::rows`] or [`Self::paired`] may read differently.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    /// The paired TVs, the most recently paired first: what Settings lists.
    pub fn paired(&self) -> Vec<KnownTv> {
        self.known.list()
    }

    /// The picker's list: paired TVs on the network, then new ones, then paired ones not seen,
    /// then ones that need an update, each by name.
    pub fn rows(&self) -> Vec<TvRow> {
        let found = lock(&self.found).clone();
        let mut rows = Vec::new();
        let mut seen = Vec::new();
        for tv in found.values() {
            let known = tv.id.as_deref().and_then(|id| self.known.by_short(id));
            let address = tv.addresses.first().copied();
            let kind = match (&known, tv.compatible()) {
                (_, false) => TvRowKind::NeedsUpdate,
                (Some(_), true) => TvRowKind::Paired,
                (None, true) => TvRowKind::New,
            };
            if let Some(known) = &known {
                seen.push(known.fingerprint);
            }
            rows.push(TvRow {
                key: match &known {
                    Some(known) => key_of(&known.fingerprint),
                    None => format!("{FOUND}{}", tv.instance),
                },
                name: known.as_ref().map_or_else(|| tv.instance.clone(), |k| k.name.clone()),
                fingerprint: known.as_ref().map(|k| k.fingerprint),
                address,
                kind,
            });
        }
        for tv in self.known.list() {
            if seen.contains(&tv.fingerprint) {
                continue;
            }
            rows.push(TvRow {
                key: key_of(&tv.fingerprint),
                name: tv.name,
                fingerprint: Some(tv.fingerprint),
                address: Some(tv.address),
                kind: TvRowKind::PairedAway,
            });
        }
        rows.sort_by(|a, b| {
            a.kind.order().cmp(&b.kind.order()).then_with(|| a.name.cmp(&b.name))
        });
        rows
    }

    /// Where a row, a scanned link or an address typed by hand points: the address, the TV's
    /// key when it is known, and a code when a link carried one.
    fn target(&self, target: &str) -> Result<PairLink, Problem> {
        if let Some(instance) = target.strip_prefix(FOUND) {
            let found = lock(&self.found).get(instance).cloned();
            let addr = found.and_then(|tv| tv.addresses.first().copied());
            return addr
                .map(|addr| PairLink { addr, tv: None, code: None })
                .ok_or(Problem::Gone);
        }
        if let Some(fingerprint) = target.strip_prefix(PAIRED).and_then(Fingerprint::from_hex) {
            let tv = self.known.get(&fingerprint).ok_or(Problem::Gone)?;
            return Ok(PairLink {
                addr: self.address_of(&tv),
                tv: Some(fingerprint),
                code: None,
            });
        }
        PairLink::from_str(target).map_err(|_| Problem::NotALink)
    }

    /// Where to reach a paired TV: where a browse found it now, or where it was last.
    fn address_of(&self, tv: &KnownTv) -> SocketAddr {
        let short = tv.fingerprint.short();
        lock(&self.found)
            .values()
            .find(|found| found.id.as_deref() == Some(short.as_str()))
            .and_then(|found| found.addresses.first().copied())
            .unwrap_or(tv.address)
    }

    /// Start pairing with a row of the picker, a scanned link or an address typed by hand:
    /// connect, and say whether a code is needed.
    pub async fn pair(&self, target: &str) -> Result<Pairing, Problem> {
        let link = self.target(target)?;
        let client = RemoteClient::connect(link.addr, &self.identity, link.tv, &self.hello)
            .await
            .map_err(|e| Problem::from(&e))?;
        Ok(Pairing {
            client,
            addr: link.addr,
            code: link.code,
            known: Arc::clone(&self.known),
            changes: Arc::clone(&self.changes),
        })
    }

    /// Keep a paired TV connected while its remote card is open. `None` when this phone has not
    /// paired with it.
    pub fn link(&self, tv: &Fingerprint) -> Option<TvLink> {
        let known = self.known.get(tv)?;
        let addr = self.address_of(&known);
        let link = Link::open(
            addr,
            self.identity.clone(),
            *tv,
            self.hello.clone(),
            LinkOptions::default(),
        );
        let (card, _) = watch::channel(RemoteCard::connecting(&known.name));
        let card = Arc::new(card);
        let names = Arc::new(Mutex::new(BTreeMap::new()));
        let task = tokio::spawn(follow(
            link.status(),
            link.state(),
            Follow {
                card: Arc::clone(&card),
                names: Arc::clone(&names),
                known: Arc::clone(&self.known),
                changes: Arc::clone(&self.changes),
                tv: known.clone(),
                addr,
            },
        ));
        Some(TvLink { link: Arc::new(link), card, names, task, tv: known })
    }

    /// Forget a paired TV: tell it to forget this device when it can be reached in a few
    /// seconds, and drop the pin either way. True when it was paired.
    pub async fn forget(&self, tv: &Fingerprint) -> Result<bool, Error> {
        let Some(known) = self.known.get(tv) else { return Ok(false) };
        let addr = self.address_of(&known);
        let told = async {
            let mut client =
                RemoteClient::connect(addr, &self.identity, Some(*tv), &self.hello).await?;
            if client.is_paired() {
                client.unpair().await?;
            }
            Ok::<_, Error>(())
        };
        let _ = tokio::time::timeout(Duration::from_secs(3), told).await;
        let removed = self.known.remove(tv)?;
        self.bump();
        Ok(removed)
    }
}

/// What a row's key starts with: a TV a browse found, by its instance name, or a paired one, by
/// its fingerprint.
const FOUND: &str = "found:";
const PAIRED: &str = "paired:";

fn key_of(fingerprint: &Fingerprint) -> String {
    format!("{PAIRED}{}", fingerprint.to_hex())
}

/// A connection made to pair.
pub struct Pairing {
    client: RemoteClient,
    addr: SocketAddr,
    /// The code a scanned link carried.
    code: Option<String>,
    known: Arc<KnownTvs>,
    changes: Arc<watch::Sender<u64>>,
}

impl Pairing {
    pub fn tv_name(&self) -> &str {
        self.client.tv_name()
    }

    /// Whether the viewer has to type a code: not for a scanned link that carried one, nor for a
    /// TV that still knows this device.
    pub fn needs_code(&self) -> bool {
        !self.client.is_paired() && self.code.is_none()
    }

    /// Ask the TV to show a code for this device.
    pub async fn request_code(&mut self) -> Result<(), Problem> {
        self.client.request_code().await.map_err(|e| Problem::from(&e))
    }

    /// Pair with `code`, or the one the link carried, and keep the TV. A TV that still knows
    /// this device is kept with no code.
    pub async fn pair(&mut self, code: Option<String>) -> Result<KnownTv, Problem> {
        if !self.client.is_paired() {
            let code = code.or_else(|| self.code.clone()).ok_or(Problem::NoCode)?;
            let digits: String = code.chars().filter(char::is_ascii_digit).collect();
            self.client.pair(&digits).await.map_err(|e| Problem::from(&e))?;
        }
        let now = now();
        let tv = KnownTv {
            fingerprint: self.client.tv(),
            name: self.client.tv_name().to_owned(),
            address: self.addr,
            paired_at: now,
            last_seen: Some(now),
        };
        self.known.add(tv.clone()).map_err(|e| Problem::from(&e))?;
        self.changes.send_modify(|n| *n += 1);
        Ok(tv)
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// A paired TV, connected while this lives.
pub struct TvLink {
    link: Arc<Link>,
    card: Arc<watch::Sender<RemoteCard>>,
    /// What this device calls the titles it played on the TV: the card shows a title by the
    /// name the viewer picked it by, in this device's title language, and before the TV has
    /// looked the title up itself.
    names: Arc<Mutex<BTreeMap<AnilistId, String>>>,
    task: JoinHandle<()>,
    tv: KnownTv,
}

impl Drop for TvLink {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TvLink {
    pub fn tv(&self) -> &KnownTv {
        &self.tv
    }

    /// The card, as it stands and as it changes.
    pub fn card(&self) -> watch::Receiver<RemoteCard> {
        self.card.subscribe()
    }

    pub async fn send(&self, command: RemoteCommand) -> Result<(), Problem> {
        self.link.send(command).await.map_err(|e| Problem::from(&e))
    }

    /// Play on TV: `title` is what this device calls it. With `from`, the phone's own position
    /// in that episode, the TV is moved there once it has started the episode, so a viewer who
    /// hands over mid-episode carries on.
    pub async fn play(
        &self,
        id: AnilistId,
        title: String,
        episode: String,
        from: Option<f64>,
    ) -> Result<(), Problem> {
        self.wait_connected().await?;
        if !title.is_empty() {
            lock(&self.names).insert(id, title);
        }
        self.send(RemoteCommand::Play { id, episode: episode.clone() }).await?;
        let Some(from) = from.filter(|from| *from >= CARRY_FROM) else { return Ok(()) };
        let link = Arc::clone(&self.link);
        let mut card = self.card.subscribe();
        tokio::spawn(async move {
            let started = tokio::time::timeout(START_WAIT, async {
                loop {
                    let ready = card.borrow_and_update().now.as_ref().is_some_and(|now| {
                        now.id == id && now.episode == episode && !now.loading
                    });
                    if ready {
                        return true;
                    }
                    if card.changed().await.is_err() {
                        return false;
                    }
                }
            });
            if started.await == Ok(true) {
                let _ = link.send(RemoteCommand::SeekTo { position: from }).await;
            }
        });
        Ok(())
    }

    /// Wait a little for the link to come up, so a command sent right after the card opens is
    /// not refused as not connected.
    async fn wait_connected(&self) -> Result<(), Problem> {
        let mut card = self.card.subscribe();
        let wait = async {
            loop {
                let phase = card.borrow_and_update().phase;
                match phase {
                    LinkPhase::Connected => return Ok(()),
                    LinkPhase::Unpaired => return Err(Problem::Unpaired),
                    LinkPhase::KeyChanged => return Err(Problem::KeyChanged),
                    LinkPhase::Connecting | LinkPhase::Retrying => {}
                }
                if card.changed().await.is_err() {
                    return Err(Problem::Unreachable);
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(12), wait)
            .await
            .unwrap_or(Err(Problem::Unreachable))
    }
}

/// What a link's card is kept up from, besides the link itself.
struct Follow {
    card: Arc<watch::Sender<RemoteCard>>,
    names: Arc<Mutex<BTreeMap<AnilistId, String>>>,
    known: Arc<KnownTvs>,
    changes: Arc<watch::Sender<u64>>,
    tv: KnownTv,
    addr: SocketAddr,
}

async fn follow(
    mut status: watch::Receiver<LinkStatus>,
    mut state: watch::Receiver<Option<RemoteState>>,
    follow: Follow,
) {
    let Follow { card, names, known, changes, tv, addr } = follow;
    let read = |current: &LinkStatus, tv_state: Option<&RemoteState>| {
        let mut read = RemoteCard::read(&tv.name, current, tv_state);
        if let Some(now) = &mut read.now
            && let Some(name) = lock(&names).get(&now.id)
        {
            now.title.clone_from(name);
        }
        read
    };
    let mut was_connected = false;
    loop {
        let current = status.borrow_and_update().clone();
        let connected = matches!(current, LinkStatus::Connected { .. });
        if let LinkStatus::Connected { tv_name } = &current
            && !was_connected
        {
            let _ = known.seen(&tv.fingerprint, Some(tv_name), addr, now());
            changes.send_modify(|n| *n += 1);
        }
        was_connected = connected;
        let tv_state = state.borrow_and_update().clone();
        card.send_replace(read(&current, tv_state.as_ref()));
        // The link stops by dropping both, the status last set first: read it once more.
        let open = tokio::select! {
            changed = status.changed() => changed.is_ok(),
            changed = state.changed() => changed.is_ok(),
        };
        if !open {
            let current = status.borrow().clone();
            let tv_state = state.borrow().clone();
            card.send_replace(read(&current, tv_state.as_ref()));
            return;
        }
    }
}

/// How a link stands, as the card shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkPhase {
    Connecting,
    Connected,
    /// Lost, trying again.
    Retrying,
    /// The TV forgot this device: pair again.
    Unpaired,
    /// The TV was reset: pair again.
    KeyChanged,
}

/// The now-playing card, as every app draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteCard {
    pub tv_name: String,
    pub phase: LinkPhase,
    /// What the card says about the link, when it is not simply connected.
    pub phase_line: Option<String>,
    pub screen: Screen,
    pub now: Option<NowCard>,
    pub rev: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NowCard {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    /// "Episode 3 · Low Tide", or "Episode 3".
    pub episode_line: String,
    pub loading: bool,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    pub volume: Option<f64>,
    pub audio: Vec<TrackInfo>,
    pub subtitles: Vec<TrackInfo>,
    /// No subtitle track is selected.
    pub subtitles_off: bool,
    pub skip: Option<SkipCard>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkipCard {
    pub to: f64,
    /// "Skip opening", "Skip ending".
    pub label: String,
}

impl RemoteCard {
    fn connecting(tv_name: &str) -> Self {
        Self {
            tv_name: tv_name.to_owned(),
            phase: LinkPhase::Connecting,
            phase_line: Some(format!("Connecting to {tv_name}")),
            screen: Screen::Home,
            now: None,
            rev: 0,
        }
    }

    /// The card for a TV known as `tv_name`, its link standing as `status`, with the state it
    /// last sent.
    pub fn read(tv_name: &str, status: &LinkStatus, state: Option<&RemoteState>) -> Self {
        let name = match status {
            LinkStatus::Connected { tv_name } if !tv_name.is_empty() => tv_name.as_str(),
            _ => tv_name,
        };
        let (phase, phase_line) = match status {
            LinkStatus::Connecting => {
                (LinkPhase::Connecting, Some(format!("Connecting to {name}")))
            }
            LinkStatus::Connected { .. } => (LinkPhase::Connected, None),
            LinkStatus::Retrying { .. } => {
                (LinkPhase::Retrying, Some(format!("Lost {name}. Trying again.")))
            }
            LinkStatus::Unpaired => {
                (LinkPhase::Unpaired, Some(Problem::Unpaired.message().to_owned()))
            }
            LinkStatus::KeyChanged => {
                (LinkPhase::KeyChanged, Some(Problem::KeyChanged.message().to_owned()))
            }
        };
        Self {
            tv_name: name.to_owned(),
            phase,
            phase_line,
            screen: state.map(|s| s.screen.clone()).unwrap_or_default(),
            now: state.and_then(|s| s.now_playing.as_ref()).map(NowCard::read),
            rev: state.map_or(0, |s| s.rev),
        }
    }
}

impl NowCard {
    fn read(now: &NowPlaying) -> Self {
        let of =
            |kind| now.tracks.iter().filter(|t| t.kind == kind).cloned().collect::<Vec<_>>();
        let subtitles = of(Track::Subtitle);
        Self {
            id: now.id,
            title: now.title.clone(),
            episode: now.episode.clone(),
            episode_line: match now.episode_title.as_deref().filter(|t| !t.is_empty()) {
                Some(title) => format!("Episode {} · {title}", now.episode),
                None => format!("Episode {}", now.episode),
            },
            loading: now.loading,
            position: now.position,
            duration: now.duration,
            paused: now.paused,
            speed: now.speed,
            volume: now.volume,
            audio: of(Track::Audio),
            subtitles_off: !subtitles.iter().any(|t| t.selected),
            subtitles,
            skip: now.skip.as_ref().map(|skip| SkipCard {
                to: skip.to,
                label: match skip.kind {
                    SkipPart::Opening => "Skip opening".into(),
                    SkipPart::Ending => "Skip ending".into(),
                },
            }),
        }
    }

    /// Where the TV's playhead is `elapsed` seconds after this card was read: moved on the
    /// phone's own clock while it plays, as the TV sends it only when it jumps.
    pub fn position_after(&self, elapsed: f64) -> f64 {
        if self.paused || self.loading {
            return self.position;
        }
        let moved = self.position + elapsed.max(0.0) * self.speed;
        self.duration.map_or(moved, |duration| moved.min(duration))
    }
}

/// What went wrong, as every app says it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Problem {
    WrongCode,
    NoCode,
    Full,
    Busy,
    Version,
    NotNow,
    Unpaired,
    KeyChanged,
    Unreachable,
    /// The row picked is no longer on the network.
    Gone,
    /// What was scanned or typed is not a TV's link or address.
    NotALink,
    Other,
}

impl Problem {
    pub fn message(self) -> &'static str {
        match self {
            Self::WrongCode => {
                "That code didn't match. Check the code on the TV and try again."
            }
            Self::NoCode => "The code on the TV ran out. Ask for a new one.",
            Self::Full => {
                "This TV has as many devices paired as it can keep. Remove one in the TV's \
                 Settings, under Remotes."
            }
            Self::Busy => "The TV is connected to as many devices as it can. Try again soon.",
            Self::Version => {
                "The TV runs another version of anistream. Update both to the same one."
            }
            Self::NotNow => "Nothing is playing on the TV.",
            Self::Unpaired => "The TV doesn't know this device anymore. Pair again.",
            Self::KeyChanged => "The TV was reset or reinstalled since you paired. Pair again.",
            Self::Unreachable => {
                "Can't reach the TV. Check that it's on, with anistream open, on the same network."
            }
            Self::Gone => "That TV is no longer on the network.",
            Self::NotALink => "That isn't a TV's pairing code or address.",
            Self::Other => "Something went wrong talking to the TV. Try again.",
        }
    }
}

impl From<&Error> for Problem {
    fn from(error: &Error) -> Self {
        match error {
            Error::PairFailed(PairFailure::WrongCode) => Self::WrongCode,
            Error::PairFailed(PairFailure::NoCode) => Self::NoCode,
            Error::PairFailed(PairFailure::Full) => Self::Full,
            Error::Rejected(Rejection::Busy) => Self::Busy,
            Error::Rejected(Rejection::Version) => Self::Version,
            Error::Rejected(Rejection::NotNow) => Self::NotNow,
            Error::Rejected(Rejection::NotPaired) | Error::Revoked => Self::Unpaired,
            Error::KeyChanged => Self::KeyChanged,
            Error::Io(_) | Error::Timeout | Error::Closed | Error::NotConnected => {
                Self::Unreachable
            }
            Error::Pair(_) => Self::WrongCode,
            _ => Self::Other,
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Problem {}
