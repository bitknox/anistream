//! Playing an episode, from the command to the window: the plays a front end asks for, the
//! streams found for them, behind the eyecatch or ahead of time, and the window handed from one
//! playback to the next; the session that runs an episode in mpv and records what was watched;
//! and the player, the window kept between episodes, the picture and how hard it is pushed, the
//! loopback proxy that mends what some hosts serve, and the subtitles muxed into a finished
//! download.
//!
//! What front ends, the command line and the probes call to play is named from here.

mod context;
mod session;

pub mod autonext;
pub mod handoff;
pub mod mend;
pub mod picture;
pub mod player;
pub mod power;
pub mod prefetch;
pub mod remux;
pub mod resolve;
pub mod shaders;
pub mod slot;
pub mod start;

pub use context::PlaybackContext;
pub use session::{
    EpisodeEntry, NextEpisode, PictureState, PlaySettings, fetch_skips, offered, play,
};
