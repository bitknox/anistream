//! Torrent indexer RSS.
//!
//! anistream ships **no indexer of its own**. The endpoint is supplied by the user as
//! `providers.torrent.rss_url`, a template containing `{query}`; this module only knows how
//! to render that template and parse what comes back.
//!
//! The expected shape is a standard RSS 2.0 feed whose `<item>`s carry seeders, leechers and
//! an info hash — the de-facto format for torrent indexers and Torznab endpoints alike. Tag
//! names are matched by local name, so any namespace prefix works.
//!
//! The RSS is parsed by hand rather than with a general XML crate. The feed is a fixed,
//! flat shape, and a hand parser keeps a dependency out of the tree while making the CDATA
//! and entity handling explicit.

use crate::torrent::release::{Release, parse as parse_release};

/// One entry from the feed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerItem {
    pub title: String,
    /// Direct `.torrent` link.
    pub link: String,
    /// Page URL, which doubles as a stable id.
    pub guid: String,
    pub seeders: u32,
    pub leechers: u32,
    pub size: Option<String>,
    pub info_hash: Option<String>,
    /// What the title told us.
    pub release: Release,
}

impl IndexerItem {
    /// Magnet URI, when the feed reported an info hash.
    ///
    /// Preferred over the `.torrent` URL: it needs no second HTTP request. Trackers are
    /// whatever the user configured — proxy mode disables DHT, so without at least one
    /// tracker there is no way to find peers.
    pub fn magnet(&self, trackers: &[String]) -> Option<String> {
        let hash = self.info_hash.as_ref()?;
        let name = urlencode(&self.title);
        let mut magnet = format!("magnet:?xt=urn:btih:{hash}&dn={name}");
        for tracker in trackers {
            magnet.push_str("&tr=");
            magnet.push_str(&urlencode(tracker));
        }
        Some(magnet)
    }
}

/// Render the configured search template.
///
/// `{query}` is replaced with the URL-encoded search terms. A template with no placeholder
/// gets the terms appended as a `q=` parameter, which covers the common Torznab shape.
pub fn search_url(template: &str, query: &str, quality: Option<u32>) -> String {
    let mut terms = query.trim().to_owned();
    if let Some(q) = quality {
        // Narrowing server-side beats filtering a full page client-side.
        terms.push_str(&format!(" {q}p"));
    }
    let encoded = urlencode(&terms);
    if template.contains("{query}") {
        return template.replace("{query}", &encoded);
    }
    let separator = if template.contains('?') { '&' } else { '?' };
    format!("{template}{separator}q={encoded}")
}

fn urlencode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Decode the handful of XML entities the feed actually uses.
fn decode_entities(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        // Ampersand last, or a doubly-encoded entity would be mangled.
        .replace("&amp;", "&")
}

/// Text of the first element with this local name.
///
/// Matched by *local* name, so `<seeders>`, `<idx:seeders>` and `<torznab:seeders>` all
/// resolve — indexers differ in namespace and the parser should not care. Attributes are
/// handled too: feeds routinely emit `<guid isPermaLink="true">`, so matching a bare
/// `<guid>` would silently find nothing.
fn tag_text(fragment: &str, tag: &str) -> Option<String> {
    let mut search_from = 0;
    let (content_start, name) = loop {
        let lt = fragment[search_from..].find('<')? + search_from;
        let after = lt + 1;
        let name_end = fragment[after..]
            .find(|c: char| c == '>' || c == '/' || c.is_whitespace())
            .map(|i| i + after)?;
        let name = &fragment[after..name_end];
        if !name.is_empty()
            && !name.starts_with('/')
            && !name.starts_with('!')
            && !name.starts_with('?')
            && name.rsplit(':').next().unwrap_or(name) == tag
        {
            let gt = fragment[name_end..].find('>')? + name_end;
            break (gt + 1, name.to_owned());
        }
        search_from = after;
    };

    let close = format!("</{name}>");
    let end = fragment[content_start..].find(&close)? + content_start;
    let raw = fragment[content_start..end].trim();

    // Titles are commonly wrapped in CDATA.
    let unwrapped =
        raw.strip_prefix("<![CDATA[").and_then(|r| r.strip_suffix("]]>")).unwrap_or(raw);
    Some(decode_entities(unwrapped).trim().to_owned())
}

/// Parse an indexer RSS document.
///
/// Malformed items are skipped rather than failing the feed: one bad entry should cost that
/// entry, not the whole search.
pub fn parse_feed(xml: &str) -> Vec<IndexerItem> {
    let mut items = Vec::new();

    for fragment in xml.split("<item>").skip(1) {
        let body = fragment.split("</item>").next().unwrap_or(fragment);

        let Some(title) = tag_text(body, "title") else {
            continue;
        };
        let Some(link) = tag_text(body, "link") else {
            continue;
        };

        items.push(IndexerItem {
            release: parse_release(&title),
            guid: tag_text(body, "guid").unwrap_or_else(|| link.clone()),
            seeders: tag_text(body, "seeders").and_then(|s| s.parse().ok()).unwrap_or(0),
            leechers: tag_text(body, "leechers").and_then(|s| s.parse().ok()).unwrap_or(0),
            size: tag_text(body, "size"),
            info_hash: tag_text(body, "infoHash").filter(|h| h.len() == 40),
            title,
            link,
        });
    }
    items
}

/// How much we want a given release, higher is better.
///
/// Seeders dominate, because a beautifully encoded torrent with two peers will not stream.
/// Everything else breaks ties.
pub fn score(
    item: &IndexerItem,
    wanted_episode: u32,
    wanted_season: Option<u32>,
    desired_quality: u32,
    prefer_dual: bool,
) -> i64 {
    if !item.release.covers(wanted_episode, wanted_season) {
        return i64::MIN;
    }

    // Diminishing returns: 200 seeders is not twice as good as 100, but 2 is far worse
    // than 20.
    let mut score = (f64::from(item.seeders.min(500)).sqrt() * 40.0) as i64;

    // A hard tier, not a weight: below ~15 seeders a torrent may not stream at all, and
    // no amount of quality bonus should promote a release that cannot arrive over one
    // that can. It stays selectable — sometimes the swarm is all there is.
    if item.seeders <= 15 {
        score -= 2000;
    }

    match item.release.quality {
        Some(q) if q == desired_quality => score += 300,
        // A step down is barely noticeable; upscaling wastes bandwidth for nothing.
        Some(q) if q < desired_quality => score += 150 - i64::from(desired_quality - q) / 10,
        Some(_) => score += 40,
        None => {}
    }

    if item.release.bluray {
        score += 80;
    }
    // Dual audio serves a dub preference outright; a dub-only release does too, but may
    // carry no subtitles, so it is also the one thing worth steering a sub watcher around.
    if prefer_dual && (item.release.dual_audio || item.release.dubbed) {
        score += 60;
    }
    if !prefer_dual && item.release.dubbed && !item.release.dual_audio {
        score -= 40;
    }
    // A single episode beats a batch when only one is wanted — far less to download
    // before playback can start.
    if !item.release.is_batch() {
        score += 120;
    }
    // v2 and later fix encoding faults.
    score += i64::from(item.release.version.unwrap_or(1).min(5)) * 10;

    score
}

/// Every release that covers the episode, best-first.
///
/// The same scoring as [`best`], kept as a list so the user can overrule it: automatic
/// resolution wants one answer, the Sources overlay wants the whole slate.
pub fn ranked(
    items: &[IndexerItem],
    episode: u32,
    season: Option<u32>,
    quality: u32,
    prefer_dual: bool,
) -> Vec<&IndexerItem> {
    let mut scored: Vec<(i64, &IndexerItem)> = items
        .iter()
        .map(|item| (score(item, episode, season, quality, prefer_dual), item))
        .filter(|(s, _)| *s > i64::MIN)
        .collect();
    scored.sort_by_key(|(s, _)| std::cmp::Reverse(*s));
    scored.into_iter().map(|(_, item)| item).collect()
}

/// Best release for an episode, or `None` if nothing covers it.
pub fn best(
    items: &[IndexerItem],
    episode: u32,
    season: Option<u32>,
    quality: u32,
    prefer_dual: bool,
) -> Option<&IndexerItem> {
    items
        .iter()
        .map(|item| (score(item, episode, season, quality, prefer_dual), item))
        .filter(|(s, _)| *s > i64::MIN)
        .max_by_key(|(s, _)| *s)
        .map(|(_, item)| item)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A representative indexer feed.
    const FEED: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<rss xmlns:idx="https://indexer.example/xmlns" version="2.0"><channel>
<item>
  <title>[SubGroup] Sousou no Frieren - 12 (1080p) [A1B2C3D4].mkv</title>
  <link>https://indexer.example/download/1234567.torrent</link>
  <guid isPermaLink="true">https://indexer.example/view/1234567</guid>
  <idx:seeders>142</idx:seeders>
  <idx:leechers>7</idx:leechers>
  <idx:size>1.4 GiB</idx:size>
  <idx:infoHash>0123456789abcdef0123456789abcdef01234567</idx:infoHash>
</item>
<item>
  <title><![CDATA[[BatchGroup] Sousou no Frieren (01-28) (BD 1080p) [Dual-Audio]]]></title>
  <link>https://indexer.example/download/7654321.torrent</link>
  <guid isPermaLink="true">https://indexer.example/view/7654321</guid>
  <idx:seeders>99</idx:seeders>
  <idx:leechers>3</idx:leechers>
  <idx:size>18.2 GiB</idx:size>
</item>
</channel></rss>"#;

    #[test]
    fn a_real_feed_shape_parses() {
        let items = parse_feed(FEED);
        assert_eq!(items.len(), 2);

        let first = &items[0];
        assert_eq!(first.seeders, 142);
        assert_eq!(first.leechers, 7);
        assert_eq!(first.size.as_deref(), Some("1.4 GiB"));
        assert_eq!(first.release.episode, Some(12));
        assert_eq!(first.release.quality, Some(1080));
        assert!(first.guid.contains("/view/1234567"));
    }

    #[test]
    fn cdata_wrapped_titles_are_unwrapped() {
        let items = parse_feed(FEED);
        assert!(items[1].title.starts_with("[BatchGroup]"), "got {:?}", items[1].title);
        assert!(!items[1].title.contains("CDATA"));
        assert_eq!(items[1].release.batch, Some((1, 28)));
    }

    #[test]
    fn tags_with_attributes_are_read() {
        // Feeds emit `<guid isPermaLink="true">`; requiring a bare `<guid>` finds nothing.
        let xml = r#"<item><title>S</title><link>l</link>
            <guid isPermaLink="true">https://indexer.example/view/42</guid></item>"#;
        assert_eq!(parse_feed(xml)[0].guid, "https://indexer.example/view/42");
    }

    #[test]
    fn a_longer_element_name_is_not_mistaken_for_a_shorter_one() {
        // `<titleAlt>` must not satisfy a search for `<title>`.
        let xml = "<item><titleAlt>Wrong</titleAlt><title>Right</title><link>l</link></item>";
        assert_eq!(parse_feed(xml)[0].title, "Right");
    }

    #[test]
    fn xml_entities_are_decoded() {
        let xml = "<item><title>Show &amp; Friends &#39;s End</title><link>x</link></item>";
        assert_eq!(parse_feed(xml)[0].title, "Show & Friends 's End");
    }

    #[test]
    fn an_item_missing_a_title_or_link_is_skipped_not_fatal() {
        let xml = "<item><title>Good</title><link>a</link></item>\
                   <item><link>b</link></item>\
                   <item><title>Also good</title><link>c</link></item>";
        assert_eq!(parse_feed(xml).len(), 2);
    }

    #[test]
    fn a_missing_seeder_count_reads_as_zero_rather_than_failing() {
        let xml = "<item><title>Show - 1</title><link>a</link></item>";
        assert_eq!(parse_feed(xml)[0].seeders, 0);
    }

    #[test]
    fn an_empty_or_garbage_feed_yields_nothing() {
        for xml in ["", "not xml", "<rss></rss>", "<item>"] {
            assert!(parse_feed(xml).is_empty(), "unexpected items from {xml:?}");
        }
    }

    #[test]
    fn a_magnet_carries_the_configured_trackers() {
        let items = parse_feed(FEED);
        let trackers = vec![
            "http://tracker.example:7777/announce".to_owned(),
            "udp://other.example:1337/announce".to_owned(),
        ];
        let magnet = items[0].magnet(&trackers).expect("first item has a hash");
        assert!(magnet.starts_with("magnet:?xt=urn:btih:0123456789abcdef"));
        // Trackers matter here: proxy mode disables DHT, so they are the only way to find
        // peers — and they come from config, never from anistream.
        assert_eq!(magnet.matches("&tr=").count(), 2);
        assert!(magnet.contains("tracker.example"));

        assert!(items[1].magnet(&trackers).is_none(), "no hash, no magnet");
    }

    #[test]
    fn a_magnet_with_no_configured_trackers_is_still_well_formed() {
        let items = parse_feed(FEED);
        let magnet = items[0].magnet(&[]).unwrap();
        assert!(magnet.starts_with("magnet:?xt=urn:btih:"));
        assert!(!magnet.contains("&tr="));
    }

    #[test]
    fn a_malformed_info_hash_is_rejected() {
        let xml =
            "<item><title>S</title><link>a</link><idx:infoHash>short</idx:infoHash></item>";
        assert!(parse_feed(xml)[0].info_hash.is_none());
    }

    #[test]
    fn seeder_tags_are_matched_by_local_name_whatever_the_namespace() {
        // Indexers differ in namespace prefix; the parser must not care.
        for tag in ["seeders", "idx:seeders", "torznab:seeders"] {
            let xml =
                format!("<item><title>S - 01</title><link>l</link><{tag}>42</{tag}></item>");
            assert_eq!(parse_feed(&xml)[0].seeders, 42, "failed for <{tag}>");
        }
    }

    #[test]
    fn the_search_template_is_rendered_with_encoded_terms() {
        let template = "https://indexer.example/?page=rss&q={query}&s=seeders";
        let url = search_url(template, "Sousou no Frieren", Some(1080));
        assert!(url.contains("q=Sousou+no+Frieren+1080p"));
        assert!(url.contains("s=seeders"), "the rest of the template survives");
        assert!(!url.contains("{query}"), "placeholder must be consumed");

        // Anything the user typed is encoded, never injected raw.
        assert!(!search_url(template, "a&b", None).contains("q=a&b"));
    }

    #[test]
    fn a_template_without_a_placeholder_gets_the_query_appended() {
        assert!(search_url("https://indexer.example/rss", "x", None).ends_with("?q=x"));
        assert!(search_url("https://indexer.example/rss?cat=1", "x", None).ends_with("&q=x"));
    }

    fn item(title: &str, seeders: u32) -> IndexerItem {
        IndexerItem {
            release: parse_release(title),
            title: title.into(),
            link: "l".into(),
            guid: "g".into(),
            seeders,
            leechers: 0,
            size: None,
            info_hash: None,
        }
    }

    #[test]
    fn a_release_that_cannot_supply_the_episode_is_never_chosen() {
        let items = [item("[G] Show - 3 (1080p)", 500)];
        assert_eq!(score(&items[0], 12, None, 1080, false), i64::MIN);
        assert!(best(&items, 12, None, 1080, false).is_none());
    }

    #[test]
    fn seeders_dominate_because_a_dead_torrent_cannot_stream() {
        let items = [
            item("[Perfect] Show - 12 (1080p) [BD]", 1),
            item("[Ordinary] Show - 12 (720p)", 400),
        ];
        assert_eq!(
            best(&items, 12, None, 1080, false).unwrap().title,
            "[Ordinary] Show - 12 (720p)",
            "a beautifully encoded torrent with one peer will not play"
        );
    }

    #[test]
    fn with_comparable_seeders_the_better_release_wins() {
        let items = [item("[A] Show - 12 (720p)", 100), item("[B] Show - 12 (1080p)", 100)];
        assert_eq!(best(&items, 12, None, 1080, false).unwrap().title, "[B] Show - 12 (1080p)");
    }

    #[test]
    fn a_single_episode_beats_a_batch_when_only_one_is_wanted() {
        // Far less to download before playback can start.
        let items = [item("[A] Show (01-24) (1080p)", 100), item("[B] Show - 12 (1080p)", 100)];
        assert_eq!(best(&items, 12, None, 1080, false).unwrap().title, "[B] Show - 12 (1080p)");
    }

    #[test]
    fn a_batch_is_still_used_when_it_is_the_only_thing_covering_the_episode() {
        let items = [item("[A] Show (01-24) (1080p)", 50)];
        assert_eq!(
            best(&items, 12, None, 1080, false).unwrap().title,
            "[A] Show (01-24) (1080p)"
        );
    }

    #[test]
    fn dual_audio_is_preferred_only_when_asked_for() {
        let items = [
            item("[A] Show - 12 (1080p)", 100),
            item("[B] Show - 12 (1080p) [Dual Audio]", 100),
        ];
        assert_eq!(
            best(&items, 12, None, 1080, true).unwrap().title,
            "[B] Show - 12 (1080p) [Dual Audio]"
        );
        assert!(best(&items, 12, None, 1080, false).is_some());
    }

    #[test]
    fn a_dub_only_release_satisfies_a_dub_preference() {
        let items = [
            item("[A] Show - 12 (1080p)", 100),
            item("[B] Show - 12 (1080p) [English Dub]", 100),
        ];
        assert_eq!(
            best(&items, 12, None, 1080, true).unwrap().title,
            "[B] Show - 12 (1080p) [English Dub]"
        );
        // And the reverse: a sub watcher is steered away from it, since a dub-only
        // release may carry no subtitles at all.
        assert_eq!(best(&items, 12, None, 1080, false).unwrap().title, "[A] Show - 12 (1080p)");
    }

    #[test]
    fn upscaling_is_not_preferred_over_a_small_step_down() {
        let items = [item("[A] Show - 12 (720p)", 100), item("[B] Show - 12 (2160p)", 100)];
        assert_eq!(
            best(&items, 12, None, 1080, false).unwrap().title,
            "[A] Show - 12 (720p)",
            "a step down is imperceptible; 4K for a 1080p request wastes bandwidth"
        );
    }

    #[test]
    fn a_later_version_is_preferred() {
        let items = [item("[A] Show - 12 (1080p)", 100), item("[A] Show - 12v2 (1080p)", 100)];
        assert_eq!(
            best(&items, 12, None, 1080, false).unwrap().title,
            "[A] Show - 12v2 (1080p)"
        );
    }

    #[test]
    fn choosing_from_nothing_yields_nothing() {
        assert!(best(&[], 1, None, 1080, false).is_none());
    }
}
