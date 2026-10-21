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
