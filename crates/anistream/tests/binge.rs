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
use anistream_ui::app::{PlayerCommand, Update};
use tokio::sync::mpsc;

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
    let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    tokio::time::timeout(
        Duration::from_secs(30),
        playback::play(
            vec![stream],
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
    updates.iter().any(|u| matches!(u, Update::Status(s) if s.starts_with("starting mpv")))
}

fn ended(updates: &[Update]) -> Option<bool> {
    updates.iter().find_map(|u| match u {
        Update::PlaybackEnded { watched } => Some(*watched),
        _ => None,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn the_next_episode_plays_in_the_window_the_last_one_left() {
    let Some((mpv, _dir)) = player().await else { return };
    let slot = PlayerSlot::default();

    let (_keep, commands) = mpsc::unbounded_channel();
    let first =
        run(&mpv, &slot, clip(2), context("1", Some("2")), settings(true), commands).await;
    assert!(spawned(&first), "the first episode opens the window");
    assert_eq!(ended(&first), Some(true), "played to the end counts as watched");

    let (_keep, commands) = mpsc::unbounded_channel();
    let second = run(&mpv, &slot, clip(2), context("2", None), settings(true), commands).await;
    assert!(!spawned(&second), "the second episode must reuse the parked window");
    assert_eq!(ended(&second), Some(true));
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
    assert_eq!(ended(&updates), Some(true));
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
    handover.send(PlayerCommand::Handover).unwrap();
    let first = first.await.unwrap();
    assert_eq!(ended(&first), None, "a handed-over playback must not announce an ending");
    assert!(
        first.iter().any(|u| matches!(u, Update::EpisodeLeft { .. })),
        "where it was left is still reflected"
    );

    let (_keep, commands) = mpsc::unbounded_channel();
    let second = run(&mpv, &slot, clip(2), context("2", None), settings(true), commands).await;
    assert!(!spawned(&second), "the new playback plays in the handed-over window");
    assert_eq!(ended(&second), Some(true));
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
