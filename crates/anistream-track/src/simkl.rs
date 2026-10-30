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
//!
//! A sign-in through Simkl's OAuth 2.0 lasts a week, and is renewed here with its refresh token a
//! day before it runs out, as MyAnimeList's is. One from its older PIN flow never runs out, and
//! comes with nothing to renew it with.
//!
//! Every request names the app, as Simkl asks: `app-name` and `app-version` in its URL, and the
//! project's User-Agent.

use std::sync::Arc;

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::{
    device::{self, DeviceEndpoints},
    held::{Held, Renews},
    mal::IdMapping,
    secret::{TokenPair, TokenStore},
};

const API: &str = "https://api.simkl.com";

/// What every request to Simkl carries in its URL, naming the app, as its docs ask.
pub(crate) const APP_PARAMS: &[(&str, &str)] =
    &[("app-name", "anistream"), ("app-version", env!("CARGO_PKG_VERSION"))];

/// Simkl's tracker.
pub struct SimklTracker {
    client_id: String,
    /// Sent only by an app Simkl registered as a server app; a device app has none.
    client_secret: Option<String>,
    http: reqwest::Client,
    mapping: Arc<dyn IdMapping>,
    /// The pair, renewed before it runs out and kept in step with other processes: see
    /// [`crate::held`].
    held: Held,
    /// Where a token is renewed: [`device::SIMKL`], except in tests.
    endpoints: DeviceEndpoints,
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
            client_secret: None,
            http,
            mapping,
            held: Held::new("simkl", "simkl", token),
            endpoints: device::SIMKL,
            api: API.to_owned(),
        }
    }

    /// The app's secret, for an app registered as a server app: its refreshes send it.
    #[must_use]
    pub fn with_client_secret(mut self, secret: Option<String>) -> Self {
        self.client_secret = secret.filter(|s| !s.trim().is_empty());
        self
    }

    /// Kept in step with the other processes signed in through `store`. Simkl keeps one live
    /// access token per sign-in, so without this the terminal and the app would end each
    /// other's every week.
    #[must_use]
    pub fn with_token_store(mut self, store: TokenStore) -> Self {
        self.held.share(store);
        self
    }

    /// A renewed token pair the caller should write back, if a refresh happened and no store
    /// was given, or the store did not take it.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.held.take_renewed().await
    }

    /// A request to Simkl. Each needs the key *and* the bearer token (the key alone gives 412),
    /// and names the app.
    fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        token: &str,
    ) -> reqwest::RequestBuilder {
        self.http
            .request(method, format!("{}{path}", self.api))
            .query(APP_PARAMS)
            .header("user-agent", crate::USER_AGENT)
            .header("simkl-api-key", &self.client_id)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
    }

    /// Send a request with a usable token, again once with a renewed one if Simkl rejects it.
    async fn send(
        &self,
        token: &mut String,
        build: impl Fn(&str) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, anistream_core::Error> {
        self.held.send(self, token, build).await
    }
}

impl Renews for SimklTracker {
    async fn renew(&self, refresh: &str, now: i64) -> Result<TokenPair, String> {
        device::refresh_token(
            &self.http,
            &self.endpoints,
            &self.client_id,
            self.client_secret.as_deref(),
            refresh,
            now,
        )
        .await
        .map_err(|e| e.to_string())
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
        self.held.is_held()
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
        self.held
            .accept(TokenPair {
                access: access.to_owned(),
                refresh: refresh.map(str::to_owned),
                expires_at,
            })
            .await;
    }

    async fn forget_credentials(&self) {
        self.held.forget().await;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        let response = self
            .send(&mut token, |t| {
                self.request(reqwest::Method::GET, "/sync/all-items/anime?extended=full", t)
            })
            .await?;

        let body = crate::page_json(response, "simkl", "simkl").await?;

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
        let mut token = self.held.access(self, crate::now_epoch()).await?;
        for (path, items) in [("/sync/history", watched), ("/sync/add-to-list", listed)] {
            if items.is_empty() {
                continue;
            }
            let body = serde_json::json!({ "shows": items });
            let response = self
                .send(&mut token, |t| self.request(reqwest::Method::POST, path, t).json(&body))
                .await?;

            let status = response.status();
            if status.as_u16() == 401 {
                return Err(anistream_core::Error::Auth("simkl rejected the token".into()));
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(anistream_core::Error::Tracker {
                    tracker: "simkl".into(),
                    message: format!("{path}: {}", crate::said(status, &body)),
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

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);
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
        serve_with(answers.into_iter().map(|answer| (200, answer)).collect()).await
    }

    /// [`serve`], each answer with its own status.
    async fn serve_with(
        answers: Vec<(u16, &'static str)>,
    ) -> (String, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            let mut seen = Vec::new();
            for (status, answer) in answers {
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
                let reason =
                    http::StatusCode::from_u16(status).unwrap().canonical_reason().unwrap();
                let reply = format!(
                    "HTTP/1.1 {status} {reason}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}",
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
    async fn a_token_close_to_running_out_is_renewed_first_and_handed_back_to_store() {
        let (base, server) = serve(vec![
            r#"{"access_token":"fresh","token_type":"Bearer","expires_in":604800,
                "refresh_token":"kept","scope":"media:read media:write"}"#,
            r#"{"anime":[]}"#,
        ])
        .await;
        let token_url: &'static str =
            Box::leak(format!("{base}/oauth2/token").into_boxed_str());
        let tracker = SimklTracker {
            endpoints: DeviceEndpoints { token_url, ..device::SIMKL },
            ..tracker_at(base)
        };
        let now = crate::now_epoch();
        // An hour left, inside the day's margin.
        tracker.accept_credentials("stale", Some("kept"), Some(now + 3_600)).await;
        tracker.pull_library().await.unwrap();

        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("POST /oauth2/token"), "{}", requests[0]);
        assert!(
            requests[0].ends_with("grant_type=refresh_token&client_id=key&refresh_token=kept"),
            "{}",
            requests[0]
        );
        assert!(requests[1].contains("authorization: Bearer fresh"), "{}", requests[1]);
        let renewed = tracker.take_renewed().await.expect("the renewed pair, to store");
        assert_eq!(
            (renewed.access.as_str(), renewed.refresh.as_deref()),
            ("fresh", Some("kept"))
        );
        assert!(renewed.expires_at.is_some_and(|at| at >= now + 604_800));
        assert!(tracker.take_renewed().await.is_none(), "handed back once");
    }

    /// What every request to Simkl carries in its URL.
    const NAMED: &str = concat!("app-name=anistream&app-version=", env!("CARGO_PKG_VERSION"));

    /// A tracker at `base` signed in through the store in `dir`, as one process would be, with
    /// its own store and so its own cache.
    fn process_at(base: &str, dir: &std::path::Path, pair: TokenPair) -> SimklTracker {
        let token_url: &'static str =
            Box::leak(format!("{base}/oauth2/token").into_boxed_str());
        let store = TokenStore::with_backend(dir, crate::secret::Backend::File);
        SimklTracker {
            endpoints: DeviceEndpoints { token_url, ..device::SIMKL },
            api: base.to_owned(),
            ..SimklTracker::new(
                "key",
                reqwest::Client::new(),
                Arc::new(FixedMapping),
                Some(pair),
            )
            .with_token_store(store)
        }
    }

    fn store_pair(dir: &std::path::Path, pair: &TokenPair) {
        TokenStore::with_backend(dir, crate::secret::Backend::File)
            .set_pair("simkl", &pair.access, pair.refresh.as_deref(), pair.expires_at)
            .unwrap();
    }

    #[tokio::test]
    async fn two_processes_on_one_store_renew_once_and_both_keep_working() {
        // The Mac app and the terminal, signed in once. Simkl keeps one live access token per
        // sign-in, so a second renewal would end the first one's.
        let (base, server) = serve(vec![
            r#"{"access_token":"second","expires_in":604800,"refresh_token":"kept"}"#,
            r#"{"anime":[]}"#,
            r#"{"anime":[]}"#,
        ])
        .await;
        let dir = tempfile::tempdir().unwrap();
        let now = crate::now_epoch();
        let first = TokenPair {
            access: "first".into(),
            refresh: Some("kept".into()),
            expires_at: Some(now + 3_600),
        };
        store_pair(dir.path(), &first);
        let app = process_at(&base, dir.path(), first.clone());
        let terminal = process_at(&base, dir.path(), first);

        app.pull_library().await.unwrap();
        terminal.pull_library().await.unwrap();

        let requests = server.await.unwrap();
        assert!(requests[0].starts_with("POST /oauth2/token"), "{}", requests[0]);
        assert!(requests[1].contains("authorization: Bearer second"), "{}", requests[1]);
        assert!(
            requests[2].contains("authorization: Bearer second"),
            "the terminal took the app's"
        );
        assert_eq!(requests.iter().filter(|r| r.contains("/oauth2/token")).count(), 1);
        assert!(app.take_renewed().await.is_none(), "stored when it was renewed");
    }

    #[tokio::test]
    async fn a_rejected_token_is_sent_again_with_the_pair_another_process_stored() {
        let (base, server) =
            serve_with(vec![(401, r#"{"error":"invalid_token"}"#), (200, r#"{"anime":[]}"#)])
                .await;
        let dir = tempfile::tempdir().unwrap();
        let week = crate::now_epoch() + 7 * 24 * 3_600;
        let held = TokenPair {
            access: "first".into(),
            refresh: Some("kept".into()),
            expires_at: Some(week),
        };
        let terminal = process_at(&base, dir.path(), held);
        // The app renewed since the terminal read the store.
        store_pair(
            dir.path(),
            &TokenPair {
                access: "second".into(),
                refresh: Some("kept".into()),
                expires_at: Some(week),
            },
        );

        terminal.pull_library().await.unwrap();

        let requests = server.await.unwrap();
        assert!(requests[0].contains("authorization: Bearer first"), "{}", requests[0]);
        assert!(requests[1].contains("authorization: Bearer second"), "{}", requests[1]);
        assert_eq!(requests.len(), 2, "taken from the store, not renewed");
    }

    #[tokio::test]
    async fn every_request_names_the_app() {
        let (base, server) =
            serve(vec![r#"{"anime":[]}"#, r#"{"added":{"shows":1},"not_found":{"shows":[]}}"#])
                .await;
        let tracker = tracker_at(base);
        tracker.pull_library().await.unwrap();
        tracker
            .push_checked(&[TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 1 }])
            .await
            .unwrap();
        let requests = server.await.unwrap();
        assert!(
            requests[0]
                .starts_with(&format!("GET /sync/all-items/anime?extended=full&{NAMED} ")),
            "{}",
            requests[0]
        );
        assert!(
            requests[1].starts_with(&format!("POST /sync/history?{NAMED} ")),
            "{}",
            requests[1]
        );
        for request in &requests {
            assert!(
                request.contains(&format!("user-agent: {}", crate::USER_AGENT)),
                "{request}"
            );
        }
    }

    #[tokio::test]
    async fn a_token_that_ran_out_with_nothing_to_renew_it_asks_for_a_new_sign_in() {
        // Nothing listens there: a refresh tried would fail differently.
        let tracker = tracker_at("http://127.0.0.1:9".into());
        tracker.accept_credentials("old", None, Some(1_000)).await;
        let e = tracker.held.access(&tracker, 2_000).await.unwrap_err();
        assert!(e.to_string().contains("no refresh token"), "{e}");
        // A PIN-flow token never runs out.
        tracker.accept_credentials("pin", None, None).await;
        assert_eq!(tracker.held.access(&tracker, 2_000).await.unwrap(), "pin");
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
                TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 },
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
            .push_checked(&[TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 }])
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
