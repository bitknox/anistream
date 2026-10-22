//! A player the front end hosts, as the session's link: libmpv in a native app, driven as mpv's
//! process is. What the session tells it goes out as `Event::PlayerOps` and `Event::PlayerUi`,
//! named with the playback driving it; what the player reports comes back as
//! `Command::PlayerReport` and is read as mpv's events are, by the same parsers. See
//! [`anistream_model::hosted`] for the protocol as a front end sees it.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
    },
};

use anistream_core::{media::Translation, traits::PlaybackRequest};
use anistream_model::{
    event::Event,
    hosted::{
        MenuChoice, MenuChoices, MenuEpisode, MenuSource, MpvOption, MpvValue, PlayerEnd,
        PlayerOp, PlayerReport, PlayerUi,
    },
    notice::NoticeKind,
    playback::{NextEpisode, PlaybackId},
};
use anistream_playback::{
    companion::{Level, Menus},
    protocol::{self, EndReason, Reader},
};
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{LinkError, Opened, Overlay, PlaybackEvent, PlayerLink, PlayerOpener};
use crate::{
    outbox::Outbox,
    playback::{STILL_WATCHING_AFTER, offered, power::ReportedPower},
};

/// The windows a front end hosts, by the playbacks driving them, and where what they report
/// goes: the opener the session opens a hosted player with, and the one place a report is
/// taken in.
pub struct HostedPlayers {
    windows: Arc<Windows>,
    /// Where a playback keeps its scratch files: the subtitles fetched ahead of it, which the
    /// player is told to load from there.
    scratch: PathBuf,
    /// What the device says it runs on, which the upscaling chain goes by.
    power: Arc<ReportedPower>,
}

/// The windows open, by every playback that has driven one. A window's playbacks stay with it
/// until it closes, so a report that crosses the handover to the next episode still lands.
#[derive(Default)]
struct Windows(Mutex<HashMap<PlaybackId, Arc<Window>>>);

impl Windows {
    fn lock(&self) -> MutexGuard<'_, HashMap<PlaybackId, Arc<Window>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Forget `window`, under every playback it was driven by.
    fn forget(&self, window: &Arc<Window>) {
        self.lock().retain(|_, open| !Arc::ptr_eq(open, window));
    }
}

/// One hosted window, as the engine knows it.
#[derive(Default)]
struct Window {
    /// Where the session reads what the player reports; `None` once it has closed, which ends
    /// what the session reads.
    events: Mutex<Option<mpsc::UnboundedSender<PlaybackEvent>>>,
    /// Its reports as the session reads them, as for mpv: positions with their duration.
    reader: Mutex<Reader>,
    /// Closed is closed: the player reported so, or the engine closed it.
    closed: AtomicBool,
    /// The player said the file ended because it was told to quit, or the viewer closed it.
    quit: AtomicBool,
}

impl HostedPlayers {
    /// No windows yet, with scratch files in `scratch`, and what the device reports of its
    /// power kept in `power`.
    pub fn new(scratch: impl Into<PathBuf>, power: Arc<ReportedPower>) -> Self {
        Self { windows: Arc::default(), scratch: scratch.into(), power }
    }

    /// Take in what a hosted player reports of `playback`, as mpv's socket is read: at once,
    /// whatever the engine is doing. A report for a playback no open window plays is let go:
    /// the window has closed, or the report crossed its closing.
    pub fn report(&self, playback: PlaybackId, report: PlayerReport) {
        if let PlayerReport::Power { on_battery, low_power } = report {
            // The device's, whichever playback said it: low power mode spends as a battery does.
            self.power.report(Some(on_battery || low_power));
            return;
        }
        let window = self.windows.lock().get(&playback).cloned();
        let Some(window) = window else {
            tracing::debug!(playback = playback.get(), "a report for no open player");
            return;
        };
        if report == PlayerReport::Closed {
            window.close();
            self.windows.forget(&window);
        } else {
            window.take(report);
        }
    }
}

#[async_trait]
impl PlayerOpener for HostedPlayers {
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError> {
        let (events, rx) = mpsc::unbounded_channel();
        let window = Arc::new(Window { events: Mutex::new(Some(events)), ..Window::default() });
        let open = PlayerOp::Open {
            fullscreen: request.fullscreen,
            speed: request.speed,
            volume: request.volume,
        };
        let link = HostedLink {
            window,
            windows: Arc::clone(&self.windows),
            driver: Mutex::default(),
            opening: Mutex::new(Some(open)),
        };
        Ok((Box::new(link), rx))
    }

    fn scratch_dir(&self) -> &Path {
        &self.scratch
    }
}

impl Window {
    /// A report, read as mpv's event would be and handed to the session.
    fn take(&self, report: PlayerReport) {
        let read = match report {
            PlayerReport::FileLoaded => self.read(protocol::Event::FileLoaded),
            PlayerReport::Property { name, json } => match serde_json::from_str(&json) {
                Ok(value) => self.read(protocol::property_change(Some(&name), Some(value))),
                Err(e) => {
                    tracing::debug!(%name, error = %e, "a property that is not JSON");
                    None
                }
            },
            PlayerReport::Ended { reason } => {
                let reason = end_reason(reason);
                // Before the ending is read, so whoever reads it knows the window goes with it.
                if reason == EndReason::Quit {
                    self.quit.store(true, Ordering::Relaxed);
                }
                self.read(protocol::Event::EndFile(reason))
            }
            PlayerReport::Display { width, height, compute, .. } => {
                Some(PlaybackEvent::Display { width, height, compute })
            }
            PlayerReport::HookDisabled { hook } => Some(PlaybackEvent::HookDisabled(hook)),
            PlayerReport::Closed | PlayerReport::Power { .. } => None,
        };
        if let Some(event) = read
            && let Some(events) = lock(&self.events).as_ref()
        {
            let _ = events.send(event);
        }
    }

    fn read(&self, event: protocol::Event) -> Option<PlaybackEvent> {
        lock(&self.reader).read(event)
    }

    /// The player has gone. As mpv's socket closing reads: a file whose ending was never
    /// reported was left, not finished, and nothing more comes from it.
    fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        if let Some(events) = lock(&self.events).take() {
            let _ = events.send(PlaybackEvent::Ended { complete: false });
        }
    }
}

/// A hosted window, as the session's link: each call one [`PlayerOp`] or [`PlayerUi`] event for
/// the playback driving it.
pub struct HostedLink {
    window: Arc<Window>,
    windows: Arc<Windows>,
    /// The playback driving the window, and the outbox it answers on.
    driver: Mutex<Option<(PlaybackId, Outbox)>>,
    /// The op that shows the player, said once the first playback drives it.
    opening: Mutex<Option<PlayerOp>>,
}

impl HostedLink {
    /// Say `event` for the playback driving the window.
    fn say(&self, event: impl FnOnce(PlaybackId) -> Event) -> Result<(), LinkError> {
        if self.window.closed.load(Ordering::Relaxed) {
            return Err(LinkError("the player has closed".into()));
        }
        let Some((playback, tx)) = lock(&self.driver).clone() else {
            return Err(LinkError("no playback drives the player yet".into()));
        };
        tx.send(event(playback))
            .map_err(|_| LinkError("nobody reads the engine any more".into()))
    }

    fn op(&self, op: PlayerOp) -> Result<(), LinkError> {
        self.say(|playback| Event::PlayerOps { playback, ops: vec![op] })
    }
}

#[async_trait]
impl PlayerLink for HostedLink {
    fn attach(&self, tx: &Outbox) {
        // An outbox answers for a playback whenever the engine starts one; only a test's
        // stands for none.
        let playback = tx.playback().unwrap_or(PlaybackId::new(0));
        *lock(&self.driver) = Some((playback, tx.clone()));
        self.windows.lock().insert(playback, Arc::clone(&self.window));
        if let Some(open) = lock(&self.opening).take() {
            let _ = self.op(open);
        }
    }

    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        let options =
            options.into_iter().map(|(name, value)| MpvOption { name, value }).collect();
        self.op(PlayerOp::Load { url: url.to_owned(), options })
    }

    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError> {
        self.op(PlayerOp::AddSubtitle { url: url.to_owned(), title, language })
    }

    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError> {
        self.op(PlayerOp::Set { property: property.to_owned(), value: mpv_value(value) })
    }

    async fn cycle(&self, property: &'static str) -> Result<(), LinkError> {
        self.op(PlayerOp::Cycle { property: property.to_owned() })
    }

    async fn toggle_fullscreen(&self) -> Result<(), LinkError> {
        // The host's window, not a property of the mpv inside it.
        self.op(PlayerOp::ToggleFullscreen)
    }

    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError> {
        self.op(PlayerOp::Add { property: property.to_owned(), delta })
    }

    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError> {
        self.op(PlayerOp::Seek { seconds, absolute })
    }

    async fn shaders(&self, paths: &[String]) -> Result<(), LinkError> {
        self.op(PlayerOp::Shaders { paths: paths.to_vec() })
    }

    async fn audio_filter(&self, add: bool, filter: &str) -> Result<(), LinkError> {
        self.op(PlayerOp::AudioFilter { add, filter: filter.to_owned() })
    }

    async fn restate_chapters(&self) -> Result<(), LinkError> {
        self.op(PlayerOp::RestateChapters)
    }

    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        match ui(overlay) {
            Some(ui) => self.say(|playback| Event::PlayerUi { playback, ui }),
            None => Ok(()),
        }
    }

    async fn stop(&self) -> Result<(), LinkError> {
        self.op(PlayerOp::Stop)
    }

    async fn quit(&self) -> Result<(), LinkError> {
        self.op(PlayerOp::Quit)
    }

    fn is_alive(&self) -> bool {
        !self.window.closed.load(Ordering::Relaxed)
    }

    fn has_quit(&self) -> bool {
        self.window.quit.load(Ordering::Relaxed)
    }

    async fn diagnostics(&self) -> Option<String> {
        None
    }

    async fn shutdown(self: Box<Self>) {
        let _ = self.op(PlayerOp::Close);
        self.window.closed.store(true, Ordering::Relaxed);
        lock(&self.window.events).take();
        self.windows.forget(&self.window);
    }
}

/// What a hosted player's own interface is told of `overlay`, if anything: a playing file is
/// what the player itself knows.
fn ui(overlay: Overlay) -> Option<PlayerUi> {
    Some(match overlay {
        Overlay::Starting {
            id, title, episode, episode_title, cover_url, banner_url, ..
        } => PlayerUi::Starting { id, title, episode, episode_title, cover_url, banner_url },
        Overlay::Playing => return None,
        Overlay::Episode(info) => PlayerUi::Episode {
            number: info.number,
            label: info.label,
            title: info.title,
            show: info.show,
            translation: translation(info.translation),
        },
        Overlay::Next { card: Some(card), still, countdown } => PlayerUi::UpNext {
            next: NextEpisode { number: card.number, title: card.title, thumbnail: still },
            auto: card.auto,
            ending_at: card.at,
            countdown,
            still_watching_after: STILL_WATCHING_AFTER,
        },
        Overlay::Next { card: None, .. } => PlayerUi::UpNextCleared,
        Overlay::Resumed { position } => PlayerUi::Resumed { position },
        Overlay::Menus(menus) => menus_ui(menus),
        Overlay::Media(media) => {
            PlayerUi::Picture { upscaler: media.upscaler, hdr: media.hdr.map(str::to_owned) }
        }
        Overlay::ThumbnailUrl(url) => PlayerUi::Thumbnails { url },
        Overlay::SkipOffer { kind, to } => PlayerUi::Skip { kind: offered(kind), to },
        Overlay::SkipCleared => PlayerUi::SkipCleared,
        Overlay::Skipped { kind, from, .. } => {
            PlayerUi::AutoSkipped { kind: offered(kind), undo_to: from }
        }
        Overlay::Toast { text, level } => PlayerUi::Alert {
            text,
            kind: match level {
                Level::Info => NoticeKind::Info,
                Level::Alert => NoticeKind::Alert,
            },
        },
        Overlay::Stall { seconds } => PlayerUi::Stall { seconds },
        Overlay::StallCleared => PlayerUi::StallCleared,
    })
}

fn menus_ui(menus: Menus) -> PlayerUi {
    let Menus { episodes, sources, translation: current, upscaling, tone_mapping, night_mode } =
        menus;
    PlayerUi::Menus {
        episodes: episodes
            .into_iter()
            .map(|e| MenuEpisode {
                number: e.number,
                label: e.label,
                title: e.title,
                watched: e.watched,
                filler: e.filler,
                current: e.current,
            })
            .collect(),
        sources: sources
            .into_iter()
            .map(|s| MenuSource {
                id: s.id,
                label: s.label,
                detail: s.detail,
                current: s.current,
                health: s.health,
            })
            .collect(),
        translation: translation(current.current),
        upscaling: choices(&upscaling)
            .unwrap_or(MenuChoices { current: String::new(), options: Vec::new() }),
        tone_mapping: choices(&tone_mapping),
        night_mode,
    }
}

/// A menu of choices as the companion is given it, `{current, options: [{id, label, hint}]}`, or
/// `None` where there is no such menu.
fn choices(menu: &serde_json::Value) -> Option<MenuChoices> {
    let text = |value: &serde_json::Value, key: &str| {
        value.get(key).and_then(serde_json::Value::as_str).map(str::to_owned)
    };
    let options = menu.get("options")?.as_array()?;
    Some(MenuChoices {
        current: text(menu, "current")?,
        options: options
            .iter()
            .filter_map(|option| {
                Some(MenuChoice {
                    id: text(option, "id")?,
                    label: text(option, "label")?,
                    hint: text(option, "hint"),
                })
            })
            .collect(),
    })
}

/// The translation the companion names.
fn translation(name: &str) -> Translation {
    if name == "dub" { Translation::Dub } else { Translation::Sub }
}

/// mpv's own reasons, as the parsers read them.
fn end_reason(reason: PlayerEnd) -> EndReason {
    match reason {
        PlayerEnd::Eof => EndReason::Eof,
        PlayerEnd::Quit => EndReason::Quit,
        PlayerEnd::Stop => EndReason::Stop,
        PlayerEnd::Redirect => EndReason::Redirect,
        PlayerEnd::Error => EndReason::Error,
        PlayerEnd::Unknown => EndReason::Unknown,
    }
}

/// A property's value in the format libmpv takes it in.
fn mpv_value(value: serde_json::Value) -> MpvValue {
    match value {
        serde_json::Value::Bool(flag) => MpvValue::Flag(flag),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(int) => MpvValue::Int(int),
            None => MpvValue::Double(n.as_f64().unwrap_or_default()),
        },
        serde_json::Value::String(text) => MpvValue::String(text),
        node => MpvValue::Node(node.to_string()),
    }
}

/// A lock that a panic elsewhere does not poison for good: each critical section is one read
/// or one write.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::playback::{power::PowerSource, shaders};

    #[test]
    fn a_value_goes_in_the_format_libmpv_takes_it_in() {
        assert_eq!(mpv_value(json!(true)), MpvValue::Flag(true));
        assert_eq!(mpv_value(json!(2)), MpvValue::Int(2));
        assert_eq!(mpv_value(json!(1.5)), MpvValue::Double(1.5));
        assert_eq!(mpv_value(json!("bt.2390")), MpvValue::String("bt.2390".into()));
        assert_eq!(
            mpv_value(json!([{ "title": "Opening", "time": 0.0 }])),
            MpvValue::Node(r#"[{"time":0.0,"title":"Opening"}]"#.into())
        );
    }

    #[test]
    fn the_menus_choices_read_as_the_companion_is_given_them() {
        let looks = choices(&shaders::menu(shaders::Look::Natural)).expect("the looks");
        assert_eq!(looks.current, "natural");
        let ids: Vec<_> = looks.options.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, ["natural", "sharp", "off"]);
        assert_eq!(looks.options[2].hint.as_deref(), Some("The source as it is"));
        // No curves while the picture is not HDR.
        assert_eq!(choices(&serde_json::Value::Null), None);
    }

    #[test]
    fn the_device_says_what_powers_it_whichever_playback_it_names() {
        let power = Arc::new(ReportedPower::default());
        let players = HostedPlayers::new("scratch", Arc::clone(&power));
        let low = PlayerReport::Power { on_battery: false, low_power: true };
        players.report(PlaybackId::new(9), low);
        assert_eq!(power.on_battery(), Some(true), "low power spends as a battery does");
        players.report(
            PlaybackId::new(9),
            PlayerReport::Power { on_battery: false, low_power: false },
        );
        assert_eq!(power.on_battery(), Some(false));
    }

    #[tokio::test]
    async fn a_player_that_closes_ends_its_file_unwatched_and_hears_no_more() {
        let players = HostedPlayers::new("scratch", Arc::default());
        let request = PlaybackRequest::default();
        let (link, mut events) = players.open(&request).await.unwrap();
        let (tx, mut said) = mpsc::unbounded_channel();
        link.attach(&tx.into());
        // The player is shown as the first playback drives it.
        let Some(Event::PlayerOps { ops, .. }) = said.recv().await else { panic!("an op") };
        assert!(matches!(ops[..], [PlayerOp::Open { .. }]));

        let playback = PlaybackId::new(0);
        players.report(
            playback,
            PlayerReport::Property { name: "time-pos".into(), json: "4".into() },
        );
        assert_eq!(
            events.recv().await,
            Some(PlaybackEvent::Progress { position: 4.0, duration: None })
        );
        players.report(playback, PlayerReport::Closed);
        assert_eq!(events.recv().await, Some(PlaybackEvent::Ended { complete: false }));
        assert_eq!(events.recv().await, None, "nothing more comes");
        assert!(!link.is_alive());
        assert!(link.seek(1.0, true).await.is_err(), "a closed player is told nothing");
        // A report crossing the closing is let go.
        players.report(playback, PlayerReport::FileLoaded);
    }
}
