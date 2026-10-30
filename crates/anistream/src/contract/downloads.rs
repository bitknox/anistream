//! The download queue: its rows changed inline, and episodes added to it off the UI thread.
//!
//! Every change republishes the whole queue, before the toast that describes it where there is
//! one. Queueing resolves the episode as playback would, so it is a job; nothing here starts a
//! torrent, which only the download manager beside a live session would.

use anistream_core::stream::{Stream, StreamKind};
use anistream_model::{command::Command, downloads::DownloadRow, event::Event};
use anistream_providers::ProviderRegistry;
use anistream_store::DownloadState;
use anistream_testkit::{fixtures::FINISHED, seed, sources};

use super::{ends, toasts};
use crate::harness::{Harness, Options, names};

/// Each row as `episode state`.
fn queue(update: &Event) -> Vec<String> {
    match update {
        Event::Downloads(rows) => {
            let state = |r: &DownloadRow| anistream_tui::screens::download_state_label(r.state);
            rows.iter().map(|r: &DownloadRow| format!("{} {}", r.episode, state(r))).collect()
        }
        other => panic!("not the queue: {other:?}"),
    }
}

/// The fixture source, offering [`FINISHED`] as a torrent: the one kind of stream that can
/// be downloaded.
fn torrents() -> ProviderRegistry {
    let torrent = Stream {
        provider_id: sources::PROVIDER_ID.into(),
        download_source: Some(
            "magnet:?xt=urn:btih:00000000000000000000000000000000000c0ffe&dn=fixture".into(),
        ),
        ..Stream::new("http://127.0.0.1:9/stream", StreamKind::TorrentHttp)
    };
    let provider = sources::provider().with_streams_for(sources::key(FINISHED), vec![torrent]);
    ProviderRegistry::new(vec![provider.arc()])
}

#[tokio::test]
async fn opening_downloads_publishes_the_queue_at_once() {
    let mut h = Harness::new().await;
    seed::download(h.store(), FINISHED, "1", DownloadState::Queued);
    seed::download(h.store(), FINISHED, "2", DownloadState::Done);
    let job = h.send(Command::LoadDownloads);
    let updates = h.settle().await;

    assert!(job.is_none(), "read from disk, nothing to wait on");
    assert_eq!(names(&updates), ["Downloads"]);
    assert_eq!(queue(&updates[0]), ["1 Queued", "2 Downloaded"]);
}

#[tokio::test]
async fn pausing_toggles_the_row_and_leaves_finished_ones_alone() {
    let mut h = Harness::new().await;
    let queued = seed::download(h.store(), FINISHED, "1", DownloadState::Queued);
    let done = seed::download(h.store(), FINISHED, "2", DownloadState::Done);
    h.send(Command::DownloadPause { id: queued.id });
    let paused = h.settle().await;
    h.send(Command::DownloadPause { id: queued.id });
    let resumed = h.settle().await;
    h.send(Command::DownloadPause { id: done.id });
    let unmoved = h.settle().await;

    for updates in [&paused, &resumed, &unmoved] {
        assert_eq!(names(updates), ["Downloads"]);
    }
    assert_eq!(queue(&paused[0]), ["1 Paused", "2 Downloaded"]);
    assert_eq!(queue(&resumed[0]), ["1 Queued", "2 Downloaded"]);
    assert_eq!(queue(&unmoved[0]), ["1 Queued", "2 Downloaded"]);
}

#[tokio::test]
async fn cancelling_takes_the_row_away_then_says_so() {
    let mut h = Harness::new().await;
    let failed = seed::download(h.store(), FINISHED, "1", DownloadState::Failed);
    seed::download(h.store(), FINISHED, "2", DownloadState::Queued);
    h.send(Command::DownloadCancel { id: failed.id });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Downloads", "Toast"]);
    assert_eq!(queue(&updates[0]), ["2 Queued"]);
    assert_eq!(toasts(&updates), ["info: Download cancelled."]);
}

#[tokio::test]
async fn deleting_takes_the_file_with_the_row() {
    let mut h = Harness::new().await;
    let done = seed::download(h.store(), FINISHED, "1", DownloadState::Done);
    // Somewhere real, rather than the seed's relative path: this one is deleted.
    let file = h.paths().data_dir.join("01.mkv");
    std::fs::create_dir_all(&h.paths().data_dir).unwrap();
    std::fs::write(&file, b"not really a video").unwrap();
    h.store().finish_download(done.id, Some(file.to_str().unwrap())).unwrap();
    h.send(Command::DownloadDelete { id: done.id });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Downloads", "Toast"]);
    assert!(queue(&updates[0]).is_empty());
    assert_eq!(toasts(&updates), ["info: Download and file deleted."]);
    assert!(!file.exists());

    // A file already gone is said as such, and the row still goes.
    let done = seed::download(h.store(), FINISHED, "2", DownloadState::Done);
    h.store().finish_download(done.id, Some(file.to_str().unwrap())).unwrap();
    h.send(Command::DownloadDelete { id: done.id });
    let updates = h.settle().await;
    assert_eq!(toasts(&updates), ["info: Download removed. Its file was already gone."]);
}

#[tokio::test]
async fn clearing_finished_downloads_counts_them() {
    let mut h = Harness::new().await;
    seed::download(h.store(), FINISHED, "1", DownloadState::Done);
    seed::download(h.store(), FINISHED, "2", DownloadState::Queued);
    h.send(Command::DownloadClearCompleted);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Downloads", "Toast"]);
    assert_eq!(queue(&updates[0]), ["2 Queued"]);
    assert_eq!(toasts(&updates), ["info: Cleared one finished download."]);
}

#[tokio::test]
async fn queueing_an_episode_says_what_happened_then_publishes_the_queue() {
    let mut h =
        Harness::with(Options { registry: Some(torrents()), ..Options::default() }).await;
    let job = h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "Status", "Downloads", "JobDone"]);
    assert_eq!(toasts(&updates), ["info: Queued episode 5 of Stardust Wayfarer."]);
    assert_eq!(queue(&updates[2]), ["5 Queued"]);
    ends(&updates, job);

    // Asked again: no second resolve, and no second row.
    let job = h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "Status", "Downloads", "JobDone"]);
    assert_eq!(
        toasts(&updates),
        ["info: Episode 5 of Stardust Wayfarer is already in downloads."]
    );
    assert_eq!(queue(&updates[2]), ["5 Queued"]);
    ends(&updates, job);
}

#[tokio::test]
async fn queueing_a_failed_episode_again_retries_it_in_place() {
    let mut h =
        Harness::with(Options { registry: Some(torrents()), ..Options::default() }).await;
    let failed = seed::download(h.store(), FINISHED, "5", DownloadState::Failed);
    let job = h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Toast", "Status", "Downloads", "JobDone"]);
    assert_eq!(toasts(&updates), ["info: Queued episode 5 of Stardust Wayfarer."]);
    let Event::Downloads(rows) = &updates[2] else { unreachable!() };
    assert_eq!(rows.len(), 1, "the same row, not a second one");
    assert_eq!(
        (rows[0].id, rows[0].state, rows[0].error.as_deref()),
        (failed.id, anistream_model::downloads::DownloadState::Queued, None)
    );
    ends(&updates, job);
}

#[tokio::test]
async fn an_episode_that_only_streams_is_refused_in_the_same_shape() {
    let mut h = Harness::new().await;
    let job = h.send(Command::DownloadEpisode { id: FINISHED, episode: "5".into() });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Toast", "Status", "Downloads", "JobDone"]);
    assert_eq!(
        toasts(&updates),
        ["alert: This source streams episodes but can't download them."]
    );
    assert!(queue(&updates[2]).is_empty());
    ends(&updates, job);
}

#[tokio::test]
async fn a_range_publishes_each_row_as_it_lands_then_sums_up() {
    let mut h =
        Harness::with(Options { registry: Some(torrents()), ..Options::default() }).await;
    seed::download(h.store(), FINISHED, "5", DownloadState::Queued);
    let job = h.send(Command::DownloadMany {
        id: FINISHED,
        episodes: vec!["6".into(), "7".into(), "5".into()],
    });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Downloads", "Downloads", "Downloads", "Toast", "JobDone"]);
    // Which rows each publish holds, not their order: rows queued within one second list by
    // insertion and rows either side of a second boundary by time, so 6 and 7 trade places
    // when the range crosses one. That is the app's own order, kept as it is.
    let rows = |update: &Event| {
        let mut rows = queue(update);
        rows.sort();
        rows
    };
    assert_eq!(rows(&updates[0]), ["5 Queued", "6 Queued"]);
    assert_eq!(rows(&updates[1]), ["5 Queued", "6 Queued", "7 Queued"]);
    assert_eq!(
        toasts(&updates),
        ["info: Queued 2 episodes. One episode is already in downloads."]
    );
    ends(&updates, job);

    // Nothing that can be downloaded: no rows, one alert.
    let mut h = Harness::new().await;
    let job =
        h.send(Command::DownloadMany { id: FINISHED, episodes: vec!["1".into(), "2".into()] });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(
        toasts(&updates),
        ["alert: Nothing queued. This source streams episodes but can't download them."]
    );
    ends(&updates, job);
}
