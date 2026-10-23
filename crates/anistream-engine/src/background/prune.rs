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

/// Days of logs kept. Long enough to cover the week before something went wrong, which is
/// how far back anyone looks; the logs were otherwise kept for as long as the app was installed.
const LOG_DAYS: u64 = 14;

/// How long the player's scratch files outlive the session that wrote them. A session writes
/// its subtitles and Up Next still when it starts and mpv reads them within the episode, so a
/// day is far past any live session's need, and a stale socket is a minute past its process.
const SCRATCH_AGE: Duration = Duration::from_secs(24 * 3600);
#[cfg(unix)]
const SOCKET_AGE: Duration = Duration::from_secs(60);

/// What a launch-time sweep removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pruned {
    pub logs: usize,
    pub images: usize,
    pub scratch: usize,
}

/// Sweep what the app accumulates besides the torrent cache: old daily logs, cover art past its
/// cache life, and the player's leftovers in the runtime directory.
///
/// Blocking; run it on a blocking thread. Every failure is skipped: this is housekeeping, and a
/// file that cannot be removed today can be tried at the next launch.
pub fn sweep_at_launch(paths: &anistream_core::config::Paths) -> Pruned {
    let now = std::time::SystemTime::now();
    let pruned = Pruned {
        logs: prune_logs(&paths.cache_dir.join("logs"), now),
        images: prune_older_than(
            &paths.image_cache(),
            Duration::from_secs(crate::artwork::CACHE_TTL_SECS),
            now,
            |name| name.ends_with(".img"),
        ),
        scratch: prune_runtime(&paths.runtime_dir(), now),
    };
    if pruned != Pruned::default() {
        tracing::info!(
            logs = pruned.logs,
            images = pruned.images,
            scratch = pruned.scratch,
            "pruned old files"
        );
    }
    pruned
}

/// Daily logs past [`LOG_DAYS`], by when they were last written — today's is being written.
///
/// The terminal's `anistream.log.<date>` and the apps' `app.log.<date>`, which share the
/// directory, so whichever launches sweeps both.
fn prune_logs(dir: &Path, now: std::time::SystemTime) -> usize {
    prune_older_than(dir, Duration::from_secs(LOG_DAYS * 86_400), now, |name| {
        name.starts_with("anistream.log") || name.starts_with("app.log")
    })
}

/// Remove the files in `dir` that `matches` names and that were last written `age` or more ago.
/// Not recursive, and never a directory or a link.
fn prune_older_than(
    dir: &Path,
    age: Duration,
    now: std::time::SystemTime,
    matches: impl Fn(&str) -> bool,
) -> usize {
    let Ok(listing) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for item in listing.flatten() {
        let name = item.file_name();
        if !name.to_str().is_some_and(&matches) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(item.path()) else { continue };
        if !meta.is_file() {
            continue;
        }
        let old = meta
            .modified()
            .ok()
            .and_then(|at| now.duration_since(at).ok())
            .is_some_and(|elapsed| elapsed >= age);
        if old && std::fs::remove_file(item.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// The player's runtime directory: sockets no player is listening on, and the subtitles and
/// Up Next stills sessions left behind.
///
/// Another instance may be playing right now, so nothing it could be using goes: a socket goes
/// only when connecting to it is refused, and a scratch file only once it is a day old.
fn prune_runtime(dir: &Path, now: std::time::SystemTime) -> usize {
    let mut removed = prune_older_than(dir, SCRATCH_AGE, now, |name| {
        name.starts_with("upnext-") && name.ends_with(".bgra")
    });
    removed += prune_older_than(&dir.join("subs"), SCRATCH_AGE, now, |_| true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;
        let Ok(listing) = std::fs::read_dir(dir) else { return removed };
        for item in listing.flatten() {
            let name = item.file_name();
            let Some(name) = name.to_str() else { continue };
            if !(name.starts_with("mpv-") && name.ends_with(".sock")) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(item.path()) else { continue };
            // Just made, or not a socket at all: not this sweep's to judge.
            let settled = meta
                .modified()
                .ok()
                .and_then(|at| now.duration_since(at).ok())
                .is_some_and(|elapsed| elapsed >= SOCKET_AGE);
            if !meta.file_type().is_socket() || !settled {
                continue;
            }
            // A player answering is a live session, whoever's it is.
            if std::os::unix::net::UnixStream::connect(item.path()).is_ok() {
                continue;
            }
            if std::fs::remove_file(item.path()).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aged(path: &Path, bytes: &[u8], days: u64) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, bytes).unwrap();
        let at = std::time::SystemTime::now() - Duration::from_secs(days * 86_400);
        std::fs::File::options().write(true).open(path).unwrap().set_modified(at).unwrap();
    }

    #[test]
    fn logs_older_than_two_weeks_go_and_the_rest_stay() {
        let dir = tempfile::tempdir().unwrap();
        aged(&dir.path().join("anistream.log.2026-08-01"), b"old", 78);
        aged(&dir.path().join("anistream.log.2026-10-04"), b"older than the window", 15);
        aged(&dir.path().join("anistream.log.2026-10-10"), b"recent", 8);
        aged(&dir.path().join("anistream.log.2026-10-18"), b"today", 0);
        aged(&dir.path().join("notes.txt"), b"not ours", 90);

        assert_eq!(prune_logs(dir.path(), std::time::SystemTime::now()), 2);
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["anistream.log.2026-10-10", "anistream.log.2026-10-18", "notes.txt"]);
    }

    #[test]
    fn the_apps_logs_are_kept_two_weeks_too() {
        // The apps write their own daily file beside the terminal's, and it was never removed.
        let dir = tempfile::tempdir().unwrap();
        aged(&dir.path().join("app.log.2026-08-01"), b"old", 78);
        aged(&dir.path().join("app.log.2026-10-04"), b"older than the window", 15);
        aged(&dir.path().join("app.log.2026-10-10"), b"recent", 8);
        aged(&dir.path().join("app.log.2026-10-18"), b"today", 0);
        aged(&dir.path().join("application notes.txt"), b"not ours", 90);

        assert_eq!(prune_logs(dir.path(), std::time::SystemTime::now()), 2);
        let mut left: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(left, ["app.log.2026-10-10", "app.log.2026-10-18", "application notes.txt"]);
    }

    #[test]
    fn covers_past_their_cache_life_are_removed() {
        // The cache's month-long life was only ever checked on read, so a cover never shown
        // again stayed on disk for good.
        let dir = tempfile::tempdir().unwrap();
        aged(&dir.path().join("0011223344556677.img"), b"stale", 40);
        aged(&dir.path().join("8899aabbccddeeff.img"), b"fresh", 2);
        let ttl = Duration::from_secs(crate::artwork::CACHE_TTL_SECS);
        let removed = prune_older_than(dir.path(), ttl, std::time::SystemTime::now(), |n| {
            n.ends_with(".img")
        });
        assert_eq!(removed, 1);
        assert!(dir.path().join("8899aabbccddeeff.img").exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_runtime_sweep_leaves_a_live_player_alone() {
        let dir = tempfile::tempdir().unwrap();
        let run = dir.path();
        // A player still listening, and one whose process is gone.
        let live = run.join("mpv-1.sock");
        let _listening = std::os::unix::net::UnixListener::bind(&live).unwrap();
        let dead = run.join("mpv-2.sock");
        drop(std::os::unix::net::UnixListener::bind(&dead).unwrap());
        let old = std::time::SystemTime::now() + Duration::from_secs(120);
        aged(&run.join("upnext-0123456789abcdef.bgra"), b"still", 3);
        aged(&run.join("upnext-fedcba9876543210.bgra"), b"tonight's", 0);
        aged(&run.join("subs/0-English.vtt"), b"WEBVTT", 2);
        aged(&run.join("subs/1-French.vtt"), b"WEBVTT", 0);

        // Swept as if two minutes from now, so both sockets are old enough to judge.
        let removed = prune_runtime(run, old);

        assert_eq!(removed, 3);
        assert!(live.exists(), "a socket a player answers on belongs to a live session");
        assert!(!dead.exists());
        assert!(run.join("upnext-fedcba9876543210.bgra").exists());
        assert!(run.join("subs/1-French.vtt").exists());
        assert!(!run.join("subs/0-English.vtt").exists());
    }

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
