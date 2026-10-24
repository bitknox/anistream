//! Search and browse: the one query every front end builds, the words its filters are shown in,
//! and what Search shows before anything is asked.
//!
//! A [`SearchQuery`] is the whole of a search: the text, if any, the sort and every filter. With
//! no text it is a browse, and with nothing set at all it is Search's start, where the engine's
//! [`BrowseRail`]s stand in for results. Each rail is itself a query, so opening one as a grid is
//! a search like any other.
//!
//! The words are here rather than in each front end, so a chip, a recent search and an empty
//! state say the same thing everywhere: [`SearchQuery::active`] lists what is set, each with the
//! key that takes it off again ([`SearchQuery::without`]).

use crate::catalog::{AiringStatus, Entry, Season, SourceMaterial};

/// The order results come in. A query with no sort of its own is sorted by
/// [`SearchSort::Relevance`] when it has text and by [`SearchSort::Popularity`] when it has none:
/// see [`SearchQuery::sort_in_effect`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SearchSort {
    /// How well a title matches the text. Only with text: a browse has nothing to match.
    Relevance,
    Trending,
    Popularity,
    Score,
    Newest,
    Oldest,
    Title,
    Favourites,
}

impl SearchSort {
    /// Every sort, in the order a menu lists them.
    pub const ALL: [Self; 8] = [
        Self::Relevance,
        Self::Trending,
        Self::Popularity,
        Self::Score,
        Self::Newest,
        Self::Oldest,
        Self::Title,
        Self::Favourites,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Relevance => "Relevance",
            Self::Trending => "Trending",
            Self::Popularity => "Popularity",
            Self::Score => "Score",
            Self::Newest => "Newest",
            Self::Oldest => "Oldest",
            Self::Title => "Title",
            Self::Favourites => "Favourites",
        }
    }
}

/// What kind of release a title is, as AniList files anime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MediaFormat {
    Tv,
    TvShort,
    Movie,
    Special,
    Ova,
    Ona,
    Music,
}

impl MediaFormat {
    pub const ALL: [Self; 7] = [
        Self::Tv,
        Self::TvShort,
        Self::Movie,
        Self::Special,
        Self::Ova,
        Self::Ona,
        Self::Music,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Tv => "TV",
            Self::TvShort => "TV short",
            Self::Movie => "Movie",
            Self::Special => "Special",
            Self::Ova => "OVA",
            Self::Ona => "ONA",
            Self::Music => "Music",
        }
    }
}

/// The statuses a search offers, in the order a menu lists them.
pub const SEARCH_STATUSES: [AiringStatus; 5] = [
    AiringStatus::Releasing,
    AiringStatus::Finished,
    AiringStatus::NotYetReleased,
    AiringStatus::Hiatus,
    AiringStatus::Cancelled,
];

/// A status as a filter names it.
pub const fn status_label(status: AiringStatus) -> &'static str {
    match status {
        AiringStatus::Releasing => "Airing",
        AiringStatus::Finished => "Finished",
        AiringStatus::NotYetReleased => "Not yet aired",
        AiringStatus::Cancelled => "Cancelled",
        AiringStatus::Hiatus => "On hiatus",
    }
}

pub const fn season_label(season: Season) -> &'static str {
    match season {
        Season::Winter => "Winter",
        Season::Spring => "Spring",
        Season::Summer => "Summer",
        Season::Fall => "Fall",
    }
}

/// The source materials a search offers, the common ones first.
pub const SEARCH_SOURCES: [SourceMaterial; 14] = [
    SourceMaterial::Original,
    SourceMaterial::Manga,
    SourceMaterial::LightNovel,
    SourceMaterial::WebNovel,
    SourceMaterial::Novel,
    SourceMaterial::VisualNovel,
    SourceMaterial::VideoGame,
    SourceMaterial::Game,
    SourceMaterial::Comic,
    SourceMaterial::Doujinshi,
    SourceMaterial::Anime,
    SourceMaterial::LiveAction,
    SourceMaterial::MultimediaProject,
    SourceMaterial::PictureBook,
];

pub const fn source_label(source: SourceMaterial) -> &'static str {
    match source {
        SourceMaterial::Original => "Original",
        SourceMaterial::Manga => "Manga",
        SourceMaterial::LightNovel => "Light novel",
        SourceMaterial::VisualNovel => "Visual novel",
        SourceMaterial::VideoGame => "Video game",
        SourceMaterial::Novel => "Novel",
        SourceMaterial::Doujinshi => "Doujinshi",
        SourceMaterial::Anime => "Anime",
        SourceMaterial::WebNovel => "Web novel",
        SourceMaterial::LiveAction => "Live action",
        SourceMaterial::Game => "Game",
        SourceMaterial::Comic => "Comic",
        SourceMaterial::MultimediaProject => "Multimedia project",
        SourceMaterial::PictureBook => "Picture book",
        SourceMaterial::Other => "Other",
    }
}

/// The countries of origin a search offers, as ISO codes.
pub const SEARCH_COUNTRIES: [&str; 4] = ["JP", "CN", "KR", "TW"];

/// A country of origin by name, or its code when it is not one of [`SEARCH_COUNTRIES`].
pub fn country_label(code: &str) -> String {
    match code {
        "JP" => "Japan".into(),
        "CN" => "China".into(),
        "KR" => "Korea".into(),
        "TW" => "Taiwan".into(),
        other => other.into(),
    }
}

/// A range, either end open. `min == max` is one value.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[serde(default)]
pub struct Bounds {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub min: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub max: Option<u32>,
}

impl Bounds {
    pub const fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// The one value both ends name, when they name the same one.
    pub fn single(&self) -> Option<u32> {
        (self.min.is_some() && self.min == self.max).then_some(self.min).flatten()
    }

    /// Both ends in order, so a range typed backwards still means what it says.
    pub fn ordered(self) -> Self {
        match (self.min, self.max) {
            (Some(lo), Some(hi)) if lo > hi => Self { min: Some(hi), max: Some(lo) },
            _ => self,
        }
    }

    /// "12 to 26", "Up to 12", "50 or more", with `unit` after the numbers.
    fn label(self, unit: &str) -> Option<String> {
        let unit = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        match (self.min, self.max) {
            (Some(lo), Some(hi)) if lo == hi => Some(format!("{lo}{unit}")),
            (Some(lo), Some(hi)) => Some(format!("{lo} to {hi}{unit}")),
            (Some(lo), None) => Some(format!("{lo}{unit} or more")),
            (None, Some(hi)) => Some(format!("Up to {hi}{unit}")),
            (None, None) => None,
        }
    }
}

/// Whether a search keeps to the titles on your list, or leaves them out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum OnList {
    /// Leave out every title already on one of your lists.
    Hide,
    /// Only the titles on your lists.
    Only,
}

/// A genre or a tag in a query: not asked about, wanted, or ruled out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Pick {
    Off,
    Include,
    Exclude,
}

/// One search: the text, the sort and every filter, each unset by default.
///
/// The same value goes out as a command and comes back on the answer, so a slow answer to an
/// earlier query is told apart from the one on screen by comparing the two.
///
/// Adult titles are not a filter here. Whether they appear is a setting, which the engine
/// applies to every query.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
#[serde(default)]
pub struct SearchQuery {
    /// What is typed. Matches romaji, English and native titles and synonyms.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub text: String,
    /// `None` is the default: relevance with text, popularity without.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sort: Option<SearchSort>,
    /// Every one of these, as AniList names them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres_excluded: Vec<String>,
    /// Every one of these.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tags: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tags_excluded: Vec<String>,
    /// How strongly a tag must apply, 0 to 100. `None` is [`DEFAULT_TAG_RANK`] when tags are
    /// set.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub min_tag_rank: Option<u8>,
    /// Any of these.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub formats: Vec<MediaFormat>,
    /// Any of these.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub statuses: Vec<AiringStatus>,
    /// The season a title premiered in; with a single year in [`Self::years`], that year's.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub season: Option<Season>,
    /// The years a title started in.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub years: Bounds,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: Bounds,
    /// Minutes per episode.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub minutes: Bounds,
    /// AniList's average score, 0 to 100, at least this.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub min_score: Option<u8>,
    /// How many people have a title on a list, at least this: what "hide obscure titles" sets.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub min_popularity: Option<u32>,
    /// Any of these.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sources: Vec<SourceMaterial>,
    /// Any of these, as ISO codes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub countries: Vec<String>,
    /// Officially streaming on any of these sites, as AniList names them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub streaming_on: Vec<String>,
    /// Only titles with an official licence somewhere.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub licensed_only: bool,
    /// Your lists, kept to or left out. Needs AniList signed in; ignored otherwise.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub on_list: Option<OnList>,
}

/// The tag rank a query with tags asks for when it names none.
pub const DEFAULT_TAG_RANK: u8 = 60;

/// What "hide obscure titles" asks for: at least this many people with the title on a list.
pub const OBSCURE_BELOW: u32 = 1_000;

/// Which filter of a query an [`ActiveFilter`] is, to take it off with [`SearchQuery::without`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FilterKey {
    Genre { name: String },
    NotGenre { name: String },
    Tag { name: String },
    NotTag { name: String },
    TagRank,
    Years,
    Premiered,
    Formats,
    Statuses,
    Episodes,
    Minutes,
    MinScore,
    Obscure,
    Sources,
    Countries,
    Streaming,
    Licensed,
    MyList,
}

/// One filter that is set, in the words a chip shows: "Mecha", "Not Romance", "2010 to 2015",
/// "TV, Movie".
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ActiveFilter {
    pub key: FilterKey,
    pub label: String,
}

fn joined<T: Copy>(items: &[T], label: impl Fn(T) -> &'static str) -> String {
    items.iter().map(|item| label(*item)).collect::<Vec<_>>().join(", ")
}

/// `items` with `item` in it or out of it, keeping the order the rest were set in.
fn toggled<T: PartialEq + Clone>(items: &[T], item: &T) -> Vec<T> {
    if items.contains(item) {
        items.iter().filter(|i| *i != item).cloned().collect()
    } else {
        let mut next = items.to_vec();
        next.push(item.clone());
        next
    }
}

fn pick_in(included: &[String], excluded: &[String], name: &str) -> Pick {
    if included.iter().any(|g| g == name) {
        Pick::Include
    } else if excluded.iter().any(|g| g == name) {
        Pick::Exclude
    } else {
        Pick::Off
    }
}

fn picked(included: &mut Vec<String>, excluded: &mut Vec<String>, name: &str, pick: Pick) {
    included.retain(|g| g != name);
    excluded.retain(|g| g != name);
    match pick {
        Pick::Include => included.push(name.to_owned()),
        Pick::Exclude => excluded.push(name.to_owned()),
        Pick::Off => {}
    }
}

impl SearchQuery {
    /// A search for `text`, with nothing else set.
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    /// A browse sorted by `sort`, with nothing else set.
    pub fn sorted(sort: SearchSort) -> Self {
        Self { sort: Some(sort), ..Self::default() }
    }

    /// One season of one year, most popular first: Seasonal, and the rails for this season and
    /// the next.
    pub fn season(season: Season, year: u16) -> Self {
        Self {
            season: Some(season),
            years: Bounds { min: Some(u32::from(year)), max: Some(u32::from(year)) },
            sort: Some(SearchSort::Popularity),
            ..Self::default()
        }
    }

    /// The text without the spaces around it, which is what is searched for.
    pub fn trimmed_text(&self) -> &str {
        self.text.trim()
    }

    /// No text: a browse.
    pub fn is_browse(&self) -> bool {
        self.trimmed_text().is_empty()
    }

    /// Any filter set, the text and the sort aside.
    pub fn has_filters(&self) -> bool {
        *self != self.cleared()
    }

    /// Nothing at all: no text, no sort, no filter. Search's start, which shows the rails.
    pub fn is_blank(&self) -> bool {
        self.is_browse() && self.sort.is_none() && !self.has_filters()
    }

    /// The sort the results come in: the one chosen, or relevance with text and popularity
    /// without. Relevance chosen for a browse is popularity too, since there is nothing to
    /// match.
    pub fn sort_in_effect(&self) -> SearchSort {
        match self.sort {
            Some(SearchSort::Relevance) | None if self.is_browse() => SearchSort::Popularity,
            None => SearchSort::Relevance,
            Some(sort) => sort,
        }
    }

    /// The tag rank asked for: the query's own, or [`DEFAULT_TAG_RANK`].
    pub fn tag_rank_in_effect(&self) -> u8 {
        self.min_tag_rank.unwrap_or(DEFAULT_TAG_RANK).min(100)
    }

    /// Every filter that is set, in the words a chip shows, in the order the filters are
    /// offered.
    pub fn active(&self) -> Vec<ActiveFilter> {
        let mut out = Vec::new();
        let mut push = |key, label: String| out.push(ActiveFilter { key, label });
        for name in &self.genres {
            push(FilterKey::Genre { name: name.clone() }, name.clone());
        }
        for name in &self.genres_excluded {
            push(FilterKey::NotGenre { name: name.clone() }, format!("Not {name}"));
        }
        for name in &self.tags {
            push(FilterKey::Tag { name: name.clone() }, name.clone());
        }
        for name in &self.tags_excluded {
            push(FilterKey::NotTag { name: name.clone() }, format!("Not {name}"));
        }
        if let Some(rank) = self.min_tag_rank {
            push(FilterKey::TagRank, format!("Tag rank {rank}% or higher"));
        }
        if let Some(years) = self.years.ordered().label("") {
            let label = match (self.years.min, self.years.max) {
                (Some(_), None) => format!("Since {}", self.years.min.unwrap_or_default()),
                _ => years,
            };
            push(FilterKey::Years, label);
        }
        if let Some(season) = self.season {
            push(FilterKey::Premiered, season_label(season).into());
        }
        if !self.formats.is_empty() {
            push(FilterKey::Formats, joined(&self.formats, MediaFormat::label));
        }
        if !self.statuses.is_empty() {
            push(FilterKey::Statuses, joined(&self.statuses, status_label));
        }
        if let Some(episodes) = self.episodes.ordered().label("episodes") {
            let label =
                if self.episodes.single() == Some(1) { "1 episode".into() } else { episodes };
            push(FilterKey::Episodes, label);
        }
        if let Some(minutes) = self.minutes.ordered().label("min") {
            push(FilterKey::Minutes, minutes);
        }
        if let Some(score) = self.min_score {
            push(FilterKey::MinScore, format!("Score {score}% or higher"));
        }
        if self.min_popularity.is_some() {
            push(FilterKey::Obscure, "Hide obscure titles".into());
        }
        if !self.sources.is_empty() {
            push(FilterKey::Sources, joined(&self.sources, source_label));
        }
        if !self.countries.is_empty() {
            let names: Vec<String> = self.countries.iter().map(|c| country_label(c)).collect();
            push(FilterKey::Countries, names.join(", "));
        }
        if !self.streaming_on.is_empty() {
            push(FilterKey::Streaming, format!("On {}", self.streaming_on.join(", ")));
        }
        if self.licensed_only {
            push(FilterKey::Licensed, "Licensed only".into());
        }
        match self.on_list {
            Some(OnList::Hide) => push(FilterKey::MyList, "Not on my list".into()),
            Some(OnList::Only) => push(FilterKey::MyList, "On my list".into()),
            None => {}
        }
        out
    }

    /// What is set, in one line: "Mecha · 2010 to 2015 · TV". Empty when nothing is.
    pub fn summary(&self) -> String {
        self.active().into_iter().map(|f| f.label).collect::<Vec<_>>().join(" \u{b7} ")
    }

    /// The text if there is any, and otherwise the filters, for a recent search's first line.
    pub fn headline(&self) -> String {
        if self.is_browse() {
            let summary = self.summary();
            if summary.is_empty() { self.sort_in_effect().label().into() } else { summary }
        } else {
            self.trimmed_text().into()
        }
    }

    /// This query with one filter taken off.
    pub fn without(&self, key: &FilterKey) -> Self {
        let mut next = self.clone();
        match key {
            FilterKey::Genre { name } => next.genres.retain(|g| g != name),
            FilterKey::NotGenre { name } => next.genres_excluded.retain(|g| g != name),
            FilterKey::Tag { name } => next.tags.retain(|t| t != name),
            FilterKey::NotTag { name } => next.tags_excluded.retain(|t| t != name),
            FilterKey::TagRank => next.min_tag_rank = None,
            FilterKey::Years => next.years = Bounds::default(),
            FilterKey::Premiered => next.season = None,
            FilterKey::Formats => next.formats.clear(),
            FilterKey::Statuses => next.statuses.clear(),
            FilterKey::Episodes => next.episodes = Bounds::default(),
            FilterKey::Minutes => next.minutes = Bounds::default(),
            FilterKey::MinScore => next.min_score = None,
            FilterKey::Obscure => next.min_popularity = None,
            FilterKey::Sources => next.sources.clear(),
            FilterKey::Countries => next.countries.clear(),
            FilterKey::Streaming => next.streaming_on.clear(),
            FilterKey::Licensed => next.licensed_only = false,
            FilterKey::MyList => next.on_list = None,
        }
        next
    }

    /// This query with every filter taken off, its text and sort kept: Clear Filters.
    pub fn cleared(&self) -> Self {
        Self { text: self.text.clone(), sort: self.sort, ..Self::default() }
    }

    /// How many filters are set among those the advanced panel holds, for the badge on its
    /// button: tags, length, score, popularity, origin, streaming and your list.
    pub fn advanced_count(&self) -> u32 {
        let counts = [
            self.tags.len() + self.tags_excluded.len(),
            usize::from(self.min_tag_rank.is_some()),
            usize::from(!self.episodes.is_open()),
            usize::from(!self.minutes.is_open()),
            usize::from(self.min_score.is_some()),
            usize::from(self.min_popularity.is_some()),
            self.sources.len(),
            self.countries.len(),
            self.streaming_on.len(),
            usize::from(self.licensed_only),
            usize::from(self.on_list.is_some()),
        ];
        u32::try_from(counts.iter().sum::<usize>()).unwrap_or(u32::MAX)
    }

    pub fn genre_pick(&self, name: &str) -> Pick {
        pick_in(&self.genres, &self.genres_excluded, name)
    }

    /// This query with a genre wanted, ruled out, or neither.
    pub fn picking_genre(&self, name: &str, pick: Pick) -> Self {
        let mut next = self.clone();
        picked(&mut next.genres, &mut next.genres_excluded, name, pick);
        next
    }

    pub fn tag_pick(&self, name: &str) -> Pick {
        pick_in(&self.tags, &self.tags_excluded, name)
    }

    pub fn picking_tag(&self, name: &str, pick: Pick) -> Self {
        let mut next = self.clone();
        picked(&mut next.tags, &mut next.tags_excluded, name, pick);
        next
    }

    /// This query with a format added, or taken off when it was there.
    pub fn toggling_format(&self, format: MediaFormat) -> Self {
        Self { formats: toggled(&self.formats, &format), ..self.clone() }
    }

    pub fn toggling_status(&self, status: AiringStatus) -> Self {
        Self { statuses: toggled(&self.statuses, &status), ..self.clone() }
    }

    pub fn toggling_source(&self, source: SourceMaterial) -> Self {
        Self { sources: toggled(&self.sources, &source), ..self.clone() }
    }

    pub fn toggling_country(&self, code: &str) -> Self {
        Self { countries: toggled(&self.countries, &code.to_owned()), ..self.clone() }
    }

    pub fn toggling_streaming(&self, site: &str) -> Self {
        Self { streaming_on: toggled(&self.streaming_on, &site.to_owned()), ..self.clone() }
    }

    /// The years as one year, or none. A season keeps only while the year is a single one.
    pub fn in_year(&self, year: Option<u32>) -> Self {
        let years = Bounds { min: year, max: year };
        Self { years, season: year.and(self.season), ..self.clone() }
    }

    /// What an empty answer says: the filters named, or the text repeated.
    pub fn nothing_found(&self) -> String {
        let summary = self.active().into_iter().map(|f| f.label).collect::<Vec<_>>().join(", ");
        match (self.is_browse(), summary.is_empty()) {
            (true, true) => "Nothing found".into(),
            (true, false) => format!("No titles match: {summary}"),
            (false, true) => {
                format!("Nothing found for \u{201c}{}\u{201d}", self.trimmed_text())
            }
            (false, false) => format!(
                "Nothing found for \u{201c}{}\u{201d} with {summary}",
                self.trimmed_text()
            ),
        }
    }
}

/// The calls a native front end makes on a query. The same rules as the methods above, which
/// the terminal calls directly; owned arguments, as the bindings pass them.
#[cfg(feature = "ffi")]
#[uniffi::export]
impl SearchQuery {
    #[uniffi::method(name = "is_browse")]
    fn ffi_is_browse(&self) -> bool {
        self.is_browse()
    }

    #[uniffi::method(name = "has_filters")]
    fn ffi_has_filters(&self) -> bool {
        self.has_filters()
    }

    #[uniffi::method(name = "is_blank")]
    fn ffi_is_blank(&self) -> bool {
        self.is_blank()
    }

    #[uniffi::method(name = "sort_in_effect")]
    fn ffi_sort_in_effect(&self) -> SearchSort {
        self.sort_in_effect()
    }

    #[uniffi::method(name = "active")]
    fn ffi_active(&self) -> Vec<ActiveFilter> {
        self.active()
    }

    #[uniffi::method(name = "summary")]
    fn ffi_summary(&self) -> String {
        self.summary()
    }

    #[uniffi::method(name = "headline")]
    fn ffi_headline(&self) -> String {
        self.headline()
    }

    #[uniffi::method(name = "without")]
    fn ffi_without(&self, key: FilterKey) -> Self {
        self.without(&key)
    }

    #[uniffi::method(name = "cleared")]
    fn ffi_cleared(&self) -> Self {
        self.cleared()
    }

    #[uniffi::method(name = "advanced_count")]
    fn ffi_advanced_count(&self) -> u32 {
        self.advanced_count()
    }

    #[uniffi::method(name = "genre_pick")]
    fn ffi_genre_pick(&self, name: String) -> Pick {
        self.genre_pick(&name)
    }

    #[uniffi::method(name = "picking_genre")]
    fn ffi_picking_genre(&self, name: String, pick: Pick) -> Self {
        self.picking_genre(&name, pick)
    }

    #[uniffi::method(name = "tag_pick")]
    fn ffi_tag_pick(&self, name: String) -> Pick {
        self.tag_pick(&name)
    }

    #[uniffi::method(name = "picking_tag")]
    fn ffi_picking_tag(&self, name: String, pick: Pick) -> Self {
        self.picking_tag(&name, pick)
    }

    #[uniffi::method(name = "toggling_format")]
    fn ffi_toggling_format(&self, format: MediaFormat) -> Self {
        self.toggling_format(format)
    }

    #[uniffi::method(name = "toggling_status")]
    fn ffi_toggling_status(&self, status: AiringStatus) -> Self {
        self.toggling_status(status)
    }

    #[uniffi::method(name = "toggling_source")]
    fn ffi_toggling_source(&self, source: SourceMaterial) -> Self {
        self.toggling_source(source)
    }

    #[uniffi::method(name = "toggling_country")]
    fn ffi_toggling_country(&self, code: String) -> Self {
        self.toggling_country(&code)
    }

    #[uniffi::method(name = "toggling_streaming")]
    fn ffi_toggling_streaming(&self, site: String) -> Self {
        self.toggling_streaming(&site)
    }

    #[uniffi::method(name = "in_year")]
    fn ffi_in_year(&self, year: Option<u32>) -> Self {
        self.in_year(year)
    }

    #[uniffi::method(name = "nothing_found")]
    fn ffi_nothing_found(&self) -> String {
        self.nothing_found()
    }
}

/// The words and choices a filter row shows, for a native front end: each one the same as the
/// terminal's.
#[cfg(feature = "ffi")]
mod words {
    use super::*;

    /// The sorts a menu offers: relevance only with text.
    #[uniffi::export]
    pub fn search_sorts(with_text: bool) -> Vec<SearchSort> {
        SearchSort::ALL
            .into_iter()
            .filter(|s| with_text || *s != SearchSort::Relevance)
            .collect()
    }

    #[uniffi::export]
    pub fn search_sort_label(sort: SearchSort) -> String {
        sort.label().into()
    }

    #[uniffi::export]
    pub fn search_formats() -> Vec<MediaFormat> {
        MediaFormat::ALL.to_vec()
    }

    #[uniffi::export]
    pub fn media_format_label(format: MediaFormat) -> String {
        format.label().into()
    }

    #[uniffi::export]
    pub fn search_statuses() -> Vec<AiringStatus> {
        SEARCH_STATUSES.to_vec()
    }

    #[uniffi::export]
    pub fn search_status_label(status: AiringStatus) -> String {
        status_label(status).into()
    }

    #[uniffi::export]
    pub fn search_season_label(season: Season) -> String {
        season_label(season).into()
    }

    #[uniffi::export]
    pub fn search_sources() -> Vec<SourceMaterial> {
        SEARCH_SOURCES.to_vec()
    }

    #[uniffi::export]
    pub fn search_source_label(source: SourceMaterial) -> String {
        source_label(source).into()
    }

    #[uniffi::export]
    pub fn search_countries() -> Vec<String> {
        SEARCH_COUNTRIES.iter().map(|c| (*c).to_owned()).collect()
    }

    #[uniffi::export]
    pub fn search_country_label(code: String) -> String {
        country_label(&code)
    }
}

/// One row of Search's start: a preset and the titles it finds. "See all" opens
/// [`Self::query`] as a search.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct BrowseRail {
    /// "Trending now", "Popular this season".
    pub title: String,
    /// Which season, for the rails that are one: "Fall 2026".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub detail: Option<String>,
    pub query: SearchQuery,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub entries: Vec<Entry>,
}

/// One tag a search can ask for.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FacetTag {
    pub name: String,
    /// What AniList says the tag means.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub description: String,
    /// The name itself gives something away: shown only once asked for.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spoiler: bool,
}

/// The tags of one category: "Theme", "Setting", "Cast".
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TagGroup {
    pub category: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tags: Vec<FacetTag>,
}

/// What the filters choose from: AniList's genres, its tags by category and the streaming sites
/// it knows. Adult genres and tags are left out unless the setting shows adult titles.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchFacets {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tag_groups: Vec<TagGroup>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub streaming_sites: Vec<String>,
}

impl SearchFacets {
    /// Every tag whose name starts a word with `prefix`, any case, spoilers left out: what a
    /// search field suggests as tokens.
    pub fn tags_matching(&self, prefix: &str, limit: usize) -> Vec<String> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        self.tag_groups
            .iter()
            .flat_map(|group| &group.tags)
            .filter(|tag| !tag.spoiler)
            .filter(|tag| {
                tag.name.to_lowercase().split([' ', '-']).any(|word| word.starts_with(&prefix))
            })
            .map(|tag| tag.name.clone())
            .take(limit)
            .collect()
    }

    /// Every genre whose name starts a word with `prefix`, any case.
    pub fn genres_matching(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.trim().to_lowercase();
        if prefix.is_empty() {
            return Vec::new();
        }
        self.genres
            .iter()
            .filter(|g| {
                g.to_lowercase().split([' ', '-']).any(|word| word.starts_with(&prefix))
            })
            .cloned()
            .collect()
    }
}

#[cfg(feature = "ffi")]
#[uniffi::export]
impl SearchFacets {
    #[uniffi::method(name = "tags_matching")]
    fn ffi_tags_matching(&self, prefix: String, limit: u32) -> Vec<String> {
        self.tags_matching(&prefix, limit as usize)
    }

    #[uniffi::method(name = "genres_matching")]
    fn ffi_genres_matching(&self, prefix: String) -> Vec<String> {
        self.genres_matching(&prefix)
    }
}

/// AniList's genres, as Search offers them before AniList has been asked: the list it has had
/// for years, without the adult one.
pub const GENRES: [&str; 18] = [
    "Action",
    "Adventure",
    "Comedy",
    "Drama",
    "Ecchi",
    "Fantasy",
    "Horror",
    "Mahou Shoujo",
    "Mecha",
    "Music",
    "Mystery",
    "Psychological",
    "Romance",
    "Sci-Fi",
    "Slice of Life",
    "Sports",
    "Supernatural",
    "Thriller",
];

/// The genre AniList counts as adult, offered only when adult titles are shown.
pub const ADULT_GENRE: &str = "Hentai";

/// The tag category AniList counts as adult.
pub const ADULT_TAG_CATEGORY: &str = "Sexual Content";

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(query: &SearchQuery) -> Vec<String> {
        query.active().into_iter().map(|f| f.label).collect()
    }

    #[test]
    fn a_blank_query_is_the_start_and_a_sort_alone_is_a_browse() {
        assert!(SearchQuery::default().is_blank());
        assert!(SearchQuery::text("  ").is_blank(), "spaces are nothing typed");
        assert!(!SearchQuery::sorted(SearchSort::Trending).is_blank(), "a rail's See All");
        assert!(SearchQuery::sorted(SearchSort::Trending).is_browse());
        assert!(!SearchQuery::text("kaze").is_browse());
        let genre = SearchQuery::default().picking_genre("Mecha", Pick::Include);
        assert!(genre.has_filters() && !genre.is_blank());
        assert!(!SearchQuery::text("kaze").has_filters(), "text is not a filter");
    }

    #[test]
    fn the_default_sort_follows_the_text() {
        assert_eq!(SearchQuery::default().sort_in_effect(), SearchSort::Popularity);
        assert_eq!(SearchQuery::text("kaze").sort_in_effect(), SearchSort::Relevance);
        let relevance = SearchQuery::sorted(SearchSort::Relevance);
        assert_eq!(relevance.sort_in_effect(), SearchSort::Popularity, "nothing to match");
        let score = SearchQuery { sort: Some(SearchSort::Score), ..SearchQuery::text("kaze") };
        assert_eq!(score.sort_in_effect(), SearchSort::Score);
    }

    #[test]
    fn chips_name_each_filter_in_plain_words() {
        let query = SearchQuery {
            genres: vec!["Mecha".into()],
            genres_excluded: vec!["Romance".into()],
            years: Bounds { min: Some(2015), max: Some(2010) },
            formats: vec![MediaFormat::Tv, MediaFormat::Movie],
            statuses: vec![AiringStatus::Releasing],
            episodes: Bounds { min: None, max: Some(12) },
            minutes: Bounds { min: Some(60), max: None },
            min_score: Some(70),
            min_popularity: Some(OBSCURE_BELOW),
            countries: vec!["JP".into(), "KR".into()],
            on_list: Some(OnList::Hide),
            ..SearchQuery::default()
        };
        assert_eq!(
            labels(&query),
            [
                "Mecha",
                "Not Romance",
                "2010 to 2015",
                "TV, Movie",
                "Airing",
                "Up to 12 episodes",
                "60 min or more",
                "Score 70% or higher",
                "Hide obscure titles",
                "Japan, Korea",
                "Not on my list",
            ]
        );
        assert_eq!(
            SearchQuery { years: Bounds { min: Some(2010), max: None }, ..Default::default() }
                .summary(),
            "Since 2010"
        );
        assert_eq!(SearchQuery::season(Season::Fall, 2026).summary(), "2026 \u{b7} Fall");
    }

    #[test]
    fn each_chip_takes_off_only_its_own_filter() {
        let query = SearchQuery {
            text: "kaze".into(),
            sort: Some(SearchSort::Score),
            genres: vec!["Mecha".into(), "Drama".into()],
            formats: vec![MediaFormat::Tv],
            ..SearchQuery::default()
        };
        let mecha = query.without(&FilterKey::Genre { name: "Mecha".into() });
        assert_eq!(mecha.genres, ["Drama"]);
        assert_eq!(mecha.formats, [MediaFormat::Tv]);
        assert_eq!(query.without(&FilterKey::Formats).formats, []);
        for filter in query.active() {
            assert!(query.without(&filter.key).active().len() < query.active().len());
        }
        let cleared = query.cleared();
        assert_eq!(cleared.text, "kaze", "the text stays");
        assert_eq!(cleared.sort, Some(SearchSort::Score), "and so does the sort");
        assert!(!cleared.has_filters());
    }

    #[test]
    fn a_genre_is_wanted_ruled_out_or_neither_and_never_both() {
        let query = SearchQuery::default().picking_genre("Mecha", Pick::Include);
        assert_eq!(query.genre_pick("Mecha"), Pick::Include);
        let query = query.picking_genre("Mecha", Pick::Exclude);
        assert_eq!(query.genres, Vec::<String>::new());
        assert_eq!(query.genres_excluded, ["Mecha"]);
        assert_eq!(query.picking_genre("Mecha", Pick::Off), SearchQuery::default());
        let tagged = SearchQuery::default().picking_tag("Time Travel", Pick::Include);
        assert_eq!(tagged.tag_pick("Time Travel"), Pick::Include);
        assert_eq!(tagged.advanced_count(), 1);
    }

    #[test]
    fn formats_toggle_in_the_order_they_were_set() {
        let query = SearchQuery::default()
            .toggling_format(MediaFormat::Movie)
            .toggling_format(MediaFormat::Tv);
        assert_eq!(query.formats, [MediaFormat::Movie, MediaFormat::Tv]);
        assert_eq!(query.toggling_format(MediaFormat::Movie).formats, [MediaFormat::Tv]);
    }

    #[test]
    fn a_season_keeps_only_with_a_single_year() {
        let query = SearchQuery::season(Season::Spring, 2024);
        assert_eq!(query.in_year(Some(2023)).season, Some(Season::Spring));
        assert_eq!(query.in_year(None).season, None);
        assert_eq!(query.in_year(None).years, Bounds::default());
    }

    #[test]
    fn an_empty_answer_names_what_was_asked() {
        assert_eq!(
            SearchQuery::text("kaze").nothing_found(),
            "Nothing found for \u{201c}kaze\u{201d}"
        );
        let browse = SearchQuery {
            genres: vec!["Mecha".into()],
            formats: vec![MediaFormat::Movie],
            years: Bounds { min: Some(2010), max: Some(2015) },
            ..SearchQuery::default()
        };
        assert_eq!(browse.nothing_found(), "No titles match: Mecha, 2010 to 2015, Movie");
        assert!(!browse.nothing_found().contains("\u{201c}\u{201d}"), "never empty quotes");
    }

    #[test]
    fn a_recent_browse_is_headed_by_its_filters() {
        assert_eq!(SearchQuery::text(" kaze ").headline(), "kaze");
        let browse = SearchQuery::default().picking_genre("Mecha", Pick::Include);
        assert_eq!(browse.headline(), "Mecha");
        assert_eq!(SearchQuery::sorted(SearchSort::Trending).headline(), "Trending");
    }

    #[test]
    fn tags_and_genres_are_suggested_by_the_start_of_a_word() {
        let facets = SearchFacets {
            genres: vec!["Slice of Life".into(), "Sci-Fi".into(), "Sports".into()],
            tag_groups: vec![TagGroup {
                category: "Theme".into(),
                tags: vec![
                    FacetTag {
                        name: "Time Travel".into(),
                        description: String::new(),
                        spoiler: false,
                    },
                    FacetTag {
                        name: "Twist Ending".into(),
                        description: String::new(),
                        spoiler: true,
                    },
                    FacetTag {
                        name: "Travel".into(),
                        description: String::new(),
                        spoiler: false,
                    },
                ],
            }],
            streaming_sites: vec![],
        };
        assert_eq!(facets.tags_matching("trav", 5), ["Time Travel", "Travel"]);
        assert!(facets.tags_matching("twist", 5).is_empty(), "a spoiler is never suggested");
        assert_eq!(facets.genres_matching("fi"), ["Sci-Fi"]);
        assert_eq!(facets.genres_matching("life"), ["Slice of Life"]);
        assert!(facets.genres_matching(" ").is_empty());
    }
}
