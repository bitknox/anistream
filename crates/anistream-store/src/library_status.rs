//! Where each title is on the signed-in tracker's list, as its last answer said: see
//! `0008_library_status`. A copy of that answer, so a title's page can say which of its related
//! titles were watched without a request of its own.

use anistream_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store};

impl Store {
    /// Put `tracker`'s whole list in place of what was kept of it: each title and its status,
    /// as the tracker words it (`COMPLETED`). A title no longer on the list is on none.
    pub fn replace_library_statuses(
        &self,
        tracker: &str,
        statuses: &[(AnilistId, &str)],
        at: i64,
    ) -> Result<()> {
        self.with_tx(|tx| {
            // Every row, whichever tracker wrote it: there is one list, the Library's.
            tx.execute("DELETE FROM library_status", [])?;
            let mut insert = tx.prepare(
                "INSERT INTO library_status (anilist_id, status, tracker_id, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id) DO UPDATE SET status = excluded.status",
            )?;
            for (id, status) in statuses {
                insert.execute(rusqlite::params![id.get(), status, tracker, at])?;
            }
            Ok(())
        })
    }

    /// The shows being watched: on the list as watching or rewatching, when `listed` says the
    /// list is a signed-in tracker's, and watched here since `since`, unless the list has them
    /// dropped or paused, or they were taken off Continue. The most recently watched here
    /// first, then by id. The new-episode check follows the same shows.
    pub fn watching(&self, since: i64, listed: bool) -> Result<Vec<AnilistId>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT id FROM (
                     SELECT s.anilist_id AS id, COALESCE(p.updated_at, 0) AS at
                       FROM library_status s
                       LEFT JOIN watch_progress p ON p.anilist_id = s.anilist_id
                      WHERE ?2 AND s.status IN ('CURRENT', 'REPEATING')
                     UNION ALL
                     SELECT p.anilist_id, p.updated_at FROM watch_progress p
                      WHERE p.hidden = 0 AND p.updated_at >= ?1
                        AND NOT (?2 AND EXISTS (
                            SELECT 1 FROM library_status s
                             WHERE s.anilist_id = p.anilist_id
                               AND s.status IN ('DROPPED', 'PAUSED')))
                 )
                 GROUP BY id
                 ORDER BY MAX(at) DESC, id",
            )?;
            let ids = statement
                .query_map(rusqlite::params![since, listed], |r| r.get::<_, u32>(0))?
                .map(|id| id.map(AnilistId::new))
                .collect::<rusqlite::Result<_>>()?;
            Ok(ids)
        })
    }

    /// The titles the list has with `status` (as the tracker words it: `PLANNING`), newest ids
    /// first: the newest titles are the likeliest to be airing.
    pub fn library_with_status(&self, status: &str) -> Result<Vec<AnilistId>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id FROM library_status WHERE status = ?1 ORDER BY anilist_id DESC",
            )?;
            let ids = statement
                .query_map([status], |r| r.get::<_, u32>(0))?
                .map(|id| id.map(AnilistId::new))
                .collect::<rusqlite::Result<_>>()?;
            Ok(ids)
        })
    }

    /// Where `id` is on the list, as the tracker words it, when it is on it.
    pub fn library_status(&self, id: AnilistId) -> Result<Option<String>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT status FROM library_status WHERE anilist_id = ?1",
                [id.get()],
                |r| r.get(0),
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: AnilistId = AnilistId::new(900_001);
    const TWO: AnilistId = AnilistId::new(900_003);

    #[test]
    fn a_list_is_kept_and_read_back_by_title() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.library_status(ONE).unwrap(), None, "nothing pulled yet");
        store
            .replace_library_statuses("anilist", &[(ONE, "COMPLETED"), (TWO, "PLANNING")], 10)
            .unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("COMPLETED"));
        assert_eq!(store.library_status(TWO).unwrap().as_deref(), Some("PLANNING"));
    }

    #[test]
    fn the_shows_being_watched_are_the_lists_and_the_ones_watched_here_lately() {
        let store = Store::open_in_memory().unwrap();
        const THREE: AnilistId = AnilistId::new(900_005);
        const FOUR: AnilistId = AnilistId::new(900_006);
        const FIVE: AnilistId = AnilistId::new(900_002);
        store
            .replace_library_statuses(
                "anilist",
                &[(ONE, "CURRENT"), (TWO, "REPEATING"), (THREE, "PAUSED"), (FOUR, "COMPLETED")],
                10,
            )
            .unwrap();
        assert_eq!(store.watching(100, true).unwrap(), [ONE, TWO], "by id, nothing watched");
        assert!(store.watching(100, false).unwrap().is_empty(), "nobody signed in to it");

        let watch = |id, at| {
            let mut event = crate::WatchEvent::new(id, "1", 30.0);
            event.at = at;
            store.record_event(&event).unwrap();
        };
        watch(TWO, 150);
        // Watched here lately: one paused on the list, one completed, one not on it at all.
        watch(THREE, 200);
        watch(FOUR, 210);
        watch(FIVE, 220);
        // And one watched too long ago.
        watch(AnilistId::new(900_003), 50);
        assert_eq!(store.watching(100, true).unwrap(), [FIVE, FOUR, TWO, ONE]);
        assert_eq!(store.watching(100, false).unwrap(), [FIVE, FOUR, THREE, TWO]);
        store.hide_from_continue(FIVE).unwrap();
        assert_eq!(store.watching(100, false).unwrap(), [FOUR, THREE, TWO]);
    }

    #[test]
    fn each_answer_replaces_the_last_whole() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library_statuses("anilist", &[(ONE, "CURRENT"), (TWO, "PLANNING")], 10)
            .unwrap();
        store.replace_library_statuses("anilist", &[(ONE, "COMPLETED")], 20).unwrap();
        assert_eq!(store.library_status(ONE).unwrap().as_deref(), Some("COMPLETED"));
        assert_eq!(store.library_status(TWO).unwrap(), None, "taken off the list");
    }
}
