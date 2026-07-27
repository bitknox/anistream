//! Optional curation: "which release of this is the good one?"

//!
//! anistream ships no curation endpoint. The URL is supplied by the user as
//! `providers.torrent.curation_url`, a template containing `{anilist_id}`; a service that
//! answers in the documented shape (a PocketBase-style `items[].expand.trs[]` collection
//! carrying `isBest`, `releaseGroup`, `dualAudio` and a release `url`) can be plugged in.
//!
//! Unset means unused — raw indexer ranking then decides, which works fine.
//!
//! Entries pointing somewhere other than the configured indexer are dropped: the picked
//! release is applied by narrowing *that* indexer's feed, so a link elsewhere — a private
//! tracker needing an account, say — is a dead end rather than a source.

use serde::Deserialize;

/// Render the configured curation template for one AniList id.
pub fn query_url(template: &str, anilist_id: u32) -> String {
    if template.contains("{anilist_id}") {
        return template.replace("{anilist_id}", &anilist_id.to_string());
    }
    let separator = if template.contains('?') { '&' } else { '?' };
    format!("{template}{separator}alID={anilist_id}")
}

#[derive(Debug, Clone, Deserialize)]
struct Response {
    #[serde(default)]
    items: Vec<Entry>,
}

#[derive(Debug, Clone, Deserialize)]
struct Entry {
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    comparison: Option<String>,
    #[serde(default)]
    expand: Option<Expand>,
}

#[derive(Debug, Clone, Deserialize)]
struct Expand {
    #[serde(default)]
    trs: Vec<Torrent>,
}

#[derive(Debug, Clone, Deserialize)]
struct Torrent {
    #[serde(rename = "releaseGroup", default)]
    release_group: Option<String>,
    #[serde(rename = "isBest", default)]
    is_best: bool,
    #[serde(rename = "dualAudio", default)]
    dual_audio: bool,
    #[serde(default)]
    url: Option<String>,
}

/// A curated release we can actually use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CuratedRelease {
    pub group: String,
    /// Release URL, on the configured indexer.
    pub url: String,
    pub is_best: bool,
    pub dual_audio: bool,
    /// Curator notes, worth surfacing in the Sources overlay.
    pub notes: Option<String>,
    pub comparison: Option<String>,
}

impl CuratedRelease {
    /// The indexer's view id, which is what a search can be narrowed to.
    pub fn view_id(&self) -> Option<&str> {
        self.url.rsplit('/').next().filter(|s| s.chars().all(|c| c.is_ascii_digit()))
    }
}

/// Host of a URL, lowercased, for comparing a curated link against the configured indexer.
fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority.rsplit('@').next()?.split(':').next()?;
    (!host.is_empty()).then(|| host.to_ascii_lowercase())
}

/// Whether a curated release is usable: it has to live on the indexer we search.
fn is_usable(url: &str, indexer_host: Option<&str>) -> bool {
    match (host_of(url), indexer_host) {
        (Some(host), Some(want)) => {
            let want = want.to_ascii_lowercase();
            host == want || host.ends_with(&format!(".{want}"))
        }
        // No indexer host to compare against: keep it and let feed narrowing decide.
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Parse a curation response, keeping only usable public releases.
pub fn parse(payload: &str, indexer_host: Option<&str>) -> Vec<CuratedRelease> {
    let Ok(response) = serde_json::from_str::<Response>(payload) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for entry in response.items {
        let Some(expand) = entry.expand else {
            continue;
        };
        for torrent in expand.trs {
            let Some(url) = torrent.url.filter(|u| u.starts_with("http")) else {
                continue;
            };
            if !is_usable(&url, indexer_host) {
                continue;
            }
            out.push(CuratedRelease {
                group: torrent.release_group.unwrap_or_else(|| "unknown".into()),
                url,
                is_best: torrent.is_best,
                dual_audio: torrent.dual_audio,
                notes: entry.notes.clone(),
                comparison: entry.comparison.clone(),
            });
        }
    }

    // Best-flagged releases first; the curator's judgement is the whole value here.
    out.sort_by_key(|r| !r.is_best);
    out
}

/// The curated pick for a translation preference, if there is one.
pub fn best(releases: &[CuratedRelease], prefer_dual: bool) -> Option<&CuratedRelease> {
    releases
        .iter()
        .filter(|r| r.is_best)
        .find(|r| r.dual_audio == prefer_dual)
        .or_else(|| releases.iter().find(|r| r.is_best))
        .or_else(|| releases.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative curation payload in the documented shape.
    const PAYLOAD: &str = r#"{
      "totalItems": 1,
      "items": [{
        "alID": 154587,
        "notes": "PMR is JPN BD Remux+LostYears properly synced to the BD",
        "comparison": "https://slow.pics/c/oaWgjcac",
        "expand": {
          "trs": [
            {"releaseGroup":"PMR","isBest":true,"dualAudio":true,
             "url":"https://indexer.example/view/1961373"},
            {"releaseGroup":"PMR","isBest":true,"dualAudio":true,
             "url":"https://elsewhere.example/torrents.php?id=86576"},
            {"releaseGroup":"LostYears","isBest":false,"dualAudio":true,
             "url":"/torrents.php?id=86576&torrentid=1162986"},
            {"releaseGroup":"LostYears","isBest":false,"dualAudio":true,
             "url":"https://indexer.example/view/1998171"}
          ]
        }
      }]
    }"#;

    const HOST: Option<&str> = Some("indexer.example");

    #[test]
    fn the_query_template_is_keyed_on_the_anilist_id() {
        let template =
            "https://curate.example/api/records?filter=(alID={anilist_id})&expand=trs";
        let url = query_url(template, 154_587);
        assert!(url.contains("alID=154587"));
        assert!(url.contains("expand=trs"), "the rest of the template survives");
        assert!(!url.contains("{anilist_id}"), "placeholder must be consumed");

        // A template with no placeholder still gets the id.
        assert!(query_url("https://curate.example/api", 7).ends_with("?alID=7"));
    }

    #[test]
    fn a_real_payload_yields_the_releases_on_the_configured_indexer() {
        let releases = parse(PAYLOAD, HOST);
        assert_eq!(releases.len(), 2, "only the two indexer.example entries are usable");
        assert!(releases.iter().all(|r| r.url.contains("indexer.example")));
    }

    #[test]
    fn releases_hosted_anywhere_else_are_dropped() {
        // A pick is applied by narrowing the configured indexer's feed, so a link on some
        // other service — one needing an account, say — is a dead end rather than a source.
        let releases = parse(PAYLOAD, HOST);
        assert!(!releases.iter().any(|r| r.url.contains("elsewhere.example")));
        assert!(!releases.iter().any(|r| r.url.contains("torrents.php")));

        assert!(is_usable("https://indexer.example/view/1", HOST));
        assert!(is_usable("https://cdn.indexer.example/view/1", HOST), "subdomains count");
        assert!(!is_usable("https://elsewhere.example/view/1", HOST));
        assert!(!is_usable("/torrents.php?id=1", HOST), "a relative link has no host");
        assert!(
            !is_usable("https://indexer.example.evil.test/x", HOST),
            "a suffix match must not be fooled by a longer host"
        );
    }

    #[test]
    fn with_no_indexer_host_to_compare_against_absolute_links_are_kept() {
        let releases = parse(PAYLOAD, None);
        assert_eq!(releases.len(), 3, "only the relative link is unusable");
    }

    #[test]
    fn the_curators_best_pick_sorts_first() {
        let releases = parse(PAYLOAD, HOST);
        assert!(releases[0].is_best);
        assert_eq!(releases[0].group, "PMR");
    }

    #[test]
    fn curator_notes_and_comparisons_survive_for_the_sources_overlay() {
        let releases = parse(PAYLOAD, HOST);
        assert!(releases[0].notes.as_deref().unwrap().contains("JPN BD Remux"));
        assert!(releases[0].comparison.as_deref().unwrap().contains("slow.pics"));
    }

    #[test]
    fn the_view_id_is_extractable_for_narrowing_a_search() {
        assert_eq!(parse(PAYLOAD, HOST)[0].view_id(), Some("1961373"));

        let odd = CuratedRelease {
            group: "g".into(),
            url: "https://indexer.example/view/not-a-number".into(),
            is_best: true,
            dual_audio: false,
            notes: None,
            comparison: None,
        };
        assert_eq!(odd.view_id(), None);
    }

    #[test]
    fn the_best_pick_respects_the_audio_preference_then_falls_back() {
        let releases = parse(PAYLOAD, HOST);
        // Everything here is dual-audio, so a sub preference still gets the best release
        // rather than nothing.
        assert_eq!(best(&releases, true).unwrap().group, "PMR");
        assert_eq!(best(&releases, false).unwrap().group, "PMR");
        assert!(best(&[], false).is_none());
    }

    #[test]
    fn an_uncovered_title_yields_nothing_rather_than_failing() {
        // The common case: curation covers a subset, and raw ranking covers the rest.
        assert!(parse(r#"{"totalItems":0,"items":[]}"#, HOST).is_empty());
    }

    #[test]
    fn malformed_or_unexpected_payloads_are_survivable() {
        for payload in ["", "not json", "{}", r#"{"items":[{}]}"#, "null"] {
            assert!(parse(payload, HOST).is_empty(), "unexpected result from {payload:?}");
        }
    }

    #[test]
    fn an_entry_with_a_relative_url_is_skipped() {
        let payload = r#"{"items":[{"expand":{"trs":[
            {"releaseGroup":"X","isBest":true,"url":"/relative"}
        ]}}]}"#;
        assert!(parse(payload, HOST).is_empty(), "a relative URL is not fetchable");
    }
}
