//! MyAnimeList: PKCE auth, refresh, and the [`Tracker`] implementation.
//!
//! Worth reading side by side with [`crate::auth`], because MAL and AniList disagree on both halves
//! of the problem and each disagreement changed the code.
//!
//! | | AniList | MAL |
//! |---|---|---|
//! | Flow | code + **client secret** (measured: PKCE ignored) | code + **PKCE**, no secret |
//! | Token life | 364 days, no refresh token | **~31 days**, with a refresh token |
//! | Identity | AniList id — our primary key | **MAL id**, needs the mapping layer |
//!
//! Measured against the live endpoint: a token exchange carrying only `client_id` and
//! `code_verifier` gets *past* client authentication — MAL answers `invalid_request` about the code
//! itself, where AniList answers `invalid_client`. So MAL is a public client and the plan's original
//! PKCE design works here, even though it did not there.
//!
//! Two consequences worth stating plainly:
//!
//! - **Refresh is not optional.** A month-long token means a client that only stored the access
//!   token would silently stop syncing every 31 days and look like it had forgotten the account.
//! - **This is the first tracker that needs the ID mapping.** AniList needed none, being the
//!   primary key. MAL keys on `mal_id`, so a title with no mapping entry cannot be synced until
//!   the datasets gain one. [`Tracker::can_address`] says so, and the drain holds that title's
//!   ops until the mappings change rather than letting them fail the batch or be dropped.

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::{
    auth::urlencode,
    held::{Held, Renews},
    secret::{TokenPair, TokenStore},
};

const AUTHORIZE: &str = "https://myanimelist.net/v1/oauth2/authorize";
const TOKEN: &str = "https://myanimelist.net/v1/oauth2/token";
const API: &str = "https://api.myanimelist.net/v2";

/// How many pages of a library to follow before deciding the server is not counting down.
///
/// A thousand entries a page, so this is a library nobody has — and the alternative to a
/// bound is trusting a remote to end a loop that allocates on every turn.
const MAX_LIBRARY_PAGES: u32 = 100;

/// A PKCE verifier and the challenge derived from it.
///
/// MAL supports only `code_challenge_method=plain`, which means the challenge *is* the verifier.
/// That is weaker than S256 — anyone who can read the authorize URL can replay it — but it is what
/// the server accepts, and the alternative is not using PKCE at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
}

impl Pkce {
    /// A fresh verifier.
    ///
    /// RFC 7636 requires 43–128 characters from an unreserved set. 64 is comfortably inside that
    /// and gives 380 bits from the OS generator.
    pub fn generate() -> Self {
        // Exactly 64 characters, all from PKCE's unreserved set. The length is the point: 256 is a
        // multiple of 64, so `byte % 64` is perfectly unbiased and needs no rejection sampling or
        // range API. (`.` and `~` are also unreserved but would make it 66 and reintroduce bias.)
        const ALPHABET: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

        use rand::Rng;
        let mut bytes = [0_u8; 64];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = bytes.iter().map(|b| ALPHABET[(*b % 64) as usize] as char).collect();
        Self { verifier }
    }

    /// The challenge to send. Equal to the verifier, because MAL only accepts `plain`.
    pub fn challenge(&self) -> &str {
        &self.verifier
    }

    /// Whether this verifier is one MAL will accept.
    pub fn is_valid(&self) -> bool {
        let len = self.verifier.len();
        (43..=128).contains(&len)
            && self
                .verifier
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
    }
}

/// The URL to open in a browser.
pub fn authorize_url(client_id: &str, pkce: &Pkce, redirect_uri: &str) -> String {
    format!(
        "{AUTHORIZE}?response_type=code&client_id={}&code_challenge={}&code_challenge_method=plain&redirect_uri={}",
        urlencode(client_id.trim()),
        urlencode(pkce.challenge()),
        urlencode(redirect_uri)
    )
}

/// [`authorize_url`], with a `state` to come back with the code: what a native app's sign-in
/// sheet opens, its redirect being the app's own URL scheme.
pub fn authorize_url_with_state(
    client_id: &str,
    pkce: &Pkce,
    redirect_uri: &str,
    state: &str,
) -> String {
    format!("{}&state={}", authorize_url(client_id, pkce, redirect_uri), urlencode(state))
}

/// Exchange an authorization code for a token pair.
pub async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    code: &str,
    verifier: &str,
    redirect_uri: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    post_token(http, &exchange_form(client_id, code, verifier, redirect_uri), now).await
}

/// What a code is exchanged with: the client, the code and its verifier, and nothing secret.
/// A client registered as "other", as an app's is, has no secret to send; PKCE is what proves
/// the code is this sign-in's. Form-encoded, not JSON: MAL's token endpoint rejects a JSON body.
fn exchange_form<'a>(
    client_id: &'a str,
    code: &'a str,
    verifier: &'a str,
    redirect_uri: &'a str,
) -> [(&'static str, &'a str); 5] {
    [
        ("grant_type", "authorization_code"),
        ("client_id", client_id.trim()),
        ("code", code.trim()),
        ("code_verifier", verifier),
        ("redirect_uri", redirect_uri),
    ]
}

/// Renew an access token.
pub async fn refresh(
    http: &reqwest::Client,
    client_id: &str,
    refresh_token: &str,
    now: i64,
) -> Result<TokenPair, crate::auth::AuthError> {
    let form = [
        ("grant_type", "refresh_token"),
        ("client_id", client_id.trim()),
        ("refresh_token", refresh_token.trim()),
    ];
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
            // `expires_in` is relative, so it is resolved to an instant here — storing the relative
            // value would make it meaningless the moment it is written to disk.
            expires_at: parsed["expires_in"].as_i64().map(|secs| now + secs),
        });
    }

    // MAL's `hint` is the useful part: "Cannot decrypt the authorization code" says far more than
    // `invalid_request` does, and `said` reads it ahead of the code.
    Err(AuthError::Exchange(crate::said(status, &body)))
}

/// MAL's `status` values.
///
/// Note what is missing: MAL has no `repeating` status, it has an `is_rewatching` flag alongside
/// `watching`. Mapping `Repeating` onto `watching` is therefore lossy but correct — the alternative
/// is refusing to sync a rewatch at all.
pub const fn status_to_mal(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Current | WatchStatus::Repeating => "watching",
        WatchStatus::Planning => "plan_to_watch",
        WatchStatus::Completed => "completed",
        WatchStatus::Paused => "on_hold",
        WatchStatus::Dropped => "dropped",
    }
}

pub fn status_from_mal(status: &str) -> WatchStatus {
    match status {
        "plan_to_watch" => WatchStatus::Planning,
        "completed" => WatchStatus::Completed,
        "on_hold" => WatchStatus::Paused,
        "dropped" => WatchStatus::Dropped,
        _ => WatchStatus::Current,
    }
}

/// Translates between AniList ids and MAL ids.
///
/// A trait rather than a `Store` handle so this crate does not depend on the store, and so the
/// tests can supply a fixed mapping. The mapping layer already carries `mal_id` for every title it
/// knows, which is exactly what the plan said would make a second tracker cheap.
pub trait IdMapping: Send + Sync {
    fn mal_id(&self, anilist_id: AnilistId) -> Option<u32>;
    fn anilist_id(&self, mal_id: u32) -> Option<AnilistId>;
}

/// The MyAnimeList tracker.
pub struct MalTracker {
    client_id: String,
    http: reqwest::Client,
    mapping: std::sync::Arc<dyn IdMapping>,
    /// The pair, renewed before it runs out and kept in step with other processes: see
    /// [`crate::held`].
    held: Held,
}

impl MalTracker {
    pub fn new(
        client_id: impl Into<String>,
        http: reqwest::Client,
        mapping: std::sync::Arc<dyn IdMapping>,
        tokens: Option<TokenPair>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            http,
            mapping,
            held: Held::new("mal", "myanimelist", tokens),
        }
    }

    /// Kept in step with the other processes signed in through `store`: a pair one renews is
    /// stored at once, and taken by the others rather than renewed again.
    #[must_use]
    pub fn with_token_store(mut self, store: TokenStore) -> Self {
        self.held.share(store);
        self
    }

    /// A renewed token pair the caller should write back, if a refresh happened and no store
    /// was given, or the store did not take it.
    ///
    /// Returned rather than persisted here because without a store this crate has no opinion
    /// about where tokens live — that is [`crate::secret::TokenStore`]'s job, and the binary
    /// owns both.
    pub async fn take_renewed(&self) -> Option<TokenPair> {
        self.held.take_renewed().await
    }
}

impl Renews for MalTracker {
    async fn renew(&self, refresh_token: &str, now: i64) -> Result<TokenPair, String> {
        refresh(&self.http, &self.client_id, refresh_token, now)
            .await
            .map_err(|e| e.to_string())
    }
}

#[async_trait::async_trait]
impl Tracker for MalTracker {
    fn id(&self) -> &str {
        "mal"
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

        let mut entries = Vec::new();
        // MAL paginates with an explicit `next` URL rather than page numbers.
        let mut next =
            Some(format!("{API}/users/@me/animelist?fields=list_status&limit=1000&nsfw=true"));

        let mut pages = 0_u32;
        while let Some(url) = next.take() {
            // The loop follows a URL the *server* chose, with the user's bearer token attached.
            // Two things follow from that and neither was checked: a `next` pointing at another
            // host would hand that host the token, and one pointing at itself is a loop that
            // grows `entries` until the process dies. A page is only followed if it is on the
            // API we started from, and only so many times — a thousand entries a page, so this
            // is a library nobody has.
            pages += 1;
            if pages > MAX_LIBRARY_PAGES {
                tracing::warn!(pages, "myanimelist pagination did not terminate; stopping");
                break;
            }
            if !url.starts_with(API) {
                tracing::warn!(url = %url, "myanimelist paged us off its own api; stopping");
                break;
            }

            let response = self
                .held
                .send(self, &mut token, |t| self.http.get(&url).bearer_auth(t))
                .await?;

            let payload = crate::page_json(response, "mal", "myanimelist").await?;

            for node in payload["data"].as_array().into_iter().flatten() {
                // A MAL entry with no mapping back to an AniList id cannot be reconciled against
                // local history, so it is skipped — with a count reported below.
                let Some(mal_id) = node["node"]["id"].as_u64().map(|id| id as u32) else {
                    continue;
                };
                let Some(anilist_id) = self.mapping.anilist_id(mal_id) else {
                    tracing::debug!(mal_id, "no anilist id mapped; skipping");
                    continue;
                };
                let status = &node["list_status"];
                entries.push(TrackedEntry {
                    anilist_id,
                    progress: status["num_episodes_watched"].as_u64().unwrap_or(0) as u32,
                    status: status_from_mal(status["status"].as_str().unwrap_or("watching")),
                    // MAL uses 0 for unscored, same as AniList.
                    score: status["score"].as_f64().map(|s| s as f32).filter(|s| *s > 0.0),
                });
            }

            next = payload["paging"]["next"].as_str().map(str::to_owned);
        }

        tracing::info!(count = entries.len(), "myanimelist library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
        let mut token = self.held.access(self, crate::now_epoch()).await?;

        // Coalesced per title, like the AniList tracker: `my_list_status` is a PATCH that sets
        // whichever fields are present, so three ops for one show are one request.
        let mut batched: Vec<(AnilistId, Vec<(&str, String)>)> = Vec::new();
        for op in ops {
            let id = op.anilist_id();
            let slot = match batched.iter_mut().find(|(existing, _)| *existing == id) {
                Some(slot) => slot,
                None => {
                    batched.push((id, Vec::new()));
                    batched.last_mut().expect("just pushed")
                }
            };
            match op {
                TrackOp::SetProgress { episode, .. } => {
                    slot.1.retain(|(k, _)| *k != "num_watched_episodes");
                    slot.1.push(("num_watched_episodes", episode.to_string()));
                }
                TrackOp::SetStatus { status, .. } => {
                    slot.1.retain(|(k, _)| *k != "status");
                    slot.1.push(("status", status_to_mal(*status).to_owned()));
                }
                TrackOp::SetScore { score, .. } => {
                    // MAL scores are integers 1–10; AniList's decimal scale has to be rounded.
                    slot.1.retain(|(k, _)| *k != "score");
                    slot.1.push(("score", score.round().clamp(0.0, 10.0).to_string()));
                }
            }
        }

        for (anilist_id, fields) in batched {
            if fields.is_empty() {
                continue;
            }
            // The drain holds back an op for a title with no MAL id (see `can_address`) until
            // the mappings change, so one only reaches here from a caller that skipped that
            // check. Failing the call for it would fail the *batch*, taking every other op
            // down with it, so it is skipped and said out loud.
            let Some(mal_id) = self.mapping.mal_id(anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no mal id mapped for this title; skipping it rather than stalling the queue"
                );
                continue;
            };
            let url = format!("{API}/anime/{mal_id}/my_list_status");
            let response = self
                .held
                .send(self, &mut token, |t| self.http.patch(&url).bearer_auth(t).form(&fields))
                .await?;

            if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(anistream_core::Error::Auth(
                    "myanimelist rejected the token".into(),
                ));
            }
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(tracker_error(crate::said(status, &body)));
            }
        }
        Ok(())
    }
}

fn tracker_error(message: String) -> anistream_core::Error {
    anistream_core::Error::Tracker { tracker: "mal".into(), message }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedMapping;
    impl IdMapping for FixedMapping {
        fn mal_id(&self, anilist_id: AnilistId) -> Option<u32> {
            (anilist_id.get() == 154_587).then_some(52_991)
        }
        fn anilist_id(&self, mal_id: u32) -> Option<AnilistId> {
            (mal_id == 52_991).then(|| AnilistId::new(154_587))
        }
    }

    fn tracker(tokens: Option<TokenPair>) -> MalTracker {
        MalTracker::new(
            "cb3abe18b2ae1dd45209a09f0c89d050",
            reqwest::Client::new(),
            std::sync::Arc::new(FixedMapping),
            tokens,
        )
    }

    #[test]
    fn a_generated_verifier_is_one_mal_will_accept() {
        // RFC 7636: 43–128 characters from an unreserved set. Outside that range MAL rejects the
        // authorize request, which would look like a bad client id.
        for _ in 0..50 {
            let pkce = Pkce::generate();
            assert!(pkce.is_valid(), "generated {:?}", pkce.verifier);
            assert_eq!(pkce.verifier.len(), 64);
        }
    }

    #[test]
    fn generated_verifiers_differ() {
        // A fixed verifier would make the challenge replayable across sign-ins.
        let a = Pkce::generate();
        let b = Pkce::generate();
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn an_out_of_range_verifier_is_rejected() {
        assert!(!Pkce { verifier: "short".into() }.is_valid());
        assert!(!Pkce { verifier: "a".repeat(129) }.is_valid());
        // `+` and `/` are base64 characters that PKCE's unreserved set excludes.
        assert!(!Pkce { verifier: format!("{}+/", "a".repeat(43)) }.is_valid());
        assert!(Pkce { verifier: "a".repeat(43) }.is_valid());
    }

    #[test]
    fn the_challenge_equals_the_verifier_because_mal_only_does_plain() {
        // Weaker than S256, and worth being explicit about rather than looking like an oversight.
        let pkce = Pkce::generate();
        assert_eq!(pkce.challenge(), pkce.verifier);
    }

    #[test]
    fn a_code_is_exchanged_with_its_verifier_and_no_secret() {
        let form =
            exchange_form(" app-client ", "a-code", "a-verifier", "anistream://auth/mal");
        assert_eq!(
            form,
            [
                ("grant_type", "authorization_code"),
                ("client_id", "app-client"),
                ("code", "a-code"),
                ("code_verifier", "a-verifier"),
                ("redirect_uri", "anistream://auth/mal"),
            ]
        );
        assert!(form.iter().all(|(name, _)| !name.contains("secret")));
    }

    #[test]
    fn the_authorize_url_carries_pkce_and_no_secret() {
        let pkce = Pkce { verifier: "v".repeat(64) };
        let url = authorize_url("abc123", &pkce, "http://127.0.0.1:45617/callback");
        assert!(url.contains("response_type=code"), "{url}");
        assert!(url.contains("code_challenge_method=plain"), "{url}");
        assert!(url.contains(&format!("code_challenge={}", "v".repeat(64))), "{url}");
        assert!(url.contains("client_id=abc123"), "{url}");
        // Measured: MAL accepts a public client, so a secret must never appear here.
        assert!(!url.contains("secret"), "{url}");
        assert!(
            url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A45617%2Fcallback"),
            "the redirect must be encoded: {url}"
        );
    }

    #[test]
    fn every_status_maps_and_comes_back() {
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
        ] {
            assert_eq!(status_from_mal(status_to_mal(status)), status, "{status:?}");
        }
    }

    #[test]
    fn repeating_degrades_to_watching_because_mal_has_no_such_status() {
        // MAL models a rewatch as `watching` plus an `is_rewatching` flag. Lossy, but refusing to
        // sync a rewatch at all would be worse.
        assert_eq!(status_to_mal(WatchStatus::Repeating), "watching");
        assert_eq!(status_from_mal("watching"), WatchStatus::Current);
    }

    #[test]
    fn the_wire_strings_are_mals_and_not_ours() {
        // These go into someone's list; a rename here would silently change what is written.
        assert_eq!(status_to_mal(WatchStatus::Planning), "plan_to_watch");
        assert_eq!(status_to_mal(WatchStatus::Paused), "on_hold");
    }

    #[test]
    fn an_unknown_status_degrades_rather_than_failing() {
        assert_eq!(status_from_mal("something_new"), WatchStatus::Current);
        assert_eq!(status_from_mal(""), WatchStatus::Current);
    }

    #[tokio::test]
    async fn an_unmapped_title_does_not_stall_the_queue_behind_it() {
        // A title the datasets carry no `mal_id` for can never be pushed. Reporting that as an
        // error failed the whole claimed batch, and since nothing discards an op that cannot
        // succeed, the same twenty-five ops retried forever — one obscure title holding up
        // every other title's progress, re-sending the already-applied ones hourly.
        //
        // It is logged rather than returned, which is the shape Trakt already uses for a show
        // it cannot map. Visibility comes from the log line, not from stopping the queue.
        let tracker =
            tracker(Some(TokenPair { access: "tok".into(), refresh: None, expires_at: None }));
        let unmapped = AnilistId::new(999_999);

        let result =
            tracker.push(&[TrackOp::SetProgress { anilist_id: unmapped, episode: 3 }]).await;

        assert!(
            result.is_ok(),
            "an unmappable title must not fail the batch it happens to share: {result:?}"
        );
    }

    #[test]
    fn a_title_is_addressable_only_once_the_mappings_name_it() {
        // What lets the drain hold an unmapped title's op instead of losing it.
        let tracker = tracker(None);
        let mapped = TrackOp::SetProgress { anilist_id: AnilistId::new(154_587), episode: 3 };
        let unmapped = TrackOp::SetProgress { anilist_id: AnilistId::new(999_999), episode: 3 };
        assert!(tracker.can_address(&mapped));
        assert!(!tracker.can_address(&unmapped));
    }

    #[tokio::test]
    async fn no_token_is_an_auth_error_not_a_retryable_one() {
        // So the outbox does not sit burning backoff on something only signing in can fix.
        let tracker = tracker(None);
        assert!(!tracker.is_authenticated());
        let err = tracker.pull_library().await.unwrap_err();
        assert!(matches!(err, anistream_core::Error::Auth(_)), "{err:?}");
    }

    #[tokio::test]
    async fn an_expired_token_with_no_refresh_token_is_an_auth_error() {
        let tracker = tracker(Some(TokenPair {
            access: "old".into(),
            refresh: None,
            expires_at: Some(0),
        }));
        let err = tracker.pull_library().await.unwrap_err();
        assert!(matches!(err, anistream_core::Error::Auth(_)), "{err:?}");
    }

    #[test]
    fn a_token_far_from_expiry_is_not_refreshed() {
        let now = 1_000_000;
        let pair = TokenPair {
            access: "tok".into(),
            refresh: Some("r".into()),
            expires_at: Some(now + 31 * 24 * 3_600),
        };
        assert!(!pair.needs_refresh(now, crate::held::MARGIN_SECS));
    }

    #[test]
    fn a_token_inside_the_margin_is_refreshed_early() {
        // Renewing early matters: a token that expires mid-request fails as a rejected credential
        // rather than an expired one, which sends you looking in the wrong place.
        let now = 1_000_000;
        let pair = TokenPair {
            access: "tok".into(),
            refresh: Some("r".into()),
            expires_at: Some(now + 3_600),
        };
        assert!(pair.needs_refresh(now, crate::held::MARGIN_SECS));
    }

    #[test]
    fn a_token_with_no_refresh_token_never_claims_to_need_refreshing() {
        // AniList's shape: nothing to refresh with, so the answer is always no.
        let pair = TokenPair { access: "tok".into(), refresh: None, expires_at: Some(0) };
        assert!(!pair.needs_refresh(1_000_000, crate::held::MARGIN_SECS));
    }
}
