//! Refreshable ID-mapping datasets.
//!
//! The mapping corpora are third-party files that update on their own schedule —
//! ThaUnknown's daily at ~02:00 UTC, Fribb's weekly on Tuesdays. Vendoring a snapshot
//! would rot within days, which is precisely the failure the mapping layer exists to
//! prevent, so nothing is baked into the binary.
//!
//! Refresh economics, measured: 7.67 MB raw, **1.24 MB gzipped**, and a `304` with an
//! empty body when nothing changed. `raw.githubusercontent.com` serves a strong `ETag` and
//! honours `If-None-Match`, so the steady-state check costs one round trip and no bytes.
//!
//! Two design points worth stating:
//!
//! - Parsing happens **once per refresh**, into indexed SQLite. Re-reading ~15 MB of JSON
//!   at every launch would make startup scale with dataset size for no benefit.
//! - Refresh never blocks startup or the UI. A stale mapping is always better than a
//!   stalled interface, and a failed refresh leaves the previous data intact.

use anistream_core::ids::AnilistId;
use anistream_net::{Conditional, ConditionalResponse, HttpClient};
use anistream_store::Store;
use serde::Deserialize;

/// How often a dataset is worth re-checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    Daily,
    Weekly,
}

impl Cadence {
    pub const fn seconds(self) -> i64 {
        match self {
            Self::Daily => 24 * 3_600,
            Self::Weekly => 7 * 24 * 3_600,
        }
    }
}

/// A dataset anistream keeps current.
///
/// Declared as data rather than hardcoded so adding or replacing a source is a config edit,
/// matching how providers and trackers work.
#[derive(Debug, Clone)]
pub struct DatasetSpec {
    pub name: &'static str,
    pub url: &'static str,
    pub cadence: Cadence,
    /// Lower wins when two datasets disagree about the same field.
    pub priority: u8,
}

/// The two mapping corpora, merged.
///
/// Same 42,868-entry corpus, but they differ where it matters: ThaUnknown carries 22,374
/// AniList ids to Fribb's 20,687 — and AniList is our primary key — while only Fribb has
/// `episode_offset`, needed for split-cour and absolute↔season numbering. Neither alone is
/// sufficient, so both are fetched and layered.
pub const MAPPING_DATASETS: &[DatasetSpec] = &[
    DatasetSpec {
        name: "thaunknown",
        url: "https://raw.githubusercontent.com/ThaUnknown/anime-lists-ts/refs/heads/main/data/anime-list.json",
        cadence: Cadence::Daily,
        priority: 0,
    },
    DatasetSpec {
        name: "fribb",
        url: "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-full.json",
        cadence: Cadence::Weekly,
        priority: 1,
    },
];

/// One row of a mapping corpus.
///
/// The two sources type several fields differently — `imdb_id` is a list in one and a
/// comma-joined string in the other, `themoviedb_id` an object in one and an integer in the
/// other. Only the fields anistream actually uses are modelled, which sidesteps most of the
/// disagreement; `tmdb` still needs a custom shape.
#[derive(Debug, Clone, Deserialize)]
pub struct MappingEntry {
    pub anilist_id: Option<u32>,
    pub mal_id: Option<u32>,
    pub kitsu_id: Option<u32>,
    pub anidb_id: Option<u32>,
    pub tvdb_id: Option<u32>,
    #[serde(default, deserialize_with = "flexible_tmdb")]
    pub themoviedb_id: Option<u32>,
    #[serde(default)]
    pub episode_offset: Option<serde_json::Value>,
}

/// Accept `themoviedb_id` as either an integer or `{"tv": 123}`.
fn flexible_tmdb<'de, D>(de: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(de)?;
    Ok(match value {
        Some(serde_json::Value::Number(n)) => n.as_u64().map(|v| v as u32),
        Some(serde_json::Value::Object(map)) => map
            .get("tv")
            .or_else(|| map.get("movie"))
            .and_then(serde_json::Value::as_u64)
            .map(|v| v as u32),
        _ => None,
    })
}

/// What a refresh attempt did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// Cadence has not elapsed; nothing was requested.
    NotDue,
    /// Server confirmed our copy is current. No body transferred.
    Unchanged,
    /// New data materialised into SQLite.
    Updated { entries: usize },
    /// Refresh failed. The previous data is untouched and still usable.
    Failed { reason: String },
}

impl RefreshOutcome {
    pub fn is_failure(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// Whether a dataset is due for a re-check.
pub fn is_due(last_fetched: Option<i64>, cadence: Cadence, now: i64) -> bool {
    match last_fetched {
        None => true,
        Some(at) => now.saturating_sub(at) >= cadence.seconds(),
    }
}

/// Parse a mapping corpus, keeping only entries usable as mappings.
///
/// Roughly half the corpus has no AniList id — those rows describe works AniList does not
/// index, and since AniList is our primary key they cannot be joined to anything.
pub fn parse_mapping(body: &[u8]) -> Result<Vec<MappingEntry>, String> {
    let entries: Vec<MappingEntry> =
        serde_json::from_slice(body).map_err(|e| format!("malformed dataset JSON: {e}"))?;
    Ok(entries.into_iter().filter(|e| e.anilist_id.is_some()).collect())
}

/// Extract the tvdb episode offset, which is the only one anistream consumes.
pub fn tvdb_offset(entry: &MappingEntry) -> Option<i32> {
    entry.episode_offset.as_ref()?.get("tvdb")?.as_i64().map(|v| v as i32)
}

/// Refresh one dataset if due, and materialise it.
pub async fn refresh(
    store: &Store,
    http: &HttpClient,
    spec: &DatasetSpec,
    now: i64,
    force: bool,
) -> RefreshOutcome {
    let state = match store.dataset_state(spec.name) {
        Ok(s) => s,
        Err(e) => return RefreshOutcome::Failed { reason: e.to_string() },
    };

    if !force && !is_due(state.as_ref().and_then(|s| s.fetched_at), spec.cadence, now) {
        return RefreshOutcome::NotDue;
    }

    let etag = state.and_then(|s| s.etag);
    let request = Conditional::new(spec.url).with_etag(etag.clone());

    let response = match request.get(http.plain()).await {
        Ok(r) => r,
        Err(e) => {
            // A failed refresh must leave the previous dataset in place — degrading to
            // stale data is always better than degrading to none.
            let reason = e.to_string();
            let _ = store.record_dataset_error(spec.name, &reason, now);
            return RefreshOutcome::Failed { reason };
        }
    };

    let (new_etag, body) = match response {
        ConditionalResponse::NotModified => {
            let _ = store.touch_dataset(spec.name, now);
            return RefreshOutcome::Unchanged;
        }
        ConditionalResponse::Fetched { etag, body } => (etag, body),
    };

    let entries = match parse_mapping(&body) {
        Ok(e) => e,
        Err(reason) => {
            let _ = store.record_dataset_error(spec.name, &reason, now);
            return RefreshOutcome::Failed { reason };
        }
    };

    let inputs: Vec<anistream_store::MappingInput> = entries.iter().map(to_input).collect();
    match store.materialise_mapping(spec.name, spec.priority, &inputs, new_etag.as_deref(), now)
    {
        Ok(count) => RefreshOutcome::Updated { entries: count },
        Err(e) => {
            let reason = e.to_string();
            let _ = store.record_dataset_error(spec.name, &reason, now);
            RefreshOutcome::Failed { reason }
        }
    }
}

/// Refresh every mapping dataset.
///
/// Returns each outcome so the caller can report partial success: one source failing is
/// normal and must not be reported as a total failure.
pub async fn refresh_all(
    store: &Store,
    http: &HttpClient,
    now: i64,
    force: bool,
) -> Vec<(&'static str, RefreshOutcome)> {
    let mut outcomes = Vec::with_capacity(MAPPING_DATASETS.len());
    for spec in MAPPING_DATASETS {
        let outcome = refresh(store, http, spec, now, force).await;
        tracing::info!(dataset = spec.name, ?outcome, "dataset refresh");
        outcomes.push((spec.name, outcome));
    }

    // Cached automatic resolutions may now be improvable, but user overrides are never
    // revised — that separation is what makes fixing a bad match worth doing.
    if outcomes.iter().any(|(_, o)| matches!(o, RefreshOutcome::Updated { .. }))
        && let Err(e) = store.clear_cached_resolutions()
    {
        tracing::warn!(error = %e, "could not clear cached resolutions after refresh");
    }
    outcomes
}

/// Convert a parsed corpus row into the store's input shape.
///
/// The conversion lives here rather than in the store so that `anistream-store` stays
/// unaware of dataset formats — the dependency runs one way only.
fn to_input(entry: &MappingEntry) -> anistream_store::MappingInput {
    anistream_store::MappingInput {
        // Safe: `parse_mapping` drops rows without one.
        anilist_id: entry.anilist_id.unwrap_or_default(),
        mal_id: entry.mal_id,
        kitsu_id: entry.kitsu_id,
        anidb_id: entry.anidb_id,
        tvdb_id: entry.tvdb_id,
        tmdb_id: entry.themoviedb_id,
        episode_offset: tvdb_offset(entry),
    }
}

/// Look up the MAL id for a title — the one mapping aniskip needs.
pub fn mal_id_for(store: &Store, anilist_id: AnilistId) -> Option<u32> {
    store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.mal_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cadences_match_the_measured_upstream_schedules() {
        assert_eq!(Cadence::Daily.seconds(), 86_400);
        assert_eq!(Cadence::Weekly.seconds(), 604_800);
        let thaunknown = &MAPPING_DATASETS[0];
        let fribb = &MAPPING_DATASETS[1];
        assert_eq!(thaunknown.cadence, Cadence::Daily);
        assert_eq!(fribb.cadence, Cadence::Weekly);
        assert!(
            thaunknown.priority < fribb.priority,
            "ThaUnknown wins ties — it carries more AniList ids, and AniList is our key"
        );
    }

    #[test]
    fn a_never_fetched_dataset_is_always_due() {
        assert!(is_due(None, Cadence::Daily, 0));
    }

    #[test]
    fn due_only_after_the_cadence_elapses() {
        let fetched = 1_000_000;
        assert!(!is_due(Some(fetched), Cadence::Daily, fetched + 3_600));
        assert!(is_due(Some(fetched), Cadence::Daily, fetched + 86_400));
        assert!(!is_due(Some(fetched), Cadence::Weekly, fetched + 86_400));
        assert!(is_due(Some(fetched), Cadence::Weekly, fetched + 604_800));
    }

    #[test]
    fn both_dataset_typings_parse() {
        // The two sources genuinely disagree about field types; parsing has to absorb it
        // rather than fail on one of them.
        let thaunknown = br#"[
            {"anilist_id": 154587, "mal_id": 52991, "imdb_id": "tt22248376", "themoviedb_id": 209867}
        ]"#;
        let fribb = br#"[
            {"anilist_id": 154587, "mal_id": 52991, "imdb_id": ["tt22248376"],
             "themoviedb_id": {"tv": 209867}, "episode_offset": {"tvdb": 2}}
        ]"#;

        let a = parse_mapping(thaunknown).unwrap();
        let b = parse_mapping(fribb).unwrap();
        assert_eq!(a[0].themoviedb_id, Some(209_867));
        assert_eq!(b[0].themoviedb_id, Some(209_867), "object form must also work");
        assert_eq!(a[0].mal_id, Some(52_991));
    }

    #[test]
    fn entries_without_an_anilist_id_are_dropped() {
        // Roughly half the corpus is unusable to us: AniList is the primary key, so a row
        // without one cannot be joined to anything.
        let body = br#"[
            {"anilist_id": 1, "mal_id": 1},
            {"mal_id": 999},
            {"anilist_id": 2}
        ]"#;
        let entries = parse_mapping(body).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.anilist_id.is_some()));
    }

    #[test]
    fn the_tvdb_offset_is_extracted_and_absent_offsets_are_none() {
        let with = parse_mapping(
            br#"[{"anilist_id": 821, "episode_offset": {"tvdb": 2, "tmdb": 2}}]"#,
        )
        .unwrap();
        assert_eq!(tvdb_offset(&with[0]), Some(2));

        let without = parse_mapping(br#"[{"anilist_id": 1}]"#).unwrap();
        assert_eq!(tvdb_offset(&without[0]), None);

        let other_only =
            parse_mapping(br#"[{"anilist_id": 1, "episode_offset": {"tmdb": 3}}]"#).unwrap();
        assert_eq!(tvdb_offset(&other_only[0]), None);
    }

    #[test]
    fn malformed_json_is_an_error_rather_than_a_panic() {
        let err = parse_mapping(b"{not json").unwrap_err();
        assert!(err.contains("malformed"), "got: {err}");
    }

    #[test]
    fn an_empty_dataset_parses_to_nothing() {
        assert!(parse_mapping(b"[]").unwrap().is_empty());
    }

    #[test]
    fn outcomes_distinguish_failure_from_no_change() {
        // "Unchanged" and "NotDue" are successes; conflating them with failure would make
        // the normal steady state look broken.
        assert!(!RefreshOutcome::Unchanged.is_failure());
        assert!(!RefreshOutcome::NotDue.is_failure());
        assert!(!RefreshOutcome::Updated { entries: 10 }.is_failure());
        assert!(RefreshOutcome::Failed { reason: "boom".into() }.is_failure());
    }
}
