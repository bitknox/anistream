// SPDX-License-Identifier: GPL-3.0-or-later
//! Native implementation of ani-cli's provider protocol, without shell subprocesses.
//! Protocol reference: pystardust/ani-cli at 0301d67 (5.1.6).
//! See LICENSE-ani-cli and docs/ani-cli-provider.md.

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::{Stream, StreamKind, Subtitle},
    traits::{Provider, ProviderKind, ProviderManifest},
};
use anistream_net::HttpClient;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use regex::Regex;
use reqwest::Url;
use serde_json::Value;

pub struct AniCliProvider {
    manifest: ProviderManifest,
    http: HttpClient,
    base: Url,
}

fn parse_error(message: impl Into<String>) -> ProviderError {
    ProviderError::Parse(message.into())
}

fn web_url(raw: &str) -> Result<Url, ProviderError> {
    let url = Url::parse(raw).map_err(|e| parse_error(e.to_string()))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(parse_error("provider returned an invalid web URL"));
    }
    Ok(url)
}

impl AniCliProvider {
    pub fn new(http: HttpClient) -> Self {
        Self {
            manifest: ProviderManifest {
                id: "ani-cli".into(),
                display_name: "ani-cli (HiAnime)".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                kind: ProviderKind::Native,
                allowed_hosts: vec!["hianime.at".into()],
                translations: vec![Translation::Sub, Translation::Dub],
            },
            http,
            base: Url::parse("https://hianime.at/").expect("static URL"),
        }
    }

    async fn fetch(&self, url: &Url, referer: Option<&str>) -> Result<String, ProviderError> {
        let mut request = self.http.emulated().get(url.as_str());
        if let Some(referer) = referer {
            request = request.header("Referer", referer);
        }
        // Some networks cannot establish the emulated TLS connection. Retry the same
        // read with the standard client, which uses the platform's trusted roots.
        let (status, body) = match request.send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response
                    .text()
                    .await
                    .map_err(|e| ProviderError::Transport(e.to_string()))?;
                (status, body)
            }
            Err(_) => {
                let mut request = self.http.plain().get(url.as_str());
                if let Some(referer) = referer {
                    request = request.header("Referer", referer);
                }
                let response = request
                    .send()
                    .await
                    .map_err(|e| ProviderError::Transport(e.to_string()))?;
                let status = response.status().as_u16();
                let body = response
                    .text()
                    .await
                    .map_err(|e| ProviderError::Transport(e.to_string()))?;
                (status, body)
            }
        };
        if status == 404 {
            return Err(ProviderError::NotFound);
        }
        if !(200..300).contains(&status) {
            return Err(ProviderError::Blocked(format!("HTTP {status}")));
        }
        if body.to_ascii_lowercase().contains("<title>just a moment") {
            return Err(ProviderError::Blocked("provider requires a browser challenge".into()));
        }
        Ok(body)
    }

    async fn episode_rows(
        &self,
        key: &ProviderKey,
    ) -> Result<Vec<(String, Episode)>, ProviderError> {
        let id = key
            .as_str()
            .rsplit('-')
            .next()
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .ok_or_else(|| parse_error("invalid title key"))?;
        let url = self
            .base
            .join(&format!("api/theme/episode/list/{id}"))
            .map_err(|e| parse_error(e.to_string()))?;
        parse_episode_rows(&self.fetch(&url, None).await?, key.as_str())
    }
}

// AJAX responses wrap markup in JSON. Parsing JSON avoids destroying escaped title text.
fn markup(raw: &str) -> Result<String, ProviderError> {
    if raw.trim_start().starts_with('{') {
        let json: Value = serde_json::from_str(raw).map_err(|e| parse_error(e.to_string()))?;
        json.get("html")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| parse_error("missing html in AJAX response"))
    } else {
        Ok(raw.to_owned())
    }
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"(?:^|\s){}\s*=\s*(?:"([^"]*)"|'([^']*)')"#, regex::escape(name));
    let regex = Regex::new(&pattern).ok()?;
    let found = regex.captures(tag)?;
    Some(entities(found.get(1).or_else(|| found.get(2))?.as_str()))
}

fn entities(raw: &str) -> String {
    let regex =
        Regex::new(r"&(#x[0-9a-fA-F]+|#[0-9]+|amp|quot|apos|lt|gt);").expect("static regex");
    regex
        .replace_all(raw, |c: &regex::Captures<'_>| {
            let value = &c[1];
            match value {
                "amp" => "&".into(),
                "quot" => "\"".into(),
                "apos" => "'".into(),
                "lt" => "<".into(),
                "gt" => ">".into(),
                _ => value
                    .strip_prefix("#x")
                    .and_then(|n| u32::from_str_radix(n, 16).ok())
                    .or_else(|| value.strip_prefix('#').and_then(|n| n.parse().ok()))
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| c[0].to_owned()),
            }
        })
        .into_owned()
}

fn parse_hits(html: &str) -> Result<Vec<SearchHit>, ProviderError> {
    let html = html.split("id=\"main-sidebar\"").next().unwrap_or(html);
    let regex =
        Regex::new(r#"(?s)<h3\b[^>]*class=["'][^"']*film-name[^"']*["'][^>]*>\s*<a\b([^>]*)>"#)
            .expect("static regex");
    let mut hits = Vec::new();
    for tag in regex.captures_iter(html) {
        if let (Some(href), Some(title)) = (attr(&tag[1], "href"), attr(&tag[1], "title")) {
            let key = href.trim_end_matches('/').rsplit('/').next().unwrap_or_default();
            if !key.is_empty() && !hits.iter().any(|h: &SearchHit| h.key.as_str() == key) {
                hits.push(SearchHit::new(ProviderKey::new(key), title));
            }
        }
    }
    if hits.is_empty() && !html.contains("film_list") && !html.contains("No results") {
        return Err(parse_error("unrecognized search page"));
    }
    Ok(hits)
}

fn parse_episode_rows(raw: &str, key: &str) -> Result<Vec<(String, Episode)>, ProviderError> {
    let html = markup(raw)?;
    let tags = Regex::new(r"<a\b[^>]*>").expect("static regex");
    let mut rows = Vec::new();
    for tag in tags.find_iter(&html) {
        let tag = tag.as_str();
        if let (Some(number), Some(id), Some(href)) =
            (attr(tag, "data-number"), attr(tag, "data-id"), attr(tag, "href"))
        {
            if !href.split('?').next().unwrap_or_default().ends_with(&format!("/watch/{key}")) {
                continue;
            }
            if !id.bytes().all(|b| b.is_ascii_digit()) || id.is_empty() {
                continue;
            }
            let mut episode = Episode::new(number.as_str());
            episode.title = attr(tag, "title");
            rows.push((id, episode));
        }
    }
    rows.sort_by(|a, b| a.1.number.cmp(&b.1.number));
    rows.dedup_by(|a, b| a.1.number == b.1.number);
    if rows.is_empty() {
        return Err(ProviderError::NotFound);
    }
    Ok(rows)
}

fn embed_url(raw: &str, translation: Translation) -> Result<Url, ProviderError> {
    let html = markup(raw)?;
    let tags = Regex::new(r"<[a-zA-Z][^>]*>").expect("static regex");
    for tag in tags.find_iter(&html) {
        let tag = tag.as_str();
        if attr(tag, "data-type").as_deref() == Some(translation.as_str())
            && attr(tag, "data-server-name").as_deref() == Some("ZokoAnime")
        {
            let encoded =
                attr(tag, "data-hash").ok_or_else(|| parse_error("missing embed URL"))?;
            let bytes = STANDARD.decode(encoded).map_err(|e| parse_error(e.to_string()))?;
            return web_url(
                std::str::from_utf8(&bytes).map_err(|e| parse_error(e.to_string()))?,
            );
        }
    }
    Err(ProviderError::NotFound)
}

fn stream_config(html: &str) -> Result<Value, ProviderError> {
    let regex = Regex::new(r#"window\.__P\s*=\s*"([^"]+)""#).expect("static regex");
    let blob =
        regex.captures(html).ok_or_else(|| parse_error("missing embed configuration"))?;
    let bytes = STANDARD.decode(&blob[1]).map_err(|e| parse_error(e.to_string()))?;
    let key = b"otaku-embed-v1";
    let decoded: Vec<u8> =
        bytes.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect();
    serde_json::from_slice(&decoded)
        .map_err(|e| parse_error(format!("invalid embed configuration: {e}")))
}

#[async_trait]
impl Provider for AniCliProvider {
    fn manifest(&self) -> &ProviderManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _translation: Translation,
    ) -> Result<Vec<SearchHit>, ProviderError> {
        let mut url = self.base.join("search").map_err(|e| parse_error(e.to_string()))?;
        url.query_pairs_mut().append_pair("keyword", query);
        parse_hits(&self.fetch(&url, None).await?)
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        _translation: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        Ok(self.episode_rows(key).await?.into_iter().map(|(_, episode)| episode).collect())
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        translation: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        let rows = self.episode_rows(key).await?;
        let (id, _) = rows
            .iter()
            .find(|(_, e)| e.number.as_str() == episode)
            .ok_or(ProviderError::NotFound)?;
        let mut url = self
            .base
            .join("api/theme/episode/servers")
            .map_err(|e| parse_error(e.to_string()))?;
        url.query_pairs_mut().append_pair("episodeId", id);
        let embed = embed_url(&self.fetch(&url, None).await?, translation)?;
        let referer = format!("{}/", embed.origin().ascii_serialization());
        let config = stream_config(&self.fetch(&embed, None).await?)?;
        let source = config
            .get("src")
            .and_then(Value::as_str)
            .ok_or_else(|| parse_error("missing stream URL"))?;
        let source = web_url(source)?;
        // Keep the master playlist intact: mpv selects variants and retains separate audio renditions.
        let mut stream =
            Stream::new(source.as_str(), StreamKind::Hls).with_header("Referer", &referer);
        stream.provider_id = self.manifest.id.clone();
        if let Some(tracks) = config.get("subtitles").and_then(Value::as_array) {
            for track in tracks {
                if let Some(src) = track.get("src").and_then(Value::as_str)
                    && let Ok(url) = web_url(src)
                {
                    stream.subtitles.push(Subtitle {
                        url: url.to_string(),
                        hard: false,
                        format: Some("vtt".into()),
                        language: track
                            .get("srclang")
                            .or_else(|| track.get("label"))
                            .and_then(Value::as_str)
                            .unwrap_or("und")
                            .to_owned(),
                    });
                }
            }
        }
        Ok(vec![stream])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn search_handles_entities_reordered_attributes_and_duplicates() {
        let html = r#"<h3 class="film-name"><a title="Stars &amp; Rain &#039;2&#039;" href="/stars-42">x</a></h3>
        <h3 class="film-name"><a href="/stars-42" title="Duplicate">x</a></h3>"#;
        let hits = parse_hits(html).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Stars & Rain '2'");
        assert_eq!(hits[0].key.as_str(), "stars-42");
        assert!(parse_hits("<html>unexpected</html>").is_err());
    }

    #[test]
    fn ajax_episode_list_filters_other_titles_and_sorts_numerically() {
        let raw = json!({"html": r#"<a href="/watch/stars-42?ep=10" data-id="10" data-number="10"></a>
        <a data-number="2" href="/watch/stars-42?ep=2" data-id="2"></a>
        <a data-number="1" data-id="1" href="/watch/other-8?ep=1"></a>"#}).to_string();
        let rows = parse_episode_rows(&raw, "stars-42").unwrap();
        assert_eq!(
            rows.iter().map(|(_, e)| e.number.as_str()).collect::<Vec<_>>(),
            ["2", "10"]
        );
        assert!(parse_episode_rows(r#"{"unexpected":true}"#, "stars-42").is_err());
    }

    #[test]
    fn server_selection_respects_translation_and_rejects_non_web_urls() {
        let sub = STANDARD.encode("https://embed.example/sub");
        let dub = STANDARD.encode("https://embed.example/dub");
        let html = format!(
            r#"<div data-hash="{sub}" data-server-name="ZokoAnime" data-type="sub"></div>
        <div data-type="dub" data-hash="{dub}" data-server-name="ZokoAnime"></div>"#
        );
        assert_eq!(embed_url(&html, Translation::Dub).unwrap().path(), "/dub");
        assert!(web_url("file:///etc/passwd").is_err());
        assert!(web_url("https://user:password@embed.example/").is_err());
    }

    #[test]
    fn configuration_decodes_and_malformed_blobs_report_errors() {
        let payload = br#"{"src":"https://cdn.example/master.m3u8","subtitles":[]}"#;
        let key = b"otaku-embed-v1";
        let encoded = STANDARD.encode(
            payload.iter().enumerate().map(|(i, b)| b ^ key[i % key.len()]).collect::<Vec<_>>(),
        );
        assert_eq!(
            stream_config(&format!(r#"window.__P="{encoded}";"#)).unwrap()["src"],
            "https://cdn.example/master.m3u8"
        );
        assert!(stream_config(r#"window.__P="%%%";"#).is_err());
    }
}
