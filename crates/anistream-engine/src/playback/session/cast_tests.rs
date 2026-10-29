//! A playback cast to the fake receiver on loopback: the session drives the TV as it drives a
//! window, the next episode plays on the TV already showing it, and a stream the TV can't play
//! says why in the cast's own words.

use std::{sync::Arc, time::Duration};

use anistream_cast::{
    CastDevice, PlainHttp, Timings,
    aac::FakeEncoder,
    fake::{
        FakeOptions, FakeReceiver,
        matroska::{Fixture, FixtureAudio, matroska},
    },
};
use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use anistream_model::{Msg, command::PlayerCommand, event::Event, playback::Ending};
use anistream_net::HttpClient;
use anistream_store::Store;
use tokio::sync::mpsc;

use super::{Machine, play, scripted::settings};
use crate::{
    cast::CastHub,
    host::Host,
    playback::{
        EpisodeEntry, NextEpisode, PlaybackContext,
        link::{PlayerTarget, cast::CastOpener},
        power::ReportedPower,
        slot::PlayerSlot,
    },
};

struct NoHost;

impl Host for NoHost {
    fn open_url(&self, _: &str) -> Result<(), String> {
        Ok(())
    }

    fn notify(&self, _: &str, _: &str) {}
}

fn timings() -> Timings {
    Timings {
        poll: Duration::from_millis(100),
        movement: Duration::from_millis(1500),
        fetch: Duration::from_secs(2),
        idle_close: Duration::from_secs(600),
    }
}

fn context(episode: &str, next: Option<&str>) -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(900_002),
        mal_id: None,
        episode: episode.into(),
        title: "Stardust Wayfarer".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: None,
        speed: None,
        volume: None,
        next: next.map(|n| NextEpisode { number: n.into(), title: None, thumbnail: None }),
        episodes: vec![EpisodeEntry {
            number: "2".into(),
            title: Some("Low Tide".into()),
            watched: false,
            filler: false,
        }],
        cover_url: None,
        banner_url: Some("https://art.example/banner.jpg".into()),
        episode_total: None,
    }
}

/// A TV on loopback and what casts to it.
struct Rig {
    receiver: FakeReceiver,
    hub: Arc<CastHub>,
    opener: Arc<CastOpener>,
    slot: PlayerSlot,
    store: Store,
    dir: tempfile::TempDir,
}

impl Rig {
    async fn new(encoder: bool) -> Self {
        let receiver =
            FakeReceiver::start(FakeOptions { speed: 8.0, ..FakeOptions::default() })
                .await
                .unwrap();
        let hub = Arc::new(CastHub::with(None, Arc::new(PlainHttp), timings()));
        if encoder {
            hub.set_encoder(Arc::new(FakeEncoder::default()));
        }
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        hub.begin(device.clone());
        let dir = tempfile::tempdir().unwrap();
        let opener = Arc::new(CastOpener::new(Arc::clone(&hub), device, dir.path().to_owned()));
        Self {
            receiver,
            hub,
            opener,
            slot: PlayerSlot::default(),
            store: Store::open_in_memory().unwrap(),
            dir,
        }
    }

    /// An episode on disk, as a download is played: `name` in the rig's directory.
    fn file(&self, name: &str, fixture: &Fixture) -> Stream {
        let path = self.dir.path().join(name);
        std::fs::write(&path, matroska(fixture)).unwrap();
        Stream::new(path.to_string_lossy(), StreamKind::Mp4)
    }

    fn subtitles(&self) -> Subtitle {
        let path = self.dir.path().join("episode.en.ass");
        std::fs::write(
            &path,
            "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, \
             Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n",
        )
        .unwrap();
        Subtitle {
            language: "en".into(),
            url: path.to_string_lossy().into_owned(),
            hard: false,
            format: None,
        }
    }

    async fn play(&self, streams: Vec<Stream>, context: PlaybackContext) -> Vec<Event> {
        let http =
            HttpClient::offline(&anistream_core::config::NetworkConfig::default()).unwrap();
        let machine = Machine {
            opener: self.opener.clone(),
            slot: self.slot.clone(),
            host: Arc::new(NoHost),
            power: Arc::new(ReportedPower::default()),
            offline: Default::default(),
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        let (_controls, commands) = mpsc::unbounded_channel::<PlayerCommand>();
        let mut settings = settings();
        settings.subtitle_language = Some("en".into());
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
        tokio::time::timeout(Duration::from_secs(30), playing)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_episode_casts_with_its_subtitles_and_the_next_plays_on_the_same_tv() {
    let rig = Rig::new(true).await;
    let first =
        Stream { subtitles: vec![rig.subtitles()], ..rig.file("1.mkv", &Fixture::default()) };

    let events = rig.play(vec![first], context("1", Some("2"))).await;
    assert_eq!(
        ending(&events),
        Some(Ending::Complete { next: Some("2".into()) }),
        "{events:?}"
    );
    let loaded = rig.receiver.heard("LOAD");
    let media = &loaded[0]["media"];
    assert_eq!(media["metadata"]["seriesTitle"], "Stardust Wayfarer");
    assert_eq!(media["metadata"]["title"], "Episode 1");
    assert_eq!(media["metadata"]["images"][0]["url"], "https://art.example/banner.jpg");
    assert_eq!(media["tracks"][0]["trackContentType"], "text/vtt");
    assert_eq!(
        loaded[0]["activeTrackIds"],
        serde_json::json!([1]),
        "subtitles on for a sub watcher"
    );
    assert!(rig.hub.view().status.and_then(|s| s.plan).is_some(), "the card knows the plan");

    // The TV waits for the next episode, and auto-next plays it there.
    let second = rig.file("2.mkv", &Fixture::default());
    let events = rig.play(vec![second], context("2", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.receiver.heard("LAUNCH").len(), 1, "the receiver kept running");
    let loaded = rig.receiver.heard("LOAD");
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[1]["media"]["metadata"]["title"], "Episode 2 · Low Tide");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_kept_for_the_next_episode_is_let_go_by_a_play_on_this_device() {
    let rig = Rig::new(true).await;
    let first = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![first], context("1", Some("2"))).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: Some("2".into()) }));

    assert!(rig.slot.take(&PlayerTarget::Local).is_none(), "a window here is not the TV");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !rig.receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()) {
        assert!(tokio::time::Instant::now() < deadline, "the TV went back to what it showed");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(rig.slot.take(&PlayerTarget::Cast("tv-1".into())).is_none(), "nothing kept");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn audio_the_tv_cant_play_says_so_and_never_takes_the_tv_over() {
    let rig = Rig::new(false).await;
    let stream = rig.file("1.mkv", &Fixture::default());
    let events = rig.play(vec![stream], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Failed));
    let said = Msg::NoticeCastAudioCodec { codec: "AC-3".into() }.english();
    assert!(toasts(&events).contains(&said), "{:?}", toasts(&events));
    assert!(rig.receiver.heard("LAUNCH").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_stream_the_tv_cant_play_fails_over_to_one_it_can() {
    let rig = Rig::new(false).await;
    let ac3 = rig.file("1-ac3.mkv", &Fixture::default());
    let aac = rig
        .file("1-aac.mkv", &Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let events = rig.play(vec![ac3, aac], context("1", None)).await;
    assert_eq!(ending(&events), Some(Ending::Complete { next: None }), "{events:?}");
    assert_eq!(rig.receiver.heard("LOAD").len(), 1, "only the stream it can play was loaded");
}
