//! Playback.
//!
//! mpv over JSON IPC is the reference player, but the [`anistream_core::traits::Player`] trait
//! is what makes the licensed path work at all: an episode of a licensed service resolves to an
//! external deep link and is opened by [`external::ExternalPlayer`], needing no new machinery
//! anywhere else. Their streams are DRM protected, so mpv cannot play them, and the answer is to
//! hand off rather than pretend.

pub mod bundle;
pub mod companion;
pub mod external;
pub mod ipc;
pub mod mpv;
pub mod protocol;

/// The pure half of playback, which anistream-playback holds, at the paths it had here.
pub use anistream_playback::{skip, tracker, tracks};

pub use bundle::Bundle;
pub use companion::{Choice, ScriptEvent};
pub use external::ExternalPlayer;
pub use mpv::{Mpv, MpvSession, PlaybackEvent, PlayerError};
pub use protocol::{Command, EndReason, Event};
pub use skip::{SkipInterval, SkipKind};
pub use tracker::{Action, PlaybackTracker};
pub use tracks::{TrackAction, TrackInput, TrackPolicy, TrackPreferences};
