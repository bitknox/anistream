//! The engine, as a native front end holds it.

use std::{
    any::Any,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    task::{Context, Poll},
};

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    engine::{Engine, EventStream},
    host::{Host, SecretStore},
    options::EngineOptions,
};
use anistream_model::{
    Msg, command::Command, event::Event, jobs::JobId, playback::PlayerTheme,
};
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
    /// Held for the whole of a shutdown, so two at once are one after the other: the second
    /// finds the runtime gone and does nothing.
    stopping: Mutex<()>,
    /// What the engine's parts lean on, kept for as long as the engine: the fake AniList of an
    /// engine started for a test, and the clock it goes by. Read only by the testing build's
    /// accessors.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    kept: Option<Box<dyn Any + Send + Sync>>,
    /// The TV's remote, when one is started, and what it asked of the front end: see
    /// [`crate::remote`].
    #[cfg(feature = "remote")]
    remote: crate::remote::Attached,
}

/// What a TV's remote needs of the engine's start: where the TV's key is kept, and the directory
/// its paired phones are written to.
pub(crate) struct RemoteHome {
    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    pub(crate) secrets: Option<Arc<dyn SecretStore>>,
    #[cfg_attr(not(feature = "remote"), allow(dead_code))]
    pub(crate) data: PathBuf,
}

impl RemoteHome {
    /// The keychain `host` keeps tokens in, and the data directory of `paths`.
    pub(crate) fn of(host: &Adapter, paths: &Paths) -> Self {
        Self { secrets: host.secrets(), data: paths.data_dir.clone() }
    }
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
        let home = RemoteHome::of(&host, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let http = HttpClient::new(&opened.config.network).map_err(|e| {
                FfiError::client(Msg::StartFailedNetwork { reason: e.to_string() })
            })?;
            let mut options = opened.options(http, &start);
            options.background = background.into();
            Self::launch(options, host).await
        });
        Self::hold(runtime, started, None, home)
    }

    /// Hand the engine a command, and say which job it starts, for work someone waits on: the
    /// job an [`Event::JobStarted`] announces and an [`Event::JobDone`] names when the work ends.
    /// `None` for work nobody waits on, such as a write or a player control.
    ///
    /// Only puts the command on a channel, so any thread may call this, the main thread among
    /// them. `None` too once the engine has stopped.
    pub fn send(&self, command: Command) -> Option<JobId> {
        #[cfg(feature = "remote")]
        if let Command::PlayerReport { report, .. } = &command {
            self.remote.report(report);
        }
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
        #[cfg(feature = "remote")]
        let read = self.remote.read(&events);
        #[cfg(not(feature = "remote"))]
        let read = async move { events.lock().await.next_events().await };
        InRuntime { handle: self.handle.clone(), future: Box::pin(read) }.await
    }

    /// Stop as the app quits: the engine winds down, ahead of the commands still queued, and then
    /// its runtime stops, without waiting for work still running on it.
    ///
    /// Blocks while the engine winds down: the playing episode's last position saved, which
    /// waits on the player reporting its end through [`Self::send`], then the torrents, all of
    /// it bounded at a few seconds. The engine takes commands until then, so the player's
    /// reports reach it. Any thread may call it, one of a host's calls among them. A call while
    /// one is under way waits for it, and calls after the first do nothing.
    pub fn shutdown(&self) {
        let _one = lock(&self.stopping);
        let Some(runtime) = lock(&self.runtime).take() else { return };
        let stopped = lock(&self.engine).as_ref().map(Engine::stopping);
        // On a thread of its own, since a thread of the runtime cannot block on it: a host's
        // calls run on those.
        let stopping = std::thread::spawn(move || {
            if let Some(stopped) = stopped {
                runtime.block_on(stopped);
            }
            runtime.shutdown_background();
        });
        let _ = stopping.join();
        // Only now: until the save was done, the player's reports went through it.
        drop(lock(&self.engine).take());
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
            .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))
    }

    /// The engine `started` on `runtime`, held for the app, with `kept` beside it; or, when it
    /// did not start, the runtime stopped and why.
    /// What [`Self::hold`] kept beside the engine, as a `T`: the fake AniList and the clock of a
    /// test's engine.
    #[cfg(feature = "testing")]
    pub(crate) fn kept<T: 'static>(&self) -> Option<&T> {
        self.kept.as_ref()?.downcast_ref()
    }

    pub(crate) fn hold(
        runtime: Runtime,
        started: Result<(Engine, EventStream), FfiError>,
        kept: Option<Box<dyn Any + Send + Sync>>,
        #[cfg_attr(not(feature = "remote"), allow(unused_variables))] home: RemoteHome,
    ) -> Result<Arc<Self>, FfiError> {
        match started {
            Ok((engine, events)) => Ok(Arc::new(Self {
                #[cfg(feature = "remote")]
                remote: crate::remote::Attached::new(home, engine.casts()),
                engine: Mutex::new(Some(engine)),
                events: Arc::new(tokio::sync::Mutex::new(events)),
                handle: runtime.handle().clone(),
                runtime: Mutex::new(Some(runtime)),
                stopping: Mutex::new(()),
                kept,
            })),
            Err(e) => {
                runtime.shutdown_background();
                Err(e)
            }
        }
    }
}

#[cfg(any(feature = "remote", all(feature = "party", feature = "testing")))]
impl FfiEngine {
    /// The runtime the engine runs on, for a server started beside it.
    pub(crate) fn runtime_handle(&self) -> Handle {
        self.handle.clone()
    }
}

#[cfg(feature = "remote")]
impl FfiEngine {
    /// What the remote sends the engine's commands through, until the engine stops.
    pub(crate) fn remote_sender(&self) -> Option<anistream_engine::engine::RemoteSender> {
        lock(&self.engine).as_ref().map(Engine::remote_sender)
    }

    pub(crate) fn remote(&self) -> &crate::remote::Attached {
        &self.remote
    }

    /// Run `work` on the engine's runtime, and wait for it from whichever thread polls.
    pub(crate) async fn on_runtime<T: Send + 'static>(
        &self,
        work: impl Future<Output = T> + Send + 'static,
    ) -> Option<T> {
        self.handle.spawn(work).await.ok()
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
    /// Where a damaged database was moved, when one was.
    pub(crate) set_aside: Option<std::path::PathBuf>,
}

impl Opened {
    /// Create the directories, read the config and open the database, a damaged one moved
    /// aside for a new one.
    pub(crate) fn open(dirs: &FfiDirs) -> Result<Self, FfiError> {
        for dir in [&dirs.config, &dirs.data, &dirs.cache] {
            std::fs::create_dir_all(dir).map_err(|e| {
                FfiError::dirs(Msg::StartFailedFolder {
                    path: dir.clone(),
                    reason: e.to_string(),
                })
            })?;
        }
        let paths = Paths {
            config_file: Path::new(&dirs.config).join("config.toml"),
            data_dir: dirs.data.clone().into(),
            cache_dir: dirs.cache.clone().into(),
        };
        let config = Config::read(&paths).map_err(|problem| {
            let path = paths.config_file.display().to_string();
            FfiError::config(Msg::StartFailedConfig { path, problem: problem.to_string() })
        })?;
        let database = paths.database();
        let (store, set_aside) = Store::open_or_set_aside(&database).map_err(|e| {
            let path = database.display().to_string();
            FfiError::store(Msg::StartFailedDatabase { path, reason: e.to_string() })
        })?;
        Ok(Self { paths, config, store, set_aside })
    }

    /// The options for the app's launch as `start` describes it, with `http` for every request.
    pub(crate) fn options(self, http: HttpClient, start: &FfiStart) -> EngineOptions {
        let Self { paths, config, store, set_aside } = self;
        let theme = start.theme.clone().unwrap_or_else(player_theme);
        let mut options = EngineOptions::new(paths, config.clone(), http, store, theme);
        options.set_aside = set_aside;
        // What a store joined with an invite calls this device: the kind of device it is.
        if let Some(platform) = start.platform {
            platform.device_label().clone_into(&mut options.device_label);
        }
        // Nothing changes the config for one session here, as the terminal's flags can, so the
        // file's is the one in effect.
        options.saved = Some(config);
        options.app_clients = start.app_clients();
        options
    }
}

/// Settings back to their defaults, for an app whose start failed on a `config.toml` nobody on
/// the device can edit: the file is moved aside to `config.damaged-<date>` beside it and kept,
/// and the next start reads the defaults, VPN and proxy included. Nothing else changes: the
/// database, and the watch history in it, stay. Answers where the old file is kept, or nothing
/// when there was no file. The app calls it only when the viewer asks, and starts again after.
#[uniffi::export]
pub fn reset_config(dirs: FfiDirs) -> Result<Option<String>, FfiError> {
    let paths = Paths {
        config_file: Path::new(&dirs.config).join("config.toml"),
        data_dir: dirs.data.into(),
        cache_dir: dirs.cache.into(),
    };
    let aside = paths.set_config_aside().map_err(|e| {
        let path = paths.config_file.display().to_string();
        FfiError::config(Msg::ConfigResetFailed { path, reason: e.to_string() })
    })?;
    if let Some(aside) = &aside {
        tracing::warn!(aside = %aside.display(), "settings reset; the old config kept");
    }
    Ok(aside.map(|aside| aside.display().to_string()))
}

/// The runtime an engine runs on: one per engine, with a worker per core.
pub(crate) fn runtime() -> Result<Runtime, FfiError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("anistream-engine")
        .build()
        .map_err(|e| FfiError::runtime(Msg::StartFailed { reason: e.to_string() }))
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

    fn dirs_in(root: &std::path::Path) -> FfiDirs {
        let dir = |name: &str| root.join(name).display().to_string();
        FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") }
    }

    #[test]
    fn a_setting_this_version_does_not_know_is_no_reason_to_refuse_the_start() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("config")).unwrap();
        let file = root.path().join("config/config.toml");
        std::fs::write(&file, "[playback]\nquality = 720\nsparkle = 1\n").unwrap();
        let files = Opened::open(&dirs_in(root.path())).expect("it opens");
        assert_eq!(files.config.playback.quality, 720);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "[playback]\nquality = 720\nsparkle = 1\n"
        );
    }

    /// Reset settings on the start-failed screen: the file nobody could edit is kept aside, the
    /// next start reads the defaults, and the database is left as it was.
    #[test]
    fn a_config_reset_keeps_the_file_aside_and_the_next_start_opens() {
        let root = tempfile::tempdir().unwrap();
        let dirs = dirs_in(root.path());
        drop(Opened::open(&dirs).expect("a first start makes the database"));
        let database = root.path().join("data/anistream.db");
        assert!(database.exists());
        std::fs::write(root.path().join("config/config.toml"), "[network\nproxy = ").unwrap();
        let Err(FfiError::Config { .. }) = Opened::open(&dirs) else {
            panic!("the file is refused");
        };

        let kept = reset_config(dirs.clone()).unwrap().expect("the file kept");
        assert!(kept.contains("config.damaged-20"), "{kept}");
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), "[network\nproxy = ");
        let files = Opened::open(&dirs).expect("it opens on the defaults");
        assert_eq!(files.config, Config::default());
        assert_eq!(files.set_aside, None, "the database is the one before");
        drop(files);
        assert!(database.exists());

        assert_eq!(reset_config(dirs).unwrap(), None, "no file, nothing to keep");
    }

    #[test]
    fn a_damaged_database_is_set_aside_and_said_at_launch() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("data")).unwrap();
        std::fs::write(root.path().join("data/anistream.db"), "a page of something else")
            .unwrap();
        let files = Opened::open(&dirs_in(root.path())).expect("it opens on a new database");
        let aside = files.set_aside.clone().expect("set aside");
        assert!(aside.exists() && aside.starts_with(root.path().join("data")), "{aside:?}");
        let options = files.options(offline(), &FfiStart::default());
        assert_eq!(options.set_aside, Some(aside));
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
            AppClients {
                anilist: Some("4242".into()),
                mal: Some("a-mal-client".into()),
                ..AppClients::default()
            }
        );
        assert_eq!(options.config.trackers.anilist.client_id, None);
        assert_eq!(options.config.trackers.mal.client_id, None);
    }
}
