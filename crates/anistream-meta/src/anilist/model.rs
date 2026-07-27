//! AniList response types.
//!
//! Deliberately a hand-written subset rather than a generated schema binding. AniList's
//! schema is enormous and mostly irrelevant here; modelling only what the UI renders keeps
//! the surface small and makes a breaking upstream change show up as one failed field
//! rather than a wall of them.

use anistream_core::{
    ids::AnilistId,
    media::{MediaFormat, MediaStatus},
};
use serde::Deserialize;

/// A title in the three forms AniList publishes.
///
/// All three matter: `romaji` is what torrent release groups use, `english` is what people
/// type, and `native` occasionally matches nothing else. The resolution ladder feeds all of
/// them to a provider search, which is why none is discarded.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

impl Title {
    /// The form to show in the UI: English if there is one, else romaji.
    pub fn display(&self) -> &str {
        self.english
            .as_deref()
            .or(self.romaji.as_deref())
            .or(self.native.as_deref())
            .unwrap_or("Untitled")
    }

    /// The form to show as a subheading, when it differs from [`Self::display`].
    pub fn secondary(&self) -> Option<&str> {
        match (&self.english, &self.romaji) {
            (Some(en), Some(ro)) if en != ro => Some(ro),
            _ => None,
        }
    }

    /// Every distinct title, in the order provider search should try them.
    pub fn all(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(3);
        for candidate in [&self.romaji, &self.english, &self.native] {
            if let Some(t) = candidate
                && !t.is_empty()
                && !out.contains(t)
            {
                out.push(t.clone());
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub color: Option<String>,
}

impl CoverImage {
    pub fn best(&self) -> Option<&str> {
        self.extra_large.as_deref().or(self.large.as_deref())
    }
}

/// When the next episode airs.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NextAiring {
    pub episode: u32,
    /// Unix seconds.
    pub airing_at: i64,
    pub time_until_airing: i64,
}

/// A link to a service that streams this title.
///
/// Carries the licensed-source story: AniList publishes per-episode Crunchyroll deep links
/// and per-series links for Netflix, Hulu and others, with no authentication at all. That
/// is how anistream can offer a legitimate route to a title before reaching for anything
/// else.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLink {
    pub site: String,
    pub url: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub language: Option<String>,
}

impl ExternalLink {
    pub fn is_streaming(&self) -> bool {
        self.kind.as_deref() == Some("STREAMING")
    }
}

/// A single episode's deep link on a streaming service.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StreamingEpisode {
    pub title: Option<String>,
    pub url: Option<String>,
    pub site: Option<String>,
    pub thumbnail: Option<String>,
}

impl StreamingEpisode {
    /// Extract the episode number from AniList's `"Episode 12 - Title"` convention.
    ///
    /// Best-effort by necessity: the field is free text and services phrase it differently,
    /// so a failure here degrades to "no deep link for this episode" rather than an error.
    pub fn episode_number(&self) -> Option<u32> {
        let title = self.title.as_deref()?;
        let rest = title.strip_prefix("Episode ").or_else(|| {
            title.split_once(char::is_numeric).and_then(|_| title.split_whitespace().nth(1))
        })?;
        rest.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty())?.parse().ok()
    }

    /// The episode's own title, with AniList's `"Episode 12 - "` prefix removed.
    ///
    /// The field is free text written by whoever listed the episode, so this trims the shapes
    /// actually seen — `-`, `–` and `:` separators — and gives up rather than guessing. A entry
    /// that is *only* the numbering carries no title, and saying nothing beats showing
    /// "Episode 12" twice.
    pub fn episode_title(&self) -> Option<&str> {
        let title = self.title.as_deref()?.trim();
        let rest = match title.strip_prefix("Episode ") {
            Some(rest) => rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.'),
            None => title,
        };
        let rest = rest.trim_start_matches([' ', '-', '\u{2013}', '\u{2014}', ':']).trim();
        (!rest.is_empty()).then_some(rest)
    }
}

/// How one title relates to another.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Relation {
    pub id: AnilistId,
    #[serde(rename = "relationType")]
    pub relation_type: Option<String>,
    pub title: Title,
    pub format: Option<MediaFormat>,
}

impl Relation {
    /// Whether this relation is part of the main watch order, as opposed to a side story,
    /// adaptation or character cameo.
    pub fn is_watch_order(&self) -> bool {
        matches!(self.relation_type.as_deref(), Some("PREQUEL" | "SEQUEL" | "PARENT"))
    }
}

/// A title, as much of it as anistream renders.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Media {
    pub id: AnilistId,
    #[serde(default)]
    pub id_mal: Option<u32>,
    #[serde(default)]
    pub title: Title,
    #[serde(default)]
    pub format: Option<MediaFormat>,
    #[serde(default)]
    pub status: Option<MediaStatus>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub episodes: Option<u32>,
    #[serde(default)]
    pub duration: Option<u32>,
    #[serde(default)]
    pub season_year: Option<u16>,
    #[serde(default)]
    pub average_score: Option<u32>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub synonyms: Vec<String>,
    #[serde(default)]
    pub cover_image: CoverImage,
    #[serde(default)]
    pub banner_image: Option<String>,
    #[serde(default)]
    pub next_airing_episode: Option<NextAiring>,
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
    #[serde(default)]
    pub streaming_episodes: Vec<StreamingEpisode>,
    #[serde(default)]
    pub studios: StudioConnection,
}

/// Studios credited on a title. Requested with `isMain: true`, so in practice this holds the
/// animation studio rather than the production committee.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct StudioConnection {
    #[serde(default)]
    pub nodes: Vec<Studio>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Studio {
    pub name: String,
}

/// The most recent broadcast of one title.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LastAired {
    pub media_id: AnilistId,
    pub episode: u32,
    /// Unix seconds.
    pub airing_at: i64,
}

impl Media {
    /// The animation studio, when AniList credits one.
    pub fn main_studio(&self) -> Option<&str> {
        self.studios.nodes.first().map(|s| s.name.as_str())
    }

    /// Strip AniList's HTML description down to plain text for the terminal.
    ///
    /// Descriptions contain `<br>`, `<i>` and occasional `<a>`, none of which a TUI can
    /// render. Left in place they would show as literal tag soup in the synopsis panel.
    pub fn plain_description(&self) -> String {
        let Some(raw) = &self.description else {
            return String::new();
        };
        let mut out = String::with_capacity(raw.len());
        let mut in_tag = false;
        for ch in raw.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    // A closed tag was acting as a separator; keep one space so words do
                    // not run together where markup was doing the spacing.
                    if !out.ends_with(' ') && !out.is_empty() {
                        out.push(' ');
                    }
                }
                c if !in_tag => out.push(c),
                _ => {}
            }
        }
        out.replace("&amp;", "&")
            .replace("&quot;", "\"")
            .replace("&#039;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Every title string a provider search should try, widest first.
    pub fn search_titles(&self) -> Vec<String> {
        let mut titles = self.title.all();
        for synonym in &self.synonyms {
            if !synonym.is_empty() && !titles.contains(synonym) {
                titles.push(synonym.clone());
            }
        }
        titles
    }

    /// Streaming services that carry this title.
    pub fn streaming_services(&self) -> Vec<&ExternalLink> {
        self.external_links.iter().filter(|l| l.is_streaming()).collect()
    }

    /// How far the streaming listings are numbered ahead of *this* entry's episode one.
    ///
    /// Services number a sequel season continuously across the franchise — Dandadan's second
    /// season is listed as episodes 25–48 — while a release, and therefore a provider, calls
    /// its first episode one. Left uncorrected the two never line up and a sequel gets no
    /// titles at all, which is exactly how this was found.
    ///
    /// Applied only when the evidence is unambiguous: the entry knows how many episodes it
    /// has, the listings form a contiguous run of exactly that many, and that run starts past
    /// episode one. A long-running series numbered absolutely — One Piece, whose listings are
    /// a moving window and whose length AniList leaves unknown — matches none of that, so its
    /// already-correct numbers are left alone.
    fn listing_offset(&self) -> u32 {
        let mut numbers: Vec<u32> =
            self.streaming_episodes.iter().filter_map(StreamingEpisode::episode_number).collect();
        numbers.sort_unstable();
        numbers.dedup();

        let (Some(&first), Some(&last)) = (numbers.first(), numbers.last()) else {
            return 0;
        };
        if first <= 1 {
            return 0;
        }
        let Some(count) = self.episodes else {
            return 0;
        };
        let contiguous = numbers.len() as u32 == count && last.saturating_sub(first) + 1 == count;
        if contiguous { first - 1 } else { 0 }
    }

    /// Listings keyed by the episode numbers this entry actually uses.
    ///
    /// Numbers seen more than once keep the first entry: services list the same episode
    /// separately per region, and the duplicates say the same thing.
    fn listings_by_episode(&self) -> std::collections::BTreeMap<u32, &StreamingEpisode> {
        let offset = self.listing_offset();
        let mut by_episode = std::collections::BTreeMap::new();
        for listing in &self.streaming_episodes {
            if let Some(number) = listing.episode_number()
                && let Some(number) = number.checked_sub(offset).filter(|n| *n >= 1)
            {
                by_episode.entry(number).or_insert(listing);
            }
        }
        by_episode
    }

    /// Episode titles by number, as published in the streaming listings.
    ///
    /// A torrent source has no catalogue and cannot name an episode, so this is where the
    /// names come from.
    pub fn episode_titles(&self) -> std::collections::BTreeMap<u32, String> {
        self.listings_by_episode()
            .into_iter()
            .filter_map(|(number, listing)| Some((number, listing.episode_title()?.to_owned())))
            .collect()
    }

    /// Episode still frames by number, as published in the streaming listings.
    ///
    /// Coverage is uneven — the listings come from licensed services, so a simulcast usually
    /// has them and an older title often does not. The caller must treat a miss as normal.
    pub fn episode_thumbnails(&self) -> std::collections::BTreeMap<u32, String> {
        self.listings_by_episode()
            .into_iter()
            .filter_map(|(number, listing)| {
                let url = listing.thumbnail.as_deref().filter(|u| u.starts_with("http"))?;
                Some((number, url.to_owned()))
            })
            .collect()
    }

    /// Deep link for a specific episode, if a service published one.
    pub fn episode_link(&self, episode: u32) -> Option<&StreamingEpisode> {
        self.streaming_episodes.iter().find(|e| e.episode_number() == Some(episode))
    }

    /// Build the matching target used by the resolution ladder.
    pub fn match_target(&self) -> crate::title::MatchTarget {
        crate::title::MatchTarget {
            titles: self.search_titles(),
            episode_count: self.episodes,
            year: self.season_year,
            format: self.format,
        }
    }
}

#[cfg(test)]
mod title_tests {
    use super::*;

    fn listing(title: &str) -> StreamingEpisode {
        StreamingEpisode { title: Some(title.into()), url: None, site: None, thumbnail: None }
    }

    #[test]
    fn the_numbering_prefix_is_stripped_from_an_episode_title() {
        // The shapes actually seen in the listings.
        assert_eq!(listing("Episode 12 - A Real Hero").episode_title(), Some("A Real Hero"));
        assert_eq!(listing("Episode 1 – Pilot").episode_title(), Some("Pilot"));
        assert_eq!(listing("Episode 7: The Village").episode_title(), Some("The Village"));
    }

    #[test]
    fn an_entry_that_is_only_numbering_carries_no_title() {
        // Showing "Episode 12" beside the number 12 is worse than showing nothing.
        assert_eq!(listing("Episode 12").episode_title(), None);
        assert_eq!(listing("Episode 12 -").episode_title(), None);
        assert_eq!(listing("").episode_title(), None);
    }

    fn media_with(episodes: Option<u32>, numbers: &[u32]) -> Media {
        let listings: Vec<_> = numbers
            .iter()
            .map(|n| {
                serde_json::json!({
                    "title": format!("Episode {n} - Ep {n}"),
                    "thumbnail": format!("https://cdn.example/{n}.jpg"),
                })
            })
            .collect();
        serde_json::from_value(serde_json::json!({
            "id": 1,
            "episodes": episodes,
            "streamingEpisodes": listings,
        }))
        .expect("media fixture")
    }

    #[test]
    fn a_sequel_numbered_across_the_franchise_is_realigned_to_its_own_episode_one() {
        // Dandadan's second season is listed as 25–48; a release calls its first episode one.
        // Uncorrected the two never meet and the season gets no titles at all.
        let media = media_with(Some(24), &(25..=48).collect::<Vec<_>>());
        let titles = media.episode_titles();
        assert_eq!(titles.keys().next(), Some(&1));
        assert_eq!(titles.keys().next_back(), Some(&24));
        assert_eq!(titles[&1], "Ep 25", "episode one is the first listing, renumbered");
        assert_eq!(media.episode_thumbnails().len(), 24, "stills move with the titles");
    }

    #[test]
    fn a_first_season_is_left_exactly_as_listed() {
        let media = media_with(Some(28), &(1..=28).collect::<Vec<_>>());
        assert_eq!(media.episode_titles().keys().next(), Some(&1));
        assert_eq!(media.episode_titles().len(), 28);
    }

    #[test]
    fn an_absolutely_numbered_long_runner_keeps_its_numbers() {
        // One Piece: the listings are a moving window into a series whose length AniList
        // leaves unknown. Shifting those to start at one would corrupt correct data.
        let media = media_with(None, &(62..=130).collect::<Vec<_>>());
        assert_eq!(media.episode_titles().keys().next(), Some(&62));

        // Same shape, but the run does not match the stated length: still no guessing.
        let partial = media_with(Some(1000), &(62..=130).collect::<Vec<_>>());
        assert_eq!(partial.episode_titles().keys().next(), Some(&62));
    }

    #[test]
    fn a_gappy_listing_is_never_realigned() {
        // A run with holes is not evidence of an offset, it is evidence of missing entries.
        let media = media_with(Some(4), &[25, 26, 40, 41]);
        assert_eq!(media.episode_titles().keys().next(), Some(&25));
    }

    #[test]
    fn a_title_with_no_numbering_at_all_is_left_alone() {
        assert_eq!(listing("Frieren the Slayer").episode_title(), Some("Frieren the Slayer"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frieren() -> Media {
        // Shaped from a real graphql.anilist.co response captured during planning.
        serde_json::from_value(serde_json::json!({
            "id": 154587,
            "idMal": 52991,
            "title": {
                "romaji": "Sousou no Frieren",
                "english": "Frieren: Beyond Journey's End",
                "native": "葬送のフリーレン"
            },
            "format": "TV",
            "status": "FINISHED",
            "description": "<p>The mage Frieren...</p><br><i>(Source: Crunchyroll)</i>",
            "episodes": 28,
            "seasonYear": 2023,
            "averageScore": 91,
            "genres": ["Adventure", "Drama", "Fantasy"],
            "synonyms": ["Frieren at the Funeral"],
            "coverImage": {
                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx154587.jpg",
                "large": "https://s4.anilist.co/small.jpg"
            },
            "bannerImage": "https://s4.anilist.co/banner/154587.jpg",
            "nextAiringEpisode": null,
            "externalLinks": [
                {"site": "Crunchyroll", "url": "https://www.crunchyroll.com/series/GG5H5XQX4", "type": "STREAMING"},
                {"site": "Netflix", "url": "https://www.netflix.com/title/81726714", "type": "STREAMING"},
                {"site": "Official Site", "url": "https://frieren-anime.jp/", "type": "INFO"}
            ],
            "streamingEpisodes": [
                {"title": "Episode 1 - The Journey's End", "url": "https://www.crunchyroll.com/watch/G2XU04E88", "site": "Crunchyroll"},
                {"title": "Episode 2 - It Didn't Have to Be Magic", "url": "https://www.crunchyroll.com/watch/G8WUNGZ48", "site": "Crunchyroll"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_real_anilist_payload_deserialises() {
        let m = frieren();
        assert_eq!(m.id, AnilistId::new(154_587));
        assert_eq!(m.id_mal, Some(52_991));
        assert_eq!(m.episodes, Some(28));
        assert_eq!(m.format, Some(MediaFormat::Tv));
        assert_eq!(m.status, Some(MediaStatus::Finished));
    }

    #[test]
    fn missing_optional_fields_do_not_break_deserialisation() {
        // AniList omits fields freely depending on the query; a sparse response is normal
        // and must not fail the whole fetch.
        let m: Media = serde_json::from_value(serde_json::json!({"id": 1})).unwrap();
        assert_eq!(m.id, AnilistId::new(1));
        assert!(m.title.all().is_empty());
        assert_eq!(m.plain_description(), "");
        assert!(m.streaming_services().is_empty());
    }

    #[test]
    fn html_is_stripped_from_descriptions() {
        // Left in, this renders as literal tag soup in the synopsis panel.
        let text = frieren().plain_description();
        assert!(!text.contains('<'), "got: {text}");
        assert!(!text.contains("&amp;"));
        assert!(text.starts_with("The mage Frieren"));
        assert!(text.contains("(Source: Crunchyroll)"));
    }

    #[test]
    fn description_stripping_does_not_run_words_together() {
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "description": "first line<br>second line"
        }))
        .unwrap();
        assert_eq!(m.plain_description(), "first line second line");
    }

    #[test]
    fn display_title_prefers_english_with_romaji_as_subheading() {
        let m = frieren();
        assert_eq!(m.title.display(), "Frieren: Beyond Journey's End");
        assert_eq!(m.title.secondary(), Some("Sousou no Frieren"));
    }

    #[test]
    fn search_titles_include_romaji_english_native_and_synonyms() {
        // Feeding all of them to a provider is what bridges an english query to a
        // romaji-only catalogue.
        let titles = frieren().search_titles();
        assert!(titles.contains(&"Sousou no Frieren".to_string()));
        assert!(titles.contains(&"Frieren: Beyond Journey's End".to_string()));
        assert!(titles.contains(&"Frieren at the Funeral".to_string()));
        assert_eq!(titles[0], "Sousou no Frieren", "romaji first — release groups use it");
    }

    #[test]
    fn duplicate_titles_are_not_repeated() {
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "Bocchi", "english": "Bocchi"},
            "synonyms": ["Bocchi"]
        }))
        .unwrap();
        assert_eq!(m.search_titles(), vec!["Bocchi"]);
        assert_eq!(m.title.secondary(), None, "no subheading when both forms match");
    }

    #[test]
    fn streaming_links_are_separated_from_informational_ones() {
        let media = frieren();
        let services = media.streaming_services();
        assert_eq!(services.len(), 2);
        assert!(services.iter().all(|s| s.site != "Official Site"));
    }

    #[test]
    fn per_episode_deep_links_resolve_by_number() {
        // This is the licensed path: no auth, straight from AniList.
        let m = frieren();
        assert_eq!(
            m.episode_link(1).and_then(|e| e.url.as_deref()),
            Some("https://www.crunchyroll.com/watch/G2XU04E88")
        );
        assert_eq!(m.episode_link(2).unwrap().site.as_deref(), Some("Crunchyroll"));
        assert!(m.episode_link(99).is_none());
    }

    #[test]
    fn match_target_carries_the_gates_the_ladder_needs() {
        let t = frieren().match_target();
        assert_eq!(t.episode_count, Some(28));
        assert_eq!(t.year, Some(2023));
        assert_eq!(t.format, Some(MediaFormat::Tv));
        assert!(t.titles.len() >= 3);
    }

    #[test]
    fn cover_image_prefers_the_largest_available() {
        let m = frieren();
        assert!(m.cover_image.best().unwrap().contains("bx154587"));
        let small = CoverImage { extra_large: None, large: Some("l.jpg".into()), color: None };
        assert_eq!(small.best(), Some("l.jpg"));
        assert_eq!(CoverImage::default().best(), None);
    }

    #[test]
    fn watch_order_relations_exclude_side_material() {
        let mk = |kind: &str| Relation {
            id: AnilistId::new(1),
            relation_type: Some(kind.into()),
            title: Title::default(),
            format: None,
        };
        assert!(mk("SEQUEL").is_watch_order());
        assert!(mk("PREQUEL").is_watch_order());
        assert!(!mk("SIDE_STORY").is_watch_order());
        assert!(!mk("CHARACTER").is_watch_order());
    }
}
