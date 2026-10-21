//! Playbacks against a scripted player, in any build: the session behind its link, with no mpv
//! and no `desktop` feature needed, as a front end's own player will drive it.

use std::sync::{Arc, Mutex};

use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use anistream_model::{command::PlayerCommand, event::Event, playback::Ending};
use anistream_net::HttpClient;
use anistream_playback::{PlaybackEvent, SkipKind, companion};
use anistream_store::Store;
use serde_json::json;
use tokio::sync::mpsc;

use super::{Machine, PlaySettings, play};
use crate::{
    host::Host,
    playback::{
        EpisodeEntry, NextEpisode, PlaybackContext,
        link::{
            Overlay,
            fake::{Call, ScriptedOpener},
        },
        power::{PowerSource, ReportedPower},
        shaders::{self, Budget, Display, Look, Source},
        slot::PlayerSlot,
    },
};

/// A host that writes down the links it is asked to open, and opens them.
#[derive(Default)]
struct Links(Mutex<Vec<String>>);

impl Host for Links {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.0.lock().unwrap().push(url.to_owned());
        Ok(())
    }

    fn notify(&self, _: &str, _: &str) {}
}

const RUNTIME: f64 = 120.0;

/// A file opening as a player reports it: loaded with English audio picked ahead of the
/// Japanese, its own chapters, and the display it plays on.
fn opening(chapters: &[(&str, f64)]) -> Vec<PlaybackEvent> {
    vec![
        PlaybackEvent::Loaded,
        PlaybackEvent::TrackList(json!([
            { "type": "video", "id": 1, "selected": true },
            { "type": "audio", "id": 1, "lang": "eng", "selected": true },
            { "type": "audio", "id": 2, "lang": "jpn" },
            { "type": "sub", "id": 1, "lang": "eng", "title": "Full" },
        ])),
        PlaybackEvent::Chapters(chapters.iter().map(|(t, s)| ((*t).to_owned(), *s)).collect()),
        PlaybackEvent::Display { width: 1920, height: 1080 },
    ]
}

/// The rest of the file: the picture, `source` wide and high, a playhead from start to end,
/// and the end.
fn playing(source: (u64, u64)) -> Vec<PlaybackEvent> {
    let mut events = vec![PlaybackEvent::VideoParams(json!({
        "w": source.0, "h": source.1, "dw": source.0, "dh": source.1,
        "primaries": "bt.709", "gamma": "bt.1886",
    }))];
    let mut second = 0.0;
    while second < RUNTIME {
        events.push(PlaybackEvent::Progress { position: second, duration: Some(RUNTIME) });
        second += 1.0;
    }
    events.push(PlaybackEvent::Ended { complete: true });
    events
}

/// One whole file as a player reports it.
fn file(chapters: &[(&str, f64)], source: (u64, u64)) -> Vec<PlaybackEvent> {
    [opening(chapters), playing(source)].concat()
}

fn context(episode: &str, next: Option<&str>) -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(900_001),
        mal_id: None,
        episode: episode.into(),
        title: "Stardust Wayfarer".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: None,
        speed: None,
        volume: None,
        next: next.map(|n| NextEpisode { number: n.into(), title: None, thumbnail: None }),
        episodes: (1..=12)
            .map(|n| EpisodeEntry {
                number: n.to_string(),
                title: None,
                watched: false,
                filler: false,
            })
            .collect(),
        cover_url: None,
        banner_url: None,
    }
}

fn stream(kind: StreamKind) -> Stream {
    Stream {
        provider_id: "yamabiko".into(),
        subtitles: vec![Subtitle {
            language: "en".into(),
            url: "file:///fixtures/stardust-wayfarer.en.ass".into(),
            hard: false,
            format: None,
        }],
        ..Stream::new("https://cdn.example/stardust-wayfarer/1.mp4", kind)
            .with_header("Referer", "https://player.example/")
    }
}

fn settings() -> PlaySettings {
    PlaySettings {
        threshold: 0.85,
        auto_skip: false,
        mark_chapters: false,
        fullscreen: true,
        subtitle_language: Some("eng".into()),
        auto_next: true,
        night_mode: false,
        remember_tracks: false,
        look: Look::Natural,
        shader_dir: None,
        tone_mapping: "auto".into(),
        tracker_ids: Vec::new(),
        presence: Default::default(),
        syncplay: Default::default(),
        party: false,
        up_next_countdown: 10,
    }
}

/// What a test plays on: the scripted opener, a slot, the host and the power source.
struct Rig {
    opener: Arc<ScriptedOpener>,
    slot: PlayerSlot,
    links: Arc<Links>,
    power: Arc<dyn PowerSource>,
    store: Store,
    _scratch: tempfile::TempDir,
}

impl Rig {
    fn new(files: Vec<Vec<PlaybackEvent>>) -> Self {
        let scratch = tempfile::tempdir().unwrap();
        Self::with(Arc::new(ScriptedOpener::new(scratch.path(), files)), scratch)
    }

    fn with(opener: Arc<ScriptedOpener>, scratch: tempfile::TempDir) -> Self {
        Self {
            opener,
            slot: PlayerSlot::default(),
            links: Arc::default(),
            power: Arc::new(ReportedPower::default()),
            store: Store::open_in_memory().unwrap(),
            _scratch: scratch,
        }
    }

    /// Play `stream` as `episode`, to its end, and return what the playback said.
    async fn play(
        &self,
        stream: Stream,
        context: PlaybackContext,
        settings: PlaySettings,
    ) -> Vec<Event> {
        let http =
            HttpClient::offline(&anistream_core::config::NetworkConfig::default()).unwrap();
        let machine = Machine {
            opener: self.opener.clone(),
            slot: self.slot.clone(),
            host: self.links.clone(),
            power: self.power.clone(),
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
        let playing = play(
            vec![stream],
            context,
            self.store.clone(),
            http,
            machine,
            settings,
            tx.into(),
            commands,
        );
        tokio::time::timeout(std::time::Duration::from_secs(30), playing)
            .await
            .expect("the playback ends");
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }
}

fn ending(events: &[Event]) -> Option<Ending> {
    events.iter().rev().find_map(|event| match event {
        Event::PlaybackEnded { ending, .. } => Some(ending.clone()),
        _ => None,
    })
}

fn toasts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Toast(notice) => Some(notice.text.clone()),
            _ => None,
        })
        .collect()
}

/// Where in `calls` the first call `matches` accepts is, if anywhere.
fn position(calls: &[Call], matches: impl Fn(&Call) -> bool) -> Option<usize> {
    calls.iter().position(matches)
}

#[tokio::test]
async fn a_playback_opens_a_window_loads_the_file_and_plays_it_to_its_end() {
    let rig = Rig::new(vec![file(&[("Opening", 0.0), ("Part A", 30.0)], (1920, 1080))]);
    let events = rig.play(stream(StreamKind::Mp4), context("1", None), settings()).await;
    let calls = rig.opener.calls();

    assert_eq!(calls.first(), Some(&Call::Open { fullscreen: true }));
    // What the window shows while the file opens, then the file with its own options: the
    // stream's headers, its title, and the sub watcher's languages.
    let opening = position(
        &calls,
        |c| matches!(c, Call::Overlay(Overlay::Starting { card, .. }) if card.phase == companion::Phase::Loading),
    );
    let load = position(&calls, |c| matches!(c, Call::Load { .. })).expect("the file loads");
    assert!(opening.is_some_and(|opening| opening < load), "{calls:#?}");
    let Call::Load { url, options } = &calls[load] else { unreachable!() };
    assert_eq!(url, "https://cdn.example/stardust-wayfarer/1.mp4");
    let option = |key: &str| options.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(option("http-header-fields"), Some("Referer: https://player.example/"));
    assert_eq!(option("force-media-title"), Some("Stardust Wayfarer · Episode 1"));
    assert_eq!(option("alang"), Some("ja,jpn,jp"));
    assert_eq!(calls.get(load - 1), Some(&Call::Overlay(Overlay::ThumbnailUrl(None))));
    // Once loaded: the external subtitles, then the Japanese audio in place of mpv's pick.
    assert!(calls.contains(&Call::AddSubtitle {
        url: "file:///fixtures/stardust-wayfarer.en.ass".into(),
        title: Some("en".into()),
        language: Some("en".into()),
    }));
    assert!(calls.contains(&Call::Set { property: "aid", value: json!(2) }));
    // The opening is offered as a skip from the file's own chapters, and cleared once past.
    assert!(
        calls
            .contains(&Call::Overlay(Overlay::SkipOffer { kind: SkipKind::Opening, to: 30.0 }))
    );
    assert!(calls.contains(&Call::Overlay(Overlay::SkipCleared)));
    // Nothing follows the episode, so the window closes.
    assert_eq!(calls.last(), Some(&Call::Shutdown));
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }));
    // And what was watched is written down.
    let completed = rig.store.completed_episodes(AnilistId::new(900_001)).unwrap();
    assert_eq!(completed, ["1"], "episode 1 is recorded as watched");
}

#[tokio::test]
async fn the_next_episode_plays_in_the_window_the_last_one_left() {
    let rig = Rig::new(vec![file(&[], (1920, 1080)), file(&[], (1920, 1080))]);
    let first = rig.play(stream(StreamKind::Mp4), context("1", Some("2")), settings()).await;
    assert_eq!(ending(&first), Some(Ending::Complete { next: Some("2".into()) }));
    let parked = rig.opener.calls();
    assert!(!parked.contains(&Call::Shutdown), "the window waits for the next episode");
    assert!(matches!(
        parked.last(),
        Some(Call::Overlay(Overlay::Starting { card, episode, .. }))
            if card.text == "Up next · Episode 2" && episode == "2"
    ));

    let second = rig.play(stream(StreamKind::Mp4), context("2", None), settings()).await;
    assert_eq!(ending(&second), Some(Ending::Complete { next: None }));
    let calls = rig.opener.calls();
    let opened = calls.iter().filter(|c| matches!(c, Call::Open { .. })).count();
    assert_eq!(opened, 1, "one window for both episodes");
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Load { .. })).count(), 2);
    assert_eq!(calls.last(), Some(&Call::Shutdown));
}

#[tokio::test]
async fn a_licensed_stream_is_handed_to_the_host_and_no_window_opens() {
    let rig = Rig::new(Vec::new());
    let link =
        Stream::new("https://hikari.example/watch/G2XU04E88", StreamKind::ExternalDeepLink);
    let events = rig.play(link, context("1", None), settings()).await;

    assert_eq!(*rig.links.0.lock().unwrap(), ["https://hikari.example/watch/G2XU04E88"]);
    assert!(rig.opener.calls().is_empty(), "{:?}", rig.opener.calls());
    assert_eq!(toasts(&events), ["opened in your browser"]);
    assert_eq!(ending(&events), Some(Ending::Elsewhere));
}

#[tokio::test]
async fn a_player_that_will_not_open_fails_the_playback_in_words() {
    let scratch = tempfile::tempdir().unwrap();
    let rig = Rig::with(Arc::new(ScriptedOpener::refusing(scratch.path())), scratch);
    let events = rig.play(stream(StreamKind::Mp4), context("1", None), settings()).await;

    assert_eq!(rig.opener.calls(), [Call::Open { fullscreen: true }]);
    assert_eq!(toasts(&events), ["mpv: not installed"]);
    assert_eq!(ending(&events), Some(Ending::Failed));
}

/// The chain the session should settle on for a 720p source on a 1080p display.
fn chain_for(on_battery: bool, dir: &std::path::Path) -> Vec<String> {
    let source = Source { width: 1280, height: 720, hdr: None };
    let display = Display { width: 1920, height: 1080 };
    let chain =
        shaders::choose(Look::Natural, &source, Some(display), Budget { on_battery, eased: 0 });
    shaders::paths(&chain, dir)
}

/// The last chain the window was told to run.
fn last_shaders(calls: &[Call]) -> Option<Vec<String>> {
    calls.iter().rev().find_map(|call| match call {
        Call::Shaders(paths) => Some(paths.clone()),
        _ => None,
    })
}

/// A power source that says `battery`, and tells the test it was asked.
struct Asked {
    battery: bool,
    asked: tokio::sync::Notify,
}

impl PowerSource for Asked {
    fn on_battery(&self) -> Option<bool> {
        self.asked.notify_one();
        Some(self.battery)
    }
}

/// The last chain a window on `battery` power settles on, for a 720p source on a 1080p display.
///
/// The picture is reported only once the battery has been asked, as a real file's is reported
/// some time into it: with every event waiting at once, the session would never wait, and the
/// timer that asks the battery could go unserved until the file had ended.
async fn settled_on(battery: bool, dir: &std::path::Path) -> Option<Vec<String>> {
    let mut rig = Rig::new(vec![opening(&[])]);
    let power = Arc::new(Asked { battery, asked: tokio::sync::Notify::new() });
    rig.power = power.clone();
    let settings = PlaySettings { shader_dir: Some(dir.to_owned()), ..settings() };
    let report = async {
        power.asked.notified().await;
        let window = rig.opener.window().expect("a window is open");
        for event in playing((1280, 720)) {
            let _ = window.send(event);
        }
    };
    tokio::join!(rig.play(stream(StreamKind::Mp4), context("1", None), settings), report);
    last_shaders(&rig.opener.calls())
}

#[tokio::test]
async fn the_chain_goes_by_what_the_power_source_says() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(settled_on(false, dir.path()).await, Some(chain_for(false, dir.path())));
    assert_eq!(settled_on(true, dir.path()).await, Some(chain_for(true, dir.path())));
    assert_ne!(
        chain_for(true, dir.path()),
        chain_for(false, dir.path()),
        "the battery lightens it"
    );
}
