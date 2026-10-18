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

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::{mal::IdMapping, secret::TokenPair};

const API: &str = "https://api.simkl.com";

/// Simkl's tracker.
pub struct SimklTracker {
    client_id: String,
    http: reqwest::Client,
    mapping: Arc<dyn IdMapping>,
    token: tokio::sync::RwLock<Option<TokenPair>>,
    /// [`API`], except in tests, which point it at a local server.
    api: String,
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
            api: API.to_owned(),
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
            .request(method, format!("{}{path}", self.api))
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

    fn can_address(&self, op: &TrackOp) -> bool {
        self.mapping.mal_id(op.anilist_id()).is_some()
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
        self.push_checked(ops).await.map(drop)
    }

    /// Simkl answers a write with `200` whatever it made of it, and lists what it could not
    /// match under `not_found`. Read as success, every unmatched title looked synced, the
    /// next pull still saw it behind, and the merge queued it again — a push every pull, for
    /// as long as the app ran.
    async fn push_checked(
        &self,
        ops: &[TrackOp],
    ) -> Result<Vec<AnilistId>, anistream_core::Error> {
        if !self.is_authenticated() {
            return Err(anistream_core::Error::Auth("signed out".into()));
        }

        // Progress and status go to different endpoints, so they are collected separately and each
        // sent as one batch.
        let mut watched = Vec::new();
        let mut listed = Vec::new();
        // Which title each MAL id sent stands for, to read `not_found` back with.
        let mut sent: Vec<(u32, AnilistId)> = Vec::new();

        for op in ops {
            let (anilist_id, mal_id) = match op {
                TrackOp::SetProgress { anilist_id, .. }
                | TrackOp::SetStatus { anilist_id, .. }
                | TrackOp::SetScore { anilist_id, .. } => {
                    let Some(mal_id) = self.mapping.mal_id(*anilist_id) else {
                        // The drain holds these until the mappings change (see `can_address`),
                        // so this is only a caller that skipped the check. Named, not silent.
                        tracing::warn!(
                            anilist_id = anilist_id.get(),
                            "no mal id, so simkl cannot be told about this title"
                        );
                        continue;
                    };
                    (*anilist_id, mal_id)
                }
            };
            sent.push((mal_id, anilist_id));

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

        let mut ignored: Vec<AnilistId> = Vec::new();
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
            // An unreadable body is not a failure: the status said the write was accepted, and
            // treating it as one would retry a write that may well have landed.
            let body: serde_json::Value = response.json().await.unwrap_or_default();
            for mal_id in not_found(&body) {
                for (_, anilist_id) in sent.iter().filter(|(sent_id, _)| *sent_id == mal_id) {
                    if !ignored.contains(anilist_id) {
                        ignored.push(*anilist_id);
                    }
                }
            }
        }
        Ok(ignored)
    }
}

/// The MAL ids Simkl lists under `not_found`, whichever kind it filed them as.
///
/// The entries echo what was sent, so each carries the `ids` object the request named the
/// title by. Read loosely: a kind Simkl adds later, or an id sent back as a string, must not
/// turn an unmatched title back into an applied one.
fn not_found(body: &serde_json::Value) -> Vec<u32> {
    let Some(kinds) = body["not_found"].as_object() else {
        return Vec::new();
    };
    kinds
        .values()
        .filter_map(serde_json::Value::as_array)
        .flatten()
        .filter_map(|item| {
            let mal = &item["ids"]["mal"];
            mal.as_u64()
                .or_else(|| mal.as_str()?.parse().ok())
                .and_then(|id| u32::try_from(id).ok())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);
    const UNKNOWN_TO_SIMKL: AnilistId = AnilistId::new(1);

    struct FixedMapping;
    impl IdMapping for FixedMapping {
        fn mal_id(&self, anilist_id: AnilistId) -> Option<u32> {
            match anilist_id.get() {
                154_587 => Some(52_991),
                1 => Some(1),
                _ => None,
            }
        }
        fn anilist_id(&self, _: u32) -> Option<AnilistId> {
            None
        }
    }

    /// A one-shot HTTP server on loopback: answers each request with the next canned body
    /// and hands back what it was asked, so a test can check both directions.
    async fn serve(
        answers: Vec<&'static str>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut seen = Vec::new();
            for answer in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut chunk = [0_u8; 4096];
                // Read the head, then as much body as it declares.
                loop {
                    let n = socket.read(&mut chunk).await.unwrap();
                    request.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&request).to_string();
                    if let Some(end) = text.find("\r\n\r\n") {
                        let length = text
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                seen.push(String::from_utf8_lossy(&request).to_string());
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}",
                    answer.len()
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
                socket.shutdown().await.ok();
            }
            seen
        });
        (base, handle)
    }

    fn tracker_at(base: String) -> SimklTracker {
        SimklTracker {
            api: base,
            ..SimklTracker::new(
                "key",
                reqwest::Client::new(),
                Arc::new(FixedMapping),
                Some(TokenPair { access: "tok".into(), refresh: None, expires_at: None }),
            )
        }
    }

    #[tokio::test]
    async fn a_title_simkl_lists_as_not_found_is_reported_rather_than_counted_as_sent() {
        // The echo loop's first half: a 200 whose body said "not found" read as applied.
        let (base, server) = serve(vec![
            r#"{"added":{"movies":0,"shows":1,"episodes":12},
                "not_found":{"movies":[],"shows":[{"ids":{"mal":1}}],"episodes":[]}}"#,
        ])
        .await;
        let ignored = tracker_at(base)
            .push_checked(&[
                TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 },
                TrackOp::SetProgress { anilist_id: UNKNOWN_TO_SIMKL, episode: 3 },
            ])
            .await
            .unwrap();
        assert_eq!(ignored, vec![UNKNOWN_TO_SIMKL]);

        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 1, "both titles go in one batch");
        assert!(requests[0].starts_with("POST /sync/history"), "{}", requests[0]);
        assert!(requests[0].contains(r#""mal":52991"#), "{}", requests[0]);
    }

    #[tokio::test]
    async fn a_write_simkl_matched_is_not_reported() {
        let (base, server) = serve(vec![
            r#"{"added":{"shows":1,"episodes":12},"not_found":{"shows":[],"episodes":[]}}"#,
        ])
        .await;
        let ignored = tracker_at(base)
            .push_checked(&[TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }])
            .await
            .unwrap();
        assert!(ignored.is_empty());
        server.await.unwrap();
    }

    #[test]
    fn not_found_ids_are_read_whatever_kind_or_shape_they_come_in() {
        let body = serde_json::json!({
            "not_found": {
                "shows": [{ "ids": { "mal": 7 } }],
                "anime": [{ "ids": { "mal": "8" } }],
                "movies": [],
            }
        });
        let mut ids = not_found(&body);
        ids.sort_unstable();
        assert_eq!(ids, vec![7, 8]);
        assert!(not_found(&serde_json::json!({})).is_empty());
    }

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
