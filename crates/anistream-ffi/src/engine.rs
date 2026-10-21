//! The engine, as a native front end holds it.

use std::{
    any::Any,
    future::Future,
    path::Path,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll},
};

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    engine::{Engine, EventStream},
    host::Host,
    options::EngineOptions,
};
use anistream_model::{command::Command, event::Event, jobs::JobId, playback::PlayerTheme};
use anistream_net::HttpClient;
use anistream_store::Store;
use tokio::runtime::{Handle, Runtime};

use crate::{
    host::{Adapter, FfiHost},
    start::{FfiBackground, FfiDirs, FfiError, FfiStart},
};

/// The engine, started from a native app: where its commands go and where its events are read.
///
/// It runs on a multi-thread tokio runtime of its own, which [`Self::shutdown`] stops; dropping
/// the last reference stops it too, without waiting.
#[derive(uniffi::Object)]
pub struct FfiEngine {
    /// Where commands go, until [`Self::shutdown`] takes it.
    engine: Mutex<Option<Engine>>,
    /// What happened, read by one [`Self::next_events`] at a time.
    events: Arc<tokio::sync::Mutex<EventStream>>,
    /// The runtime's handle, which every poll of [`Self::next_events`] enters.
    handle: Handle,
    /// The runtime the engine runs on, until [`Self::shutdown`] stops it.
    runtime: Mutex<Option<Runtime>>,
    /// What the engine's parts lean on, kept for as long as the engine: the fake AniList of an
    /// engine started for a test.
    _kept: Option<Box<dyn Any + Send + Sync>>,
}

#[uniffi::export]
impl FfiEngine {
    /// Stand the engine up on the app's directories, as the terminal app launches it: the
    /// sources the config names, and as much of the work the engine does on its own beside the
    /// commands as `background` says. Tokens are kept where `start.secrets` says, the host's
    /// keychain unless the app says otherwise, and the player dressed as `start.theme` says.
    ///
    /// Reads `config.toml` from `dirs.config`, or starts on the defaults when there is none, as
    /// on a first launch; the engine writes the file once a setting is saved. Opens the
    /// database in `dirs.data`. Blocks for as long as that takes, which is short: what is slow
    /// to come up, such as the plugins, comes up behind it.
    #[uniffi::constructor(default(start))]
    pub fn start(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        background: FfiBackground,
        start: FfiStart,
    ) -> Result<Arc<Self>, FfiError> {
        let opened = Opened::open(&dirs)?;
        let host = Adapter::new(host, start.secrets_mode(), &opened.config, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let http = HttpClient::new(&opened.config.network)
                .map_err(|e| FfiError::Client { reason: e.to_string() })?;
            let mut options = opened.options(http, &start);
            options.background = background.into();
            Self::launch(options, host).await
        });
        Self::hold(runtime, started, None)
    }

    /// Hand the engine a command, and say which job it starts, for work someone waits on: the
    /// job an [`Event::JobStarted`] announces and an [`Event::JobDone`] names when the work ends.
    /// `None` for work nobody waits on, such as a write or a player control.
    ///
    /// Only puts the command on a channel, so any thread may call this, the main thread among
    /// them. `None` too once the engine has stopped.
    pub fn send(&self, command: Command) -> Option<JobId> {
        lock(&self.engine).as_ref()?.send(command)
    }

    /// The events there are, waiting for at least one: a batch for the front end to apply in
    /// one go, in the order the engine applied them. Launch's come first: the settings, the
    /// trackers and the stores as they were left.
    ///
    /// Empty once the engine has stopped, which ends a loop over it. Cancel-safe: a task that
    /// stops waiting loses nothing, and the next call has what it would have had.
    pub async fn next_events(&self) -> Vec<Event> {
        let events = Arc::clone(&self.events);
        let read = async move { events.lock().await.next_events().await };
        InRuntime { handle: self.handle.clone(), future: Box::pin(read) }.await
    }

    /// Stop, after everything sent before: the engine winds down as the app quits, and then
    /// its runtime stops, without waiting for work still running on it.
    ///
    /// Blocks while the engine winds down, which with torrents on takes up to a second. Any
    /// thread may call it, one of a host's calls among them. Calls after the first do nothing.
    pub fn shutdown(&self) {
        let engine = lock(&self.engine).take();
        let Some(runtime) = lock(&self.runtime).take() else { return };
        // On a thread of its own, since a thread of the runtime cannot block on it: a host's
        // calls run on those.
        let stopping = std::thread::spawn(move || {
            if let Some(engine) = engine {
                runtime.block_on(engine.shutdown());
            }
            runtime.shutdown_background();
        });
        let _ = stopping.join();
    }
}

impl FfiEngine {
    /// Start the engine from `options` on the app's `host`, on the runtime this is polled in.
    pub(crate) async fn launch(
        options: EngineOptions,
        host: Adapter,
    ) -> Result<(Engine, EventStream), FfiError> {
        let host: Arc<dyn Host> = Arc::new(host);
        Engine::start(options, host)
            .await
            .map_err(|e| FfiError::Engine { reason: e.to_string() })
    }

    /// The engine `started` on `runtime`, held for the app, with `kept` beside it; or, when it
    /// did not start, the runtime stopped and why.
    pub(crate) fn hold(
        runtime: Runtime,
        started: Result<(Engine, EventStream), FfiError>,
        kept: Option<Box<dyn Any + Send + Sync>>,
    ) -> Result<Arc<Self>, FfiError> {
        match started {
            Ok((engine, events)) => Ok(Arc::new(Self {
                engine: Mutex::new(Some(engine)),
                events: Arc::new(tokio::sync::Mutex::new(events)),
                handle: runtime.handle().clone(),
                runtime: Mutex::new(Some(runtime)),
                _kept: kept,
            })),
            Err(e) => {
                runtime.shutdown_background();
                Err(e)
            }
        }
    }
}

impl Drop for FfiEngine {
    /// Stop the runtime without waiting, when the app let go without [`Self::shutdown`]. A
    /// runtime dropped in place would wait for the work still running on it, and panics on a
    /// thread of another runtime.
    fn drop(&mut self) {
        let runtime = self.runtime.get_mut().unwrap_or_else(PoisonError::into_inner).take();
        if let Some(runtime) = runtime {
            runtime.shutdown_background();
        }
    }
}

/// The files a start opens before the engine stands up.
pub(crate) struct Opened {
    pub(crate) paths: Paths,
    pub(crate) config: Config,
    pub(crate) store: Store,
}

impl Opened {
    /// Create the directories, read the config and open the database.
    pub(crate) fn open(dirs: &FfiDirs) -> Result<Self, FfiError> {
        for dir in [&dirs.config, &dirs.data, &dirs.cache] {
            std::fs::create_dir_all(dir).map_err(|e| FfiError::Dirs {
                reason: format!("{dir} cannot be created: {e}"),
            })?;
        }
        let paths = Paths {
            config_file: Path::new(&dirs.config).join("config.toml"),
            data_dir: dirs.data.clone().into(),
            cache_dir: dirs.cache.clone().into(),
        };
        let config = Config::read(&paths).map_err(|problem| FfiError::Config {
            reason: format!("{}, {problem}", paths.config_file.display()),
        })?;
        let store = Store::open(paths.database())
            .map_err(|e| FfiError::Store { reason: format!("the database: {e}") })?;
        Ok(Self { paths, config, store })
    }

    /// The options for the app's launch as `start` describes it, with `http` for every request.
    pub(crate) fn options(self, http: HttpClient, start: &FfiStart) -> EngineOptions {
        let Self { paths, config, store } = self;
        // `start.platform` is said for what comes later: nothing differs by it yet.
        let theme = start.theme.clone().unwrap_or_else(player_theme);
        let mut options = EngineOptions::new(paths, config.clone(), http, store, theme);
        // Nothing changes the config for one session here, as the terminal's flags can, so the
        // file's is the one in effect.
        options.saved = Some(config);
        options.app_clients = start.app_clients();
        options
    }
}

/// The runtime an engine runs on: one per engine, with a worker per core.
pub(crate) fn runtime() -> Result<Runtime, FfiError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("anistream-engine")
        .build()
        .map_err(|e| FfiError::Runtime { reason: e.to_string() })
}

/// How mpv's own interface is dressed when the app hands in no theme of its own: the terminal's
/// immersive palette, so the player looks the same whichever front end starts it.
///
/// Copied from the terminal front end's palette, which this crate may not depend on.
fn player_theme() -> PlayerTheme {
    PlayerTheme {
        text: "#E4E0D6".into(),
        dim: "#9298B5".into(),
        obi: "#F2A64B".into(),
        state: "#7FC7D9".into(),
        alert: "#E06B54".into(),
        ground: "#161A2E".into(),
        font: "Zen Kaku Gothic New".into(),
    }
}

/// The engine's state behind a lock that a panic elsewhere does not poison for good: each
/// critical section is a single take or read, which leaves nothing half-done.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A future polled inside the engine's runtime, whichever thread polls it.
///
/// The bindings poll an async call on the foreign side's own threads, which tokio knows nothing
/// of. Reading the event stream needs no more than its wakers today, but with the runtime
/// entered for each poll, a timer or a spawn added to it later works rather than panicking for
/// want of a runtime.
struct InRuntime<F> {
    handle: Handle,
    future: Pin<Box<F>>,
}

impl<F: Future> Future for InRuntime<F> {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<F::Output> {
        let this = &mut *self;
        let _entered = this.handle.enter();
        this.future.as_mut().poll(cx)
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::config::NetworkConfig;
    use anistream_engine::options::AppClients;

    use super::*;

    /// What a start opens, in a new temporary directory kept beside it.
    fn opened() -> (Opened, tempfile::TempDir) {
        let root = tempfile::tempdir().unwrap();
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        (Opened::open(&dirs).expect("it opens"), root)
    }

    fn offline() -> HttpClient {
        HttpClient::offline(&NetworkConfig::default()).unwrap()
    }

    #[test]
    fn the_player_wears_the_apps_theme_and_the_terminals_without_one() {
        let (files, _root) = opened();
        let options = files.options(offline(), &FfiStart::default());
        assert_eq!(options.player_theme, player_theme());
        assert_eq!(options.app_clients, AppClients::default());

        let theme = PlayerTheme { ground: "#0B0D17".into(), ..player_theme() };
        let start = FfiStart {
            theme: Some(theme.clone()),
            anilist_client_id: Some("4242".into()),
            mal_client_id: Some("a-mal-client".into()),
            ..FfiStart::default()
        };
        let (files, _root) = opened();
        let options = files.options(offline(), &start);
        assert_eq!(options.player_theme, theme);
        // Kept for signing in, and never put in the config, which the terminal shares.
        assert_eq!(
            options.app_clients,
            AppClients { anilist: Some("4242".into()), mal: Some("a-mal-client".into()) }
        );
        assert_eq!(options.config.trackers.anilist.client_id, None);
        assert_eq!(options.config.trackers.mal.client_id, None);
    }
}
