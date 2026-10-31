//! A Jellyfin server as a source: the Provider the engine puts in the chain.
//!
//! It answers from the matches the engine made between the server's series and AniList. It never
//! searches AniList and keeps nothing on disk. Playing is a direct stream of the stored file: mpv
//! plays what Jellyfin keeps, so the server is never asked to transcode.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anistream_core::{
    error::ProviderError,
    ids::{AnilistId, ExternalIds, ProviderKey},
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind, Subtitle},
    traits::{Provider, ProviderKind, ProviderManifest},
};
use async_trait::async_trait;

use crate::{
    client::Jellyfin,
    error::JellyfinError,
    model::{EpisodeItem, Series, TICKS_PER_SECOND},
};

/// The id the chain, `providers.order` and health know a server by.
pub fn source_id(server_id: &str) -> String {
    format!("jellyfin-{server_id}")
}

/// A series on the server and the AniList entry it was matched to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeriesMatch {
    pub series: String,
    pub name: String,
    pub anilist: AnilistId,
    /// The ids the server's metadata carries, which `lookup` also answers by.
    pub ids: ExternalIds,
    /// The season of the series this entry covers, when the series holds several and AniList
    /// splits them. `None` takes every regular season, in order.
    pub season: Option<u32>,
}

/// One server, and what it holds that AniList knows.
pub struct JellyfinSource {
    manifest: ProviderManifest,
    client: Jellyfin,
    matches: Arc<RwLock<Vec<SeriesMatch>>>,
}

impl JellyfinSource {
    pub fn new(
        server_id: &str,
        name: &str,
        client: Jellyfin,
        matches: Vec<SeriesMatch>,
    ) -> Self {
        let host = client.address().host().to_owned();
        Self {
            manifest: ProviderManifest {
                id: source_id(server_id),
                display_name: name.to_owned(),
                version: "1".into(),
                kind: ProviderKind::Server,
                allowed_hosts: vec![host],
                translations: vec![Translation::Sub],
            },
            client,
            matches: Arc::new(RwLock::new(matches)),
        }
    }

    /// Replace the matches after a refresh.
    pub fn set_matches(&self, matches: Vec<SeriesMatch>) {
        *self.matches.write().unwrap_or_else(|e| e.into_inner()) = matches;
    }

    pub fn client(&self) -> &Jellyfin {
        &self.client
    }

    fn by_series(&self, series: &str) -> Option<SeriesMatch> {
        let matches = self.matches.read().unwrap_or_else(|e| e.into_inner());
        matches.iter().find(|m| m.series == series).cloned()
    }

    fn hit(m: &SeriesMatch) -> SearchHit {
        SearchHit::new(m.series.as_str(), m.name.as_str())
    }

    /// The episodes of a matched series that belong to its AniList entry, in order.
    async fn items(&self, m: &SeriesMatch) -> Result<Vec<(u32, EpisodeItem)>, ProviderError> {
        let items = self.client.episodes(&m.series).await.map_err(provider_error)?;
        Ok(entry_episodes(items, m.season))
    }
}

/// The episodes that make up one AniList entry. Specials (season 0) are not part of one. An
/// entry that names a season takes that season alone, numbered as the server numbers it.
/// Otherwise every regular season is counted on, in order, so a series the server keeps as one
/// season of 24 and AniList as 24 episodes agree.
pub fn entry_episodes(items: Vec<EpisodeItem>, season: Option<u32>) -> Vec<(u32, EpisodeItem)> {
    let mut regular: Vec<EpisodeItem> = items
        .into_iter()
        .filter(|item| item.number.is_some() && item.season.unwrap_or(1) > 0)
        .filter(|item| season.is_none_or(|s| item.season.unwrap_or(1) == s))
        .collect();
    regular.sort_by_key(|item| (item.season.unwrap_or(1), item.number.unwrap_or(0)));
    if season.is_some() {
        return regular.into_iter().map(|i| (i.number.unwrap_or(0), i)).collect();
    }
    // Number across seasons only when the seasons would otherwise repeat a number.
    let mut seen = std::collections::BTreeSet::new();
    let repeats = regular.iter().any(|i| !seen.insert(i.number.unwrap_or(0)));
    if !repeats {
        return regular.into_iter().map(|i| (i.number.unwrap_or(0), i)).collect();
    }
    regular.into_iter().enumerate().map(|(n, i)| (n as u32 + 1, i)).collect()
}

fn provider_error(error: JellyfinError) -> ProviderError {
    match error {
        JellyfinError::SignedOut | JellyfinError::BadCredentials => {
            ProviderError::Blocked("the server no longer accepts this sign-in".into())
        }
        JellyfinError::Unreachable { .. } | JellyfinError::UntrustedCertificate { .. } => {
            ProviderError::Transport(error.to_string())
        }
        other => ProviderError::Other(other.to_string()),
    }
}

/// A series' ids as the engine's lookup reads them.
pub fn ids_of(series: &Series) -> ExternalIds {
    use anistream_core::ids::{AnidbId, TmdbId, TvdbId};
    ExternalIds {
        anilist: series.provider_id("AniList").map(AnilistId::new),
        mal: None,
        anidb: series.provider_id("AniDB").map(AnidbId::new),
        tvdb: series.provider_id("Tvdb").map(TvdbId::new),
        tmdb: series.provider_id("Tmdb").map(TmdbId::new),
    }
}

#[async_trait]
impl Provider for JellyfinSource {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let needle = query.trim().to_lowercase();
        let matches = self.matches.read().unwrap_or_else(|e| e.into_inner());
        Ok(matches
            .iter()
            .filter(|m| !needle.is_empty() && m.name.to_lowercase().contains(&needle))
            .map(Self::hit)
            .collect())
    }

    async fn lookup(&self, ids: &ExternalIds) -> Result<Option<SearchHit>, ProviderError> {
        let matches = self.matches.read().unwrap_or_else(|e| e.into_inner());
        Ok(matches
            .iter()
            .find(|m| {
                ids.anilist == Some(m.anilist)
                    || (ids.anidb.is_some() && ids.anidb == m.ids.anidb)
                    || (ids.tvdb.is_some() && ids.tvdb == m.ids.tvdb)
                    || (ids.tmdb.is_some() && ids.tmdb == m.ids.tmdb)
            })
            .map(Self::hit))
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        let m = self.by_series(key.as_str()).ok_or(ProviderError::NotFound)?;
        Ok(self
            .items(&m)
            .await?
            .into_iter()
            .map(|(number, item)| {
                let mut episode = Episode::new(number);
                episode.title = Some(item.name).filter(|name| !name.is_empty());
                episode.duration = item
                    .run_time_ticks
                    .filter(|t| *t > 0)
                    .map(|t| Duration::from_secs((t / TICKS_PER_SECOND) as u64));
                episode
            })
            .collect())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        _translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        let m = self.by_series(key.as_str()).ok_or(ProviderError::NotFound)?;
        let wanted: u32 = episode.trim().parse().map_err(|_| ProviderError::NotFound)?;
        let item = self
            .items(&m)
            .await?
            .into_iter()
            .find(|(number, _)| *number == wanted)
            .map(|(_, item)| item)
            .ok_or(ProviderError::NotFound)?;
        let info = self.client.playback_info(&item.id).await.map_err(provider_error)?;
        let source = info.media_sources.first().ok_or(ProviderError::NotFound)?;
        let mut stream =
            Stream::new(self.client.direct_stream_url(&item.id, &source.id), StreamKind::Mp4);
        stream.provider_id = self.manifest.id.clone();
        stream.headers = self.client.player_headers();
        stream.subtitles = source
            .media_streams
            .iter()
            .filter(|s| s.kind == "Subtitle" && s.is_external)
            .filter_map(|s| {
                let url = self.client.subtitle_url(s.delivery_url.as_deref()?);
                Some(Subtitle {
                    language: s.language.clone().unwrap_or_default(),
                    url,
                    hard: false,
                    format: s.codec.clone(),
                })
            })
            .collect();
        Ok(vec![stream])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(season: u32, number: u32) -> EpisodeItem {
        EpisodeItem {
            id: format!("e{season}x{number}"),
            name: String::new(),
            season: Some(season),
            number: Some(number),
            run_time_ticks: None,
            user_data: None,
        }
    }

    fn numbers(items: Vec<(u32, EpisodeItem)>) -> Vec<u32> {
        items.into_iter().map(|(n, _)| n).collect()
    }

    #[test]
    fn specials_are_not_part_of_an_entry() {
        let items = vec![item(0, 1), item(1, 1), item(1, 2)];
        assert_eq!(numbers(entry_episodes(items, None)), [1, 2]);
    }

    #[test]
    fn seasons_that_repeat_numbers_are_counted_on() {
        let items = vec![item(1, 1), item(1, 2), item(2, 1), item(2, 2)];
        assert_eq!(numbers(entry_episodes(items, None)), [1, 2, 3, 4]);
    }

    #[test]
    fn an_entry_that_names_a_season_takes_that_season_alone() {
        let items = vec![item(1, 1), item(1, 2), item(2, 1), item(2, 2), item(2, 3)];
        assert_eq!(numbers(entry_episodes(items, Some(2))), [1, 2, 3]);
    }

    #[test]
    fn the_source_id_is_stable_and_safe_in_a_file_name() {
        assert_eq!(source_id("ab12"), "jellyfin-ab12");
    }
}
