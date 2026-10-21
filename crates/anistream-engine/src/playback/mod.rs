//! Playing an episode, from the command to the window: the plays a front end asks for, the
//! streams found for them, behind the eyecatch or ahead of time, and the window handed from one
//! playback to the next; the session that runs an episode in mpv and records what was watched;
//! and the player, the window kept between episodes, the picture and how hard it is pushed, the
//! loopback proxy that mends what some hosts serve, and the subtitles muxed into a finished
//! download.
//!
//! What front ends, the command line and the probes call to play is named from here.
//!
//! What plays in mpv is the `desktop` feature's: the plays themselves and the session that runs
//! them, the window and its handoff, the picture, the shaders and the battery. Without it a play
//! is answered with [`refuse`], and what is left is what any player would need: what a playback
//! is about, the streams found for it and ahead of it, the rules for what follows an ending,
//! the mending proxy, and the subtitles muxed into a download.

mod context;
#[cfg(feature = "desktop")]
mod session;

pub mod autonext;
#[cfg(feature = "desktop")]
pub mod handoff;
pub mod link;
pub mod mend;
#[cfg(feature = "desktop")]
pub mod picture;
#[cfg(feature = "desktop")]
pub mod player;
#[cfg(feature = "desktop")]
pub mod power;
pub mod prefetch;
pub mod remux;
pub mod resolve;
#[cfg(feature = "desktop")]
pub mod shaders;
#[cfg(feature = "desktop")]
pub mod slot;
#[cfg(feature = "desktop")]
pub mod start;

// What the episode table says of the episodes around this one: the front end sends them ahead
// of a play, so they are the model's, and named from here for whatever plays.
pub use anistream_model::playback::{EpisodeEntry, NextEpisode};
pub use context::PlaybackContext;
#[cfg(feature = "desktop")]
pub use session::{PictureState, PlaySettings, fetch_skips, offered, play};

/// What a play is answered with where there is no player to play it in, said of each play.
#[cfg(not(feature = "desktop"))]
pub const NO_PLAYER: &str = "playback isn't built into this front end";

/// Answer a play where the engine was built without the `desktop` feature, which carries the
/// mpv child process: an alert that says so, then the job's end. Nothing is staged, and nothing
/// ends, since nothing started.
///
/// For now. A front end on a phone is to play the stream itself, in its platform's player, once
/// the engine can hand one over.
#[cfg(not(feature = "desktop"))]
pub(crate) fn refuse(job: Option<crate::jobs::JobGuard>, tx: &crate::outbox::Outbox) {
    use anistream_model::{event::Event, notice::Notice};
    let _ = tx.send(Event::Toast(Notice::alert(NO_PLAYER)));
    drop(job);
}
