//! Small persistent odds and ends: sync cursors and a title lookup.
//!
//! Two things that would each be over-served by their own table. The key/value side holds
//! cursors — chiefly the last successful library pull per tracker, which is the baseline
//! last-write-wins compares a local edit against, and therefore has to survive a restart or
//! every startup would re-litigate settled conflicts.

use anistream_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store, now};

impl Store {
    /// Read an integer cursor.
    pub fn get_meta_i64(&self, key: &str) -> Result<Option<i64>> {
        self.with_conn(|c| {
            let raw: Option<String> = c
                .query_row("SELECT value FROM app_state WHERE key = ?1", [key], |r| r.get(0))
                .optional()?;
            Ok(raw.and_then(|v| v.parse().ok()))
        })
    }

    /// Write an integer cursor.
    pub fn set_meta_i64(&self, key: &str, value: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO app_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value.to_string()],
            )?;
            Ok(())
        })
    }

    /// Remember a title for an id.
    ///
    /// Called whenever a title passes through the app, so that later — in a sync conflict, or a
    /// history row with no metadata loaded — it can be named rather than shown as a number.
    pub fn remember_title(&self, anilist_id: AnilistId, title: &str) -> Result<()> {
        if title.trim().is_empty() {
            return Ok(());
        }
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO title_index (anilist_id, title, seen_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(anilist_id) DO UPDATE SET title = excluded.title,
                                                      seen_at = excluded.seen_at",
                rusqlite::params![anilist_id.get(), title, now()],
            )?;
            Ok(())
        })
    }

    /// A remembered title, if we have ever seen one.
    pub fn cached_title(&self, anilist_id: AnilistId) -> Result<Option<String>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT title FROM title_index WHERE anilist_id = ?1",
                [anilist_id.get()],
                |r| r.get(0),
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn a_cursor_round_trips_and_overwrites() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.get_meta_i64("last_pull:anilist").unwrap(), None);

        store.set_meta_i64("last_pull:anilist", 1_700).unwrap();
        assert_eq!(store.get_meta_i64("last_pull:anilist").unwrap(), Some(1_700));

        store.set_meta_i64("last_pull:anilist", 1_900).unwrap();
        assert_eq!(store.get_meta_i64("last_pull:anilist").unwrap(), Some(1_900));
    }

    #[test]
    fn cursors_are_independent() {
        // Shared, one tracker's pull would reset the other's last-write-wins baseline and
        // resurrect conflicts the user had already settled.
        let store = Store::open_in_memory().unwrap();
        store.set_meta_i64("last_pull:anilist", 100).unwrap();
        store.set_meta_i64("last_pull:mal", 200).unwrap();
        assert_eq!(store.get_meta_i64("last_pull:anilist").unwrap(), Some(100));
        assert_eq!(store.get_meta_i64("last_pull:mal").unwrap(), Some(200));
    }

    #[test]
    fn a_title_round_trips() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.cached_title(FRIEREN).unwrap(), None);
        store.remember_title(FRIEREN, "Sousou no Frieren").unwrap();
        assert_eq!(store.cached_title(FRIEREN).unwrap(), Some("Sousou no Frieren".into()));
    }

    #[test]
    fn a_blank_title_is_not_remembered() {
        // Storing an empty string would make the fallback ("anilist 154587") unreachable, and
        // a nameless row is worse than a numbered one.
        let store = Store::open_in_memory().unwrap();
        store.remember_title(FRIEREN, "   ").unwrap();
        assert_eq!(store.cached_title(FRIEREN).unwrap(), None);
    }

    #[test]
    fn a_retitled_show_takes_the_newer_name() {
        // AniList does rename entries — usually romaji to an official English title.
        let store = Store::open_in_memory().unwrap();
        store.remember_title(FRIEREN, "Sousou no Frieren").unwrap();
        store.remember_title(FRIEREN, "Frieren: Beyond Journey's End").unwrap();
        assert_eq!(
            store.cached_title(FRIEREN).unwrap(),
            Some("Frieren: Beyond Journey's End".into())
        );
    }
}
