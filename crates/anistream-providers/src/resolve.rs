//! The resolution ladder: turning an AniList title into a provider's own key.
//!
//! Six rungs, tried in order:
//!
//! | # | Rung | When it applies |
//! |---|---|---|
//! | 1 | Manual override | The user has corrected this pairing before. Always wins. |
//! | 2 | Cached resolution | Resolved automatically before, above the confidence floor. |
//! | 3 | Dataset id | Only for id-keyed consumers — aniskip's `mal_id`, or a tvdb source. |
//! | 4 | Provider search | **The primary path**, not a fallback. |
//! | 5 | Disambiguation | Candidates exist but none is confident. Ask. |
//! | 6 | Manual query | Nothing found. Let the user say what to look for. |
//!
//! Rung 4 being *primary* is the part that is easy to get wrong. The mapping datasets
//! translate between catalogue ids — anilist, mal, kitsu, tvdb — but a web or torrent
//! source keys on its own opaque id or on title text, which appears in no dataset. For
//! those, there is nothing to look up and searching is the only road.
//!
//! Rungs 5 and 6 are what make this robust rather than merely clever: below the confidence
//! floor the ladder *stops and asks* instead of guessing, and a user's answer is written
//! back as a rung-1 override so the same title is never disambiguated twice.

use anistream_core::{
    ids::{AnilistId, ProviderKey},
    media::{SearchHit, Translation},
};
use anistream_meta::title::{CONFIDENCE_FLOOR, MatchTarget, rank};
use anistream_store::{ResolutionRung, Store};

use crate::registry::ProviderRegistry;

/// A candidate offered to the user when the ladder cannot decide.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub hit: SearchHit,
    pub score: f64,
    /// Why this was ruled out, when it was.
    pub rejected: Option<&'static str>,
}

/// Where the ladder stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    /// A provider key we are confident about.
    Resolved { provider_id: String, key: ProviderKey, confidence: f64, rung: ResolutionRung },
    /// Candidates exist but none was confident enough to use. Rung 5.
    Ambiguous { provider_id: String, candidates: Vec<Candidate> },
    /// Nothing usable was found. Rung 6 — offer a manual query.
    NotFound { reason: String },
}

impl Resolution {
    pub fn key(&self) -> Option<&ProviderKey> {
        match self {
            Self::Resolved { key, .. } => Some(key),
            _ => None,
        }
    }

    pub fn needs_user_input(&self) -> bool {
        matches!(self, Self::Ambiguous { .. } | Self::NotFound { .. })
    }

    /// What the UI should say about how this was decided.
    pub fn explain(&self) -> String {
        match self {
            Self::Resolved { rung: ResolutionRung::Override, .. } => "your correction".into(),
            Self::Resolved { rung: ResolutionRung::Cache, confidence, .. } => {
                format!("remembered ({:.0}%)", confidence * 100.0)
            }
            Self::Resolved { rung: ResolutionRung::DatasetId, .. } => "id mapping".into(),
            Self::Resolved { confidence, .. } => {
                format!("title match ({:.0}%)", confidence * 100.0)
            }
            Self::Ambiguous { candidates, .. } => {
                format!("{} possible matches", candidates.len())
            }
            Self::NotFound { reason } => reason.clone(),
        }
    }
}

/// Resolve a title against the first usable provider.
///
/// `now` is passed in rather than read from the clock so the caller controls time and the
/// behaviour stays testable.
pub async fn resolve(
    store: &Store,
    registry: &ProviderRegistry,
    anilist_id: AnilistId,
    target: &MatchTarget,
    translation: Translation,
    now: i64,
) -> Resolution {
    let Some(provider_id) = registry.ids().first().cloned() else {
        return Resolution::NotFound { reason: "no providers configured".into() };
    };

    // Rungs 1 and 2: the store answers both, and an override always shadows a cache.
    if let Ok(Some(existing)) = store.lookup_mapping(anilist_id, &provider_id) {
        return Resolution::Resolved {
            provider_id,
            key: existing.provider_key,
            confidence: existing.confidence,
            rung: existing.rung,
        };
    }

    // Rung 4. (Rung 3, dataset id lookup, is handled by the callers that are actually
    // id-keyed — aniskip and the trackers — because no web or torrent source is.)
    let Some(query) = target.titles.first() else {
        return Resolution::NotFound { reason: "no title to search for".into() };
    };

    let attempt = registry.search(query, translation, now).await;
    let Some(hits) = attempt.value else {
        let reason = if attempt.failures.is_empty() {
            "no providers available".to_owned()
        } else {
            attempt.summary()
        };
        return Resolution::NotFound { reason };
    };
    let provider_id = attempt.provider.unwrap_or(provider_id);

    if hits.is_empty() {
        return Resolution::NotFound { reason: format!("{provider_id} found no matches") };
    }

    let ranked = rank(target, &hits);
    let candidates: Vec<Candidate> = ranked
        .iter()
        .map(|s| Candidate { hit: s.hit.clone(), score: s.score, rejected: s.rejected })
        .collect();

    let best = &ranked[0];
    if best.is_confident() {
        let key = best.hit.key.clone();
        // Cache it, but never as an override: an automatic match stays revisable, and a
        // dataset refresh is allowed to clear it.
        let _ = store.cache_resolution(
            anilist_id,
            &provider_id,
            &key,
            best.score,
            ResolutionRung::ProviderSearch,
            now,
        );
        return Resolution::Resolved {
            provider_id,
            key,
            confidence: best.score,
            rung: ResolutionRung::ProviderSearch,
        };
    }

    // Rung 5. Below the floor, stop and ask rather than guess — being wrong here means
    // silently playing the wrong show, which is worse than one extra keystroke.
    Resolution::Ambiguous { provider_id, candidates }
}

/// Record a user's choice from the disambiguation overlay.
///
/// Written as a rung-1 override, so the title is disambiguated at most once and the answer
/// survives every dataset refresh.
pub fn confirm(
    store: &Store,
    anilist_id: AnilistId,
    provider_id: &str,
    key: &ProviderKey,
    now: i64,
) -> Resolution {
    let _ = store.set_override(anilist_id, provider_id, key, now);
    Resolution::Resolved {
        provider_id: provider_id.to_owned(),
        key: key.clone(),
        confidence: 1.0,
        rung: ResolutionRung::Override,
    }
}

/// The confidence below which the ladder asks instead of deciding.
pub const fn confidence_floor() -> f64 {
    CONFIDENCE_FLOOR
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;
    use anistream_core::{error::ProviderError, media::MediaFormat};

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    fn target() -> MatchTarget {
        MatchTarget {
            titles: vec!["Sousou no Frieren".into(), "Frieren: Beyond Journey's End".into()],
            episode_count: Some(28),
            year: Some(2023),
            format: Some(MediaFormat::Tv),
        }
    }

    fn hit(key: &str, title: &str) -> SearchHit {
        SearchHit {
            episode_count: Some(28),
            year: Some(2023),
            format: Some(MediaFormat::Tv),
            ..SearchHit::new(ProviderKey::new(key), title)
        }
    }

    fn registry_with(hits: Vec<SearchHit>) -> ProviderRegistry {
        ProviderRegistry::new(vec![MockProvider::new("torrent").with_hits(hits).arc()])
    }

    #[tokio::test]
    async fn a_confident_title_match_resolves_and_is_cached() {
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![hit("t-1", "Sousou no Frieren")]);

        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        match &r {
            Resolution::Resolved { key, rung, .. } => {
                assert_eq!(key.as_str(), "t-1");
                assert_eq!(*rung, ResolutionRung::ProviderSearch);
            }
            other => panic!("expected a resolution, got {other:?}"),
        }

        // Cached as rung 2 for next time, but *not* as an override.
        let cached = store.lookup_mapping(FRIEREN, "torrent").unwrap().unwrap();
        assert_eq!(cached.provider_key.as_str(), "t-1");
        assert_ne!(cached.rung, ResolutionRung::Override, "must stay revisable");
    }

    #[tokio::test]
    async fn an_override_wins_without_touching_the_provider() {
        // Rung 1 must short-circuit: the user already told us the answer.
        let store = Store::open_in_memory().unwrap();
        store.set_override(FRIEREN, "torrent", &ProviderKey::new("corrected"), 0).unwrap();

        let provider = MockProvider::new("torrent").with_hits(vec![hit("wrong", "Frieren")]);
        let calls = provider.call_count();
        let registry = ProviderRegistry::new(vec![provider.arc()]);

        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        assert_eq!(r.key().unwrap().as_str(), "corrected");
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0, "no search needed");
        assert_eq!(r.explain(), "your correction");
    }

    #[tokio::test]
    async fn a_weak_match_asks_instead_of_guessing() {
        // Being wrong silently plays the wrong show; one extra keystroke does not.
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![SearchHit {
            episode_count: None,
            year: None,
            format: None,
            ..SearchHit::new(ProviderKey::new("maybe"), "Something Entirely Different")
        }]);

        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        assert!(r.needs_user_input());
        match r {
            Resolution::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 1),
            other => panic!("expected ambiguity, got {other:?}"),
        }
        // Nothing weak may be cached, or the guess would stick.
        assert!(store.lookup_mapping(FRIEREN, "torrent").unwrap().is_none());
    }

    #[tokio::test]
    async fn confirming_a_choice_writes_an_override_that_survives_a_refresh() {
        // This is what makes disambiguating worth doing exactly once.
        let store = Store::open_in_memory().unwrap();
        let r = confirm(&store, FRIEREN, "torrent", &ProviderKey::new("picked"), 0);
        assert_eq!(r.key().unwrap().as_str(), "picked");

        store.clear_cached_resolutions().unwrap();
        let after = store.lookup_mapping(FRIEREN, "torrent").unwrap().unwrap();
        assert_eq!(after.provider_key.as_str(), "picked");
        assert_eq!(after.rung, ResolutionRung::Override);
    }

    #[tokio::test]
    async fn a_previously_confirmed_title_is_never_asked_about_again() {
        let store = Store::open_in_memory().unwrap();
        confirm(&store, FRIEREN, "torrent", &ProviderKey::new("picked"), 0);

        let registry = registry_with(vec![hit("something-else", "Frieren")]);
        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 1).await;
        assert!(!r.needs_user_input());
        assert_eq!(r.key().unwrap().as_str(), "picked");
    }

    #[tokio::test]
    async fn an_empty_result_set_offers_a_manual_query() {
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![]);
        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        match &r {
            Resolution::NotFound { reason } => assert!(reason.contains("no matches")),
            other => panic!("expected not-found, got {other:?}"),
        }
        assert!(r.needs_user_input(), "the user must get a way forward");
    }

    #[tokio::test]
    async fn a_provider_failure_is_reported_rather_than_looking_like_no_match() {
        // "Cloudflare blocked us" and "this show does not exist" must not look the same.
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("torrent")
                .failing(ProviderError::Blocked("cloudflare".into()))
                .arc(),
        ]);
        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        match r {
            Resolution::NotFound { reason } => {
                assert!(reason.contains("blocked"), "got {reason}")
            }
            other => panic!("expected a reported failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn no_providers_configured_is_stated_plainly() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![]);
        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        assert!(matches!(r, Resolution::NotFound { .. }));
        assert!(r.explain().contains("no providers"));
    }

    #[tokio::test]
    async fn the_ova_trap_is_avoided() {
        // The classic silent failure: a search for the TV series matches its OVA, which
        // often scores higher on title alone.
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![
            SearchHit {
                format: Some(MediaFormat::Ova),
                episode_count: Some(1),
                ..SearchHit::new(ProviderKey::new("ova"), "Sousou no Frieren")
            },
            hit("tv", "Sousou no Frieren"),
        ]);

        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        assert_eq!(r.key().unwrap().as_str(), "tv", "the OVA must not win");
    }

    #[tokio::test]
    async fn resolution_falls_through_to_a_healthy_provider() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("dead").failing(ProviderError::Transport("timeout".into())).arc(),
            MockProvider::new("alive").with_hits(vec![hit("a-1", "Sousou no Frieren")]).arc(),
        ]);

        let r = resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        match &r {
            Resolution::Resolved { provider_id, key, .. } => {
                assert_eq!(provider_id, "alive");
                assert_eq!(key.as_str(), "a-1");
            }
            other => panic!("expected failover to succeed, got {other:?}"),
        }
        // And the cache is keyed to the provider that actually answered.
        assert!(store.lookup_mapping(FRIEREN, "alive").unwrap().is_some());
        assert!(store.lookup_mapping(FRIEREN, "dead").unwrap().is_none());
    }

    #[tokio::test]
    async fn a_target_with_no_titles_cannot_search() {
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![hit("x", "y")]);
        let empty = MatchTarget::default();
        let r = resolve(&store, &registry, FRIEREN, &empty, Translation::Sub, 0).await;
        assert!(matches!(r, Resolution::NotFound { .. }));
    }

    #[test]
    fn explanations_distinguish_how_a_match_was_reached() {
        // Shown in the UI, so a remembered guess must not look like a certainty.
        let resolved = |rung, confidence| Resolution::Resolved {
            provider_id: "p".into(),
            key: ProviderKey::new("k"),
            confidence,
            rung,
        };
        assert_eq!(resolved(ResolutionRung::Override, 1.0).explain(), "your correction");
        assert_eq!(resolved(ResolutionRung::Cache, 0.8).explain(), "remembered (80%)");
        assert_eq!(resolved(ResolutionRung::DatasetId, 1.0).explain(), "id mapping");
        assert!(
            resolved(ResolutionRung::ProviderSearch, 0.9).explain().contains("title match")
        );
    }

    #[test]
    fn the_confidence_floor_is_shared_with_the_matcher() {
        // Two different floors would mean the ladder and the scorer disagreed about what
        // counts as certain.
        assert_eq!(confidence_floor(), CONFIDENCE_FLOOR);
    }

    #[tokio::test]
    async fn a_cached_resolution_is_reused_without_searching_again() {
        let store = Store::open_in_memory().unwrap();
        let first =
            MockProvider::new("torrent").with_hits(vec![hit("t-1", "Sousou no Frieren")]);
        let calls = first.call_count();
        let registry = ProviderRegistry::new(vec![first.arc()]);

        resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 0).await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

        resolve(&store, &registry, FRIEREN, &target(), Translation::Sub, 1).await;
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the second call should have been served from cache"
        );
    }
}
