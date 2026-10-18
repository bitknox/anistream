//! Provider registry and failover.
//!
//! The single most important behaviour in this module is *when not to fail over*. A
//! provider that answers correctly and simply has no such title has done its job; moving on
//! to the next one would burn the whole chain to reach the same answer, and would mask a
//! correct "not found" behind whatever the last provider in the list happens to say.
//!
//! So `NotFound` stops the walk, while `Blocked`, `Parse` and `Transport` continue it. That
//! asymmetry is [`ProviderError::should_failover`], and it is the reason the error enum is
//! shaped the way it is.

use std::{sync::Arc, time::Instant};

use anistream_core::{
    Error,
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::Stream,
    traits::{Provider, ProviderKind, SourceCandidate},
};

use crate::health::HealthTracker;

/// Providers in preference order, with shared health.
#[derive(Clone)]
pub struct ProviderRegistry {
    /// Behind a lock because sources can arrive *after* startup. Plugins are the reason: compiling
    /// a component costs hundreds of milliseconds, and paying that before the first frame made the
    /// app feel slow for a capability most launches never use. They now load in the background and
    /// join the chain when they are ready.
    providers: Arc<std::sync::RwLock<Vec<Arc<dyn Provider>>>>,
    health: HealthTracker,
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("providers", &self.ids())
            .finish_non_exhaustive()
    }
}

/// The outcome of walking the chain, including what each provider said.
///
/// Failures are carried rather than discarded so the UI can name the source that broke.
/// An empty list with no explanation is the failure mode this design exists to avoid.
#[derive(Debug)]
pub struct Attempt<T> {
    pub value: Option<T>,
    /// Which provider produced `value`.
    pub provider: Option<String>,
    pub failures: Vec<(String, ProviderError)>,
}

impl<T> Attempt<T> {
    fn success(provider: String, value: T, failures: Vec<(String, ProviderError)>) -> Self {
        Self { value: Some(value), provider: Some(provider), failures }
    }

    fn exhausted(failures: Vec<(String, ProviderError)>) -> Self {
        Self { value: None, provider: None, failures }
    }

    pub fn is_success(&self) -> bool {
        self.value.is_some()
    }

    /// Turn an exhausted attempt into a reportable error.
    pub fn into_error(self, title: &str, episode: &str) -> Error {
        Error::AllProvidersFailed {
            title: title.to_owned(),
            episode: episode.to_owned(),
            failures: self.failures,
        }
    }

    /// One-line summary for a toast.
    pub fn summary(&self) -> String {
        if self.failures.is_empty() {
            return "no providers configured".into();
        }
        self.failures
            .iter()
            .map(|(id, e)| format!("{id}: {}", described(e)))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A failure as a toast names it: by its kind, unless it says it timed out.
///
/// The plugin vocabulary has no transport failure, so a plugin's timeout arrives as `Blocked`,
/// and a toast reading "blocked" over a log reading "operation timed out" sent the viewer after
/// a ban when the source had only been slow to answer.
fn described(error: &ProviderError) -> &'static str {
    let detail = match error {
        ProviderError::NotFound => "",
        ProviderError::Blocked(d)
        | ProviderError::Parse(d)
        | ProviderError::Transport(d)
        | ProviderError::Unavailable(d)
        | ProviderError::Other(d) => d,
    };
    if detail.to_ascii_lowercase().contains("timed out") {
        "timed out"
    } else {
        error.state_label()
    }
}

impl ProviderRegistry {
    pub fn new(providers: Vec<Arc<dyn Provider>>) -> Self {
        let health = HealthTracker::new(providers.iter().map(|p| p.manifest().id.clone()));
        Self { providers: Arc::new(std::sync::RwLock::new(providers)), health }
    }

    /// Add sources discovered after startup, keeping config order.
    ///
    /// Appended rather than inserted at a position: `providers.order` already put plugins last by
    /// default, and a late arrival jumping ahead of a working source would change which one serves
    /// a stream depending on how fast it compiled — a race deciding playback quality.
    pub fn extend(&self, more: Vec<Arc<dyn Provider>>) {
        if more.is_empty() {
            return;
        }
        self.health.register(more.iter().map(|p| p.manifest().id.clone()));
        let mut providers = self.providers.write().unwrap_or_else(|e| e.into_inner());
        providers.extend(more);
    }

    /// Swap the whole chain, for a configuration change that redefines what sources exist.
    ///
    /// In place rather than by handing out a new registry: every caller holds a clone, and
    /// a new object would leave all of them talking to the old chain — which for the torrent
    /// source means a session the user has just switched off carrying on regardless.
    pub fn replace(&self, providers: Vec<Arc<dyn Provider>>) {
        self.health.register(providers.iter().map(|p| p.manifest().id.clone()));
        let mut held = self.providers.write().unwrap_or_else(|e| e.into_inner());
        *held = providers;
    }

    /// Add a source, or swap it for a newer build of itself.
    ///
    /// A replacement keeps its place in the chain. An updated plugin moving to the back would
    /// change which source serves a stream because of when it was installed, the same race
    /// [`Self::extend`] avoids for late arrivals.
    pub fn put(&self, provider: Arc<dyn Provider>) {
        let id = provider.manifest().id.clone();
        self.health.register([id.clone()]);
        let mut providers = self.providers.write().unwrap_or_else(|e| e.into_inner());
        match providers.iter().position(|p| p.manifest().id == id) {
            Some(at) => providers[at] = provider,
            None => providers.push(provider),
        }
    }

    /// Add a source that came up after the chain was built, at the place a ranking gives it.
    ///
    /// Before the first source that ranks after it, so a source that arrives late — the torrent
    /// source waits for the VPN guard — still sits where `providers.order` put it rather than
    /// wherever its arrival time would. A source with the same id is replaced.
    pub fn insert_ranked(
        &self,
        provider: Arc<dyn Provider>,
        rank: impl Fn(&dyn Provider) -> usize,
    ) {
        let id = provider.manifest().id.clone();
        self.health.register([id.clone()]);
        let mut providers = self.providers.write().unwrap_or_else(|e| e.into_inner());
        providers.retain(|p| p.manifest().id != id);
        let place = rank(provider.as_ref());
        let at =
            providers.iter().position(|p| rank(p.as_ref()) > place).unwrap_or(providers.len());
        providers.insert(at, provider);
    }

    /// Take a source out of the chain. Returns whether there was one to take.
    pub fn remove(&self, id: &str) -> bool {
        let mut providers = self.providers.write().unwrap_or_else(|e| e.into_inner());
        let before = providers.len();
        providers.retain(|p| p.manifest().id != id);
        providers.len() != before
    }

    /// The whole chain, for a caller assembling a replacement for it.
    pub fn all(&self) -> Vec<Arc<dyn Provider>> {
        self.snapshot()
    }

    /// The providers of one kind, so a rebuild can keep what it did not rebuild.
    ///
    /// Plugins are the case this exists for: they compile in the background and join the
    /// chain late, so a reload that only knows about configured sources would drop them.
    pub fn of_kind(&self, kind: ProviderKind) -> Vec<Arc<dyn Provider>> {
        self.snapshot().into_iter().filter(|p| p.manifest().kind == kind).collect()
    }

    /// A snapshot of the chain.
    ///
    /// Cloned out rather than held: every caller below is `async`, and keeping a `std` lock alive
    /// across an `await` is how a deadlock gets written.
    fn snapshot(&self) -> Vec<Arc<dyn Provider>> {
        self.providers.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn health(&self) -> &HealthTracker {
        &self.health
    }

    pub fn ids(&self) -> Vec<String> {
        self.snapshot().iter().map(|p| p.manifest().id.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.providers.read().unwrap_or_else(|e| e.into_inner()).is_empty()
    }

    /// Returns an owned handle rather than a borrow: the chain is behind a lock now, so nothing
    /// can hand out a reference into it and outlive the read.
    pub fn get(&self, id: &str) -> Option<Arc<dyn Provider>> {
        self.snapshot().into_iter().find(|p| p.manifest().id == id)
    }

    /// The same registry narrowed to one provider, for a title the user has pinned.
    ///
    /// A view rather than a new registry: the [`HealthTracker`] is shared, so a failure while
    /// pinned still counts against that provider everywhere else, and the Providers screen keeps
    /// telling the truth. Building a fresh registry would give the pinned title its own private
    /// health record and quietly re-try a source the rest of the app had already given up on.
    ///
    /// `None` when the id names nothing — a plugin that was removed, or one that has not finished
    /// compiling yet. Callers are expected to read that as "fall back to the normal chain" rather
    /// than as an error: refusing to play because a remembered preference is momentarily
    /// unresolvable would be a worse answer than playing from the default source.
    pub fn pinned(&self, provider_id: &str) -> Option<Self> {
        let provider = self.get(provider_id)?;
        Some(Self {
            providers: Arc::new(std::sync::RwLock::new(vec![provider])),
            health: self.health.clone(),
        })
    }

    /// Providers that are worth trying right now, in preference order.
    fn candidates(&self) -> Vec<Arc<dyn Provider>> {
        let usable = self.health.usable(&self.ids());
        self.snapshot().into_iter().filter(|p| usable.contains(&p.manifest().id)).collect()
    }

    /// Walk the chain until one provider produces a result.
    async fn walk<T, F, Fut>(&self, now: i64, call: F) -> Attempt<T>
    where
        F: Fn(Arc<dyn Provider>) -> Fut,
        Fut: Future<Output = Result<T, ProviderError>>,
    {
        let mut failures = Vec::new();

        for provider in self.candidates() {
            let id = provider.manifest().id.clone();

            // Local policy first: a provider held back by the VPN guard must not be
            // contacted at all, and that is not a health event.
            //
            // Asked every time rather than remembered, because a hold is a statement about the
            // present. Recording it and then skipping the provider forever meant a tunnel that
            // recovered never got to say so.
            match provider.is_available() {
                Err(reason) => {
                    self.health.record_failure(&id, &reason, now);
                    failures.push((id, reason));
                    continue;
                }
                Ok(()) => self.health.release(&id),
            }

            let started = Instant::now();
            match call(provider).await {
                Ok(value) => {
                    self.health.record_success(&id, started.elapsed(), now);
                    return Attempt::success(id, value, failures);
                }
                Err(error) => {
                    self.health.record_failure(&id, &error, now);
                    let should_continue = error.should_failover();
                    tracing::debug!(provider = %id, %error, should_continue, "provider failed");
                    failures.push((id, error));

                    // `NotFound` is an answer, not a fault. Continuing past it would spend
                    // every remaining provider to arrive at the same conclusion.
                    if !should_continue {
                        break;
                    }
                }
            }
        }

        Attempt::exhausted(failures)
    }

    pub async fn search(
        &self,
        query: &str,
        translation: Translation,
        now: i64,
    ) -> Attempt<Vec<SearchHit>> {
        self.walk(now, |p| {
            let query = query.to_owned();
            async move { p.search(&query, translation).await }
        })
        .await
    }

    pub async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
        now: i64,
    ) -> Attempt<Vec<Episode>> {
        self.walk(now, |p| {
            let key = key.clone();
            async move { p.episodes(&key, translation).await }
        })
        .await
    }

    /// Every selectable release across usable providers, in provider order.
    ///
    /// An aggregation rather than a walk: choosing a release is the user's veto over
    /// ranking, so hiding what later providers offer because an earlier one answered
    /// would defeat the point. Failing providers are skipped — a partial slate is
    /// still a slate.
    pub async fn sources(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        now: i64,
    ) -> Vec<SourceCandidate> {
        // Every provider is asked at once. This gathers rather than fails over, so nothing
        // depends on order except the slate's, and `join_all` keeps that in candidate order:
        // the picker waits for the slowest source instead of the sum of them all.
        let listings = self.candidates().into_iter().map(|provider| async move {
            let id = provider.manifest().id.clone();
            if provider.is_available().is_err() {
                return Vec::new();
            }
            let started = Instant::now();
            match provider.sources(key, episode, translation).await {
                Ok(candidates) => {
                    self.health.record_success(&id, started.elapsed(), now);
                    candidates
                }
                Err(error) => {
                    self.health.record_failure(&id, &error, now);
                    tracing::debug!(provider = %id, %error, "sources listing failed");
                    Vec::new()
                }
            }
        });
        futures::future::join_all(listings).await.concat()
    }

    /// Resolve a specific pick from [`Self::sources`] on the provider that offered it.
    ///
    /// Deliberately no failover: substituting another provider's stream for the exact
    /// release the user chose would silently undo the choice.
    pub async fn resolve_source(
        &self,
        provider_id: &str,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        source_id: &str,
        now: i64,
    ) -> Result<Vec<Stream>, ProviderError> {
        let provider = self
            .snapshot()
            .into_iter()
            .find(|p| p.manifest().id == provider_id)
            .ok_or(ProviderError::NotFound)?;
        provider.is_available()?;
        let started = Instant::now();
        match provider.resolve_source(key, episode, translation, source_id).await {
            Ok(streams) if streams.is_empty() => Err(ProviderError::NotFound),
            Ok(streams) => {
                self.health.record_success(provider_id, started.elapsed(), now);
                Ok(streams)
            }
            Err(error) => {
                self.health.record_failure(provider_id, &error, now);
                Err(error)
            }
        }
    }

    pub async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        now: i64,
    ) -> Attempt<Vec<Stream>> {
        self.walk(now, |p| {
            let key = key.clone();
            let episode = episode.to_owned();
            async move {
                let streams = p.resolve(&key, &episode, translation).await?;
                // A provider returning zero streams has not succeeded. Treating an empty
                // list as success would end the walk and leave nothing to play.
                if streams.is_empty() {
                    return Err(ProviderError::NotFound);
                }
                Ok(streams)
            }
        })
        .await
    }

    /// Probe every provider, for the Providers screen.
    ///
    /// Concurrently: each provider is its own host, so the screen waits for the slowest probe
    /// rather than the sum of them. Each latency is still timed from its own start.
    pub async fn check_all(&self, now: i64) {
        let probes = self.snapshot().into_iter().map(|provider| async move {
            let id = provider.manifest().id.clone();
            if let Err(reason) = provider.is_available() {
                self.health.record_failure(&id, &reason, now);
                return;
            }
            let started = Instant::now();
            match provider.health_check().await {
                Ok(()) => self.health.record_success(&id, started.elapsed(), now),
                Err(e) => self.health.record_failure(&id, &e, now),
            }
        });
        futures::future::join_all(probes).await;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::mock::MockProvider;

    #[tokio::test(start_paused = true)]
    async fn every_provider_is_probed_at_once() {
        let slow = |id| MockProvider::new(id).with_latency(Duration::from_millis(300)).arc();
        let registry = registry(vec![slow("a"), slow("b"), slow("c")]);

        let started = tokio::time::Instant::now();
        registry.check_all(0).await;
        let took = started.elapsed();
        assert!(took < Duration::from_millis(400), "probes ran in turn: {took:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn sources_are_gathered_at_once_and_kept_in_provider_order() {
        let listing = |id: &str, ms| {
            MockProvider::new(id)
                .with_latency(Duration::from_millis(ms))
                .with_sources(vec![SourceCandidate {
                    id: format!("{id}-release"),
                    provider_id: id.into(),
                    title: id.into(),
                    quality: None,
                    seeders: None,
                    size: None,
                    dual_audio: false,
                    dubbed: false,
                    auto_pick: false,
                }])
                .arc()
        };
        // The first provider answers last, so arrival order is the reverse of provider order.
        let registry = registry(vec![listing("a", 300), listing("b", 100)]);

        let started = tokio::time::Instant::now();
        let slate = registry.sources(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        let took = started.elapsed();
        assert!(took < Duration::from_millis(400), "providers were asked in turn: {took:?}");
        let order: Vec<_> = slate.iter().map(|c| c.provider_id.as_str()).collect();
        assert_eq!(order, ["a", "b"], "the slate must keep provider order, not arrival order");
    }

    fn registry(providers: Vec<Arc<dyn Provider>>) -> ProviderRegistry {
        ProviderRegistry::new(providers)
    }

    #[test]
    fn an_updated_source_keeps_its_place_and_a_new_one_goes_last() {
        let r = registry(vec![
            MockProvider::new("a").arc(),
            MockProvider::new("b").arc(),
            MockProvider::new("c").arc(),
        ]);
        r.put(MockProvider::new("b").with_streams(vec![stream("newer")]).arc());
        assert_eq!(r.ids(), ["a", "b", "c"]);
        r.put(MockProvider::new("d").arc());
        assert_eq!(r.ids(), ["a", "b", "c", "d"]);
    }

    #[test]
    fn a_late_source_takes_the_place_its_rank_gives_it() {
        // The torrent source comes up once the VPN guard passes, after sources that were
        // ready at once; it must still be tried in the order the configuration names.
        let rank = |p: &dyn Provider| match p.manifest().id.as_str() {
            "early" => 0,
            "late" => 1,
            _ => 2,
        };
        let r =
            registry(vec![MockProvider::new("early").arc(), MockProvider::new("after").arc()]);
        r.insert_ranked(MockProvider::new("late").arc(), rank);
        assert_eq!(r.ids(), ["early", "late", "after"]);
        r.insert_ranked(MockProvider::new("late").arc(), rank);
        assert_eq!(r.ids(), ["early", "late", "after"], "replaced, not doubled");
    }

    #[test]
    fn a_removed_source_leaves_the_chain() {
        let r = registry(vec![MockProvider::new("a").arc(), MockProvider::new("b").arc()]);
        assert!(r.remove("a"));
        assert!(!r.remove("a"));
        assert_eq!(r.ids(), ["b"]);
    }

    fn stream(url: &str) -> Stream {
        Stream::new(url, anistream_core::stream::StreamKind::Hls)
    }

    #[tokio::test]
    async fn the_first_working_provider_wins() {
        let r = registry(vec![
            MockProvider::new("a").with_streams(vec![stream("from-a")]).arc(),
            MockProvider::new("b").with_streams(vec![stream("from-b")]).arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert_eq!(attempt.provider.as_deref(), Some("a"));
        assert_eq!(attempt.value.unwrap()[0].url, "from-a");
        assert!(attempt.failures.is_empty());
    }

    #[tokio::test]
    async fn pinning_uses_the_named_provider_rather_than_the_first() {
        let r = registry(vec![
            MockProvider::new("a").with_streams(vec![stream("from-a")]).arc(),
            MockProvider::new("b").with_streams(vec![stream("from-b")]).arc(),
        ]);
        let pinned = r.pinned("b").expect("b is registered");
        let attempt = pinned.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert_eq!(attempt.provider.as_deref(), Some("b"));
        assert_eq!(attempt.value.unwrap()[0].url, "from-b");
    }

    #[tokio::test]
    async fn a_pinned_provider_does_not_fail_over_to_the_others() {
        // The point of pinning: substituting another source for the one the user chose would
        // silently undo the choice, exactly as it would for a picked release.
        let r = registry(vec![
            MockProvider::new("a").with_streams(vec![stream("from-a")]).arc(),
            MockProvider::new("b").failing(ProviderError::Blocked("down".into())).arc(),
        ]);
        let pinned = r.pinned("b").expect("b is registered");
        let attempt = pinned.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert!(attempt.value.is_none(), "must not quietly serve a's stream instead");
        assert_eq!(attempt.failures.len(), 1);
    }

    #[test]
    fn pinning_an_unknown_provider_is_none_rather_than_an_empty_registry() {
        // An empty registry would resolve to "no providers configured", which reads as a
        // misconfiguration. `None` lets the caller fall back to the normal chain instead.
        let r = registry(vec![MockProvider::new("a").arc()]);
        assert!(r.pinned("a").is_some());
        assert!(r.pinned("removed-plugin").is_none());
    }

    #[tokio::test]
    async fn a_hold_is_lifted_once_the_provider_is_available_again() {
        // A hold is a statement about the present, so it has to be re-examined. Filtering held
        // providers out of the chain meant the question was never asked again: one failed VPN
        // check took torrenting out until the process restarted, however healthy the tunnel
        // became afterwards.
        let r =
            registry(vec![MockProvider::new("a").with_streams(vec![stream("from-a")]).arc()]);
        r.health().hold_back("a", "vpn guard failing");

        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;

        assert_eq!(
            attempt.provider.as_deref(),
            Some("a"),
            "a held provider must still be asked whether it is available"
        );
        assert!(
            r.health().get("a").is_none_or(|h| h.held_back.is_none()),
            "the hold outlived the reason for it"
        );
    }

    #[tokio::test]
    async fn a_provider_that_is_still_withheld_is_not_contacted() {
        // The other half: lifting a stale hold must not weaken a live one.
        let provider = MockProvider::new("a").unavailable("vpn guard failing");
        let calls = provider.call_count();
        let r = registry(vec![provider.arc()]);

        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;

        assert!(attempt.value.is_none(), "a withheld provider must not serve");
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0, "it was contacted");
        assert_eq!(
            r.health().get("a").and_then(|h| h.held_back),
            Some("vpn guard failing".to_string())
        );
    }

    #[tokio::test]
    async fn a_pinned_view_shares_health_with_the_registry_it_came_from() {
        // Shared rather than copied, so a failure while pinned is still visible on the
        // Providers screen and to every other title.
        let r = registry(vec![
            MockProvider::new("a").arc(),
            MockProvider::new("b").failing(ProviderError::Blocked("down".into())).arc(),
        ]);
        let pinned = r.pinned("b").expect("b is registered");
        let _ = pinned.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        let recorded = r.health().get("b").expect("b is tracked");
        assert!(
            recorded.consecutive_failures > 0,
            "the failure should be recorded on the shared tracker"
        );
    }

    #[tokio::test]
    async fn a_blocked_provider_fails_over_to_the_next() {
        let r = registry(vec![
            MockProvider::new("a").failing(ProviderError::Blocked("cloudflare".into())).arc(),
            MockProvider::new("b").with_streams(vec![stream("from-b")]).arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert_eq!(attempt.provider.as_deref(), Some("b"));
        assert_eq!(attempt.failures.len(), 1, "the failure is carried, not discarded");
        assert_eq!(attempt.failures[0].0, "a");
    }

    #[tokio::test]
    async fn a_not_found_stops_the_walk_instead_of_burning_the_chain() {
        // The most important behaviour here. A source that answers "I do not have that" has
        // done its job; continuing would spend every remaining provider for the same answer
        // and could mask it behind a different provider's error.
        let b = MockProvider::new("b").with_streams(vec![stream("from-b")]);
        let calls = b.call_count();
        let r = registry(vec![
            MockProvider::new("a").failing(ProviderError::NotFound).arc(),
            b.arc(),
        ]);

        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert!(!attempt.is_success());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0, "b must not be called");
    }

    #[tokio::test]
    async fn every_failure_is_reported_when_the_chain_is_exhausted() {
        let r = registry(vec![
            MockProvider::new("a").failing(ProviderError::Blocked("403".into())).arc(),
            MockProvider::new("b").failing(ProviderError::Parse("no url".into())).arc(),
            MockProvider::new("c").failing(ProviderError::Transport("timeout".into())).arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert!(!attempt.is_success());
        assert_eq!(attempt.failures.len(), 3);

        // The UI needs to name what broke, never show a bare empty list.
        let summary = attempt.summary();
        assert!(summary.contains("a: blocked"));
        assert!(summary.contains("b: parse error"));
        assert!(summary.contains("c: unreachable"));
    }

    #[tokio::test]
    async fn a_timeout_is_named_whatever_it_was_filed_under() {
        // How a plugin reports one: its vocabulary has no transport failure.
        let r = registry(vec![
            MockProvider::new("a")
                .failing(ProviderError::Blocked("operation timed out".into()))
                .arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert_eq!(attempt.summary(), "a: timed out");
    }

    #[tokio::test]
    async fn an_empty_stream_list_is_not_treated_as_success() {
        // Otherwise the walk ends with nothing to play and no explanation.
        let r = registry(vec![
            MockProvider::new("a").with_streams(vec![]).arc(),
            MockProvider::new("b").with_streams(vec![stream("from-b")]).arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        // Empty maps to NotFound, which stops the walk — so this reports failure rather
        // than silently succeeding with nothing.
        assert!(!attempt.is_success());
        assert_eq!(attempt.failures[0].1, ProviderError::NotFound);
    }

    #[tokio::test]
    async fn a_held_back_provider_is_never_contacted() {
        // The VPN guard has to prevent the request, not merely discard its result.
        let torrent = MockProvider::new("torrent")
            .with_streams(vec![stream("leaky")])
            .unavailable("vpn guard failing");
        let calls = torrent.call_count();

        let r = registry(vec![
            torrent.arc(),
            MockProvider::new("remote").with_streams(vec![stream("safe")]).arc(),
        ]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;

        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0, "no request may leave");
        assert_eq!(attempt.provider.as_deref(), Some("remote"));
        assert_eq!(r.health().get("torrent").unwrap().state_label(), "held back");
    }

    #[tokio::test]
    async fn a_downed_provider_is_skipped_on_later_attempts() {
        let a = MockProvider::new("a").failing(ProviderError::Blocked("403".into()));
        let calls = a.call_count();
        let r = registry(vec![
            a.arc(),
            MockProvider::new("b").with_streams(vec![stream("from-b")]).arc(),
        ]);

        // Three failures take it down.
        for i in 0..3 {
            r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, i).await;
        }
        let before = calls.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(before, 3);

        r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 4).await;
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            before,
            "a downed provider should not keep being retried every request"
        );
    }

    #[tokio::test]
    async fn success_records_latency_for_the_providers_screen() {
        let r = registry(vec![MockProvider::new("a").with_streams(vec![stream("x")]).arc()]);
        r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert!(r.health().get("a").unwrap().last_latency.is_some());
    }

    #[tokio::test]
    async fn an_empty_registry_reports_that_rather_than_hanging() {
        let r = registry(vec![]);
        let attempt = r.resolve(&ProviderKey::new("k"), "1", Translation::Sub, 0).await;
        assert!(!attempt.is_success());
        assert_eq!(attempt.summary(), "no providers configured");
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn search_and_episodes_walk_the_chain_the_same_way() {
        let r = registry(vec![
            MockProvider::new("a").failing(ProviderError::Blocked("x".into())).arc(),
            MockProvider::new("b")
                .with_hits(vec![SearchHit::new(ProviderKey::new("k"), "Frieren")])
                .with_episodes(vec![Episode::new(1u32)])
                .arc(),
        ]);
        assert_eq!(
            r.search("frieren", Translation::Sub, 0).await.provider.as_deref(),
            Some("b")
        );
        assert_eq!(
            r.episodes(&ProviderKey::new("k"), Translation::Sub, 0).await.provider.as_deref(),
            Some("b")
        );
    }

    #[tokio::test]
    async fn checking_all_providers_populates_the_screen() {
        let r = registry(vec![
            MockProvider::new("a").with_hits(vec![]).arc(),
            MockProvider::new("b").failing(ProviderError::Transport("down".into())).arc(),
            MockProvider::new("c").unavailable("vpn down").arc(),
        ]);
        r.check_all(0).await;

        let all = r.health().all();
        assert_eq!(all.len(), 3);
        assert_eq!(r.health().get("a").unwrap().state_label(), "ready");
        assert_eq!(r.health().get("b").unwrap().state_label(), "degraded");
        assert_eq!(r.health().get("c").unwrap().state_label(), "held back");
    }
}
