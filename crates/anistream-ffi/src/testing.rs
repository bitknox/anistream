//! The engine started for a test: on a fake AniList, with nothing able to leave the machine.

use std::sync::Arc;

use anistream_core::config::NetworkConfig;
use anistream_engine::{
    clock::Clock,
    options::{Background, EngineOptions, Parts},
    plugin_stores, tracking,
};
use anistream_meta::Meta;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_testkit::FakeAniList;
use anistream_track::{
    AniListTracker,
    secret::{Backend, TokenStore},
};

use crate::{
    engine::{FfiEngine, Opened, RemoteHome, runtime},
    host::{Adapter, FfiHost},
    start::{FfiDirs, FfiError, FfiStart, SecretsMode},
};

#[uniffi::export]
impl FfiEngine {
    /// [`FfiEngine::start`], for a test: the engine on the app's directories as the app's, but
    /// with what it reaches beyond them stood in for.
    ///
    /// - Metadata comes from the test kit's fake AniList on loopback, with its fixture titles:
    ///   a search for "hoshikuzu" finds two. It serves their art too, so covers, banners and
    ///   stills arrive as real images.
    /// - The only source is the test kit's fixture provider.
    /// - Every request goes through a client that cannot resolve a host name, so one the test
    ///   did not think of fails at once instead of leaving the machine.
    /// - Tokens are files under `dirs.data`, never the keychain, whatever `start.secrets` says.
    /// - None of the engine's own work runs beside the commands: no update check, no sync loops,
    ///   no check of the player.
    /// - A play opens the app's hosted player, in any build: its ops come as
    ///   `Event::PlayerOps`, for the test to answer with `Command::PlayerReport`.
    ///
    /// The rest of `start` is taken as [`FfiEngine::start`] takes it, the player's theme and the
    /// sign-in clients. Only in a build with the `testing` feature, which an app never turns on.
    #[uniffi::constructor(default(start))]
    pub fn start_testing(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        start: FfiStart,
    ) -> Result<Arc<Self>, FfiError> {
        Self::start_testing_from(dirs, host, start, None)
    }
}

#[uniffi::export]
impl FfiEngine {
    /// What the fake AniList has been asked so far, an operation's name a request ("Detail",
    /// "Many", "LastAired", …) in arrival order, for counting what an action in the app costs
    /// against AniList's rate limit. Empty on an engine that is not on the fake. Only in a build
    /// with the `testing` feature.
    pub fn testing_anilist_requests(&self) -> Vec<String> {
        self.kept::<Testing>()
            .map(|kept| {
                kept.server.requests().iter().map(|r| format!("{:?}", r.operation)).collect()
            })
            .unwrap_or_default()
    }

    /// Let `secs` pass, for the engine and the fake AniList alike: the broadcasts stay where
    /// they are, so an episode due in an hour has aired once an hour is let pass. What the
    /// new-episode check and the widgets' record are dated by moves on; nothing else in the
    /// engine does. Does nothing on an engine that is not on the fake. Only in a build with the
    /// `testing` feature.
    pub fn testing_advance_clock(&self, secs: i64) {
        if let Some(kept) = self.kept::<Testing>() {
            kept.clock.advance(secs);
            kept.server.advance(secs);
        }
    }
}

/// What a test's engine is kept beside: the fake AniList it asks, and the clock it goes by.
pub(crate) struct Testing {
    pub(crate) server: FakeAniList,
    pub(crate) clock: Clock,
}

impl FfiEngine {
    /// [`FfiEngine::start_testing`] with `sources` in place of the fixture provider alone, for a
    /// Rust test that scripts the streams an episode has. Not in the bindings.
    pub fn start_testing_with(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        sources: ProviderRegistry,
    ) -> Result<Arc<Self>, FfiError> {
        Self::start_testing_from(dirs, host, FfiStart::default(), Some(sources))
    }

    fn start_testing_from(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        start: FfiStart,
        sources: Option<ProviderRegistry>,
    ) -> Result<Arc<Self>, FfiError> {
        let opened = Opened::open(&dirs)?;
        // The test's token files are the trackers' and the stores' own, so the host's keychain
        // is never asked.
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let home = RemoteHome::of(&host, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            // The fake answers on the engine's runtime, for as long as the engine holds it.
            let testing =
                Testing { server: FakeAniList::start().await, clock: Clock::default() };
            let options = options(opened, &testing, &start, sources)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, testing))
        });
        match started {
            Ok((started, testing)) => Self::hold(runtime, Ok(started), Some(Box::new(testing)), home),
            Err(e) => Self::hold(runtime, Err(e), None, home),
        }
    }
}

/// The options [`FfiEngine::start_testing`] starts from: the harness's in the binary, on the
/// app's directories rather than a temporary one, asking `testing`'s fake and going by its clock.
/// `sources` stands in for the fixture provider, whose stills the fake serves, when given.
pub(crate) fn options(
    opened: Opened,
    testing: &Testing,
    start: &FfiStart,
    sources: Option<ProviderRegistry>,
) -> Result<EngineOptions, FfiError> {
    let server = &testing.server;
    let Opened { paths, config, store } = opened;
    // The fixtures' covers, banners and stills, from the fake, for an app that shows them.
    server.serve_art();
    let http =
        HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..config.network.clone() })
            .map_err(|e| FfiError::Client { reason: e.to_string() })?;
    let meta = Meta::new(
        server.anilist(&http),
        config.meta.fallback.then(|| server.tenrai(&http, &store)),
        store.clone(),
    );
    let tokens = TokenStore::with_backend(paths.data_dir.join("tokens"), Backend::File);
    // The trackers as the app's own sign-in clients turn them on, as the engine builds them.
    let tracked = tracking::native::with_app_clients(&config, &start.app_clients());
    let mut sync = tracking::Sync::with_tokens(&tracked, &store, &http, tokens.clone());
    // AniList's on the fake rather than AniList itself, signed in or not: a scene can leave it
    // signed in, so the Library is the fake's, and a sign-in from the app reaches the fake.
    if let Some(at) = sync.trackers.iter().position(|t| t.id() == "anilist") {
        let client = server.anilist(&http).with_token(tokens.get("anilist").ok());
        let tracker = Arc::new(AniListTracker::new(client));
        sync.trackers[at] = tracker.clone();
        sync.anilist = Some(tracker);
    }
    let stores = plugin_stores::Stores::with_tokens(&config, &paths, tokens)
        .map_err(|reason| FfiError::Engine { reason })?;

    let mut options = Opened { paths, config, store }.options(http, start);
    options.sources =
        Some(sources.unwrap_or_else(|| {
            anistream_testkit::sources::registry_with_art(&server.art_base())
        }));
    options.background = Background::none();
    options.hosted_player = true;
    options.parts = Some(Parts { meta, sync, stores, clock: testing.clock.clone() });
    Ok(options)
}
