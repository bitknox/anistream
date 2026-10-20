//! What the effect layer keeps between one command and the next, on its own side of the seam.
//!
//! The interface's `App` held some of it, and the loop the rest, and `dispatch` read the config
//! and the episode table off the `App`. This is the engine side's half: what it was told the
//! config is and what the next play needs, the live player's controls, and the torrent stack the
//! settings stand down and back up. The engine's actor owns it, beside the `Services`, and
//! nothing here reads the interface.
//!
//! Not here: the receiving ends of the channels the actor reads, and the count the jobs are
//! numbered by, which `Engine::send` keeps so that it can say a job's number at once.

use std::sync::{Arc, RwLock};

use anistream_core::config::{Config, Paths};
use anistream_model::{
    command::PlayerCommand,
    playback::{EpisodeEntry, NextEpisode},
};
use anistream_providers::{torrent::TorrentSession, vpn::VpnGuard};
use tokio::sync::mpsc;

use crate::{
    SharedConfig,
    config::ConfigState,
    sources::{TorrentUp, torrent::TorrentTasks},
};

/// Where a bring-up of the torrent source reports in, tagged with the bring-up it answers.
pub type Arrivals = mpsc::UnboundedSender<(u64, Result<TorrentUp, String>)>;

/// The effect layer's own state: what commands read and leave for the commands after them.
pub struct EngineState {
    /// The config: the one in effect, which the work runs on and each playback builds its
    /// player from, so a setting changed on screen reaches the next one; and the file's, which
    /// a new source order is checked against, to tell a reorder from a change of which sources
    /// there are. What the front end last said with `UseConfig`, until the engine owns it.
    pub config: ConfigState,
    /// What the front end said the next play needs, with `PlayHints`, until that play takes it.
    ///
    /// Temporary, until P4 gives the engine its own record of the titles opened.
    pub hints: Option<PlayHints>,
    /// The live player's control channel. Replaced on each new playback, so a stale sender
    /// from a finished episode can never steer the current one.
    pub player_tx: Option<mpsc::UnboundedSender<PlayerCommand>>,
    /// The copy of the config the background tasks read: the download manager, the seeding
    /// watch and the stream cache's. Brought up to `config` whenever a setting is written, so
    /// "saved" and "in effect" are the same moment.
    pub shared_config: SharedConfig,
    /// The torrent stack's live parts, held so a settings change can tear them down and stand
    /// them back up without a restart.
    pub torrent_tasks: TorrentTasks,
    /// The VPN guard and the session it protects, once the torrent source is up. Replaced along
    /// with the tasks above when the settings stand the stack down and back up.
    pub vpn_guard: Option<(VpnGuard, Arc<TorrentSession>)>,
    /// Where the torrent source reports in once it is up. The actor holds the other end.
    pub arrivals: Arrivals,
}

impl EngineState {
    /// The engine side as it starts: on the config the app was launched with, and the file as
    /// it is at `paths`. No torrent stack yet, and `arrivals` for it to report on when one is
    /// brought up.
    pub fn new(config: Config, paths: &Paths, arrivals: Arrivals) -> Self {
        Self {
            shared_config: Arc::new(RwLock::new(config.clone())),
            config: ConfigState::new(config, paths),
            hints: None,
            player_tx: None,
            torrent_tasks: TorrentTasks::default(),
            vpn_guard: None,
            arrivals,
        }
    }
}

/// What a play takes from the `PlayHints` sent ahead of it. Nothing, when none was: no title to
/// announce, and an episode list the playback loads for itself.
#[derive(Debug, Default)]
pub struct PlayHints {
    /// The name the window being handed over announces the new playback by.
    pub title: String,
    /// The episode after this one, for the Up Next card.
    pub next: Option<NextEpisode>,
    /// The title's episodes, for the player's Episodes menu.
    pub episodes: Vec<EpisodeEntry>,
}
