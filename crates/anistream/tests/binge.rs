//! One player window for a binge, end to end against a real mpv.
//!
//! Skipped when mpv is not on the PATH — CI has none — but run locally this is the only check
//! that the window actually survives: the slot, the idle window, the per-file load and the
//! playback loop's decision to park all have to agree, and each can be right on its own while
//! the binge still opens a new window every episode.

use std::time::Duration;

use anistream::{
    playback::{self, NextEpisode, PlaySettings, PlaybackContext},
    slot::PlayerSlot,
};
use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use anistream_net::HttpClient;
use anistream_player::Mpv;
use anistream_store::Store;
use anistream_ui::app::{Ending, PlayerCommand, Successor, Update};
use tokio::{io::AsyncWriteExt, sync::mpsc};

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
    }
}

fn settings(auto_next: bool) -> PlaySettings {
    PlaySettings {
        threshold: 0.85,
        auto_skip: false,
        mark_chapters: false,
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
        syncplay: Default::default(),
        party: false,
    }
}

async fn player() -> Option<(Mpv, tempfile::TempDir)> {
    let dir = tempfile::tempdir().unwrap();
    let mpv =
        Mpv::new(dir.path()).with_extra_args(vec!["--vo=null".into(), "--ao=null".into()]);
    if !mpv.is_available().await {
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
) -> Vec<Update> {
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
) -> Vec<Update> {
    let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::time::timeout(
        Duration::from_secs(30),
        playback::play(
            streams,
            context,
            store,
            http,
            mpv.clone(),
            settings,
            slot.clone(),
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

fn spawned(updates: &[Update]) -> bool {
    windows(updates) > 0
}

/// How many player windows the playback opened.
fn windows(updates: &[Update]) -> usize {
    updates
        .iter()
        .filter(|u| matches!(u, Update::Status(s) if s.starts_with("starting mpv")))
        .count()
}

/// The tracker's verdict and how the playback ended.
fn ended(updates: &[Update]) -> Option<(bool, Ending)> {
    updates.iter().find_map(|u| match u {
        Update::PlaybackEnded { watched, ending } => Some((*watched, ending.clone())),
        _ => None,
    })
}

fn complete(next: Option<&str>) -> Ending {
    Ending::Complete { next: next.map(Into::into) }
}

fn alerts(updates: &[Update]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Update::Toast(toast) if toast.kind == anistream_ui::app::ToastKind::Alert => {
                Some(toast.text.clone())
            }
            _ => None,
        })
        .collect()
}

/// Episodes asked for by the player: its N/P, Up Next's Play now, the Episodes menu, n/N.
fn asked_for(updates: &[Update]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Update::PlayerPlayEpisode { episode, .. } => Some(episode.clone()),
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
    let mut stream = anistream_player::ipc::connect(&socket).await.unwrap();
    let line = format!("{}\n", serde_json::json!({ "command": command }));
    stream.write_all(line.as_bytes()).await.unwrap();
    stream.flush().await.unwrap();
}

/// Wait until mpv has loaded a file and is far enough in to have reported a position.
async fn until_playing(updates: &mut mpsc::UnboundedReceiver<Update>) -> Vec<Update> {
    let mut seen = Vec::new();
    while let Some(update) = updates.recv().await {
        let playing = matches!(update, Update::Playback { .. });
        seen.push(update);
        if playing {
            break;
        }
    }
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
    assert!(slot.take().is_none(), "a finale must not leave a window parked");
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
    assert!(slot.take().is_none());
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
        first.iter().any(|u| matches!(u, Update::EpisodeLeft { .. })),
        "where it was left is still reflected"
    );

    let (_keep, commands) = mpsc::unbounded_channel();
    let second = run(&mpv, &slot, clip(2), context("2", None), settings(true), commands).await;
    assert!(!spawned(&second), "the new playback plays in the handed-over window");
    assert_eq!(ended(&second), Some((true, complete(None))));
}

fn prefetches(updates: &[Update]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|u| match u {
            Update::PrefetchNext { episode, .. } => Some(episode.clone()),
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

fn row(number: &str) -> anistream_ui::app::EpisodeRow {
    anistream_ui::app::EpisodeRow {
        number: number.into(),
        title: Some(format!("Title {number}")),
        thumbnail: None,
        description: None,
        duration_secs: None,
        watched: 0.0,
        completed: false,
        kind: None,
        skippable: false,
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
    assert!(slot.take().is_some(), "the window waits for episode 2");
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
) -> (tokio::task::JoinHandle<()>, mpsc::UnboundedReceiver<Update>) {
    let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(playback::play(
        streams,
        context,
        store,
        http,
        mpv.clone(),
        settings,
        slot.clone(),
        tx,
        commands,
    ));
    (task, rx)
}

/// Wait for a background playback to end, returning what it sent after `seen`.
async fn finish(
    task: tokio::task::JoinHandle<()>,
    mut rx: mpsc::UnboundedReceiver<Update>,
    mut seen: Vec<Update>,
) -> Vec<Update> {
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .expect("playback should end on its own")
        .unwrap();
    while let Ok(update) = rx.try_recv() {
        seen.push(update);
    }
    seen
}

/// Wait until the playback has handed a file to mpv.
async fn until_loaded(updates: &mut mpsc::UnboundedReceiver<Update>) -> Vec<Update> {
    let mut seen = Vec::new();
    while let Some(update) = updates.recv().await {
        let loaded = matches!(&update, Update::Status(s) if s.starts_with("playing"));
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

fn attempts(updates: &[Update]) -> usize {
    updates.iter().filter(|u| matches!(u, Update::ActiveProvider(_))).count()
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
    let seen = until_playing(&mut rx).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    tell_mpv(dir.path(), serde_json::json!(["quit"])).await;
    let updates = finish(task, rx, seen).await;

    assert_eq!(ended(&updates), Some((true, Ending::Closed)));
    assert!(slot.take().is_none(), "a window was kept for an episode that is not coming");
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
    assert!(slot.take().is_none());
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
    assert!(slot.take().is_some(), "the window waits for episode 3");
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
    assert!(slot.take().is_none(), "a window waited for a cancelled Up Next");
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
        updates
            .iter()
            .any(|u| matches!(u, Update::Toast(t) if t.text.starts_with("no next episode"))),
        "the terminal is told there is no next episode"
    );
    assert_eq!(ended(&updates), Some((true, complete(None))), "the finale played to its end");
    assert!(slot.take().is_none(), "a finale must not leave a window parked");
}
