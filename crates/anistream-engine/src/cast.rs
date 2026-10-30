//! Casting to TVs with Chromecast built in: the devices the TV picker's browse found, what the
//! engine has learned of each TV, and the cast running now.
//!
//! A cast is a playback like any other, run by the engine's session here: the streams are found
//! on this device, history and skipping and auto-next work as they do in a window, and the
//! player the session drives is the TV, through [`CastLink`]. [`CastHub`] is what the parts
//! share: the sender's picker reads its devices, [`Command::CastTo`] opens a cast through it,
//! and the card reads the cast's status and the engine's events off it.
//!
//! [`CastLink`]: crate::playback::link::cast::CastLink
//! [`Command::CastTo`]: anistream_model::command::Command::CastTo

use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::{
        Arc, Mutex, OnceLock, PoisonError, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

pub use anistream_cast as protocol;
use anistream_cast::{
    AacEncoder, CastDevice, CastFailure, CastMemory, CastPlayer, CastState, CastStatus,
    NoEncoder, PlainHttp, Timings, Upstream, UpstreamResponse, formats::is_loopback_url,
};
use anistream_core::ids::AnilistId;
use anistream_model::{event::Event, playback::SkipKind};
use anistream_playback::{PlaybackEvent, ScriptEvent};
use anistream_net::HttpClient;
use anistream_remote::state::{RemoteState, StateFolder};
use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::watch;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the parts of casting share. One per engine.
pub struct CastHub {
    /// Where `cast-tvs.json` is kept; `None` keeps it in memory, for tests.
    dir: Option<PathBuf>,
    memory: OnceLock<Arc<CastMemory>>,
    upstream: Arc<dyn Upstream>,
    encoder: Mutex<Arc<dyn AacEncoder>>,
    timings: Timings,
    /// What the browse found, by TXT `id`.
    devices: Mutex<BTreeMap<String, CastDevice>>,
    cast: Mutex<Current>,
    /// Counts up whenever the devices or the cast read differently.
    changes: watch::Sender<u64>,
    /// Whether the TV picker lists TVs with Chromecast built in: Settings' row.
    show_tvs: AtomicBool,
    /// The language the front end speaks, for the TV's episode lines.
    language: Mutex<Option<String>>,
}

/// How long Up Next counts down without an ending to start it, when the setting says none:
/// the hosted players' own.
const UP_NEXT_COUNTDOWN: u32 = 10;
/// Without an ending, a file longer than two minutes shows Up Next this long before its end.
const UP_NEXT_WITHOUT_ENDING: f64 = 30.0;
/// An automatic skip's Undo stays for this much playback.
const UNDO_PLAYBACK: f64 = 6.0;
/// And for this long at most, however little played: a paused cast lets it go too.
const UNDO_LONGEST: Duration = Duration::from_secs(15);

/// What plays next, as the session said it, and how far the card's countdown has gone.
#[derive(Debug, Clone, PartialEq)]
pub struct UpNext {
    pub number: String,
    pub title: Option<String>,
    /// The episode's still, by URL.
    pub still: Option<String>,
    /// The episode rolls on by itself when the count ends.
    pub auto: bool,
    /// Where the card shows: the ending's start, when known.
    at: Option<f64>,
    countdown: u32,
    /// The viewer said no, for this episode.
    cancelled: bool,
    /// Play next was asked for, by the count or the viewer.
    played: bool,
    /// Where the count runs from since a seek landed in it.
    restarted_at: Option<f64>,
}

impl UpNext {
    /// Where the card shows and its count starts: the ending, or near the end of the file.
    fn shows_at(&self, duration: Option<f64>) -> Option<f64> {
        if let Some(at) = self.at {
            return Some(at);
        }
        let duration = duration.filter(|d| *d > 0.0)?;
        Some(if duration > 120.0 {
            duration - UP_NEXT_WITHOUT_ENDING
        } else {
            (duration - f64::from(self.countdown())).max(0.0)
        })
    }

    fn countdown(&self) -> u32 {
        if self.countdown > 0 { self.countdown } else { UP_NEXT_COUNTDOWN }
    }

    /// Whether the card is up at `position`.
    pub fn shown(&self, position: f64, duration: Option<f64>) -> bool {
        !self.cancelled && !self.played && self.shows_at(duration).is_some_and(|at| position >= at)
    }

    /// Seconds left on the count at `position`, while it runs: never with no countdown, where
    /// the card waits for the viewer.
    pub fn remaining(&self, position: f64, duration: Option<f64>) -> Option<f64> {
        if !self.auto || self.cancelled || self.countdown == 0 {
            return None;
        }
        let shows = self.shows_at(duration)?;
        let start = self.restarted_at.map_or(shows, |r| r.max(shows));
        (position >= start).then(|| (f64::from(self.countdown) - (position - start)).max(0.0))
    }
}

/// The note that an automatic skip can be undone.
#[derive(Debug, Clone, PartialEq)]
pub struct Undo {
    pub kind: SkipKind,
    /// Where Undo seeks back to.
    pub to: f64,
    /// Where playback was when the note was first seen with a position after the skip.
    landed: Option<f64>,
    since: Instant,
}

impl Undo {
    /// When the skip was made.
    pub fn since(&self) -> Instant {
        self.since
    }
}

/// The cast running now, and what the card reads of it.
#[derive(Default)]
struct Current {
    /// The TV the viewer cast to, from the moment they asked.
    device: Option<CastDevice>,
    player: Weak<CastPlayer>,
    status: Option<CastStatus>,
    folder: StateFolder,
    state: Option<RemoteState>,
    /// The viewer stopped casting, or the cast ended for good.
    over: bool,
    /// When the episode now starting was asked for: the tap, or the end of the one before.
    /// `None` once it plays.
    starting: Option<Instant>,
    /// A load began since `starting`, so a status that plays is this episode's.
    loading: bool,
    next: Option<UpNext>,
    /// The skip on offer: what it skips and where to.
    skip: Option<(SkipKind, f64)>,
    undo: Option<Undo>,
    /// The title and episode the cast was last asked to play, for Try again.
    asked: Option<(AnilistId, String)>,
}

/// The cast as the card reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CastView {
    pub device: Option<CastDevice>,
    /// A player is connected to the TV.
    pub connected: bool,
    pub status: Option<CastStatus>,
    /// What the engine's events say of the playback.
    pub state: Option<RemoteState>,
    pub over: bool,
    /// When the episode now starting was asked for, until it plays.
    pub starting: Option<Instant>,
    pub next: Option<UpNext>,
    pub skip: Option<(SkipKind, f64)>,
    pub undo: Option<Undo>,
    /// The title and episode the cast was last asked to play.
    pub asked: Option<(AnilistId, String)>,
}

impl CastHub {
    /// The hub for an engine whose data directory holds the remote's files in `dir`, fetching
    /// what a TV can't with `http`.
    pub fn new(dir: PathBuf, http: HttpClient) -> Self {
        Self::with(Some(dir), Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub that keeps what it learns of TVs in memory alone: what a test's engine has.
    pub fn in_memory(http: HttpClient) -> Self {
        Self::with(None, Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub with its parts given: what tests stand casting up with.
    pub fn with(dir: Option<PathBuf>, upstream: Arc<dyn Upstream>, timings: Timings) -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            dir,
            memory: OnceLock::new(),
            upstream,
            encoder: Mutex::new(Arc::new(NoEncoder)),
            timings,
            devices: Mutex::new(BTreeMap::new()),
            cast: Mutex::new(Current::default()),
            changes,
            show_tvs: AtomicBool::new(true),
            language: Mutex::new(None),
        }
    }

    /// Whether the TV picker lists TVs with Chromecast built in, from Settings.
    pub fn set_show_tvs(&self, show: bool) {
        if self.show_tvs.swap(show, Ordering::Relaxed) != show {
            if !show {
                lock(&self.devices).clear();
            }
            self.bump();
        }
    }

    pub fn shows_tvs(&self) -> bool {
        self.show_tvs.load(Ordering::Relaxed)
    }

    /// The language the front end speaks, as a tag, for what the TV shows.
    pub fn set_language(&self, language: Option<String>) {
        *lock(&self.language) = language;
    }

    pub fn language(&self) -> Option<String> {
        lock(&self.language).clone()
    }

    /// What the engine has learned of each TV, read the first time it is needed.
    pub fn memory(&self) -> Arc<CastMemory> {
        Arc::clone(self.memory.get_or_init(|| {
            let Some(dir) = &self.dir else { return Arc::new(CastMemory::memory()) };
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!(error = %e, "couldn't make the remote's directory");
            }
            Arc::new(CastMemory::open(dir).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "couldn't read what was learned of TVs");
                CastMemory::memory()
            }))
        }))
    }

    pub fn upstream(&self) -> Arc<dyn Upstream> {
        Arc::clone(&self.upstream)
    }

    pub fn timings(&self) -> Timings {
        self.timings
    }

    /// The front end's AAC encoder, which turns AC-3, E-AC-3 and other audio a TV can't play
    /// into AAC. Until one is set, such audio is uncastable.
    pub fn set_encoder(&self, encoder: Arc<dyn AacEncoder>) {
        *lock(&self.encoder) = encoder;
    }

    pub fn encoder(&self) -> Arc<dyn AacEncoder> {
        Arc::clone(&lock(&self.encoder))
    }

    /// A device the browse found, from its TXT record. Speaker groups and receivers without a
    /// screen are left out.
    pub fn found(&self, device: CastDevice) {
        if !device.shown() || !self.shows_tvs() {
            return;
        }
        lock(&self.devices).insert(device.id.clone(), device);
        self.bump();
    }

    /// The browse lost the device with this service instance name.
    pub fn lost(&self, instance: &str) {
        let removed = {
            let mut devices = lock(&self.devices);
            let before = devices.len();
            devices.retain(|_, d| d.instance != instance);
            before != devices.len()
        };
        if removed {
            self.bump();
        }
    }

    /// Forget what the browse found, as the picker closes.
    pub fn forget_found(&self) {
        let had = !std::mem::take(&mut *lock(&self.devices)).is_empty();
        if had {
            self.bump();
        }
    }

    /// The devices found, by name.
    pub fn devices(&self) -> Vec<CastDevice> {
        let mut devices: Vec<_> = lock(&self.devices).values().cloned().collect();
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        devices
    }

    /// The device found with this TXT `id`, or the one casting now.
    pub fn device(&self, id: &str) -> Option<CastDevice> {
        lock(&self.devices)
            .get(id)
            .cloned()
            .or_else(|| lock(&self.cast).device.clone().filter(|d| d.id == id))
    }

    /// The viewer cast to `device`: the card is for it from now.
    pub fn begin(&self, device: CastDevice) {
        *lock(&self.cast) = Current {
            device: Some(device),
            starting: Some(Instant::now()),
            ..Current::default()
        };
        self.bump();
    }

    /// The episode the cast was asked to play, for Try again to ask for it once more.
    pub fn asked(&self, id: AnilistId, episode: String) {
        lock(&self.cast).asked = Some((id, episode));
    }

    /// The viewer stopped casting, or it ended for good.
    pub fn end(&self) {
        {
            let mut cast = lock(&self.cast);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// The cast couldn't reach the TV: the card says why, and casting ends, so the next play
    /// opens on this device.
    pub(crate) fn failed(&self, failure: CastFailure) {
        {
            let mut cast = lock(&self.cast);
            let mut status = cast.status.clone().unwrap_or_default();
            status.state = CastState::Ended;
            status.failure = Some(failure);
            cast.status = Some(status);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// Whether a cast was asked for and has not ended.
    pub fn casting(&self) -> bool {
        let cast = lock(&self.cast);
        cast.device.is_some() && !cast.over
    }

    /// The player a cast opened, which the card's volume and mute go to.
    pub(crate) fn attach(self: &Arc<Self>, player: &Arc<CastPlayer>) {
        let mut status = player.status();
        lock(&self.cast).player = Arc::downgrade(player);
        self.set_status(status.borrow_and_update().clone());
        let hub = Arc::downgrade(self);
        tokio::spawn(async move {
            while status.changed().await.is_ok() {
                let Some(hub) = hub.upgrade() else { return };
                hub.set_status(status.borrow_and_update().clone());
            }
        });
    }

    pub(crate) fn set_status(&self, status: CastStatus) {
        let play_next = {
            let mut cast = lock(&self.cast);
            if status.step.is_some() {
                cast.loading = true;
            }
            let playing =
                matches!(status.state, CastState::Playing | CastState::Paused | CastState::Buffering);
            if cast.loading && playing && status.plan.is_some() && status.step.is_none() {
                cast.starting = None;
                cast.loading = false;
            }
            let (position, duration) = (status.position, status.duration);
            if let Some(undo) = cast.undo.as_mut().filter(|_| playing) {
                let landed = *undo.landed.get_or_insert(position);
                if position - landed >= UNDO_PLAYBACK
                    || position < landed - 1.0
                    || undo.since.elapsed() >= UNDO_LONGEST
                {
                    cast.undo = None;
                }
            }
            // The count ran out: the TV plays the next episode, as a hosted player asks.
            let ran_out = status.state == CastState::Playing
                && cast.next.as_ref().is_some_and(|next| {
                    !next.played && next.remaining(position, duration).is_some_and(|r| r <= 0.0)
                });
            if ran_out && let Some(next) = cast.next.as_mut() {
                next.played = true;
            }
            cast.status = Some(status);
            ran_out.then(|| cast.player.upgrade()).flatten()
        };
        if let Some(player) = play_next {
            tracing::info!("Up Next's count ended on the cast; playing the next episode");
            player.say(PlaybackEvent::Script(ScriptEvent::PlayNext));
        }
        self.bump();
    }

    /// What the session shows a player: the cast keeps what the sender's card draws, Up Next,
    /// the skip on offer and the Undo note, and that the next episode is starting.
    pub(crate) fn overlay(&self, overlay: &crate::playback::link::Overlay) {
        use crate::playback::{link::Overlay, offered};
        {
            let mut cast = lock(&self.cast);
            match overlay {
                Overlay::Next { card: Some(card), still, countdown } => {
                    // The same episode keeps what the viewer did to its card.
                    let kept = cast.next.take().filter(|next| next.number == card.number);
                    cast.next = Some(UpNext {
                        number: card.number.clone(),
                        title: card.title.clone(),
                        still: still.clone(),
                        auto: card.auto,
                        at: card.at,
                        countdown: *countdown,
                        cancelled: kept.as_ref().is_some_and(|k| k.cancelled),
                        played: kept.as_ref().is_some_and(|k| k.played),
                        restarted_at: kept.and_then(|k| k.restarted_at),
                    });
                }
                Overlay::Next { card: None, .. } => cast.next = None,
                Overlay::SkipOffer { kind, to } => cast.skip = Some((offered(*kind), *to)),
                Overlay::SkipCleared => cast.skip = None,
                Overlay::Skipped { kind, from, .. } => {
                    cast.undo = Some(Undo {
                        kind: offered(*kind),
                        to: *from,
                        landed: None,
                        since: Instant::now(),
                    });
                }
                // A file is starting on the TV: the first, or the next after one ended.
                Overlay::Starting { id, episode, .. } => {
                    cast.asked = Some((*id, episode.clone()));
                    if cast.starting.is_none() {
                        cast.starting = Some(Instant::now());
                        cast.loading = false;
                    }
                    cast.skip = None;
                    cast.undo = None;
                }
                _ => return,
            }
        }
        self.bump();
    }

    /// The TV was asked to seek to `target`: an Up Next count it lands in runs again from its
    /// full length there.
    pub(crate) fn seeked(&self, target: f64) {
        {
            let mut cast = lock(&self.cast);
            let duration = cast.status.as_ref().and_then(|s| s.duration);
            let Some(next) = cast.next.as_mut().filter(|n| !n.played) else { return };
            next.restarted_at =
                next.shows_at(duration).filter(|start| target >= *start).map(|_| target);
        }
        self.bump();
    }

    /// The viewer said no to the next episode, or asked for it now: the card comes down.
    pub fn next_answered(&self, play: bool) {
        {
            let mut cast = lock(&self.cast);
            let Some(next) = cast.next.as_mut() else { return };
            if play {
                next.played = true;
            } else {
                next.cancelled = true;
            }
        }
        self.bump();
    }

    /// The Undo note was taken.
    pub fn undo_taken(&self) -> Option<Undo> {
        let undo = lock(&self.cast).undo.take();
        if undo.is_some() {
            self.bump();
        }
        undo
    }

    /// The player casting now, while it is.
    pub fn player(&self) -> Option<Arc<CastPlayer>> {
        lock(&self.cast).player.upgrade()
    }

    /// An engine event on its way to the front end, for the card's title, episode and skips.
    pub fn observe(&self, event: &Event) {
        let changed = {
            let mut cast = lock(&self.cast);
            if cast.device.is_none() || !cast.folder.event(event) {
                false
            } else {
                cast.state = Some(cast.folder.publish());
                true
            }
        };
        if changed {
            self.bump();
        }
    }

    /// The cast as the card reads it now.
    pub fn view(&self) -> CastView {
        let cast = lock(&self.cast);
        let player = cast.player.upgrade();
        CastView {
            device: cast.device.clone(),
            connected: player.as_ref().is_some_and(|p| p.is_alive()),
            status: cast.status.clone(),
            state: cast.state.clone(),
            over: cast.over,
            starting: cast.starting,
            next: cast.next.clone(),
            skip: cast.skip,
            undo: cast.undo.clone(),
            asked: cast.asked.clone(),
        }
    }

    /// Counts up whenever the devices or the cast read differently.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn bump(&self) {
        self.changes.send_modify(|n| *n += 1);
    }
}

/// What the relay fetches upstream with: the emulated client for the source's own hosts, with
/// the headers they want, and plain HTTP for the mender and the torrent server on loopback.
struct HttpUpstream(HttpClient);

#[async_trait]
impl Upstream for HttpUpstream {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> io::Result<UpstreamResponse> {
        if is_loopback_url(url) {
            return PlainHttp.get(url, headers).await;
        }
        let mut request = self.0.emulated().get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = request.send().await.map_err(io::Error::other)?;
        let status = response.status().as_u16();
        // Redirects are followed: a CDN's playlist often comes from an edge host.
        let landed = Some(response.uri().to_string()).filter(|landed| landed != url);
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value.to_str().ok().map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        let body = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(io::Error::other));
        Ok(UpstreamResponse { status, headers, body: Box::pin(body), url: landed })
    }
}
