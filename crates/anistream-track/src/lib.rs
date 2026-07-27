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
pub mod mal;
pub mod merge;
pub mod secret;
pub mod simkl;
pub mod sync;
pub mod trakt;

pub use anilist::AniListTracker;
pub use auth::{AuthError, Flow};
pub use device::{DeviceCode, DeviceEndpoints, DeviceError};
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
