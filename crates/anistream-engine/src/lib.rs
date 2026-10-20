//! The engine: the work behind the front ends, with no terminal in it.
//!
//! Everything that talks to the outside world on the interface's behalf lives here: playback
//! and the player, sources and their plugins, sync with the trackers, downloads, artwork, plugin
//! stores and the background jobs. The terminal front end keeps its terminal setup and event
//! loop in the binary. No terminal crate is a dependency, not even transitively, so a front end
//! that is not a terminal can link this; `tools/check-boundaries.sh` holds it to that in CI.
//!
//! An example or an integration test drives the *real* orchestration here rather than a copy of
//! it. That matters most for [`playback`], where the thing being verified — a torrent stream
//! reaching mpv and the resulting position landing in SQLite — cannot be covered by unit tests
//! at all.

/// Configuration shared with the long-running background tasks.
///
/// The Settings screen edits the app's own copy, and anything holding a snapshot from
/// launch would keep acting on values the user has already changed — which is how a
/// download folder, a seeding choice or an upscaler stayed put until a restart. Readers
/// take a snapshot per unit of work rather than holding the guard: these are async tasks,
/// and a `std` guard held across an `await` is how a future stops being `Send`.
pub type SharedConfig = std::sync::Arc<std::sync::RwLock<anistream_core::config::Config>>;

pub mod artwork;
pub mod background;
pub mod data;
pub mod downloads;
pub mod outbox;
pub mod playback;
pub mod plugin_stores;
pub mod sources;
pub mod tracking;
pub mod updates;

pub fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}
