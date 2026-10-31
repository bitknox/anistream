//! Torrent playback for the sources that name torrents.
//!
//! anistream finds no torrents. A source plugin that declares the `torrents` capability names
//! them, as [`StreamKind::Torrent`] streams carrying a magnet link, and [`TorrentBridge`] plays
//! them through the engine's session: behind the protection check, with the user's trackers added,
//! and served over loopback like any other torrent. The source's own order is kept and nothing is
//! ranked here.
//!
//! The session comes and goes with the settings and the check, so the bridge holds a
//! [`TorrentPlayback`] that the engine fills in once the check passes and empties when it lets
//! the session go. While it is empty, or the check fails, or the network is one torrents may not
//! run on, the source reports itself [`ProviderError::Unavailable`], as the built-in torrent source
//! did: policy, not a failure, so the registry passes it over without contacting it and nothing
//! counts against its health. It is asked again each time, so it comes back with the session.

use std::sync::{Arc, RwLock};

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind},
    traits::{Provider, ProviderManifest, SourceCandidate},
};
use async_trait::async_trait;

use crate::torrent::session::TorrentSession;

/// What [`ProviderError::Unavailable`] says while torrent playback is off or waiting for the
/// check. The engine words it for the person; this is for the log.
pub const PLAYBACK_OFF: &str = "torrent playback is off";

/// The engine's torrent session as sources see it: there while torrent playback is on and the
/// protection check has passed, and gone otherwise.
#[derive(Clone, Default)]
pub struct TorrentPlayback {
    inner: Arc<RwLock<Playback>>,
}

#[derive(Default)]
struct Playback {
    session: Option<Arc<TorrentSession>>,
    /// Added to every torrent played, from `providers.torrent.trackers`.
    trackers: Vec<String>,
}

impl std::fmt::Debug for TorrentPlayback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorrentPlayback").field("up", &self.session().is_some()).finish()
    }
}

impl TorrentPlayback {
    /// Hand sources the session that has just come up, or take it away.
    pub fn set(&self, session: Option<Arc<TorrentSession>>, trackers: Vec<String>) {
        let mut playback = self.inner.write().unwrap_or_else(|e| e.into_inner());
        playback.session = session;
        playback.trackers = trackers;
    }

    pub fn session(&self) -> Option<Arc<TorrentSession>> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).session.clone()
    }

    fn trackers(&self) -> Vec<String> {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).trackers.clone()
    }

    /// `link` with the user's trackers it lacks, as the session is to add it.
    pub fn with_trackers(&self, link: &str) -> String {
        with_trackers(link, &self.trackers())
    }

    /// Start one torrent a source named and wrap it as the stream the player gets.
    ///
    /// The magnet, not the loopback URL, is what a download keeps: the loopback address stops
    /// existing the moment the session does.
    pub async fn start(&self, named: &Stream, episode: &str) -> Result<Stream, ProviderError> {
        let session =
            self.session().ok_or_else(|| ProviderError::Unavailable(PLAYBACK_OFF.into()))?;
        let magnet = self.with_trackers(&named.url);
        let active = session.stream(&magnet, episode.trim().parse().ok()).await?;
        tracing::info!(provider = %named.provider_id, file = %active.file.name, "starting torrent");
        Ok(Stream {
            url: active.url().to_owned(),
            kind: StreamKind::TorrentHttp,
            download_source: Some(magnet),
            // Never `url` for thumbnails: every read of that is a stream librqbit fetches ahead
            // for, and a thumbnailer seeking around would starve playback.
            thumbnail_url: active.thumbnail_url().map(str::to_owned),
            ..named.clone()
        })
    }
}

/// Add each tracker `link` does not already carry.
fn with_trackers(link: &str, trackers: &[String]) -> String {
    let mut link = link.to_owned();
    for tracker in trackers.iter().map(|t| t.trim()).filter(|t| !t.is_empty()) {
        let encoded = urlencode(tracker);
        if !link.contains(&format!("tr={encoded}")) && !link.contains(&format!("tr={tracker}"))
        {
            link.push_str("&tr=");
            link.push_str(&encoded);
        }
    }
    link
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// A source with torrent playback lent: its torrents start through the engine's session, and
/// everything else it answers passes through as it was.
pub struct TorrentBridge {
    inner: Arc<dyn Provider>,
    playback: TorrentPlayback,
}

impl TorrentBridge {
    pub fn new(inner: Arc<dyn Provider>, playback: TorrentPlayback) -> Self {
        Self { inner, playback }
    }

    /// Lend `playback` to `source` when it names torrents; a source that does not is returned as
    /// it is.
    pub fn lend(source: Arc<dyn Provider>, playback: &TorrentPlayback) -> Arc<dyn Provider> {
        if source.manifest().torrents {
            Arc::new(Self::new(source, playback.clone()))
        } else {
            source
        }
    }

    /// The streams with the source's first torrent that starts in place of the torrents it named.
    ///
    /// One torrent, not every one: each is a swarm joined. The rest are tried in the source's
    /// order only when the one before fails to start. Streams that are not torrents keep their
    /// places.
    async fn started(
        &self,
        streams: Vec<Stream>,
        episode: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        if !streams.iter().any(|s| s.kind == StreamKind::Torrent) {
            return Ok(streams);
        }
        let mut out = Vec::with_capacity(streams.len());
        let mut started = false;
        let mut failure = None;
        for stream in streams {
            if stream.kind != StreamKind::Torrent {
                out.push(stream);
                continue;
            }
            if started {
                continue;
            }
            match self.playback.start(&stream, episode).await {
                Ok(playing) => {
                    started = true;
                    out.push(playing);
                }
                Err(error) => {
                    tracing::info!(provider = %stream.provider_id, %error, "torrent did not start");
                    failure = Some(error);
                }
            }
        }
        match (out.is_empty(), failure) {
            (true, Some(error)) => Err(error),
            _ => Ok(out),
        }
    }
}

#[async_trait]
impl Provider for TorrentBridge {
    fn manifest(&self) -> &ProviderManifest {
        self.inner.manifest()
    }

    async fn search(
        &self,
        query: &str,
        translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        self.inner.search(query, translation).await
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.inner.episodes(key, translation).await
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        let streams = self.inner.resolve(key, episode, translation).await?;
        self.started(streams, episode).await
    }

    async fn sources(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        self.inner.sources(key, episode, translation).await
    }

    async fn resolve_source(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
        source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        let streams = self.inner.resolve_source(key, episode, translation, source_id).await?;
        self.started(streams, episode).await
    }

    async fn health_check(&self) -> Result<(), ProviderError> {
        self.inner.health_check().await
    }

    fn is_available(&self) -> Result<(), ProviderError> {
        self.inner.is_available()?;
        let session = self
            .playback
            .session()
            .ok_or_else(|| ProviderError::Unavailable(PLAYBACK_OFF.into()))?;
        session.guard().permit()?;
        session.network_permit()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;

    const LINK: &str = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567";

    fn torrent(link: &str) -> Stream {
        Stream { provider_id: "index".into(), ..Stream::new(link, StreamKind::Torrent) }
    }

    fn bridged(streams: Vec<Stream>) -> TorrentBridge {
        let source = MockProvider::new("index").torrents().with_streams(streams).arc();
        TorrentBridge::new(source, TorrentPlayback::default())
    }

    #[test]
    fn the_users_trackers_are_added_once() {
        let trackers = vec!["udp://tracker.example:1337/announce".to_owned(), " ".into()];
        let once = with_trackers(LINK, &trackers);
        assert_eq!(once, format!("{LINK}&tr=udp%3A%2F%2Ftracker.example%3A1337%2Fannounce"));
        assert_eq!(with_trackers(&once, &trackers), once, "not twice");
        assert_eq!(with_trackers(LINK, &[]), LINK);
    }

    #[test]
    fn with_playback_off_a_source_that_names_torrents_is_held_back_as_policy() {
        let err = bridged(vec![torrent(LINK)]).is_available().unwrap_err();
        assert_eq!(err, ProviderError::Unavailable(PLAYBACK_OFF.into()));
        assert!(!err.counts_against_health(), "policy is not a broken source");
    }

    #[tokio::test]
    async fn a_torrent_never_reaches_the_player_unstarted() {
        // The session went as the call was under way: the torrent is not handed on as it was.
        let streams = bridged(vec![torrent(LINK)])
            .resolve(&ProviderKey::new("k"), "1", Translation::Sub)
            .await;
        assert_eq!(streams, Err(ProviderError::Unavailable(PLAYBACK_OFF.into())));

        let web = Stream {
            provider_id: "index".into(),
            ..Stream::new("https://cdn.example/1.m3u8", StreamKind::Hls)
        };
        let streams = bridged(vec![torrent(LINK), web.clone()])
            .resolve(&ProviderKey::new("k"), "1", Translation::Sub)
            .await
            .unwrap();
        assert_eq!(streams, vec![web], "what is not a torrent keeps its place");
    }

    #[test]
    fn only_a_source_that_names_torrents_is_lent_playback() {
        let playback = TorrentPlayback::default();
        let plain: Arc<dyn Provider> = MockProvider::new("plain").arc();
        let lent = TorrentBridge::lend(Arc::clone(&plain), &playback);
        assert!(Arc::ptr_eq(&plain, &lent));
        let named: Arc<dyn Provider> = MockProvider::new("index").torrents().arc();
        let lent = TorrentBridge::lend(Arc::clone(&named), &playback);
        assert!(!Arc::ptr_eq(&named, &lent));
        assert_eq!(lent.manifest().id, "index");
    }
}
