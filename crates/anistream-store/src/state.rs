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

    /// Read a text value, such as a small JSON document a screen keeps between launches.
    pub fn get_meta_text(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|c| {
            Ok(c.query_row("SELECT value FROM app_state WHERE key = ?1", [key], |r| r.get(0))
                .optional()?)
        })
    }

    /// Write a text value.
    pub fn set_meta_text(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO app_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![key, value],
            )?;
            Ok(())
        })
    }

    /// Every text value whose key starts with `prefix`, with its key.
    pub fn meta_texts_with_prefix(&self, prefix: &str) -> Result<Vec<(String, String)>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT key, value FROM app_state WHERE substr(key, 1, length(?1)) = ?1",
            )?;
            let rows = stmt.query_map([prefix], |r| Ok((r.get(0)?, r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Drop one value.
    pub fn delete_meta(&self, key: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM app_state WHERE key = ?1", [key])?;
            Ok(())
        })
    }

    /// Drop every cursor whose key starts with `prefix`, returning how many there were.
    pub fn clear_meta_prefix(&self, prefix: &str) -> Result<usize> {
        self.with_conn(|c| {
            // `substr` rather than LIKE, so a `_` or `%` in the prefix means itself.
            Ok(c.execute(
                "DELETE FROM app_state WHERE substr(key, 1, length(?1)) = ?1",
                [prefix],
            )?)
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

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    #[test]
    fn clearing_by_prefix_takes_only_what_starts_with_it() {
        let store = Store::open_in_memory().unwrap();
        store.set_meta_i64("pushed_progress:mal:1", 3).unwrap();
        store.set_meta_i64("pushed_progress:mal:2", 4).unwrap();
        store.set_meta_i64("pushed_progress:malx:1", 5).unwrap();
        store.set_meta_i64("last_pull:mal", 6).unwrap();

        assert_eq!(store.clear_meta_prefix("pushed_progress:mal:").unwrap(), 2);
        assert_eq!(store.get_meta_i64("pushed_progress:mal:1").unwrap(), None);
        assert_eq!(store.get_meta_i64("pushed_progress:malx:1").unwrap(), Some(5));
        assert_eq!(store.get_meta_i64("last_pull:mal").unwrap(), Some(6));
        // A wildcard character in the prefix means itself.
        assert_eq!(store.clear_meta_prefix("pushed_progress:_").unwrap(), 0);
    }

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
    fn a_text_value_round_trips_and_overwrites() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.get_meta_text("search:recent").unwrap(), None);
        store.set_meta_text("search:recent", "[]").unwrap();
        store.set_meta_text("search:recent", r#"[{"text":"kaze"}]"#).unwrap();
        assert_eq!(
            store.get_meta_text("search:recent").unwrap().as_deref(),
            Some(r#"[{"text":"kaze"}]"#)
        );
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
        assert_eq!(store.cached_title(HOSHIKUZU).unwrap(), None);
        store.remember_title(HOSHIKUZU, "Hoshikuzu no Tabibito").unwrap();
        assert_eq!(
            store.cached_title(HOSHIKUZU).unwrap(),
            Some("Hoshikuzu no Tabibito".into())
        );
    }

    #[test]
    fn a_blank_title_is_not_remembered() {
        // Storing an empty string would make the fallback ("anilist 154587") unreachable, and
        // a nameless row is worse than a numbered one.
        let store = Store::open_in_memory().unwrap();
        store.remember_title(HOSHIKUZU, "   ").unwrap();
        assert_eq!(store.cached_title(HOSHIKUZU).unwrap(), None);
    }

    #[test]
    fn a_retitled_show_takes_the_newer_name() {
        // AniList does rename entries — usually romaji to an official English title.
        let store = Store::open_in_memory().unwrap();
        store.remember_title(HOSHIKUZU, "Hoshikuzu no Tabibito").unwrap();
        store.remember_title(HOSHIKUZU, "Stardust Wayfarer").unwrap();
        assert_eq!(store.cached_title(HOSHIKUZU).unwrap(), Some("Stardust Wayfarer".into()));
    }
}
