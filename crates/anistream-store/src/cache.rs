//! The media cache: the last successful metadata answer for each title.
//!
//! Present in the schema since 0001 and wired the day AniList's instability made the
//! reason obvious: metadata should be fetched fresh whenever a fresh source answers,
//! but "no source answered" must degrade to *yesterday's* data rather than to a screen
//! that cannot play an episode watched thirty times. Every successful fetch rewrites
//! its row, so the cache is exactly as stale as the last outage is long.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

impl Store {
    /// Upsert one title's serialized metadata.
    pub fn cache_media(&self, anilist_id: AnilistId, payload: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO media_cache (anilist_id, payload, fetched_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(anilist_id) DO UPDATE
                    SET payload = excluded.payload, fetched_at = excluded.fetched_at",
                rusqlite::params![anilist_id.get(), payload, now],
            )?;
            Ok(())
        })
    }

    /// How many titles the cache can answer for — the Providers screen's number.
    pub fn cached_media_count(&self) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row("SELECT COUNT(*) FROM media_cache", [], |r| r.get(0))?)
        })
    }

    /// The last cached payload and when it was fetched.
    pub fn cached_media(&self, anilist_id: AnilistId) -> Result<Option<(String, i64)>> {
        self.with_conn(|c| {
            let row = c
                .query_row(
                    "SELECT payload, fetched_at FROM media_cache WHERE anilist_id = ?1",
                    [anilist_id.get()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            Ok(row)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn a_fresh_fetch_replaces_the_stale_row() {
        let store = Store::open_in_memory().unwrap();
        store.cache_media(FRIEREN, r#"{"id":154587,"old":true}"#, 1_000).unwrap();
        store.cache_media(FRIEREN, r#"{"id":154587,"old":false}"#, 2_000).unwrap();

        let (payload, at) = store.cached_media(FRIEREN).unwrap().expect("cached");
        assert!(payload.contains("false"));
        assert_eq!(at, 2_000);
    }

    #[test]
    fn a_title_never_fetched_is_simply_absent() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.cached_media(FRIEREN).unwrap().is_none());
    }
}
