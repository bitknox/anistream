//! The player a playback runs in, as the session sees it: what it tells the player, and what the
//! player reports back.
//!
//! The session decides everything a playback does: which stream, which tracks, when to offer a
//! skip, what the player's own interface shows. A [`PlayerLink`] is how those decisions reach a
//! player, in mpv's own vocabulary, since mpv is what every player here is: the terminal's
//! child process over its socket, and later the libmpv a native app embeds. What the player
//! reports comes back as [`PlaybackEvent`]s on a channel beside the link.
//!
//! The overlay is the player's own interface: the loading card, the Up Next card, the skip
//! button, the menus. mpv's is the companion script, told through `user-data` and script
//! messages; a native app draws its own from the same [`Overlay`] calls.

use anistream_playback::{
    SkipKind,
    companion::{EpisodeInfo, Level, Media, Menus, NextUp, Status},
};
use async_trait::async_trait;

#[cfg(feature = "desktop")]
mod mpv;

pub use anistream_playback::PlaybackEvent;

/// Why a player did not take what it was told, in words for a log line. Mostly that it has gone
/// away: what the session tells a player is fire and forget, and a player that has closed says
/// so through its events.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct LinkError(pub String);

/// What the session does to the player a playback runs in.
///
/// Called from the playback's task, one call at a time, in the order the session decides; a call
/// returns once the player has been told, not once it has acted, and what it did comes back as
/// events. Shared by the playback that holds the window and the slot that keeps it between
/// episodes, so it is `Send + Sync`.
#[async_trait]
pub trait PlayerLink: Send + Sync {
    /// Open `url` in place of whatever is playing, with `options` that belong to that file
    /// alone, as mpv's per-file options: the headers, the resume point, the title and the
    /// track languages.
    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError>;

    /// Attach an external subtitle track to the loaded file without selecting it.
    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError>;

    /// Set an mpv property.
    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError>;

    /// Flip a boolean property, such as `pause`, without reading it first: a read could be
    /// stale by the time the write lands.
    async fn cycle(&self, property: &'static str) -> Result<(), LinkError>;

    /// Step a numeric property, such as `speed`, by `delta`, clamped by the player.
    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError>;

    /// Seek to `seconds`, or by them when not `absolute`; exactly, in either case.
    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError>;

    /// Replace the upscaling chain with the shaders at `paths`, all at once. Empty turns
    /// upscaling off.
    async fn shaders(&self, paths: &[String]) -> Result<(), LinkError>;

    /// Add or remove one labelled audio filter, leaving any the viewer set alone.
    async fn audio_filter(&self, add: bool, filter: &str) -> Result<(), LinkError>;

    /// Ask the player to report the chapter list again, as it stands now: what decides, once
    /// the file has loaded, whether the stream brought chapters of its own.
    async fn restate_chapters(&self) -> Result<(), LinkError>;

    /// Tell the player's own interface something.
    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError>;

    /// Stop the file but keep the window, for the next one.
    async fn stop(&self) -> Result<(), LinkError>;

    /// Ask the player to close.
    async fn quit(&self) -> Result<(), LinkError>;

    /// Whether the player is still there to be told anything.
    fn is_alive(&self) -> bool;

    /// Whether the player has said it is closing, which it may say a moment before it has gone.
    fn has_quit(&self) -> bool;

    /// The one line of what the player said about itself most worth showing a viewer when
    /// nothing played, if it said anything.
    async fn diagnostics(&self) -> Option<String>;

    /// Close the player and wait for it, insisting if it does not go.
    async fn shutdown(self: Box<Self>);
}

/// What the player's own interface is told: a state it shows until told otherwise, or a moment
/// to show.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    /// The loading card, or playing.
    Status(Status),
    /// The episode on screen.
    Episode(EpisodeInfo),
    /// What plays next, for the Up Next card; `None` when nothing does.
    Next(Option<NextUp>),
    /// What the player's menus list.
    Menus(Menus),
    /// The badges for what the picture is and what is done to it.
    Media(Media),
    /// Where the seek bar's thumbnails are read from instead of the stream, when that differs.
    ThumbnailUrl(Option<String>),
    /// The playhead entered a segment that can be skipped, to `to`.
    SkipOffer { kind: SkipKind, to: f64 },
    /// The segment on offer has passed.
    SkipCleared,
    /// A segment was skipped automatically, from `from` to `to`, and can be undone.
    Skipped { kind: SkipKind, from: f64, to: f64 },
    /// A message on the video.
    Toast { text: String, level: Level },
    /// The stream has been held waiting for data this long, mid-episode.
    Stall { seconds: u64 },
    /// The stall is over.
    StallCleared,
}
