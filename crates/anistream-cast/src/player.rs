//! A TV as the player a playback runs in: what the engine's `CastLink` wraps.
//!
//! The session drives every player in mpv's vocabulary. [`CastPlayer`] takes those calls and
//! turns them into Cast commands, and turns what the receiver reports into the same
//! [`PlaybackEvent`]s mpv's reports become, so resume, skips, Up Next and the history the session
//! writes work unchanged.
//!
//! A load is tried plan by plan, best first ([`crate::formats::cast_plans`]):
//! - A plan is refused when the receiver answers `LOAD_FAILED`, `LOAD_CANCELLED` or
//!   `INVALID_REQUEST`, sends `ERROR` or goes idle with `ERROR`. It stalls when its position
//!   hasn't moved [`Timings::movement`] after the load.
//! - A refusal counts against a format part only when the TV asked the relay for media, counted
//!   as the request arrives: a TV that never asked for the picture said nothing about its codec.
//! - Nothing counts while the TV waits on the source behind the relay, or after that source was
//!   slow or failed: a torrent that has only just started is not a codec the TV lacks. A stall
//!   with the source served in good time blames a copied audio codec alone, never the picture
//!   or the container.
//! - Before a file whose picture this TV hasn't played, a capability clip is loaded
//!   ([`crate::clips`]): a TV with no decoder for a file's picture can play its sound alone and
//!   report nothing wrong.
//! - A relayed plan whose bytes nobody fetched within [`Timings::fetch`] ends the cast: the TV
//!   can't reach this device (a firewall, a guest network), and no other plan would fare better.
//! - Until a plan plays, the session hears nothing; then `Loaded`, as from mpv.
//!
//! The receiver app is launched at the first load that has a plan, so a stream that can't be
//! cast never takes the TV over. The media status is asked for every [`Timings::poll`] for the
//! whole cast, since the Default Media Receiver reports on changes only. After
//! [`Timings::idle_close`] paused, the cast ends and the TV goes back to what it showed.

use std::{
    collections::HashMap,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anistream_playback::PlaybackEvent;
use serde_json::Value;
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc, watch},
    task::JoinHandle,
};

use crate::{
    aac::AacEncoder,
    channel::{CastError, Incoming, NS_CONNECTION, NS_MEDIA, NS_RECEIVER},
    client::{
        App, DEFAULT_MEDIA_RECEIVER, Device, Load, Media, MediaStatus, Metadata,
        ReceiverStatus, TextTrack, VolumeControl,
    },
    clips::{Clip, clip},
    device::{CastDevice, local_address_towards, same_network},
    fetch::{RangeFile, Upstream},
    formats::{
        AudioPlan, CastPlan, CastRoute, CastSource, Delivery, MediaInfo, SourceKind,
        Uncastable, cast_plans,
    },
    memory::CastMemory,
    probe,
    relay::{Dynamic, Made, Relay},
    remux::{AudioMode, RemuxOptions, Remuxer, Source},
    vtt,
};

/// How long the cast waits on each thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// Between status polls.
    pub poll: Duration,
    /// From an accepted load to a position that moves, or the plan is refused.
    pub movement: Duration,
    /// From the load to the TV's first fetch from the relay, or it never fetches.
    pub fetch: Duration,
    /// Paused this long, the cast ends.
    pub idle_close: Duration,
    /// A wait on the source this long, for a request of the TV's, is a slow source: what the
    /// TV does meanwhile says nothing about the format.
    pub slow_source: Duration,
    /// How long a plan may wait on a slow source before it is given up, unlearnt.
    pub patience: Duration,
    /// The TV said nothing this long, heartbeat and status polls unanswered: it is gone.
    pub silence: Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            poll: Duration::from_secs(1),
            movement: Duration::from_secs(8),
            fetch: Duration::from_secs(15),
            idle_close: Duration::from_secs(10 * 60),
            slow_source: Duration::from_secs(4),
            patience: Duration::from_secs(60),
            silence: crate::channel::SILENCE,
        }
    }
}

/// Everything a cast to one TV is set up with.
#[derive(Clone)]
pub struct CastSetup {
    pub device: CastDevice,
    pub memory: Arc<CastMemory>,
    pub upstream: Arc<dyn Upstream>,
    pub encoder: Arc<dyn AacEncoder>,
    pub timings: Timings,
}

/// A subtitle file on this device, side-loaded as a WebVTT track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastSubtitle {
    pub path: PathBuf,
    pub title: Option<String>,
    pub language: Option<String>,
}

/// One file to cast.
#[derive(Debug, Clone, PartialEq)]
pub struct CastLoad {
    /// The stream as its source gave it.
    pub source: CastSource,
    /// What a local player would open: the mender's or the torrent server's loopback address,
    /// or the file's path.
    pub url: String,
    /// The headers `url` wants.
    pub headers: Vec<(String, String)>,
    pub start: Option<f64>,
    pub metadata: Metadata,
    pub subtitles: Vec<CastSubtitle>,
    /// Which of `subtitles` is on.
    pub subtitle: Option<usize>,
    /// The language the audio track is chosen by.
    pub audio_language: Option<String>,
    /// The audio track the viewer picked, by its place among the file's, over the language.
    pub audio_track: Option<usize>,
}

/// Why a cast didn't play, or stopped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CastFailure {
    #[error(transparent)]
    Uncastable(#[from] Uncastable),
    /// The TV never fetched from the relay.
    #[error("the TV never fetched the stream")]
    NeverFetches,
    /// Every plan was refused.
    #[error("the TV couldn't play this stream")]
    Refused,
    /// Another device started something on the TV.
    #[error("another device took over the TV")]
    TakenOver,
    /// This device isn't on the TV's network, or reaches it through a VPN.
    #[error("this device isn't on the TV's network")]
    NotSameNetwork,
    /// The TV stopped answering.
    #[error("the TV went away: {0}")]
    Lost(String),
}

/// Where a cast is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CastState {
    #[default]
    Connecting,
    Loading,
    Playing,
    Paused,
    Buffering,
    Idle,
    Ended,
}

/// What a cast that hasn't started playing is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastStep {
    /// This device reads the stream and makes it ready for the TV: the probe, the remux.
    Preparing,
    /// The TV opens its player or loads the stream.
    Waiting,
}

/// An audio track of the file casting, as the card lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastAudio {
    /// Its place among the file's audio tracks: what [`CastPlayer::set_audio`] takes.
    pub index: usize,
    pub language: Option<String>,
    pub codec: Option<String>,
    pub channels: u16,
    pub selected: bool,
}

/// A text track sent with the load, as the card lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastText {
    /// The track's id on the TV, from one: what [`CastPlayer::set_subtitle`] takes.
    pub id: u32,
    pub language: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
}

/// What the card shows of a cast, as it changes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CastStatus {
    pub state: CastState,
    pub position: f64,
    pub duration: Option<f64>,
    /// The receiver's volume, 0 to 1.
    pub level: Option<f64>,
    pub muted: Option<bool>,
    pub volume: VolumeControl,
    /// The plan playing, once one does.
    pub plan: Option<CastPlan>,
    pub failure: Option<CastFailure>,
    /// What a load not yet playing is doing.
    pub step: Option<CastStep>,
    /// When the load not yet playing was asked for.
    pub since: Option<Instant>,
    /// The file's audio tracks the TV can be given, when there is a choice: a file the relay
    /// remuxes, with more than one.
    pub audio: Vec<CastAudio>,
    /// The text tracks sent with the load.
    pub subtitles: Vec<CastText>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What the relay makes on request for one cast: the remuxed stream and the text tracks.
#[derive(Default)]
struct Resources {
    remuxer: Mutex<Option<Arc<Remuxer>>>,
    texts: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl Dynamic for Resources {
    fn get<'a>(&'a self, path: &'a str) -> Made<'a> {
        Box::pin(async move {
            if let Some(text) = lock(&self.texts).get(path).cloned() {
                return Some(("text/vtt".into(), text.to_vec()));
            }
            let remuxer = lock(&self.remuxer).clone()?;
            match path {
                "r/index.m3u8" => Some((
                    "application/vnd.apple.mpegurl".into(),
                    remuxer.playlist("seg", "init.mp4").into_bytes(),
                )),
                "r/init.mp4" => remuxer.init().ok().map(|b| ("video/mp4".into(), b)),
                _ => {
                    let index =
                        path.strip_prefix("r/seg")?.strip_suffix(".m4s")?.parse().ok()?;
                    let made =
                        tokio::task::spawn_blocking(move || remuxer.segment(index)).await;
                    match made {
                        Ok(Ok(segment)) => Some(("video/mp4".into(), segment)),
                        Ok(Err(e)) => {
                            tracing::warn!(error = %e, index, "a segment couldn't be made");
                            None
                        }
                        Err(_) => None,
                    }
                }
            }
        })
    }
}

#[derive(Default)]
struct State {
    app: Option<App>,
    /// A plan is being tried: the session hears nothing yet.
    trying: bool,
    /// A reload of our own is out (a late subtitle): its new media session is taken quietly.
    reloading: bool,
    /// The media session the session heard `Loaded` for.
    loaded: Option<i64>,
    last: Option<MediaStatus>,
    paused: Option<bool>,
    paused_since: Option<Instant>,
    buffering: bool,
    rate: Option<f64>,
    /// The speed the viewer asked for, kept for the next load: a `LOAD` starts at normal speed.
    asked_rate: Option<f64>,
    level: Option<f64>,
    muted: Option<bool>,
    control: VolumeControl,
    ended: bool,
    closed: bool,
    quit: bool,
    failure: Option<CastFailure>,
    /// The receiver sent `ERROR` while a plan was tried: the Nest Hub's word for audio it can't
    /// decode, after it accepted the load.
    errored: bool,
    /// The load in force, with its tracks, for a reload when a subtitle comes later.
    load: Option<Load>,
    plan: Option<CastPlan>,
    /// What the session asked to cast, with the subtitles that came later: what a change of
    /// audio loads again.
    asked: Option<CastLoad>,
    step: Option<CastStep>,
    since: Option<Instant>,
    audio: Vec<CastAudio>,
    subtitles: Vec<CastText>,
    /// The text track on, from one.
    subtitle: Option<u32>,
}

impl State {
    fn status(&self) -> CastStatus {
        let state = if self.closed || self.quit {
            CastState::Ended
        } else if self.trying {
            CastState::Loading
        } else if self.ended {
            CastState::Idle
        } else {
            match self.last.as_ref().map(|s| s.state.as_str()) {
                Some("PLAYING") => CastState::Playing,
                Some("PAUSED") => CastState::Paused,
                Some("BUFFERING" | "LOADING") => CastState::Buffering,
                Some(_) => CastState::Idle,
                None if self.app.is_some() => CastState::Loading,
                None => CastState::Connecting,
            }
        };
        CastStatus {
            state,
            position: self.last.as_ref().map_or(0.0, |s| s.position),
            duration: self.last.as_ref().and_then(|s| s.duration),
            level: self.level,
            muted: self.muted,
            volume: self.control,
            plan: self.plan.clone(),
            failure: self.failure.clone(),
            step: self.step.filter(|_| self.trying),
            since: self.since.filter(|_| self.trying),
            audio: self.audio.clone(),
            subtitles: self
                .subtitles
                .iter()
                .map(|t| CastText { selected: Some(t.id) == self.subtitle, ..t.clone() })
                .collect(),
        }
    }
}

struct Inner {
    setup: CastSetup,
    device: Device,
    relay: Relay,
    resources: Arc<Resources>,
    /// The media session in force. Read without waiting, so a control never queues behind a
    /// launch.
    media: Mutex<Option<Media>>,
    /// Held while the receiver app launches, so two loads launch it once.
    launching: AsyncMutex<()>,
    state: Mutex<State>,
    events: Mutex<Option<mpsc::UnboundedSender<PlaybackEvent>>>,
    status: watch::Sender<CastStatus>,
}

impl Inner {
    fn say(&self, event: PlaybackEvent) {
        if let Some(events) = lock(&self.events).as_ref() {
            let _ = events.send(event);
        }
    }

    fn publish(&self) {
        let status = lock(&self.state).status();
        self.publish_status(status);
    }

    /// Publish the status of a state the caller already holds locked.
    fn publish_locked(&self, state: &State) {
        self.publish_status(state.status());
    }

    fn publish_status(&self, status: CastStatus) {
        self.status.send_if_modified(|current| {
            let changed = *current != status;
            *current = status;
            changed
        });
    }

    /// Say an event from inside the state lock. The status goes out first, so whoever reacts to
    /// the event reads a status that already includes what caused it.
    fn say_locked(&self, state: &State, event: PlaybackEvent) {
        self.publish_locked(state);
        self.say(event);
    }

    /// The running receiver app, launched when there is none yet.
    ///
    /// The launch runs in a task of its own, which a Stop doesn't abort: a receiver that comes
    /// up after the viewer stopped is closed again, so it isn't left on the TV with no sender.
    async fn app(self: &Arc<Self>) -> Result<Media, CastError> {
        let _launching = self.launching.lock().await;
        if let Some(media) = self.media() {
            return Ok(media);
        }
        let inner = Arc::clone(self);
        let launch = tokio::spawn(async move {
            let app = inner.device.launch(DEFAULT_MEDIA_RECEIVER).await?;
            {
                // Under the state's lock, so a quit either sees this media or is seen here.
                let mut state = lock(&inner.state);
                if !state.quit {
                    state.app = Some(app.clone());
                    let fresh = Media::new(inner.device.channel.clone(), app);
                    *lock(&inner.media) = Some(fresh.clone());
                    return Ok(fresh);
                }
            }
            tracing::info!("the receiver came up after the stop; closing it");
            let _ = tokio::time::timeout(CONTROL_WAIT, inner.device.stop(&app)).await;
            Err(CastError::Closed)
        });
        launch.await.unwrap_or(Err(CastError::Closed))
    }

    /// The media session in force, for a command.
    fn media(&self) -> Option<Media> {
        lock(&self.media).clone()
    }
}

/// A TV playing what the session tells it.
pub struct CastPlayer {
    inner: Arc<Inner>,
    tasks: Vec<JoinHandle<()>>,
    attempt: Mutex<Option<JoinHandle<()>>>,
}

impl Drop for CastPlayer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        if let Some(attempt) = lock(&self.attempt).take() {
            attempt.abort();
        }
    }
}

impl CastPlayer {
    /// Connect to the TV, read its volume and firmware, and open the relay it will fetch from,
    /// on the interface facing it and for it alone. Nothing starts on the TV yet.
    pub async fn connect(
        setup: CastSetup,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PlaybackEvent>), CastFailure> {
        let address = setup.device.address;
        let IpAddr::V4(tv) = address.ip() else {
            return Err(CastFailure::NotSameNetwork);
        };
        // Loopback is the fake receiver in tests, and nothing else ever has that address.
        let bind = if tv.is_loopback() {
            tv
        } else {
            let local = local_address_towards(tv).map_err(|_| CastFailure::NotSameNetwork)?;
            if !same_network(local, tv) {
                return Err(CastFailure::NotSameNetwork);
            }
            local
        };
        let lost = |e: CastError| CastFailure::Lost(e.to_string());
        let relay = Relay::start(bind, address.ip(), Arc::clone(&setup.upstream))
            .await
            .map_err(|e| CastFailure::Lost(e.to_string()))?;
        relay.set_slow_source(setup.timings.slow_source);
        let resources = Arc::new(Resources::default());
        relay.set_dynamic(Some(Arc::clone(&resources) as Arc<dyn Dynamic>));
        let (device, incoming) =
            Device::connect(address, setup.timings.silence).await.map_err(lost)?;
        let receiver = device.status().await.map_err(lost)?;
        let firmware =
            if tv.is_loopback() { None } else { firmware(setup.upstream.as_ref(), tv).await };
        if let Err(e) = setup.memory.seen(
            &setup.device.id,
            &setup.device.name,
            &setup.device.model,
            firmware.as_deref(),
            receiver.volume,
        ) {
            tracing::warn!(error = %e, "couldn't note the TV");
        }

        let state = State {
            control: receiver.volume,
            level: receiver.level,
            muted: receiver.muted,
            ..State::default()
        };
        let (events, rx) = mpsc::unbounded_channel();
        let (status, _) = watch::channel(state.status());
        let inner = Arc::new(Inner {
            setup,
            device,
            relay,
            resources,
            media: Mutex::new(None),
            launching: AsyncMutex::new(()),
            state: Mutex::new(state),
            events: Mutex::new(Some(events)),
            status,
        });
        let pump = tokio::spawn(pump(incoming, Arc::downgrade(&inner)));
        let poll = tokio::spawn(poll(Arc::downgrade(&inner)));
        Ok((Self { inner, tasks: vec![pump, poll], attempt: Mutex::new(None) }, rx))
    }

    /// The card's view of the cast, as it changes.
    pub fn status(&self) -> watch::Receiver<CastStatus> {
        self.inner.status.subscribe()
    }

    pub fn device(&self) -> &CastDevice {
        &self.inner.setup.device
    }

    /// Where the TV is in the file.
    pub fn position(&self) -> Option<f64> {
        lock(&self.inner.state).last.as_ref().map(|s| s.position)
    }

    /// Say `event` to the session as if the TV had: what the sender's card asks of the session
    /// that only a player would, such as playing the next episode when Up Next's count ends.
    pub fn say(&self, event: PlaybackEvent) {
        self.inner.say(event);
    }

    /// Cast `load` in place of whatever plays. Returns at once: what happened comes back as
    /// events, `Loaded` once a plan plays, or `Ended` unplayed with [`CastPlayer::failure`] set.
    pub fn load(&self, load: CastLoad) {
        self.start(load, false);
    }

    /// Load the file casting again with the audio track at `index` among its tracks, from where
    /// it is: a TV takes its tracks with the load. The session hears no new `Loaded`, as for a
    /// late subtitle. Nothing when no file is casting or the track is the one playing.
    pub fn set_audio(&self, index: usize) {
        let load = {
            let state = lock(&self.inner.state);
            if state.trying || !state.audio.iter().any(|a| a.index == index && !a.selected) {
                return;
            }
            let Some(mut load) = state.asked.clone() else { return };
            load.audio_track = Some(index);
            load.start = state.last.as_ref().map(|s| s.position).or(load.start);
            load.subtitle =
                state.subtitle.and_then(|id| usize::try_from(id).ok()?.checked_sub(1));
            load
        };
        tracing::info!(index, "loading the cast again for another audio track");
        self.start(load, true);
    }

    fn start(&self, load: CastLoad, quiet: bool) {
        {
            let mut state = lock(&self.inner.state);
            state.trying = true;
            state.reloading = false;
            state.ended = false;
            state.failure = None;
            state.loaded = None;
            state.step = Some(CastStep::Preparing);
            state.since = Some(Instant::now());
            state.asked = Some(load.clone());
        }
        self.inner.publish();
        let inner = Arc::clone(&self.inner);
        let task = tokio::spawn(async move {
            let outcome = attempt(&inner, load).await;
            let speed = {
                let mut state = lock(&inner.state);
                state.trying = false;
                state.step = None;
                state.since = None;
                match outcome {
                    Ok(plan) => {
                        tracing::info!(route = ?plan.route, delivery = ?plan.delivery, "cast playing");
                        state.plan = Some(plan);
                        // The same file where it was, for the session: no new `Loaded`.
                        state.reloading = quiet;
                        // The status that confirmed it is the session's first.
                        if let Some(last) = state.last.clone() {
                            apply(&inner, &mut state, &last);
                        }
                    }
                    Err(failure) => {
                        tracing::warn!(%failure, "nothing cast");
                        state.failure = Some(failure);
                        state.ended = true;
                        inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
                    }
                }
                // A load starts at normal speed: the one asked for is set once it plays.
                let playing = state.plan.is_some() && !state.ended;
                state.asked_rate.filter(|r| playing && (r - 1.0).abs() > f64::EPSILON)
            };
            inner.publish();
            if let (Some(rate), Some(media)) = (speed, inner.media()) {
                let _ = answered(async move { media.set_rate(rate).await.map(|_| ()) }).await;
            }
        });
        if let Some(previous) = lock(&self.attempt).replace(task) {
            previous.abort();
        }
    }

    /// Side-load a subtitle file that came after the load, by a reload where it is: tracks are
    /// fixed at `LOAD`.
    pub async fn add_subtitle(
        &self,
        path: &Path,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), CastError> {
        let track = text_track(
            &self.inner,
            &CastSubtitle { path: path.to_owned(), title, language },
            None,
        )?;
        let reload = {
            let mut state = lock(&self.inner.state);
            let position = state.last.as_ref().map(|s| s.position);
            if let Some(asked) = state.asked.as_mut() {
                asked.subtitles.push(CastSubtitle {
                    path: path.to_owned(),
                    title: track_title(&track),
                    language: track_language(&track),
                });
            }
            let reload = state.load.as_mut().map(|load| {
                let mut track = track.clone();
                track.id = u32::try_from(load.tracks.len() + 1).unwrap_or(u32::MAX);
                load.tracks.push(track);
                load.start = position.or(load.start);
                load.clone()
            });
            if let Some(load) = &reload
                && let Some(added) = load.tracks.last()
            {
                state.subtitles.push(CastText {
                    id: added.id,
                    language: track_language(added),
                    title: track_title(added),
                    selected: false,
                });
            }
            if reload.is_some() && !state.trying {
                state.reloading = true;
            }
            reload
        };
        match reload {
            Some(load) if !lock(&self.inner.state).trying => {
                let mut media = self.inner.app().await?;
                let inner = Arc::downgrade(&self.inner);
                answered(async move {
                    media.load(&load).await?;
                    if let Some(inner) = inner.upgrade() {
                        *lock(&inner.media) = Some(media);
                    }
                    Ok(())
                })
                .await
            }
            _ => Ok(()),
        }
    }

    pub async fn set_paused(&self, paused: bool) -> Result<(), CastError> {
        let Some(media) = self.inner.media() else { return Ok(()) };
        answered(async move {
            if paused { media.pause().await } else { media.play().await }.map(|_| ())
        })
        .await
    }

    pub async fn toggle_pause(&self) -> Result<(), CastError> {
        let paused = lock(&self.inner.state).paused.unwrap_or(false);
        self.set_paused(!paused).await
    }

    /// Seek to `seconds`, or by them when not `absolute`.
    pub async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), CastError> {
        let Some(media) = self.inner.media() else { return Ok(()) };
        let target = if absolute { seconds } else { self.position().unwrap_or(0.0) + seconds };
        self.inner.say(PlaybackEvent::Seeking(true));
        let inner = Arc::downgrade(&self.inner);
        answered(async move {
            let result = media.seek(target.max(0.0)).await;
            if let Some(inner) = inner.upgrade() {
                inner.say(PlaybackEvent::Seeking(false));
            }
            result.map(|_| ())
        })
        .await
    }

    pub async fn set_rate(&self, rate: f64) -> Result<(), CastError> {
        let rate = rate.clamp(0.5, 2.0);
        lock(&self.inner.state).asked_rate = Some(rate);
        // Before a load there is nothing to set: the load applies it once it plays.
        let Some(media) = self.inner.media() else { return Ok(()) };
        answered(async move { media.set_rate(rate).await.map(|_| ()) }).await
    }

    pub fn rate(&self) -> f64 {
        lock(&self.inner.state).rate.unwrap_or(1.0)
    }

    /// The receiver's volume, 0 to 100. Nothing on a TV whose volume is its own.
    pub async fn set_volume(&self, level: f64) -> Result<(), CastError> {
        if lock(&self.inner.state).control == VolumeControl::MuteOnly {
            return Ok(());
        }
        let device = self.inner.device.clone();
        answered(
            async move { device.set_volume(Some((level / 100.0).clamp(0.0, 1.0)), None).await },
        )
        .await
    }

    pub fn volume(&self) -> f64 {
        lock(&self.inner.state).level.map_or(100.0, |l| (l * 100.0).round())
    }

    pub fn volume_control(&self) -> VolumeControl {
        lock(&self.inner.state).control
    }

    pub async fn set_muted(&self, muted: bool) -> Result<(), CastError> {
        let device = self.inner.device.clone();
        answered(async move { device.set_volume(None, Some(muted)).await }).await
    }

    pub fn muted(&self) -> bool {
        lock(&self.inner.state).muted.unwrap_or(false)
    }

    /// Show the text track `id` (1-based, as loaded), or none.
    pub async fn set_subtitle(&self, id: Option<u32>) -> Result<(), CastError> {
        {
            let mut state = lock(&self.inner.state);
            state.subtitle = id.filter(|id| state.subtitles.iter().any(|t| t.id == *id));
        }
        self.inner.publish();
        let Some(media) = self.inner.media() else { return Ok(()) };
        let ids: Vec<u32> = id.into_iter().collect();
        answered(async move { media.active_tracks(&ids).await.map(|_| ()) }).await
    }

    /// Stop the file, keeping the receiver for the next.
    pub async fn stop(&self) -> Result<(), CastError> {
        if let Some(attempt) = lock(&self.attempt).take() {
            attempt.abort();
        }
        let Some(media) = self.inner.media() else { return Ok(()) };
        if media.media_session_id.is_none() {
            return Ok(());
        }
        answered(async move { media.stop().await.map(|_| ()) }).await
    }

    /// Stop playing and close the receiver app, so the TV goes back to what it showed. The
    /// event stream ends with it.
    pub async fn quit(&self) -> Result<(), CastError> {
        if let Some(attempt) = lock(&self.attempt).take() {
            attempt.abort();
        }
        lock(&self.inner.state).quit = true;
        let result = close(&self.inner).await;
        self.inner.relay.close();
        self.inner.publish();
        lock(&self.inner.events).take();
        result
    }

    pub fn is_alive(&self) -> bool {
        let state = lock(&self.inner.state);
        self.inner.device.channel.is_open() && !state.closed && !state.quit
    }

    pub fn has_quit(&self) -> bool {
        lock(&self.inner.state).quit
    }

    /// Why nothing played, or why the cast stopped.
    pub fn failure(&self) -> Option<CastFailure> {
        lock(&self.inner.state).failure.clone()
    }
}

/// How long a control waits for the TV's answer before it lets the session go on. mpv answers
/// in milliseconds; a TV that takes longer is answered off the session's loop, and one that has
/// gone is noticed by its channel's silence.
const CONTROL_WAIT: Duration = Duration::from_secs(2);

/// Run a command to the TV, waiting for its answer up to [`CONTROL_WAIT`]. Past that it
/// finishes by itself, and what it says is only logged.
async fn answered(
    command: impl std::future::Future<Output = Result<(), CastError>> + Send + 'static,
) -> Result<(), CastError> {
    let task = tokio::spawn(async move {
        let result = command.await;
        if let Err(e) = &result {
            tracing::debug!(error = %e, "the TV didn't take a command");
        }
        result
    });
    match tokio::time::timeout(CONTROL_WAIT, task).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Ok(()),
        Err(_) => {
            tracing::debug!("the TV is slow to answer; going on without it");
            Ok(())
        }
    }
}

/// Stop the media and the app, when one runs.
async fn close(inner: &Inner) -> Result<(), CastError> {
    let media = lock(&inner.media).take();
    let Some(media) = media else { return Ok(()) };
    lock(&inner.state).app = None;
    let device = inner.device.clone();
    answered(async move {
        if media.media_session_id.is_some() {
            let _ = media.stop().await;
        }
        device.stop(&media.app).await
    })
    .await
}

/// `cast_build_revision` from the TV's local setup API, when it answers within two seconds.
async fn firmware(upstream: &dyn Upstream, tv: std::net::Ipv4Addr) -> Option<String> {
    let url = format!("http://{tv}:8008/setup/eureka_info?params=build_info");
    let fetch = async {
        let response = upstream.get(&url, &[]).await.ok()?;
        let body = response.bytes(256 * 1024).await.ok()?;
        let info: Value = serde_json::from_slice(&body).ok()?;
        info["build_info"]["cast_build_revision"]
            .as_str()
            .or_else(|| info["cast_build_revision"].as_str())
            .map(str::to_owned)
    };
    tokio::time::timeout(Duration::from_secs(2), fetch).await.ok().flatten()
}

/// What `load` holds, read as cheaply as its kind allows.
async fn media_info(setup: &CastSetup, load: &CastLoad) -> (MediaInfo, probe::CorsProbe) {
    let upstream = setup.upstream.as_ref();
    let mut probe = probe::CorsProbe::default();
    let mut info = match load.source.kind {
        SourceKind::Hls => {
            // The source itself, as the TV would ask it.
            let (info, found) =
                probe::hls(upstream, &load.source.url, &load.source.headers).await;
            probe = found;
            info
        }
        SourceKind::File => {
            let path = PathBuf::from(load.url.strip_prefix("file://").unwrap_or(&load.url));
            tokio::task::spawn_blocking(move || {
                let mut file = RangeFile::local(&path).ok()?;
                let mut head = vec![0u8; 64];
                let n = std::io::Read::read(&mut file, &mut head).ok()?;
                let container = probe::sniff(&head[..n]);
                if container == Some(Delivery::Mkv) {
                    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).ok()?;
                    return probe::matroska(&mut file).ok();
                }
                let mut info = MediaInfo { container, ..MediaInfo::default() };
                if container == Some(Delivery::Mp4) {
                    let mut buffer = Vec::new();
                    std::io::Seek::seek(&mut file, std::io::SeekFrom::Start(0)).ok()?;
                    std::io::Read::read_to_end(
                        &mut std::io::Read::take(&mut file, 4 << 20),
                        &mut buffer,
                    )
                    .ok()?;
                    probe::read_sample_entries(&buffer, &mut info);
                }
                Some(info)
            })
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
        }
        SourceKind::Progressive | SourceKind::Torrent => {
            let info = probe::progressive(upstream, &load.url, &load.headers).await;
            if info.container == Some(Delivery::Mkv) {
                probe::matroska_remote(
                    Arc::clone(&setup.upstream),
                    &load.url,
                    load.headers.clone(),
                )
                .await
                .unwrap_or(info)
            } else {
                info
            }
        }
        SourceKind::DeepLink => MediaInfo::default(),
    };
    info.choose_language(load.audio_language.as_deref());
    if let Some(index) = load.audio_track.filter(|i| *i < info.audio.len()) {
        info.audio_choice = index;
    }
    (info, probe)
}

/// A subtitle file as a WebVTT track the relay serves.
fn text_track(
    inner: &Inner,
    subtitle: &CastSubtitle,
    id: Option<u32>,
) -> Result<TextTrack, CastError> {
    let text = std::fs::read(&subtitle.path)?;
    let text = String::from_utf8_lossy(&text);
    let extension = subtitle.path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    let webvtt = to_webvtt(&text, extension.as_deref());
    let mut texts = lock(&inner.resources.texts);
    let id = id.unwrap_or_else(|| u32::try_from(texts.len() + 1).unwrap_or(u32::MAX));
    let name = format!("t/{}-{id}.vtt", texts.len() + 1);
    texts.insert(name.clone(), Arc::new(webvtt.into_bytes()));
    Ok(TextTrack {
        id,
        url: inner.relay.dynamic_url(&name),
        name: subtitle.title.clone().unwrap_or_else(|| {
            subtitle.language.clone().unwrap_or_else(|| format!("Subtitles {id}"))
        }),
        language: subtitle.language.clone().unwrap_or_else(|| "und".into()),
    })
}

/// A subtitle file's text as WebVTT: ASS converted with its typesetting dropped, SRT given a
/// header and full stops, WebVTT as it is.
pub fn to_webvtt(text: &str, extension: Option<&str>) -> String {
    let text = text.trim_start_matches('\u{feff}');
    let ass = matches!(extension, Some("ass" | "ssa"))
        || text.contains("[Script Info]")
        || text.contains("[Events]");
    if ass {
        return vtt::convert(text).0;
    }
    if text.trim_start().starts_with("WEBVTT") {
        return text.to_owned();
    }
    srt_to_vtt(text)
}

/// SRT to WebVTT: a header, and full stops for the decimal commas.
fn srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in srt.lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else if !line.trim().is_empty() && line.trim().chars().all(|c| c.is_ascii_digit()) {
            continue;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

/// What a plan's try came to.
enum Tried {
    Played,
    /// Not played, and what that teaches.
    Refused(Blame),
}

/// What a plan the TV didn't play is held against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blame {
    /// Nothing: the TV never asked for media, or waited on a slow or failing source.
    Nothing,
    /// The format: the receiver refused it in its own words.
    Format,
    /// A copied audio codec at most: the plan stalled with every byte served.
    Audio,
}

/// Try `load`'s plans in order until one plays.
async fn attempt(inner: &Arc<Inner>, load: CastLoad) -> Result<CastPlan, CastFailure> {
    let setup = &inner.setup;
    let (info, cors) = media_info(setup, &load).await;
    let caps = setup.memory.caps(&setup.device.id);
    let plans = cast_plans(&load.source, &info, &caps, &cors, setup.encoder.as_ref())?;
    tracing::info!(plans = plans.len(), video = ?info.video, "casting");
    let audio = audio_choices(setup, &load, &info, &cors);

    // The text tracks, once for every plan.
    lock(&inner.resources.texts).clear();
    let mut tracks = Vec::new();
    for (index, subtitle) in load.subtitles.iter().enumerate() {
        let id = u32::try_from(index + 1).unwrap_or(u32::MAX);
        match text_track(inner, subtitle, Some(id)) {
            Ok(track) => tracks.push(track),
            Err(e) => tracing::warn!(error = %e, "a subtitle couldn't be read"),
        }
    }
    let active: Vec<u32> = load
        .subtitle
        .and_then(|i| u32::try_from(i + 1).ok())
        .filter(|id| tracks.iter().any(|t| t.id == *id))
        .into_iter()
        .collect();
    {
        let mut state = lock(&inner.state);
        state.audio = audio;
        state.subtitles = tracks
            .iter()
            .map(|t| CastText {
                id: t.id,
                language: track_language(t),
                title: track_title(t),
                selected: false,
            })
            .collect();
        state.subtitle = active.first().copied();
    }
    inner.publish();

    // A clip's refusal, when it is what ended the last plan that could be made.
    let mut why = None;
    for plan in plans {
        // What an earlier plan or clip of this load taught is heeded at once.
        let caps = setup.memory.caps(&setup.device.id);
        if plan.parts().iter().any(|part| caps.refused(part)) {
            continue;
        }
        step(inner, CastStep::Waiting);
        if plan.wants_clip()
            && !caps.played(&plan.video_part())
            && let Some(clip) = clip(&plan.video, plan.height)
            && !ask_with_clip(inner, &plan, clip, &load).await?
        {
            why = Some(Uncastable::VideoCodec(plan.video.to_string()));
            continue;
        }
        step(inner, CastStep::Preparing);
        let Some((url, content_type)) = stage(inner, &load, &info, &plan).await else {
            continue;
        };
        step(inner, CastStep::Waiting);
        let cast = Load {
            url,
            content_type,
            hls_segment_format: match plan.delivery {
                Delivery::HlsFmp4 => Some("fmp4".into()),
                Delivery::HlsTs => Some("ts".into()),
                _ => None,
            },
            metadata: load.metadata.clone(),
            start: load.start,
            tracks: tracks.clone(),
            active_tracks: active.clone(),
            style: None,
            autoplay: true,
        };
        match try_plan(inner, plan.relayed(), cast).await? {
            Tried::Played => {
                if let Err(e) = setup.memory.record(&setup.device.id, &plan, true) {
                    tracing::warn!(error = %e, "couldn't note what the TV played");
                }
                return Ok(plan);
            }
            Tried::Refused(blame) => {
                why = None;
                tracing::info!(route = ?plan.route, delivery = ?plan.delivery, ?blame, "the TV didn't play a plan");
                let noted = match blame {
                    Blame::Nothing => Ok(()),
                    Blame::Format => setup.memory.record(&setup.device.id, &plan, false),
                    Blame::Audio => {
                        setup.memory.record_stall(&setup.device.id, &plan).map(drop)
                    }
                };
                if let Err(e) = noted {
                    tracing::warn!(error = %e, "couldn't note what the TV refused");
                }
            }
        }
    }
    // When what this load taught leaves nothing, the reason is the part the TV refused.
    let caps = setup.memory.caps(&setup.device.id);
    cast_plans(&load.source, &info, &caps, &cors, setup.encoder.as_ref())?;
    Err(why.map_or(CastFailure::Refused, CastFailure::Uncastable))
}

/// Say what a load not yet playing is doing.
fn step(inner: &Inner, step: CastStep) {
    let changed = {
        let mut state = lock(&inner.state);
        let changed = state.step != Some(step);
        state.step = Some(step);
        changed
    };
    if changed {
        inner.publish();
    }
}

/// The audio tracks of `info` the card offers, each one the TV could be given: only a file the
/// relay remuxes can change its audio, and only when it has more than one.
fn audio_choices(
    setup: &CastSetup,
    load: &CastLoad,
    info: &MediaInfo,
    cors: &probe::CorsProbe,
) -> Vec<CastAudio> {
    if info.container != Some(Delivery::Mkv) || info.audio.len() < 2 {
        return Vec::new();
    }
    let caps = setup.memory.caps(&setup.device.id);
    info.audio
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            let info = MediaInfo { audio_choice: *index, ..info.clone() };
            cast_plans(&load.source, &info, &caps, cors, setup.encoder.as_ref()).is_ok_and(
                |plans| plans.iter().any(|p| p.route == CastRoute::Relayed { remux: true }),
            )
        })
        .map(|(index, track)| CastAudio {
            index,
            language: track.language.clone(),
            codec: track.codec.as_ref().map(ToString::to_string),
            channels: track.channels,
            selected: index == info.audio_choice,
        })
        .collect()
}

/// A text track's language as the file named it, `None` for "und".
fn track_language(track: &TextTrack) -> Option<String> {
    Some(track.language.clone()).filter(|l| !l.is_empty() && l != "und")
}

/// A text track's own name, when it is more than its language or a number.
fn track_title(track: &TextTrack) -> Option<String> {
    Some(track.name.clone()).filter(|name| {
        !name.is_empty()
            && Some(name) != track_language(track).as_ref()
            && !name.starts_with("Subtitles ")
    })
}

/// Ask the TV whether it decodes `plan`'s picture with `clip`, before a file whose sound would
/// play without it. A refusal is noted for this TV and the plan skipped; a clip that plays is
/// noted too. A TV that never asked for the clip said nothing, and the plan is tried.
async fn ask_with_clip(
    inner: &Arc<Inner>,
    plan: &CastPlan,
    clip: Clip,
    load: &CastLoad,
) -> Result<bool, CastFailure> {
    let setup = &inner.setup;
    let url = inner.relay.share_bytes(clip.name, clip.content_type, clip.bytes);
    let cast = Load {
        url,
        content_type: clip.content_type.to_owned(),
        hls_segment_format: None,
        metadata: load.metadata.clone(),
        start: None,
        tracks: Vec::new(),
        active_tracks: Vec::new(),
        style: None,
        autoplay: true,
    };
    let part = plan.video_part();
    // A clip has no sound, so only the receiver's own refusal says it lacks the decoder.
    let played = match try_plan(inner, true, cast).await? {
        Tried::Played => true,
        Tried::Refused(Blame::Format) => false,
        Tried::Refused(Blame::Nothing | Blame::Audio) => return Ok(true),
    };
    tracing::info!(clip = clip.name, part, played, "asked the TV with a clip");
    if let Err(e) = setup.memory.note(&setup.device.id, &part, played) {
        tracing::warn!(error = %e, "couldn't note the clip's answer");
    }
    Ok(played)
}

/// Make `plan`'s address ready: the relay's route to the source, or the remuxer's playlist.
/// `None` when the plan can't be made, as for a file the remuxer can't read.
async fn stage(
    inner: &Inner,
    load: &CastLoad,
    info: &MediaInfo,
    plan: &CastPlan,
) -> Option<(String, String)> {
    let relay = &inner.relay;
    *lock(&inner.resources.remuxer) = None;
    let content_type = plan.delivery.content_type().to_owned();
    match &plan.route {
        CastRoute::Direct { url } => Some((url.clone(), content_type)),
        CastRoute::Relayed { remux: true } => {
            let options = RemuxOptions {
                audio_track: info.audio_choice,
                audio: match &plan.audio {
                    AudioPlan::ToAac { .. } => {
                        AudioMode::ToAac(Arc::clone(&inner.setup.encoder))
                    }
                    _ => AudioMode::Copy,
                },
            };
            let (upstream, url, headers) =
                (Arc::clone(&inner.setup.upstream), load.url.clone(), load.headers.clone());
            let local = load.source.kind == SourceKind::File;
            let runtime = tokio::runtime::Handle::current();
            let opened = tokio::task::spawn_blocking(move || {
                let source: Box<dyn Source> = if local {
                    let path = url.strip_prefix("file://").unwrap_or(&url);
                    Box::new(RangeFile::local(Path::new(path))?)
                } else {
                    Box::new(RangeFile::remote(upstream, &url, headers, runtime)?)
                };
                Remuxer::open(source, options).map_err(|e| std::io::Error::other(e.to_string()))
            })
            .await
            .ok()?;
            match opened {
                Ok(remuxer) => {
                    *lock(&inner.resources.remuxer) = Some(Arc::new(remuxer));
                    Some((relay.dynamic_url("r/index.m3u8"), content_type))
                }
                Err(e) => {
                    tracing::info!(error = %e, "the remuxer can't read this file; passing it through");
                    None
                }
            }
        }
        CastRoute::Relayed { remux: false } => {
            if load.source.kind == SourceKind::File {
                let path = load.url.strip_prefix("file://").unwrap_or(&load.url);
                return Some((relay.share_file(Path::new(path)), content_type));
            }
            relay.set_headers(load.headers.clone());
            Some((relay.upstream_url(&load.url), content_type))
        }
    }
}

/// What a plan the TV didn't play teaches, by how it went.
fn refused(inner: &Inner) -> Tried {
    let source = inner.relay.source();
    // A TV that gave up while it waited on a slow source said nothing about the format.
    let fair = inner.relay.media_served() && !source.busy && !source.troubled;
    Tried::Refused(if fair { Blame::Format } else { Blame::Nothing })
}

/// What a plan that never moved teaches: a stall says less than a refusal (see
/// [`CastCaps::record_stall`]), and one behind a slow source says nothing.
///
/// [`CastCaps::record_stall`]: crate::memory::CastCaps::record_stall
fn stalled(inner: &Inner) -> Tried {
    let source = inner.relay.source();
    let fair = inner.relay.media_served() && !source.busy && !source.troubled;
    Tried::Refused(if fair { Blame::Audio } else { Blame::Nothing })
}

/// Whether a plan that hasn't moved `waited` after its load has had its chance: the TV is
/// waiting on nothing of ours, and when the source was slow, it has had `movement` since the
/// source caught up.
fn had_its_chance(inner: &Inner, waited: Duration) -> bool {
    let movement = inner.setup.timings.movement;
    let source = inner.relay.source();
    waited >= movement && !source.busy && (!source.troubled || source.quiet >= movement)
}

/// Load one plan, or a capability clip, and watch whether it plays. `relayed` when the TV
/// reads it from the relay.
///
/// Only the receiver's own word (`LOAD_FAILED`, `ERROR`, idle with an error) is a refusal of
/// the format. A plan that doesn't move is a stall, and while the TV waits on the source behind
/// the relay (a torrent that has only just started, a slow CDN) it is not even that: the plan
/// is given until [`Timings::patience`].
async fn try_plan(inner: &Arc<Inner>, relayed: bool, cast: Load) -> Result<Tried, CastFailure> {
    const TICK: Duration = Duration::from_millis(100);
    let timings = inner.setup.timings;
    inner.relay.reset_stats();
    let lost = |e: CastError| CastFailure::Lost(e.to_string());
    let mut media = inner.app().await.map_err(lost)?;
    {
        let mut state = lock(&inner.state);
        state.last = None;
        state.errored = false;
        state.load = Some(cast.clone());
    }
    let started = Instant::now();
    let never_fetched = || relayed && inner.relay.requests() == 0;
    let closed = || {
        let state = lock(&inner.state);
        state
            .closed
            .then(|| state.failure.clone().unwrap_or(CastFailure::Lost("closed".into())))
    };

    // Until the TV answers the load.
    let answer = {
        let loading = media.load(&cast);
        tokio::pin!(loading);
        loop {
            tokio::select! {
                answer = &mut loading => break answer,
                () = tokio::time::sleep(TICK.min(timings.poll)) => {}
            }
            if let Some(failure) = closed() {
                return Err(failure);
            }
            if lock(&inner.state).errored {
                return Ok(refused(inner));
            }
            let waited = started.elapsed();
            if never_fetched() {
                if waited >= timings.fetch {
                    return Err(CastFailure::NeverFetches);
                }
                continue;
            }
            if waited >= timings.fetch && had_its_chance(inner, waited) {
                return Ok(stalled(inner));
            }
            if waited >= timings.patience {
                return Ok(Tried::Refused(Blame::Nothing));
            }
        }
    };
    let first = match answer {
        Ok(status) => status,
        Err(CastError::Refused(_)) => return Ok(refused(inner)),
        Err(CastError::Timeout(_)) if never_fetched() => return Err(CastFailure::NeverFetches),
        Err(CastError::Timeout(_)) => return Ok(stalled(inner)),
        Err(e) => return Err(lost(e)),
    };

    // Until its position moves.
    *lock(&inner.media) = Some(media.clone());
    let session = first.media_session_id;
    let from = first.position;
    let answered = Instant::now();
    loop {
        tokio::time::sleep(TICK.min(timings.poll)).await;
        if let Some(failure) = closed() {
            return Err(failure);
        }
        {
            let state = lock(&inner.state);
            if state.errored {
                return Ok(refused(inner));
            }
            if let Some(last) = state.last.as_ref().filter(|s| s.media_session_id == session) {
                match last.idle_reason.as_deref() {
                    // A short file, a capability clip above all, can end between two polls.
                    Some("FINISHED") => return Ok(Tried::Played),
                    Some(_) => return Ok(refused(inner)),
                    None if last.position > from + 0.2 => return Ok(Tried::Played),
                    None => {}
                }
            }
        }
        if never_fetched() && started.elapsed() >= timings.fetch {
            return Err(CastFailure::NeverFetches);
        }
        let waited = answered.elapsed();
        if had_its_chance(inner, waited) {
            return Ok(stalled(inner));
        }
        if waited >= timings.patience {
            return Ok(Tried::Refused(Blame::Nothing));
        }
    }
}

/// Apply a status the session may hear: `Loaded` for a new media session, then what changed.
fn apply(inner: &Inner, state: &mut State, status: &MediaStatus) {
    let new_session = state.loaded != Some(status.media_session_id);
    if let Some(reason) = status.idle_reason.as_deref() {
        if new_session || state.ended || state.reloading {
            return;
        }
        state.ended = true;
        match reason {
            "FINISHED" => inner.say_locked(state, PlaybackEvent::Ended { complete: true }),
            "INTERRUPTED" => {
                // Something else was loaded in its place.
                state.ended = false;
                inner.say_locked(state, PlaybackEvent::Replaced);
            }
            _ => inner.say_locked(state, PlaybackEvent::Ended { complete: false }),
        }
        return;
    }
    if status.state == "IDLE" {
        return;
    }
    if new_session {
        state.loaded = Some(status.media_session_id);
        state.ended = false;
        if state.reloading {
            // Our own reload, for a late subtitle: the same file, where it was.
            state.reloading = false;
        } else {
            state.paused = None;
            state.rate = None;
            inner.say_locked(state, PlaybackEvent::Loaded);
        }
    }
    inner.say_locked(
        &*state,
        PlaybackEvent::Progress { position: status.position, duration: status.duration },
    );
    let paused = status.state == "PAUSED";
    if state.paused != Some(paused) {
        state.paused = Some(paused);
        state.paused_since = paused.then(Instant::now);
        inner.say_locked(state, PlaybackEvent::Paused(paused));
    }
    let buffering = matches!(status.state.as_str(), "BUFFERING" | "LOADING");
    if state.buffering != buffering {
        state.buffering = buffering;
        inner.say_locked(state, PlaybackEvent::Buffering(buffering));
    }
    if state.rate != Some(status.rate) {
        state.rate = Some(status.rate);
        inner.say_locked(state, PlaybackEvent::Speed(status.rate));
    }
}

/// Note the receiver's volume, and say it when it moved.
fn volume(inner: &Inner, state: &mut State, level: Option<f64>, muted: Option<bool>) {
    if let Some(level) = level
        && state.level != Some(level)
    {
        state.level = Some(level);
        inner.say_locked(state, PlaybackEvent::Volume((level * 100.0).round()));
    }
    if let Some(muted) = muted
        && state.muted != Some(muted)
    {
        state.muted = Some(muted);
    }
}

/// Read what the receiver says into the session's events.
async fn pump(mut incoming: mpsc::UnboundedReceiver<Incoming>, inner: std::sync::Weak<Inner>) {
    while let Some(message) = incoming.recv().await {
        let Some(inner) = inner.upgrade() else { return };
        receive(&inner, &message);
        inner.publish();
    }
    // The channel closed: the TV went away.
    if let Some(inner) = inner.upgrade() {
        let mut state = lock(&inner.state);
        if !state.closed && !state.quit {
            inner.relay.close();
            state.closed = true;
            state.failure.get_or_insert(CastFailure::Lost("the connection closed".into()));
            if !state.ended {
                state.ended = true;
                inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
            }
        }
        drop(state);
        inner.publish();
    }
}

fn receive(inner: &Inner, message: &Incoming) {
    let mut state = lock(&inner.state);
    if message.namespace == NS_RECEIVER && message.payload["type"] == "RECEIVER_STATUS" {
        let Some(receiver) = ReceiverStatus::read(&message.payload) else { return };
        state.control = receiver.volume;
        volume(inner, &mut state, receiver.level, receiver.muted);
        let ours = state.app.as_ref().map(|a| a.session_id.clone());
        let still_ours =
            message.payload["status"]["applications"].as_array().is_some_and(|apps| {
                apps.iter().any(|a| {
                    Some(a["sessionId"].as_str().unwrap_or_default().to_owned()) == ours
                })
            });
        if ours.is_some() && !still_ours && !state.quit {
            let taken = receiver.app.is_some();
            end_cast(
                inner,
                &mut state,
                if taken {
                    CastFailure::TakenOver
                } else {
                    CastFailure::Lost("the TV closed the app".into())
                },
            );
        }
        return;
    }
    let ours = state.app.as_ref().is_some_and(|a| a.transport_id == message.source);
    if message.namespace == NS_CONNECTION && message.payload["type"] == "CLOSE" && ours {
        if !state.quit {
            end_cast(inner, &mut state, CastFailure::Lost("the TV closed the app".into()));
        }
        return;
    }
    if message.namespace != NS_MEDIA {
        return;
    }
    if message.payload["type"] == "ERROR" {
        tracing::info!(payload = %message.payload, "the receiver reported an error");
        if state.trying {
            state.errored = true;
        }
        return;
    }
    let Some(status) = MediaStatus::first(&message.payload) else { return };
    volume(inner, &mut state, status.volume, status.muted);
    state.last = Some(status.clone());
    if !state.trying {
        apply(inner, &mut state, &status);
    }
}

/// The receiver app is gone, or someone else's: the cast is over.
fn end_cast(inner: &Inner, state: &mut State, failure: CastFailure) {
    if state.closed {
        return;
    }
    tracing::info!(%failure, "cast ended");
    inner.relay.close();
    state.closed = true;
    state.app = None;
    state.failure = Some(failure);
    if !state.ended {
        state.ended = true;
        inner.say_locked(state, PlaybackEvent::Ended { complete: false });
    }
}

/// Ask for the media status every [`Timings::poll`], and end a cast paused too long.
async fn poll(inner: std::sync::Weak<Inner>) {
    let period = inner.upgrade().map_or(Duration::from_secs(1), |i| i.setup.timings.poll);
    let mut tick = tokio::time::interval(period);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let Some(inner) = inner.upgrade() else { return };
        let (done, idle) = {
            let state = lock(&inner.state);
            let idle = state
                .paused_since
                .filter(|_| state.paused == Some(true) && !state.trying)
                .is_some_and(|since| since.elapsed() >= inner.setup.timings.idle_close);
            (state.closed || state.quit, idle)
        };
        if done {
            return;
        }
        if idle {
            tracing::info!("paused too long; ending the cast");
            let _ = close(&inner).await;
            inner.relay.close();
            let mut state = lock(&inner.state);
            state.closed = true;
            if !state.ended {
                state.ended = true;
                inner.say_locked(&state, PlaybackEvent::Ended { complete: false });
            }
            drop(state);
            inner.publish();
            return;
        }
        let media = inner.media();
        if let Some(media) = media
            && media.poll().is_err()
        {
            return;
        }
    }
}

#[cfg(test)]
#[path = "player_tests.rs"]
mod tests;
