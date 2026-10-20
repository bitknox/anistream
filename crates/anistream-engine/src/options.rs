//! What an engine is started with.

use anistream_core::config::{Config, Paths};
use anistream_model::playback::PlayerTheme;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

/// Everything [`crate::engine::Engine::start`] needs to stand the engine up.
///
/// [`Self::new`] gives the app's own launch: the source chain config describes, built by the
/// engine with its torrent source and plugins coming up behind it, and the work the engine does
/// on its own. A still frame or a test changes the fields after.
pub struct EngineOptions {
    /// Where the config, the database, the caches and the tokens are.
    pub paths: Paths,
    /// The config the app was launched with: the file's, with whatever the command line
    /// changed for this session.
    pub config: Config,
    /// The config the file holds, when the front end has read it already, or `None` for the
    /// engine to read it from `paths`. What tells the session's own choices from the file's on
    /// the Settings screen.
    pub saved: Option<Config>,
    /// The client every request goes through, with the configured proxy and timeouts.
    pub http: HttpClient,
    /// The local database.
    pub store: Store,
    /// How the player's own interface is dressed: the front end's choice, from its palette.
    pub player_theme: PlayerTheme,
    /// The source chain, whole, for an engine that is not to build its own: a still frame
    /// waits for every source before it starts, and a test brings sources of its own.
    ///
    /// `None` for the app's launch, where the engine builds the chain config describes. The
    /// torrent source joins it once its VPN guard has passed, and the plugins once compiled,
    /// so that the first frame waits for neither. A chain given whole is final: nothing joins
    /// it, and its rows are said at launch.
    pub sources: Option<ProviderRegistry>,
    /// Whether the engine does its own work beside the commands: the sweeps of old files, the
    /// update check, the airing digest, the trackers' sync loops, the joined stores asked for
    /// their lists, the mapping datasets kept current, and the check of the player said at
    /// launch.
    ///
    /// Off for a still frame and for a test, which must reach no further than they are sent.
    /// P6 splits it for the platforms where some of it has no meaning.
    pub background: bool,
    /// Parts a test stands the engine up with instead of the ones it builds.
    #[cfg(feature = "testing")]
    pub parts: Option<Parts>,
}

impl EngineOptions {
    /// The options for the app's own launch: the engine builds its source chain and does its
    /// own work beside the commands.
    pub fn new(
        paths: Paths,
        config: Config,
        http: HttpClient,
        store: Store,
        player_theme: PlayerTheme,
    ) -> Self {
        Self {
            paths,
            config,
            saved: None,
            http,
            store,
            player_theme,
            sources: None,
            background: true,
            #[cfg(feature = "testing")]
            parts: None,
        }
    }
}

/// What a test stands the engine up with, in place of what the engine would build: a metadata
/// source pointed at a fake, and trackers and plugin stores that keep their tokens in a
/// temporary directory, the trackers' clients pointed at the fake too.
///
/// With these, nothing the engine does reaches the network or the user's keychain. An engine
/// started with them also says in its event stream what it did with each message it left
/// itself, and when it has nothing left to do: see
/// [`EventStream::next_traced`](crate::engine::EventStream::next_traced).
#[cfg(feature = "testing")]
pub struct Parts {
    /// The metadata source.
    pub meta: anistream_meta::Meta,
    /// The trackers, built with the test's token store.
    pub sync: crate::tracking::Sync,
    /// The plugin stores, built with the test's token store.
    pub stores: crate::plugin_stores::Stores,
}
