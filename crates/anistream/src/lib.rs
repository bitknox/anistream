//! Wiring: the parts of the binary that are worth exercising on their own.
//!
//! The binary keeps its terminal setup and event loop; everything that talks to the outside
//! world on behalf of the UI lives here, so an example or an integration test can drive the
//! *real* orchestration rather than a copy of it. That distinction matters most for
//! [`playback`], where the thing being verified — a torrent stream reaching mpv and the
//! resulting position landing in SQLite — cannot be covered by unit tests at all.

pub mod artwork;
pub mod data;
pub mod downloads;
pub mod mend;
pub mod playback;
pub mod remux;
pub mod shaders;
pub mod sources;
pub mod tracking;
pub mod updates;
