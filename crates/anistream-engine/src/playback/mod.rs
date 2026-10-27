//! Playing an episode, from the command to the window: the plays a front end asks for, the
//! streams found for them, behind the eyecatch or ahead of time, the servers among them tried in
//! the order their [`health`] says, and the window handed from one playback to the next; the
//! session that runs an episode in mpv and records what was watched; and the player, the window
//! kept between episodes, the picture and how hard it is pushed, the loopback proxy that mends
//! what some hosts serve, and the subtitles muxed into a finished download.
//!
//! What front ends, the command line and the probes call to play is named from here.
//!
//! The session runs in any build, behind the [`link`] it plays through. What is the `desktop`
//! feature's is mpv's child process, the player it is, and the battery as the system says it:
//! without it there is nothing yet to open a player with, and a play is answered with
//! [`refuse`]. What is left plays all the same: what a playback is about, the streams found for
//! it and ahead of it, the window kept between episodes, the picture and its shaders, the rules
//! for what follows an ending, the mending proxy, and the subtitles muxed into a download.

mod context;
mod session;

pub mod autonext;
pub mod handoff;
pub mod health;
pub mod link;
pub mod mend;
pub mod picture;
#[cfg(feature = "desktop")]
pub mod player;
pub mod power;
pub mod prefetch;
pub mod remux;
pub mod resolve;
pub mod seat;
pub mod shaders;
pub mod slot;
pub mod start;
pub mod stats;

// What the episode table says of the episodes around this one: the front end sends them ahead
// of a play, so they are the model's, and named from here for whatever plays.
pub use anistream_model::playback::{EpisodeEntry, NextEpisode};
pub use context::PlaybackContext;
pub use session::{
    Machine, NO_PARTY, PictureState, PlaySettings, STILL_WATCHING_AFTER, SkipTimes,
    UP_NEXT_COUNTDOWN, offered, play,
};

/// Answer a play where this build has no player to open, which without the `desktop` feature is
/// every play for now: an alert that says so, then the job's end. Nothing is staged, and nothing
/// ends, since nothing started.
///
/// A front end on a phone is to play the stream itself, in a player of its own, once the engine
/// can hand one over: see [`link::opener`].
pub(crate) fn refuse(job: Option<crate::jobs::JobGuard>, tx: &crate::outbox::Outbox) {
    use anistream_model::{event::Event, notice::Notice};
    let _ = tx.send(Event::Toast(Notice::alert(anistream_model::Msg::NoticeNoPlayer)));
    drop(job);
}
