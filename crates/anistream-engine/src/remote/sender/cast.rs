//! The TV picker's side of casting: TVs with Chromecast built in that a browse found, and the
//! card of the cast running now.
//!
//! The app browses `_googlecast._tcp` with the platform's own DNS-SD, as it browses for TVs
//! running anistream, and reports each record to a [`CastSink`]. Casting itself runs in the
//! engine, as [`Command::CastTo`] starts it: a [`CastSession`] sends the engine its commands and
//! reads the card from what the engine and the TV say.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anistream_cast::{CastDevice, CastRoute, CastState, CastStatus, CastStep, VolumeControl};
use anistream_core::ids::AnilistId;
use anistream_model::{
    Msg,
    command::{Command, PlayerCommand},
    playback::SkipKind,
};
use anistream_remote::{
    command::{RemoteCommand, Routed, Track},
    state::{Screen, TrackInfo},
};
use tokio::task::JoinHandle;

use super::{LinkPhase, NowCard, Problem, RemoteCard, SkipCard, TvSender, lock};
use crate::{
    cast::{CastHub, CastView},
    engine::RemoteSender,
    playback::link::cast::notice,
};

/// What a row's key starts with when it is a Cast device: the TXT `id` follows.
const CAST: &str = "cast:";

/// A position closer to the start than this is not carried back to this device.
const CARRY_FROM: f64 = 5.0;

/// For this long after a start is asked for, the card says only that it is starting; then the
/// engine's line, with the seconds counted.
const QUIET_START: Duration = Duration::from_secs(4);

/// How long the Undo note stays at most, played or not.
const UNDO_LONGEST: Duration = Duration::from_secs(15);

/// What the app's `_googlecast._tcp` browser reports: the TXT record as it came.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundCast {
    pub instance: String,
    pub address: SocketAddr,
    pub txt: Vec<(String, String)>,
}

/// The platform's browser for `_googlecast._tcp`, run while the TV picker is open.
pub trait CastBrowser: Send + Sync {
    fn start(&self, sink: CastSink);
    fn stop(&self);
}

/// Where a browser reports what it finds. Cheap to clone.
#[derive(Clone)]
pub struct CastSink {
    hub: Arc<CastHub>,
}

impl CastSink {
    pub fn found(&self, found: FoundCast) {
        match CastDevice::from_txt(&found.instance, found.address, &found.txt) {
            Some(device) => self.hub.found(device),
            None => tracing::debug!(instance = %found.instance, "a Cast record without an id"),
        }
    }

    pub fn lost(&self, instance: &str) {
        self.hub.lost(instance);
    }
}

/// How a cast to a row's TV reaches it, for the line under its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastRouteHint {
    /// The TV fetches by itself for this title's usual source.
    Direct,
    /// This device serves the stream while it plays.
    ThroughThisDevice,
}

/// A row of the TV picker for a TV with Chromecast built in that doesn't run anistream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastRow {
    /// What [`TvSender::cast`] takes for this row.
    pub key: String,
    pub name: String,
    pub model: String,
    pub address: SocketAddr,
    pub route_hint: CastRouteHint,
}

/// The sender's casting parts, once the engine's hub is attached.
#[derive(Default)]
pub(super) struct Casting {
    attached: Mutex<Option<Attached>>,
    /// Shared with the task that follows the hub, which stops the browse when Settings turns
    /// the Chromecast group off.
    browser: Arc<Mutex<Option<Arc<dyn CastBrowser>>>>,
}

struct Attached {
    hub: Arc<CastHub>,
    engine: RemoteSender,
    /// Tells the sender's watchers when the devices or the cast change.
    task: JoinHandle<()>,
}

impl Drop for Attached {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TvSender {
    /// Cast through the engine `engine` sends to, whose hub is `hub`. Until this is called the
    /// picker has no Cast rows. Spawns on the runtime it is called from.
    pub fn attach_casting(&self, hub: Arc<CastHub>, engine: RemoteSender) {
        let mut changes = hub.changes();
        let ours = Arc::clone(&self.changes);
        let browser = Arc::clone(&self.casting.browser);
        let following = Arc::downgrade(&hub);
        let task = tokio::spawn(async move {
            while changes.changed().await.is_ok() {
                if following.upgrade().is_some_and(|hub| !hub.shows_tvs())
                    && let Some(browser) = lock(&browser).take()
                {
                    browser.stop();
                }
                ours.send_modify(|n| *n += 1);
            }
        });
        *lock(&self.casting.attached) = Some(Attached { hub, engine, task });
    }

    fn hub(&self) -> Option<(Arc<CastHub>, RemoteSender)> {
        lock(&self.casting.attached).as_ref().map(|a| (Arc::clone(&a.hub), a.engine.clone()))
    }

    /// Browse for TVs with Chromecast built in with the platform's browser while the picker is
    /// open. A browse already running is stopped first. Nothing without [`Self::attach_casting`].
    pub fn browse_cast(&self, browser: Arc<dyn CastBrowser>) {
        let Some((hub, _)) = self.hub() else { return };
        self.stop_browsing_cast();
        if !hub.shows_tvs() {
            return;
        }
        browser.start(CastSink { hub });
        *lock(&self.casting.browser) = Some(browser);
    }

    pub(super) fn stop_browsing_cast(&self) {
        if let Some(browser) = lock(&self.casting.browser).take() {
            browser.stop();
        }
        if let Some((hub, _)) = self.hub() {
            hub.forget_found();
        }
    }

    /// The picker's Cast rows, by name: what the browse found, less any TV that runs anistream
    /// at the same address, which its own row stands for. None while Settings turns them off.
    ///
    /// Each row says the stream plays through this device until a cast has played and the
    /// route of what plays is known; from then the rows say that route.
    pub fn cast_rows(&self) -> Vec<CastRow> {
        let Some((hub, _)) = self.hub() else { return Vec::new() };
        if !hub.shows_tvs() {
            return Vec::new();
        }
        let view = hub.view();
        let route_hint = view
            .status
            .as_ref()
            .filter(|_| !view.over)
            .and_then(route_of)
            .unwrap_or(CastRouteHint::ThroughThisDevice);
        let ours: Vec<_> = lock(&self.found)
            .values()
            .flat_map(|tv| tv.addresses.iter().map(SocketAddr::ip))
            .collect();
        hub.devices()
            .into_iter()
            .filter(|device| !ours.contains(&device.address.ip()))
            .map(|device| CastRow {
                key: format!("{CAST}{}", device.id),
                name: device.name,
                model: device.model,
                address: device.address,
                // A source's own address reaches the TV only for a stream that needs no
                // headers, which is not known until the episode resolves.
                route_hint,
            })
            .collect()
    }

    /// Cast `episode` of `id` to the Cast device behind `key`, from `position` when carried
    /// over from this device.
    pub fn cast(
        &self,
        key: &str,
        id: AnilistId,
        episode: String,
        position: Option<f64>,
    ) -> Result<Arc<CastSession>, Problem> {
        let (hub, engine) = self.hub().ok_or(Problem::Other)?;
        let device_id = key.strip_prefix(CAST).unwrap_or(key);
        let device = hub.device(device_id).ok_or(Problem::Gone)?;
        // The cast is the hub's from the tap, so the engine still finds the TV when the picker
        // closes, and forgets what the browse found, before it takes the command.
        hub.begin(device.clone());
        let command = Command::CastTo { device: device.id.clone(), id, episode, position };
        if engine.send(command).is_none() {
            hub.end();
            return Err(Problem::Other);
        }
        Ok(Arc::new(CastSession { hub, engine, device }))
    }

    /// The cast running now, for a card opened again.
    pub fn cast_session(&self) -> Option<Arc<CastSession>> {
        let (hub, engine) = self.hub()?;
        let view = hub.view();
        let device = view.device.filter(|_| !view.over)?;
        Some(Arc::new(CastSession { hub, engine, device }))
    }
}

/// The card's source for a cast, read as [`RemoteCard`] is for Play on TV.
pub struct CastSession {
    hub: Arc<CastHub>,
    engine: RemoteSender,
    device: CastDevice,
}

/// The card for a cast: the card Play on TV draws, and what only a cast has.
#[derive(Debug, Clone, PartialEq)]
pub struct CastCard {
    pub card: RemoteCard,
    /// Whether the TV's volume can be set, or only muted.
    pub volume_control: VolumeControl,
    pub muted: Option<bool>,
    /// Why the cast stopped, or why nothing played, in the engine's words.
    pub notice: Option<Msg>,
    /// What an episode starting on the TV is doing: "Starting on Living room" for its first
    /// seconds, then the engine's line with the seconds since it was asked for. `None` once it
    /// plays.
    pub phase_line: Option<Msg>,
    /// Up Next, while its card is up.
    pub up_next: Option<CastUpNext>,
    /// An automatic skip that can be undone, while its note is up.
    pub undo: Option<CastUndo>,
    /// How the stream reaches the TV, once one plays.
    pub route: Option<CastRouteHint>,
}

/// Up Next on the casting card.
#[derive(Debug, Clone, PartialEq)]
pub struct CastUpNext {
    pub number: String,
    pub title: Option<String>,
    /// The episode's still, by URL.
    pub still: Option<String>,
    /// The episode plays by itself when the count ends.
    pub auto: bool,
    /// Whole seconds left on the count, while it runs. `None` with auto-next off or no
    /// countdown, where the card waits for Play now.
    pub seconds_left: Option<u32>,
}

/// The note that an automatic skip can be undone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastUndo {
    pub kind: SkipKind,
    /// Where Undo goes back to.
    pub to: f64,
}

/// How the playing stream reaches the TV.
fn route_of(status: &CastStatus) -> Option<CastRouteHint> {
    status.plan.as_ref().map(|plan| match plan.route {
        CastRoute::Direct { .. } => CastRouteHint::Direct,
        CastRoute::Relayed { .. } => CastRouteHint::ThroughThisDevice,
    })
}

/// The card's line while an episode starts on the TV called `name`.
fn phase_line(view: &CastView, name: &str, now: Instant) -> Option<Msg> {
    let status = view.status.as_ref();
    let since = view.starting.or_else(|| status.and_then(|s| s.since))?;
    let name = name.to_owned();
    let waited = now.saturating_duration_since(since);
    if waited < QUIET_START {
        return Some(Msg::CastPhaseStarting { name });
    }
    let seconds = i64::try_from(waited.as_secs()).unwrap_or(i64::MAX);
    Some(match status.and_then(|s| s.step) {
        Some(CastStep::Preparing) => Msg::CastPhasePreparing { seconds },
        Some(CastStep::Waiting) => Msg::CastPhaseWaiting { name, seconds },
        None => Msg::CastPhaseStartingFor { name, seconds },
    })
}

/// The card's audio and subtitle tracks, from what was loaded on the TV.
fn tracks(card: &mut NowCard, status: &CastStatus) {
    card.audio = status
        .audio
        .iter()
        .map(|a| TrackInfo {
            id: i64::try_from(a.index).unwrap_or(i64::MAX),
            kind: Track::Audio,
            title: None,
            language: a.language.clone(),
            selected: a.selected,
        })
        .collect();
    card.subtitles = status
        .subtitles
        .iter()
        .map(|t| TrackInfo {
            id: i64::from(t.id),
            kind: Track::Subtitle,
            title: t.title.clone(),
            language: t.language.clone(),
            selected: t.selected,
        })
        .collect();
    card.subtitles_off = !card.subtitles.iter().any(|t| t.selected);
}

impl CastSession {
    pub fn device_name(&self) -> &str {
        &self.device.name
    }

    /// Counts up whenever [`Self::card`] may read differently.
    pub fn changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.hub.changes()
    }

    pub fn card(&self) -> CastCard {
        self.card_at(Instant::now())
    }

    fn card_at(&self, at: Instant) -> CastCard {
        let view = self.hub.view();
        let ours = view.device.as_ref().is_some_and(|d| d.id == self.device.id);
        let status = view.status.clone().filter(|_| ours);
        let failure = status.as_ref().and_then(|s| s.failure.clone());
        let over = !ours || view.over || (!view.connected && failure.is_some());
        let phase =
            if view.connected && ours { LinkPhase::Connected } else { LinkPhase::Connecting };
        let now = view.state.clone().filter(|_| ours).and_then(|state| state.now_playing).map(
            |now| {
                let mut card = NowCard::read(&now);
                if let Some(status) = &status {
                    card.position = status.position;
                    card.duration = status.duration.or(card.duration);
                    card.paused = status.state == CastState::Paused;
                    card.loading = matches!(
                        status.state,
                        CastState::Connecting | CastState::Loading | CastState::Idle
                    ) && status.plan.is_none()
                        || status.step.is_some();
                    card.volume = status.level.map(|level| (level * 100.0).round());
                    tracks(&mut card, status);
                }
                card.skip = view.skip.filter(|_| !over).map(|(kind, to)| SkipCard {
                    to,
                    label: match kind {
                        SkipKind::Opening => "Skip opening".into(),
                        SkipKind::Ending => "Skip ending".into(),
                    },
                });
                card
            },
        );
        let (position, duration) =
            status.as_ref().map_or((0.0, None), |s| (s.position, s.duration));
        let up_next = view
            .next
            .as_ref()
            .filter(|next| !over && next.shown(position, duration))
            .map(|next| CastUpNext {
                number: next.number.clone(),
                title: next.title.clone(),
                still: next.still.clone(),
                auto: next.auto,
                seconds_left: next.remaining(position, duration).map(|left| left.ceil() as u32),
            });
        let undo = view
            .undo
            .as_ref()
            .filter(|undo| !over && at.saturating_duration_since(undo.since()) < UNDO_LONGEST)
            .map(|undo| CastUndo { kind: undo.kind, to: undo.to });
        let phase_line =
            if over || !ours { None } else { phase_line(&view, &self.device.name, at) };
        CastCard {
            card: RemoteCard {
                tv_name: self.device.name.clone(),
                phase,
                phase_line: None,
                screen: Screen::Player,
                now,
                party: None,
                rev: 0,
                over,
                reached: view.connected || status.is_some(),
            },
            volume_control: status.as_ref().map_or(VolumeControl::Level, |s| s.volume),
            muted: status.as_ref().and_then(|s| s.muted),
            notice: failure.as_ref().map(|failure| notice(failure, &self.device.name)),
            phase_line,
            up_next,
            undo,
            route: status.as_ref().and_then(route_of),
        }
    }

    /// Whether the card reads differently as time passes with nothing said: the seconds of a
    /// start, or an Undo note running out while paused. A front end waiting for the next card
    /// is woken each second meanwhile.
    pub fn ticking(&self) -> bool {
        let card = self.card();
        card.phase_line.is_some() || card.undo.is_some()
    }

    /// What the card's controls ask: the TV's volume and mute go to the receiver, and so do the
    /// tracks, which a TV takes with its load; the rest to the engine's playback, as the TV's
    /// own front end would send it.
    pub fn command(&self, command: RemoteCommand) {
        match command {
            RemoteCommand::Track { kind: Track::Audio, id } => {
                let Some(player) = self.hub.player() else { return };
                if let Some(index) = id.and_then(|id| usize::try_from(id).ok()) {
                    player.set_audio(index);
                }
            }
            RemoteCommand::Track { kind: Track::Subtitle, id } => {
                let Some(player) = self.hub.player() else { return };
                let id = id.and_then(|id| u32::try_from(id).ok());
                tokio::spawn(async move {
                    if let Err(e) = player.set_subtitle(id).await {
                        tracing::debug!(error = %e, "the TV didn't take the subtitles");
                    }
                });
            }
            RemoteCommand::Volume { step } => {
                let Some(player) = self.hub.player() else { return };
                if player.volume_control() == VolumeControl::MuteOnly {
                    return;
                }
                // The step and the player's volume are both 0 to 100.
                let level = (player.volume() + step).clamp(0.0, 100.0);
                tokio::spawn(async move {
                    if let Err(e) = player.set_volume(level).await {
                        tracing::debug!(error = %e, "the TV didn't take the volume");
                    }
                });
            }
            RemoteCommand::Mute { muted } => {
                let Some(player) = self.hub.player() else { return };
                tokio::spawn(async move {
                    if let Err(e) = player.set_muted(muted).await {
                        tracing::debug!(error = %e, "the TV didn't take the mute");
                    }
                });
            }
            command => {
                if let Routed::Engine(command) = command.route() {
                    let _ = self.engine.send(command);
                }
            }
        }
    }

    /// Stop casting and say where the episode was, for this device to play on from there.
    pub fn play_here(&self) -> Option<f64> {
        let position = self.card().card.now.map(|now| now.position);
        self.stop();
        position.filter(|p| *p >= CARRY_FROM)
    }

    /// Stop casting: the TV goes back to what it showed.
    pub fn stop(&self) {
        let _ = self.engine.send(Command::StopCasting);
    }

    /// Up Next's Play now: the next episode starts on the TV.
    pub fn play_next(&self) {
        self.hub.next_answered(true);
        let _ = self.engine.send(Command::Player(PlayerCommand::PlayNext));
    }

    /// Up Next's Cancel: this episode doesn't roll on, and the card comes down.
    pub fn cancel_next(&self) {
        self.hub.next_answered(false);
        let _ = self.engine.send(Command::Player(PlayerCommand::CancelNext));
    }

    /// The Undo note's button: back to where the automatic skip started.
    pub fn undo_skip(&self) {
        if let Some(undo) = self.hub.undo_taken() {
            let _ = self.engine.send(Command::Player(PlayerCommand::UndoSkip { to: undo.to }));
        }
    }

    /// Cast the episode that didn't start to the same TV again, from where it was. `false`
    /// when there is nothing to try again.
    pub fn try_again(&self) -> bool {
        let view = self.hub.view();
        let Some((id, episode)) = view.asked.clone() else { return false };
        let position = view
            .status
            .as_ref()
            .map(|s| s.position)
            .or_else(|| view.state.as_ref()?.now_playing.as_ref().map(|now| now.position))
            .filter(|p| *p >= CARRY_FROM);
        self.hub.begin(self.device.clone());
        let command = Command::CastTo { device: self.device.id.clone(), id, episode, position };
        if self.engine.send(command).is_none() {
            self.hub.end();
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_cast::{
        CastFailure, CastPlayer, CastSetup, CastStatus, PlainHttp, Timings,
        fake::{FakeOptions, FakeReceiver},
    };
    use anistream_model::{command::PlayerCommand, event::Event};
    use anistream_remote::{
        client::Hello, discovery::FoundTv, identity::Identity, known::KnownTvs,
    };

    use super::*;

    fn sender() -> (TvSender, Arc<CastHub>, impl FnMut() -> Vec<Command>) {
        let sender = TvSender::new(
            Identity::generate().unwrap(),
            Hello { name: "Phone".into(), app: "anistream-test/1".into() },
            KnownTvs::memory(),
        );
        let hub = Arc::new(CastHub::with(None, Arc::new(PlainHttp), Timings::default()));
        let (engine, sent) = RemoteSender::recording();
        sender.attach_casting(Arc::clone(&hub), engine);
        (sender, hub, sent)
    }

    fn txt(id: &str, name: &str, model: &str, ca: &str) -> Vec<(String, String)> {
        [("id", id), ("fn", name), ("md", model), ("ca", ca)]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }

    fn found(instance: &str, ip: &str, txt: Vec<(String, String)>) -> FoundCast {
        FoundCast {
            instance: instance.into(),
            address: format!("{ip}:8009").parse().unwrap(),
            txt,
        }
    }

    #[tokio::test]
    async fn the_picker_lists_screens_and_leaves_out_speakers_and_tvs_running_anistream() {
        let (sender, hub, _) = sender();
        let sink = CastSink { hub };
        sink.found(found(
            "Chromecast-a",
            "10.0.0.20",
            txt("a", "Bedroom", "Chromecast", "4101"),
        ));
        sink.found(found("Speaker-b", "10.0.0.21", txt("b", "Kitchen", "Google Home", "2048")));
        sink.found(found(
            "Group-c",
            "10.0.0.22",
            txt("c", "Upstairs", "Google Cast Group", "4101"),
        ));
        sink.found(found("TV-d", "10.0.0.23", txt("d", "Hall TV", "Android TV", "4101")));
        sink.found(found("NoId-e", "10.0.0.24", txt("", "Nameless", "Chromecast", "4101")));
        // The hall TV runs anistream: its own row stands for it.
        let anistream = FoundTv::from_txt(
            "anistream-hall",
            vec!["10.0.0.23:9400".parse().unwrap()],
            &[("v".to_owned(), "1".to_owned())],
        );
        lock(&sender.found).insert(anistream.instance.clone(), anistream);

        let rows = sender.cast_rows();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].key, "cast:a");
        assert_eq!(rows[0].name, "Bedroom");
        assert_eq!(rows[0].route_hint, CastRouteHint::ThroughThisDevice);

        sink.lost("Chromecast-a");
        assert!(sender.cast_rows().is_empty());
    }

    #[tokio::test]
    async fn a_cast_asks_the_engine_and_its_card_reads_the_engine_and_the_tv() {
        let (sender, hub, mut sent) = sender();
        CastSink { hub: Arc::clone(&hub) }.found(found(
            "Chromecast-a",
            "10.0.0.20",
            txt("a", "Bedroom", "Chromecast", "5"),
        ));
        assert_eq!(
            sender.cast("cast:z", AnilistId::new(7), "3".into(), None).err(),
            Some(Problem::Gone)
        );

        let session = sender.cast("cast:a", AnilistId::new(7), "3".into(), Some(42.0)).unwrap();
        assert_eq!(
            sent(),
            vec![Command::CastTo {
                device: "a".into(),
                id: AnilistId::new(7),
                episode: "3".into(),
                position: Some(42.0),
            }]
        );
        // What the engine does with it: the cast begins, the play is staged.
        hub.begin(hub.device("a").unwrap());
        hub.observe(&Event::PlaybackStaged {
            id: AnilistId::new(7),
            title: "Harbour Lights".into(),
            episode: "3".into(),
            episode_title: Some("Low Tide".into()),
            speed: 1.0,
        });
        let card = session.card();
        assert_eq!(card.card.tv_name, "Bedroom");
        assert_eq!(card.card.phase, LinkPhase::Connecting);
        assert!(!card.card.over);
        let now = card.card.now.unwrap();
        assert_eq!(now.episode_line, "Episode 3 · Low Tide");

        session.command(RemoteCommand::Seek { by: 10.0 });
        session.command(RemoteCommand::Skip);
        session.command(RemoteCommand::Text { text: "moon".into(), submit: true });
        assert_eq!(
            sent(),
            vec![
                Command::Player(PlayerCommand::Seek(10.0)),
                Command::Player(PlayerCommand::TakeSkip),
            ],
            "what only a TV running anistream has goes nowhere"
        );
        assert!(sender.cast_session().is_some());
        assert_eq!(session.play_here(), None, "nothing played yet");
        assert_eq!(sent(), vec![Command::StopCasting]);
        hub.end();
        assert!(session.card().card.over);
        assert!(sender.cast_session().is_none());
    }

    #[tokio::test]
    async fn closing_the_picker_right_after_a_tap_still_finds_the_tv() {
        let (sender, hub, mut sent) = sender();
        CastSink { hub: Arc::clone(&hub) }.found(found(
            "Chromecast-a",
            "10.0.0.20",
            txt("a", "Bedroom", "Chromecast", "5"),
        ));
        sender.cast("cast:a", AnilistId::new(7), "3".into(), None).unwrap();
        // The app closes the picker before the engine has taken the command.
        sender.stop_browsing_cast();
        assert!(sender.cast_rows().is_empty(), "the browse's rows went with the picker");
        assert!(matches!(sent().as_slice(), [Command::CastTo { .. }]));
        assert_eq!(
            hub.device("a").map(|d| d.name),
            Some("Bedroom".into()),
            "what the engine looks the TV up by"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_tv_gone_from_the_network_ends_the_card_with_a_notice() {
        let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
        let (sender, _, _sent) = sender();
        let timings = Timings { silence: Duration::from_secs(1), ..Timings::default() };
        let hub = Arc::new(CastHub::with(None, Arc::new(PlainHttp), timings));
        let (engine, _sent) = RemoteSender::recording();
        sender.attach_casting(Arc::clone(&hub), engine);
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        hub.found(device.clone());
        let session = sender.cast("cast:tv-1", AnilistId::new(7), "1".into(), None).unwrap();
        let (player, _events) = CastPlayer::connect(CastSetup {
            device,
            memory: hub.memory(),
            upstream: hub.upstream(),
            encoder: hub.encoder(),
            timings: hub.timings(),
        })
        .await
        .unwrap();
        let player = Arc::new(player);
        hub.attach(&player);
        assert_eq!(session.card().card.phase, LinkPhase::Connected);

        receiver.go_silent();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while session.card().notice.is_none() {
            assert!(tokio::time::Instant::now() < deadline, "the card noticed");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let card = session.card();
        assert_eq!(card.notice, Some(Msg::NoticeCastLost { name: "Living room".into() }));
        assert!(card.card.over);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_cards_volume_steps_the_tvs_level_in_hundredths() {
        let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
        let (sender, hub, _sent) = sender();
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        hub.found(device.clone());
        let session = sender.cast("cast:tv-1", AnilistId::new(7), "1".into(), None).unwrap();
        let (player, _events) = CastPlayer::connect(CastSetup {
            device,
            memory: hub.memory(),
            upstream: hub.upstream(),
            encoder: hub.encoder(),
            timings: hub.timings(),
        })
        .await
        .unwrap();
        let player = Arc::new(player);
        hub.attach(&player);
        assert_eq!(player.volume(), 40.0, "the fake starts at 0.4");

        // A step of 10 on the card is ten points of the TV's 100.
        session.command(RemoteCommand::Volume { step: 10.0 });
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while receiver.heard("SET_VOLUME").is_empty() {
            assert!(tokio::time::Instant::now() < deadline, "the step reached the TV");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let level = receiver.heard("SET_VOLUME")[0]["volume"]["level"].as_f64().unwrap();
        assert!((level - 0.5).abs() < 1e-9, "{level}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_tv_with_fixed_volume_takes_mute_and_not_steps() {
        let receiver =
            FakeReceiver::start(FakeOptions { control: "fixed", ..FakeOptions::default() })
                .await
                .unwrap();
        let (sender, hub, _sent) = sender();
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        hub.found(device.clone());
        let session = sender.cast("cast:tv-1", AnilistId::new(7), "1".into(), None).unwrap();
        hub.begin(device.clone());
        let (player, _events) = CastPlayer::connect(CastSetup {
            device,
            memory: hub.memory(),
            upstream: hub.upstream(),
            encoder: hub.encoder(),
            timings: hub.timings(),
        })
        .await
        .unwrap();
        let player = Arc::new(player);
        hub.attach(&player);
        let card = session.card();
        assert_eq!(card.volume_control, VolumeControl::MuteOnly);
        assert!(card.card.phase == LinkPhase::Connected);

        session.command(RemoteCommand::Volume { step: 10.0 });
        session.command(RemoteCommand::Mute { muted: true });
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while session.card().muted != Some(true) {
            assert!(tokio::time::Instant::now() < deadline, "the mute reached the TV");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let volumes = receiver.heard("SET_VOLUME");
        assert_eq!(volumes.len(), 1, "{volumes:?}");
        assert_eq!(volumes[0]["volume"]["muted"], true);
        assert!(volumes[0]["volume"].get("level").is_none());
        let status: CastStatus = player.status().borrow().clone();
        assert_eq!(status.muted, Some(true));
    }

    fn living_room() -> CastDevice {
        CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: "10.0.0.20:8009".parse().unwrap(),
            video: true,
            group: false,
        }
    }

    fn plan(route: CastRoute) -> anistream_cast::CastPlan {
        anistream_cast::CastPlan {
            route,
            video: anistream_cast::VideoFormat::H264,
            height: None,
            audio: anistream_cast::AudioPlan::Copy(anistream_cast::AudioCodec::Aac),
            delivery: anistream_cast::Delivery::HlsTs,
        }
    }

    fn playing(position: f64, duration: f64) -> CastStatus {
        CastStatus {
            state: CastState::Playing,
            position,
            duration: Some(duration),
            plan: Some(plan(CastRoute::Relayed { remux: false })),
            ..CastStatus::default()
        }
    }

    fn next_up(at: Option<f64>, auto: bool, countdown: u32) -> crate::playback::link::Overlay {
        crate::playback::link::Overlay::Next {
            card: Some(anistream_playback::companion::NextUp {
                number: "4".into(),
                label: "Episode 4".into(),
                title: Some("The Far Shore".into()),
                at,
                auto,
                thumbnail: None,
            }),
            still: Some("https://img.example/4.jpg".into()),
            countdown,
        }
    }

    /// A cast to the Living room the sender asked for, with nothing on the TV yet.
    fn casting() -> (TvSender, Arc<CastHub>, Arc<CastSession>, impl FnMut() -> Vec<Command>) {
        let (sender, hub, mut sent) = sender();
        hub.found(living_room());
        let session = sender.cast("cast:tv-1", AnilistId::new(7), "3".into(), None).unwrap();
        hub.asked(AnilistId::new(7), "3".into());
        sent();
        (sender, hub, session, sent)
    }

    #[test]
    fn the_phase_line_says_starting_then_what_the_load_is_doing() {
        let asked = Instant::now();
        let mut view = CastView { starting: Some(asked), ..CastView::default() };
        let at = |secs| asked + Duration::from_secs(secs);
        assert_eq!(
            phase_line(&view, "Living room", at(1)),
            Some(Msg::CastPhaseStarting { name: "Living room".into() }),
            "no counter in the first seconds"
        );
        assert_eq!(
            phase_line(&view, "Living room", at(6)),
            Some(Msg::CastPhaseStartingFor { name: "Living room".into(), seconds: 6 }),
            "still finding the stream"
        );
        let step = |step| CastStatus { step: Some(step), ..CastStatus::default() };
        view.status = Some(step(CastStep::Waiting));
        assert_eq!(
            phase_line(&view, "Living room", at(6)),
            Some(Msg::CastPhaseWaiting { name: "Living room".into(), seconds: 6 })
        );
        view.status = Some(step(CastStep::Preparing));
        assert_eq!(
            phase_line(&view, "Living room", at(7)),
            Some(Msg::CastPhasePreparing { seconds: 7 })
        );
        // An audio change reloads with nothing asked of the engine: its own clock counts.
        let view = CastView {
            status: Some(CastStatus {
                step: Some(CastStep::Waiting),
                since: Some(asked),
                ..CastStatus::default()
            }),
            ..CastView::default()
        };
        assert_eq!(
            phase_line(&view, "Living room", at(5)),
            Some(Msg::CastPhaseWaiting { name: "Living room".into(), seconds: 5 })
        );
        assert_eq!(phase_line(&CastView::default(), "Living room", at(5)), None);
    }

    #[tokio::test]
    async fn the_card_has_a_phase_line_until_the_episode_plays_on_the_tv() {
        let (_sender, hub, session, _sent) = casting();
        assert_eq!(
            session.card().phase_line,
            Some(Msg::CastPhaseStarting { name: "Living room".into() })
        );
        assert!(session.ticking(), "the seconds wake the card");
        hub.set_status(CastStatus {
            state: CastState::Loading,
            step: Some(CastStep::Waiting),
            ..CastStatus::default()
        });
        assert!(session.card().phase_line.is_some());
        hub.set_status(playing(0.5, 1400.0));
        let card = session.card();
        assert_eq!(card.phase_line, None);
        assert!(!session.ticking());
        assert_eq!(card.route, Some(CastRouteHint::ThroughThisDevice));
    }

    #[tokio::test]
    async fn up_next_counts_down_on_the_card_and_a_seek_into_it_starts_the_count_again() {
        let (_sender, hub, session, mut sent) = casting();
        hub.overlay(&next_up(Some(1290.0), true, 10));
        hub.set_status(playing(1280.0, 1400.0));
        assert_eq!(session.card().up_next, None, "before the ending");

        hub.set_status(playing(1292.4, 1400.0));
        let up = session.card().up_next.unwrap();
        assert_eq!(up.number, "4");
        assert_eq!(up.title.as_deref(), Some("The Far Shore"));
        assert_eq!(up.still.as_deref(), Some("https://img.example/4.jpg"));
        assert!(up.auto);
        assert_eq!(up.seconds_left, Some(8));

        // A seek to 1295 lands in the count: it runs its full ten seconds from there.
        session.command(RemoteCommand::Seek { by: 3.0 });
        hub.seeked(1295.0);
        hub.set_status(playing(1296.0, 1400.0));
        assert_eq!(session.card().up_next.unwrap().seconds_left, Some(9));
        assert_eq!(sent(), vec![Command::Player(PlayerCommand::Seek(3.0))]);

        session.cancel_next();
        assert_eq!(session.card().up_next, None);
        assert_eq!(sent(), vec![Command::Player(PlayerCommand::CancelNext)]);
        // The session says the same card again: it stays down.
        hub.overlay(&next_up(Some(1290.0), true, 10));
        assert_eq!(session.card().up_next, None);
    }

    #[tokio::test]
    async fn up_next_without_an_ending_shows_near_the_end_and_waits_with_auto_next_off() {
        let (_sender, hub, session, mut sent) = casting();
        hub.overlay(&next_up(None, false, 10));
        hub.set_status(playing(1369.0, 1400.0));
        assert_eq!(session.card().up_next, None);
        hub.set_status(playing(1371.0, 1400.0));
        let up = session.card().up_next.unwrap();
        assert!(!up.auto);
        assert_eq!(up.seconds_left, None, "it waits for Play now");

        session.play_next();
        assert_eq!(session.card().up_next, None);
        assert_eq!(sent(), vec![Command::Player(PlayerCommand::PlayNext)]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_count_running_out_plays_the_next_episode_on_the_tv() {
        let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
        let (sender, hub, _sent) = sender();
        let device = CastDevice { address: receiver.address, ..living_room() };
        hub.found(device.clone());
        let _session = sender.cast("cast:tv-1", AnilistId::new(7), "3".into(), None).unwrap();
        let (player, mut events) = CastPlayer::connect(CastSetup {
            device,
            memory: hub.memory(),
            upstream: hub.upstream(),
            encoder: hub.encoder(),
            timings: hub.timings(),
        })
        .await
        .unwrap();
        let player = Arc::new(player);
        hub.attach(&player);
        hub.overlay(&next_up(Some(40.0), true, 5));
        hub.set_status(playing(44.0, 60.0));
        hub.set_status(playing(45.2, 60.0));
        let said = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match events.recv().await {
                    Some(anistream_playback::PlaybackEvent::Script(
                        anistream_playback::ScriptEvent::PlayNext,
                    )) => return true,
                    Some(_) => {}
                    None => return false,
                }
            }
        })
        .await;
        assert_eq!(said, Ok(true), "the player asked the session for the next episode");
        // Once: the count doesn't ask again while the next one loads.
        hub.set_status(playing(46.0, 60.0));
        assert!(hub.view().next.is_some_and(|next| !next.shown(46.0, Some(60.0))));
    }

    #[tokio::test]
    async fn an_automatic_skip_can_be_undone_for_six_seconds_of_playback() {
        use crate::playback::link::Overlay;
        use anistream_playback::SkipKind as Part;

        let (_sender, hub, session, mut sent) = casting();
        hub.observe(&Event::PlaybackStaged {
            id: AnilistId::new(7),
            title: "Harbour Lights".into(),
            episode: "3".into(),
            episode_title: None,
            speed: 1.0,
        });
        hub.set_status(playing(20.0, 1400.0));
        hub.overlay(&Overlay::SkipOffer { kind: Part::Opening, to: 110.0 });
        let skip = session.card().card.now.unwrap().skip.unwrap();
        assert_eq!((skip.to, skip.label.as_str()), (110.0, "Skip opening"));
        hub.overlay(&Overlay::SkipCleared);
        assert_eq!(session.card().card.now.unwrap().skip, None);

        hub.overlay(&Overlay::Skipped { kind: Part::Opening, from: 30.0, to: 110.0 });
        hub.set_status(playing(110.0, 1400.0));
        assert_eq!(session.card().undo, Some(CastUndo { kind: SkipKind::Opening, to: 30.0 }));
        assert!(session.ticking());
        hub.set_status(playing(115.0, 1400.0));
        assert!(session.card().undo.is_some());
        hub.set_status(playing(116.5, 1400.0));
        assert_eq!(session.card().undo, None, "six seconds played");
        session.undo_skip();
        assert_eq!(sent(), vec![], "nothing to undo");

        hub.overlay(&Overlay::Skipped { kind: Part::Ending, from: 1290.0, to: 1380.0 });
        hub.set_status(playing(1380.0, 1400.0));
        session.undo_skip();
        assert_eq!(sent(), vec![Command::Player(PlayerCommand::UndoSkip { to: 1290.0 })]);
        assert_eq!(session.card().undo, None);
    }

    #[tokio::test]
    async fn rows_say_the_route_of_what_plays_once_it_is_known() {
        let (sender, hub, session, _sent) = casting();
        let route = |sender: &TvSender| sender.cast_rows()[0].route_hint;
        assert_eq!(route(&sender), CastRouteHint::ThroughThisDevice);
        assert_eq!(session.card().route, None);
        hub.set_status(CastStatus {
            plan: Some(plan(CastRoute::Direct { url: "https://cdn.example/3.m3u8".into() })),
            ..playing(1.0, 1400.0)
        });
        assert_eq!(route(&sender), CastRouteHint::Direct);
        assert_eq!(session.card().route, Some(CastRouteHint::Direct));
        hub.end();
        assert_eq!(route(&sender), CastRouteHint::ThroughThisDevice, "the cast is over");
    }

    struct Browser(Arc<Mutex<Vec<&'static str>>>);

    impl CastBrowser for Browser {
        fn start(&self, _: CastSink) {
            lock(&self.0).push("start");
        }

        fn stop(&self) {
            lock(&self.0).push("stop");
        }
    }

    #[tokio::test]
    async fn settings_can_leave_tvs_with_chromecast_built_in_out() {
        let (sender, hub, _sent) = sender();
        let said = Arc::new(Mutex::new(Vec::new()));
        sender.browse_cast(Arc::new(Browser(Arc::clone(&said))));
        hub.found(living_room());
        assert_eq!(sender.cast_rows().len(), 1);

        hub.set_show_tvs(false);
        assert!(sender.cast_rows().is_empty());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while lock(&said).len() < 2 {
            assert!(tokio::time::Instant::now() < deadline, "the browse stopped");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(*lock(&said), ["start", "stop"]);
        hub.found(living_room());
        assert!(hub.devices().is_empty(), "what a browse still says is left out");
        sender.browse_cast(Arc::new(Browser(Arc::clone(&said))));
        assert_eq!(lock(&said).len(), 2, "no browse starts");

        hub.set_show_tvs(true);
        sender.browse_cast(Arc::new(Browser(Arc::clone(&said))));
        hub.found(living_room());
        assert_eq!(sender.cast_rows().len(), 1);
    }

    #[tokio::test]
    async fn try_again_casts_the_same_episode_to_the_same_tv_from_where_it_was() {
        let (_sender, hub, session, mut sent) = casting();
        hub.set_status(playing(42.0, 1400.0));
        hub.failed(CastFailure::NeverFetches);
        let card = session.card();
        assert!(card.card.over);
        assert_eq!(
            card.notice,
            Some(Msg::NoticeCastNeverFetches { name: "Living room".into() })
        );

        assert!(session.try_again());
        assert_eq!(
            sent(),
            vec![Command::CastTo {
                device: "tv-1".into(),
                id: AnilistId::new(7),
                episode: "3".into(),
                position: Some(42.0),
            }]
        );
        let card = session.card();
        assert!(!card.card.over, "the card starts over");
        assert_eq!(card.notice, None);
        assert!(card.phase_line.is_some());
    }

    #[tokio::test]
    async fn try_again_with_nothing_asked_does_nothing() {
        let (sender, hub, mut sent) = sender();
        hub.found(living_room());
        let session = sender.cast("cast:tv-1", AnilistId::new(7), "3".into(), None).unwrap();
        sent();
        assert!(!session.try_again());
        assert_eq!(sent(), vec![]);
    }
}
