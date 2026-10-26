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

use crate::registry::{Attempt, ProviderRegistry};

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
/// One provider's key, for the callers that work with one source. To ask the whole chain for
/// something about a title, use [`first_answer`], which matches every source on its own.
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

/// What the first source that could answer for a title said.
#[derive(Debug)]
pub enum Answer<T> {
    /// A source matched the title, under its own key, and answered.
    Answered { provider_id: String, key: ProviderKey, value: T },
    /// A source found the title's candidates but could not choose between them. A question for
    /// the user, asked before any source further down the order is tried.
    Ambiguous { provider_id: String, candidates: Vec<Candidate> },
    /// No source answered. What each one said, in order, and whether any of them matched the
    /// title at all — a title no source has is a different answer from an episode none has.
    Unanswered { reasons: Vec<String>, matched: bool },
}

impl<T> Answer<T> {
    /// Why nothing came back, for a toast. Empty for an answer.
    pub fn explain(&self) -> String {
        match self {
            Self::Answered { .. } => String::new(),
            Self::Ambiguous { provider_id, candidates } => {
                format!("{} possible matches on {provider_id}", candidates.len())
            }
            Self::Unanswered { reasons, .. } if reasons.is_empty() => {
                "no providers available".into()
            }
            Self::Unanswered { reasons, .. } => reasons.join(", "),
        }
    }
}

/// What one source made of a title: see [`match_and_ask`].
enum Outcome<T> {
    Answered(Contribution<T>),
    Ambiguous { provider_id: String, candidates: Vec<Candidate> },
    /// No match or no answer, with why. `matched` is whether the title was found at all.
    Failed { reason: String, matched: bool },
    /// Not asked: switched off between the walk starting and this source's turn.
    Skipped,
}

/// Match a title on one source, under that source's own key, and ask it.
async fn match_and_ask<T, F, Fut>(
    store: &Store,
    registry: &ProviderRegistry,
    provider_id: &str,
    anilist_id: AnilistId,
    target: &MatchTarget,
    translation: Translation,
    now: i64,
    ask: &F,
) -> Outcome<T>
where
    F: Fn(ProviderRegistry, ProviderKey) -> Fut,
    Fut: std::future::Future<Output = Attempt<T>>,
{
    let Some(source) = registry.pinned(provider_id) else { return Outcome::Skipped };
    let began = std::time::Instant::now();
    let ms = |began: std::time::Instant| began.elapsed().as_millis() as u64;
    match resolve(store, &source, anilist_id, target, translation, now).await {
        Resolution::Resolved { key, confidence, rung, .. } => {
            tracing::info!(provider = %provider_id, ?rung, confidence, elapsed_ms = ms(began), "title matched");
            let asked = std::time::Instant::now();
            let attempt = ask(source, key.clone()).await;
            let summary = attempt.summary();
            match attempt.value {
                Some(value) => {
                    tracing::info!(provider = %provider_id, elapsed_ms = ms(asked), "source answered");
                    Outcome::Answered(Contribution {
                        provider_id: provider_id.to_owned(),
                        key,
                        value,
                    })
                }
                None => {
                    tracing::info!(provider = %provider_id, reason = %summary, elapsed_ms = ms(asked), "source had no answer");
                    Outcome::Failed { reason: summary, matched: true }
                }
            }
        }
        Resolution::Ambiguous { provider_id, candidates } => {
            tracing::info!(provider = %provider_id, candidates = candidates.len(), elapsed_ms = ms(began), "title ambiguous; asking");
            Outcome::Ambiguous { provider_id, candidates }
        }
        // Named once: a reason that already names its source keeps its own wording.
        Resolution::NotFound { reason } => {
            tracing::info!(provider = %provider_id, %reason, elapsed_ms = ms(began), "no match");
            let reason = if reason.starts_with(provider_id) {
                reason
            } else {
                format!("{provider_id}: {reason}")
            };
            Outcome::Failed { reason, matched: false }
        }
    }
}

/// Ask each usable source in turn about a title, each under its own key, until one answers.
///
/// A key is only meaningful to the source that issued it, so every source is matched on its own
/// — from its remembered match or by searching it — before it is asked, and asked alone. Sources
/// keep their own catalogues, so one that cannot match the title, or has no such episode, says
/// nothing about the next, and the walk moves on, as it does past a source that fails. A source
/// that finds candidates but cannot choose stops it instead: the user is asked, the answer is
/// remembered as an override, and the order stays the user's rather than skipping ahead to
/// whichever source happened to be sure.
///
/// For something asked of one episode at a time, such as its streams. For a whole listing, which
/// one source may have only part of, use [`gather`].
pub async fn first_answer<T, F, Fut>(
    store: &Store,
    registry: &ProviderRegistry,
    anilist_id: AnilistId,
    target: &MatchTarget,
    translation: Translation,
    now: i64,
    ask: F,
) -> Answer<T>
where
    F: Fn(ProviderRegistry, ProviderKey) -> Fut,
    Fut: std::future::Future<Output = Attempt<T>>,
{
    let mut reasons = Vec::new();
    let mut matched = false;
    let usable = registry.usable_ids();
    let skipped: Vec<_> =
        registry.ids().into_iter().filter(|id| !usable.contains(id)).collect();
    if !skipped.is_empty() {
        tracing::info!(?skipped, "sources resting after failures, not asked");
    }
    for provider_id in usable {
        match match_and_ask(store, registry, &provider_id, anilist_id, target, translation, now, &ask)
            .await
        {
            Outcome::Answered(Contribution { provider_id, key, value }) => {
                return Answer::Answered { provider_id, key, value };
            }
            Outcome::Ambiguous { provider_id, candidates } => {
                return Answer::Ambiguous { provider_id, candidates };
            }
            Outcome::Failed { reason, matched: found } => {
                matched |= found;
                reasons.push(reason);
            }
            Outcome::Skipped => {}
        }
    }
    Answer::Unanswered { reasons, matched }
}

/// What one source contributed to a [`gather`]: its answer, and the key it was matched under.
#[derive(Debug, Clone, PartialEq)]
pub struct Contribution<T> {
    pub provider_id: String,
    pub key: ProviderKey,
    pub value: T,
}

/// What [`gather`] came to.
#[derive(Debug)]
pub enum Gathered<T> {
    /// Every source that answered, highest ranked first. Never empty.
    Answered(Vec<Contribution<T>>),
    /// The source first in the order that matched anything found candidates and could not
    /// choose: a question for the user. See [`Answer::Ambiguous`].
    Ambiguous { provider_id: String, candidates: Vec<Candidate> },
    /// No source answered. See [`Answer::Unanswered`].
    Unanswered { reasons: Vec<String>, matched: bool },
}

impl<T> Gathered<T> {
    /// Why nothing came back, for a toast. Empty for an answer.
    pub fn explain(&self) -> String {
        match self {
            Self::Answered(_) => String::new(),
            Self::Ambiguous { provider_id, candidates } => {
                format!("{} possible matches on {provider_id}", candidates.len())
            }
            Self::Unanswered { reasons, .. } if reasons.is_empty() => {
                "no providers available".into()
            }
            Self::Unanswered { reasons, .. } => reasons.join(", "),
        }
    }
}

/// How long a source ranked below the first waits to be asked, when the sources above it have
/// not yet shown they fall short. Most titles are whole on the first source, and asking the
/// rest would be a search for nothing, on a torrent source one behind a tunnel.
const HEDGE: std::time::Duration = std::time::Duration::from_millis(400);

/// Ask the usable sources about a title together and keep every answer, for a listing that one
/// source may have only part of.
///
/// Each source is matched and asked under its own key, as in [`first_answer`]. The first source
/// is asked at once. The others start after [`HEDGE`], or as soon as an answer comes back that
/// `complete` says does not cover everything, so a slow source costs the wait for the slowest
/// one that was needed and not the sum of them. Sources that cannot outrank anything already
/// found are not waited for once `complete` is satisfied, and are dropped.
///
/// `progress` sees the answers in rank order each time one more arrives, so a page can show the
/// first table without waiting for the rest.
///
/// The user's order decides which source wins an episode, and decides what is asked: a source
/// first in the order that finds candidates and cannot choose is a question, as in
/// [`first_answer`], whatever the sources below it said.
pub async fn gather<T, F, Fut>(
    store: &Store,
    registry: &ProviderRegistry,
    anilist_id: AnilistId,
    target: &MatchTarget,
    translation: Translation,
    now: i64,
    ask: F,
    complete: impl Fn(&[Contribution<T>]) -> bool,
    mut progress: impl FnMut(&[Contribution<T>]),
) -> Gathered<T>
where
    T: Clone,
    F: Fn(ProviderRegistry, ProviderKey) -> Fut,
    Fut: std::future::Future<Output = Attempt<T>>,
{
    use futures::StreamExt;

    let usable = registry.usable_ids();
    let skipped: Vec<_> =
        registry.ids().into_iter().filter(|id| !usable.contains(id)).collect();
    if !skipped.is_empty() {
        tracing::info!(?skipped, "sources resting after failures, not asked");
    }
    let (nudge, _) = tokio::sync::watch::channel(());
    let mut running = futures::stream::FuturesUnordered::new();
    for (rank, provider_id) in usable.iter().enumerate() {
        let ask = &ask;
        // Subscribed now rather than when first polled, so a nudge sent before this source
        // has been polled at all is not missed.
        let mut hurried = nudge.subscribe();
        running.push(async move {
            if rank > 0 {
                tokio::select! {
                    () = tokio::time::sleep(HEDGE) => {}
                    _ = hurried.changed() => {}
                }
            }
            let outcome = match_and_ask(
                store, registry, provider_id, anilist_id, target, translation, now, ask,
            )
            .await;
            (rank, outcome)
        });
    }

    let mut outcomes: Vec<Option<Outcome<T>>> = usable.iter().map(|_| None).collect();
    while let Some((rank, outcome)) = running.next().await {
        outcomes[rank] = Some(outcome);
        let answered: Vec<Contribution<T>> = outcomes
            .iter()
            .filter_map(|o| match o {
                Some(Outcome::Answered(c)) => Some(c.clone()),
                _ => None,
            })
            .collect();
        if matches!(outcomes[rank], Some(Outcome::Answered(_))) {
            progress(&answered);
        }
        // The first source in the order that settled with anything but "no": if the sources
        // above it are all settled and it is a question, it is the answer already.
        let settled_question = outcomes
            .iter()
            .take_while(|o| {
                matches!(o, Some(Outcome::Failed { .. } | Outcome::Skipped))
            })
            .count();
        if matches!(outcomes.get(settled_question), Some(Some(Outcome::Ambiguous { .. }))) {
            break;
        }
        if !complete(&answered) {
            let _ = nudge.send(());
            continue;
        }
        // Complete: only a source that outranks one that answered could still change who
        // serves an episode.
        let last = outcomes
            .iter()
            .rposition(|o| matches!(o, Some(Outcome::Answered(_))))
            .unwrap_or(0);
        if outcomes[..last].iter().all(Option::is_some) {
            break;
        }
    }
    drop(running);

    let mut reasons = Vec::new();
    let mut matched = false;
    let mut answered = Vec::new();
    let mut question = None;
    for outcome in outcomes.into_iter().flatten() {
        match outcome {
            Outcome::Answered(contribution) => answered.push(contribution),
            Outcome::Ambiguous { provider_id, candidates } => {
                // Only the first of the order that is not a plain "no" asks.
                if answered.is_empty() && question.is_none() {
                    question = Some((provider_id, candidates));
                }
            }
            Outcome::Failed { reason, matched: found } => {
                matched |= found;
                reasons.push(reason);
            }
            Outcome::Skipped => {}
        }
    }
    if let Some((provider_id, candidates)) = question {
        return Gathered::Ambiguous { provider_id, candidates };
    }
    if answered.is_empty() {
        return Gathered::Unanswered { reasons, matched };
    }
    Gathered::Answered(answered)
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

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    fn target() -> MatchTarget {
        MatchTarget {
            titles: vec![
                "Hoshikuzu no Tabibito".into(),
                "Hoshikuzu: Wayfarer's Long Road".into(),
            ],
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
        let registry = registry_with(vec![hit("t-1", "Hoshikuzu no Tabibito")]);

        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
        match &r {
            Resolution::Resolved { key, rung, .. } => {
                assert_eq!(key.as_str(), "t-1");
                assert_eq!(*rung, ResolutionRung::ProviderSearch);
            }
            other => panic!("expected a resolution, got {other:?}"),
        }

        // Cached as rung 2 for next time, but *not* as an override.
        let cached = store.lookup_mapping(HOSHIKUZU, "torrent").unwrap().unwrap();
        assert_eq!(cached.provider_key.as_str(), "t-1");
        assert_ne!(cached.rung, ResolutionRung::Override, "must stay revisable");
    }

    #[tokio::test]
    async fn an_override_wins_without_touching_the_provider() {
        // Rung 1 must short-circuit: the user already told us the answer.
        let store = Store::open_in_memory().unwrap();
        store.set_override(HOSHIKUZU, "torrent", &ProviderKey::new("corrected"), 0).unwrap();

        let provider = MockProvider::new("torrent").with_hits(vec![hit("wrong", "Hoshikuzu")]);
        let calls = provider.call_count();
        let registry = ProviderRegistry::new(vec![provider.arc()]);

        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
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

        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
        assert!(r.needs_user_input());
        match r {
            Resolution::Ambiguous { candidates, .. } => assert_eq!(candidates.len(), 1),
            other => panic!("expected ambiguity, got {other:?}"),
        }
        // Nothing weak may be cached, or the guess would stick.
        assert!(store.lookup_mapping(HOSHIKUZU, "torrent").unwrap().is_none());
    }

    #[tokio::test]
    async fn confirming_a_choice_writes_an_override_that_survives_a_refresh() {
        // This is what makes disambiguating worth doing exactly once.
        let store = Store::open_in_memory().unwrap();
        let r = confirm(&store, HOSHIKUZU, "torrent", &ProviderKey::new("picked"), 0);
        assert_eq!(r.key().unwrap().as_str(), "picked");

        store.clear_cached_resolutions().unwrap();
        let after = store.lookup_mapping(HOSHIKUZU, "torrent").unwrap().unwrap();
        assert_eq!(after.provider_key.as_str(), "picked");
        assert_eq!(after.rung, ResolutionRung::Override);
    }

    #[tokio::test]
    async fn a_previously_confirmed_title_is_never_asked_about_again() {
        let store = Store::open_in_memory().unwrap();
        confirm(&store, HOSHIKUZU, "torrent", &ProviderKey::new("picked"), 0);

        let registry = registry_with(vec![hit("something-else", "Hoshikuzu")]);
        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 1).await;
        assert!(!r.needs_user_input());
        assert_eq!(r.key().unwrap().as_str(), "picked");
    }

    #[tokio::test]
    async fn an_empty_result_set_offers_a_manual_query() {
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![]);
        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
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
        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
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
        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
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
                ..SearchHit::new(ProviderKey::new("ova"), "Hoshikuzu no Tabibito")
            },
            hit("tv", "Hoshikuzu no Tabibito"),
        ]);

        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
        assert_eq!(r.key().unwrap().as_str(), "tv", "the OVA must not win");
    }

    #[tokio::test]
    async fn resolution_falls_through_to_a_healthy_provider() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("dead").failing(ProviderError::Transport("timeout".into())).arc(),
            MockProvider::new("alive")
                .with_hits(vec![hit("a-1", "Hoshikuzu no Tabibito")])
                .arc(),
        ]);

        let r = resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
        match &r {
            Resolution::Resolved { provider_id, key, .. } => {
                assert_eq!(provider_id, "alive");
                assert_eq!(key.as_str(), "a-1");
            }
            other => panic!("expected failover to succeed, got {other:?}"),
        }
        // And the cache is keyed to the provider that actually answered.
        assert!(store.lookup_mapping(HOSHIKUZU, "alive").unwrap().is_some());
        assert!(store.lookup_mapping(HOSHIKUZU, "dead").unwrap().is_none());
    }

    #[tokio::test]
    async fn a_target_with_no_titles_cannot_search() {
        let store = Store::open_in_memory().unwrap();
        let registry = registry_with(vec![hit("x", "y")]);
        let empty = MatchTarget::default();
        let r = resolve(&store, &registry, HOSHIKUZU, &empty, Translation::Sub, 0).await;
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
            MockProvider::new("torrent").with_hits(vec![hit("t-1", "Hoshikuzu no Tabibito")]);
        let calls = first.call_count();
        let registry = ProviderRegistry::new(vec![first.arc()]);

        resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 0).await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

        resolve(&store, &registry, HOSHIKUZU, &target(), Translation::Sub, 1).await;
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the second call should have been served from cache"
        );
    }

    /// An invented title, for the tests that walk several sources.
    fn wanderer() -> MatchTarget {
        MatchTarget {
            titles: vec!["Hoshikuzu no Tabibito".into()],
            episode_count: Some(12),
            year: Some(2024),
            format: Some(MediaFormat::Tv),
        }
    }

    fn wanderer_hit(key: &str) -> SearchHit {
        SearchHit {
            episode_count: Some(12),
            year: Some(2024),
            format: Some(MediaFormat::Tv),
            ..SearchHit::new(ProviderKey::new(key), "Hoshikuzu no Tabibito")
        }
    }

    const WANDERER: AnilistId = AnilistId::new(900_001);

    async fn ask_for_streams(
        store: &Store,
        registry: &ProviderRegistry,
    ) -> Answer<Vec<anistream_core::stream::Stream>> {
        first_answer(
            store,
            registry,
            WANDERER,
            &wanderer(),
            Translation::Sub,
            0,
            |source, key| async move { source.resolve(&key, "1", Translation::Sub, 0).await },
        )
        .await
    }

    fn stream(url: &str) -> anistream_core::stream::Stream {
        anistream_core::stream::Stream::new(url, anistream_core::stream::StreamKind::Hls)
    }

    #[tokio::test]
    async fn a_source_without_the_title_hands_over_to_the_next() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![]).arc(),
            MockProvider::new("beta")
                .with_hits(vec![wanderer_hit("beta-1")])
                .with_streams(vec![stream("https://beta.example/1.m3u8")])
                .arc(),
        ]);

        match ask_for_streams(&store, &registry).await {
            Answer::Answered { provider_id, key, .. } => {
                assert_eq!(provider_id, "beta");
                assert_eq!(key.as_str(), "beta-1", "each source is asked under its own key");
            }
            other => panic!("expected beta to answer, got {other:?}"),
        }
        // Matched where it was found, so the next play goes straight there.
        let cached = store.lookup_mapping(WANDERER, "beta").unwrap().unwrap();
        assert_eq!(cached.provider_key.as_str(), "beta-1");
        assert!(store.lookup_mapping(WANDERER, "alpha").unwrap().is_none());
    }

    #[tokio::test]
    async fn a_source_without_the_episode_hands_over_to_the_next() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![wanderer_hit("alpha-1")]).arc(),
            MockProvider::new("beta")
                .with_hits(vec![wanderer_hit("beta-1")])
                .with_streams(vec![stream("https://beta.example/1.m3u8")])
                .arc(),
        ]);
        let answer = ask_for_streams(&store, &registry).await;
        assert!(
            matches!(&answer, Answer::Answered { provider_id, .. } if provider_id == "beta")
        );

        // Matched and not played: the episode is what is missing, not the title.
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![wanderer_hit("alpha-1")]).arc(),
        ]);
        let answer = ask_for_streams(&store, &registry).await;
        assert!(matches!(answer, Answer::Unanswered { matched: true, .. }));
    }

    /// The episode numbers of each answer, in rank order, joined by the source.
    async fn gathered(
        store: &Store,
        registry: &ProviderRegistry,
        wanted: u32,
    ) -> Gathered<Vec<anistream_core::media::Episode>> {
        gather(
            store,
            registry,
            WANDERER,
            &wanderer(),
            Translation::Sub,
            0,
            |source, key| async move { source.episodes(&key, Translation::Sub, 0).await },
            |answers| {
                let have: std::collections::BTreeSet<_> = answers
                    .iter()
                    .flat_map(|a| a.value.iter().map(|e| e.number.to_string()))
                    .collect();
                have.len() as u32 >= wanted
            },
            |_| {},
        )
        .await
    }

    fn numbered(numbers: std::ops::RangeInclusive<u32>) -> Vec<anistream_core::media::Episode> {
        numbers.map(anistream_core::media::Episode::new).collect()
    }

    #[tokio::test(start_paused = true)]
    async fn every_source_that_answers_is_kept_and_asked_together() {
        let store = Store::open_in_memory().unwrap();
        let slow = std::time::Duration::from_secs(2);
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha")
                .with_latency(slow)
                .with_hits(vec![wanderer_hit("alpha-1")])
                .with_episodes(numbered(1..=6))
                .arc(),
            MockProvider::new("beta")
                .with_latency(slow)
                .with_hits(vec![wanderer_hit("beta-1")])
                .with_episodes(numbered(1..=12))
                .arc(),
        ]);
        let began = tokio::time::Instant::now();
        let Gathered::Answered(answers) = gathered(&store, &registry, 12).await else {
            panic!("both answer")
        };
        let who: Vec<_> = answers.iter().map(|a| a.provider_id.as_str()).collect();
        assert_eq!(who, ["alpha", "beta"], "in rank order");
        // Each takes two calls of two seconds, in turn within a source. Asked together, the
        // pair costs about one source, plus the short wait before the second starts.
        assert!(began.elapsed() < std::time::Duration::from_secs(5), "{:?}", began.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_source_that_has_it_all_leaves_the_rest_unasked() {
        let store = Store::open_in_memory().unwrap();
        let beta = MockProvider::new("beta").with_hits(vec![wanderer_hit("beta-1")]);
        let beta_calls = beta.call_count();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha")
                .with_hits(vec![wanderer_hit("alpha-1")])
                .with_episodes(numbered(1..=12))
                .arc(),
            beta.arc(),
        ]);
        let Gathered::Answered(answers) = gathered(&store, &registry, 12).await else {
            panic!("alpha answers")
        };
        assert_eq!(answers.len(), 1);
        assert_eq!(beta_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_source_that_falls_short_starts_the_next_at_once() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha")
                .with_hits(vec![wanderer_hit("alpha-1")])
                .with_episodes(numbered(1..=6))
                .arc(),
            MockProvider::new("beta")
                .with_hits(vec![wanderer_hit("beta-1")])
                .with_episodes(numbered(1..=12))
                .arc(),
        ]);
        let began = tokio::time::Instant::now();
        let Gathered::Answered(answers) = gathered(&store, &registry, 12).await else {
            panic!("both answer")
        };
        assert_eq!(answers.len(), 2);
        assert!(began.elapsed() < HEDGE, "waited out the hedge: {:?}", began.elapsed());
    }

    #[tokio::test(start_paused = true)]
    async fn a_source_that_cannot_choose_is_the_question_even_if_a_later_one_answers() {
        let store = Store::open_in_memory().unwrap();
        let unsure = SearchHit {
            episode_count: None,
            year: None,
            format: None,
            ..SearchHit::new(ProviderKey::new("alpha-x"), "Tabibito")
        };
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![unsure]).arc(),
            MockProvider::new("beta")
                .with_hits(vec![wanderer_hit("beta-1")])
                .with_episodes(numbered(1..=12))
                .arc(),
        ]);
        let got = gathered(&store, &registry, 12).await;
        assert!(matches!(&got, Gathered::Ambiguous { provider_id, .. } if provider_id == "alpha"));
    }

    #[tokio::test(start_paused = true)]
    async fn nothing_answering_says_why_for_each_source() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![]).arc(),
            MockProvider::new("beta").with_hits(vec![wanderer_hit("beta-1")]).failing(
                ProviderError::NotFound,
            ).arc(),
        ]);
        let Gathered::Unanswered { reasons, .. } = gathered(&store, &registry, 12).await else {
            panic!("no answer")
        };
        assert_eq!(reasons.len(), 2);
    }

    #[tokio::test]
    async fn a_source_that_cannot_choose_asks_before_the_next_is_tried() {
        let store = Store::open_in_memory().unwrap();
        let beta = MockProvider::new("beta").with_hits(vec![wanderer_hit("beta-1")]);
        let beta_calls = beta.call_count();
        let unsure = SearchHit {
            episode_count: None,
            year: None,
            format: None,
            ..SearchHit::new(ProviderKey::new("alpha-x"), "Tabibito")
        };
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![unsure]).arc(),
            beta.arc(),
        ]);

        let answer = ask_for_streams(&store, &registry).await;
        assert!(
            matches!(&answer, Answer::Ambiguous { provider_id, .. } if provider_id == "alpha")
        );
        assert_eq!(beta_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn when_no_source_answers_each_says_why() {
        let store = Store::open_in_memory().unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("alpha").with_hits(vec![]).arc(),
            MockProvider::new("beta")
                .failing(ProviderError::Blocked("operation timed out".into()))
                .arc(),
        ]);
        let answer = ask_for_streams(&store, &registry).await;
        assert_eq!(answer.explain(), "alpha found no matches, beta: timed out");
        assert!(matches!(answer, Answer::Unanswered { matched: false, .. }));
    }
}
