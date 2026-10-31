//! One player window for a binge, end to end against a real mpv.
//!
//! Skipped when mpv is not on the PATH — CI has none — but run locally this is the only check
//! that the window actually survives: the slot, the idle window, the per-file load and the
//! playback loop's decision to park all have to agree, and each can be right on its own while
//! the binge still opens a new window every episode.

use std::{sync::Arc, time::Duration};

use anistream_core::traits::PlaybackRequest;
use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use anistream_engine::{
    host::Host,
    outbox::Outbox,
    playback::{
        self, Machine, NextEpisode, PlaySettings, PlaybackContext,
        link::{LinkError, Opened, PlayerOpener, PlayerTarget},
        slot::PlayerSlot,
    },
};
use anistream_model::{
    command::PlayerCommand,
    event::Event,
    playback::{Ending, Successor},
};
use anistream_net::HttpClient;
use anistream_player::Mpv;
use anistream_store::Store;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    sync::mpsc,
};

/// The host a test plays under: it opens nothing and shows nothing. No playback here hands a
/// stream to the browser.
struct NoLinks;

impl Host for NoLinks {
    fn open_url(&self, _: &str) -> Result<(), String> {
        Err("a test opens nothing".into())
    }

    fn notify(&self, _: &str, _: &str) {}
}

/// What a playback's events say, in this test, as each window opens.
const WINDOW: &str = "a player window opened";

/// mpv's opener, marking each window it opens in the playback's events: the engine says nothing
/// of windows itself.
struct Marking {
    inner: Arc<dyn PlayerOpener>,
    tx: Outbox,
}

#[async_trait::async_trait]
impl PlayerOpener for Marking {
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError> {
        let opened = self.inner.open(request).await?;
        let _ = self.tx.send(Event::Clipboard { text: WINDOW.into() });
        Ok(opened)
    }

    fn scratch_dir(&self) -> &std::path::Path {
        self.inner.scratch_dir()
    }

    fn target(&self) -> PlayerTarget {
        self.inner.target()
    }
}

/// mpv's process, on the desktop as it is, with `slot` for its window, each window it opens
/// marked in `tx`.
fn machine(mpv: &Mpv, slot: &PlayerSlot, tx: &Outbox) -> Machine {
    let mut machine = Machine::desktop(mpv.clone(), slot.clone(), Arc::new(NoLinks));
    machine.opener = Arc::new(Marking { inner: machine.opener, tx: tx.clone() });
    machine
}

fn clip(seconds: u32) -> Stream {
    Stream {
        provider_id: "test".into(),
        ..Stream::new(
            format!("av://lavfi:testsrc=duration={seconds}:size=320x240:rate=30"),
            StreamKind::Mp4,
        )
    }
}

fn context(episode: &str, next: Option<&str>) -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(1),
        // No MAL id: no skip lookup, so nothing here touches the network.
        mal_id: None,
        episode: episode.into(),
        title: "Test".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: None,
        speed: None,
        volume: None,
        next: next.map(|n| NextEpisode { number: n.into(), title: None, thumbnail: None }),
        episodes: Vec::new(),
        cover_url: None,
        banner_url: None,
        episode_total: None,
    }
}

fn settings(auto_next: bool) -> PlaySettings {
    PlaySettings {
        threshold: 0.85,
        auto_skip: false,
        mark_chapters: false,
        skip_button: false,
        fullscreen: false,
        subtitle_language: None,
        auto_next,
        night_mode: false,
        remember_tracks: true,
        look: Default::default(),
        shader_dir: None,
        tone_mapping: "auto".into(),
        tracker_ids: Vec::new(),
        presence: Default::default(),
        reports: Default::default(),
        party: Default::default(),
        up_next_countdown: 10,
        mpv_options: Vec::new(),
    }
}

async fn player() -> Option<(Mpv, tempfile::TempDir)> {
    let dir = tempfile::tempdir().unwrap();
    let mpv =
        Mpv::new(dir.path()).with_extra_args(vec!["--vo=null".into(), "--ao=null".into()]);
    if !mpv.is_available().await {
        // CI's mpv job installs mpv and sets this, so a missing mpv fails there
        // instead of passing as a skip.
        assert!(
            std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
            "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
        );
        eprintln!("skipping: mpv is not on PATH");
        return None;
    }
    Some((mpv, dir))
}

/// Run one playback to its end, returning every update it sent.
async fn run(
    mpv: &Mpv,
    slot: &PlayerSlot,
    stream: Stream,
    context: PlaybackContext,
    settings: PlaySettings,
    commands: mpsc::UnboundedReceiver<PlayerCommand>,
) -> Vec<Event> {
    let store = Store::open_in_memory().unwrap();
    run_streams(mpv, slot, vec![stream], context, settings, commands, store).await
}

/// [`run`] over a list of streams, recording into `store`.
async fn run_streams(
    mpv: &Mpv,
    slot: &PlayerSlot,
    streams: Vec<Stream>,
    context: PlaybackContext,
    settings: PlaySettings,
    commands: mpsc::UnboundedReceiver<PlayerCommand>,
    store: Store,
) -> Vec<Event> {
    let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let tx = Outbox::from(tx);
    // A bound for a playback that hangs. On a busy machine mpv can take most of the 20 s the
    // playback gives it to open its socket, and then decodes its way to any resume point.
    tokio::time::timeout(
        Duration::from_secs(90),
        playback::play(
            streams,
            context,
            store,
            http,
            machine(mpv, slot, &tx),
            settings,
            tx,
            commands,
        ),
    )
    .await
    .expect("playback should end on its own");
    let mut updates = Vec::new();
    while let Ok(update) = rx.try_recv() {
        updates.push(update);
    }
    updates
}

fn spawned(updates: &[Event]) -> bool {
    windows(updates) > 0
}

/// How many player windows the playback opened.
fn windows(updates: &[Event]) -> usize {
    updates.iter().filter(|u| matches!(u, Event::Clipboard { text } if text == WINDOW)).count()
}

/// The tracker's verdict and how the playback ended.
fn ended(updates: &[Event]) -> Option<(bool, Ending)> {
    updates.iter().find_map(|u| match u {
        Event::PlaybackEnded { watched, ending, .. } => Some((*watched, ending.clone())),
        _ => None,
    })
}

fn complete(next: Option<&str>) -> Ending {
    Ending::Complete { next: next.map(Into::into) }
}

fn alerts(updates: &[Event]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::Toast(toast) if toast.kind == anistream_model::notice::NoticeKind::Alert => {
                Some(toast.text.clone())
            }
            _ => None,
        })
        .collect()
}

/// Episodes asked for by the player: its N/P, Up Next's Play now, the Episodes menu, n/N.
fn asked_for(updates: &[Event]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::PlayerPlayEpisode { episode, .. } => Some(episode.clone()),
            _ => None,
        })
        .collect()
}

fn successor(episode: &str) -> Successor {
    Successor {
        id: AnilistId::new(1),
        title: "Test".into(),
        episode: episode.into(),
        translation: Translation::Sub,
        cover_url: None,
        banner_url: None,
    }
}

/// Send one command to the player window in `dir`, as the viewer's own keys or another
/// client would: the playback under test only ever sees what mpv reports back.
async fn tell_mpv(dir: &std::path::Path, command: serde_json::Value) {
    let socket = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .find(|path| path.extension().is_some_and(|ext| ext == "sock"))
        .expect("the player's socket");
    let stream = anistream_player::ipc::connect(&socket).await.unwrap();
    let (reader, mut writer) = tokio::io::split(stream);
    let line = format!("{}\n", serde_json::json!({ "command": command, "request_id": 1 }));
    writer.write_all(line.as_bytes()).await.unwrap();
    writer.flush().await.unwrap();
    // Held open until mpv answers. Hanging up straight after the write sometimes lost the
    // command: a `cancel-next` sent seconds before the end never reached the playback. A
    // `quit` closes the socket instead of answering, which is just as conclusive.
    let mut lines = tokio::io::BufReader::new(reader).lines();
    let answered = async {
        while let Ok(Some(line)) = lines.next_line().await {
            if serde_json::from_str::<serde_json::Value>(&line)
                .is_ok_and(|reply| reply["request_id"] == 1)
            {
                return;
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(5), answered)
        .await
        .expect("mpv neither answered the command nor closed the socket");
}

/// Wait until mpv has loaded a file and is far enough in to have reported a position.
async fn until_playing(updates: &mut mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
    let mut seen = Vec::new();
    while let Some(update) = updates.recv().await {
        let playing = matches!(update, Event::Playback { .. });
        seen.push(update);
        if playing {
            break;
        }
    }
    seen
}

/// Wait until the playback reports the playhead at `seconds` or later.
///
/// Waited for rather than slept for: a busy machine plays the clip slower than the clock on
/// the wall, so three seconds of waiting can be less than three seconds of playback.
async fn until_at(updates: &mut mpsc::UnboundedReceiver<Event>, seconds: f64) -> Vec<Event> {
    let mut seen = Vec::new();
    while let Some(update) = updates.recv().await {
        let there = matches!(update, Event::Playback { position, .. } if position >= seconds);
        seen.push(update);
        if there {
            break;
        }
    }
    seen
}

/// Wait until the playback reports a position past `threshold` by the tracker's own rule: a
/// known duration, and the playhead at least that far through it.
///
/// A resume point is only reached once mpv lands there, and on a generated clip it decodes its
/// way from the start to do so: 0.75 s for 180 s on CI. mpv 0.37 reports no position on the
/// way and the tracker learns the duration only on landing, so a fixed half second was not
/// enough there; 0.41 reports the decoding as it goes, against a duration that grows with it.
async fn until_watched(
    updates: &mut mpsc::UnboundedReceiver<Event>,
    threshold: f64,
) -> Vec<Event> {
    let mut seen = Vec::new();
    let past = async {
        while let Some(update) = updates.recv().await {
            let past = matches!(update, Event::Playback { position, duration: Some(d), .. }
                if d > 0.0 && position / d >= threshold);
            seen.push(update);
            if past {
                return;
            }
        }
    };
    // A bound for a player that hangs, not a measure of how fast it starts: on a busy machine
    // mpv can take most of the 20 s the playback gives it to open its socket.
    tokio::time::timeout(Duration::from_secs(60), past)
        .await
        .expect("the playback never got past the threshold");
    seen
}

#[tokio::test(flavor = "multi_thread")]
async fn the_next_episode_plays_in_the_window_the_last_one_left() {
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();

    let (_keep, commands) = mpsc::unbounded_channel();
    let first =
        run(&mpv, &slot, clip(2), context("1", Some("2")), settings(true), commands).await;
    assert!(spawned(&first), "the first episode opens the window");
    assert_eq!(
        ended(&first),
        Some((true, complete(Some("2")))),
        "played to the end counts as watched, and rolls on to the episode kept for"
    );

    let (_keep, commands) = mpsc::unbounded_channel();
    let second = run(&mpv, &slot, clip(2), context("2", None), settings(true), commands).await;
    assert!(!spawned(&second), "the second episode must reuse the parked window");
    assert_eq!(ended(&second), Some((true, complete(None))));
    // No episode after the second, so nothing waits: the window closed with it.
    assert!(
        slot.take(&PlayerTarget::Local).is_none(),
        "a finale must not leave a window parked"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_or_absent_successor_closes_the_window() {
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (_keep, commands) = mpsc::unbounded_channel();
    // Auto-next off: the end of the episode is the end of the session.
    let updates =
        run(&mpv, &slot, clip(2), context("1", Some("2")), settings(false), commands).await;
    assert_eq!(ended(&updates), Some((true, complete(None))), "nothing is named to roll on to");
    assert!(slot.take(&PlayerTarget::Local).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_newer_playback_takes_the_window_mid_episode() {
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();

    let (handover, commands) = mpsc::unbounded_channel();
    let first = {
        let (mpv, slot) = (mpv.clone(), slot.clone());
        tokio::spawn(async move {
            run(&mpv, &slot, clip(60), context("1", Some("2")), settings(true), commands).await
        })
    };
    // Long enough for playback to be under way, nowhere near the end of a minute-long clip.
    tokio::time::sleep(Duration::from_secs(2)).await;
    handover.send(PlayerCommand::Handover(successor("2"))).unwrap();
    let first = first.await.unwrap();
    assert_eq!(ended(&first), None, "a handed-over playback must not announce an ending");
    assert!(
        first.iter().any(|u| matches!(u, Event::EpisodeLeft { .. })),
        "where it was left is still reflected"
    );

    let (_keep, commands) = mpsc::unbounded_channel();
    let second = run(&mpv, &slot, clip(2), context("2", None), settings(true), commands).await;
    assert!(!spawned(&second), "the new playback plays in the handed-over window");
    assert_eq!(ended(&second), Some((true, complete(None))));
}

fn prefetches(updates: &[Event]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::PrefetchNext { episode, .. } => Some(episode.clone()),
            _ => None,
        })
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_next_episode_is_asked_for_near_the_end_when_it_will_roll_on() {
    // A 200-second episode started 5 seconds from its end is well past the 90-second lead.
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let near_end = |next| PlaybackContext { resume_at: Some(195.0), ..context("1", next) };

    let (_keep, commands) = mpsc::unbounded_channel();
    let rolls_on =
        run(&mpv, &slot, clip(200), near_end(Some("2")), settings(true), commands).await;
    assert_eq!(prefetches(&rolls_on), ["2"], "asked for once, for the episode that follows");

    // Without auto-next nothing would play it, so nothing is fetched for it either.
    slot.release();
    let (_keep, commands) = mpsc::unbounded_channel();
    let stops =
        run(&mpv, &slot, clip(200), near_end(Some("2")), settings(false), commands).await;
    assert!(prefetches(&stops).is_empty());
    slot.release();
}

fn row(number: &str) -> anistream_model::episodes::EpisodeRow {
    anistream_model::episodes::EpisodeRow {
        number: number.into(),
        title: Some(format!("Title {number}")),
        thumbnail: None,
        description: None,
        duration_secs: None,
        watched: 0.0,
        completed: false,
        kind: None,
        skippable: false,
        airs_at: None,
        delayed_from: None,
        metadata_only: false,
        source: None,
        unsourced: false,
        past_progress: false,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_episode_list_learned_after_starting_still_keeps_the_window_for_the_next() {
    // Started from somewhere the episode table did not describe: no list, no next episode.
    // The list arrives once playback is under way, and the episode then rolls on as usual.
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (controls, commands) = mpsc::unbounded_channel();
    controls
        .send(PlayerCommand::Episodes {
            rows: vec![row("1"), row("2"), row("3")],
            next: Some("2".into()),
        })
        .unwrap();
    let updates = run(&mpv, &slot, clip(2), context("1", None), settings(true), commands).await;
    assert_eq!(ended(&updates), Some((true, complete(Some("2")))));
    assert!(slot.take(&PlayerTarget::Local).is_some(), "the window waits for episode 2");
}

/// Start a playback in the background, for a test that acts on it while it plays.
fn start(
    mpv: &Mpv,
    slot: &PlayerSlot,
    streams: Vec<Stream>,
    context: PlaybackContext,
    settings: PlaySettings,
    commands: mpsc::UnboundedReceiver<PlayerCommand>,
    store: Store,
) -> (tokio::task::JoinHandle<()>, mpsc::UnboundedReceiver<Event>) {
    let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    let tx = Outbox::from(tx);
    let task = tokio::spawn(playback::play(
        streams,
        context,
        store,
        http,
        machine(mpv, slot, &tx),
        settings,
        tx,
        commands,
    ));
    (task, rx)
}

/// Wait for a background playback to end, returning what it sent after `seen`.
async fn finish(
    task: tokio::task::JoinHandle<()>,
    mut rx: mpsc::UnboundedReceiver<Event>,
    mut seen: Vec<Event>,
) -> Vec<Event> {
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .expect("playback should end on its own")
        .unwrap();
    while let Ok(update) = rx.try_recv() {
        seen.push(update);
    }
    seen
}

/// Wait until the playback has opened its window, which it hands the file to next.
async fn until_loaded(updates: &mut mpsc::UnboundedReceiver<Event>) -> Vec<Event> {
    let mut seen = Vec::new();
    while let Some(update) = updates.recv().await {
        let loaded = matches!(&update, Event::Clipboard { text } if text == WINDOW);
        seen.push(update);
        if loaded {
            break;
        }
    }
    seen
}

/// A stream that is never answered: mpv connects, asks, and waits, and no frame ever comes.
async fn silent_stream() -> (Stream, tokio::task::JoinHandle<()>) {
    let listener =
        tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let url = format!("http://{}/episode.mp4", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    (Stream { provider_id: "test".into(), ..Stream::new(url, StreamKind::Mp4) }, server)
}

fn attempts(updates: &[Event]) -> usize {
    updates.iter().filter(|u| matches!(u, Event::ActiveProvider(_))).count()
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_the_window_after_the_threshold_does_not_roll_on() {
    // Watched past the commit threshold, then the window closed during the credits. That is
    // a watched episode, and it is not a finished one: no window waits for episode 2, and
    // nothing tells the terminal to start it in a new one.
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let near_end = PlaybackContext { resume_at: Some(180.0), ..context("1", Some("2")) };
    let (_keep, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let (task, mut rx) =
        start(&mpv, &slot, vec![clip(200)], near_end, settings(true), commands, store);
    let seen = until_watched(&mut rx, settings(true).threshold).await;
    tell_mpv(dir.path(), serde_json::json!(["quit"])).await;
    let updates = finish(task, rx, seen).await;

    assert_eq!(ended(&updates), Some((true, Ending::Closed)));
    assert!(
        slot.take(&PlayerTarget::Local).is_none(),
        "a window was kept for an episode that is not coming"
    );
    assert!(alerts(&updates).is_empty(), "{:?}", alerts(&updates));
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_the_window_before_the_first_frame_does_not_fall_over() {
    // Two streams, the first one silent. Closing the window while it waits is the viewer
    // leaving, not the stream failing: no second stream, no second window, and no alert
    // blaming mpv for playing nothing.
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (silent, _server) = silent_stream().await;
    let (_keep, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let streams = vec![silent, clip(5)];
    let (task, mut rx) =
        start(&mpv, &slot, streams, context("1", Some("2")), settings(true), commands, store);
    let seen = until_loaded(&mut rx).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    tell_mpv(dir.path(), serde_json::json!(["quit"])).await;
    let updates = finish(task, rx, seen).await;

    assert_eq!(ended(&updates), Some((false, Ending::Closed)));
    assert_eq!(attempts(&updates), 1, "it fell over to the next stream");
    assert_eq!(windows(&updates), 1);
    assert!(alerts(&updates).is_empty(), "{:?}", alerts(&updates));
    assert!(slot.take(&PlayerTarget::Local).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_before_the_first_frame_is_not_reported_as_a_failure() {
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (silent, _server) = silent_stream().await;
    let (controls, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let streams = vec![silent, clip(5)];
    let (task, mut rx) =
        start(&mpv, &slot, streams, context("1", None), settings(true), commands, store);
    let seen = until_loaded(&mut rx).await;
    controls.send(PlayerCommand::Stop).unwrap();
    let updates = finish(task, rx, seen).await;

    assert_eq!(ended(&updates), Some((false, Ending::Stopped)));
    assert_eq!(attempts(&updates), 1, "a stop fell over to the next stream");
    assert!(alerts(&updates).is_empty(), "{:?}", alerts(&updates));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_up_next_holds_for_its_own_episode_only() {
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();

    // Episode 1 with Up Next cancelled, then left for episode 2 from the player, as N, Play
    // now or the Episodes menu leave it.
    let (handover, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let (task, mut rx) = start(
        &mpv,
        &slot,
        vec![clip(60)],
        context("1", Some("2")),
        settings(true),
        commands,
        store,
    );
    let seen = until_playing(&mut rx).await;
    tell_mpv(dir.path(), serde_json::json!(["script-message", "anistream", "cancel-next"]))
        .await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    handover.send(PlayerCommand::Handover(successor("2"))).unwrap();
    let first = finish(task, rx, seen).await;
    assert_eq!(ended(&first), None, "a handed-over playback announces no ending");

    // Episode 2 rolls on at its own end, in the window it was handed.
    let (_keep, commands) = mpsc::unbounded_channel();
    let second =
        run(&mpv, &slot, clip(2), context("2", Some("3")), settings(true), commands).await;
    assert!(!spawned(&second));
    assert_eq!(ended(&second), Some((true, complete(Some("3")))), "the cancel carried over");
    assert!(slot.take(&PlayerTarget::Local).is_some(), "the window waits for episode 3");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cancelled_up_next_ends_the_binge_at_this_episode() {
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (_keep, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let (task, mut rx) = start(
        &mpv,
        &slot,
        vec![clip(3)],
        context("1", Some("2")),
        settings(true),
        commands,
        store,
    );
    let seen = until_playing(&mut rx).await;
    tell_mpv(dir.path(), serde_json::json!(["script-message", "anistream", "cancel-next"]))
        .await;
    let updates = finish(task, rx, seen).await;
    assert_eq!(ended(&updates), Some((true, complete(None))));
    assert!(
        slot.take(&PlayerTarget::Local).is_none(),
        "a window waited for a cancelled Up Next"
    );
}

fn entry(number: &str) -> playback::EpisodeEntry {
    playback::EpisodeEntry { number: number.into(), title: None, watched: false, filler: false }
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_follows_the_finale() {
    // The last episode of a two-episode list. Asking for the next one, from the terminal or
    // inside the player, says there is none and leaves the finale playing; and its end names
    // no next episode, so nothing is searched for after it.
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let finale =
        PlaybackContext { episodes: vec![entry("1"), entry("2")], ..context("2", None) };
    let (controls, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let (task, mut rx) =
        start(&mpv, &slot, vec![clip(4)], finale, settings(true), commands, store);
    let seen = until_playing(&mut rx).await;
    controls.send(PlayerCommand::Step(1)).unwrap();
    tell_mpv(dir.path(), serde_json::json!(["script-message", "anistream", "next"])).await;
    // The way back is still open.
    controls.send(PlayerCommand::Step(-1)).unwrap();
    let updates = finish(task, rx, seen).await;

    assert_eq!(asked_for(&updates), ["1"], "only the previous episode exists");
    assert!(
        updates.iter().any(
            |u| matches!(u, Event::Toast(t) if t.text.starts_with("There's no next episode"))
        ),
        "the terminal is told there is no next episode"
    );
    assert_eq!(ended(&updates), Some((true, complete(None))), "the finale played to its end");
    assert!(
        slot.take(&PlayerTarget::Local).is_none(),
        "a finale must not leave a window parked"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn stopping_records_where_it_was_left_before_saying_it_ended() {
    // Quitting the app stops the player this way and waits for the ending, so whatever the
    // ending reports has to already be in the history: a row is otherwise written only every
    // ten seconds of playback.
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (controls, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let (task, mut rx) = start(
        &mpv,
        &slot,
        vec![clip(60)],
        context("1", None),
        settings(true),
        commands,
        store.clone(),
    );
    let seen = until_at(&mut rx, 3.0).await;
    controls.send(PlayerCommand::Stop).unwrap();
    let updates = finish(task, rx, seen).await;

    assert_eq!(ended(&updates).map(|(_, ending)| ending), Some(Ending::Stopped));
    let last = store.events_for(AnilistId::new(1), 1).unwrap();
    let position = last.first().map(|e| e.position_secs).unwrap_or_default();
    assert!(position >= 2.5, "the last row is from {position}s, not where it stopped");
}

/// Read one property from the player window in `dir`.
async fn ask_mpv(dir: &std::path::Path, property: &str) -> serde_json::Value {
    use tokio::io::AsyncBufReadExt;
    let socket = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .find(|path| path.extension().is_some_and(|ext| ext == "sock"))
        .expect("the player's socket");
    let stream = anistream_player::ipc::connect(&socket).await.unwrap();
    let (read, mut write) = anistream_player::ipc::split(stream);
    let line = format!(
        "{}\n",
        serde_json::json!({ "command": ["get_property", property], "request_id": 4242 })
    );
    write.write_all(line.as_bytes()).await.unwrap();
    let mut lines = tokio::io::BufReader::new(read).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let value: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if value["request_id"] == 4242 {
            return value["data"].clone();
        }
    }
    serde_json::Value::Null
}

#[tokio::test(flavor = "multi_thread")]
async fn night_mode_can_be_turned_back_on_from_the_menu() {
    // The menu's toggle offers the opposite of what it was last told; told nothing after the
    // first toggle, it offered "off" again and night mode could not come back.
    let Some((mpv, dir)) = player().await else { return };
    let slot = PlayerSlot::default();
    let (_keep, commands) = mpsc::unbounded_channel();
    let store = Store::open_in_memory().unwrap();
    let night = PlaySettings { night_mode: true, ..settings(false) };
    let (task, mut rx) =
        start(&mpv, &slot, vec![clip(8)], context("1", None), night, commands, store);
    let seen = until_playing(&mut rx).await;
    let menu = || async {
        ask_mpv(dir.path(), "user-data/anistream/menus").await["night_mode"].clone()
    };
    assert_eq!(menu().await, true);

    let choose = |on: bool| {
        let payload = serde_json::json!({ "menu": "night_mode", "value": on }).to_string();
        serde_json::json!(["script-message", "anistream", "choose", payload])
    };
    tell_mpv(dir.path(), choose(false)).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while menu().await != false && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(menu().await, false, "the menu still offers turning it off");

    tell_mpv(dir.path(), choose(true)).await;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while menu().await != true && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(menu().await, true);
    let filters = ask_mpv(dir.path(), "af").await.to_string();
    assert!(filters.contains("anistream-night"), "{filters}");

    tell_mpv(dir.path(), serde_json::json!(["quit"])).await;
    let updates = finish(task, rx, seen).await;
    let saved: Vec<bool> = updates
        .iter()
        .filter_map(|u| match u {
            Event::PlayerNightMode(on) => Some(*on),
            _ => None,
        })
        .collect();
    assert_eq!(saved, [false, true]);
}
