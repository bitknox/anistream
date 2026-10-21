//! Playback's pure half: the rules that decide what a playback records, when it offers a skip
//! and which tracks it picks, and the shapes a player reports in and is shown.
//!
//! No I/O, no process and no mpv. The policy takes [`PlaybackEvent`]s and returns what to do,
//! so any player that reports them can be driven by it: the terminal's mpv over its socket,
//! through anistream-player, which re-exports all of this at the paths it had there.

pub mod companion;
mod event;
pub mod protocol;
pub mod skip;
pub mod tracker;
pub mod tracks;

pub use companion::{Choice, ScriptEvent};
pub use event::PlaybackEvent;
pub use protocol::{Command, EndReason, Event};
pub use skip::{SkipInterval, SkipKind};
pub use tracker::{Action, PlaybackTracker};
pub use tracks::{TrackAction, TrackInput, TrackPolicy, TrackPreferences};
