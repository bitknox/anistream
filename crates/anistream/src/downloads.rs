//! The download manager.
//!
//! One task owns the queue. It polls the torrent session, writes progress to SQLite, and reports to
//! the UI — the UI never touches librqbit, and librqbit never touches the interface.
//!
//! Three decisions worth stating.
//!
//! **Concurrency is capped.** A swarm gives you a share of its bandwidth, so eight downloads at once
//! is not eight times faster — it is eight things all finishing late, and on a VPN proxy it is also
//! eight times the connection count through one tunnel. The queue drains oldest-first.
//!
//! **Progress is persisted, not just displayed.** A download interrupted by a crash comes back
//! showing where it got to, and librqbit resumes from the partial file rather than from zero.
//!
//! **The VPN guard is re-checked on every start.** A download runs unattended for a long time,
//! which makes it the most important thing to gate, not the least.

use std::{path::PathBuf, sync::Arc, time::Duration};

use anistream_core::{config::Config, ids::AnilistId, media::Translation};
use anistream_providers::{ProviderRegistry, torrent::TorrentSession};
use anistream_store::{Download, DownloadState, Store};
use anistream_ui::app::{Toast, Update};
use tokio::sync::mpsc;

/// How many torrents fetch at once.
///
/// Two, not one: a single stalled swarm should not block the whole queue. Not eight, because a
/// download's speed is set by the swarm rather than by how many you start.
const CONCURRENCY: usize = 2;

/// How often progress is polled and written.
///
/// A second is enough for a progress meter and cheap enough to run for hours. librqbit's stats are
/// in-memory, so the cost here is the SQLite write, not the read.
const POLL: Duration = Duration::from_secs(1);

/// Live state for one running download.
struct Running {
    row_id: i64,
    torrent_id: usize,
    /// Set once metadata has arrived and the file is known.
    path: Option<PathBuf>,
}

/// Run the download queue until the process ends.
pub fn spawn(
    store: Store,
    session: Arc<TorrentSession>,
    config: Config,
    tx: mpsc::UnboundedSender<Update>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut running: Vec<Running> = Vec::new();

        // Anything the last run left mid-flight is back in the queue, not lost. It was `active` in
        // the database and there is no torrent for it in this process, so without this it would sit
        // at "downloading" forever with nothing behind it.
        if let Ok(pending) = store.pending_downloads() {
            for row in pending.iter().filter(|d| d.state == DownloadState::Active) {
                let _ = store.set_download_state(row.id, DownloadState::Queued);
            }
            if !pending.is_empty() {
                tracing::info!(count = pending.len(), "download queue restored");
            }
        }

        loop {
            tokio::time::sleep(POLL).await;
            poll_running(&store, &session, &config, &tx, &mut running).await;
            start_queued(&store, &session, &tx, &mut running).await;
        }
    })
}

/// Advance every running download and retire the ones that finished.
async fn poll_running(
    store: &Store,
    session: &TorrentSession,
    config: &Config,
    tx: &mpsc::UnboundedSender<Update>,
    running: &mut Vec<Running>,
) {
    let mut finished = Vec::new();

    for entry in running.iter_mut() {
        // The row is re-read rather than cached: the user can pause or cancel from the screen, and
        // this loop has to notice.
        let row = match store
            .downloads()
            .ok()
            .and_then(|all| all.into_iter().find(|d| d.id == entry.row_id))
        {
            Some(row) => row,
            // Cancelled out from under us: drop the torrent *and* the partial file. Only running
            // downloads are tracked here, so this can never reach a finished one — cancelling
            // something half-fetched should not leave gigabytes behind that nothing knows about.
            None => {
                let _ = session.forget(entry.torrent_id, true).await;
                finished.push(entry.row_id);
                continue;
            }
        };

        match row.state {
            DownloadState::Paused => {
                let _ = session.pause_one(entry.torrent_id).await;
                continue;
            }
            DownloadState::Done | DownloadState::Failed => {
                finished.push(entry.row_id);
                continue;
            }
            _ => {}
        }

        let Some(progress) = session.progress_of(entry.torrent_id) else {
            // The session has forgotten it, which after a resume it may legitimately have. Requeue
            // rather than fail: the partial file is still on disk and can be picked up again.
            tracing::warn!(row = row.id, "torrent vanished from the session; requeueing");
            let _ = store.set_download_state(row.id, DownloadState::Queued);
            finished.push(entry.row_id);
            continue;
        };

        // Un-pause a torrent whose row has come back. Pausing sets librqbit's own paused flag, and
        // nothing was clearing it — so "resume" moved the row back to `queued`, this loop carried on
        // polling a torrent that had stopped fetching, and the meter sat still forever. Found by
        // looking for the call site `resume_one` did not have.
        if progress.paused {
            match session.resume_one(entry.torrent_id).await {
                Ok(()) => tracing::info!(row = row.id, "download resumed"),
                // Refused means the VPN guard is failing. Back to paused rather than pretending:
                // a download that silently does not run is exactly the failure this app avoids.
                Err(e) => {
                    tracing::warn!(row = row.id, error = %e, "could not resume; pausing again");
                    let _ = store.set_download_state(row.id, DownloadState::Paused);
                    continue;
                }
            }
        }

        if entry.path.is_none() && !progress.name.is_empty() {
            entry.path = Some(session.output_dir().join(&progress.name));
        }
        let path_string = entry.path.as_ref().map(|p| p.display().to_string());
        let _ = store.update_download_progress(
            row.id,
            progress.downloaded,
            progress.total,
            path_string.as_deref(),
        );

        // Complete when every byte has arrived *and* the total is known. Without the second half, a
        // poll before metadata reports 0 of 0 and would look finished.
        if progress.total > 0 && progress.downloaded >= progress.total {
            let merged = finish(store, config, &row, entry.path.clone()).await;
            let _ = tx.send(Update::Toast(Toast::info(match merged {
                Some(note) => format!("downloaded {} ep {} — {note}", row.title, row.episode),
                None => format!("downloaded {} ep {}", row.title, row.episode),
            })));
            finished.push(entry.row_id);
        }
    }

    running.retain(|r| !finished.contains(&r.row_id));
    if !finished.is_empty() {
        publish(store, tx);
    }
}

/// Start downloads while there is room.
async fn start_queued(
    store: &Store,
    session: &TorrentSession,
    tx: &mpsc::UnboundedSender<Update>,
    running: &mut Vec<Running>,
) {
    if running.len() >= CONCURRENCY {
        return;
    }
    let Ok(pending) = store.pending_downloads() else { return };

    for row in pending {
        if running.len() >= CONCURRENCY {
            break;
        }
        if running.iter().any(|r| r.row_id == row.id) {
            continue;
        }
        if row.state != DownloadState::Queued {
            continue;
        }

        let episode = row.episode.trim().parse::<u32>().ok();
        match session.download(&row.magnet, episode).await {
            Ok(handle) => {
                tracing::info!(row = row.id, torrent = handle.id, file = %handle.name, "download started");
                let _ = store.update_download_progress(
                    row.id,
                    0,
                    handle.total,
                    Some(&session.output_dir().join(&handle.name).display().to_string()),
                );
                running.push(Running {
                    row_id: row.id,
                    torrent_id: handle.id,
                    path: Some(session.output_dir().join(&handle.name)),
                });
            }
            Err(e) => {
                // Named on the row rather than only toasted: the screen has to be able to say why
                // afterwards, and a toast lasts seconds.
                let reason = e.to_string();
                tracing::warn!(row = row.id, %reason, "download could not start");
                let _ = store.fail_download(row.id, &reason);
                let _ = tx.send(Update::Toast(Toast::alert(format!(
                    "{} ep {}: {reason}",
                    row.title, row.episode
                ))));
            }
        }
        publish(store, tx);
    }
}

/// Mark a download finished, merging subtitles in if there are any to merge.
async fn finish(
    store: &Store,
    config: &Config,
    row: &Download,
    path: Option<PathBuf>,
) -> Option<String> {
    let path_string = path.as_ref().map(|p| p.display().to_string());
    let _ = store.finish_download(row.id, path_string.as_deref());

    if !config.downloads.merge_subtitles {
        return None;
    }
    let path = path?;
    match crate::remux::merge_sidecar_subtitles(&path).await {
        Ok(crate::remux::Merge::Merged { tracks, .. }) => {
            Some(format!("{tracks} subtitle track(s) merged in"))
        }
        Ok(crate::remux::Merge::NothingToDo) => None,
        Err(e) => {
            // Never fatal. The video is downloaded and playable; a failed remux costs you an
            // external subtitle file, which mpv will pick up beside the video anyway.
            tracing::warn!(error = %e, path = %path.display(), "subtitle merge failed");
            Some(format!("subtitles left as separate files: {e}"))
        }
    }
}

fn publish(store: &Store, tx: &mpsc::UnboundedSender<Update>) {
    if let Ok(rows) = store.downloads() {
        let _ = tx.send(Update::Downloads(rows.iter().map(to_row).collect()));
    }
}

/// Flatten a stored download for the screen.
pub fn to_row(download: &Download) -> anistream_ui::app::DownloadRow {
    anistream_ui::app::DownloadRow {
        id: download.id,
        title: download.title.clone(),
        episode: download.episode.clone(),
        state: download.state.label(),
        fraction: download.fraction(),
        downloaded: download.downloaded,
        total: download.total,
        error: download.error.clone(),
        path: download.path.clone(),
    }
}

/// Publish the queue as it currently stands, for a screen that has just been opened.
pub fn publish_now(store: &Store, tx: &mpsc::UnboundedSender<Update>) {
    publish(store, tx);
}

/// Queue an episode, resolving a magnet for it first.
///
/// Resolution happens here rather than in the manager because it needs the provider chain and the
/// mapping ladder — the same walk playback does. A download is "play it, but to disk".
pub async fn enqueue(
    store: &Store,
    registry: &ProviderRegistry,
    anilist: &anistream_meta::anilist::AniList,
    anilist_id: AnilistId,
    episode: &str,
    translation: Translation,
) -> Result<Download, String> {
    // Already queued or finished: report that instead of resolving again.
    if let Ok(Some(existing)) = store.download_for(anilist_id, episode)
        && existing.state != DownloadState::Failed
    {
        return Ok(existing);
    }

    let media = anilist.media(anilist_id).await.map_err(|e| e.to_string())?;
    let title = media.title.display();
    let now = anistream_store::now();
    let resolution = anistream_providers::resolve(
        store,
        registry,
        anilist_id,
        &media.match_target(),
        translation,
        now,
    )
    .await;
    let key = resolution
        .key()
        .cloned()
        .ok_or_else(|| format!("could not match this title: {}", resolution.explain()))?;

    let attempt = registry.resolve(&key, episode, translation, now).await;
    // Summarised before the value is taken: the summary names which providers failed, and that is
    // the useful half of the message when nothing came back.
    let summary = attempt.summary();
    let streams = attempt.value.unwrap_or_default();
    // Only a torrent can be downloaded. An HLS stream would need remuxing a live playlist, and a
    // deep link is somebody else's player — saying so is better than a mystery failure.
    let magnet = streams
        .iter()
        .find(|s| s.kind == anistream_core::stream::StreamKind::TorrentHttp)
        .and_then(|s| s.download_source.clone())
        .ok_or_else(|| {
            if streams.is_empty() {
                format!("no source has this episode: {summary}")
            } else {
                "this source streams but cannot be downloaded".to_string()
            }
        })?;

    store.enqueue_download(anilist_id, episode, title, &magnet).map_err(|e| e.to_string())
}
