//! Playback.
//!
//! mpv over JSON IPC is the reference player, but the [`anistream_core::traits::Player`] trait
//! is what makes the licensed path work at all: a Crunchyroll episode resolves to an external
//! deep link and is opened by [`external::ExternalPlayer`], needing no new machinery anywhere
//! else. Their streams are Widevine + PlayReady protected, so mpv cannot play them, and the
//! honest answer is to hand off rather than pretend.

pub mod external;
pub mod ipc;
pub mod mpv;
pub mod presence;
pub mod protocol;
pub mod skip;
pub mod tracker;

pub use external::ExternalPlayer;
pub use mpv::{Mpv, MpvSession, PlaybackEvent, PlayerError};
pub use presence::{Activity, Presence};
pub use protocol::{Command, EndReason, Event};
pub use skip::{SkipInterval, SkipKind};
pub use tracker::{Action, PlaybackTracker};
