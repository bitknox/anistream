//! AniList GraphQL client.
//!
//! The stable spine. Everything else in anistream can fail and the app degrades; if this
//! layer is unavailable there is nothing to browse, so it is the one place where rate
//! limiting and caching are treated as correctness rather than optimisation.
//!
//! The observed budget is **30 requests/minute**, shared across search, seasonal, calendar
//! and library. That is small enough that a careless screen refresh can exhaust it, which
//! is why queries are wide (one round trip per screen) and responses are cached.

pub mod model;
pub mod query;
pub mod threads;

use std::sync::Arc;

use anistream_core::ids::AnilistId;
use anistream_net::{HttpClient, RateLimiter, ratelimit::parse_rate_headers};
use serde::de::DeserializeOwned;

pub use model::{
    Broadcast, DescriptionSpan, ExternalLink, FuzzyDate, LastAired, Media, Ranking, Relation,
    SourceMaterial, StreamingEpisode, Tag, Title, TitleLanguage, Viewer, spoiler_runs,
};
pub use threads::{Liked, Thread, ThreadComment};

const ENDPOINT: &str = "https://graphql.anilist.co";

#[derive(Debug, Clone, thiserror::Error)]
pub enum AniListError {
    #[error("network: {0}")]
    Network(String),

    /// AniList returns `200` with an `errors` array rather than an HTTP error status, so
    /// this is the *normal* failure shape and has to be checked explicitly.
    #[error("anilist: {0}")]
    Api(String),

    #[error("unexpected response shape: {0}")]
    Decode(String),

    #[error("not found")]
    NotFound,

    #[error("authentication required")]
    Unauthenticated,
}

pub type Result<T> = std::result::Result<T, AniListError>;

/// Which season a browse query is for, and the one a title premiered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

impl Season {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Winter => "WINTER",
            Self::Spring => "SPRING",
            Self::Summer => "SUMMER",
            Self::Fall => "FALL",
        }
    }

    /// The season AniList calls `name`.
    pub fn parse(name: &str) -> Option<Self> {
        [Self::Winter, Self::Spring, Self::Summer, Self::Fall]
            .into_iter()
            .find(|season| season.as_str() == name)
    }

    /// The season containing a given month.
    pub const fn of_month(month: u8) -> Self {
        match month {
            1..=3 => Self::Winter,
            4..=6 => Self::Spring,
            7..=9 => Self::Summer,
            _ => Self::Fall,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Winter => Self::Spring,
            Self::Spring => Self::Summer,
            Self::Summer => Self::Fall,
            Self::Fall => Self::Winter,
        }
    }

    pub const fn previous(self) -> Self {
        match self {
            Self::Winter => Self::Fall,
            Self::Spring => Self::Winter,
            Self::Summer => Self::Spring,
            Self::Fall => Self::Summer,
        }
    }
}

/// Server-side browse filters: every argument of `Page.media` a search can set, in AniList's
/// own words and values.
///
/// Every field here is applied by AniList rather than locally, which is what makes a wide
/// filter surface affordable against a 30/minute budget. Bounds are exclusive, as AniList's
/// `_greater` and `_lesser` arguments are. An unset field is left out of the request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrowseFilter {
    /// AniList's sort names, the first deciding. Empty is relevance with a term and
    /// popularity without. Ties are broken by id either way, so pages do not shuffle.
    pub sort: Vec<String>,
    /// Every one of these.
    pub genres: Vec<String>,
    pub genres_excluded: Vec<String>,
    /// Every one of these.
    pub tags: Vec<String>,
    pub tags_excluded: Vec<String>,
    pub min_tag_rank: Option<u32>,
    /// Any of these (`TV`, `TV_SHORT`, …).
    pub formats: Vec<String>,
    /// Any of these (`RELEASING`, …).
    pub statuses: Vec<String>,
    /// `WINTER`…
    pub season: Option<String>,
    /// The year of [`Self::season`], or on its own, the year a title aired in.
    pub year: Option<u32>,
    /// Start dates as AniList's `YYYYMMDD` integers, exclusive.
    pub started_after: Option<u32>,
    pub started_before: Option<u32>,
    pub episodes_greater: Option<u32>,
    pub episodes_lesser: Option<u32>,
    /// Minutes an episode.
    pub duration_greater: Option<u32>,
    pub duration_lesser: Option<u32>,
    /// Average score, 0 to 100, above this.
    pub min_score: Option<u32>,
    /// People with the title on a list, above this.
    pub min_popularity: Option<u32>,
    /// Any of these (`MANGA`, `LIGHT_NOVEL`, …).
    pub sources: Vec<String>,
    /// Any of these ISO codes.
    pub countries: Vec<String>,
    /// Streaming officially on any of these sites.
    pub licensed_by: Vec<String>,
    pub licensed: Option<bool>,
    /// On the signed-in viewer's lists, or not; needs the token.
    pub on_list: Option<bool>,
    /// Adult or not; `None` asks for both.
    pub adult: Option<bool>,
}

impl BrowseFilter {
    pub fn is_empty(&self) -> bool {
        *self == Self { adult: self.adult, ..Self::default() }
    }

    /// The arguments of one page of this browse for `term`, as [`query::media_pages`] takes
    /// them: only those set.
    pub fn arguments(
        &self,
        term: &str,
        page: u32,
        per_page: u32,
    ) -> Vec<(query::Argument, serde_json::Value)> {
        use serde_json::json;
        let term = term.trim();
        let mut out: Vec<(query::Argument, serde_json::Value)> = vec![
            (("page", "Int"), json!(page.max(1))),
            (("perPage", "Int"), json!(per_page.clamp(1, 50))),
        ];
        let mut sort = self.sort.clone();
        if sort.is_empty() {
            // `SEARCH_MATCH` means nothing without a term to match.
            sort.push(if term.is_empty() { "POPULARITY_DESC" } else { "SEARCH_MATCH" }.into());
        }
        if !sort.iter().any(|s| s == "ID" || s == "ID_DESC") {
            sort.push("ID".into());
        }
        out.push((("sort", "[MediaSort]"), json!(sort)));
        if !term.is_empty() {
            out.push((("search", "String"), json!(term)));
        }
        let lists: [(query::Argument, &Vec<String>); 9] = [
            (("genre_in", "[String]"), &self.genres),
            (("genre_not_in", "[String]"), &self.genres_excluded),
            (("tag_in", "[String]"), &self.tags),
            (("tag_not_in", "[String]"), &self.tags_excluded),
            (("format_in", "[MediaFormat]"), &self.formats),
            (("status_in", "[MediaStatus]"), &self.statuses),
            (("source_in", "[MediaSource]"), &self.sources),
            (("licensedBy_in", "[String]"), &self.licensed_by),
            (("countryOfOrigin_in", "[CountryCode]"), &self.countries),
        ];
        for (argument, values) in lists {
            if values.is_empty() {
                continue;
            }
            // One country goes as the plain argument, which every schema has had.
            if argument.0 == "countryOfOrigin_in" && values.len() == 1 {
                out.push((("countryOfOrigin", "CountryCode"), json!(values[0])));
            } else {
                out.push((argument, json!(values)));
            }
        }
        if !self.tags.is_empty() || !self.tags_excluded.is_empty() {
            out.push((("minimumTagRank", "Int"), json!(self.min_tag_rank.unwrap_or(60))));
        }
        if let Some(season) = &self.season {
            out.push((("season", "MediaSeason"), json!(season)));
        }
        let numbers: [(query::Argument, Option<u32>); 9] = [
            (("seasonYear", "Int"), self.year),
            (("startDate_greater", "FuzzyDateInt"), self.started_after),
            (("startDate_lesser", "FuzzyDateInt"), self.started_before),
            (("episodes_greater", "Int"), self.episodes_greater),
            (("episodes_lesser", "Int"), self.episodes_lesser),
            (("duration_greater", "Int"), self.duration_greater),
            (("duration_lesser", "Int"), self.duration_lesser),
            (("averageScore_greater", "Int"), self.min_score),
            (("popularity_greater", "Int"), self.min_popularity),
        ];
        for (argument, value) in numbers {
            if let Some(value) = value {
                out.push((argument, json!(value)));
            }
        }
        let flags: [(query::Argument, Option<bool>); 3] = [
            (("isLicensed", "Boolean"), self.licensed),
            (("onList", "Boolean"), self.on_list),
            (("isAdult", "Boolean"), self.adult),
        ];
        for (argument, value) in flags {
            if let Some(value) = value {
                out.push((argument, json!(value)));
            }
        }
        out
    }

    /// A filter with every argument set, for the tests that hold the documents to GraphQL.
    #[cfg(test)]
    pub(crate) fn every_argument() -> Self {
        Self {
            sort: vec!["SCORE_DESC".into()],
            genres: vec!["Mecha".into()],
            genres_excluded: vec!["Romance".into()],
            tags: vec!["Time Travel".into()],
            tags_excluded: vec!["Isekai".into()],
            min_tag_rank: Some(80),
            formats: vec!["TV".into(), "MOVIE".into()],
            statuses: vec!["FINISHED".into()],
            season: Some("FALL".into()),
            year: Some(2024),
            started_after: Some(2009_9999),
            started_before: Some(2020_0000),
            episodes_greater: Some(0),
            episodes_lesser: Some(13),
            duration_greater: Some(15),
            duration_lesser: Some(61),
            min_score: Some(69),
            min_popularity: Some(1000),
            sources: vec!["MANGA".into()],
            countries: vec!["JP".into(), "KR".into()],
            licensed_by: vec!["Hikari Stream".into()],
            licensed: Some(true),
            on_list: Some(false),
            adult: Some(false),
        }
    }
}

/// What the filters choose from, as AniList lists it.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Facets {
    pub genres: Vec<String>,
    pub tags: Vec<FacetTag>,
    /// Streaming sites, each once, in AniList's order.
    pub streaming: Vec<String>,
}

/// One of AniList's tags.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FacetTag {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// "Theme-Action", "Cast-Main Cast", "Setting-Scene", "Demographic".
    #[serde(default)]
    pub category: String,
    #[serde(default, rename = "isAdult")]
    pub adult: bool,
    #[serde(default, rename = "isGeneralSpoiler")]
    pub spoiler: bool,
}

tokio::task_local! {
    /// Set for work nobody is looking at: see [`in_background`].
    static BACKGROUND: ();
}

/// AniList requests made inside `work` leave the last few tokens of the budget for whatever
/// someone is looking at: a sync, a relations walk or the new-episode check never makes a
/// search wait.
pub async fn in_background<F: Future>(work: F) -> F::Output {
    BACKGROUND.scope((), work).await
}

/// How many tokens background work leaves in the bucket.
const INTERACTIVE_RESERVE: f64 = 6.0;

/// One upcoming broadcast, for the Calendar screen.
#[derive(Debug, Clone)]
pub struct AiringEntry {
    pub episode: u32,
    pub airing_at: i64,
    pub media: Media,
}

/// A page of results.
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_next: bool,
}

/// One row of the authenticated user's list.
///
/// Carries the full `Media` as well as the list state, so a library pull populates the Library
/// screen without a second round of lookups against a 30-per-minute budget.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    /// The `MediaList` entry id — distinct from the media id, and what deletion needs.
    pub entry_id: Option<u32>,
    pub media: Media,
    /// AniList's own status string: `CURRENT`, `PLANNING`, `COMPLETED`, …
    pub status: String,
    pub progress: u32,
    /// On the 10-point decimal scale. `0` means unscored on AniList, mapped to `None` here so
    /// "unscored" and "scored zero" stay distinguishable.
    pub score: Option<f32>,
    /// When AniList last changed this entry — the timestamp last-write-wins needs.
    pub updated_at: i64,
}

/// The AniList client.
#[derive(Clone)]
pub struct AniList {
    http: HttpClient,
    limiter: Arc<RateLimiter>,
    /// Behind a lock so a sign-in *during* a session takes effect without a restart.
    ///
    /// It was a plain `Option<String>` captured at construction, which is why signing in used to
    /// end with "restart to start syncing": the token reached the keychain and nothing that could
    /// use it ever saw it. A shared cell is the whole difference.
    token: Arc<std::sync::RwLock<Option<String>>>,
    /// Where GraphQL requests go: [`ENDPOINT`], unless a test pointed it elsewhere.
    endpoint: Arc<str>,
}

impl AniList {
    pub fn new(http: HttpClient, requests_per_minute: u32) -> Self {
        Self {
            http,
            limiter: Arc::new(RateLimiter::per_minute(requests_per_minute)),
            token: Arc::new(std::sync::RwLock::new(None)),
            endpoint: Arc::from(ENDPOINT),
        }
    }

    /// Send every request to `endpoint` instead of AniList: a stand-in server on loopback.
    ///
    /// For tests and probes only, which is why it is hidden from the docs and has no config
    /// key. A configurable endpoint would be one more way for a user's metadata to come from
    /// somewhere that is not AniList, and nothing but a test has a reason to want that. The
    /// request is otherwise unchanged, so it is the production path that gets exercised: the
    /// same rate limiter, and the same plain client, which speaks `http://` as readily as
    /// `https://`.
    #[doc(hidden)]
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = Arc::from(endpoint.into());
        self
    }

    /// Attach an OAuth token for library and mutation access.
    pub fn with_token(self, token: Option<String>) -> Self {
        self.set_token(token);
        self
    }

    /// Swap the token in, for a sign-in that happens while the app is running.
    pub fn set_token(&self, token: Option<String>) {
        *self.token.write().unwrap_or_else(|e| e.into_inner()) = token;
    }

    pub fn is_authenticated(&self) -> bool {
        self.token().is_some()
    }

    /// A snapshot of the token. Cloned out rather than held: the lock must not be alive across an
    /// `await`, and this is read on every request.
    fn token(&self) -> Option<String> {
        self.token.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// How long the next request would spend waiting on the rate limit.
    ///
    /// Surfaced so a slow screen can say *why* it is slow. AniList's measured budget is 30 requests
    /// a minute shared across every screen, so a burst of navigation genuinely can drain it — and
    /// "loading" forever with no reason given is indistinguishable from a hang.
    pub async fn rate_limit_wait(&self) -> Option<std::time::Duration> {
        self.limiter.would_wait().await
    }

    /// Execute a GraphQL operation.
    ///
    /// Uses the plain client: AniList has no bot protection, so fingerprint emulation would
    /// only add handshake cost.
    async fn execute<T: DeserializeOwned>(
        &self,
        document: &str,
        variables: serde_json::Value,
    ) -> Result<T> {
        if BACKGROUND.try_with(|()| ()).is_ok() {
            self.limiter.acquire_leaving(INTERACTIVE_RESERVE).await;
        } else {
            self.limiter.acquire().await;
        }

        let mut request = self
            .http
            .plain()
            .post(&*self.endpoint)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        if let Some(token) = &self.token() {
            request = request.header("authorization", format!("Bearer {token}"));
        }

        let response = request
            .json(&serde_json::json!({ "query": document, "variables": variables }))
            .send()
            .await
            .map_err(|e| AniListError::Network(e.to_string()))?;

        let status = response.status();

        // Feed the server's own accounting back into the limiter before anything else —
        // it is authoritative, and our local bucket is only an estimate.
        let headers = response.headers().clone();
        let (remaining, reset) = parse_rate_headers(
            |name| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_owned),
            now_epoch(),
        );
        self.limiter.observe(remaining, reset).await;

        if status.as_u16() == 429 {
            let wait = reset.unwrap_or(std::time::Duration::from_secs(60));
            self.limiter.back_off(wait).await;
            return Err(AniListError::Api(format!(
                "rate limited, retry in {}s",
                wait.as_secs()
            )));
        }
        if status.as_u16() == 401 {
            return Err(AniListError::Unauthenticated);
        }

        let body: serde_json::Value =
            response.json().await.map_err(|e| AniListError::Decode(e.to_string()))?;

        // GraphQL reports failure inside a 200, so this check is the real error path.
        if let Some(errors) = body.get("errors").and_then(|e| e.as_array())
            && !errors.is_empty()
        {
            let message = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(|m| m.as_str()))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(AniListError::Api(if message.is_empty() {
                format!("request failed with HTTP {}", status.as_u16())
            } else {
                message
            }));
        }

        let data = body
            .get("data")
            .ok_or_else(|| AniListError::Decode("response had no data field".into()))?;

        serde_json::from_value(data.clone()).map_err(|e| AniListError::Decode(e.to_string()))
    }

    /// Full-text search, with nothing else set.
    pub async fn search(&self, term: &str, page: u32, per_page: u32) -> Result<Page<Media>> {
        self.browse(term, &BrowseFilter::default(), page, per_page).await
    }

    /// One page of a search for `term` with `filter`, or a browse when `term` is empty.
    pub async fn browse(
        &self,
        term: &str,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let mut pages = self.browse_many(&[(term, filter)], page, per_page).await?;
        Ok(pages.pop().unwrap_or(Page { items: Vec::new(), has_next: false }))
    }

    /// Page `page` of several browses in one request, in the order asked: Search's rails.
    pub async fn browse_many(
        &self,
        browses: &[(&str, &BrowseFilter)],
        page: u32,
        per_page: u32,
    ) -> Result<Vec<Page<Media>>> {
        if browses.is_empty() {
            return Ok(Vec::new());
        }
        let arguments: Vec<_> = browses
            .iter()
            .map(|(term, filter)| filter.arguments(term, page, per_page))
            .collect();
        let (document, variables) = query::media_pages(&arguments);
        let data: serde_json::Value = self.execute(&document, variables).await?;
        (0..browses.len()).map(|i| decode_media_page(&data[format!("r{i}")])).collect()
    }

    /// The genres, tags and streaming sites the filters choose from.
    pub async fn facets(&self) -> Result<Facets> {
        let data: serde_json::Value =
            self.execute(query::FACETS, serde_json::json!({})).await?;
        let genres = data["GenreCollection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| g.as_str().map(str::to_owned))
            .collect();
        let tags = data["MediaTagCollection"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|t| serde_json::from_value::<FacetTag>(t.clone()).ok())
            .collect();
        let mut streaming: Vec<String> = Vec::new();
        for site in data["ExternalLinkSourceCollection"].as_array().into_iter().flatten() {
            if site["isDisabled"].as_bool() == Some(true) {
                continue;
            }
            if let Some(name) = site["site"].as_str()
                && !streaming.iter().any(|s| s == name)
            {
                streaming.push(name.to_owned());
            }
        }
        Ok(Facets { genres, tags, streaming })
    }

    /// How many requests could go now without waiting on the limit.
    pub async fn budget_left(&self) -> f64 {
        self.limiter.available().await
    }

    /// One title, with relations, recommendations and streaming links.
    /// Several titles in one request, for the `CONTINUE` rail.
    ///
    /// Returned in *the order asked for*, not AniList's. The rail is ordered by when you last
    /// watched each title, and that ordering is the whole point of it — letting the API's own
    /// order through would shuffle it into something meaningless.
    pub async fn media_many(&self, ids: &[AnilistId]) -> Result<Vec<Media>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                &query::by_ids(),
                serde_json::json!({ "ids": raw, "perPage": ids.len().min(50) }),
            )
            .await?;
        let page: Page<Media> = decode_media_page(&data["Page"])?;
        let mut found = page.items;
        let mut ordered = Vec::with_capacity(found.len());
        for id in ids {
            if let Some(index) = found.iter().position(|m| m.id == *id) {
                ordered.push(found.remove(index));
            }
        }
        Ok(ordered)
    }

    /// What people who liked each of `ids` (at most [`query::RECOMMENDED_FROM`]) rated highest:
    /// each title with its name and the ids recommended off it, best rated first. Titles
    /// AniList has nothing recommended for come back with none. See [`query::recommended_from`].
    pub async fn recommended_from(
        &self,
        ids: &[AnilistId],
    ) -> Result<Vec<(AnilistId, String, Vec<AnilistId>)>> {
        let ids = &ids[..ids.len().min(query::RECOMMENDED_FROM)];
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                &query::recommended_from(),
                serde_json::json!({ "ids": raw, "perPage": ids.len() }),
            )
            .await?;
        let id_of = |v: &serde_json::Value| v.as_u64().map(|n| AnilistId::new(n as u32));
        let mut found: Vec<(AnilistId, String, Vec<AnilistId>)> = data["Page"]["media"]
            .as_array()
            .map(|media| {
                media
                    .iter()
                    .filter_map(|m| {
                        let name = ["english", "romaji", "native"]
                            .iter()
                            .find_map(|k| m["title"][*k].as_str())
                            .unwrap_or_default()
                            .to_owned();
                        let recommended = m["recommendations"]["nodes"]
                            .as_array()
                            .map(|nodes| {
                                nodes
                                    .iter()
                                    .filter_map(|n| id_of(&n["mediaRecommendation"]["id"]))
                                    .collect()
                            })
                            .unwrap_or_default();
                        Some((id_of(&m["id"])?, name, recommended))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // In the order asked, as `media_many` does: the caller asked most recent first.
        found.sort_by_key(|(id, ..)| ids.iter().position(|asked| asked == id));
        Ok(found)
    }

    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        let data: serde_json::Value =
            self.execute(&query::by_id(), serde_json::json!({ "id": id.get() })).await?;
        decode_detail(&data)
    }

    /// The cards of at most fifty titles, each with its relations two levels deep: one step of
    /// the season walk. See [`query::relations`].
    ///
    /// A title AniList has no anime for is not in the answer; one it answers in a shape this
    /// build cannot read is left out too, so the walk takes both for a link that leads nowhere.
    pub async fn relations(&self, ids: &[AnilistId]) -> Result<Vec<model::RelationNode>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = &ids[..ids.len().min(50)];
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                &query::relations(),
                serde_json::json!({ "ids": raw, "perPage": ids.len() }),
            )
            .await?;
        Ok(data["Page"]["media"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|node| serde_json::from_value(node.clone()).ok())
            .collect())
    }

    /// Upcoming broadcasts in a time window — the Calendar screen.
    pub async fn airing_between(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Page<AiringEntry>> {
        self.airing_between_sorted(from, to, page, per_page, false).await
    }

    /// The same window, with the sort direction chosen by the caller.
    ///
    /// `newest_first` matters for the past half of the calendar: see [`query::airing_schedule`].
    pub async fn airing_between_sorted(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
        newest_first: bool,
    ) -> Result<Page<AiringEntry>> {
        let sort = if newest_first { "TIME_DESC" } else { "TIME" };
        let data: serde_json::Value = self
            .execute(
                &query::airing_schedule(),
                serde_json::json!({
                    "from": from, "to": to, "page": page.max(1), "perPage": per_page,
                    "sort": [sort]
                }),
            )
            .await?;
        Ok(airing_page(&data["Page"]))
    }

    /// What aired for `ids` after `from` and before `to`, oldest first: a page of at most
    /// fifty broadcasts, and whether there are more after it. Each carries its title's forms,
    /// cover and banner, and nothing else of the title.
    ///
    /// One request for every title, however many: see [`query::AIRED_SINCE`].
    pub async fn aired_since(
        &self,
        ids: &[AnilistId],
        from: i64,
        to: i64,
    ) -> Result<Page<AiringEntry>> {
        if ids.is_empty() {
            return Ok(Page { items: Vec::new(), has_next: false });
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        let data: serde_json::Value = self
            .execute(
                query::AIRED_SINCE,
                serde_json::json!({ "ids": raw, "from": from, "to": to, "perPage": 50 }),
            )
            .await?;
        Ok(airing_page(&data["Page"]))
    }

    /// When each of these titles last aired an episode.
    ///
    /// Batched deliberately — see [`query::LAST_AIRED`] for why this cannot be folded into the
    /// list query and why the weekly-cadence shortcut does not work. Callers should pass only
    /// the titles that are actually releasing; a finished show has nothing to report and would
    /// spend request budget crowding out one that does.
    pub async fn last_aired(&self, ids: &[AnilistId]) -> Result<Vec<LastAired>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let raw: Vec<i64> = ids.iter().map(|id| i64::from(id.get())).collect();
        // Sorted newest-first across every id, so a few rows per title is ample to find each
        // one's latest. Asking for one row per title would return only the newest overall.
        let per_page = (ids.len() * 2).min(50);
        let data: serde_json::Value = self
            .execute(query::LAST_AIRED, serde_json::json!({ "ids": raw, "perPage": per_page }))
            .await?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let mut out: Vec<LastAired> = data["Page"]["airingSchedules"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect()
            })
            .unwrap_or_default();
        // The `notYetAired` filter is AniList's own bookkeeping and it lags around a
        // broadcast; a row whose air time is still ahead is not an answer to "what last
        // aired", and dropped *before* the dedup so it cannot shadow the row that is.
        out.retain(|r| r.airing_at <= now);
        // Keep the newest row per title and drop the rest.
        out.sort_by(|a, b| {
            a.media_id.get().cmp(&b.media_id.get()).then(b.airing_at.cmp(&a.airing_at))
        });
        out.dedup_by_key(|r| r.media_id);
        Ok(out)
    }

    /// The authenticated user's whole anime list.
    ///
    /// One request for every status at once — AniList returns the lists nested under
    /// `MediaListCollection`, and paginating per status would cost five round trips against a
    /// 30-per-minute budget.
    pub async fn library(&self, user_id: u32) -> Result<Vec<LibraryEntry>> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value = self
            .execute(&query::user_library(), serde_json::json!({ "userId": user_id }))
            .await?;

        let lists = data["MediaListCollection"]["lists"]
            .as_array()
            .ok_or_else(|| AniListError::Decode("library had no lists".into()))?;

        let mut out = Vec::new();
        for list in lists {
            for entry in list["entries"].as_array().into_iter().flatten() {
                // One malformed entry must not blank a whole library. Someone with 800 titles
                // and one broken row should still see 799.
                match decode_library_entry(entry) {
                    Some(decoded) => out.push(decoded),
                    None => tracing::warn!("skipping undecodable library entry"),
                }
            }
        }
        Ok(out)
    }

    /// Write progress, status and/or score for one title.
    ///
    /// Idempotent by construction: `SaveMediaListEntry` is an upsert keyed on `mediaId`, so
    /// replaying a queued op after an ambiguous failure is safe. That is what lets the outbox
    /// retry without bookkeeping.
    ///
    /// `None` fields are omitted rather than sent as null, because sending null would *clear*
    /// the value — pushing progress would wipe the user's score.
    /// Remove a title from the user's list entirely.
    ///
    /// Takes the **MediaList entry** id from [`Self::save_entry`], not a media id — they are
    /// different numbers, and confusing them would delete an unrelated entry.
    pub async fn delete_entry(&self, entry_id: u32) -> Result<bool> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value =
            self.execute(query::DELETE_ENTRY, serde_json::json!({ "id": entry_id })).await?;
        Ok(data["DeleteMediaListEntry"]["deleted"].as_bool().unwrap_or(false))
    }

    pub async fn save_entry(
        &self,
        id: AnilistId,
        progress: Option<u32>,
        status: Option<&str>,
        score: Option<f32>,
    ) -> Result<Option<u32>> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }

        let mut variables = serde_json::Map::new();
        variables.insert("mediaId".into(), id.get().into());
        if let Some(progress) = progress {
            variables.insert("progress".into(), progress.into());
        }
        if let Some(status) = status {
            variables.insert("status".into(), status.into());
        }
        if let Some(score) = score {
            // `scoreRaw` is always out of 100, whatever scale the user's profile displays.
            // The plain `score` argument is read in that profile scale, so sending our
            // 10-point value through it would write 8/100 for a 100-point user.
            variables.insert("scoreRaw".into(), ((score * 10.0).round() as u32).into());
        }

        let data: serde_json::Value =
            self.execute(query::SAVE_PROGRESS, serde_json::Value::Object(variables)).await?;
        // The entry id, so a caller that needs to remove the entry again can. `None` is not a
        // failure — the mutation succeeded either way.
        Ok(data["SaveMediaListEntry"]["id"].as_u64().map(|id| id as u32))
    }

    /// The authenticated user's id, for library queries.
    pub async fn viewer_id(&self) -> Result<u32> {
        self.viewer().await.map(|viewer| viewer.id)
    }

    /// The authenticated user: their id, name and the title language they read AniList in.
    pub async fn viewer(&self) -> Result<Viewer> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value =
            self.execute(query::VIEWER, serde_json::json!({})).await?;
        decode_viewer(&data["Viewer"])
    }
}

/// The single-title answer: the title, with the broadcasts already out that were asked for
/// beside it put on it. Those are best-effort, so a shape this build does not expect leaves the
/// title without them rather than failing it.
fn decode_detail(data: &serde_json::Value) -> Result<Media> {
    let node = data.get("Media").filter(|m| !m.is_null()).ok_or(AniListError::NotFound)?;
    let mut media: Media = serde_json::from_value(node.clone())
        .map_err(|e| AniListError::Decode(e.to_string()))?;
    media.aired = data["aired"]["airingSchedules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| serde_json::from_value(row.clone()).ok())
        .collect();
    Ok(media)
}

/// The `Viewer` node: an id it cannot do without, a name and a language it can.
fn decode_viewer(node: &serde_json::Value) -> Result<Viewer> {
    let id = node["id"]
        .as_u64()
        .map(|v| v as u32)
        .ok_or_else(|| AniListError::Decode("viewer response had no id".into()))?;
    Ok(Viewer {
        id,
        name: node["name"].as_str().map(str::to_owned),
        title_language: node["options"]["titleLanguage"]
            .as_str()
            .and_then(TitleLanguage::from_wire),
    })
}

/// Decode one `MediaList` row.
///
/// Returns `None` rather than an error: the caller skips the row and keeps the rest of the
/// library, which matters when the alternative is showing an empty list.
fn decode_library_entry(entry: &serde_json::Value) -> Option<LibraryEntry> {
    let media = serde_json::from_value::<Media>(entry.get("media")?.clone()).ok()?;
    Some(LibraryEntry {
        entry_id: entry["id"].as_u64().map(|id| id as u32),
        media,
        status: entry["status"].as_str().unwrap_or("CURRENT").to_owned(),
        progress: entry["progress"].as_u64().unwrap_or(0) as u32,
        // AniList sends 0 for unscored. Keeping that as `Some(0.0)` would look like someone
        // deliberately rated a show zero, and would then be pushed back as such.
        score: entry["score"].as_f64().map(|s| s as f32).filter(|s| *s > 0.0),
        updated_at: entry["updatedAt"].as_i64().unwrap_or(0),
    })
}

fn decode_media_page(page: &serde_json::Value) -> Result<Page<Media>> {
    let items = page["media"]
        .as_array()
        .map(|arr| {
            // Skip individually-undecodable entries rather than failing the page: one
            // malformed title should not blank an entire screen.
            arr.iter()
                .filter_map(|m| match serde_json::from_value::<Media>(m.clone()) {
                    Ok(media) => Some(media),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping undecodable media entry");
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Page { items, has_next: has_next_page(page) })
}

/// A page of broadcasts, each with its title as far as the query asked for it. A row that does
/// not read is passed over rather than failing the page.
fn airing_page(page: &serde_json::Value) -> Page<AiringEntry> {
    let items = page["airingSchedules"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|entry| {
                    Some(AiringEntry {
                        episode: entry.get("episode")?.as_u64()? as u32,
                        airing_at: entry.get("airingAt")?.as_i64()?,
                        media: serde_json::from_value(entry.get("media")?.clone()).ok()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Page { items, has_next: has_next_page(page) }
}

fn has_next_page(page: &serde_json::Value) -> bool {
    page["pageInfo"]["hasNextPage"].as_bool().unwrap_or(false)
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seasons_cycle_in_both_directions() {
        assert_eq!(Season::Winter.next(), Season::Spring);
        assert_eq!(Season::Fall.next(), Season::Winter, "must wrap");
        assert_eq!(Season::Winter.previous(), Season::Fall, "must wrap");
        for s in [Season::Winter, Season::Spring, Season::Summer, Season::Fall] {
            assert_eq!(s.next().previous(), s);
        }
    }

    #[test]
    fn months_map_to_the_right_season() {
        assert_eq!(Season::of_month(1), Season::Winter);
        assert_eq!(Season::of_month(4), Season::Spring);
        assert_eq!(Season::of_month(7), Season::Summer);
        assert_eq!(Season::of_month(10), Season::Fall);
        assert_eq!(Season::of_month(12), Season::Fall);
    }

    #[test]
    fn a_media_page_decodes_and_reports_pagination() {
        let payload = serde_json::json!({
            "pageInfo": {"hasNextPage": true},
            "media": [{"id": 154587, "title": {"romaji": "Hoshikuzu no Tabibito"}}]
        });
        let page = decode_media_page(&payload).unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.has_next);
    }

    #[test]
    fn one_malformed_entry_does_not_blank_the_page() {
        // A single bad title should cost that title, not the whole screen.
        let payload = serde_json::json!({
            "pageInfo": {"hasNextPage": false},
            "media": [
                {"id": 1, "title": {"romaji": "Good"}},
                {"title": {"romaji": "Missing its id"}},
                {"id": 3, "title": {"romaji": "Also good"}}
            ]
        });
        let page = decode_media_page(&payload).unwrap();
        assert_eq!(page.items.len(), 2);
        assert!(!page.has_next);
    }

    #[test]
    fn an_empty_page_is_not_an_error() {
        // A search with no matches is a valid answer, not a failure.
        let page = decode_media_page(&serde_json::json!({"media": []})).unwrap();
        assert!(page.items.is_empty());
        assert!(!page.has_next);
    }

    #[test]
    fn missing_page_info_defaults_to_no_further_pages() {
        assert!(!has_next_page(&serde_json::json!({})));
    }

    #[test]
    fn the_broadcasts_asked_for_beside_a_title_are_put_on_it() {
        let data = serde_json::json!({
            "Media": {"id": 1, "title": {"romaji": "One"}},
            "aired": {"airingSchedules": [
                {"episode": 9, "airingAt": 900},
                {"episode": "not a number"},
                {"episode": 8, "airingAt": 800}
            ]}
        });
        let media = decode_detail(&data).unwrap();
        let aired: Vec<_> = media.aired.iter().map(|b| (b.episode, b.airing_at)).collect();
        assert_eq!(aired, [(9, 900), (8, 800)], "a malformed row is passed over");

        // A title answered without them is a title all the same.
        let bare = decode_detail(&serde_json::json!({"Media": {"id": 1}})).unwrap();
        assert!(bare.aired.is_empty());
        assert!(matches!(
            decode_detail(&serde_json::json!({"Media": null})),
            Err(AniListError::NotFound)
        ));
    }

    #[test]
    fn the_viewer_carries_the_title_language_they_read_in() {
        let viewer = decode_viewer(&serde_json::json!({
            "id": 5150, "name": "fixture-viewer", "options": {"titleLanguage": "NATIVE_STYLISED"}
        }))
        .unwrap();
        assert_eq!(viewer.id, 5150);
        assert_eq!(viewer.name.as_deref(), Some("fixture-viewer"));
        assert_eq!(viewer.title_language, Some(TitleLanguage::Native));

        // Options AniList left out, or a language this build does not know, are no language.
        let bare = decode_viewer(&serde_json::json!({"id": 1})).unwrap();
        assert_eq!(bare.title_language, None);
        assert!(decode_viewer(&serde_json::json!({"name": "no id"})).is_err());
    }

    #[test]
    fn an_unauthenticated_client_refuses_viewer_queries_without_a_round_trip() {
        // Spending one of only 30 requests per minute to be told we are not logged in
        // would be a waste; we already know.
        let client = AniList::new(
            HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap(),
            30,
        );
        assert!(!client.is_authenticated());
        let err = futures_lite_block(client.viewer_id());
        assert!(matches!(err, Err(AniListError::Unauthenticated)));
    }

    #[test]
    fn requests_go_to_anilist_unless_a_test_says_otherwise() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let client = AniList::new(http, 30);
        assert_eq!(&*client.endpoint, ENDPOINT);
        let pointed = client.with_endpoint("http://127.0.0.1:9/graphql");
        assert_eq!(&*pointed.endpoint, "http://127.0.0.1:9/graphql");
    }

    /// Minimal blocking executor so this test needs no async runtime feature.
    fn futures_lite_block<T>(fut: impl Future<Output = T>) -> T {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(fut)
    }
}
