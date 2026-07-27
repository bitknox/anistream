//! Persistence for refreshable datasets and the materialised mapping table.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

/// Refresh bookkeeping for one dataset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetState {
    pub name: String,
    pub etag: Option<String>,
    pub fetched_at: Option<i64>,
    pub item_count: Option<u32>,
    pub last_error: Option<String>,
}

/// A merged mapping row: one title's identity across every catalogue that has one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mapping {
    pub anilist_id: u32,
    pub mal_id: Option<u32>,
    pub kitsu_id: Option<u32>,
    pub anidb_id: Option<u32>,
    pub tvdb_id: Option<u32>,
    pub tmdb_id: Option<u32>,
    pub episode_offset: Option<i32>,
}

/// The fields a dataset contributes for one title.
///
/// Deliberately a plain struct rather than the dataset crate's own type, so `anistream-store`
/// does not depend on `anistream-meta` — the dependency runs the other way.
#[derive(Debug, Clone, Default)]
pub struct MappingInput {
    pub anilist_id: u32,
    pub mal_id: Option<u32>,
    pub kitsu_id: Option<u32>,
    pub anidb_id: Option<u32>,
    pub tvdb_id: Option<u32>,
    pub tmdb_id: Option<u32>,
    pub episode_offset: Option<i32>,
}

impl Store {
    pub fn dataset_state(&self, name: &str) -> Result<Option<DatasetState>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT name, etag, fetched_at, item_count, last_error
                   FROM dataset_state WHERE name = ?1",
            )?;
            let mut rows = stmt.query_map([name], |r| {
                Ok(DatasetState {
                    name: r.get(0)?,
                    etag: r.get(1)?,
                    fetched_at: r.get(2)?,
                    item_count: r.get(3)?,
                    last_error: r.get(4)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
    }

    /// Record that a `304` confirmed our copy is current.
    ///
    /// Updates `fetched_at` without touching the data, so the cadence timer restarts and
    /// we do not re-check on every launch.
    pub fn touch_dataset(&self, name: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE dataset_state SET fetched_at = ?1, last_error = NULL WHERE name = ?2",
                rusqlite::params![now, name],
            )?;
            Ok(())
        })
    }

    /// Record a failure without discarding the existing data.
    pub fn record_dataset_error(&self, name: &str, error: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO dataset_state (name, url, last_error, fetched_at)
                 VALUES (?1, '', ?2, NULL)
                 ON CONFLICT(name) DO UPDATE SET last_error = excluded.last_error",
                rusqlite::params![name, error],
            )?;
            let _ = now;
            Ok(())
        })
    }

    /// Write a parsed dataset into the mapping table.
    ///
    /// Merge semantics: a lower `priority` wins outright for fields it supplies, but a
    /// higher-priority source still **fills gaps** the primary left empty. That is exactly
    /// what the two corpora need — ThaUnknown has more AniList ids, Fribb has the
    /// `episode_offset` values, and neither should erase the other's contribution.
    pub fn materialise_mapping(
        &self,
        source: &str,
        priority: u8,
        entries: &[MappingInput],
        etag: Option<&str>,
        now: i64,
    ) -> Result<usize> {
        self.with_tx(|tx| {
            let mut written = 0usize;
            {
                let mut upsert = tx.prepare(
                    "INSERT INTO mapping
                        (anilist_id, mal_id, kitsu_id, anidb_id, tvdb_id, tmdb_id,
                         episode_offset, source)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(anilist_id) DO UPDATE SET
                        mal_id   = COALESCE(?2, mapping.mal_id),
                        kitsu_id = COALESCE(?3, mapping.kitsu_id),
                        anidb_id = COALESCE(?4, mapping.anidb_id),
                        tvdb_id  = COALESCE(?5, mapping.tvdb_id),
                        tmdb_id  = COALESCE(?6, mapping.tmdb_id),
                        episode_offset = COALESCE(?7, mapping.episode_offset),
                        source = CASE WHEN ?9 = 0 THEN ?8 ELSE mapping.source END",
                )?;

                for e in entries {
                    upsert.execute(rusqlite::params![
                        e.anilist_id,
                        e.mal_id,
                        e.kitsu_id,
                        e.anidb_id,
                        e.tvdb_id,
                        e.tmdb_id,
                        e.episode_offset,
                        source,
                        priority,
                    ])?;
                    written += 1;
                }
            }

            tx.execute(
                "INSERT INTO dataset_state (name, url, etag, fetched_at, item_count, last_error)
                 VALUES (?1, '', ?2, ?3, ?4, NULL)
                 ON CONFLICT(name) DO UPDATE SET
                    etag       = excluded.etag,
                    fetched_at = excluded.fetched_at,
                    item_count = excluded.item_count,
                    last_error = NULL",
                rusqlite::params![source, etag, now, written as u32],
            )?;

            Ok(written)
        })
    }

    /// Full mapping row for a title.
    pub fn mapping_for(&self, anilist_id: AnilistId) -> Result<Option<Mapping>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT anilist_id, mal_id, kitsu_id, anidb_id, tvdb_id, tmdb_id, episode_offset
                   FROM mapping WHERE anilist_id = ?1",
            )?;
            let mut rows = stmt.query_map([anilist_id.get()], |r| {
                Ok(Mapping {
                    anilist_id: r.get(0)?,
                    mal_id: r.get(1)?,
                    kitsu_id: r.get(2)?,
                    anidb_id: r.get(3)?,
                    tvdb_id: r.get(4)?,
                    tmdb_id: r.get(5)?,
                    episode_offset: r.get(6)?,
                })
            })?;
            Ok(rows.next().transpose()?)
        })
    }

    /// Reverse lookup, for services that speak MAL ids.
    pub fn anilist_id_for_mal(&self, mal_id: u32) -> Result<Option<AnilistId>> {
        self.with_conn(|c| {
            let found: Option<u32> = c
                .query_row(
                    "SELECT anilist_id FROM mapping WHERE mal_id = ?1 LIMIT 1",
                    [mal_id],
                    |r| r.get(0),
                )
                .ok();
            Ok(found.map(AnilistId::new))
        })
    }

    /// Reverse lookup by TVDB id, for Trakt.
    ///
    /// `ORDER BY anilist_id` rather than a bare `LIMIT 1`, because a TVDB series maps to *several*
    /// AniList entries — one per season, since AniList splits cours and TVDB does not. Ordering
    /// makes the answer stable across runs instead of whatever SQLite happens to visit first, and
    /// the lowest id is the first season, which is the least surprising choice.
    pub fn anilist_id_for_tvdb(&self, tvdb_id: u32) -> Result<Option<AnilistId>> {
        self.with_conn(|c| {
            let found: Option<u32> = c
                .query_row(
                    "SELECT anilist_id FROM mapping WHERE tvdb_id = ?1
                      ORDER BY anilist_id LIMIT 1",
                    [tvdb_id],
                    |r| r.get(0),
                )
                .ok();
            Ok(found.map(AnilistId::new))
        })
    }

    /// How many AniList entries share this TVDB id.
    ///
    /// More than one means the TVDB series covers several AniList entries — TVDB keeps sequels in
    /// one series where AniList splits them — so an episode number cannot be placed in a season
    /// without an offset. Trakt is the only consumer, and it refuses rather than guessing.
    pub fn anilist_entries_for_tvdb(&self, tvdb_id: u32) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM mapping WHERE tvdb_id = ?1",
                [tvdb_id],
                |r| r.get(0),
            )?)
        })
    }

    pub fn mapping_count(&self) -> Result<u32> {
        self.with_conn(|c| Ok(c.query_row("SELECT COUNT(*) FROM mapping", [], |r| r.get(0))?))
    }

    /// How many titles carry an episode offset.
    ///
    /// Only one corpus supplies these, so a non-zero count is direct evidence that the
    /// merge is a union rather than one source overwriting the other.
    pub fn mappings_with_offset_count(&self) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM mapping WHERE episode_offset IS NOT NULL",
                [],
                |r| r.get(0),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: u32 = 154_587;

    fn entry(anilist_id: u32) -> MappingInput {
        MappingInput { anilist_id, ..Default::default() }
    }

    #[test]
    fn materialising_records_entries_and_the_etag() {
        let store = Store::open_in_memory().unwrap();
        let entries = vec![MappingInput { mal_id: Some(52_991), ..entry(FRIEREN) }];
        let n = store
            .materialise_mapping("thaunknown", 0, &entries, Some("\"v1\""), 1_000)
            .unwrap();
        assert_eq!(n, 1);

        let state = store.dataset_state("thaunknown").unwrap().unwrap();
        assert_eq!(state.etag.as_deref(), Some("\"v1\""));
        assert_eq!(state.fetched_at, Some(1_000));
        assert_eq!(state.item_count, Some(1));
        assert!(state.last_error.is_none());
    }

    #[test]
    fn a_secondary_source_fills_gaps_without_erasing_the_primary() {
        // The whole reason both corpora are fetched: ThaUnknown has more AniList ids,
        // Fribb has the episode offsets, and each must keep the other's contribution.
        let store = Store::open_in_memory().unwrap();

        store
            .materialise_mapping(
                "thaunknown",
                0,
                &[MappingInput {
                    mal_id: Some(52_991),
                    kitsu_id: Some(46_474),
                    ..entry(FRIEREN)
                }],
                None,
                1_000,
            )
            .unwrap();

        store
            .materialise_mapping(
                "fribb",
                1,
                &[MappingInput {
                    mal_id: Some(52_991),
                    tvdb_id: Some(424_536),
                    episode_offset: Some(2),
                    ..entry(FRIEREN)
                }],
                None,
                2_000,
            )
            .unwrap();

        let m = store.mapping_for(AnilistId::new(FRIEREN)).unwrap().unwrap();
        assert_eq!(m.kitsu_id, Some(46_474), "primary's field survived");
        assert_eq!(m.tvdb_id, Some(424_536), "secondary filled a gap");
        assert_eq!(m.episode_offset, Some(2), "only Fribb has this");
        assert_eq!(store.mapping_count().unwrap(), 1, "merged, not duplicated");
    }

    #[test]
    fn a_secondary_null_does_not_wipe_an_existing_value() {
        // COALESCE semantics matter here: a source that simply lacks a field must not be
        // able to delete it.
        let store = Store::open_in_memory().unwrap();
        store
            .materialise_mapping(
                "a",
                0,
                &[MappingInput { mal_id: Some(1), ..entry(7) }],
                None,
                0,
            )
            .unwrap();
        store.materialise_mapping("b", 1, &[entry(7)], None, 0).unwrap();
        assert_eq!(store.mapping_for(AnilistId::new(7)).unwrap().unwrap().mal_id, Some(1));
    }

    #[test]
    fn re_materialising_the_same_source_updates_in_place() {
        let store = Store::open_in_memory().unwrap();
        for round in 0..3 {
            store.materialise_mapping("thaunknown", 0, &[entry(FRIEREN)], None, round).unwrap();
        }
        assert_eq!(store.mapping_count().unwrap(), 1);
    }

    #[test]
    fn a_304_touch_restarts_the_cadence_without_touching_data() {
        let store = Store::open_in_memory().unwrap();
        store
            .materialise_mapping("thaunknown", 0, &[entry(FRIEREN)], Some("\"v1\""), 1_000)
            .unwrap();
        store.touch_dataset("thaunknown", 5_000).unwrap();

        let state = store.dataset_state("thaunknown").unwrap().unwrap();
        assert_eq!(state.fetched_at, Some(5_000));
        assert_eq!(state.etag.as_deref(), Some("\"v1\""), "etag must be retained");
        assert_eq!(store.mapping_count().unwrap(), 1, "data untouched");
    }

    #[test]
    fn a_failed_refresh_leaves_the_previous_data_usable() {
        // Degrading to stale mappings is always better than degrading to none.
        let store = Store::open_in_memory().unwrap();
        store
            .materialise_mapping(
                "thaunknown",
                0,
                &[MappingInput { mal_id: Some(52_991), ..entry(FRIEREN) }],
                Some("\"v1\""),
                1_000,
            )
            .unwrap();

        store.record_dataset_error("thaunknown", "connection reset", 2_000).unwrap();

        let state = store.dataset_state("thaunknown").unwrap().unwrap();
        assert_eq!(state.last_error.as_deref(), Some("connection reset"));
        assert_eq!(state.etag.as_deref(), Some("\"v1\""));
        assert_eq!(
            store.mapping_for(AnilistId::new(FRIEREN)).unwrap().unwrap().mal_id,
            Some(52_991),
            "mappings must survive a failed refresh"
        );
    }

    #[test]
    fn reverse_lookup_by_mal_id_works() {
        let store = Store::open_in_memory().unwrap();
        store
            .materialise_mapping(
                "t",
                0,
                &[MappingInput { mal_id: Some(52_991), ..entry(FRIEREN) }],
                None,
                0,
            )
            .unwrap();
        assert_eq!(store.anilist_id_for_mal(52_991).unwrap(), Some(AnilistId::new(FRIEREN)));
        assert_eq!(store.anilist_id_for_mal(1).unwrap(), None);
    }

    #[test]
    fn an_unknown_dataset_and_title_report_absence_rather_than_erroring() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.dataset_state("nope").unwrap().is_none());
        assert!(store.mapping_for(AnilistId::new(1)).unwrap().is_none());
        assert_eq!(store.mapping_count().unwrap(), 0);
    }
}
