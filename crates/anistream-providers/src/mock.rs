//! A configurable provider for tests.
//!
//! Exists so the registry's failover semantics — which are the part most likely to be got
//! wrong — can be exercised deterministically, with no network. It also counts calls, which
//! is how "a held-back provider is never contacted" is actually proven rather than assumed.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::Stream,
    traits::{Provider, ProviderKind, ProviderManifest, SourceCandidate},
};
use async_trait::async_trait;

pub struct MockProvider {
    manifest: ProviderManifest,
    hits: Vec<SearchHit>,
    episodes: Vec<Episode>,
    streams: Vec<Stream>,
    /// Answers for one key, in place of the shared lists above. A key names one title, so a
    /// mock standing in for a catalogue of several has to tell them apart by it.
    episodes_for: HashMap<ProviderKey, Vec<Episode>>,
    /// What a listing in the dub answers for one key, where it differs from the sub's.
    dub_for: HashMap<ProviderKey, Vec<Episode>>,
    /// A listing in the dub of a key `dub_for` does not name finds nothing, rather than the
    /// sub's list.
    no_dub: bool,
    streams_for: HashMap<ProviderKey, Vec<Stream>>,
    sources: Vec<SourceCandidate>,
    /// What each release in `sources` resolves to, by its id, for a pick by hand.
    source_streams: HashMap<String, Vec<Stream>>,
    error: Option<ProviderError>,
    unavailable: Option<String>,
    calls: Arc<AtomicUsize>,
    /// How long each call takes to answer, so tests can tell asking at once from asking in turn.
    latency: Duration,
}

impl MockProvider {
    pub fn new(id: &str) -> Self {
        Self {
            manifest: ProviderManifest {
                id: id.to_owned(),
                display_name: id.to_owned(),
                version: "0".into(),
                kind: ProviderKind::Native,
                allowed_hosts: Vec::new(),
                translations: vec![Translation::Sub],
            },
            hits: Vec::new(),
            episodes: Vec::new(),
            streams: Vec::new(),
            episodes_for: HashMap::new(),
            dub_for: HashMap::new(),
            no_dub: false,
            streams_for: HashMap::new(),
            sources: Vec::new(),
            source_streams: HashMap::new(),
            error: None,
            unavailable: None,
            calls: Arc::new(AtomicUsize::new(0)),
            latency: Duration::ZERO,
        }
    }

    pub fn with_hits(mut self, hits: Vec<SearchHit>) -> Self {
        self.hits = hits;
        self
    }

    pub fn with_episodes(mut self, episodes: Vec<Episode>) -> Self {
        self.episodes = episodes;
        self
    }

    pub fn with_streams(mut self, streams: Vec<Stream>) -> Self {
        self.streams = streams;
        self
    }

    /// Answer `episodes` for this key with these; every other key keeps the shared list.
    pub fn with_episodes_for(
        mut self,
        key: impl Into<ProviderKey>,
        episodes: Vec<Episode>,
    ) -> Self {
        self.episodes_for.insert(key.into(), episodes);
        self
    }

    /// Answer a listing of this key in the dub with these: a dub that stops short of the sub.
    /// Every other key's dub is its sub's list, unless [`Self::without_dub`].
    pub fn with_dub_for(mut self, key: impl Into<ProviderKey>, episodes: Vec<Episode>) -> Self {
        self.dub_for.insert(key.into(), episodes);
        self
    }

    /// Find nothing in the dub for a key [`Self::with_dub_for`] does not name.
    pub fn without_dub(mut self) -> Self {
        self.no_dub = true;
        self
    }

    /// Answer `resolve` for this key with these, whatever the episode; every other key keeps
    /// the shared list.
    pub fn with_streams_for(
        mut self,
        key: impl Into<ProviderKey>,
        streams: Vec<Stream>,
    ) -> Self {
        self.streams_for.insert(key.into(), streams);
        self
    }

    /// Fail every call with this error.
    pub fn with_sources(mut self, sources: Vec<SourceCandidate>) -> Self {
        self.sources = sources;
        self
    }

    /// Answer `resolve_source` for the release `source_id` with these, whatever the key and the
    /// episode. A release not named here is refused, as the trait's default refuses every one.
    pub fn with_source_streams(mut self, source_id: &str, streams: Vec<Stream>) -> Self {
        self.source_streams.insert(source_id.to_owned(), streams);
        self
    }

    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }

    pub fn failing(mut self, error: ProviderError) -> Self {
        self.error = Some(error);
        self
    }

    /// Report as withheld by local policy, e.g. a failing VPN guard.
    pub fn unavailable(mut self, reason: &str) -> Self {
        self.unavailable = Some(reason.to_owned());
        self
    }

    pub fn kind(mut self, kind: ProviderKind) -> Self {
        self.manifest.kind = kind;
        self
    }

    /// Handle to the call counter, taken before the provider is boxed.
    pub fn call_count(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.calls)
    }

    pub fn arc(self) -> Arc<dyn Provider> {
        Arc::new(self)
    }

    async fn record(&self) -> Result<(), ProviderError> {
        if !self.latency.is_zero() {
            tokio::time::sleep(self.latency).await;
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        _query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        self.record().await?;
        Ok(self.hits.clone())
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.record().await?;
        if translation == Translation::Dub {
            if let Some(dub) = self.dub_for.get(key) {
                return Ok(dub.clone());
            }
            if self.no_dub {
                return Err(ProviderError::NotFound);
            }
        }
        Ok(self.episodes_for.get(key).unwrap_or(&self.episodes).clone())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.record().await?;
        Ok(self.streams_for.get(key).unwrap_or(&self.streams).clone())
    }

    async fn sources(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        self.record().await?;
        Ok(self.sources.clone())
    }

    async fn resolve_source(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
        source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.record().await?;
        self.source_streams.get(source_id).cloned().ok_or(ProviderError::NotFound)
    }

    fn is_available(&self) -> Result<(), ProviderError> {
        match &self.unavailable {
            Some(reason) => Err(ProviderError::Unavailable(reason.clone())),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_mock_returns_what_it_was_given_and_counts_calls() {
        let p = MockProvider::new("m")
            .with_streams(vec![Stream::new("x", anistream_core::stream::StreamKind::Hls)]);
        let calls = p.call_count();

        let streams = p.resolve(&ProviderKey::new("k"), "1", Translation::Sub).await.unwrap();
        assert_eq!(streams.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn answers_named_for_a_key_are_that_keys_alone() {
        let p = MockProvider::new("m")
            .with_episodes(vec![Episode::new(1)])
            .with_episodes_for("two", vec![Episode::new(1), Episode::new(2)])
            .with_streams_for(
                "two",
                vec![Stream::new("y", anistream_core::stream::StreamKind::Hls)],
            );

        let two = ProviderKey::new("two");
        assert_eq!(p.episodes(&two, Translation::Sub).await.unwrap().len(), 2);
        assert_eq!(p.resolve(&two, "2", Translation::Sub).await.unwrap()[0].url, "y");

        let other = ProviderKey::new("other");
        assert_eq!(p.episodes(&other, Translation::Sub).await.unwrap().len(), 1, "shared list");
        assert!(p.resolve(&other, "1", Translation::Sub).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failing_mock_reports_its_error_and_still_counts_the_call() {
        let p = MockProvider::new("m").failing(ProviderError::Blocked("x".into()));
        let calls = p.call_count();
        assert!(p.search("q", Translation::Sub).await.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn an_unavailable_mock_reports_why() {
        let p = MockProvider::new("m").unavailable("vpn down");
        assert_eq!(p.is_available(), Err(ProviderError::Unavailable("vpn down".into())));
        assert!(MockProvider::new("m").is_available().is_ok());
    }
}
