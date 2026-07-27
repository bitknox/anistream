//! Persistence for the resolution ladder.
//!
//! The ladder runs override → cache → dataset id → provider search → disambiguation →
//! manual query. This module owns the first two rungs and the separation between them,
//! which is the part that has to be right: **a dataset refresh clears cached automatic
//! resolutions but must never touch a correction the user made by hand.** With sources
//! this noisy, mismatches are certain, and a fix that silently evaporates on the next
//! daily refresh would be worse than no fix at all.

use anistream_core::ids::{AnilistId, ProviderKey};

use crate::{Result, Store};

/// Which rung of the ladder produced a resolution.
///
/// Stored alongside the result so a low-confidence match can be re-examined later rather
/// than being indistinguishable from a certain one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum ResolutionRung {
    /// User correction. Always wins, never expires.
    Override = 1,
    /// Previously resolved automatically.
    Cache = 2,
    /// Looked up by external id in the merged dataset.
    DatasetId = 3,
    /// Matched by searching the provider and scoring candidates.
    ProviderSearch = 4,
    /// User picked from ambiguous candidates.
    Disambiguated = 5,
    /// User supplied a query by hand.
    ManualQuery = 6,
}

impl ResolutionRung {
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Override,
            2 => Self::Cache,
            3 => Self::DatasetId,
            5 => Self::Disambiguated,
            6 => Self::ManualQuery,
            _ => Self::ProviderSearch,
        }
    }

    /// Whether a result from this rung should be treated as authoritative — i.e. written
    /// as an override so the title is never disambiguated twice.
    pub const fn is_user_confirmed(self) -> bool {
        matches!(self, Self::Override | Self::Disambiguated | Self::ManualQuery)
    }
}

/// A provider key resolved for a title.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedMapping {
    pub provider_key: ProviderKey,
    pub confidence: f64,
    pub rung: ResolutionRung,
}

impl Store {
    /// Record a user correction. Idempotent, and overwrites any prior override.
    pub fn set_override(
        &self,
        anilist_id: AnilistId,
        provider_id: &str,
        provider_key: &ProviderKey,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO mapping_override (anilist_id, provider_id, provider_key, created_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id, provider_id) DO UPDATE SET
                    provider_key = excluded.provider_key,
                    created_at   = excluded.created_at",
                rusqlite::params![
                    anilist_id.get(),
                    provider_id,
                    provider_key.as_str(),
                    at
                ],
            )?;
            Ok(())
        })
    }

    pub fn clear_override(&self, anilist_id: AnilistId, provider_id: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM mapping_override WHERE anilist_id = ?1 AND provider_id = ?2",
                rusqlite::params![anilist_id.get(), provider_id],
            )?;
            Ok(())
        })
    }

    /// Cache an automatic resolution.
    pub fn cache_resolution(
        &self,
        anilist_id: AnilistId,
        provider_id: &str,
        provider_key: &ProviderKey,
        confidence: f64,
        rung: ResolutionRung,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO mapping_resolution
                    (anilist_id, provider_id, provider_key, confidence, rung, resolved_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(anilist_id, provider_id) DO UPDATE SET
                    provider_key = excluded.provider_key,
                    confidence   = excluded.confidence,
                    rung         = excluded.rung,
                    resolved_at  = excluded.resolved_at",
                rusqlite::params![
                    anilist_id.get(),
                    provider_id,
                    provider_key.as_str(),
                    confidence,
                    rung as u8,
                    at
                ],
            )?;
            Ok(())
        })
    }

    /// Look up rungs 1 and 2, in that order.
    ///
    /// An override always shadows a cached resolution, even a more recent one.
    pub fn lookup_mapping(
        &self,
        anilist_id: AnilistId,
        provider_id: &str,
    ) -> Result<Option<ResolvedMapping>> {
        self.with_conn(|c| {
            let over: Option<String> = c
                .query_row(
                    "SELECT provider_key FROM mapping_override
                      WHERE anilist_id = ?1 AND provider_id = ?2",
                    rusqlite::params![anilist_id.get(), provider_id],
                    |r| r.get(0),
                )
                .ok();
            if let Some(key) = over {
                return Ok(Some(ResolvedMapping {
                    provider_key: ProviderKey::new(key),
                    confidence: 1.0,
                    rung: ResolutionRung::Override,
                }));
            }

            let cached: Option<(String, f64, u8)> = c
                .query_row(
                    "SELECT provider_key, confidence, rung FROM mapping_resolution
                      WHERE anilist_id = ?1 AND provider_id = ?2",
                    rusqlite::params![anilist_id.get(), provider_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .ok();
            Ok(cached.map(|(key, confidence, rung)| ResolvedMapping {
                provider_key: ProviderKey::new(key),
                confidence,
                rung: ResolutionRung::from_u8(rung),
            }))
        })
    }

    /// Drop cached automatic resolutions, leaving overrides intact.
    ///
    /// Called after a dataset refresh: new mapping data may produce better matches, but
    /// the user's own corrections are not up for revision.
    pub fn clear_cached_resolutions(&self) -> Result<usize> {
        self.with_conn(|c| Ok(c.execute("DELETE FROM mapping_resolution", [])?))
    }

    /// Resolutions below a confidence floor, for a "check these matches" review.
    pub fn low_confidence_resolutions(
        &self,
        floor: f64,
    ) -> Result<Vec<(AnilistId, String, f64)>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT anilist_id, provider_id, confidence FROM mapping_resolution
                  WHERE confidence < ?1 ORDER BY confidence ASC",
            )?;
            let rows = stmt.query_map([floor], |r| {
                Ok((
                    AnilistId::new(r.get::<_, u32>(0)?),
                    r.get::<_, String>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn overrides_shadow_cached_resolutions() {
        let store = Store::open_in_memory().unwrap();
        store
            .cache_resolution(
                FRIEREN,
                "webprov",
                &ProviderKey::new("wrong-match"),
                0.62,
                ResolutionRung::ProviderSearch,
                1_000,
            )
            .unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("correct"), 2_000).unwrap();

        let got = store.lookup_mapping(FRIEREN, "webprov").unwrap().unwrap();
        assert_eq!(got.provider_key.as_str(), "correct");
        assert_eq!(got.rung, ResolutionRung::Override);
        assert_eq!(got.confidence, 1.0);
    }

    #[test]
    fn a_dataset_refresh_never_discards_a_user_correction() {
        // The property that makes fixing a bad match worth doing: it has to stick through
        // the next daily dataset update.
        let store = Store::open_in_memory().unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("correct"), 1_000).unwrap();
        store
            .cache_resolution(
                FRIEREN,
                "torrent",
                &ProviderKey::new("guessed"),
                0.7,
                ResolutionRung::ProviderSearch,
                1_000,
            )
            .unwrap();

        let cleared = store.clear_cached_resolutions().unwrap();
        assert_eq!(cleared, 1, "only the automatic resolution should go");

        assert_eq!(
            store.lookup_mapping(FRIEREN, "webprov").unwrap().unwrap().provider_key.as_str(),
            "correct"
        );
        assert!(store.lookup_mapping(FRIEREN, "torrent").unwrap().is_none());
    }

    #[test]
    fn overrides_are_per_provider() {
        // Different providers key titles completely differently, so a
        // correction for one says nothing about the other.
        let store = Store::open_in_memory().unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("aa-key"), 0).unwrap();
        assert!(store.lookup_mapping(FRIEREN, "torrent").unwrap().is_none());
    }

    #[test]
    fn setting_an_override_twice_updates_rather_than_duplicating() {
        let store = Store::open_in_memory().unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("first"), 0).unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("second"), 1).unwrap();
        assert_eq!(
            store.lookup_mapping(FRIEREN, "webprov").unwrap().unwrap().provider_key.as_str(),
            "second"
        );
    }

    #[test]
    fn clearing_an_override_falls_back_to_the_cached_resolution() {
        let store = Store::open_in_memory().unwrap();
        store
            .cache_resolution(
                FRIEREN,
                "webprov",
                &ProviderKey::new("auto"),
                0.9,
                ResolutionRung::DatasetId,
                0,
            )
            .unwrap();
        store.set_override(FRIEREN, "webprov", &ProviderKey::new("manual"), 1).unwrap();
        store.clear_override(FRIEREN, "webprov").unwrap();

        let got = store.lookup_mapping(FRIEREN, "webprov").unwrap().unwrap();
        assert_eq!(got.provider_key.as_str(), "auto");
        assert_eq!(got.rung, ResolutionRung::DatasetId);
    }

    #[test]
    fn user_confirmed_rungs_are_identified_correctly() {
        assert!(ResolutionRung::Disambiguated.is_user_confirmed());
        assert!(ResolutionRung::ManualQuery.is_user_confirmed());
        assert!(ResolutionRung::Override.is_user_confirmed());
        // Automatic rungs are not authoritative and stay revisable.
        assert!(!ResolutionRung::ProviderSearch.is_user_confirmed());
        assert!(!ResolutionRung::DatasetId.is_user_confirmed());
        assert!(!ResolutionRung::Cache.is_user_confirmed());
    }

    #[test]
    fn rung_survives_the_round_trip_through_sqlite() {
        let store = Store::open_in_memory().unwrap();
        for rung in [
            ResolutionRung::Cache,
            ResolutionRung::DatasetId,
            ResolutionRung::ProviderSearch,
            ResolutionRung::Disambiguated,
            ResolutionRung::ManualQuery,
        ] {
            store.cache_resolution(FRIEREN, "p", &ProviderKey::new("k"), 0.5, rung, 0).unwrap();
            assert_eq!(store.lookup_mapping(FRIEREN, "p").unwrap().unwrap().rung, rung);
        }
    }

    #[test]
    fn low_confidence_resolutions_are_reviewable() {
        let store = Store::open_in_memory().unwrap();
        store
            .cache_resolution(
                FRIEREN,
                "a",
                &ProviderKey::new("k"),
                0.55,
                ResolutionRung::ProviderSearch,
                0,
            )
            .unwrap();
        store
            .cache_resolution(
                FRIEREN,
                "b",
                &ProviderKey::new("k"),
                0.95,
                ResolutionRung::DatasetId,
                0,
            )
            .unwrap();

        let shaky = store.low_confidence_resolutions(0.8).unwrap();
        assert_eq!(shaky.len(), 1);
        assert_eq!(shaky[0].1, "a");
    }
}
