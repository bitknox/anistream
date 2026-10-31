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
use anistream_model::{Msg, command::PlayerCommand, playback::PlaybackId};
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

/// Where a bring-up of torrent playback reports in, tagged with the bring-up it answers.
pub type Arrivals = mpsc::UnboundedSender<(u64, Result<TorrentUp, Msg>)>;

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
    /// The playback started last, while it finds its stream: a Stop that reaches it before it
    /// opens a player sets this, and it opens none. Closed once it opens, so a Stop from then
    /// on is the session's, read off `player_tx` as every other control is. `None` for a play
    /// that opens at once.
    pub finding: Option<tokio::sync::watch::Sender<bool>>,
    /// The copy of the config the background tasks read: the download manager, the seeding
    /// watch and the stream cache's. Brought up to `config` whenever a setting is written, so
    /// "saved" and "in effect" are the same moment.
    pub shared_config: SharedConfig,
    /// The torrent stack's live parts, held so a settings change can tear them down and stand
    /// them back up without a restart.
    pub torrent_tasks: TorrentTasks,
    /// The VPN guard and the session it protects, once torrent playback is up. Replaced along
    /// with the tasks above when the settings stand the stack down and back up.
    pub vpn_guard: Option<(VpnGuard, Arc<TorrentSession>)>,
    /// Where torrent playback reports in once it is up. The actor holds the other end.
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
    /// The network the device uses, as the front end last said: on cellular only what the
    /// settings let run there runs, and in Low Data Mode downloads and seeding wait.
    pub network: crate::phases::Network,
    /// What to ask for again once the network is back: see [`crate::connectivity`].
    pub missed: crate::connectivity::Missed,
    /// Once "Up next for you" has been asked for: the titles on Continue when it was last sent,
    /// so that a change to them sends it again. See [`crate::catalog::up_next`].
    pub up_next_queue: Option<Vec<u32>>,
    /// Home as last sent, and the parts on their way: see [`crate::catalog::home`].
    pub home: crate::catalog::home::HomeMemory,
    /// Once Home has been asked for: where Continue stood when it was last loaded, so that a
    /// change to it sends Home again.
    pub home_mark: Option<u64>,
    /// When each title's sources were last checked from Home, so a title looked at twice in a
    /// row is asked about once.
    pub probed: std::collections::HashMap<anistream_core::ids::AnilistId, i64>,
    /// What the device last said of itself: see [`crate::autoplay`].
    pub device: anistream_model::trailer::Device,
    /// Whether trailers may start by themselves, as last said: see [`crate::autoplay`].
    pub autoplay: Option<anistream_model::trailer::AutoplayView>,
    /// The TV every play casts to, from [`Command::CastTo`] until casting stops: what auto-next
    /// opens its cast with when the TV's window was not kept.
    ///
    /// [`Command::CastTo`]: anistream_model::command::Command::CastTo
    #[cfg(feature = "cast")]
    pub casting: Option<std::sync::Arc<crate::playback::link::cast::CastOpener>>,
    /// What the torrent session was last told of it, for a setting saved to tell whether that
    /// changed: see [`crate::phases::gate_for`].
    pub gated: Option<crate::phases::Gate>,
    /// Held by each new-episode check while it runs, so two asked for together take turns
    /// rather than both saying the same episode.
    pub episode_check: Arc<tokio::sync::Mutex<()>>,
    /// The week's broadcasts as the widgets' record last fetched them: see [`Schedule`].
    pub widget_schedule: Schedule,
    /// Keep next: whether it runs, and the top-up asked for while one runs. See
    /// [`crate::downloads::ahead`].
    pub ahead: crate::downloads::ahead::Ahead,
    /// Search's kept answers and the search waiting or running: see
    /// [`crate::catalog::search::Searches`].
    pub search: crate::catalog::search::Searches,
    /// How far each title's dub goes, kept, and the lookup running: see
    /// [`crate::titles::dub`].
    pub dubs: crate::titles::dub::Dubs,
    /// The calendar's last window, kept ten minutes: see [`crate::catalog::calendar`].
    pub calendar: crate::catalog::calendar::CalendarMemory,
    /// Episode titles and stills another source gave a table without them, kept, and the
    /// lookup running: see [`crate::titles::names`].
    pub names: crate::titles::names::Names,
    /// The titles whose episodes were listed lately, for listing them again when the sources
    /// change: see [`crate::titles::refresh`].
    pub listed: crate::titles::refresh::Listed,
    /// The episode discussion open, and the episodes whose threads were looked up: see
    /// [`crate::discussion`].
    pub discussion: crate::discussion::Discussions,
    /// The watch party going, if one is: see [`crate::party`].
    #[cfg(feature = "party")]
    pub party: Option<crate::party::Running>,
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
            finding: None,
            torrent_tasks: TorrentTasks::default(),
            vpn_guard: None,
            arrivals,
            playbacks: 0,
            cache: TitleCache::default(),
            staging: Staging::default(),
            phase: Default::default(),
            network: Default::default(),
            missed: Default::default(),
            up_next_queue: None,
            home: Default::default(),
            home_mark: None,
            probed: Default::default(),
            device: Default::default(),
            autoplay: None,
            #[cfg(feature = "cast")]
            casting: None,
            gated: None,
            episode_check: Arc::default(),
            widget_schedule: Schedule::default(),
            ahead: Default::default(),
            search: Default::default(),
            dubs: Default::default(),
            calendar: Default::default(),
            names: Default::default(),
            listed: Default::default(),
            discussion: Default::default(),
            #[cfg(feature = "party")]
            party: None,
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
