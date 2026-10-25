//! The handles every effect shares, and how they are built.

use std::sync::Arc;

use anistream_core::config::{Config, Paths};
use anistream_meta::{Meta, Tenrai, anilist::AniList};
use anistream_model::playback::PlayerTheme;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_track::TokenStore;

use crate::{
    host::Host,
    options::AppClients,
    playback::{
        link::hosted::HostedPlayers,
        power::{PowerSource, ReportedPower},
        prefetch::Prefetch,
    },
    plugin_stores, tracking,
};

/// The handles every effect shares, which the command line uses too.
///
/// Built once at launch and lent to every task, which clones the handles it needs. Each one is
/// a handle onto state its clones share, and none is replaced while the app runs: a rebuilt
/// source chain is swapped into the same registry, a sign-in is handed to the tracker already
/// built, and a store joined is written through the same stores. So a clone taken before a
/// change sees it, and nothing has to hand out a new set.
///
/// Not here: the front end's own state, the channel its events go back on, and what belongs to
/// one playback or one task, such as the live player's controls and a job. Nor the torrent
/// session and its guard, which the engine side's state holds, to stand them down and back up as
/// the settings change.
pub struct Services {
    /// Where the config, the database and the caches are.
    pub paths: Paths,
    /// The client every request goes through, with the configured proxy and timeouts.
    pub http: HttpClient,
    /// The local database.
    pub store: Store,
    /// The metadata source, as [`build_meta`] builds it.
    pub meta: Meta,
    /// The source chain, in the configured order.
    pub registry: ProviderRegistry,
    /// The trackers, signed in or not.
    pub sync: tracking::Sync,
    /// The plugin stores joined.
    pub stores: plugin_stores::Stores,
    /// The player window kept between episodes, claimed by whichever playback comes next.
    pub slot: crate::playback::slot::PlayerSlot,
    /// Whether the machine is on battery, which the upscaling chain goes by: the system's word
    /// on the desktop, and what the front end reports elsewhere.
    pub power: Arc<dyn PowerSource>,
    /// The players a front end hosts, and where what they report is taken in.
    pub hosted: Arc<HostedPlayers>,
    /// Whether a play opens the front end's hosted player rather than mpv's process: always
    /// without the desktop feature, and on the desktop when the front end asks for it.
    pub hosted_player: bool,
    /// The next episode's streams, found while the current one's ending plays.
    pub prefetch: Prefetch,
    /// The covers downloading, a few at a time.
    pub artwork: crate::artwork::Downloads,
    /// How the player's own interface is dressed: the front end's choice, made once from its
    /// palette and the same for every playback.
    pub player_theme: PlayerTheme,
    /// The sign-in clients a native app ships with, where signing in will read them.
    /// TODO(E6): read by native sign-in; nothing reads them yet.
    pub app_clients: AppClients,
    /// The machine the engine runs on, as the front end offers it: where a link is opened and a
    /// notification shown.
    pub host: Arc<dyn Host>,
    /// The time the new-episode check and the widgets' record go by: the system's, except in a
    /// test that moves it on.
    pub clock: crate::clock::Clock,
    /// The exit network the connection check answers with, in a fixture build or a test: the
    /// guard then makes no lookup. `None` for the app, which looks it up.
    pub fixture_exit: Option<anistream_providers::vpn::Egress>,
    /// Where playbacks find the watch party, when there is one. Opened by the `party` feature's
    /// party, and never without it.
    pub party: crate::playback::seat::PartyHub,
}

impl Services {
    /// The handles as the app builds them at launch, around the source chain it starts with.
    ///
    /// The chain is passed in because the entry points stand it up differently: the interface
    /// without its torrent source, which comes up behind it, and a preview with every source in.
    /// The player's theme and the host are passed in because they are the front end's.
    /// A test assembles `Services` from parts of its own instead, an offline client and a fake
    /// AniList among them, so that nothing it runs can reach the network.
    pub fn build(
        config: &Config,
        paths: Paths,
        http: HttpClient,
        store: Store,
        registry: ProviderRegistry,
        player_theme: PlayerTheme,
        host: Arc<dyn Host>,
    ) -> Result<Self, String> {
        let meta = build_meta(config, &http, &store);
        // Tokens wherever the host keeps them: see `host`. One store for the plugin stores and
        // the trackers alike, so a token is read from the keychain once.
        let secrets = host.secrets().unwrap_or_else(|| crate::host::file_secrets(&paths));
        let tokens = TokenStore::over(secrets);
        let stores = plugin_stores::Stores::with_tokens(config, &paths, tokens.clone())?;
        // Trackers are built whether or not they hold a token: the Accounts overlay needs
        // something to offer a sign-in for, and the outbox accumulates against the id meanwhile.
        let sync = tracking::Sync::with_tokens(config, &store, &http, tokens);
        let (power, hosted) = players(&paths);
        Ok(Self {
            paths,
            http,
            store,
            meta,
            registry,
            sync,
            stores,
            slot: Default::default(),
            power,
            hosted,
            hosted_player: !cfg!(feature = "desktop"),
            prefetch: Prefetch::default(),
            artwork: Default::default(),
            player_theme,
            app_clients: AppClients::default(),
            host,
            clock: Default::default(),
            fixture_exit: None,
            party: Default::default(),
        })
    }
}

/// What a playback learns of the battery from, and the players a front end hosts, which take
/// in what the device reports of its power: the system's word on the desktop, and the device's
/// as its player reports it without the desktop feature, unknown until it does.
pub(crate) fn players(paths: &Paths) -> (Arc<dyn PowerSource>, Arc<HostedPlayers>) {
    let reported = Arc::new(ReportedPower::default());
    #[cfg(feature = "desktop")]
    let power: Arc<dyn PowerSource> = Arc::new(crate::playback::power::SystemPower);
    #[cfg(not(feature = "desktop"))]
    let power: Arc<dyn PowerSource> = reported.clone();
    (power, Arc::new(HostedPlayers::new(paths.runtime_dir(), reported)))
}

/// The metadata source: AniList first, Tenrai when it fails, the cache when both do.
///
/// Built identically at every entry point — the TUI, `--search`, `--play`, previews —
/// so "anistream said X" never depends on which door was used to ask.
pub fn build_meta(config: &Config, http: &HttpClient, store: &Store) -> Meta {
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);
    let tenrai = config.meta.fallback.then(|| Tenrai::new(http.clone(), store.clone()));
    Meta::new(anilist, tenrai, store.clone())
}
