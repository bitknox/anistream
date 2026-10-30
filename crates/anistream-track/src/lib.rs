//! Tracker sync.
//!
//! **Local is the source of truth; sync is a projection of it.** The app has to be fully usable
//! with no account and no network, so nothing about history depends on a tracker existing. What
//! goes outward is the small syncable part — progress, status, score — and it goes through a
//! durable queue rather than a live call, because progress recorded on a plane has to survive
//! the process exiting.
//!
//! The layout follows that split:
//!
//! - [`merge`] decides what to do when the two sides disagree. Pure, and the most heavily
//!   tested module here, because this is the code that can overwrite someone's list.
//! - [`sync`] drains the outbox and reconciles a pulled library. Boring under failure by
//!   design.
//! - [`auth`] gets a token. See its docs for why the plan's PKCE flow does not work against
//!   AniList.
//! - [`secret`] keeps the token in the OS keychain, or a `0600` file where there is none.
//! - [`anilist`] is the reference [`Tracker`](anistream_core::traits::Tracker) implementation.
//!
//! Adding a second tracker needs an auth flow and a `Tracker` impl — the mapping layer already
//! carries `mal_id`, `kitsu_id` and the rest, so identity is a solved problem rather than a new
//! subsystem.

pub mod anilist;
pub mod auth;
pub mod device;
mod held;
pub mod kitsu;
pub mod mal;
pub mod merge;
pub mod secret;
pub mod simkl;
pub mod sync;
pub mod trakt;

pub use anilist::AniListTracker;
pub use auth::{AuthError, Flow};
pub use device::{DeviceCode, DeviceEndpoints, DeviceError, DeviceFlow};
pub use kitsu::KitsuTracker;
pub use mal::MalTracker;
pub use merge::{Conflict, Field, LocalState, Merged};
pub use secret::{Storage, TokenPair, TokenStore};
pub use simkl::SimklTracker;
pub use sync::{DrainReport, PullReport, drain, pull, queue_progress};
pub use trakt::{SeasonMapping, TraktTracker};

/// What the project's requests call themselves, as `anistream-net`'s plain client does.
pub(crate) const USER_AGENT: &str = concat!("anistream/", env!("CARGO_PKG_VERSION"));

/// Seconds since the Unix epoch.
///
/// Duplicated rather than taken from the store, so this crate stays independent of it.
pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// A tracker's answer as JSON, or the error it amounts to.
///
/// A 401 is an auth error, which only signing in again fixes. Any other failure (a maintenance
/// page, a 500, a 429) is a tracker error the next sync retries. Reading a failure's body as if
/// it were a page would find no entries in it, and an outage would pass for an empty library.
///
/// `tracker` is the id errors carry (`mal`), `name` what the person reads (`myanimelist`).
pub(crate) async fn page_json(
    response: reqwest::Response,
    tracker: &str,
    name: &str,
) -> Result<serde_json::Value, anistream_core::Error> {
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(anistream_core::Error::Auth(format!("{name} rejected the token")));
    }
    let failed = |message: String| anistream_core::Error::Tracker {
        tracker: tracker.to_owned(),
        message,
    };
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(failed(said(status, &body)));
    }
    response.json().await.map_err(|e| failed(e.to_string()))
}

/// What a tracker's refusal says, for a person: the status, and the first sentence of the reason
/// when the body is JSON that gives one. Never the body itself, which can be a whole HTML page.
pub(crate) fn said(status: reqwest::StatusCode, body: &str) -> String {
    match reason_in(body) {
        Some(reason) => format!("{status}: {reason}"),
        None => status.to_string(),
    }
}

/// The first sentence of the reason a JSON error body gives: OAuth's `error_description`, MAL's
/// `hint`, a `message`, the first of an `errors` list, or the bare `error` code last.
fn reason_in(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let first = &parsed["errors"][0];
    let reason = [
        &parsed["error_description"],
        &parsed["hint"],
        &parsed["message"],
        &first["detail"],
        &first["message"],
        &first["title"],
        &parsed["error"],
    ]
    .into_iter()
    .find_map(|said| said.as_str().map(str::trim).filter(|said| !said.is_empty()))?;
    // One sentence, and not a long one.
    let end = reason.find(['\n', '\r']).unwrap_or(reason.len());
    let line = &reason[..end];
    let sentence = line.find(". ").map_or(line, |at| &line[..=at]);
    Some(sentence.chars().take(160).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(status: u16, body: &'static str) -> reqwest::Response {
        http::Response::builder().status(status).body(body).unwrap().into()
    }

    #[tokio::test]
    async fn a_maintenance_page_is_an_error_not_an_empty_library() {
        let body = r#"{"error":"maintenance","message":"temporarily in maintenance mode"}"#;
        let err = page_json(answer(503, body), "mal", "myanimelist").await.unwrap_err();
        let anistream_core::Error::Tracker { tracker, message } = err else {
            panic!("expected a tracker error, got {err:?}");
        };
        assert_eq!(tracker, "mal");
        assert!(message.starts_with("503"), "{message}");
        assert!(message.contains("maintenance"), "{message}");
    }

    #[tokio::test]
    async fn a_rate_limit_is_an_error_too() {
        let err = page_json(answer(429, "slow down"), "kitsu", "kitsu").await.unwrap_err();
        assert!(matches!(err, anistream_core::Error::Tracker { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn a_rejected_token_is_an_auth_error() {
        let err = page_json(answer(401, "{}"), "mal", "myanimelist").await.unwrap_err();
        let anistream_core::Error::Auth(message) = err else { panic!("{err:?}") };
        assert_eq!(message, "myanimelist rejected the token");
    }

    #[tokio::test]
    async fn a_good_answer_is_its_json() {
        let page = page_json(answer(200, r#"{"data":[]}"#), "simkl", "simkl").await.unwrap();
        assert!(page["data"].as_array().is_some_and(Vec::is_empty));
    }

    #[tokio::test]
    async fn a_long_error_body_is_cut_short() {
        let body: &'static str = Box::leak("x".repeat(5_000).into_boxed_str());
        let err = page_json(answer(500, body), "mal", "myanimelist").await.unwrap_err();
        let anistream_core::Error::Tracker { message, .. } = err else { unreachable!() };
        assert!(message.len() < 300, "{}", message.len());
    }

    #[test]
    fn a_refusal_is_its_status_and_one_sentence_of_why_never_a_page() {
        use reqwest::StatusCode;
        let page = "<!DOCTYPE html><html><body><h1>503 Service Unavailable</h1></body></html>";
        assert_eq!(said(StatusCode::SERVICE_UNAVAILABLE, page), "503 Service Unavailable");
        let oauth = r#"{"error":"invalid_grant","error_description":"The code has expired. Ask for a new one.\nTrace 42"}"#;
        assert_eq!(
            said(StatusCode::BAD_REQUEST, oauth),
            "400 Bad Request: The code has expired."
        );
        let mal =
            r#"{"error":"invalid_request","hint":"Cannot decrypt the authorization code"}"#;
        assert_eq!(
            said(StatusCode::BAD_REQUEST, mal),
            "400 Bad Request: Cannot decrypt the authorization code"
        );
        let listed = r#"{"errors":[{"title":"Forbidden","detail":"Not yours to change."}]}"#;
        assert_eq!(said(StatusCode::FORBIDDEN, listed), "403 Forbidden: Not yours to change.");
        assert_eq!(
            said(StatusCode::BAD_REQUEST, r#"{"error":"invalid_client"}"#),
            "400 Bad Request: invalid_client"
        );
        assert_eq!(said(StatusCode::INTERNAL_SERVER_ERROR, ""), "500 Internal Server Error");
    }
}
