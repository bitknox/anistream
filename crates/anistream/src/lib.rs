//! Wiring: the parts of the binary that are worth exercising on their own.
//!
//! The binary keeps its terminal setup and event loop; everything that talks to the outside
//! world on behalf of the UI lives here, so an example or an integration test can drive the
//! *real* orchestration rather than a copy of it. That distinction matters most for
//! [`playback`], where the thing being verified — a torrent stream reaching mpv and the
//! resulting position landing in SQLite — cannot be covered by unit tests at all.

/// Configuration shared with the long-running background tasks.
///
/// The Settings screen edits the app's own copy, and anything holding a snapshot from
/// launch would keep acting on values the user has already changed — which is how a
/// download folder, a seeding choice or an upscaler stayed put until a restart. Readers
/// take a snapshot per unit of work rather than holding the guard: these are async tasks,
/// and a `std` guard held across an `await` is how a future stops being `Send`.
pub type SharedConfig = std::sync::Arc<std::sync::RwLock<anistream_core::config::Config>>;

pub mod artwork;
pub mod data;
pub mod digest;
pub mod downloads;
pub mod focus;
pub mod mend;
pub mod playback;
pub mod remux;
pub mod shaders;
pub mod sources;
pub mod tracking;
pub mod updates;
