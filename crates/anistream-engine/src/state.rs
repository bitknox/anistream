//! What the effect layer keeps between one command and the next, on its own side of the seam.
//!
//! The interface's `App` held some of it, and the loop the rest, and `dispatch` read the config
//! and the episode table off the `App`. This is the engine side's half: the config, which it
//! owns, its record of the titles the front end shows, the live player's controls, and the
//! torrent stack the settings stand down and back up. The engine's actor owns it, beside the
//! `Services`, and nothing here reads the interface.
//!
//! Not here: the receiving ends of the channels the actor reads, and the count the jobs are
//! numbered by, which `Engine::send` keeps so that it can say a job's number at once.

use std::sync::{Arc, RwLock};

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
};
use anistream_model::{command::PlayerCommand, playback::PlaybackId};
use anistream_providers::{torrent::TorrentSession, vpn::VpnGuard};
use tokio::sync::mpsc;

use crate::{
    SharedConfig,
    cache::TitleCache,
    config::ConfigState,
    outbox::Played,
    playback::autonext::Staging,
    sources::{TorrentUp, torrent::TorrentTasks},
    widgets::Schedule,
};

/// Where a bring-up of the torrent source reports in, tagged with the bring-up it answers.
pub type Arrivals = mpsc::UnboundedSender<(u64, Result<TorrentUp, String>)>;

/// The effect layer's own state: what commands read and leave for the commands after them.
pub struct EngineState {
    /// The config: the one in effect, which the work runs on and each playback builds its
    /// player from, so a setting changed on screen reaches the next one; and the file's, which
    /// a new source order is checked against, to tell a reorder from a change of which sources
    /// there are. The engine's own: front ends ask it for changes.
    pub config: ConfigState,
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
    /// Playbacks started so far: what the next one is numbered after, so that what each says
    /// is told apart from what another of the same episode says.
    pub playbacks: u64,
    /// The engine's record of the titles the front end shows: see [`TitleCache`].
    pub cache: TitleCache,
    /// Which playback Now Playing shows, and what it goes back to: see [`Staging`].
    pub staging: Staging,
    /// Where the app is, as the front end last said: in the background torrents wait.
    pub phase: anistream_model::command::AppState,
    /// Whether the network is metered, cellular or Low Data Mode, as the front end last said:
    /// downloads and seeding wait on one.
    pub metered: bool,
    /// Held by each new-episode check while it runs, so two asked for together take turns
    /// rather than both saying the same episode.
    pub episode_check: Arc<tokio::sync::Mutex<()>>,
    /// The week's broadcasts as the widgets' record last fetched them: see [`Schedule`].
    pub widget_schedule: Schedule,
}

impl EngineState {
    /// The engine side as it starts: on the config the app was launched with, and the file's,
    /// as given or as it is at `paths`. No torrent stack yet, and `arrivals` for it to report on
    /// when one is brought up.
    pub fn new(
        config: Config,
        saved: Option<Config>,
        paths: &Paths,
        arrivals: Arrivals,
    ) -> Self {
        Self {
            shared_config: Arc::new(RwLock::new(config.clone())),
            config: ConfigState::new(config, saved, paths),
            player_tx: None,
            torrent_tasks: TorrentTasks::default(),
            vpn_guard: None,
            arrivals,
            playbacks: 0,
            cache: TitleCache::default(),
            staging: Staging::default(),
            phase: Default::default(),
            metered: false,
            episode_check: Arc::default(),
            widget_schedule: Schedule::default(),
        }
    }

    /// The next playback, of `episode` of `id`, numbered, and the record told it is starting.
    pub(crate) fn next_playback(&mut self, id: AnilistId, episode: &str) -> Played {
        self.playbacks += 1;
        let played =
            Played { serial: PlaybackId::new(self.playbacks), id, episode: episode.to_owned() };
        self.cache.started(&played);
        played
    }
}
