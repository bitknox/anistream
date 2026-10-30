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
use anistream_playback::{PlaybackEvent, ScriptEvent, SkipKind, companion};
use anistream_store::{ServerFailure, ServerHealth, ServerKey, Store};
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
        PlaybackEvent::Display {
            width: 1920,
            height: 1080,
            compute: true,
            refresh_hz: None,
            edr_headroom: None,
        },
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
        episode_total: None,
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

pub(super) fn settings() -> PlaySettings {
    PlaySettings {
        threshold: 0.85,
        auto_skip: false,
        mark_chapters: false,
        skip_button: true,
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
        party: Default::default(),
        up_next_countdown: 10,
        mpv_options: Vec::new(),
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
        self.play_all(vec![stream], context, settings).await
    }

    /// Play `streams`, best first, as resolution hands them over, and return what was said.
    async fn play_all(
        &self,
        streams: Vec<Stream>,
        context: PlaybackContext,
        settings: PlaySettings,
    ) -> Vec<Event> {
        let (_controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
        self.play_steered(streams, context, settings, commands).await
    }

    /// Play `streams` steered by `commands`, which the test sends, and return what was said.
    async fn play_steered(
        &self,
        streams: Vec<Stream>,
        context: PlaybackContext,
        settings: PlaySettings,
        commands: mpsc::UnboundedReceiver<PlayerCommand>,
    ) -> Vec<Event> {
        let http =
            HttpClient::offline(&anistream_core::config::NetworkConfig::default()).unwrap();
        let machine = Machine {
            opener: self.opener.clone(),
            slot: self.slot.clone(),
            host: self.links.clone(),
            power: self.power.clone(),
            offline: Default::default(),
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let playing = play(
            streams,
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
async fn the_seek_bar_is_told_its_marks_and_what_is_held_but_nothing_held_while_paused() {
    let cached = |end: f64| PlaybackEvent::Cached(vec![(0.0, end)]);
    let events = [
        opening(&[("Opening", 0.0), ("Part A", 30.0)]),
        vec![
            cached(10.0),
            PlaybackEvent::Paused(true),
            cached(50.0),
            cached(60.0),
            PlaybackEvent::Paused(false),
        ],
        playing((1920, 1080)),
    ]
    .concat();
    let rig = Rig::new(vec![events]);
    rig.play(stream(StreamKind::Mp4), context("1", None), settings()).await;
    let calls = rig.opener.calls();

    let marks: Vec<_> = calls
        .iter()
        .filter_map(|call| match call {
            Call::Overlay(Overlay::Timeline(marks)) => Some(marks.clone()),
            _ => None,
        })
        .collect();
    assert!(
        marks.last().is_some_and(|marks| marks.len() == 1
            && marks[0].kind == SkipKind::Opening
            && marks[0].start == 0.0
            && marks[0].end == 30.0),
        "the file's opening, once its chapters are in: {marks:#?}"
    );
    let held: Vec<_> = calls
        .iter()
        .filter_map(|call| match call {
            Call::Overlay(Overlay::Buffered(ranges)) => Some(ranges.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(held, [vec![(0.0, 10.0)], vec![(0.0, 60.0)]], "the latest once play carries on");
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
    assert_eq!(toasts(&events), ["Opened in your browser."]);
    assert_eq!(ending(&events), Some(Ending::Elsewhere));
}

#[tokio::test]
async fn a_player_that_will_not_open_fails_the_playback_in_words() {
    let scratch = tempfile::tempdir().unwrap();
    let rig = Rig::with(Arc::new(ScriptedOpener::refusing(scratch.path())), scratch);
    let events = rig.play(stream(StreamKind::Mp4), context("1", None), settings()).await;

    assert_eq!(rig.opener.calls(), [Call::Open { fullscreen: true }]);
    assert_eq!(
        toasts(&events),
        ["Couldn't start mpv. Check that it runs on its own. (not installed)"]
    );
    assert_eq!(ending(&events), Some(Ending::Failed));
}

/// The chain the session should settle on for a 720p source on a 1080p display.
fn chain_for(on_battery: bool, dir: &std::path::Path) -> Vec<String> {
    let source = Source { width: 1280, height: 720, hdr: None, planar: true };
    let display = Display { width: 1920, height: 1080, compute: true };
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

/// The last chain a window settles on for a file whose picture reports `events` once it has
/// opened on `display`.
async fn chain_after(
    display: PlaybackEvent,
    events: Vec<PlaybackEvent>,
    dir: &std::path::Path,
) -> Option<Vec<String>> {
    let mut file = opening(&[]);
    *file.last_mut().expect("the opening ends on the display") = display;
    file.extend(events);
    file.push(PlaybackEvent::Ended { complete: true });
    let rig = Rig::new(vec![file]);
    let settings = PlaySettings { shader_dir: Some(dir.to_owned()), ..settings() };
    rig.play(stream(StreamKind::Mp4), context("1", None), settings).await;
    last_shaders(&rig.opener.calls())
}

/// `video-params` for a picture `size` wide and high, in `pixelformat`.
fn params(size: (u32, u32), pixelformat: &str) -> PlaybackEvent {
    PlaybackEvent::VideoParams(json!({
        "w": size.0, "h": size.1, "dw": size.0, "dh": size.1, "pixelformat": pixelformat,
        "primaries": "bt.709", "gamma": "bt.1886",
    }))
}

const P720: (u32, u32) = (1280, 720);
const P480: (u32, u32) = (854, 480);

/// The chain for a source `size` wide and high on a 1080p display, as `choose` gives it.
fn chosen(
    size: (u32, u32),
    planar: bool,
    compute: bool,
    eased: u8,
    dir: &std::path::Path,
) -> Vec<String> {
    let source = Source { width: size.0, height: size.1, hdr: None, planar };
    let display = Display { width: 1920, height: 1080, compute };
    let budget = Budget { on_battery: false, eased };
    shaders::paths(&shaders::choose(Look::Natural, &source, Some(display), budget), dir)
}

/// A 1080p display, its renderer with compute shaders or without.
fn display(compute: bool) -> PlaybackEvent {
    PlaybackEvent::Display {
        width: 1920,
        height: 1080,
        compute,
        refresh_hz: None,
        edr_headroom: None,
    }
}

#[tokio::test]
async fn a_renderer_without_compute_gets_no_artcnn() {
    let dir = tempfile::tempdir().unwrap();
    let settled = chain_after(display(false), vec![params(P720, "yuv420p")], dir.path()).await;
    assert_eq!(settled, Some(chosen(P720, true, false, 0, dir.path())));
    assert_ne!(settled, Some(chosen(P720, true, true, 0, dir.path())), "ArtCNN is left out");
}

#[tokio::test]
async fn a_picture_that_turns_zero_copy_drops_the_hooks_that_read_planes() {
    let dir = tempfile::tempdir().unwrap();
    // Software frames first, as a first-frame fallback gives, then the decoder's own.
    let events = vec![params(P720, "yuv420p"), params(P720, "mediacodec")];
    let settled = chain_after(display(true), events, dir.path()).await;
    assert_eq!(settled, Some(chosen(P720, false, true, 0, dir.path())));
    assert_eq!(settled, Some(Vec::new()), "nothing for a 720p picture on a 1080p display");
}

#[tokio::test]
async fn a_hook_the_renderer_disabled_steps_the_chain_down_once() {
    let dir = tempfile::tempdir().unwrap();
    // AniSD, as the emulator's GLES fails it: two of its hooks, the second said after the
    // chain had already stepped down to ArtCNN C4F16 for the first.
    let anisd =
        |pass: u8| PlaybackEvent::HookDisabled(format!("ArtCNN C4F32 (Conv2D-{pass}-ReLU)"));
    let events = vec![params(P480, "yuv420p"), anisd(1), anisd(2)];
    let settled = chain_after(display(true), events, dir.path()).await;
    assert_eq!(settled, Some(chosen(P480, true, true, 1, dir.path())));
    assert_ne!(settled, Some(chosen(P480, true, true, 0, dir.path())));
    assert_ne!(settled, Some(chosen(P480, true, true, 2, dir.path())), "stepped once");

    // A hook nothing here describes cannot be placed in a chain: it eases once, and no more.
    let unknown = || PlaybackEvent::HookDisabled("user shader".into());
    let events = vec![params(P480, "yuv420p"), unknown(), unknown()];
    let settled = chain_after(display(true), events, dir.path()).await;
    assert_eq!(settled, Some(chosen(P480, true, true, 1, dir.path())));
}

/// The fixture's stream, served from `host`.
fn served_from(host: &str) -> Stream {
    Stream {
        url: format!("https://{host}/stardust-wayfarer/1.mp4?token=signed"),
        ..stream(StreamKind::Mp4)
    }
}

/// What the rig's store knows of `host`, as the fixture's provider streams from it.
fn server(rig: &Rig, host: &str) -> Option<ServerHealth> {
    let key = ServerKey::of("yamabiko", &format!("https://{host}")).unwrap();
    rig.store.server_health(&key, anistream_store::now()).unwrap()
}

#[tokio::test]
async fn each_attempt_is_written_down_against_its_server() {
    // The first server's file ends in an error before a frame; the next one plays.
    let failing = vec![PlaybackEvent::Loaded, PlaybackEvent::Ended { complete: false }];
    let rig = Rig::new(vec![failing, file(&[], (1920, 1080))]);
    let streams = vec![served_from("cdn-a.example"), served_from("cdn-b.example")];
    let events = rig.play_all(streams, context("1", None), settings()).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }));

    let a = server(&rig, "cdn-a.example").expect("the server that failed has a record");
    assert_eq!((a.successes, a.failures, a.consecutive_failures), (0, 1, 1));
    assert_eq!(a.last_failure, Some(ServerFailure::Load));
    let b = server(&rig, "cdn-b.example").expect("the server that played has a record");
    assert_eq!((b.successes, b.failures, b.consecutive_failures), (1, 0, 0));
    assert!(b.first_frame.is_some(), "with how long its first frame took");
}

#[tokio::test]
async fn a_stall_the_viewer_switched_away_from_counts_against_its_server() {
    // It played, then held waiting for data until the viewer took the stall prompt's offer.
    let stalling = vec![
        PlaybackEvent::Loaded,
        PlaybackEvent::Progress { position: 0.0, duration: Some(RUNTIME) },
        PlaybackEvent::Progress { position: 1.0, duration: Some(RUNTIME) },
        PlaybackEvent::Buffering(true),
        PlaybackEvent::Script(ScriptEvent::StallSwitch),
    ];
    let rig = Rig::new(vec![stalling, file(&[], (1920, 1080))]);
    let streams = vec![served_from("cdn-a.example"), served_from("cdn-b.example")];
    rig.play_all(streams, context("1", None), settings()).await;

    let a = server(&rig, "cdn-a.example").unwrap();
    assert_eq!((a.successes, a.failures, a.consecutive_failures), (1, 1, 1));
    assert_eq!(a.last_failure, Some(ServerFailure::Stalled));
    assert_eq!(server(&rig, "cdn-b.example").unwrap().successes, 1);
}

#[tokio::test]
async fn the_sources_menu_says_which_server_has_been_failing() {
    let rig = Rig::new(vec![file(&[], (1920, 1080))]);
    let failing = ServerKey::of("yamabiko", "https://cdn-a.example").unwrap();
    for _ in 0..2 {
        rig.store
            .record_server_failure(&failing, ServerFailure::Http(502), anistream_store::now())
            .unwrap();
    }
    // As resolution hands them over: the server that has been failing last.
    let streams = vec![served_from("cdn-b.example"), served_from("cdn-a.example")];
    rig.play_all(streams, context("1", None), settings()).await;

    let menus = rig.opener.calls().into_iter().find_map(|call| match call {
        Call::Overlay(Overlay::Menus(menus)) => Some(menus),
        _ => None,
    });
    let notes: Vec<Option<String>> =
        menus.expect("the menus").sources.into_iter().map(|s| s.health).collect();
    assert_eq!(notes, [None, Some("failed the last 2 times · just now".into())]);
}

/// A playback at a watch party, as the party sees it: its seat taken, what the room asks of
/// the player done, and what the player does reported. The file ends after `last` seconds,
/// played to its end when `complete`.
async fn at_a_party(
    complete: bool,
    last: f64,
) -> (Rig, Vec<Event>, Vec<crate::playback::seat::SeatReport>) {
    use crate::playback::seat::{PartyHub, SeatCue, SeatMessage};

    let hub = PartyHub::default();
    let mut party = hub.open();
    let rig = Rig::new(vec![opening(&[])]);
    let settings = PlaySettings { party: hub.clone(), ..settings() };
    let driving = async {
        let Some(SeatMessage::Seated(cues)) = party.recv().await else { panic!("no seat") };
        for cue in
            [SeatCue::Pause(true), SeatCue::Seek(42.0), SeatCue::Notice("Mika paused".into())]
        {
            cues.send(cue).unwrap();
        }
        let asked = |calls: &[Call]| {
            calls.contains(&Call::Set { property: "pause", value: json!(true) })
                && calls.contains(&Call::Seek { seconds: 42.0, absolute: true })
                && calls.iter().any(|c| matches!(c, Call::Overlay(Overlay::Toast { text, .. }) if text == "Mika paused"))
        };
        for _ in 0..200 {
            if asked(&rig.opener.calls()) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(asked(&rig.opener.calls()), "{:#?}", rig.opener.calls());
        let window = rig.opener.window().expect("a window is open");
        // A few seconds in, the viewer pauses and plays on; the party hears both.
        for second in 0..=last as u32 {
            let position = f64::from(second);
            window.send(PlaybackEvent::Progress { position, duration: Some(RUNTIME) }).unwrap();
            if second == 3 {
                window.send(PlaybackEvent::Paused(true)).unwrap();
                window.send(PlaybackEvent::Paused(false)).unwrap();
            }
        }
        window.send(PlaybackEvent::Ended { complete }).unwrap();
        let mut reports = Vec::new();
        while let Some(message) = party.recv().await {
            if let SeatMessage::Report(report) = message {
                let ended = matches!(report, crate::playback::seat::SeatReport::Ended { .. });
                reports.push(report);
                if ended {
                    break;
                }
            }
        }
        reports
    };
    let (events, reports) = tokio::join!(
        rig.play(stream(StreamKind::Mp4), context("1", Some("2")), settings),
        driving
    );
    (rig, events, reports)
}

#[tokio::test]
async fn a_party_steers_the_player_and_hears_what_it_does() {
    use crate::playback::seat::SeatReport;

    let (rig, events, reports) = at_a_party(true, RUNTIME - 1.0).await;
    let id = AnilistId::new(900_001);
    assert!(
        matches!(reports.first(), Some(SeatReport::Playing { id: played, episode, name, .. })
            if *played == id && episode == "1" && name == "Stardust Wayfarer · Episode 1"),
        "{reports:?}"
    );
    assert!(reports.iter().any(|r| matches!(r, SeatReport::Sample { paused: true, .. })));
    assert!(reports.iter().any(|r| matches!(r, SeatReport::Sample { paused: false, .. })));
    // Played to its end, it stays in the party and is marked watched; with the party there is
    // no rolling on to the next episode.
    assert_eq!(reports.last(), Some(&SeatReport::Ended { leave: false }));
    assert_eq!(rig.store.completed_episodes(id).unwrap(), ["1"]);
    assert!(
        !events.iter().any(|e| matches!(e, Event::PlaybackEnded { rolling_on: true, .. })),
        "{events:?}"
    );
}

#[tokio::test]
async fn a_party_play_left_part_way_keeps_no_position() {
    let (rig, _, reports) = at_a_party(false, 50.0).await;
    assert!(
        matches!(reports.last(), Some(crate::playback::seat::SeatReport::Ended { .. })),
        "{reports:?}"
    );
    let id = AnilistId::new(900_001);
    assert_eq!(rig.store.resume_position(id, "1").unwrap(), None);
    assert!(rig.store.completed_episodes(id).unwrap().is_empty());
}

/// Until `calls` holds a call `matches` accepts, for a test that reports more once the session
/// has done something.
async fn until_called(rig: &Rig, matches: impl Fn(&Call) -> bool) {
    for _ in 0..500 {
        if rig.opener.calls().iter().any(&matches) {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("never called: {:#?}", rig.opener.calls());
}

fn seeks(calls: &[Call]) -> Vec<(f64, bool)> {
    calls
        .iter()
        .filter_map(|call| match call {
            Call::Seek { seconds, absolute } => Some((*seconds, *absolute)),
            _ => None,
        })
        .collect()
}

/// A held arrow key sends seeks while the next episode is still being found and while its file
/// opens. They were aimed at what was on screen then, the episode before or the starting
/// card, and reach this playback's controls only because they are the newest: once its file
/// opened they all landed on it at once and ran it to its end, and the next, and the next.
#[tokio::test]
async fn seeks_sent_before_the_file_loads_are_let_go() {
    let rig = Rig::new(vec![Vec::new()]);
    let (controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
    for command in
        [PlayerCommand::Seek(5.0), PlayerCommand::Seek(5.0), PlayerCommand::SeekTo(80.0)]
    {
        controls.send(command).unwrap();
    }
    // Read after the seeks, so once it is applied they have been read too.
    controls.send(PlayerCommand::Volume(2.0)).unwrap();
    let driving = async {
        until_called(&rig, |call| matches!(call, Call::Add { property: "volume", .. })).await;
        let window = rig.opener.window().expect("a window is open");
        for event in file(&[], (1920, 1080)) {
            let _ = window.send(event);
        }
    };
    let (events, ()) = tokio::join!(
        rig.play_steered(
            vec![stream(StreamKind::Mp4)],
            context("1", None),
            settings(),
            commands
        ),
        driving
    );
    assert_eq!(seeks(&rig.opener.calls()), [], "{:#?}", rig.opener.calls());
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }));
    drop(controls);
}

/// Forward past the end stops short of it, and the episode ends on its own from there, rather
/// than mpv ending the file at once and the seeks after it landing on the next one.
#[tokio::test]
async fn a_seek_forward_past_the_end_stops_short_of_it() {
    let rig = Rig::new(vec![opening(&[])]);
    let (controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
    let driving = async {
        until_called(&rig, |call| matches!(call, Call::Load { .. })).await;
        let window = rig.opener.window().expect("a window is open");
        window
            .send(PlaybackEvent::Progress { position: 100.0, duration: Some(RUNTIME) })
            .unwrap();
        until_called(&rig, |call| *call == Call::Overlay(Overlay::Playing)).await;
        // Inside the file, as it is.
        controls.send(PlayerCommand::Seek(5.0)).unwrap();
        until_called(&rig, |call| matches!(call, Call::Seek { .. })).await;
        window
            .send(PlaybackEvent::Progress { position: 118.5, duration: Some(RUNTIME) })
            .unwrap();
        controls.send(PlayerCommand::Volume(1.0)).unwrap();
        until_called(&rig, |call| matches!(call, Call::Add { property: "volume", .. })).await;
        // Past it, from 118.5 s of 120: as far as the end allows, however often.
        controls.send(PlayerCommand::Seek(5.0)).unwrap();
        controls.send(PlayerCommand::Seek(5.0)).unwrap();
        controls.send(PlayerCommand::Volume(1.0)).unwrap();
        for _ in 0..500 {
            let added = rig
                .opener
                .calls()
                .iter()
                .filter(|call| matches!(call, Call::Add { property: "volume", .. }))
                .count();
            if added == 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        window.send(PlaybackEvent::Ended { complete: true }).unwrap();
    };
    tokio::join!(
        rig.play_steered(
            vec![stream(StreamKind::Mp4)],
            context("1", None),
            settings(),
            commands
        ),
        driving
    );
    let seeks = seeks(&rig.opener.calls());
    assert_eq!(seeks[0], (5.0, false), "{seeks:?}");
    assert!(
        seeks[1..].iter().all(|&(to, absolute)| absolute && to == RUNTIME - 1.0),
        "past the end: {seeks:?}"
    );
    assert_eq!(seeks.len(), 3, "{seeks:?}");
}
