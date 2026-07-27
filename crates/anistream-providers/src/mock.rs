//! A configurable provider for tests.
//!
//! Exists so the registry's failover semantics — which are the part most likely to be got
//! wrong — can be exercised deterministically, with no network. It also counts calls, which
//! is how "a held-back provider is never contacted" is actually proven rather than assumed.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::Stream,
    traits::{Provider, ProviderKind, ProviderManifest},
};
use async_trait::async_trait;

pub struct MockProvider {
    manifest: ProviderManifest,
    hits: Vec<SearchHit>,
    episodes: Vec<Episode>,
    streams: Vec<Stream>,
    error: Option<ProviderError>,
    unavailable: Option<String>,
    calls: Arc<AtomicUsize>,
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
            error: None,
            unavailable: None,
            calls: Arc::new(AtomicUsize::new(0)),
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

    /// Fail every call with this error.
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

    fn record(&self) -> Result<(), ProviderError> {
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
        self.record()?;
        Ok(self.hits.clone())
    }

    async fn episodes(
        &self,
        _key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.record()?;
        Ok(self.episodes.clone())
    }

    async fn resolve(
        &self,
        _key: &ProviderKey,
        _episode: &str,
        _translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.record()?;
        Ok(self.streams.clone())
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
