//! The Discord presence for one playback: connected for exactly as long as something plays,
//! and nothing at all wherever it is turned off or Discord is not running, or the build has no
//! `presence` feature, Discord being a desktop program.

use crate::playback::PlaybackContext;

/// Where the player is, as the presence needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Playhead {
    pub position: f64,
    pub duration: Option<f64>,
    pub speed: f64,
    pub paused: bool,
    /// Held waiting for data.
    pub buffering: bool,
}

#[cfg(feature = "presence")]
pub(super) use live::Rpc;

#[cfg(feature = "presence")]
mod live;

/// No presence without the presence feature: every call does nothing.
#[cfg(not(feature = "presence"))]
pub(super) struct Rpc;

#[cfg(not(feature = "presence"))]
impl Rpc {
    pub(super) async fn start(
        _: &anistream_core::config::PresenceConfig,
        _: &PlaybackContext,
    ) -> Self {
        Self
    }

    pub(super) async fn observe(&mut self, _: Playhead) {}

    pub(super) async fn finish(&mut self) {}
}
