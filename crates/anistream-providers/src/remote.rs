//! A provider backed by a self-hosted HTTP API.
//!
//! The escape hatch. When a native source dies, pointing this at a Consumet-shaped service
//! you run yourself restores playback with a config edit rather than a release — which is
//! the whole reason the `Provider` trait exists.
//!
//! Deliberately tolerant about response shape. Every one of these services names its fields
//! slightly differently, and a provider that refuses to parse a working API because it said
//! `episodeId` instead of `id` would defeat the point of being an escape hatch.

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, MediaFormat, SearchHit, Translation},
    stream::{Stream, StreamKind},
    traits::{Provider, ProviderKind, ProviderManifest},
};
use anistream_net::HttpClient;
use async_trait::async_trait;
use serde_json::Value;

pub struct RemoteHttpProvider {
    manifest: ProviderManifest,
    base_url: String,
    http: HttpClient,
}

impl RemoteHttpProvider {
    pub fn new(base_url: impl Into<String>, http: HttpClient) -> Self {
        let base_url = base_url.into().trim_end_matches('/').to_owned();
        let host = base_url
            .split("://")
            .nth(1)
            .and_then(|r| r.split('/').next())
            .unwrap_or_default()
            .to_owned();

        Self {
            manifest: ProviderManifest {
                id: "remote".into(),
                display_name: "Self-hosted API".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                kind: ProviderKind::Remote,
                allowed_hosts: vec![host],
                translations: vec![Translation::Sub, Translation::Dub],
            },
            base_url,
            http,
        }
    }

    async fn get(&self, path: &str) -> Result<Value, ProviderError> {
        let url = format!("{}/{}", self.base_url, path.trim_start_matches('/'));
        let response = self
            .http
            .plain()
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        if status == 404 {
            return Err(ProviderError::NotFound);
        }
        if !response.status().is_success() {
            return Err(ProviderError::Blocked(format!("HTTP {status}")));
        }
        response.json().await.map_err(|e| ProviderError::Parse(format!("invalid JSON: {e}")))
    }
}

/// First present string field among several candidate names.
fn field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|n| value.get(*n).and_then(Value::as_str))
}

fn number(value: &Value, names: &[&str]) -> Option<u64> {
    names.iter().find_map(|n| {
        value
            .get(*n)
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())))
    })
}

/// Find the array of results, wherever this particular service put it.
fn results(value: &Value) -> &[Value] {
    for key in ["results", "data", "items", "episodes", "sources"] {
        if let Some(array) = value.get(key).and_then(Value::as_array) {
            return array;
        }
    }
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}

pub fn parse_hits(payload: &Value) -> Vec<SearchHit> {
    results(payload)
        .iter()
        .filter_map(|item| {
            let id = field(item, &["id", "animeId", "slug", "session"])?;
            let title =
                field(item, &["title", "name", "romaji"]).map(str::to_owned).or_else(|| {
                    // Some services nest the title the way AniList does.
                    item.get("title")
                        .and_then(|t| field(t, &["romaji", "english", "userPreferred"]))
                        .map(str::to_owned)
                })?;

            Some(SearchHit {
                episode_count: number(item, &["totalEpisodes", "episodes", "episodeCount"])
                    .map(|n| n as u32),
                year: number(item, &["releaseDate", "year", "seasonYear"]).map(|n| n as u16),
                format: field(item, &["type", "format", "subOrDub"]).and_then(parse_format),
                ..SearchHit::new(ProviderKey::new(id), title)
            })
        })
        .collect()
}

fn parse_format(raw: &str) -> Option<MediaFormat> {
    match raw.to_ascii_uppercase().as_str() {
        "TV" | "TV_SHOW" => Some(MediaFormat::Tv),
        "TV_SHORT" => Some(MediaFormat::TvShort),
        "MOVIE" => Some(MediaFormat::Movie),
        "OVA" => Some(MediaFormat::Ova),
        "ONA" => Some(MediaFormat::Ona),
        "SPECIAL" => Some(MediaFormat::Special),
        "MUSIC" => Some(MediaFormat::Music),
        _ => None,
    }
}

pub fn parse_episodes(payload: &Value) -> Vec<Episode> {
    let mut episodes: Vec<Episode> = results(payload)
        .iter()
        .filter_map(|item| {
            let number = field(item, &["number", "episode", "episodeNumber"])
                .map(str::to_owned)
                .or_else(|| {
                    number(item, &["number", "episode", "episodeNumber"]).map(|n| n.to_string())
                })?;

            Some(Episode {
                title: field(item, &["title", "name"]).map(str::to_owned),
                duration: number_duration(item),
                // Consumet-shaped services publish a still under one of these names.
                thumbnail: field(item, &["image", "thumbnail", "img"])
                    .filter(|url| url.starts_with("http"))
                    .map(str::to_owned),
                ..Episode::new(number.as_str())
            })
        })
        .collect();
    // Services disagree about ordering; the UI expects ascending.
    episodes.sort_by(|a, b| a.number.cmp(&b.number));
    episodes
}

fn number_duration(item: &Value) -> Option<std::time::Duration> {
    number(item, &["duration", "durationSeconds", "runtime"])
        .map(|secs| std::time::Duration::from_secs(if secs < 600 { secs * 60 } else { secs }))
}

pub fn parse_streams(payload: &Value, provider_id: &str) -> Vec<Stream> {
    let headers: Vec<(String, String)> = payload
        .get("headers")
        .and_then(Value::as_object)
        .map(|map| {
            map.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned()))).collect()
        })
        .unwrap_or_default();

    let mut streams: Vec<Stream> = results(payload)
        .iter()
        .filter_map(|item| {
            let url = field(item, &["url", "file", "src"])?;
            let quality = field(item, &["quality", "label"])
                .and_then(|q| q.trim_end_matches(['p', 'P']).parse().ok())
                .or_else(|| number(item, &["quality", "height"]).map(|n| n as u32));

            let kind = if url.contains(".m3u8") || field(item, &["isM3U8"]).is_some() {
                StreamKind::Hls
            } else {
                StreamKind::Mp4
            };

            Some(Stream {
                quality,
                headers: headers.clone(),
                provider_id: provider_id.to_owned(),
                ..Stream::new(url, kind)
            })
        })
        .collect();
    // Best first, so the registry's caller can take the head.
    streams.sort_by_key(|s| std::cmp::Reverse(s.quality.unwrap_or(0)));
    streams
}

#[async_trait]
impl Provider for RemoteHttpProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let payload = self.get(&format!("search/{}", urlencode(query))).await?;
        Ok(parse_hits(&payload))
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        let payload = self.get(&format!("info/{}", urlencode(key.as_str()))).await?;
        Ok(parse_episodes(&payload))
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        _translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        let payload = self
            .get(&format!("watch/{}-episode-{}", urlencode(key.as_str()), urlencode(episode)))
            .await?;
        Ok(parse_streams(&payload, &self.manifest.id))
    }
}

/// Percent-encode a path segment.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn hits_parse_from_the_common_consumet_shape() {
        let payload = json!({
            "results": [
                {"id": "frieren", "title": "Frieren", "totalEpisodes": 28, "type": "TV"}
            ]
        });
        let hits = parse_hits(&payload);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key.as_str(), "frieren");
        assert_eq!(hits[0].episode_count, Some(28));
        assert_eq!(hits[0].format, Some(MediaFormat::Tv));
    }

    #[test]
    fn hits_parse_when_the_service_uses_different_field_names() {
        // The point of an escape hatch is that it works with whatever you pointed it at.
        let payload = json!({
            "data": [{"animeId": "x1", "name": "Dandadan", "episodeCount": "12"}]
        });
        let hits = parse_hits(&payload);
        assert_eq!(hits[0].key.as_str(), "x1");
        assert_eq!(hits[0].title, "Dandadan");
        assert_eq!(hits[0].episode_count, Some(12), "numbers may arrive as strings");
    }

    #[test]
    fn a_bare_array_response_parses() {
        let payload = json!([{"id": "a", "title": "A"}]);
        assert_eq!(parse_hits(&payload).len(), 1);
    }

    #[test]
    fn entries_missing_an_id_or_title_are_skipped_not_fatal() {
        // One malformed row should cost that row, not the whole response.
        let payload = json!({"results": [
            {"id": "ok", "title": "Fine"},
            {"title": "No id"},
            {"id": "no-title"}
        ]});
        assert_eq!(parse_hits(&payload).len(), 1);
    }

    #[test]
    fn episodes_sort_ascending_regardless_of_response_order() {
        let payload = json!({"episodes": [
            {"number": 10, "title": "Ten"},
            {"number": 2, "title": "Two"},
            {"number": 9, "title": "Nine"}
        ]});
        let episodes = parse_episodes(&payload);
        let numbers: Vec<&str> = episodes.iter().map(|e| e.number.as_str()).collect();
        assert_eq!(numbers, ["2", "9", "10"], "a string sort would give 10, 2, 9");
    }

    #[test]
    fn episode_durations_in_minutes_are_converted_to_seconds() {
        // Services report both; 24 means minutes, 1440 means seconds.
        let minutes = parse_episodes(&json!({"episodes": [{"number": 1, "duration": 24}]}));
        assert_eq!(minutes[0].duration, Some(std::time::Duration::from_secs(1440)));

        let seconds = parse_episodes(&json!({"episodes": [{"number": 1, "duration": 1440}]}));
        assert_eq!(seconds[0].duration, Some(std::time::Duration::from_secs(1440)));
    }

    #[test]
    fn streams_are_ranked_best_first_and_carry_required_headers() {
        // Dropping the headers turns a working stream into a 403 on referer-locked CDNs.
        let payload = json!({
            "headers": {"Referer": "https://example.test/"},
            "sources": [
                {"url": "https://cdn.test/480.m3u8", "quality": "480p"},
                {"url": "https://cdn.test/1080.m3u8", "quality": "1080p"}
            ]
        });
        let streams = parse_streams(&payload, "remote");
        assert_eq!(streams[0].quality, Some(1080));
        assert_eq!(streams[0].kind, StreamKind::Hls);
        assert_eq!(
            streams[0].headers,
            vec![("Referer".into(), "https://example.test/".into())]
        );
        assert_eq!(streams[0].provider_id, "remote");
    }

    #[test]
    fn a_progressive_url_is_detected_as_mp4() {
        let payload = json!({"sources": [{"file": "https://cdn.test/v.mp4", "label": "720p"}]});
        let streams = parse_streams(&payload, "remote");
        assert_eq!(streams[0].kind, StreamKind::Mp4);
        assert_eq!(streams[0].quality, Some(720));
    }

    #[test]
    fn an_empty_or_unrecognised_payload_yields_nothing_rather_than_panicking() {
        for payload in [json!({}), json!(null), json!({"unexpected": 1}), json!("text")] {
            assert!(parse_hits(&payload).is_empty());
            assert!(parse_episodes(&payload).is_empty());
            assert!(parse_streams(&payload, "remote").is_empty());
        }
    }

    #[test]
    fn path_segments_are_encoded() {
        assert_eq!(urlencode("one two"), "one%20two");
        assert_eq!(urlencode("a/b?c"), "a%2Fb%3Fc");
        assert_eq!(urlencode("safe-id_1.0~x"), "safe-id_1.0~x");
        // A title with non-ASCII must not produce a broken URL.
        assert!(!urlencode("葬送のフリーレン").contains('葬'));
    }

    #[test]
    fn the_manifest_declares_only_the_configured_host() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let p = RemoteHttpProvider::new("https://consumet.internal:8080/anime/", http);
        assert_eq!(p.manifest().allowed_hosts, vec!["consumet.internal:8080"]);
        assert_eq!(p.manifest().kind, ProviderKind::Remote);
    }
}
