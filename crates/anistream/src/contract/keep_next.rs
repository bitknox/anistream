//! Keep next: the next episodes of the shows being watched, queued ahead of the viewer as they
//! move on — on start, as the number to keep changes, as an episode is watched or played to its
//! end, and as a list sync moves a show — and not while downloads wait.
//!
//! Off in every other test, as the rest of the engine's own work is; on here, with the device
//! taken to have a terabyte free. Nothing starts a torrent: the harness has no session, so what
//! is queued stays queued, which is all these read.

use anistream_core::{
    ids::AnilistId,
    settings::SettingValue,
    stream::{Stream, StreamKind},
};
use anistream_model::{
    command::{AppState, Command},
    event::Event,
    playback::Ending,
    settings::SettingKey,
    show::ShowSettingKey,
};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_testkit::{
    fixtures::{AIRING, FINISHED, FIXTURE_NOW},
    seed, sources,
};

use super::{WITH_ANILIST, toasts};
use crate::harness::{Harness, Options};

/// The fixture source, offering a torrent for every episode of [`FINISHED`] and [`AIRING`].
fn torrents() -> ProviderRegistry {
    let torrent = |n: u32| Stream {
        provider_id: sources::PROVIDER_ID.into(),
        download_source: Some(format!("magnet:?xt=urn:btih:{n:040x}&dn=fixture")),
        ..Stream::new("http://127.0.0.1:9/stream", StreamKind::TorrentHttp)
    };
    let provider = sources::provider()
        .with_streams_for(sources::key(FINISHED), vec![torrent(1)])
        .with_streams_for(sources::key(AIRING), vec![torrent(2)]);
    ProviderRegistry::new(vec![provider.arc()])
}

/// The app with keep next on, and the torrents above.
async fn keeping(options: Options) -> Harness {
    Harness::with(Options { keep_next: true, registry: Some(torrents()), ..options }).await
}

/// `config.toml` keeping `n` episodes.
fn keep(n: u32) -> Option<String> {
    Some(format!("[downloads]\nkeep_next = {n}\n"))
}

/// [`FINISHED`] being watched, four episodes in.
fn four_in(store: &Store) {
    store.replace_library_statuses("anilist", &[(FINISHED, "CURRENT")], FIXTURE_NOW).unwrap();
    seed::watched(store, FINISHED, 4, FIXTURE_NOW - 600);
}

/// What the queue has of show `id`: each episode, with `ahead` beside one keep next queued.
fn queue(store: &Store, id: AnilistId) -> Vec<String> {
    let mut rows: Vec<_> =
        store.downloads().unwrap().into_iter().filter(|row| row.anilist_id == id).collect();
    rows.sort_by_key(|row| row.episode.parse::<u32>().unwrap_or(0));
    rows.iter()
        .map(|row| match row.ahead {
            true => format!("{} ahead", row.episode),
            false => row.episode.clone(),
        })
        .collect()
}

/// The last queue among `updates`, as the Downloads screen was told it: each episode, with
/// `ahead` beside one keep next queued.
fn said(updates: &[Event]) -> Vec<String> {
    let rows = updates
        .iter()
        .rev()
        .find_map(|update| match update {
            Event::Downloads(rows) => Some(rows),
            _ => None,
        })
        .expect("the queue was said");
    let mut rows: Vec<_> =
        rows.iter().map(|row| (row.episode.parse::<u32>().unwrap_or(0), row.ahead)).collect();
    rows.sort();
    rows.into_iter()
        .map(
            |(episode, ahead)| {
                if ahead { format!("{episode} ahead") } else { episode.to_string() }
            },
        )
        .collect()
}

#[tokio::test]
async fn on_start_the_episodes_after_where_the_viewer_is_are_queued() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    let updates = h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
    assert_eq!(said(&updates), ["5 ahead", "6 ahead"], "the Downloads screen says so");
    assert!(toasts(&updates).is_empty(), "quietly: {:?}", toasts(&updates));
}

#[tokio::test]
async fn the_number_kept_changing_tops_the_show_up_settings_or_its_own() {
    let mut h = keeping(Options::default()).await;
    four_in(h.store());
    h.send(Command::SetSetting {
        key: SettingKey::KeepNextEpisodes,
        value: SettingValue::Int(2),
    });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);

    h.send(Command::SetShowSetting {
        id: FINISHED,
        key: ShowSettingKey::KeepNext,
        value: Some(SettingValue::Int(3)),
    });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead"]);

    // Fewer kept: nothing deleted.
    h.send(Command::StepSetting { key: SettingKey::KeepNextEpisodes, delta: -1 });
    h.send(Command::ResetShowSettings { id: FINISHED });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead"]);
}

#[tokio::test]
async fn marking_an_episode_watched_tops_the_show_up_once() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);

    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead"]);

    // Asked again with nothing new to keep: the one row each.
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.send(Command::LoadDownloads);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead"]);
}

#[tokio::test]
async fn an_episode_played_to_its_end_or_counted_watched_tops_the_show_up() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    h.settle().await;

    // As the playback of 5 records it, then says it ended.
    seed::watched(h.store(), FINISHED, 5, FIXTURE_NOW);
    let ended = Event::playback_ended(true, Ending::Complete { next: None });
    h.playback_says(1_000_001, FINISHED, "5", ended);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead"]);

    // Counted watched partway through the credits of 6.
    seed::watched(h.store(), FINISHED, 6, FIXTURE_NOW + 60);
    h.player_says(Event::ProgressQueued);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "7 ahead", "8 ahead"]);
}

#[tokio::test]
async fn a_list_sync_that_moves_a_show_tops_it_up_with_only_what_is_out() {
    let config = format!("{WITH_ANILIST}\n[downloads]\nkeep_next = 4\n");
    let mut h = keeping(Options {
        config: Some(config),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    // The fake's list: three episodes into the airing show, of which five are out and the
    // sixth airs in two days; the finished one completed.
    h.send(Command::SyncNow);
    h.settle().await;
    assert_eq!(queue(h.store(), AIRING), ["4 ahead", "5 ahead"]);
    assert!(queue(h.store(), FINISHED).is_empty(), "completed, so not being watched");
}

#[tokio::test]
async fn nothing_is_kept_while_downloads_wait_on_cellular_or_off_screen() {
    let mut h = keeping(Options { config: keep(2), ..Options::default() }).await;
    h.send(Command::NetworkPath { cellular: true, constrained: false });
    h.settle().await;
    four_in(h.store());
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["4".into()], watched: true });
    h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "Wi-Fi only");

    // Back on Wi-Fi, but off screen: still waiting.
    h.send(Command::AppPhase { phase: AppState::Background { playing: false } });
    h.send(Command::NetworkPath { cellular: false, constrained: false });
    h.settle().await;
    assert!(queue(h.store(), FINISHED).is_empty(), "off screen");

    h.send(Command::AppPhase { phase: AppState::Foreground });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
}

#[tokio::test]
async fn a_show_that_leaves_watching_keeps_its_downloads_and_is_topped_up_no_more() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);

    h.store()
        .replace_library_statuses("anilist", &[(FINISHED, "PAUSED")], FIXTURE_NOW)
        .unwrap();
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead"]);
}

#[tokio::test]
async fn an_episode_asked_for_is_the_viewers_and_one_kept_ahead_becomes_theirs() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::DownloadEpisode { id: FINISHED, episode: "9".into() });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6 ahead", "9"]);

    h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;
    assert_eq!(said(&updates), ["5", "6 ahead", "9"]);
}

#[tokio::test]
async fn an_episode_the_viewer_lets_go_of_is_not_queued_again_until_asked_for() {
    let mut h =
        keeping(Options { config: keep(2), before_start: Some(four_in), ..Options::default() })
            .await;
    h.send(Command::LoadDownloads);
    h.settle().await;
    let six = h.store().download_for(FINISHED, "6").unwrap().unwrap();
    h.send(Command::DownloadCancel { id: six.id });
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["4".into()], watched: true });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead"]);

    h.send(Command::DownloadEpisode { id: FINISHED, episode: "6".into() });
    h.settle().await;
    assert_eq!(queue(h.store(), FINISHED), ["5 ahead", "6"]);
}
