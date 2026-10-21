//! The engine started for a test: on a fake AniList, with nothing able to leave the machine.

use std::sync::Arc;

use anistream_core::config::NetworkConfig;
use anistream_engine::{
    options::{Background, EngineOptions, Parts},
    plugin_stores, tracking,
};
use anistream_meta::Meta;
use anistream_net::HttpClient;
use anistream_testkit::FakeAniList;
use anistream_track::secret::{Backend, TokenStore};

use crate::{
    engine::{FfiEngine, Opened, runtime},
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
    ///
    /// The rest of `start` is taken as [`FfiEngine::start`] takes it, the player's theme and the
    /// sign-in clients. Only in a build with the `testing` feature, which an app never turns on.
    #[uniffi::constructor(default(start))]
    pub fn start_testing(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        start: FfiStart,
    ) -> Result<Arc<Self>, FfiError> {
        let opened = Opened::open(&dirs)?;
        // The test's token files are the trackers' and the stores' own, so the host's keychain
        // is never asked.
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            // The fake answers on the engine's runtime, for as long as the engine holds it.
            let server = FakeAniList::start().await;
            let options = options(opened, &server, &start)?;
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
pub(crate) fn options(
    opened: Opened,
    server: &FakeAniList,
    start: &FfiStart,
) -> Result<EngineOptions, FfiError> {
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
    let sync = tracking::Sync::with_tokens(&config, &store, &http, tokens.clone());
    let stores = plugin_stores::Stores::with_tokens(&config, &paths, tokens)
        .map_err(|reason| FfiError::Engine { reason })?;

    let mut options = Opened { paths, config, store }.options(http, start);
    options.sources = Some(anistream_testkit::sources::registry_with_art(&server.art_base()));
    options.background = Background::none();
    options.parts = Some(Parts { meta, sync, stores });
    Ok(options)
}
