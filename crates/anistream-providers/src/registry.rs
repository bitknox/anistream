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
    traits::Provider,
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
            .map(|(id, e)| format!("{id}: {}", e.state_label()))
            .collect::<Vec<_>>()
            .join(", ")
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
            if let Err(reason) = provider.is_available() {
                self.health.record_failure(&id, &reason, now);
                failures.push((id, reason));
                continue;
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
    pub async fn check_all(&self, now: i64) {
        for provider in &self.snapshot() {
            let id = provider.manifest().id.clone();
            if let Err(reason) = provider.is_available() {
                self.health.record_failure(&id, &reason, now);
                continue;
            }
            let started = Instant::now();
            match provider.health_check().await {
                Ok(()) => self.health.record_success(&id, started.elapsed(), now),
                Err(e) => self.health.record_failure(&id, &e, now),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;

    fn registry(providers: Vec<Arc<dyn Provider>>) -> ProviderRegistry {
        ProviderRegistry::new(providers)
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
