//! Playing an episode: the session that runs it in mpv and records what was watched, the player
//! and the window kept between episodes, the picture and how hard it is pushed, the loopback proxy
//! that mends what some hosts serve, and the subtitles muxed into a finished download.
//!
//! What front ends, the command line and the probes call to play is named from here.

mod session;

pub mod mend;
pub mod picture;
pub mod player;
pub mod power;
pub mod prefetch;
pub mod remux;
pub mod resolve;
pub mod shaders;
pub mod slot;

pub use session::{
    EpisodeEntry, NextEpisode, PictureState, PlaySettings, PlaybackContext, fetch_skips,
    offered, play,
};
