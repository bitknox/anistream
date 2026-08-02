//! Kitsu: password-grant auth, refresh, and the [`Tracker`] implementation.
//!
//! Kitsu is the one tracker with **no app registration at all**: its token endpoint
//! accepts the OAuth password grant with no client id and no secret. That removes the
//! usual setup step, and adds the one obligation the other trackers never had — the
//! user's actual password passes through `--login` once. It is exchanged immediately
//! and never stored; what lands in the keychain is the same access/refresh pair every
//! other tracker keeps.
//!
//! | | MAL | Kitsu |
//! |---|---|---|
//! | Sign-in | browser redirect + PKCE | username and password, once |
//! | Token life | ~31 days, refreshed | ~30 days, refreshed |
//! | Identity | `mal_id` via the mapping | `kitsu_id` via the same mapping |
//! | Wire shape | plain JSON | JSON:API — ids are strings, writes name their type |
//!
//! The JSON:API shape drives the push design. Kitsu has no upsert: creating an entry
//! that exists is a validation error, updating one that does not is a 404. So every
//! push first asks which of the two it is doing — one extra request per *title*, on a
//! queue that batches per title, against an API without the 30/minute budget AniList
//! has.

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::secret::TokenPair;

const TOKEN: &str = "https://kitsu.io/api/oauth/token";
const API: &str = "https://kitsu.io/api/edge";
const JSON_API: &str = "application/vnd.api+json";

/// Renew this many seconds before expiry — same margin, same reasoning as MAL.
const REFRESH_MARGIN_SECS: i64 = 24 * 3_600;

/// Library pages to follow before deciding the server is not counting down.
/// Five hundred entries a page, so this is a library nobody has.
const MAX_LIBRARY_PAGES: u32 = 40;

/// Exchange a username and password for a token pair.
///
/// The password's only journey: it goes to Kitsu's token endpoint and nowhere else,
/// and the caller should drop it the moment this returns.
pub async fn login(
    http: &reqwest::Client,
    username: &str,
    password: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    let form = [
        ("grant_type", "password"),
        ("username", username.trim()),
        ("password", password),
    ];
    post_token(http, &form, now).await
}

/// Renew an access token.
pub async fn refresh(
    http: &reqwest::Client,
    refresh_token: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    let form = [("grant_type", "refresh_token"), ("refresh_token", refresh_token.trim())];
    post_token(http, &form, now).await
}

async fn post_token(
    http: &reqwest::Client,
    form: &[(&str, &str)],
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    use crate::auth::AuthError;

    let response = http
        .post(TOKEN)
        .form(form)
        .send()
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

    if let Some(access) = parsed["access_token"].as_str().filter(|t| !t.is_empty()) {
        return Ok(TokenPair {
            access: access.to_owned(),
            refresh: parsed["refresh_token"].as_str().map(str::to_owned),
            expires_at: parsed["expires_in"].as_i64().map(|secs| now + secs),
        });
    }

    let message = parsed["error_description"]
        .as_str()
        .or_else(|| parsed["error"].as_str())
        .unwrap_or(&body);
    Err(AuthError::Exchange(format!("{status}: {message}")))
}

/// Kitsu's `status` values.
pub const fn status_to_kitsu(status: WatchStatus) -> &'static str {
    match status {
        // Kitsu, like MAL, models rewatching as a flag rather than a status.
        WatchStatus::Current | WatchStatus::Repeating => "current",
        WatchStatus::Planning => "planned",
        WatchStatus::Completed => "completed",
        WatchStatus::Paused => "on_hold",
        WatchStatus::Dropped => "dropped",
    }
}

pub fn status_from_kitsu(status: &str) -> WatchStatus {
    match status {
        "planned" => WatchStatus::Planning,
        "completed" => WatchStatus::Completed,
        "on_hold" => WatchStatus::Paused,
        "dropped" => WatchStatus::Dropped,
        _ => WatchStatus::Current,
    }
}

/// Our 10-point score as Kitsu's `ratingTwenty`, clamped into its 2–20 band.
///
/// The floor matters: a 1/10 maps to 2, because Kitsu reserves 0 and 1 for "unrated"
/// and rejects them on write.
pub fn rating_twenty(score: f32) -> u32 {
    ((score * 2.0).round() as u32).clamp(2, 20)
}

/// Translates between AniList ids and Kitsu ids.
///
/// Its own trait rather than a reuse of [`crate::mal::IdMapping`], because a mapping
/// that answers for MAL and not Kitsu is common in the datasets and the two must be
/// able to disagree.
pub trait IdMapping: Send + Sync {
    fn kitsu_id(&self, anilist_id: AnilistId) -> Option<u32>;
    fn anilist_id(&self, kitsu_id: u32) -> Option<AnilistId>;
}

/// The Kitsu tracker.
pub struct KitsuTracker {
    http: reqwest::Client,
    mapping: std::sync::Arc<dyn IdMapping>,
    tokens: tokio::sync::RwLock<Option<TokenPair>>,
    /// Set when a refresh produced a new pair the caller should persist.
    pending_save: tokio::sync::RwLock<Option<TokenPair>>,
    /// The signed-in user's id, fetched once per process. JSON:API ids are strings.
    user_id: tokio::sync::RwLock<Option<String>>,
}

impl KitsuTracker {
    pub fn new(
        http: reqwest::Client,
        mapping: std::sync::Arc<dyn IdMapping>,
        tokens: Option<TokenPair>,
    ) -> Self {
        Self {
            http,
            mapping,
            tokens: tokio::sync::RwLock::new(tokens),
            pending_save: tokio::sync::RwLock::new(None),
            user_id: tokio::sync::RwLock::new(None),
        }
    }

    /// A renewed token pair the caller should write back, if a refresh happened.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.pending_save.write().await.take()
    }

    /// A usable access token, refreshing first if it is close to expiry.
    async fn access_token(&self, now: i64) -> Result<String, anistream_core::Error> {
        {
            let held = self.tokens.read().await;
            let Some(pair) = held.as_ref() else {
                return Err(anistream_core::Error::Auth("not signed in to kitsu".into()));
            };
            if !pair.needs_refresh(now, REFRESH_MARGIN_SECS) {
                return Ok(pair.access.clone());
            }
        }

        // Refresh under the write lock, re-checking, for the same reason as MAL: two
        // concurrent calls should produce one refresh, and the old refresh token dies
        // when a new one is issued.
        let mut held = self.tokens.write().await;
        let Some(pair) = held.as_ref() else {
            return Err(anistream_core::Error::Auth("not signed in to kitsu".into()));
        };
        if !pair.needs_refresh(now, REFRESH_MARGIN_SECS) {
            return Ok(pair.access.clone());
        }
        let Some(refresh_token) = pair.refresh.clone() else {
            return Err(anistream_core::Error::Auth(
                "kitsu token expired and there is no refresh token".into(),
            ));
        };

        tracing::info!("refreshing the kitsu token");
        let renewed = refresh(&self.http, &refresh_token, now)
            .await
            .map_err(|e| anistream_core::Error::Auth(e.to_string()))?;

        let access = renewed.access.clone();
        *self.pending_save.write().await = Some(renewed.clone());
        *held = Some(renewed);
        Ok(access)
    }

    /// The signed-in user's id, fetched once and kept.
    async fn user_id(&self, token: &str) -> Result<String, anistream_core::Error> {
        if let Some(id) = self.user_id.read().await.clone() {
            return Ok(id);
        }
        let payload = self
            .request(reqwest::Method::GET, &format!("{API}/users?filter[self]=true"), token)
            .send()
            .await
            .map_err(|e| tracker_error(e.to_string()))?;
        if payload.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(anistream_core::Error::Auth("kitsu rejected the token".into()));
        }
        let body: serde_json::Value =
            payload.json().await.map_err(|e| tracker_error(e.to_string()))?;
        let Some(id) = body["data"][0]["id"].as_str().map(str::to_owned) else {
            return Err(tracker_error("kitsu returned no user for this token".into()));
        };
        *self.user_id.write().await = Some(id.clone());
        Ok(id)
    }

    fn request(&self, method: reqwest::Method, url: &str, token: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .bearer_auth(token)
            .header("Accept", JSON_API)
            .header("Content-Type", JSON_API)
    }

    /// The existing library entry for one anime, if the user has one.
    async fn entry_id_for(
        &self,
        token: &str,
        user_id: &str,
        kitsu_id: u32,
    ) -> Result<Option<String>, anistream_core::Error> {
        let url = format!(
            "{API}/library-entries?filter[userId]={user_id}&filter[animeId]={kitsu_id}\
             &fields[libraryEntries]=status&page[limit]=1"
        );
        let body: serde_json::Value = self
            .request(reqwest::Method::GET, &url, token)
            .send()
            .await
            .map_err(|e| tracker_error(e.to_string()))?
            .json()
            .await
            .map_err(|e| tracker_error(e.to_string()))?;
        Ok(body["data"][0]["id"].as_str().map(str::to_owned))
    }
}

/// Coalesce a batch per title, like every other tracker: three ops for one show
/// become one write. Progress keeps the highest episode, because it is monotonic.
fn coalesce(ops: &[TrackOp]) -> Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> {
    let mut batched: Vec<(AnilistId, serde_json::Map<String, serde_json::Value>)> = Vec::new();
    for op in ops {
        let id = op.anilist_id();
        let slot = match batched.iter_mut().find(|(existing, _)| *existing == id) {
            Some((_, attributes)) => attributes,
            None => {
                batched.push((id, serde_json::Map::new()));
                &mut batched.last_mut().expect("just pushed").1
            }
        };
        match op {
            TrackOp::SetProgress { episode, .. } => {
                let highest =
                    slot.get("progress").and_then(serde_json::Value::as_u64).unwrap_or(0);
                slot.insert("progress".into(), u64::from(*episode).max(highest).into());
            }
            TrackOp::SetStatus { status, .. } => {
                slot.insert("status".into(), status_to_kitsu(*status).into());
            }
            TrackOp::SetScore { score, .. } => {
                slot.insert("ratingTwenty".into(), rating_twenty(*score).into());
            }
        }
    }
    batched
}

#[async_trait::async_trait]
impl Tracker for KitsuTracker {
    fn id(&self) -> &str {
        "kitsu"
    }

    fn is_authenticated(&self) -> bool {
        // `try_read` so the UI is never blocked by an in-flight refresh; a contended
        // lock means a refresh is happening, which means we are signed in.
        self.tokens.try_read().map_or(true, |held| held.is_some())
    }

    async fn accept_credentials(
        &self,
        access: &str,
        refresh: Option<&str>,
        expires_at: Option<i64>,
    ) {
        *self.tokens.write().await = Some(TokenPair {
            access: access.to_owned(),
            refresh: refresh.map(str::to_owned),
            expires_at,
        });
    }

    async fn forget_credentials(&self) {
        *self.tokens.write().await = None;
        *self.pending_save.write().await = None;
        // The user id belongs to the account, not the process.
        *self.user_id.write().await = None;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
        let now = crate::now_epoch();
        let token = self.access_token(now).await?;
        let user_id = self.user_id(&token).await?;

        let mut entries = Vec::new();
        // `include=anime` is what makes the relationship carry its id: JSON:API sends
        // resource linkage only for included relationships.
        let mut next = Some(format!(
            "{API}/library-entries?filter[userId]={user_id}&filter[kind]=anime\
             &fields[libraryEntries]=progress,status,ratingTwenty,anime\
             &include=anime&fields[anime]=id&page[limit]=500"
        ));

        let mut pages = 0_u32;
        while let Some(url) = next.take() {
            // The same two guards as MAL, for the same reasons: a `next` pointing off
            // the API would hand that host the bearer token, and one pointing at
            // itself is a loop that grows `entries` until the process dies.
            pages += 1;
            if pages > MAX_LIBRARY_PAGES {
                tracing::warn!(pages, "kitsu pagination did not terminate; stopping");
                break;
            }
            if !url.starts_with(API) {
                tracing::warn!(url = %url, "kitsu paged us off its own api; stopping");
                break;
            }

            let response = self
                .request(reqwest::Method::GET, &url, &token)
                .send()
                .await
                .map_err(|e| tracker_error(e.to_string()))?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(anistream_core::Error::Auth("kitsu rejected the token".into()));
            }
            let payload: serde_json::Value =
                response.json().await.map_err(|e| tracker_error(e.to_string()))?;

            for node in payload["data"].as_array().into_iter().flatten() {
                let Some(kitsu_id) = node["relationships"]["anime"]["data"]["id"]
                    .as_str()
                    .and_then(|id| id.parse::<u32>().ok())
                else {
                    continue;
                };
                let Some(anilist_id) = self.mapping.anilist_id(kitsu_id) else {
                    tracing::debug!(kitsu_id, "no anilist id mapped; skipping");
                    continue;
                };
                let attributes = &node["attributes"];
                entries.push(TrackedEntry {
                    anilist_id,
                    progress: attributes["progress"].as_u64().unwrap_or(0) as u32,
                    status: status_from_kitsu(
                        attributes["status"].as_str().unwrap_or("current"),
                    ),
                    // `ratingTwenty` is 2–20 or null; halved onto the shared scale.
                    score: attributes["ratingTwenty"]
                        .as_f64()
                        .filter(|r| *r > 0.0)
                        .map(|r| (r / 2.0) as f32),
                });
            }

            next = payload["links"]["next"].as_str().map(str::to_owned);
        }

        tracing::info!(count = entries.len(), "kitsu library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
        let now = crate::now_epoch();
        let token = self.access_token(now).await?;
        let user_id = self.user_id(&token).await?;

        for (anilist_id, attributes) in coalesce(ops) {
            if attributes.is_empty() {
                continue;
            }
            // Skipped and said out loud, never raised: failing the batch for one
            // unmappable title would hold every other title's progress behind it.
            let Some(kitsu_id) = self.mapping.kitsu_id(anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no kitsu id, so kitsu cannot be told about this title"
                );
                continue;
            };

            // Kitsu has no upsert, so ask which write this is. An entry appearing
            // between the check and the POST fails as a validation error and retries
            // through the outbox — by which time this check answers PATCH.
            let existing = self.entry_id_for(&token, &user_id, kitsu_id).await?;
            let response = match &existing {
                Some(entry_id) => {
                    let body = serde_json::json!({ "data": {
                        "id": entry_id,
                        "type": "libraryEntries",
                        "attributes": attributes,
                    }});
                    self.request(
                        reqwest::Method::PATCH,
                        &format!("{API}/library-entries/{entry_id}"),
                        &token,
                    )
                    .json(&body)
                    .send()
                    .await
                }
                None => {
                    // Creation demands a status — Kitsu answers 422 "status can't be
                    // blank" without one, and a bare progress push for a never-listed
                    // title would sit in the outbox retrying forever. Watching is the
                    // only honest default for an entry that exists because of a watch;
                    // an op that carries its own status still wins.
                    let mut attributes = attributes.clone();
                    attributes
                        .entry("status")
                        .or_insert_with(|| status_to_kitsu(WatchStatus::Current).into());
                    let body = serde_json::json!({ "data": {
                        "type": "libraryEntries",
                        "attributes": attributes,
                        "relationships": {
                            "anime": { "data": { "type": "anime", "id": kitsu_id.to_string() } },
                            "user": { "data": { "type": "users", "id": user_id } },
                        },
                    }});
                    self.request(
                        reqwest::Method::POST,
                        &format!("{API}/library-entries"),
                        &token,
                    )
                    .json(&body)
                    .send()
                    .await
                }
            }
            .map_err(|e| tracker_error(e.to_string()))?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(anistream_core::Error::Auth("kitsu rejected the token".into()));
            }
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(tracker_error(format!("kitsu answered {status}: {body}")));
            }
        }
        Ok(())
    }
}

fn tracker_error(message: String) -> anistream_core::Error {
    anistream_core::Error::Tracker { tracker: "kitsu".into(), message }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn every_status_survives_a_round_trip() {
        // These strings go into someone's list; a mismatch quietly marks a show dropped.
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
        ] {
            assert_eq!(status_from_kitsu(status_to_kitsu(status)), status);
        }
        // Lossy on purpose, like MAL: rewatching is a flag on Kitsu, not a status.
        assert_eq!(status_to_kitsu(WatchStatus::Repeating), "current");
    }

    #[test]
    fn scores_land_inside_kitsu_s_two_to_twenty_band() {
        assert_eq!(rating_twenty(10.0), 20);
        assert_eq!(rating_twenty(8.5), 17);
        // The floor: Kitsu reserves 0 and 1 for "unrated" and rejects them on write.
        assert_eq!(rating_twenty(1.0), 2);
        assert_eq!(rating_twenty(0.0), 2);
    }

    #[test]
    fn a_title_s_ops_coalesce_into_one_write() {
        let batched = coalesce(&[
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 3 },
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 7 },
            TrackOp::SetStatus { anilist_id: FRIEREN, status: WatchStatus::Current, at: 1 },
            TrackOp::SetScore { anilist_id: FRIEREN, score: 9.0, at: 2 },
        ]);
        assert_eq!(batched.len(), 1);
        let (id, attributes) = &batched[0];
        assert_eq!(*id, FRIEREN);
        // Progress is monotonic: the higher episode wins whatever order ops arrive in.
        assert_eq!(attributes["progress"], 7);
        assert_eq!(attributes["status"], "current");
        assert_eq!(attributes["ratingTwenty"], 18);
    }

    #[test]
    fn two_titles_stay_two_writes() {
        let other = AnilistId::new(1);
        let batched = coalesce(&[
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 3 },
            TrackOp::SetProgress { anilist_id: other, episode: 1 },
        ]);
        assert_eq!(batched.len(), 2);
    }
}
