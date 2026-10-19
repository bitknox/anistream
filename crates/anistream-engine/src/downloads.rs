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
use anistream_model::{event::Event, notice::Notice};
use anistream_providers::{ProviderRegistry, torrent::TorrentSession};
use anistream_store::{Download, DownloadState, Store};
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
    config: crate::SharedConfig,
    tx: mpsc::UnboundedSender<Event>,
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
            // Snapshotted per tick, so a folder or seeding change applies to the next
            // download to finish rather than the next launch. Copied out rather than held:
            // a `std` guard across an `await` would make this future non-`Send`.
            let current =
                config.read().map(|c| c.clone()).unwrap_or_else(|e| e.into_inner().clone());
            poll_running(&store, &session, &current, &tx, &mut running).await;
            start_queued(&store, &session, &tx, &mut running).await;
        }
    })
}

/// Advance every running download and retire the ones that finished.
async fn poll_running(
    store: &Store,
    session: &TorrentSession,
    config: &Config,
    tx: &mpsc::UnboundedSender<Event>,
    running: &mut Vec<Running>,
) {
    let mut finished = Vec::new();
    // How many queued rows each torrent backs, counted before the walk so a cancellation can
    // tell "this torrent is mine alone" from "another episode is still fetching from it".
    let mut rows_per_torrent: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    for entry in running.iter() {
        *rows_per_torrent.entry(entry.torrent_id).or_default() += 1;
    }

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
            //
            // Unless another row is still using it. librqbit dedups by infohash, so two
            // episodes queued from one season pack share a torrent id, and deleting its files
            // for the cancelled episode took the other one's data with it — while that other
            // row still read as active. The torrent is only forgotten once nothing else wants
            // it; until then the row simply stops being tracked.
            None => {
                let shared = rows_per_torrent.get(&entry.torrent_id).copied().unwrap_or(0) > 1;
                if shared {
                    tracing::info!(
                        torrent_id = entry.torrent_id,
                        "cancelled, but another queued episode shares this torrent; keeping it"
                    );
                } else {
                    let _ = session.forget(entry.torrent_id, true).await;
                }
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
            let mut final_path = entry.path.clone();
            // Seeding stops here unless asked for. Leaving the torrent in the session
            // advertised the user as a source indefinitely — exactly what the config's
            // privacy default promises not to do silently.
            if !config.downloads.keep_seeding {
                let _ = session.forget(entry.torrent_id, false).await;
                // Only a forgotten file can be relocated — while seeding, the torrent
                // needs it exactly where the session put it.
                if let (Some(path), Some(dir)) =
                    (entry.path.clone(), config.downloads.directory.as_ref())
                {
                    final_path = Some(move_into(path, PathBuf::from(dir)));
                }
            }
            let merged = finish(store, config, &row, final_path.clone()).await;
            let _ = tx.send(Event::Toast(Notice::info(match merged {
                Some(note) => format!("downloaded {} ep {} — {note}", row.title, row.episode),
                None => format!("downloaded {} ep {}", row.title, row.episode),
            })));
            run_completion_hook(config, &row, final_path.as_deref());
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
    tx: &mpsc::UnboundedSender<Event>,
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
                let _ = tx.send(Event::Toast(Notice::alert(format!(
                    "{} ep {}: {reason}",
                    row.title, row.episode
                ))));
            }
        }
        publish(store, tx);
    }
}

/// Run the user's `downloads.on_complete` command, detached and best-effort.
///
/// Through the platform shell, so a config line can be a real pipeline. The hook gets
/// the facts as environment variables rather than argv — no quoting rules to explain,
/// no title with a quote in it breaking anything.
fn run_completion_hook(config: &Config, row: &Download, path: Option<&std::path::Path>) {
    let Some(hook) = config.downloads.on_complete.clone() else { return };
    let (title, episode) = (row.title.clone(), row.episode.clone());
    let path = path.map(|p| p.display().to_string()).unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("sh", "-c") };
        let outcome = std::process::Command::new(shell)
            .arg(flag)
            .arg(&hook)
            .env("ANISTREAM_PATH", &path)
            .env("ANISTREAM_TITLE", &title)
            .env("ANISTREAM_EPISODE", &episode)
            // The TUI owns the terminal; a chatty hook must not paint over it.
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match outcome {
            Ok(status) if status.success() => {}
            Ok(status) => tracing::warn!(%status, "on_complete hook exited nonzero"),
            Err(e) => tracing::warn!(error = %e, "on_complete hook failed to start"),
        }
    });
}

/// A path in `dir` for `name` that does not already exist.
///
/// `rename` and `copy` both destroy whatever is at the destination, and release names collide
/// readily: the same episode from two groups, or a re-queue of something already fetched. The
/// downloads directory holds files the user chose to keep, so a completed download must never
/// be the reason one disappears. A suffix is added instead, the way a browser does it.
fn unique_destination(dir: &std::path::Path, name: &std::ffi::OsStr) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let name = std::path::Path::new(name);
    let stem = name.file_stem().unwrap_or(name.as_os_str()).to_string_lossy().into_owned();
    let extension = name.extension().map(|e| format!(".{}", e.to_string_lossy()));
    // Bounded: past a few hundred the directory is telling us something other than "try again".
    for n in 2..1000 {
        let attempt =
            dir.join(format!("{stem} ({n}){}", extension.as_deref().unwrap_or_default()));
        if !attempt.exists() {
            return attempt;
        }
    }
    candidate
}

/// Move a finished download into the configured directory, falling back to copy+delete
/// across filesystems. On any failure the file stays where it was — a download in the
/// wrong folder beats one lost in transit. Rename is instant on the same filesystem,
/// which is the overwhelmingly common case; the copy fallback is rare enough to wear.
fn move_into(path: PathBuf, dir: PathBuf) -> PathBuf {
    let Some(name) = path.file_name() else { return path };
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, dir = %dir.display(), "could not create downloads dir");
        return path;
    }
    let dest = unique_destination(&dir, name);
    if dest == path {
        return path;
    }
    match std::fs::rename(&path, &dest) {
        Ok(()) => dest,
        Err(_) => match std::fs::copy(&path, &dest) {
            Ok(_) => {
                let _ = std::fs::remove_file(&path);
                dest
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not move download into downloads dir");
                path
            }
        },
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

fn publish(store: &Store, tx: &mpsc::UnboundedSender<Event>) {
    if let Ok(rows) = store.downloads() {
        let _ = tx.send(Event::Downloads(rows.iter().map(to_row).collect()));
    }
}

/// Flatten a stored download for the screen.
pub fn to_row(download: &Download) -> anistream_model::downloads::DownloadRow {
    anistream_model::downloads::DownloadRow {
        id: download.id,
        anilist_id: download.anilist_id,
        title: download.title.clone(),
        episode: download.episode.clone(),
        state: row_state(download.state),
        fraction: download.fraction(),
        downloaded: download.downloaded,
        total: download.total,
        error: download.error.clone(),
        path: download.path.clone(),
    }
}

/// The queue's state for a download, as its row has it.
fn row_state(state: DownloadState) -> anistream_model::downloads::DownloadState {
    use anistream_model::downloads::DownloadState as Row;
    match state {
        DownloadState::Queued => Row::Queued,
        DownloadState::Active => Row::Active,
        DownloadState::Paused => Row::Paused,
        DownloadState::Done => Row::Done,
        DownloadState::Failed => Row::Failed,
    }
}

/// Publish the queue as it currently stands, for a screen that has just been opened.
pub fn publish_now(store: &Store, tx: &mpsc::UnboundedSender<Event>) {
    publish(store, tx);
}

/// Why an episode was not queued, and whether the same answer holds for the rest of the title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// About this episode only: no source has it, and the next one may still be there.
    Episode(String),
    /// About the title: it could not be matched, or its source streams but cannot be downloaded.
    /// Asking again for another episode would walk the same chain to the same answer.
    Title(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Episode(reason) | Self::Title(reason) => f.write_str(reason),
        }
    }
}

/// Whether an episode [`enqueue`] did not refuse was queued just now or was already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Queued {
    New,
    /// Queued, running, paused or finished before this was asked, so nothing was resolved.
    Existing,
}

/// What a range came to, for the toast that reports it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RangeOutcome {
    /// Rows this range added.
    pub queued: usize,
    /// Episodes that already had a row. Counted apart, because "queued 2 episodes" for two that
    /// were in the queue before claimed work that was never done.
    pub existing: usize,
    /// The last reason an episode was not queued.
    pub failed: Option<String>,
}

impl RangeOutcome {
    /// Worded from what happened: what was added, what was there already, and what stopped it.
    pub fn toast(&self) -> Notice {
        use anistream_model::wording::plural;
        let mut text = match (self.queued, self.existing) {
            (0, 0) => "nothing queued".to_owned(),
            (0, _) => "nothing new queued".to_owned(),
            (n, _) => format!("queued {}", plural(n, "episode")),
        };
        if self.existing > 0 {
            text.push_str(&format!(" — {} already in downloads", self.existing));
        }
        match (&self.failed, self.queued, self.existing) {
            (None, ..) => Notice::info(text),
            (Some(reason), 0, 0) => Notice::alert(format!("{text}: {reason}")),
            (Some(reason), 0, _) => Notice::alert(format!("{text}; {reason}")),
            (Some(reason), ..) => Notice::alert(format!("{text}, then: {reason}")),
        }
    }
}

/// Queue a range of episodes one at a time, stopping at the first refusal that holds for the
/// whole title.
///
/// Without the stop, a source that streams but cannot download was asked again for every episode
/// left — about fifty seconds for six, with the job still running and the Downloads screen empty
/// — to arrive at the answer it gave for the first.
pub async fn enqueue_each<F, Fut>(episodes: &[String], mut enqueue: F) -> RangeOutcome
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Queued, Refusal>>,
{
    let mut outcome = RangeOutcome::default();
    for episode in episodes {
        match enqueue(episode.clone()).await {
            Ok(Queued::New) => outcome.queued += 1,
            Ok(Queued::Existing) => outcome.existing += 1,
            Err(Refusal::Title(reason)) => {
                outcome.failed = Some(reason);
                return outcome;
            }
            Err(Refusal::Episode(reason)) => outcome.failed = Some(reason),
        }
    }
    outcome
}

/// Queue an episode, resolving a magnet for it first.
///
/// Resolution happens here rather than in the manager because it needs the provider chain and the
/// mapping ladder — the same walk playback does. A download is "play it, but to disk".
pub async fn enqueue(
    store: &Store,
    registry: &ProviderRegistry,
    meta: &anistream_meta::Meta,
    anilist_id: AnilistId,
    episode: &str,
    translation: Translation,
) -> Result<(Download, Queued), Refusal> {
    // Already queued or finished: report that instead of resolving again.
    if let Ok(Some(existing)) = store.download_for(anilist_id, episode)
        && existing.state != DownloadState::Failed
    {
        return Ok((existing, Queued::Existing));
    }

    let media = meta.media(anilist_id).await.map_err(|e| Refusal::Title(e.to_string()))?;
    let title = media.title.display();
    let now = anistream_store::now();
    let answer = anistream_providers::first_answer(
        store,
        registry,
        anilist_id,
        &media.match_target(),
        translation,
        now,
        |source, key| async move { source.resolve(&key, episode, translation, now).await },
    )
    .await;
    // Explained before the value is taken: the reasons name which providers failed, and that is
    // the useful half of the message when nothing came back.
    let summary = answer.explain();
    let streams = match answer {
        anistream_providers::Answer::Answered { value, .. } => value,
        anistream_providers::Answer::Ambiguous { .. } => {
            return Err(Refusal::Title(format!("could not match this title: {summary}")));
        }
        // No source has the title: every episode of it would be refused the same way.
        anistream_providers::Answer::Unanswered { matched: false, .. } => {
            return Err(Refusal::Title(format!("could not match this title: {summary}")));
        }
        anistream_providers::Answer::Unanswered { .. } => Vec::new(),
    };
    // Only a torrent can be downloaded. An HLS stream would need remuxing a live playlist, and a
    // deep link is somebody else's player — saying so is better than a mystery failure.
    let magnet = streams
        .iter()
        .find(|s| s.kind == anistream_core::stream::StreamKind::TorrentHttp)
        .and_then(|s| s.download_source.clone())
        .ok_or_else(|| {
            if streams.is_empty() {
                Refusal::Episode(format!("no source has this episode: {summary}"))
            } else {
                Refusal::Title("this source streams but cannot be downloaded".to_string())
            }
        })?;

    // A store that cannot write one row will not write the next.
    store
        .enqueue_download(anilist_id, episode, title, &magnet)
        .map(|row| (row, Queued::New))
        .map_err(|e| Refusal::Title(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn episodes(n: u32) -> Vec<String> {
        (1..=n).map(|n| n.to_string()).collect()
    }

    #[tokio::test]
    async fn a_range_stops_at_the_first_refusal_that_holds_for_the_title() {
        let mut asked = Vec::new();
        let outcome = enqueue_each(&episodes(6), |episode| {
            asked.push(episode.clone());
            async move {
                match episode.as_str() {
                    "1" => Ok(Queued::New),
                    _ => Err(Refusal::Title("cannot be downloaded".into())),
                }
            }
        })
        .await;
        assert_eq!(outcome.queued, 1);
        assert_eq!(outcome.failed.as_deref(), Some("cannot be downloaded"));
        assert_eq!(asked, ["1", "2"], "the rest of the range was asked for anyway");
    }

    #[tokio::test]
    async fn a_missing_episode_does_not_stop_the_rest() {
        let outcome = enqueue_each(&episodes(4), |episode| async move {
            match episode.as_str() {
                "2" => Err(Refusal::Episode("no source has this episode".into())),
                _ => Ok(Queued::New),
            }
        })
        .await;
        assert_eq!(outcome.queued, 3);
        assert_eq!(outcome.failed.as_deref(), Some("no source has this episode"));
    }

    /// Episodes already in the queue were counted as queued, so a range that added nothing said
    /// "queued 2 episodes, then: …".
    #[tokio::test]
    async fn a_range_counts_rows_that_were_already_there_apart() {
        let range: Vec<String> = (3..=8).map(|n| n.to_string()).collect();
        let outcome = enqueue_each(&range, |episode| async move {
            match episode.as_str() {
                "3" | "4" => Ok(Queued::Existing),
                _ => Err(Refusal::Title("this source streams but cannot be downloaded".into())),
            }
        })
        .await;
        assert_eq!((outcome.queued, outcome.existing), (0, 2));
        let toast = outcome.toast();
        assert_eq!(
            toast.text,
            "nothing new queued — 2 already in downloads; \
             this source streams but cannot be downloaded"
        );
        assert_eq!(toast.kind, anistream_model::notice::NoticeKind::Alert);

        let mixed = RangeOutcome { queued: 2, existing: 1, failed: None };
        assert_eq!(mixed.toast().text, "queued 2 episodes — 1 already in downloads");
        let stopped = RangeOutcome { queued: 1, existing: 0, failed: Some("why".into()) };
        assert_eq!(stopped.toast().text, "queued 1 episode, then: why");
        let refused = RangeOutcome { failed: Some("why".into()), ..RangeOutcome::default() };
        assert_eq!(refused.toast().text, "nothing queued: why");
    }
}
