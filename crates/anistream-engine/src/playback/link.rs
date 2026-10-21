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
//!
//! A [`PlayerOpener`] opens the window a link talks to, and [`opener`] is the one a play uses:
//! mpv's process with the `desktop` feature, and the player a front end hosts, [`hosted`],
//! without it, or on the desktop when the front end asks for it.

use std::{path::Path, sync::Arc};

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    traits::PlaybackRequest,
};
use anistream_model::playback::PlayerTheme;
use anistream_playback::{
    SkipKind,
    companion::{EpisodeInfo, Level, Media, Menus, NextUp, Status},
};
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::outbox::Outbox;

#[cfg(test)]
pub(crate) mod fake;
pub mod hosted;
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
    /// The playback driving this window from now: the one that opened it, or the next, which
    /// took it from the one before. What the link says, it says for that playback, through `tx`.
    /// Nothing for mpv, which hears only the session.
    fn attach(&self, _tx: &Outbox) {}

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

    /// Take the player's window into full screen, or out of it. mpv's own window is mpv's
    /// `fullscreen` property; a hosted player's is the host's window, which mpv inside it
    /// knows nothing of.
    async fn toggle_fullscreen(&self) -> Result<(), LinkError> {
        self.cycle("fullscreen").await
    }

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

/// A player window just opened: what it is told through, and what it reports on.
pub type Opened = (Box<dyn PlayerLink>, UnboundedReceiver<PlaybackEvent>);

/// What opens a player window for a playback, with nothing in it yet: files are loaded into it
/// through its link, which is what lets one window carry a whole binge.
#[async_trait]
pub trait PlayerOpener: Send + Sync {
    /// Open a window for `request`, which says how it starts: fullscreen, at what speed and
    /// volume. What fails to open says why, in words for the viewer.
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError>;

    /// Where a playback keeps its scratch files beside the player: the subtitles fetched ahead
    /// of it, and the next episode's still.
    fn scratch_dir(&self) -> &Path;
}

/// What a play opens its player with, from the config as it stands now; `None` where this build
/// has nothing to play in, and the play is refused.
///
/// The front end's `hosted` player when it hosts one: always without the `desktop` feature, and
/// on the desktop when it asks. Otherwise, with the `desktop` feature, mpv's child process,
/// built per play: see [`player`](crate::playback::player::player).
#[cfg_attr(not(feature = "desktop"), allow(unused_variables))]
pub fn opener(
    config: &Config,
    paths: &Paths,
    theme: &PlayerTheme,
    hosted: Option<Arc<dyn PlayerOpener>>,
) -> Option<Arc<dyn PlayerOpener>> {
    if hosted.is_some() {
        return hosted;
    }
    #[cfg(feature = "desktop")]
    return Some(Arc::new(crate::playback::player::player(config, paths, theme)));
    #[cfg(not(feature = "desktop"))]
    None
}

/// What the player's own interface is told: a state it shows until told otherwise, or a moment
/// to show.
#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    /// The loading card, for an episode being found or opened: `episode` of `title`, the title
    /// `id` names, with the card's own words in `card` and the title's art by URL where it is
    /// known.
    Starting {
        card: Status,
        id: AnilistId,
        title: String,
        episode: String,
        episode_title: Option<String>,
        cover_url: Option<String>,
        banner_url: Option<String>,
    },
    /// The file is playing: the loading card comes down.
    Playing,
    /// The episode on screen.
    Episode(EpisodeInfo),
    /// What plays next, for the Up Next card, with its still by URL; `None` when nothing does.
    /// `countdown` is how many seconds a hosted player's card counts down, `0` for none.
    Next { card: Option<NextUp>, still: Option<String>, countdown: u32 },
    /// The file opened at `position`, where the viewer left it.
    Resumed { position: f64 },
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
