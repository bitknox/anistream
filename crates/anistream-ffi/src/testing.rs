//! The engine started for a test: on a fake AniList, with nothing able to leave the machine.

use std::sync::Arc;

use anistream_core::config::NetworkConfig;
use anistream_engine::{
    options::{Background, EngineOptions, Parts},
    plugin_stores, tracking,
};
use anistream_meta::Meta;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_testkit::FakeAniList;
use anistream_track::secret::{Backend, TokenStore};

use crate::{
    engine::{FfiEngine, Opened, runtime},
    host::FfiHost,
    start::{FfiDirs, FfiError},
};

#[uniffi::export]
impl FfiEngine {
    /// [`FfiEngine::start`], for a test: the engine on the app's directories as the app's, but
    /// with what it reaches beyond them stood in for.
    ///
    /// - Metadata comes from the test kit's fake AniList on loopback, with its fixture titles:
    ///   a search for "hoshikuzu" finds two.
    /// - The only source is the test kit's fixture provider.
    /// - Every request goes through a client that cannot resolve a host name, so one the test
    ///   did not think of fails at once instead of leaving the machine.
    /// - Tokens are files under `dirs.data`, never the keychain.
    /// - None of the engine's own work runs beside the commands: no update check, no sync loops,
    ///   no check of the player.
    /// - A play opens the app's hosted player, in any build: its ops come as
    ///   `Event::PlayerOps`, for the test to answer with `Command::PlayerReport`.
    ///
    /// Only in a build with the `testing` feature, which an app never turns on.
    #[uniffi::constructor]
    pub fn start_testing(dirs: FfiDirs, host: Arc<dyn FfiHost>) -> Result<Arc<Self>, FfiError> {
        Self::start_testing_with(dirs, host, anistream_testkit::sources::registry())
    }
}

impl FfiEngine {
    /// [`FfiEngine::start_testing`] with `sources` in place of the fixture provider alone, for a
    /// Rust test that scripts the streams an episode has. Not in the bindings.
    pub fn start_testing_with(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        sources: ProviderRegistry,
    ) -> Result<Arc<Self>, FfiError> {
        let opened = Opened::open(&dirs)?;
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            // The fake answers on the engine's runtime, for as long as the engine holds it.
            let server = FakeAniList::start().await;
            let options = options(opened, &server, sources)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, server))
        });
        match started {
            Ok((started, server)) => Self::hold(runtime, Ok(started), Some(Box::new(server))),
            Err(e) => Self::hold(runtime, Err(e), None),
        }
    }
}

/// The options [`FfiEngine::start_testing`] starts from: the harness's in the binary, on the
/// app's directories rather than a temporary one.
fn options(
    opened: Opened,
    server: &FakeAniList,
    sources: ProviderRegistry,
) -> Result<EngineOptions, FfiError> {
    let Opened { paths, config, store } = opened;
    let http =
        HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..config.network.clone() })
            .map_err(|e| FfiError::Client { reason: e.to_string() })?;
    let meta = Meta::new(
        server.anilist(&http),
        config.meta.fallback.then(|| server.tenrai(&http, &store)),
        store.clone(),
    );
    let tokens = TokenStore::with_backend(paths.data_dir.join("tokens"), Backend::File);
    let sync = tracking::Sync::with_tokens(&config, &store, &http, tokens.clone());
    let stores = plugin_stores::Stores::with_tokens(&config, &paths, tokens)
        .map_err(|reason| FfiError::Engine { reason })?;

    let mut options = Opened { paths, config, store }.options(http);
    options.sources = Some(sources);
    options.background = Background::none();
    options.hosted_player = true;
    options.parts = Some(Parts { meta, sync, stores });
    Ok(options)
}
