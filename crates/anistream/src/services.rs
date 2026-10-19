//! The handles every effect shares, and how they are built.

use anistream_core::config::{Config, Paths};
use anistream_engine::{plugin_stores, slot::PlayerSlot, tracking};
use anistream_meta::{Meta, Tenrai, anilist::AniList};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anyhow::Result;

use crate::Prefetch;

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
/// session and its guard, which the loop stands down and back up as the settings change.
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
    pub slot: PlayerSlot,
    /// The next episode's streams, found while the current one's ending plays.
    pub prefetch: Prefetch,
}

impl Services {
    /// The handles as the app builds them at launch, around the source chain it starts with.
    ///
    /// The chain is passed in because the entry points stand it up differently: the interface
    /// without its torrent source, which comes up behind it, and a preview with every source in.
    /// A test assembles `Services` from parts of its own instead, an offline client and a fake
    /// AniList among them, so that nothing it runs can reach the network.
    pub fn build(
        config: &Config,
        paths: Paths,
        http: HttpClient,
        store: Store,
        registry: ProviderRegistry,
    ) -> Result<Self> {
        let meta = build_meta(config, &http, &store);
        let stores =
            plugin_stores::Stores::new(config, &paths).map_err(|e| anyhow::anyhow!(e))?;
        // Trackers are built whether or not they hold a token: the Accounts overlay needs
        // something to offer a sign-in for, and the outbox accumulates against the id meanwhile.
        let sync = tracking::Sync::build(config, &store, &http);
        Ok(Self {
            paths,
            http,
            store,
            meta,
            registry,
            sync,
            stores,
            slot: PlayerSlot::default(),
            prefetch: Prefetch::default(),
        })
    }
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
