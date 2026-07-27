//! The download queue.
//!
//! Persistent because the *files* outlive the process. A queue held in memory would, after a
//! restart, either re-fetch from zero or leave partial files on disk that nothing knows about —
//! and librqbit can resume a partial torrent, so throwing away the magnet is throwing away the
//! only thing that makes resumption possible.
//!
//! Progress is written here as it changes rather than only at the end. That is a deliberate cost:
//! it means a download interrupted by a crash comes back showing where it actually got to, instead
//! of claiming zero and re-downloading what is already on disk.

use rusqlite::OptionalExtension;

use crate::{Result, Store, now};
use anistream_core::ids::AnilistId;

/// Where a download is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadState {
    /// Accepted, not started. The queue is bounded, so this is a real state and not a formality.
    Queued,
    Active,
    Paused,
    Done,
    Failed,
}

impl DownloadState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    /// Parse a stored value. An unrecognised state is treated as queued rather than dropped: a row
    /// written by a newer build should reappear as something the user can act on, not vanish.
    pub fn parse(value: &str) -> Self {
        match value {
            "active" => Self::Active,
            "paused" => Self::Paused,
            "done" => Self::Done,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }

    /// Whether this download still wants the network.
    pub const fn is_running(self) -> bool {
        matches!(self, Self::Queued | Self::Active)
    }

    /// The label the Downloads screen shows.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Active => "downloading",
            Self::Paused => "paused",
            Self::Done => "complete",
            Self::Failed => "failed",
        }
    }
}

/// One row of the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct Download {
    pub id: i64,
    pub anilist_id: AnilistId,
    pub episode: String,
    pub title: String,
    pub magnet: String,
    pub state: DownloadState,
    pub path: Option<String>,
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Download {
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
    }
}

impl Store {
    /// Add an episode to the queue, or return the existing row.
    ///
    /// Idempotent on `(anilist_id, episode)`. A held-down key must not queue the same file eight
    /// times, and re-requesting something already downloaded should surface *that* rather than
    /// starting again.
    pub fn enqueue_download(
        &self,
        anilist_id: AnilistId,
        episode: &str,
        title: &str,
        magnet: &str,
    ) -> Result<Download> {
        let at = now();
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO download
                    (anilist_id, episode, title, magnet, state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?5)
                 ON CONFLICT(anilist_id, episode) DO UPDATE SET
                    -- A failed download asked for again is a retry, so it re-enters the queue. One
                    -- that is running or finished is left exactly as it is.
                    state = CASE WHEN download.state = 'failed' THEN 'queued' ELSE download.state END,
                    error = CASE WHEN download.state = 'failed' THEN NULL ELSE download.error END,
                    magnet = excluded.magnet,
                    updated_at = ?5",
                rusqlite::params![anilist_id.get(), episode, title, magnet, at],
            )?;
            Ok(())
        })?;
        self.download_for(anilist_id, episode)?
            .ok_or_else(|| crate::StoreError::Missing("download row after insert".into()))
    }

    pub fn download_for(
        &self,
        anilist_id: AnilistId,
        episode: &str,
    ) -> Result<Option<Download>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT id, anilist_id, episode, title, magnet, state, path, downloaded, total,
                        error, created_at, updated_at
                   FROM download WHERE anilist_id = ?1 AND episode = ?2",
                rusqlite::params![anilist_id.get(), episode],
                row_to_download,
            )
            .optional()?)
        })
    }

    /// The whole queue, newest request first.
    pub fn downloads(&self) -> Result<Vec<Download>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, anilist_id, episode, title, magnet, state, path, downloaded, total,
                        error, created_at, updated_at
                   FROM download ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], row_to_download)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Downloads that still want the network, oldest first.
    ///
    /// Oldest first deliberately, unlike the display order: the queue should drain in the order it
    /// was filled, while the screen should show what you just asked for at the top.
    pub fn pending_downloads(&self) -> Result<Vec<Download>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, anilist_id, episode, title, magnet, state, path, downloaded, total,
                        error, created_at, updated_at
                   FROM download
                  WHERE state IN ('queued', 'active')
                  ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], row_to_download)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Record progress against a download.
    pub fn update_download_progress(
        &self,
        id: i64,
        downloaded: u64,
        total: u64,
        path: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE download
                    SET downloaded = ?2,
                        -- Never overwrite a known total with zero: metadata arrives after the
                        -- torrent is added, and a stats poll in between reports nothing.
                        total = CASE WHEN ?3 > 0 THEN ?3 ELSE total END,
                        path = COALESCE(?4, path),
                        state = CASE WHEN state = 'queued' THEN 'active' ELSE state END,
                        updated_at = ?5
                  WHERE id = ?1",
                // Cast at the boundary: SQLite integers are signed, and a torrent will never
                // approach the point where that matters.
                rusqlite::params![id, downloaded as i64, total as i64, path, now()],
            )?;
            Ok(())
        })
    }

    pub fn set_download_state(&self, id: i64, state: DownloadState) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE download SET state = ?2, updated_at = ?3 WHERE id = ?1",
                rusqlite::params![id, state.as_str(), now()],
            )?;
            Ok(())
        })
    }

    /// Mark a download finished, recording where the file ended up.
    pub fn finish_download(&self, id: i64, path: Option<&str>) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE download
                    SET state = 'done', downloaded = total, path = COALESCE(?2, path),
                        error = NULL, updated_at = ?3
                  WHERE id = ?1",
                rusqlite::params![id, path, now()],
            )?;
            Ok(())
        })
    }

    pub fn fail_download(&self, id: i64, reason: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE download SET state = 'failed', error = ?2, updated_at = ?3 WHERE id = ?1",
                rusqlite::params![id, reason, now()],
            )?;
            Ok(())
        })
    }

    pub fn remove_download(&self, id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM download WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Drop every finished row, returning how many went.
    pub fn clear_completed_downloads(&self) -> Result<usize> {
        self.with_conn(|c| Ok(c.execute("DELETE FROM download WHERE state = 'done'", [])?))
    }

    /// How many downloads still want the network — the rail badge.
    pub fn active_download_count(&self) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM download WHERE state IN ('queued', 'active')",
                [],
                |r| r.get(0),
            )?)
        })
    }
}

fn row_to_download(row: &rusqlite::Row<'_>) -> rusqlite::Result<Download> {
    Ok(Download {
        id: row.get(0)?,
        anilist_id: AnilistId::new(row.get::<_, u32>(1)?),
        episode: row.get(2)?,
        title: row.get(3)?,
        magnet: row.get(4)?,
        state: DownloadState::parse(&row.get::<_, String>(5)?),
        path: row.get(6)?,
        downloaded: row.get::<_, i64>(7)?.max(0) as u64,
        total: row.get::<_, i64>(8)?.max(0) as u64,
        error: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: AnilistId = AnilistId::new(154_587);

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn queueing_the_same_episode_twice_is_one_download() {
        // A held-down key must not queue eight copies of one file.
        let store = store();
        let first = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        let second = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(store.downloads().unwrap().len(), 1);
    }

    #[test]
    fn asking_again_for_a_failed_download_retries_it() {
        let store = store();
        let row = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        store.fail_download(row.id, "no peers").unwrap();
        assert_eq!(store.download_for(ID, "1").unwrap().unwrap().state, DownloadState::Failed);

        let retried = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        assert_eq!(retried.state, DownloadState::Queued, "a retry must re-enter the queue");
        assert_eq!(retried.error, None, "and forget the previous reason");
    }

    #[test]
    fn asking_again_for_a_finished_download_does_not_restart_it() {
        // Re-downloading something already on disk is the worst possible interpretation of a
        // second keypress.
        let store = store();
        let row = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        store.finish_download(row.id, Some("/tmp/ep1.mkv")).unwrap();
        let again = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        assert_eq!(again.state, DownloadState::Done);
        assert_eq!(again.path.as_deref(), Some("/tmp/ep1.mkv"));
    }

    #[test]
    fn progress_never_overwrites_a_known_total_with_zero() {
        // Metadata arrives after the torrent is added, so a stats poll in between reports a total
        // of nothing — and letting that through would make the meter jump back to empty.
        let store = store();
        let row = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        store.update_download_progress(row.id, 100, 5_000, None).unwrap();
        store.update_download_progress(row.id, 200, 0, None).unwrap();
        let after = store.download_for(ID, "1").unwrap().unwrap();
        assert_eq!(after.total, 5_000, "the total must survive a poll that does not know it");
        assert_eq!(after.downloaded, 200);
        assert!((after.fraction() - 0.04).abs() < 1e-9);
    }

    #[test]
    fn progress_promotes_a_queued_download_to_active() {
        let store = store();
        let row = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        store.update_download_progress(row.id, 1, 10, None).unwrap();
        assert_eq!(store.download_for(ID, "1").unwrap().unwrap().state, DownloadState::Active);
    }

    #[test]
    fn progress_does_not_resurrect_a_paused_download() {
        // A final stats poll can arrive after the user pauses, and it must not undo that.
        let store = store();
        let row = store.enqueue_download(ID, "1", "Frieren", "magnet:?xt=a").unwrap();
        store.set_download_state(row.id, DownloadState::Paused).unwrap();
        store.update_download_progress(row.id, 5, 10, None).unwrap();
        assert_eq!(store.download_for(ID, "1").unwrap().unwrap().state, DownloadState::Paused);
    }

    #[test]
    fn the_queue_drains_oldest_first_but_displays_newest_first() {
        let store = store();
        for episode in ["1", "2", "3"] {
            // Distinct timestamps: `now()` has second resolution and these would otherwise tie.
            let row = store.enqueue_download(ID, episode, "Frieren", "magnet:?xt=a").unwrap();
            store
                .with_conn(|c| {
                    c.execute(
                        "UPDATE download SET created_at = ?2 WHERE id = ?1",
                        rusqlite::params![row.id, row.id * 100],
                    )?;
                    Ok(())
                })
                .unwrap();
        }
        let displayed: Vec<String> =
            store.downloads().unwrap().into_iter().map(|d| d.episode).collect();
        let draining: Vec<String> =
            store.pending_downloads().unwrap().into_iter().map(|d| d.episode).collect();
        assert_eq!(displayed, vec!["3", "2", "1"], "newest request at the top");
        assert_eq!(draining, vec!["1", "2", "3"], "but the queue is first-in-first-out");
    }

    #[test]
    fn only_running_downloads_count_toward_the_badge() {
        let store = store();
        let a = store.enqueue_download(ID, "1", "Frieren", "m").unwrap();
        let b = store.enqueue_download(ID, "2", "Frieren", "m").unwrap();
        store.enqueue_download(ID, "3", "Frieren", "m").unwrap();
        store.finish_download(a.id, None).unwrap();
        store.set_download_state(b.id, DownloadState::Paused).unwrap();
        assert_eq!(store.active_download_count().unwrap(), 1);
        assert_eq!(store.pending_downloads().unwrap().len(), 1);
    }

    #[test]
    fn clearing_completed_leaves_everything_else() {
        let store = store();
        let done = store.enqueue_download(ID, "1", "Frieren", "m").unwrap();
        store.enqueue_download(ID, "2", "Frieren", "m").unwrap();
        let failed = store.enqueue_download(ID, "3", "Frieren", "m").unwrap();
        store.finish_download(done.id, None).unwrap();
        store.fail_download(failed.id, "no peers").unwrap();

        assert_eq!(store.clear_completed_downloads().unwrap(), 1);
        let left: Vec<DownloadState> =
            store.downloads().unwrap().into_iter().map(|d| d.state).collect();
        assert!(!left.contains(&DownloadState::Done));
        assert_eq!(left.len(), 2, "a failed download is still there to retry");
    }

    #[test]
    fn an_unknown_state_reappears_as_something_actionable() {
        // Forward compatibility: a row written by a newer build must not vanish from the queue.
        assert_eq!(DownloadState::parse("teleporting"), DownloadState::Queued);
        assert!(DownloadState::parse("teleporting").is_running());
    }
}
