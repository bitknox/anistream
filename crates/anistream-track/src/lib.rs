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
pub mod kitsu;
pub mod mal;
pub mod merge;
pub mod secret;
pub mod simkl;
pub mod sync;
pub mod trakt;

pub use anilist::AniListTracker;
pub use auth::{AuthError, Flow};
pub use device::{DeviceCode, DeviceEndpoints, DeviceError};
pub use kitsu::KitsuTracker;
pub use mal::MalTracker;
pub use merge::{Conflict, Field, LocalState, Merged};
pub use secret::{Storage, TokenPair, TokenStore};
pub use simkl::SimklTracker;
pub use sync::{DrainReport, PullReport, drain, pull, queue_progress};
pub use trakt::{SeasonMapping, TraktTracker};

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
        // Enough of the body to say why (a maintenance notice, a rate limit), not a whole page.
        let reason: String = body.trim().chars().take(200).collect();
        return Err(failed(format!("{status}: {reason}")));
    }
    response.json().await.map_err(|e| failed(e.to_string()))
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
}
