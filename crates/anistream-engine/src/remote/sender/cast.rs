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
};

use anistream_cast::{CastDevice, CastState, VolumeControl};
use anistream_core::ids::AnilistId;
use anistream_model::{Msg, command::Command};
use anistream_remote::{
    command::{RemoteCommand, Routed},
    state::Screen,
};
use tokio::task::JoinHandle;

use super::{LinkPhase, NowCard, Problem, RemoteCard, TvSender, lock};
use crate::{cast::CastHub, engine::RemoteSender, playback::link::cast::notice};

/// What a row's key starts with when it is a Cast device: the TXT `id` follows.
const CAST: &str = "cast:";

/// A position closer to the start than this is not carried back to this device.
const CARRY_FROM: f64 = 5.0;

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
    browser: Mutex<Option<Arc<dyn CastBrowser>>>,
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
        let task = tokio::spawn(async move {
            while changes.changed().await.is_ok() {
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
    /// at the same address, which its own row stands for.
    pub fn cast_rows(&self) -> Vec<CastRow> {
        let Some((hub, _)) = self.hub() else { return Vec::new() };
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
                route_hint: CastRouteHint::ThroughThisDevice,
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
        let command = Command::CastTo { device: device.id.clone(), id, episode, position };
        engine.send(command).ok_or(Problem::Other)?;
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
        let view = self.hub.view();
        let ours = view.device.as_ref().is_some_and(|d| d.id == self.device.id);
        let status = view.status.filter(|_| ours);
        let failure = status.as_ref().and_then(|s| s.failure.clone());
        let over = !ours || view.over || (!view.connected && failure.is_some());
        let phase =
            if view.connected && ours { LinkPhase::Connected } else { LinkPhase::Connecting };
        let now = view.state.filter(|_| ours).and_then(|state| state.now_playing).map(|now| {
            let mut card = NowCard::read(&now);
            if let Some(status) = &status {
                card.position = status.position;
                card.duration = status.duration.or(card.duration);
                card.paused = status.state == CastState::Paused;
                card.loading = matches!(
                    status.state,
                    CastState::Connecting | CastState::Loading | CastState::Idle
                ) && status.plan.is_none();
                card.volume = status.level.map(|level| (level * 100.0).round());
            }
            card
        });
        CastCard {
            card: RemoteCard {
                tv_name: self.device.name.clone(),
                phase,
                phase_line: None,
                screen: Screen::Player,
                now,
                rev: 0,
                over,
                reached: view.connected || status.is_some(),
            },
            volume_control: status.as_ref().map_or(VolumeControl::Level, |s| s.volume),
            muted: status.as_ref().and_then(|s| s.muted),
            notice: failure.as_ref().map(notice),
        }
    }

    /// What the card's controls ask: the TV's volume and mute go to the receiver; the rest to
    /// the engine's playback, as the TV's own front end would send it.
    pub fn command(&self, command: RemoteCommand) {
        match command {
            RemoteCommand::Volume { step } => {
                let Some(player) = self.hub.player() else { return };
                if player.volume_control() == VolumeControl::MuteOnly {
                    return;
                }
                let level = (player.volume() + step / 100.0).clamp(0.0, 1.0);
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
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_cast::{
        CastPlayer, CastSetup, CastStatus, PlainHttp, Timings,
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
}
