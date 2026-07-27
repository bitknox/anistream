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

use std::sync::Arc;

use anistream_core::ids::AnilistId;
use anistream_net::{HttpClient, RateLimiter, ratelimit::parse_rate_headers};
use serde::de::DeserializeOwned;

pub use model::{ExternalLink, LastAired, Media, Relation, StreamingEpisode, Title};

const ENDPOINT: &str = "https://graphql.anilist.co";

#[derive(Debug, thiserror::Error)]
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

/// Which season a browse query is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Server-side browse filters.
///
/// Every field here is applied by AniList rather than locally, which is what makes a wide
/// filter surface affordable against a 30/minute budget.
#[derive(Debug, Clone, Default)]
pub struct BrowseFilter {
    pub genres: Vec<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub min_score: Option<u32>,
    pub sort: Option<String>,
}

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
}

impl AniList {
    pub fn new(http: HttpClient, requests_per_minute: u32) -> Self {
        Self {
            http,
            limiter: Arc::new(RateLimiter::per_minute(requests_per_minute)),
            token: Arc::new(std::sync::RwLock::new(None)),
        }
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
        self.limiter.acquire().await;

        let mut request = self
            .http
            .plain()
            .post(ENDPOINT)
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

    /// Full-text search.
    pub async fn search(&self, term: &str, page: u32, per_page: u32) -> Result<Page<Media>> {
        let data: serde_json::Value = self
            .execute(
                &query::search(),
                serde_json::json!({ "search": term, "page": page.max(1), "perPage": per_page }),
            )
            .await?;
        decode_media_page(&data["Page"])
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

    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        let data: serde_json::Value =
            self.execute(&query::by_id(), serde_json::json!({ "id": id.get() })).await?;
        let node = data.get("Media").filter(|m| !m.is_null()).ok_or(AniListError::NotFound)?;
        serde_json::from_value(node.clone()).map_err(|e| AniListError::Decode(e.to_string()))
    }

    /// Seasonal browse with server-side filters.
    pub async fn seasonal(
        &self,
        season: Season,
        year: u16,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let mut variables = serde_json::json!({
            "season": season.as_str(),
            "seasonYear": year,
            "page": page.max(1),
            "perPage": per_page,
            "sort": [filter.sort.clone().unwrap_or_else(|| "POPULARITY_DESC".into())],
        });
        // Omit absent filters entirely: AniList treats an explicit null differently from a
        // missing argument for some fields.
        if !filter.genres.is_empty() {
            variables["genres"] = serde_json::json!(filter.genres);
        }
        if let Some(f) = &filter.format {
            variables["format"] = serde_json::json!(f);
        }
        if let Some(s) = &filter.status {
            variables["status"] = serde_json::json!(s);
        }
        if let Some(m) = filter.min_score {
            variables["minScore"] = serde_json::json!(m);
        }

        let data: serde_json::Value = self.execute(&query::seasonal(), variables).await?;
        decode_media_page(&data["Page"])
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

        let page_node = &data["Page"];
        let items = page_node["airingSchedules"]
            .as_array()
            .map(|arr| {
                arr.iter()
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

        Ok(Page { items, has_next: has_next_page(page_node) })
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

        let mut out: Vec<LastAired> = data["Page"]["airingSchedules"]
            .as_array()
            .map(|arr| {
                arr.iter().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect()
            })
            .unwrap_or_default();
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
            variables.insert("score".into(), score.into());
        }

        let data: serde_json::Value =
            self.execute(query::SAVE_PROGRESS, serde_json::Value::Object(variables)).await?;
        // The entry id, so a caller that needs to remove the entry again can. `None` is not a
        // failure — the mutation succeeded either way.
        Ok(data["SaveMediaListEntry"]["id"].as_u64().map(|id| id as u32))
    }

    /// The authenticated user's id, for library queries.
    pub async fn viewer_id(&self) -> Result<u32> {
        if self.token().is_none() {
            return Err(AniListError::Unauthenticated);
        }
        let data: serde_json::Value =
            self.execute(query::VIEWER, serde_json::json!({})).await?;
        data["Viewer"]["id"]
            .as_u64()
            .map(|v| v as u32)
            .ok_or_else(|| AniListError::Decode("viewer response had no id".into()))
    }
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
            "media": [{"id": 154587, "title": {"romaji": "Sousou no Frieren"}}]
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

    /// Minimal blocking executor so this test needs no async runtime feature.
    fn futures_lite_block<T>(fut: impl Future<Output = T>) -> T {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(fut)
    }
}
