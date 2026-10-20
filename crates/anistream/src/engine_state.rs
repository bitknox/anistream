//! What the effect layer keeps between one command and the next, on its own side of the seam.
//!
//! The interface's `App` held some of it, and the loop the rest, and `dispatch` read the config
//! and the episode table off the `App`. This is the engine side's half: what it was told the
//! config is and what the next play needs, and the live player's controls. The loop, `--preview` and the contract-test harness each keep one, as they
//! each keep a `Services`, and nothing here reads the interface. In P3e it becomes the actor's
//! own state.

use anistream_core::config::Config;
use anistream_model::{
    command::PlayerCommand,
    playback::{EpisodeEntry, NextEpisode},
};
use tokio::sync::mpsc;

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
}

impl EngineState {
    /// The engine side as it starts: on the config the app was launched with, as the file's
    /// too, as the interface starts, until the front end's first `UseConfig` says otherwise.
    pub fn new(config: Config) -> Self {
        Self { saved_config: config.clone(), config, hints: None, player_tx: None }
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
