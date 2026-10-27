//! AniList response types.
//!
//! Deliberately a hand-written subset rather than a generated schema binding. AniList's
//! schema is enormous and mostly irrelevant here; modelling only what the UI renders keeps
//! the surface small and makes a breaking upstream change show up as one failed field
//! rather than a wall of them.

use std::collections::BTreeMap;

use anistream_core::{
    ids::AnilistId,
    media::{MediaFormat, MediaStatus},
};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

use super::Season;

/// A field AniList may one day give a value this build does not know, read so that value is no
/// value rather than a failed title: one season too many must not blank the Title screen.
fn lenient<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| serde_json::from_value(value).ok()))
}

/// A date AniList may know only part of: often a year alone, or a year and a month, for a title
/// announced but not yet scheduled.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct FuzzyDate {
    #[serde(default)]
    pub year: Option<u16>,
    #[serde(default)]
    pub month: Option<u8>,
    #[serde(default)]
    pub day: Option<u8>,
}

/// What a title was adapted from, as AniList's `source(version: 3)` says.
///
/// Version 3, because the default version folds the newer kinds — a web novel, a live-action
/// film — into `OTHER`. A kind newer still reads as [`Self::Other`] rather than failing.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceMaterial {
    Original,
    Manga,
    LightNovel,
    VisualNovel,
    VideoGame,
    Novel,
    Doujinshi,
    Anime,
    WebNovel,
    LiveAction,
    Game,
    Comic,
    MultimediaProject,
    PictureBook,
    #[serde(other)]
    Other,
}

/// One of AniList's tags on a title: what it is about, by votes.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    #[serde(default)]
    pub name: String,
    /// How relevant the tag is to the title, out of 100.
    #[serde(default)]
    pub rank: Option<u32>,
    /// The tag gives away something of this title's story.
    #[serde(default)]
    pub is_media_spoiler: bool,
    /// The tag is a spoiler in general, whatever title it is on.
    #[serde(default)]
    pub is_general_spoiler: bool,
    #[serde(default)]
    pub category: Option<String>,
}

/// One episode's broadcast.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Broadcast {
    pub episode: u32,
    /// Unix seconds.
    pub airing_at: i64,
}

/// The wire shape of `airingSchedule { nodes { … } }`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct BroadcastConnection {
    #[serde(default)]
    pub nodes: Vec<Broadcast>,
}

/// Where a title ranks: `#3 highest rated 2026`, `#12 most popular all time`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Ranking {
    #[serde(default)]
    pub rank: Option<u32>,
    /// `RATED` or `POPULAR`.
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    /// AniList's own words for the list: `highest rated all time`.
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub all_time: bool,
    #[serde(default, deserialize_with = "lenient")]
    pub season: Option<Season>,
    #[serde(default)]
    pub year: Option<u16>,
}

/// A person as a title's cast and crew name them.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Person {
    #[serde(default)]
    pub name: PersonName,
    #[serde(default)]
    pub image: Option<PersonImage>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PersonName {
    #[serde(default)]
    pub full: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct PersonImage {
    #[serde(default)]
    pub medium: Option<String>,
}

/// The wire shape of `characters { edges { … } }`, main characters first.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct CharacterConnection {
    #[serde(default)]
    pub edges: Vec<CharacterEdge>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CharacterEdge {
    /// `MAIN`, `SUPPORTING` or `BACKGROUND`.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub node: Option<Person>,
    /// The character's Japanese voice actors, usually one.
    #[serde(default)]
    pub voice_actors: Vec<Person>,
}

/// The wire shape of `staff { edges { … } }`.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct StaffConnection {
    #[serde(default)]
    pub edges: Vec<StaffEdge>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct StaffEdge {
    /// What they did, in AniList's words: `Director`, `Original Creator`.
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub node: Option<Person>,
}

/// The title language a signed-in user reads AniList in. The stylised forms are AniList's own
/// capitalisation of the same title, and read here as the form they style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleLanguage {
    Romaji,
    English,
    Native,
}

impl TitleLanguage {
    /// AniList's `UserTitleLanguage`, or `None` for one this build does not know.
    pub fn from_wire(value: &str) -> Option<Self> {
        Some(match value {
            "ROMAJI" | "ROMAJI_STYLISED" => Self::Romaji,
            "ENGLISH" | "ENGLISH_STYLISED" => Self::English,
            "NATIVE" | "NATIVE_STYLISED" => Self::Native,
            _ => return None,
        })
    }
}

/// The signed-in user, as [`super::query::VIEWER`] asks for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    pub id: u32,
    pub name: Option<String>,
    /// `None` when AniList did not say, or said something this build does not know.
    pub title_language: Option<TitleLanguage>,
}

/// A title in the three forms AniList publishes.
///
/// All three matter: `romaji` is what torrent release groups use, `english` is what people
/// type, and `native` occasionally matches nothing else. The resolution ladder feeds all of
/// them to a provider search, which is why none is discarded.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

impl Title {
    /// The form to show in the UI: English if there is one, else romaji. What [`Self::shown`]
    /// gives for English.
    pub fn display(&self) -> &str {
        self.shown(TitleLanguage::English)
    }

    /// The form to show as a subheading, when it differs from [`Self::display`].
    pub fn secondary(&self) -> Option<&str> {
        self.beside(TitleLanguage::English)
    }

    /// The form to show when `language` is the one asked for: that form, else English, else
    /// romaji, else native. An empty form is no form.
    pub fn shown(&self, language: TitleLanguage) -> &str {
        let chosen = match language {
            TitleLanguage::Romaji => &self.romaji,
            TitleLanguage::English => &self.english,
            TitleLanguage::Native => &self.native,
        };
        [chosen, &self.english, &self.romaji, &self.native]
            .into_iter()
            .filter_map(|form| form.as_deref())
            .find(|form| !form.is_empty())
            .unwrap_or("Untitled")
    }

    /// The form to show beside [`Self::shown`] as a subheading: romaji under English or native,
    /// English under romaji. `None` when it would only repeat the title.
    pub fn beside(&self, language: TitleLanguage) -> Option<&str> {
        let shown = self.shown(language);
        [&self.romaji, &self.english]
            .into_iter()
            .filter_map(|form| form.as_deref())
            .find(|form| !form.is_empty() && *form != shown)
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

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

    /// The episode's own title, with AniList's `"Episode 12 - "` prefix removed: see
    /// [`unnumbered`].
    pub fn episode_title(&self) -> Option<&str> {
        unnumbered(self.title.as_deref()?)
    }
}

/// An episode's title without the numbering put in front of it, or `None` when the numbering is
/// all there is: showing "Episode 12" beside the number 12 is worse than showing nothing.
///
/// Titles are free text, from AniList's streaming listings and from sources alike, so this trims
/// the shapes actually seen and gives up rather than guessing: "Episode 12 - ", "Episode 1 – ",
/// "Episode 7: ", "Ep. 3: ", "EP03 - ", "Episode 12.5 - ". The word has to be followed by a
/// number, so a title that only starts with one ("Episodes of Spring", "Epilogue") is kept whole,
/// as is one that starts with a number of its own.
pub fn unnumbered(title: &str) -> Option<&str> {
    let title = title.trim();
    let lower = title.to_ascii_lowercase();
    let numbered = ["episode", "ep.", "ep"].into_iter().find_map(|word| {
        // The word's bytes are ASCII, so its length in the lowercased copy is its length here.
        let after = title.get(word.len()..)?.trim_start_matches([' ', '#']);
        let number = after.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.');
        let has_number = after.starts_with(|c: char| c.is_ascii_digit());
        (lower.starts_with(word) && has_number).then_some(number)
    });
    let rest = numbered
        .unwrap_or(title)
        .trim_start_matches([' ', '-', '\u{2013}', '\u{2014}', ':', '.', '|'])
        .trim();
    (!rest.is_empty()).then_some(rest)
}

/// How one title relates to another.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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

/// The wire shape of `relations { edges { relationType node { … } } }`.
///
/// Flattened into [`Relation`] by [`Media::watch_order`]; the edge/node split is
/// AniList's, not ours.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RelationConnection {
    #[serde(default)]
    pub edges: Vec<RelationEdge>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct RelationEdge {
    #[serde(rename = "relationType")]
    pub relation_type: Option<String>,
    pub node: RelationNode,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelationNode {
    pub id: AnilistId,
    #[serde(default)]
    pub title: Title,
    #[serde(default)]
    pub format: Option<MediaFormat>,
    /// The cover a card shows. Only `large` is asked for.
    #[serde(default)]
    pub cover_image: CoverImage,
    #[serde(default, deserialize_with = "lenient")]
    pub season: Option<Season>,
    #[serde(default)]
    pub season_year: Option<u16>,
    /// This title's own relations, one level further out, where the query asked for them: the
    /// title page's does, and so does the season walk's ([`super::query::relations`]). Each
    /// edge names its title by id and format only. `None` where they were not asked for, which
    /// says nothing of whether there are any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relations: Option<RelationConnection>,
}

/// A title, as much of it as anistream renders.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
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
    /// The season it premiered in.
    #[serde(default, deserialize_with = "lenient")]
    pub season: Option<Season>,
    #[serde(default)]
    pub start_date: Option<FuzzyDate>,
    #[serde(default)]
    pub end_date: Option<FuzzyDate>,
    #[serde(default)]
    pub source: Option<SourceMaterial>,
    #[serde(default)]
    pub is_adult: Option<bool>,
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
    /// How this title connects to others. Requested by `by_id`, so present on the detail
    /// fetch and empty on list fetches.
    #[serde(default)]
    pub relations: RelationConnection,
    /// What people who liked this rated highest. Same availability as `relations`.
    #[serde(default)]
    pub recommendations: RecommendationConnection,
    /// The promotional video, when AniList has one. Rides the detail fetch.
    #[serde(default)]
    pub trailer: Option<Trailer>,
    /// What the title is about, by votes. Rides the detail fetch.
    #[serde(default)]
    pub tags: Vec<Tag>,
    /// Every broadcast still to come, in episode order. Rides the detail fetch.
    #[serde(default)]
    pub upcoming: BroadcastConnection,
    /// The broadcasts already out, newest first, fifty at most. Not a field of AniList's
    /// `Media`: the detail fetch asks for it beside the title and puts it here, so the cache
    /// keeps it with the rest.
    #[serde(default)]
    pub aired: Vec<Broadcast>,
    /// Where the title ranks. Rides the detail fetch.
    #[serde(default)]
    pub rankings: Vec<Ranking>,
    /// The main and supporting cast, with their Japanese voices. Rides the detail fetch.
    #[serde(default)]
    pub characters: CharacterConnection,
    /// The people who made it, most relevant first. Rides the detail fetch.
    #[serde(default)]
    pub staff: StaffConnection,
}

/// A trailer reference: a video id on a host, not a playable URL.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Trailer {
    #[serde(default)]
    pub id: Option<String>,
    /// `youtube` or `dailymotion`, per AniList's docs.
    #[serde(default)]
    pub site: Option<String>,
}

/// The wire shape of `recommendations { nodes { mediaRecommendation { … } } }`.
///
/// The node reuses [`RelationNode`]: a recommendation row needs exactly what a relation
/// row needs — id, title, format — and a second identical struct would only drift.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct RecommendationConnection {
    #[serde(default)]
    pub nodes: Vec<RecommendationNode>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RecommendationNode {
    /// Null when the recommended title was deleted; skipped rather than surfaced.
    #[serde(default)]
    pub media_recommendation: Option<RelationNode>,
}

/// Studios credited on a title. Requested with `isMain: true`, so in practice this holds the
/// animation studio rather than the production committee.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct StudioConnection {
    #[serde(default)]
    pub nodes: Vec<Studio>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Studio {
    pub name: String,
}

/// A run of a description: plain text, and whether AniList marks it a spoiler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DescriptionSpan {
    pub text: String,
    pub spoiler: bool,
}

/// AniList's text cut at its spoiler marks, `~!` opening one and `!~` closing it: each run,
/// and whether it is a spoiler. Runs come as written, empty ones included, markup and all.
///
/// A spoiler left open runs to the end, hidden: guessing where it was meant to stop would show
/// what it was meant to hide. Descriptions and forum comments mark spoilers alike.
pub fn spoiler_runs(raw: &str) -> Vec<(&str, bool)> {
    let mut runs = Vec::new();
    let mut rest = raw;
    let mut spoiler = false;
    loop {
        let marker = if spoiler { "!~" } else { "~!" };
        match rest.find(marker) {
            Some(at) => {
                runs.push((&rest[..at], spoiler));
                rest = &rest[at + marker.len()..];
                spoiler = !spoiler;
            }
            None => {
                runs.push((rest, spoiler));
                return runs;
            }
        }
    }
}

/// AniList's HTML as plain text: tags out, the entities it uses put back, every run of
/// whitespace one space.
fn plain_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                // A closed tag was acting as a separator; keep one space so words do not run
                // together where markup was doing the spacing.
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

/// The most recent broadcast of one title.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
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

    /// The titles adjacent to this one in the main watch order: prequels first, then the
    /// parent story, then sequels. Side stories, adaptations and cameos are filtered out
    /// — the question this answers is "what do I watch before and after", not "what else
    /// exists in this franchise".
    pub fn watch_order(&self) -> Vec<Relation> {
        let mut related: Vec<Relation> = self
            .relations
            .edges
            .iter()
            .map(|edge| Relation {
                id: edge.node.id,
                relation_type: edge.relation_type.clone(),
                title: edge.node.title.clone(),
                format: edge.node.format,
            })
            .filter(Relation::is_watch_order)
            .collect();
        let rank = |r: &Relation| match r.relation_type.as_deref() {
            Some("PREQUEL") => 0,
            Some("PARENT") => 1,
            _ => 2,
        };
        related.sort_by_key(rank);
        related
    }

    /// Titles recommended off this one, best-rated first — AniList's order, kept.
    ///
    /// Distinct from [`Self::watch_order`]: that answers "what comes before and after",
    /// this answers "what else is like it". A deleted recommendation arrives as a null
    /// node and is skipped, and so is anything that is not an anime format — opening a
    /// recommended manga would dead-end, since every detail fetch asks for `type: ANIME`.
    pub fn recommended(&self) -> Vec<&RelationNode> {
        self.recommendations
            .nodes
            .iter()
            .filter_map(|n| n.media_recommendation.as_ref())
            .filter(|node| !matches!(node.format, Some(MediaFormat::Unknown)))
            .collect()
    }

    /// The trailer as a watchable URL, when the id and a known host are both present.
    ///
    /// An unknown host returns `None` rather than a guessed URL that opens a 404.
    pub fn trailer_url(&self) -> Option<String> {
        let trailer = self.trailer.as_ref()?;
        let id = trailer.id.as_deref().filter(|id| !id.is_empty())?;
        match trailer.site.as_deref() {
            Some("youtube") => Some(format!("https://www.youtube.com/watch?v={id}")),
            Some("dailymotion") => Some(format!("https://www.dailymotion.com/video/{id}")),
            _ => None,
        }
    }

    /// Strip AniList's HTML description down to plain text for the terminal, with what AniList
    /// marks a spoiler left out: [`Self::description_spans`]' text that is not a spoiler, a
    /// space between.
    ///
    /// Descriptions contain `<br>`, `<i>` and occasional `<a>`, none of which a TUI can
    /// render. Left in place they would show as literal tag soup in the synopsis panel. And
    /// `~!…!~` around a spoiler, which showed as it was, spoiler and markers both.
    pub fn plain_description(&self) -> String {
        let shown: Vec<String> = self
            .description_spans()
            .into_iter()
            .filter(|span| !span.spoiler)
            .map(|span| span.text)
            .collect();
        shown.join(" ")
    }

    /// The description as plain text, in runs, each marked whether AniList's `~!…!~` says it
    /// gives the story away. A front end shows the runs one after another with a space between,
    /// and hides the spoilers.
    ///
    /// A spoiler left open runs to the end, hidden: guessing where it was meant to stop would
    /// show what it was meant to hide.
    pub fn description_spans(&self) -> Vec<DescriptionSpan> {
        let Some(raw) = &self.description else { return Vec::new() };
        spoiler_runs(raw)
            .into_iter()
            .map(|(run, spoiler)| DescriptionSpan { text: plain_text(run), spoiler })
            .filter(|span| !span.text.is_empty())
            .collect()
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
    /// Services number a sequel season continuously across the franchise — a second season of
    /// twenty-four is listed as episodes 25–48 — while a release, and therefore a provider,
    /// calls its first episode one. Left uncorrected the two never line up and a sequel gets no
    /// titles at all, which is exactly how this was found.
    ///
    /// Applied only when the evidence is unambiguous: the entry knows how many episodes it
    /// has, the listings form a contiguous run of exactly that many, and that run starts past
    /// episode one. A long-running series numbered absolutely — one past its thousandth
    /// episode, whose listings are a moving window and whose length AniList leaves unknown —
    /// matches none of that, so its already-correct numbers are left alone.
    fn listing_offset(&self) -> u32 {
        let mut numbers: Vec<u32> = self
            .streaming_episodes
            .iter()
            .filter_map(StreamingEpisode::episode_number)
            .collect();
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
        let contiguous =
            numbers.len() as u32 == count && last.saturating_sub(first) + 1 == count;
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

    /// When each episode airs or aired, by number, from every broadcast the title carries: the
    /// ones already out, the ones to come, and the next one, which a list fetch has too.
    ///
    /// A long runner's oldest broadcasts are not here — only its newest fifty — so a missing
    /// episode means not known, never not aired.
    pub fn air_times(&self) -> BTreeMap<u32, i64> {
        let mut times = BTreeMap::new();
        let next = self
            .next_airing_episode
            .map(|n| Broadcast { episode: n.episode, airing_at: n.airing_at });
        for broadcast in self.aired.iter().chain(&self.upcoming.nodes).chain(next.as_ref()) {
            times.insert(broadcast.episode, broadcast.airing_at);
        }
        times
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
        assert_eq!(
            listing("Episode 12 - The Sky Archive").episode_title(),
            Some("The Sky Archive")
        );
        assert_eq!(listing("Episode 1 – Pilot").episode_title(), Some("Pilot"));
        assert_eq!(listing("Episode 7: The Village").episode_title(), Some("The Village"));
    }

    #[test]
    fn every_shape_of_numbering_is_stripped_and_nothing_else() {
        assert_eq!(unnumbered("Ep. 3: The Ferryman's Ledger"), Some("The Ferryman's Ledger"));
        assert_eq!(unnumbered("EP03 - Embers in the Snow"), Some("Embers in the Snow"));
        assert_eq!(unnumbered("ep 4 | A Letter Never Sent"), Some("A Letter Never Sent"));
        assert_eq!(
            unnumbered("Episode #9 - The Last Lamplighter"),
            Some("The Last Lamplighter")
        );
        assert_eq!(unnumbered("Episode 12.5 - Looking Back"), Some("Looking Back"));
        assert_eq!(unnumbered("  Episode 2 — Overdue  "), Some("Overdue"));
        assert_eq!(unnumbered("Ep. 3"), None, "the numbering alone");
        // A word that only looks like the numbering, and a title's own number, are the title.
        assert_eq!(unnumbered("Episodes of Spring"), Some("Episodes of Spring"));
        assert_eq!(unnumbered("Epilogue"), Some("Epilogue"));
        assert_eq!(unnumbered("Episode Zero"), Some("Episode Zero"));
        assert_eq!(unnumbered("100 Lanterns"), Some("100 Lanterns"));
        assert_eq!(listing("Ep. 3: The Village").episode_title(), Some("The Village"));
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
        // A second season of 24 is listed as 25–48; a release calls its first episode one.
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
        // A long runner: the listings are a moving window into a series whose length AniList
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
        assert_eq!(
            listing("Where the Roads Cross").episode_title(),
            Some("Where the Roads Cross")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hoshikuzu() -> Media {
        // Shaped from a real graphql.anilist.co response captured during planning.
        serde_json::from_value(serde_json::json!({
            "id": 154587,
            "idMal": 52991,
            "title": {
                "romaji": "Hoshikuzu no Tabibito",
                "english": "Stardust Wayfarer",
                "native": "星屑の旅人"
            },
            "format": "TV",
            "status": "FINISHED",
            "description": "<p>The lamplighter walks on...</p><br><i>(Source: Hikari Stream)</i>",
            "episodes": 28,
            "seasonYear": 2023,
            "averageScore": 91,
            "genres": ["Adventure", "Drama", "Fantasy"],
            "synonyms": ["Hoshitabi"],
            "coverImage": {
                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx154587.jpg",
                "large": "https://s4.anilist.co/small.jpg"
            },
            "bannerImage": "https://s4.anilist.co/banner/154587.jpg",
            "nextAiringEpisode": null,
            "externalLinks": [
                {"site": "Hikari Stream", "url": "https://hikari.example/series/GG5H5XQX4", "type": "STREAMING"},
                {"site": "Lantern Stream", "url": "https://stream.example/title/81726714", "type": "STREAMING"},
                {"site": "Official Site", "url": "https://hoshikuzu.example/", "type": "INFO"}
            ],
            "streamingEpisodes": [
                {"title": "Episode 1 - The Lantern Road", "url": "https://hikari.example/watch/G2XU04E88", "site": "Hikari Stream"},
                {"title": "Episode 2 - A Town Without Night", "url": "https://hikari.example/watch/G8WUNGZ48", "site": "Hikari Stream"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_real_anilist_payload_deserialises() {
        let m = hoshikuzu();
        assert_eq!(m.id, AnilistId::new(154_587));
        assert_eq!(m.id_mal, Some(52_991));
        assert_eq!(m.episodes, Some(28));
        assert_eq!(m.format, Some(MediaFormat::Tv));
        assert_eq!(m.status, Some(MediaStatus::Finished));
    }

    #[test]
    fn a_manga_relation_does_not_break_the_title() {
        // Relations ride the detail query, and source material is routinely MANGA or
        // NOVEL — formats the anime enum does not model. One related manga must never
        // fail the whole title. Regression: `unknown variant MANGA` on every detail open.
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "X"},
            "relations": {"edges": [
                {"relationType": "ADAPTATION",
                 "node": {"id": 2, "title": {"romaji": "The Manga"}, "format": "MANGA"}},
                {"relationType": "SEQUEL",
                 "node": {"id": 3, "title": {"romaji": "Season 2"}, "format": "TV"}},
            ]}
        }))
        .unwrap();
        let order = m.watch_order();
        assert_eq!(order.len(), 1, "adaptations are not watch order");
        assert_eq!(order[0].id, AnilistId::new(3));
    }

    #[test]
    fn recommendations_deserialise_and_skip_what_cannot_be_watched() {
        // A deleted recommendation arrives as a null node and must not fail the fetch.
        // A MANGA one parses — `#[serde(other)]` — but opening it would dead-end on
        // `type: ANIME`, so it is dropped rather than offered.
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "X"},
            "recommendations": {"nodes": [
                {"mediaRecommendation": {"id": 7, "title": {"romaji": "Like It"}, "format": "TV"}},
                {"mediaRecommendation": null},
                {"mediaRecommendation": {"id": 9, "title": {"romaji": "Read It"}, "format": "MANGA"}},
            ]}
        }))
        .unwrap();
        let recs = m.recommended();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].id, AnilistId::new(7));
    }

    #[test]
    fn a_trailer_becomes_a_url_only_on_a_known_host() {
        let with = |site: serde_json::Value| -> Media {
            serde_json::from_value(serde_json::json!({
                "id": 1,
                "trailer": {"id": "abc123", "site": site}
            }))
            .unwrap()
        };
        assert_eq!(
            with("youtube".into()).trailer_url().as_deref(),
            Some("https://www.youtube.com/watch?v=abc123")
        );
        assert_eq!(
            with("dailymotion".into()).trailer_url().as_deref(),
            Some("https://www.dailymotion.com/video/abc123")
        );
        // An unknown host must not become a guessed URL that opens a 404.
        assert_eq!(with("bilibili".into()).trailer_url(), None);
        assert_eq!(with(serde_json::Value::Null).trailer_url(), None);
    }

    /// A title page's answer with every field the richer detail asks for, and the broadcasts
    /// already out as the client puts them on the title.
    fn detailed() -> Media {
        serde_json::from_value(serde_json::json!({
            "id": 900001,
            "title": {"romaji": "Hoshikuzu no Tabibito", "english": "Stardust Wayfarer"},
            "season": "SPRING",
            "startDate": {"year": 2024, "month": 4, "day": 6},
            "endDate": {"year": null, "month": null, "day": null},
            "source": "MANGA",
            "isAdult": false,
            "nextAiringEpisode": {"episode": 4, "airingAt": 3_000, "timeUntilAiring": 10},
            "tags": [
                {"name": "Travel", "rank": 94, "isMediaSpoiler": false,
                 "isGeneralSpoiler": false, "category": "Theme-Other"},
                {"name": "The Last Lantern", "rank": 58, "isMediaSpoiler": true,
                 "isGeneralSpoiler": false, "category": "Theme-Drama"}
            ],
            "upcoming": {"nodes": [{"episode": 4, "airingAt": 3_000}, {"episode": 5, "airingAt": 4_000}]},
            "aired": [{"episode": 3, "airingAt": 2_000}, {"episode": 2, "airingAt": 1_000}],
            "rankings": [{"rank": 4, "type": "RATED", "context": "highest rated",
                          "allTime": false, "season": "SPRING", "year": 2024}],
            "characters": {"edges": [{
                "role": "MAIN",
                "node": {"name": {"full": "Ren Amagiri"}, "image": {"medium": "https://img.example/people/ren.jpg"}},
                "voiceActors": [{"name": {"full": "Ren's voice"}, "image": null}]
            }]},
            "staff": {"edges": [{"role": "Director", "node": {"name": {"full": "Director of Stardust Wayfarer"}}}]},
            "relations": {"edges": [{"relationType": "SEQUEL", "node": {
                "id": 900004, "title": {"romaji": "Hoshikuzu no Tabibito 2nd Season"}, "format": "TV",
                "coverImage": {"large": "https://img.example/cover/900004.jpg"},
                "season": "WINTER", "seasonYear": 2025
            }}]}
        }))
        .unwrap()
    }

    #[test]
    fn the_title_pages_richer_fields_deserialise() {
        let m = detailed();
        assert_eq!(m.season, Some(Season::Spring));
        assert_eq!(
            m.start_date,
            Some(FuzzyDate { year: Some(2024), month: Some(4), day: Some(6) })
        );
        assert_eq!(m.end_date, Some(FuzzyDate::default()), "an unknown end is all nulls");
        assert_eq!(m.source, Some(SourceMaterial::Manga));
        assert_eq!(m.is_adult, Some(false));
        assert_eq!(m.tags.len(), 2);
        assert!(m.tags[1].is_media_spoiler);
        assert_eq!(m.rankings[0].kind.as_deref(), Some("RATED"));
        assert_eq!(m.rankings[0].season, Some(Season::Spring));
        let cast = &m.characters.edges[0];
        assert_eq!(cast.role.as_deref(), Some("MAIN"));
        assert_eq!(cast.voice_actors[0].name.full.as_deref(), Some("Ren's voice"));
        assert_eq!(m.staff.edges[0].role.as_deref(), Some("Director"));
        let sequel = &m.relations.edges[0].node;
        assert_eq!(sequel.cover_image.best(), Some("https://img.example/cover/900004.jpg"));
        assert_eq!((sequel.season, sequel.season_year), (Some(Season::Winter), Some(2025)));
    }

    #[test]
    fn a_kind_or_season_anilist_adds_later_does_not_break_the_title() {
        // A season this build has never heard of reads as none; a source kind as other. Either
        // failing would blank the Title screen over one field.
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "season": "MONSOON",
            "source": "SOMETHING_NEW",
            "rankings": [{"rank": 1, "type": "TRENDING", "season": "MONSOON"}]
        }))
        .unwrap();
        assert_eq!(m.season, None);
        assert_eq!(m.source, Some(SourceMaterial::Other));
        assert_eq!(m.rankings[0].season, None);
    }

    #[test]
    fn air_times_gather_every_broadcast_by_episode() {
        // What is out, what is to come, and the next one, which a list fetch carries alone.
        let times = detailed().air_times();
        assert_eq!(
            times.into_iter().collect::<Vec<_>>(),
            [(2, 1_000), (3, 2_000), (4, 3_000), (5, 4_000)]
        );

        let listed: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "nextAiringEpisode": {"episode": 7, "airingAt": 9_000, "timeUntilAiring": 5}
        }))
        .unwrap();
        assert_eq!(listed.air_times().into_iter().collect::<Vec<_>>(), [(7, 9_000)]);
    }

    #[test]
    fn the_richer_fields_survive_the_cache_round_trip() {
        // What the cache recalls during an outage is what the Title screen showed.
        let m = detailed();
        let back: Media = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        assert_eq!(back.aired.len(), 2, "the broadcasts put on the title are kept with it");
    }

    #[test]
    fn a_stylised_title_language_reads_as_the_form_it_styles() {
        assert_eq!(TitleLanguage::from_wire("ROMAJI"), Some(TitleLanguage::Romaji));
        assert_eq!(TitleLanguage::from_wire("ENGLISH_STYLISED"), Some(TitleLanguage::English));
        assert_eq!(TitleLanguage::from_wire("NATIVE_STYLISED"), Some(TitleLanguage::Native));
        assert_eq!(TitleLanguage::from_wire("SOMETHING_NEW"), None);
    }

    #[test]
    fn a_media_survives_the_cache_round_trip() {
        // The media cache stores serialized Media; a lossy round trip would mean the
        // fallback path silently shows different data than the fresh one did.
        let m = hoshikuzu();
        let json = serde_json::to_string(&m).expect("serialize");
        let back: Media = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, m);
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
        let text = hoshikuzu().plain_description();
        assert!(!text.contains('<'), "got: {text}");
        assert!(!text.contains("&amp;"));
        assert!(text.starts_with("The lamplighter"));
        assert!(text.contains("(Source: Hikari Stream)"));
    }

    #[test]
    fn a_spoiler_is_its_own_run_and_left_out_of_the_plain_text() {
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "description": "The lamplighter walks on.<br><br>~!She was the lantern all along.!~ \
                            The road goes <i>on</i>."
        }))
        .unwrap();
        let spans: Vec<_> =
            m.description_spans().into_iter().map(|s| (s.text, s.spoiler)).collect();
        assert_eq!(
            spans,
            [
                ("The lamplighter walks on.".into(), false),
                ("She was the lantern all along.".into(), true),
                ("The road goes on .".into(), false),
            ]
        );
        assert_eq!(
            m.plain_description(),
            "The lamplighter walks on. The road goes on .",
            "no spoiler, and no markers"
        );

        // Left open, the rest is the spoiler: never shown for want of a closing mark.
        let open: Media = serde_json::from_value(serde_json::json!({
            "id": 1, "description": "Before. ~!The twist, and the end"
        }))
        .unwrap();
        assert_eq!(open.plain_description(), "Before.");
        assert_eq!(open.description_spans().last().map(|s| s.spoiler), Some(true));
        assert!(hoshikuzu().description_spans().iter().all(|s| !s.spoiler), "none marked");
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
        let m = hoshikuzu();
        assert_eq!(m.title.display(), "Stardust Wayfarer");
        assert_eq!(m.title.secondary(), Some("Hoshikuzu no Tabibito"));
    }

    #[test]
    fn search_titles_include_romaji_english_native_and_synonyms() {
        // Feeding all of them to a provider is what bridges an english query to a
        // romaji-only catalogue.
        let titles = hoshikuzu().search_titles();
        assert!(titles.contains(&"Hoshikuzu no Tabibito".to_string()));
        assert!(titles.contains(&"Stardust Wayfarer".to_string()));
        assert!(titles.contains(&"Hoshitabi".to_string()));
        assert_eq!(titles[0], "Hoshikuzu no Tabibito", "romaji first — release groups use it");
    }

    #[test]
    fn duplicate_titles_are_not_repeated() {
        let m: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "Tomoshibi", "english": "Tomoshibi"},
            "synonyms": ["Tomoshibi"]
        }))
        .unwrap();
        assert_eq!(m.search_titles(), vec!["Tomoshibi"]);
        assert_eq!(m.title.secondary(), None, "no subheading when both forms match");
    }

    #[test]
    fn streaming_links_are_separated_from_informational_ones() {
        let media = hoshikuzu();
        let services = media.streaming_services();
        assert_eq!(services.len(), 2);
        assert!(services.iter().all(|s| s.site != "Official Site"));
    }

    #[test]
    fn per_episode_deep_links_resolve_by_number() {
        // This is the licensed path: no auth, straight from AniList.
        let m = hoshikuzu();
        assert_eq!(
            m.episode_link(1).and_then(|e| e.url.as_deref()),
            Some("https://hikari.example/watch/G2XU04E88")
        );
        assert_eq!(m.episode_link(2).unwrap().site.as_deref(), Some("Hikari Stream"));
        assert!(m.episode_link(99).is_none());
    }

    #[test]
    fn match_target_carries_the_gates_the_ladder_needs() {
        let t = hoshikuzu().match_target();
        assert_eq!(t.episode_count, Some(28));
        assert_eq!(t.year, Some(2023));
        assert_eq!(t.format, Some(MediaFormat::Tv));
        assert!(t.titles.len() >= 3);
    }

    #[test]
    fn cover_image_prefers_the_largest_available() {
        let m = hoshikuzu();
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
