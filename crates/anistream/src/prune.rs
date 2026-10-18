//! Keeping what anistream writes to disk from growing without end.
//!
//! The torrent cache is the one that matters: a stream is gigabytes, and nothing else removes
//! one. Removal itself lives with the session and the cache module in `anistream-providers`;
//! what is here is deciding *when*, and what must be left alone — which only the app knows,
//! because downloads live in its database.

use std::{
    collections::HashSet,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anistream_providers::torrent::{TorrentSession, cache};
use anistream_store::Store;

/// One sweep of the torrent cache at a time.
///
/// The launch-time sweep and a torrent session coming up can otherwise overlap — a source
/// enabled from Settings seconds after launch — and a sweep that read the directory before the
/// session started could remove the folder the session has just begun writing into.
static TORRENT_SWEEP: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// How often the cap is enforced while a session runs. A stream grows by an episode in minutes,
/// so this bounds the overshoot without walking the directory for nothing.
const TORRENT_SWEEP_EVERY: Duration = Duration::from_secs(120);

/// The cache entries that hold a download, in any state.
///
/// `None` when the queue cannot be read. A sweep that does not know which files are the user's
/// must not run at all, so a database hiccup costs disk space rather than a download.
pub fn download_entries(store: &Store, dir: &Path) -> Option<HashSet<OsString>> {
    let downloads = store
        .downloads()
        .inspect_err(|e| tracing::warn!(error = %e, "cannot read downloads; not pruning"))
        .ok()?;
    Some(
        downloads
            .iter()
            .filter_map(|d| d.path.as_deref())
            .filter_map(|path| cache::entry_of(dir, Path::new(path)))
            .collect(),
    )
}

/// Bring the torrent cache within `limit` bytes, leaving downloads and `live` alone.
///
/// Runs on a blocking thread: a cache left uncapped by an older version can hold thousands of
/// files, and walking them must not hold up a frame.
pub async fn sweep_torrent_cache(
    dir: PathBuf,
    limit: u64,
    store: &Store,
    live: HashSet<OsString>,
) -> Option<cache::Swept> {
    let mut keep = download_entries(store, &dir)?;
    keep.extend(live);
    let outcome = tokio::task::spawn_blocking(move || {
        let _one_at_a_time = TORRENT_SWEEP.lock().unwrap_or_else(|e| e.into_inner());
        cache::sweep(&dir, limit, |name| keep.contains(name))
    })
    .await
    .ok()?;
    match outcome {
        Ok(swept) => {
            if swept.removed > 0 {
                tracing::info!(
                    removed = swept.removed,
                    freed_mb = swept.freed / 1_000_000,
                    left_mb = swept.left / 1_000_000,
                    limit_mb = limit / 1_000_000,
                    "torrent cache pruned"
                );
            }
            Some(swept)
        }
        Err(e) => {
            tracing::warn!(error = %e, "could not read the torrent cache");
            None
        }
    }
}

/// Keep the torrent cache within its limit for as long as a session runs.
///
/// Reads the limit each time, so a change on the Settings screen or in the file applies on the
/// next sweep. Everything the session is using — the episode playing, the one fetched ahead,
/// what is seeding, what is downloading — is left alone however far over the limit it is.
pub async fn torrent_cache_watch(
    session: Arc<TorrentSession>,
    store: Store,
    config: crate::SharedConfig,
) {
    let mut ticker = tokio::time::interval(TORRENT_SWEEP_EVERY);
    // The first tick fires at once, and the session has only just been swept on its way up.
    ticker.tick().await;
    loop {
        ticker.tick().await;
        let limit = config
            .read()
            .map(|c| c.providers.torrent.cache_limit_bytes())
            .unwrap_or_else(|e| e.into_inner().providers.torrent.cache_limit_bytes());
        let live = session.live_entries();
        sweep_torrent_cache(session.output_dir().to_path_buf(), limit, &store, live).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_sweep_keeps_every_download_the_queue_knows_about() {
        // A finished download with no downloads directory stays where the session wrote it,
        // in the cache. It is the user's file all the same.
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("torrents");
        std::fs::create_dir_all(dir.join("Kept Batch")).unwrap();
        std::fs::write(dir.join("Kept Batch/Episode 03.mkv"), [0_u8; 64]).unwrap();
        std::fs::write(dir.join("Watched Episode.mkv"), [0_u8; 64]).unwrap();
        std::fs::write(dir.join("Playing Episode.mkv"), [0_u8; 64]).unwrap();

        let store = Store::open_in_memory().unwrap();
        let id = anistream_core::ids::AnilistId::new(7);
        let row = store.enqueue_download(id, "3", "An Invented Show", "magnet:?xt=x").unwrap();
        let file = dir.join("Kept Batch/Episode 03.mkv");
        store.finish_download(row.id, Some(&file.display().to_string())).unwrap();

        let live: HashSet<OsString> = [OsString::from("Playing Episode.mkv")].into();
        let swept = sweep_torrent_cache(dir.clone(), 0, &store, live).await.unwrap();

        assert_eq!(swept.removed, 1);
        assert!(file.exists(), "a download is never pruned");
        assert!(dir.join("Playing Episode.mkv").exists(), "nor is what is playing");
        assert!(!dir.join("Watched Episode.mkv").exists());
    }
}
