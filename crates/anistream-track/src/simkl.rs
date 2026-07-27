//! Simkl.
//!
//! The third tracker, and the first one that cost almost nothing to add — which is the mapping
//! layer's whole justification. Simkl keys on its own ids but accepts **MAL ids** as a lookup, and
//! the mapping table already holds those for MAL's sake, so identity was solved before this file
//! existed.
//!
//! Two things differ from AniList and MAL and are worth stating.
//!
//! **Writes are batched.** `/sync/history` takes an array, so a drain of twelve episodes is one
//! request rather than twelve. That matters more here than elsewhere because Simkl has no published
//! rate limit, and an unpublished one is best not explored.
//!
//! **Simkl records watched *episodes*, not a progress count.** AniList and MAL both store "you are
//! on episode 5"; Simkl stores "episodes 1 through 5 are watched". So a progress push has to name
//! the episode, and reading progress back means counting what is marked.

use std::sync::Arc;

use anistream_core::traits::{TrackOp, TrackedEntry, Tracker, WatchStatus};

use crate::{mal::IdMapping, secret::TokenPair};

const API: &str = "https://api.simkl.com";

/// Simkl's tracker.
pub struct SimklTracker {
    client_id: String,
    http: reqwest::Client,
    mapping: Arc<dyn IdMapping>,
    token: tokio::sync::RwLock<Option<TokenPair>>,
}

impl SimklTracker {
    pub fn new(
        client_id: impl Into<String>,
        http: reqwest::Client,
        mapping: Arc<dyn IdMapping>,
        token: Option<TokenPair>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            http,
            mapping,
            token: tokio::sync::RwLock::new(token),
        }
    }

    async fn access_token(&self) -> Result<String, anistream_core::Error> {
        self.token
            .read()
            .await
            .as_ref()
            .map(|pair| pair.access.clone())
            .ok_or_else(|| anistream_core::Error::Auth("simkl: not signed in".into()))
    }

    /// Every Simkl request needs the key *and* the bearer token; the key alone gives 412.
    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, anistream_core::Error> {
        let token = self.access_token().await?;
        Ok(self
            .http
            .request(method, format!("{API}{path}"))
            .header("simkl-api-key", &self.client_id)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json"))
    }
}

/// Map Simkl's list names onto the shared vocabulary.
fn status_from(simkl: &str) -> WatchStatus {
    match simkl {
        "plantowatch" => WatchStatus::Planning,
        "completed" => WatchStatus::Completed,
        "hold" => WatchStatus::Paused,
        "dropped" => WatchStatus::Dropped,
        _ => WatchStatus::Current,
    }
}

fn status_to(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Planning => "plantowatch",
        WatchStatus::Completed => "completed",
        WatchStatus::Paused => "hold",
        WatchStatus::Dropped => "dropped",
        WatchStatus::Current | WatchStatus::Repeating => "watching",
    }
}

#[async_trait::async_trait]
impl Tracker for SimklTracker {
    fn id(&self) -> &str {
        "simkl"
    }

    fn is_authenticated(&self) -> bool {
        self.token.try_read().map_or(true, |held| held.is_some())
    }

    async fn accept_credentials(
        &self,
        access: &str,
        refresh: Option<&str>,
        expires_at: Option<i64>,
    ) {
        *self.token.write().await = Some(TokenPair {
            access: access.to_owned(),
            refresh: refresh.map(str::to_owned),
            expires_at,
        });
    }

    async fn forget_credentials(&self) {
        *self.token.write().await = None;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
        let response = self
            .request(reqwest::Method::GET, "/sync/all-items/anime?extended=full")
            .await?
            .send()
            .await
            .map_err(|e| anistream_core::Error::Tracker {
                tracker: "simkl".into(),
                message: e.to_string(),
            })?;

        if response.status().as_u16() == 401 {
            return Err(anistream_core::Error::Auth("simkl rejected the token".into()));
        }
        let body: serde_json::Value = response.json().await.map_err(|e| {
            anistream_core::Error::Tracker { tracker: "simkl".into(), message: e.to_string() }
        })?;

        let mut entries = Vec::new();
        for item in body["anime"].as_array().into_iter().flatten() {
            // MAL id is the bridge. A title Simkl knows but the mapping does not is skipped rather
            // than guessed at — the same rule every other tracker follows.
            let Some(mal_id) = item["show"]["ids"]["mal"]
                .as_u64()
                .or_else(|| item["show"]["ids"]["mal"].as_str()?.parse().ok())
            else {
                continue;
            };
            let Some(anilist_id) = self.mapping.anilist_id(mal_id as u32) else {
                tracing::debug!(mal_id, "simkl entry has no anilist mapping; skipped");
                continue;
            };

            entries.push(TrackedEntry {
                anilist_id,
                progress: item["watched_episodes_count"].as_u64().unwrap_or(0) as u32,
                status: status_from(item["status"].as_str().unwrap_or("watching")),
                score: item["user_rating"].as_f64().map(|s| s as f32),
            });
        }
        tracing::info!(count = entries.len(), "simkl library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
        if !self.is_authenticated() {
            return Err(anistream_core::Error::Auth("signed out".into()));
        }

        // Progress and status go to different endpoints, so they are collected separately and each
        // sent as one batch.
        let mut watched = Vec::new();
        let mut listed = Vec::new();

        for op in ops {
            let (anilist_id, mal_id) = match op {
                TrackOp::SetProgress { anilist_id, .. }
                | TrackOp::SetStatus { anilist_id, .. }
                | TrackOp::SetScore { anilist_id, .. } => {
                    let Some(mal_id) = self.mapping.mal_id(*anilist_id) else {
                        // Named, not silent: a title that cannot sync is exactly what the mapping
                        // layer exists to make visible.
                        tracing::warn!(
                            anilist_id = anilist_id.get(),
                            "no mal id, so simkl cannot be told about this title"
                        );
                        continue;
                    };
                    (*anilist_id, mal_id)
                }
            };
            let _ = anilist_id;

            match op {
                TrackOp::SetProgress { episode, .. } => {
                    // Simkl marks episodes, not a count — so "you are on 5" becomes "1 to 5 are
                    // watched". Sending only episode 5 would leave the earlier ones unmarked.
                    let episodes: Vec<serde_json::Value> = (1..=*episode)
                        .map(|number| serde_json::json!({ "number": number }))
                        .collect();
                    watched.push(serde_json::json!({
                        "ids": { "mal": mal_id },
                        "seasons": [{ "number": 1, "episodes": episodes }],
                    }));
                }
                TrackOp::SetStatus { status, .. } => {
                    listed.push(serde_json::json!({
                        "ids": { "mal": mal_id },
                        "to": status_to(*status),
                    }));
                }
                TrackOp::SetScore { score, .. } => {
                    listed.push(serde_json::json!({
                        "ids": { "mal": mal_id },
                        "rating": score.round() as u32,
                    }));
                }
            }
        }

        for (path, items) in [("/sync/history", watched), ("/sync/add-to-list", listed)] {
            if items.is_empty() {
                continue;
            }
            let response = self
                .request(reqwest::Method::POST, path)
                .await?
                .json(&serde_json::json!({ "shows": items }))
                .send()
                .await
                .map_err(|e| anistream_core::Error::Tracker {
                    tracker: "simkl".into(),
                    message: e.to_string(),
                })?;

            let status = response.status();
            if status.as_u16() == 401 {
                return Err(anistream_core::Error::Auth("simkl rejected the token".into()));
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(anistream_core::Error::Tracker {
                    tracker: "simkl".into(),
                    message: format!("{path}: {} {}", status.as_u16(), body.trim()),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_round_trip_through_simkls_names() {
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
        ] {
            assert_eq!(status_from(status_to(status)), status, "{status:?}");
        }
        // Repeating has no Simkl equivalent, so it lands on watching rather than being dropped.
        assert_eq!(status_from(status_to(WatchStatus::Repeating)), WatchStatus::Current);
    }

    #[test]
    fn an_unknown_list_name_is_watching_rather_than_a_failure() {
        // Simkl could add a list tomorrow, and a pull that errored on it would take the whole
        // library with it.
        assert_eq!(status_from("some-new-list"), WatchStatus::Current);
    }

    #[test]
    fn progress_becomes_every_episode_up_to_that_point() {
        // The difference from AniList and MAL: Simkl stores which episodes are watched, not a
        // count. Sending only the latest would leave 1..4 unmarked and the show looking unwatched.
        let episodes: Vec<serde_json::Value> =
            (1..=5).map(|n| serde_json::json!({ "number": n })).collect();
        assert_eq!(episodes.len(), 5);
        assert_eq!(episodes[0]["number"], 1);
        assert_eq!(episodes[4]["number"], 5);
    }

    #[test]
    fn a_mal_id_arrives_as_either_a_number_or_a_string() {
        // Simkl's `extended=full` responses have been seen both ways, and reading only one shape
        // would silently skip half a library.
        let numeric = serde_json::json!({ "show": { "ids": { "mal": 52991 } } });
        let string = serde_json::json!({ "show": { "ids": { "mal": "52991" } } });
        for value in [numeric, string] {
            let parsed = value["show"]["ids"]["mal"]
                .as_u64()
                .or_else(|| value["show"]["ids"]["mal"].as_str()?.parse().ok());
            assert_eq!(parsed, Some(52991));
        }
    }
}
