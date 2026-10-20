//! What the effect layer keeps between one command and the next, on its own side of the seam.
//!
//! The interface's `App` held some of it, and the loop the rest, and `dispatch` read the config
//! and the episode table off the `App`, which numbered the jobs too. This is the engine side's
//! half: what it was told the config is and what the next play needs, the live player's
//! controls, the count the jobs are numbered by, and the torrent stack the settings stand down
//! and back up. The loop, `--preview` and the contract-test harness each keep one, as they each
//! keep a `Services`, and nothing here reads the interface. In P3e it becomes the actor's own
//! state.
//!
//! Not here: the receiving ends of the channels the loop waits on, which it selects over beside
//! the terminal's own, and anything of the terminal's.

use std::sync::{Arc, RwLock};

use anistream_core::config::Config;
use anistream_engine::{SharedConfig, sources::TorrentUp};
use anistream_model::{
    command::PlayerCommand,
    jobs::JobId,
    playback::{EpisodeEntry, NextEpisode},
};
use anistream_providers::{torrent::TorrentSession, vpn::VpnGuard};
use tokio::sync::mpsc;

use anistream_engine::sources::torrent::TorrentTasks;

/// Where a bring-up of the torrent source reports in, tagged with the bring-up it answers.
pub type Arrivals = mpsc::UnboundedSender<(u64, Result<TorrentUp, String>)>;

/// The effect layer's own state: what commands read and leave for the commands after them.
pub struct EngineState {
    /// The config in effect, as the front end last said with `UseConfig`. What the work runs
    /// on, and what each playback builds its player from, so a setting changed on screen
    /// reaches the next one.
    ///
    /// Temporary, a copy of the interface's own, until P4 makes the engine the config's owner.
    pub config: Config,
    /// The config as the file holds it, from the same `UseConfig`: what a new source order is
    /// checked against, to tell a reorder from a change of which sources there are.
    pub saved_config: Config,
    /// What the front end said the next play needs, with `PlayHints`, until that play takes it.
    ///
    /// Temporary, until P4 gives the engine its own record of the titles opened.
    pub hints: Option<PlayHints>,
    /// The live player's control channel. Replaced on each new playback, so a stale sender
    /// from a finished episode can never steer the current one.
    pub player_tx: Option<mpsc::UnboundedSender<PlayerCommand>>,
    /// Jobs numbered so far: see [`Self::number_job`].
    jobs: u64,
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
    /// Where the torrent source reports in once it is up. The loop holds the other end.
    pub arrivals: Arrivals,
}

impl EngineState {
    /// The engine side as it starts: on the config the app was launched with, as the file's
    /// too, as the interface starts, until the front end's first `UseConfig` says otherwise.
    /// No torrent stack yet, and `arrivals` for it to report on when one is brought up.
    pub fn new(config: Config, arrivals: Arrivals) -> Self {
        Self {
            shared_config: Arc::new(RwLock::new(config.clone())),
            saved_config: config.clone(),
            config,
            hints: None,
            player_tx: None,
            jobs: 0,
            torrent_tasks: TorrentTasks::default(),
            vpn_guard: None,
            arrivals,
        }
    }

    /// The next job's number: one each, from one, for as long as the app runs, so whatever
    /// ends a job can only ever end that one.
    pub fn number_job(&mut self) -> JobId {
        self.jobs += 1;
        JobId::new(self.jobs)
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
