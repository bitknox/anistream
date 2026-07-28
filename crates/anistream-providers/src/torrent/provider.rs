//! `TorrentProvider`: streaming over BitTorrent, behind the VPN guard.
//!
//! Fits the existing [`Provider`] trait with no special-casing, which was the point of
//! choosing librqbit — it returns a plain loopback HTTP URL that mpv seeks like any other
//! stream, so the player layer needs to know nothing about torrents.
//!
//! Two structural differences from a web source are worth naming:
//!
//! - **There is no stable show id.** Torrent sites have no catalogue. The provider key is
//!   therefore the *title* itself, and every call re-searches. That is why the resolution
//!   ladder treats provider search as the primary path rather than a fallback.
//! - **Selection can be curated first.** When a curation endpoint is configured, the pick it
//!   names is preferred; raw indexer ranking is the fallback, and the only path otherwise.
//!
//! Both endpoints come from the user's config. anistream ships neither, and this provider
//! is never constructed without an indexer URL.

use std::sync::Arc;

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind},
    traits::{Provider, ProviderKind, ProviderManifest, SourceCandidate},
};
use anistream_net::HttpClient;
use async_trait::async_trait;

use crate::{
    torrent::{
        curation,
        indexer::{self, IndexerItem},
        session::TorrentSession,
    },
    vpn::VpnGuard,
};

/// The endpoints the user supplied. anistream has no opinion about what they point at.
#[derive(Debug, Clone)]
pub struct IndexerSettings {
    /// RSS search template containing `{query}`.
    pub rss_url: String,
    /// Trackers added to every magnet.
    pub trackers: Vec<String>,
    /// Optional curation template containing `{anilist_id}`.
    pub curation_url: Option<String>,
}

impl IndexerSettings {
    /// Build from config, or `None` when no indexer is configured.
    pub fn from_config(config: &anistream_core::config::TorrentConfig) -> Option<Self> {
        let rss_url = config.rss_url.as_deref().map(str::trim).filter(|u| !u.is_empty())?;
        Some(Self {
            rss_url: rss_url.to_owned(),
            trackers: config.trackers.clone(),
            curation_url: config
                .curation_url
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
                .map(str::to_owned),
        })
    }

    /// Hosts this provider will contact, for the manifest.
    fn hosts(&self) -> Vec<String> {
        let mut hosts: Vec<String> =
            [Some(self.rss_url.as_str()), self.curation_url.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(host_of)
                .collect();
        hosts.sort();
        hosts.dedup();
        hosts
    }
}

/// Host of a URL, lowercased.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

pub struct TorrentProvider {
    manifest: ProviderManifest,
    http: HttpClient,
    guard: VpnGuard,
    session: Arc<TorrentSession>,
    quality: u32,
    settings: IndexerSettings,
}

impl TorrentProvider {
    pub fn new(
        http: HttpClient,
        guard: VpnGuard,
        session: Arc<TorrentSession>,
        quality: u32,
        settings: IndexerSettings,
    ) -> Self {
        let hosts = settings.hosts();
        Self {
            manifest: ProviderManifest {
                id: "torrent".into(),
                // Named for what it is, not for wherever it was pointed.
                display_name: "Torrent (external indexer)".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                kind: ProviderKind::Torrent,
                allowed_hosts: hosts,
                translations: vec![Translation::Sub, Translation::Dub],
            },
            http,
            guard,
            session,
            quality,
            settings,
        }
    }

    /// Fetch and parse a search against the configured indexer.
    async fn feed(&self, query: &str) -> Result<Vec<IndexerItem>, ProviderError> {
        let url = indexer::search_url(&self.settings.rss_url, query, Some(self.quality));
        let response = self
            .http
            .plain()
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ProviderError::Blocked(format!(
                "indexer returned HTTP {}",
                response.status()
            )));
        }
        let body = response.text().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
        Ok(indexer::parse_feed(&body))
    }

    /// Ask the configured curation endpoint for a pick, keyed on the AniList id.
    ///
    /// A miss is completely normal — curation covers a subset, and the endpoint is optional
    /// — so this returns `None` rather than an error and the caller falls back to ranking.
    async fn curated(&self, anilist_id: u32, prefer_dual: bool) -> Option<String> {
        let template = self.settings.curation_url.as_deref()?;
        let response = self
            .http
            .plain()
            .get(curation::query_url(template, anilist_id))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let body = response.text().await.ok()?;
        let releases = curation::parse(&body, host_of(&self.settings.rss_url).as_deref());
        let pick = curation::best(&releases, prefer_dual)?;
        tracing::info!(group = %pick.group, url = %pick.url, "using curated release");
        pick.view_id().map(str::to_owned)
    }

    /// The item automatic resolution would choose: the curated pick when it is present
    /// in the feed and covers the episode, otherwise the top-ranked release.
    async fn auto_choice<'a>(
        &self,
        items: &'a [IndexerItem],
        anilist_id: Option<u32>,
        wanted: u32,
        prefer_dual: bool,
    ) -> Option<&'a IndexerItem> {
        let curated_id = match anilist_id {
            Some(id) => self.curated(id, prefer_dual).await,
            None => None,
        };
        match &curated_id {
            Some(id) => items
                .iter()
                .find(|item| {
                    item.guid.contains(id.as_str()) && item.release.covers(wanted, None)
                })
                .or_else(|| indexer::best(items, wanted, None, self.quality, prefer_dual)),
            None => indexer::best(items, wanted, None, self.quality, prefer_dual),
        }
    }

    /// Start a torrent session for a chosen item and wrap it as a playable stream.
    async fn stream_from(
        &self,
        chosen: &IndexerItem,
        wanted: u32,
    ) -> Result<Vec<Stream>, ProviderError> {
        let magnet = chosen
            .magnet(&self.settings.trackers)
            .ok_or_else(|| ProviderError::Parse("release has no info hash".into()))?;

        tracing::info!(
            title = %chosen.title,
            seeders = chosen.seeders,
            "starting torrent stream"
        );

        let active = self.session.stream(&magnet, Some(wanted)).await?;

        Ok(vec![Stream {
            quality: chosen.release.quality,
            provider_id: self.manifest.id.clone(),
            // The magnet, not the loopback URL: the download queue persists this and resumes from
            // it, and the loopback address stops existing the moment the session does.
            download_source: Some(magnet.clone()),
            ..Stream::new(active.url(), StreamKind::TorrentHttp)
        }])
    }
}

/// Split a provider key into its title and optional AniList id.
///
/// The key carries both because torrents have no catalogue id of their own, and curation needs
/// the AniList id to be useful. Encoded as `title\u{1}id` so a title containing any ordinary
/// punctuation still round-trips.
pub fn encode_key(title: &str, anilist_id: Option<u32>) -> ProviderKey {
    match anilist_id {
        Some(id) => ProviderKey::new(format!("{title}\u{1}{id}")),
        None => ProviderKey::new(title),
    }
}

pub fn decode_key(key: &ProviderKey) -> (String, Option<u32>) {
    match key.as_str().split_once('\u{1}') {
        Some((title, id)) => (title.to_owned(), id.parse().ok()),
        None => (key.as_str().to_owned(), None),
    }
}

#[async_trait]
impl Provider for TorrentProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    /// Whether the source may run at all.
    ///
    /// Delegates to the VPN guard, which reports [`ProviderError::Unavailable`] — not a
    /// health error. A source held back by local policy is not a broken source, and the
    /// registry skips it without ever contacting it.
    fn is_available(&self) -> Result<(), ProviderError> {
        self.guard.permit()
    }

    /// Torrent sites have no catalogue, so a "search hit" is a distinct show name seen in
    /// the feed rather than a catalogue entry.
    async fn search(
        &self,
        query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        self.is_available()?;
        let items = self.feed(query).await?;
        if items.is_empty() {
            return Err(ProviderError::NotFound);
        }

        // Collapse releases to the distinct show names present, keeping the highest episode
        // seen for each as an episode-count hint the matcher can score against.
        let mut by_title: std::collections::BTreeMap<String, u32> =
            std::collections::BTreeMap::new();
        for item in &items {
            let title = if item.release.title.is_empty() {
                item.title.clone()
            } else {
                item.release.title.clone()
            };
            let highest =
                item.release.episode.or(item.release.batch.map(|(_, to)| to)).unwrap_or(0);
            let entry = by_title.entry(title).or_insert(0);
            *entry = (*entry).max(highest);
        }

        Ok(by_title
            .into_iter()
            .map(|(title, episodes)| SearchHit {
                episode_count: (episodes > 0).then_some(episodes),
                ..SearchHit::new(encode_key(&title, None), title)
            })
            .collect())
    }

    /// Episodes are whatever the feed can actually supply.
    ///
    /// Derived from releases rather than a catalogue: a batch contributes its whole range, a
    /// single release contributes one episode. That means the list reflects what is
    /// *obtainable*, which is more useful here than what officially exists.
    async fn episodes(
        &self,
        key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.is_available()?;
        let (title, _) = decode_key(key);
        let items = self.feed(&title).await?;

        let mut numbers: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for item in &items {
            if let Some(episode) = item.release.episode {
                numbers.insert(episode);
            }
            if let Some((from, to)) = item.release.batch {
                // Guard against an implausible range claiming thousands of episodes.
                if to.saturating_sub(from) < 200 {
                    numbers.extend(from..=to);
                }
            }
        }

        if numbers.is_empty() {
            return Err(ProviderError::NotFound);
        }
        Ok(numbers.into_iter().map(Episode::new).collect())
    }

    /// Resolve an episode to a playable loopback URL.
    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.is_available()?;
        let (title, anilist_id) = decode_key(key);
        let wanted: u32 = episode.trim().parse().map_err(|_| {
            ProviderError::Parse(format!("episode {episode:?} is not a number"))
        })?;
        let prefer_dual = translation == Translation::Dub;

        let items = self.feed(&title).await?;
        let chosen = self
            .auto_choice(&items, anilist_id, wanted, prefer_dual)
            .await
            .ok_or(ProviderError::NotFound)?;

        self.stream_from(chosen, wanted).await
    }

    /// The ranked slate for an episode, with the automatic pick marked.
    async fn sources(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        self.is_available()?;
        let (title, anilist_id) = decode_key(key);
        let wanted: u32 = episode.trim().parse().map_err(|_| {
            ProviderError::Parse(format!("episode {episode:?} is not a number"))
        })?;
        let prefer_dual = translation == Translation::Dub;

        let items = self.feed(&title).await?;
        let auto = self
            .auto_choice(&items, anilist_id, wanted, prefer_dual)
            .await
            .map(|item| item.guid.clone());

        Ok(indexer::ranked(&items, wanted, None, self.quality, prefer_dual)
            .into_iter()
            .map(|item| SourceCandidate {
                id: item.guid.clone(),
                provider_id: self.manifest.id.clone(),
                title: item.title.clone(),
                quality: item.release.quality,
                seeders: Some(item.seeders),
                size: item.size.clone(),
                dual_audio: item.release.dual_audio,
                dubbed: item.release.dubbed,
                auto_pick: auto.as_deref() == Some(item.guid.as_str()),
            })
            .collect())
    }

    /// Resolve the exact release the user picked, by feed guid. No ranking, no curation
    /// — the pick *is* the decision.
    async fn resolve_source(
        &self,
        key: &ProviderKey,
        episode: &str,
        _translation: Translation,
        source_id: &str,
    ) -> Result<Vec<Stream>, ProviderError> {
        self.is_available()?;
        let (title, _) = decode_key(key);
        let wanted: u32 = episode.trim().parse().map_err(|_| {
            ProviderError::Parse(format!("episode {episode:?} is not a number"))
        })?;

        let items = self.feed(&title).await?;
        let chosen =
            items.iter().find(|item| item.guid == source_id).ok_or(ProviderError::NotFound)?;

        self.stream_from(chosen, wanted).await
    }

    /// Cheap liveness probe that does not start a torrent.
    async fn health_check(&self) -> Result<(), ProviderError> {
        self.is_available()?;
        self.feed("a").await.map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_round_trips_with_and_without_an_anilist_id() {
        let with = encode_key("Sousou no Frieren", Some(154_587));
        assert_eq!(decode_key(&with), ("Sousou no Frieren".into(), Some(154_587)));

        let without = encode_key("Sousou no Frieren", None);
        assert_eq!(decode_key(&without), ("Sousou no Frieren".into(), None));
    }

    #[test]
    fn a_title_containing_punctuation_survives_the_round_trip() {
        // The separator is a control character precisely so ordinary punctuation is safe.
        for title in [
            "Frieren: Beyond Journey's End",
            "Re:Zero - Starting Life in Another World",
            "K-On!",
            "86",
        ] {
            let key = encode_key(title, Some(1));
            assert_eq!(decode_key(&key), (title.to_owned(), Some(1)));
        }
    }

    #[test]
    fn a_key_from_an_older_cache_without_a_separator_still_decodes() {
        // Cached resolutions persist across upgrades, so a bare title must remain valid.
        let legacy = ProviderKey::new("Sousou no Frieren");
        assert_eq!(decode_key(&legacy), ("Sousou no Frieren".into(), None));
    }

    #[test]
    fn a_malformed_id_degrades_to_no_id_rather_than_failing() {
        let key = ProviderKey::new("Title\u{1}not-a-number");
        assert_eq!(decode_key(&key), ("Title".into(), None));
    }

    #[test]
    fn the_manifest_declares_the_configured_hosts_and_nothing_else() {
        let settings = IndexerSettings {
            rss_url: "https://indexer.example/?q={query}".into(),
            trackers: vec!["udp://tracker.example:1337/announce".into()],
            curation_url: Some("https://curate.example/api?alID={anilist_id}".into()),
        };
        assert_eq!(settings.hosts(), vec!["curate.example", "indexer.example"]);

        // With no curation endpoint, only the indexer is ever contacted.
        let bare = IndexerSettings { curation_url: None, ..settings };
        assert_eq!(bare.hosts(), vec!["indexer.example"]);
    }

    #[test]
    fn an_unconfigured_indexer_yields_no_settings() {
        use anistream_core::config::TorrentConfig;

        // The source cannot be built without an endpoint: anistream ships none.
        assert!(IndexerSettings::from_config(&TorrentConfig::default()).is_none());
        assert!(
            IndexerSettings::from_config(&TorrentConfig {
                rss_url: Some("   ".into()),
                ..Default::default()
            })
            .is_none(),
            "whitespace is not a configuration"
        );

        let configured = IndexerSettings::from_config(&TorrentConfig {
            rss_url: Some("https://indexer.example/?q={query}".into()),
            ..Default::default()
        });
        assert!(configured.is_some());
    }
}
