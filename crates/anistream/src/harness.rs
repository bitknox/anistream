//! The app with its engine, driven as the event loop drives it, for the contract tests.
//!
//! The engine is the layer that turns a [`Command`] into work and the work's answers into
//! [`Event`]s. Nothing tested it: the reducer's tests stop at the task it returns, and the
//! end-to-end tests start mpv. The contract tests pin what each task sends back and in what order,
//! so that moving all of it into an engine could be shown to change nothing.
//!
//! So the tests are written against this, and this alone: [`Harness::send`] a task, then
//! [`Harness::settle`] for every update it led to, in the order the app applied them. The app's
//! commands go through [`Engine::send`] and its updates come back on the engine's
//! [`EventStream`], as the event loop sends and reads them, and nothing of the engine's insides
//! reaches a test. They moved behind `Engine::send` with the insides of this file changing and
//! the tests not.
//!
//! What it stands up, all of it in a temporary directory or in memory:
//!
//! - the fake AniList from the test kit, with its empty Tenrai beside it when the config keeps
//!   the fallback on, as the default does;
//! - an in-memory store, the test kit's fixture sources, and plugin stores whose tokens are
//!   files under the temporary directory;
//! - trackers built from the config, holding tokens in that directory too, never the OS
//!   keychain — and a signed-in AniList pointed at the fake;
//! - one [`HttpClient::offline`] for everything, so a request for any host by name fails at
//!   once rather than leaving the machine. The plugin store client is the exception, being its
//!   own `reqwest` client: it is only ever given loopback addresses here;
//! - a [`Host`] that opens no page and shows no notification, and keeps what it was asked;
//!   the tokens above are its store's.
//!
//! The engine is started from those parts, with none of the work it does on its own at launch.
//! What it does not stand up: a torrent session, a VPN guard, mpv or a browser. A config that
//! asks for torrents is refused, and so are the tasks that would start a player or open a page.

use std::{
    collections::{HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use anistream_core::config::{Config, NetworkConfig, Paths};
use anistream_engine::{
    cache::TitleCache,
    engine::{Engine, EventStream, Traced},
    host::Host,
    options::{Background, EngineOptions, Parts},
    outbox::Internal,
    plugin_stores, sources, tracking,
};
use anistream_meta::Meta;
use anistream_model::{command::Command, event::Event, jobs::JobId, notice::Notice};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_testkit::{FakeAniList, fixtures::FIXTURE_NOW};
use anistream_track::{
    AniListTracker,
    secret::{Backend, LocalSecrets, SecretStore, TokenStore},
};
use anistream_tui::{
    Clock, Zone,
    app::App,
    image::ImageEngine,
    keymap::{Action, Keymap},
    screens, theme,
};

use crate::{covers, tui::run::show_config_problem};

/// How long [`Harness::settle`] waits for work to finish before failing the test.
///
/// Every request goes to loopback with a five-second timeout, so a wait this long is a task
/// that never ends — a bug the failure names.
const PATIENCE: Duration = Duration::from_secs(30);

/// A config that runs mpv with no window and no sound, for a test that plays: the fixture
/// source has a four-second test pattern for every episode, which mpv draws itself.
pub const HEADLESS_MPV: &str = "[playback]\nmpv_args = [\"--vo=null\", \"--ao=null\"]\n";

/// Whether mpv is here to play with, for a test that plays. CI's Linux job installs it and sets
/// `ANISTREAM_REQUIRE_MPV`, so there a missing mpv fails the test instead of skipping it.
pub async fn mpv_here() -> bool {
    let dir = tempfile::tempdir().expect("a temporary directory for mpv");
    let here = anistream_player::Mpv::new(dir.path()).is_available().await;
    if !here {
        assert!(
            std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
            "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
        );
        eprintln!("skipping: mpv is not on PATH");
    }
    here
}

/// The zone the app's clock reads dates in: nine hours east, far enough from UTC that a date
/// taken in the wrong zone shows.
pub fn zone() -> Zone {
    Zone::hours_east(9).expect("nine hours is a zone")
}

/// How the app is set up before it starts.
pub struct Options {
    /// `config.toml` as it is on disk at launch, or `None` for no file and so the defaults.
    /// A file that does not load starts the app on the defaults, with the problem shown, as the
    /// app itself does.
    pub config: Option<String>,
    /// Trackers signed in at launch, each with a made-up token. AniList's client talks to the
    /// fake, so its library and its pushes stay on loopback; any other tracker keeps its real
    /// endpoint and finds the network closed.
    pub signed_in: Vec<&'static str>,
    /// The sources, or `None` for the test kit's fixture provider alone.
    pub registry: Option<ProviderRegistry>,
    /// The terminal: how many rows a list shows, and the size [`Harness::screen`] draws at.
    pub size: (u16, u16),
    /// The moment the app's clock is stopped at, which the fake answers as of too.
    pub now: i64,
    /// The title language the signed-in AniList viewer reads in, as AniList names it, in place
    /// of the fixture viewer's English: set before launch, which asks it.
    pub viewer_title_language: Option<&'static str>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            config: None,
            signed_in: Vec::new(),
            registry: None,
            size: (120, 34),
            now: FIXTURE_NOW,
            viewer_title_language: None,
        }
    }
}

/// The app, its engine, and a fake AniList, in a temporary directory.
pub struct Harness {
    app: App,
    server: FakeAniList,
    tokens: TokenStore,
    size: (u16, u16),
    // The engine and what it says, held as the event loop holds them.
    engine: Engine,
    events: EventStream,
    jobs: Jobs,
    /// How many commands the engine had routed when it last said it had nothing left to do.
    idle: u64,
    /// The last settle, by name: see [`Harness::trace`].
    trace: Vec<&'static str>,
    // Handles onto what the engine holds, for a test to seed and to read: each shares its state
    // with the engine's copy.
    store: Store,
    paths: Paths,
    registry: ProviderRegistry,
    /// Last, so it is removed after everything that might still be writing into it.
    _root: tempfile::TempDir,
}

impl Harness {
    /// The app on the default config, signed in nowhere, with the fixture sources.
    pub async fn new() -> Self {
        Self::with(Options::default()).await
    }

    /// The app set up as `options` say, in the state it is in a moment after launch: stores,
    /// sources and trackers on their rows, and nothing loaded yet.
    pub async fn with(options: Options) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory for the app");
        let paths = Paths {
            config_file: root.path().join("config").join("config.toml"),
            data_dir: root.path().join("data"),
            cache_dir: root.path().join("cache"),
        };
        if let Some(text) = &options.config {
            std::fs::create_dir_all(root.path().join("config")).expect("the config directory");
            std::fs::write(&paths.config_file, text).expect("writing config.toml");
        }
        // As `main` reads it: a file that does not load means the defaults, said on screen.
        let (config, problem) = match Config::read(&paths) {
            Ok(config) => (config, None),
            Err(problem) => (Config::default(), Some(problem)),
        };
        assert!(
            !sources::torrent_wanted(&config),
            "the harness has no torrent session or VPN guard to bring up"
        );

        let http =
            HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..config.network.clone() })
                .expect("the offline client builds");
        let store = anistream_testkit::store();
        let server = FakeAniList::start().await;
        server.set_now(options.now);
        if let Some(language) = options.viewer_title_language {
            server.set_viewer_title_language(language);
        }
        // `build_meta`, pointed at the fake.
        let meta = Meta::new(
            server.anilist(&http),
            config.meta.fallback.then(|| server.tenrai(&http, &store)),
            store.clone(),
        );

        // The host's store, which the engine's own parts are built over below: files in the
        // temporary directory.
        let host = Arc::new(TestHost::in_dir(&root.path().join("tokens")));
        let tokens = TokenStore::over(host.secrets.clone());
        for tracker in &options.signed_in {
            tokens.set(tracker, &format!("harness-{tracker}-token")).expect("a token file");
        }
        let mut sync = tracking::Sync::with_tokens(&config, &store, &http, tokens.clone());
        // Built as `with_tokens` builds it, but asking the fake rather than AniList itself.
        if options.signed_in.contains(&"anilist")
            && let Some(at) = sync.trackers.iter().position(|t| t.id() == "anilist")
        {
            let client = server.anilist(&http).with_token(tokens.get("anilist").ok());
            let tracker = Arc::new(AniListTracker::new(client));
            sync.trackers[at] = tracker.clone();
            sync.anilist = Some(tracker);
        }
        let stores = plugin_stores::Stores::with_tokens(&config, &paths, tokens.clone())
            .expect("the plugin stores open");
        let registry = options.registry.unwrap_or_else(anistream_testkit::sources::registry);

        // `run`'s start, short of the terminal.
        let palette = theme::resolve_with(config.theme.mode, None);
        let mut keymap = Keymap::new();
        let problems = keymap.apply_overrides(&config.keys);
        let saved = Config::load(&paths).unwrap_or_default();
        let settings = anistream_model::settings::view(&config, &saved);
        let mut app = App::with_images(settings, palette, keymap, ImageEngine::disabled());
        app.clock = Clock::fixed(options.now, zone());
        for problem in problems {
            app.push_toast(Notice::alert(format!("keybinding not applied: {problem}")));
        }
        if let Some(problem) = &problem {
            show_config_problem(&mut app, &paths, problem);
        }

        // Started from the parts above rather than its own: it would point the metadata source
        // at AniList itself, and keep tokens wherever the user's are. With the chain whole and
        // nothing of its own to do, it does no more than it is sent.
        let mut start = EngineOptions::new(
            paths.clone(),
            config,
            http,
            store.clone(),
            theme::player_theme(),
        );
        start.saved = Some(saved);
        start.sources = Some(registry.clone());
        start.background = Background::none();
        start.parts = Some(Parts { meta, sync, stores });
        let (engine, mut events) = Engine::start(start, host).await.expect("the engine starts");
        // What launch says, applied as the loop applies it before its first frame: the stores,
        // the chain and the trackers on their rows.
        for update in events.launch().await {
            app.apply(update);
        }

        Self {
            app,
            server,
            tokens,
            size: options.size,
            engine,
            events,
            jobs: Jobs::default(),
            idle: 0,
            trace: Vec::new(),
            store,
            paths,
            registry,
            _root: root,
        }
    }

    /// Run a task as the event loop runs one: sent to the engine, which starts its job and
    /// routes it. Returns the job, as the engine numbered it, for work someone waits on.
    ///
    /// Nothing is applied until [`Self::settle`]. Tasks sent one after another without a
    /// settle between them are routed one after another, as keys pressed in quick succession
    /// are, and whatever one of them writes inline is there for the next.
    ///
    /// # Panics
    ///
    /// On a task that would start mpv or open a browser: these tests have neither.
    pub fn send(&mut self, task: Command) -> Option<JobId> {
        assert!(
            !matches!(
                task,
                Command::Play { .. }
                    | Command::PlaySource { .. }
                    | Command::PlayParty { .. }
                    | Command::PlayLocal { .. }
                    | Command::PlayTrailer { .. }
                    | Command::OpenExternal { .. }
                    | Command::OpenAdmin { .. }
                    | Command::Connect { .. }
            ),
            "the harness does not run {task:?}: it would start mpv or open a browser"
        );
        self.dispatch(task)
    }

    /// Press a key, as the event loop does: the reducer's answer, sent.
    pub fn press(&mut self, action: Action) -> Option<JobId> {
        let task = self.app.handle(action, self.rows())?;
        self.send(task)
    }

    /// Press a key that plays, as the event loop does: the reducer stages Now Playing and asks
    /// for the play, which is sent.
    ///
    /// Only where nothing would start a player in front of the test: a source with no stream
    /// for the episode, so the lookup fails before any player starts, or a config that runs mpv
    /// with no window and no sound.
    ///
    /// # Panics
    ///
    /// When the key does not play.
    pub fn press_to_play(&mut self, action: Action) -> Option<JobId> {
        let task = self.app.handle(action, self.rows());
        let Some(task @ (Command::Play { .. } | Command::PlaySource { .. })) = task else {
            panic!("{action:?} played nothing: {task:?}")
        };
        self.dispatch(task)
    }

    /// Whether the engine has a playback under way, as quitting now would find it.
    pub async fn under_way(&self) -> bool {
        self.engine.under_way().await
    }

    /// Quit, as the event loop does once it has left: the engine shut down and waited for.
    /// Returns how long the shutdown took.
    pub async fn quit(self) -> Duration {
        let started = tokio::time::Instant::now();
        self.engine.shutdown().await;
        started.elapsed()
    }

    /// Type into whatever field has the keyboard, a character at a time.
    pub fn type_text(&mut self, text: &str) {
        text.chars().for_each(|c| self.app.type_char(c));
    }

    /// The load the view on screen asks for, as launch and a change of section ask for it.
    pub fn reload(&mut self) -> Option<JobId> {
        let task = self.app.reload()?;
        self.send(task)
    }

    /// Apply what the work sent back until there is none left, and return it in the order
    /// the app applied it.
    ///
    /// All but `JobStarted`, which every job opens with: the jobs are held to it here instead.
    /// Each job a command started has to be announced, in the order they were started and by
    /// the number its send returned, before anything else is said about it, and ended once.
    ///
    /// The engine's reactions are in it, each where the engine has them: a metadata notice as
    /// the status rows and a toast just before the update it arrived with, a queued progress
    /// drained at once, and what the engine side left itself to do done in its place among
    /// them — the chain put in a new order and its rows sent, or the sources rebuilt and
    /// `SourcesChanged` sent ahead of what the rebuild reports. And the loop's: work the
    /// reducer queued sent before the next update is read, and covers fetched as the loop
    /// fetches them, which with images off is never.
    ///
    /// Done means done, not quiet for a while. The engine says when it falls idle: no command
    /// queued, nothing unread, and no work holding a way to answer. Once it has said so with
    /// every command sent routed, everything they led to is in, and there is nothing left to
    /// wait for. A job still open then is a job that never ended, and fails the test by name.
    /// The one piece of work that holds no way to answer is a prefetch, which reports nothing,
    /// and is not waited for.
    pub async fn settle(&mut self) -> Vec<Event> {
        self.trace.clear();
        let deadline = tokio::time::Instant::now() + PATIENCE;
        let mut seen = Vec::new();
        loop {
            // The reducer's follow-ups, as at the top of the loop.
            for task in self.app.take_commands() {
                self.dispatch(task);
            }
            for url in self.app.visible_artwork(self.rows()) {
                self.dispatch(Command::FetchArtwork { url });
            }
            if self.idle == self.engine.sent() {
                break;
            }
            let wait = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(wait, self.events.next_traced()).await {
                Ok(Some(Traced::Event(update))) => self.apply(update, &mut seen),
                Ok(Some(Traced::Handled(message))) => self.handled(message),
                Ok(Some(Traced::Idle { routed })) => self.idle = routed,
                Ok(None) => panic!("the engine stopped, having seen {:?}", names(&seen)),
                Err(_) => panic!(
                    "still waiting after {PATIENCE:?} on {:?}, having seen {:?}",
                    self.job_labels(),
                    names(&seen)
                ),
            }
        }
        assert!(
            self.app.jobs.is_empty(),
            "the work ended and these jobs did not: {:?}, having seen {:?}",
            self.job_labels(),
            names(&seen)
        );
        assert!(
            self.jobs.started.is_empty(),
            "jobs started and never announced: {:?}, having seen {:?}",
            self.jobs.started,
            names(&seen)
        );
        seen
    }

    /// Apply what the work sends until `done` says one is the last, without waiting for the
    /// rest: for a test that acts while work is still under way, as a person can. Returns what
    /// was applied, in order, all but `JobStarted` as [`Self::settle`] does; the jobs still
    /// running are left to run.
    pub async fn settle_until(&mut self, done: impl Fn(&Event) -> bool) -> Vec<Event> {
        let deadline = tokio::time::Instant::now() + PATIENCE;
        let mut seen = Vec::new();
        loop {
            for task in self.app.take_commands() {
                self.dispatch(task);
            }
            if seen.last().is_some_and(&done) {
                return seen;
            }
            let wait = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(wait, self.events.next_traced()).await {
                Ok(Some(Traced::Event(update))) => self.apply(update, &mut seen),
                Ok(Some(Traced::Handled(message))) => self.handled(message),
                Ok(Some(Traced::Idle { routed })) => self.idle = routed,
                Ok(None) => panic!("the engine stopped, having seen {:?}", names(&seen)),
                Err(_) => {
                    panic!("still waiting after {PATIENCE:?}, having seen {:?}", names(&seen))
                }
            }
        }
    }

    /// The engine's record of the titles, as it stands: what a play would take from it, for a
    /// test to ask, since what a play takes goes to the player, which these tests have none of.
    pub async fn title_cache(&self) -> TitleCache {
        self.engine.title_cache().await
    }

    /// Hand the engine what a playback says, as its player would: these tests have none. The
    /// engine does its part with it in its turn and passes it on, for the next settle to apply.
    pub fn player_says(&mut self, event: Event) {
        self.engine.inject(event);
    }

    /// [`Self::player_says`], as the playback of `episode` of `id` would say it: named with
    /// that playback, which `playback` numbers, from a million up. Its first word starts it.
    pub fn playback_says(
        &mut self,
        playback: u64,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        event: Event,
    ) {
        self.engine.inject_from(playback, id, episode, event);
    }

    /// [`Self::settle`], with everything the engine says before it next falls idle applied in
    /// one batch, before the loop's own work: as a front end applies what
    /// `EventStream::next_events` hands it, rather than one event a pass as the terminal's loop
    /// takes them. Then settled as usual, for whatever the batch led to; the trace is that
    /// settle's.
    pub async fn settle_in_one_batch(&mut self) -> Vec<Event> {
        let deadline = tokio::time::Instant::now() + PATIENCE;
        let mut batch = Vec::new();
        while self.idle != self.engine.sent() {
            let wait = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(wait, self.events.next_traced()).await {
                Ok(Some(Traced::Event(update))) => batch.push(update),
                Ok(Some(Traced::Handled(message))) => self.handled(message),
                Ok(Some(Traced::Idle { routed })) => self.idle = routed,
                Ok(None) => panic!("the engine stopped, having seen {:?}", names(&batch)),
                Err(_) => {
                    panic!("still waiting after {PATIENCE:?}, having seen {:?}", names(&batch))
                }
            }
        }
        let mut seen = Vec::new();
        for update in batch {
            self.apply(update, &mut seen);
        }
        seen.extend(self.settle().await);
        seen
    }

    /// What the last [`Self::settle`] went through, by name and in order: every event it
    /// returned, and between them each message the engine side left itself and kept, where
    /// the engine handles it. So `SettingsSaved` and `SourcesReordered` are in it, though no
    /// front end ever sees them. `SourcesChanged` is passed on once handled, and is in it once,
    /// as the event. `JobStarted` is not, as in what the settle returns.
    pub fn trace(&self) -> &[&'static str] {
        &self.trace
    }

    /// The app, as the work has left it.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The app, for a test that sets the stage the way a person would have by now.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// The fake AniList: what it was asked, and how it should fail.
    pub fn server(&self) -> &FakeAniList {
        &self.server
    }

    /// The local database, for seeding and for checking what a task wrote.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Where the app's files are: all of them under one temporary directory.
    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    /// The live source chain.
    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    /// Where tracker and plugin store tokens are kept: files in the temporary directory.
    pub fn tokens(&self) -> &TokenStore {
        &self.tokens
    }

    /// The frame as text: one line per row, trailing spaces trimmed, as `--preview` prints it.
    /// Drawn as the loop draws one, so what it reports goes back to the app.
    pub fn screen(&mut self) -> String {
        let (width, height) = self.size;
        let buffer = screens::render_to_buffer(&mut self.app, width, height);
        (0..height)
            .map(|y| {
                let line: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                line.trim_end().to_owned()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rows(&self) -> usize {
        let (width, height) = self.size;
        screens::visible_rows(
            ratatui::layout::Rect::new(0, 0, width, height),
            self.app.nav.section(),
        )
    }

    fn job_labels(&self) -> Vec<String> {
        self.app.jobs.iter().map(|job| job.label.clone()).collect()
    }

    /// A command sent as the loop sends one, its job numbered as it is sent.
    fn dispatch(&mut self, task: Command) -> Option<JobId> {
        let job = self.engine.send(task);
        self.jobs.started.extend(job);
        self.jobs.numbered.extend(job);
        job
    }

    /// A message the engine side left itself, handled: in the trace after what it said, where
    /// the App applied it when it rode the event channel. Unless it was passed on, and is there
    /// as the event already.
    fn handled(&mut self, message: Internal) {
        // What `sources::torrent_wanted` asks of the config, of the sources the App was told.
        let configured = &self.app.settings.sources;
        let torrent_wanted = configured.torrent
            && configured.order.iter().any(|id| id == "torrent")
            && !configured.disabled.iter().any(|id| id == "torrent");
        assert!(
            message != Internal::SourcesChanged || !torrent_wanted,
            "the harness has no torrent session or VPN guard to bring up"
        );
        match message {
            Internal::SettingsSaved => self.trace.push("SettingsSaved"),
            Internal::SourcesReordered => self.trace.push("SourcesReordered"),
            Internal::SourcesChanged => {}
        }
    }

    fn apply(&mut self, update: Event, seen: &mut Vec<Event>) {
        self.jobs.check(&update, seen);
        if !matches!(update, Event::JobStarted { .. }) {
            self.trace.push(name(&update));
            seen.push(copy(&update));
        }
        match update {
            // Decoded at once rather than on the blocking pool: settling has no frame to keep.
            Event::ArtworkReady { url, bytes } => {
                covers::hand_over(&mut self.app, covers::now(url, &bytes));
            }
            update => self.app.apply(update),
        }
    }
}

/// The machine the harness runs on: it opens nothing and notifies nobody, and keeps what it was
/// asked to do, in order. Tokens are files in the temporary directory.
///
/// Every page it is asked to open opens, as far as the engine can tell, so the work carries on
/// as it would once the browser is up.
struct TestHost {
    asked: Mutex<Vec<String>>,
    secrets: Arc<dyn SecretStore>,
}

impl TestHost {
    /// Tokens in files under `dir`, never the keychain.
    fn in_dir(dir: &std::path::Path) -> Self {
        Self {
            asked: Mutex::default(),
            secrets: Arc::new(LocalSecrets::new(dir, Backend::File)),
        }
    }
}

impl Host for TestHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.asked.lock().expect("the host's record").push(format!("open {url}"));
        Ok(())
    }

    fn notify(&self, title: &str, body: &str) {
        self.asked.lock().expect("the host's record").push(format!("notify {title}: {body}"));
    }

    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        Some(self.secrets.clone())
    }
}

/// The jobs the harness's commands started, and the engine on its own, held to what the work
/// says of them.
#[derive(Default)]
struct Jobs {
    /// Numbered by a command, and not yet announced: in the order the commands were sent.
    started: VecDeque<JobId>,
    /// Every number a command's send returned.
    numbered: HashSet<JobId>,
    /// Announced, and not yet ended.
    running: HashSet<JobId>,
    /// Every job announced so far.
    announced: HashSet<JobId>,
}

impl Jobs {
    fn check(&mut self, update: &Event, seen: &[Event]) {
        match update {
            // A job of a command is announced in the order the commands were sent, by the
            // number its send returned. One the engine started on its own — the next episode an
            // ending rolls on to, an episode picked in the player — has a number no send
            // returned, from the same count, and comes in its own place among them.
            Event::JobStarted { job, .. } if !self.numbered.contains(job) => {
                assert!(
                    self.announced.insert(*job),
                    "{job:?} announced twice, having seen {:?}",
                    names(seen)
                );
                self.running.insert(*job);
            }
            Event::JobStarted { job, .. } => {
                let expected = self.started.pop_front();
                assert_eq!(
                    expected,
                    Some(*job),
                    "a job announced out of turn, or by another number than its send returned, \
                     having seen {:?}",
                    names(seen)
                );
                self.announced.insert(*job);
                self.running.insert(*job);
            }
            Event::JobNote { job, .. } => assert!(
                self.running.contains(job),
                "a note for {job:?}, which is not running, having seen {:?}",
                names(seen)
            ),
            Event::JobDone(job) => assert!(
                self.running.remove(job),
                "{job:?} ended without being announced, or twice, having seen {:?}",
                names(seen)
            ),
            _ => {}
        }
    }
}

/// The variant's name, the part of an update that outlives any change to what it carries.
///
/// Exhaustive on purpose: a new variant does not compile until it has a name here, which is
/// the moment to decide what the contract says about it.
pub fn name(update: &Event) -> &'static str {
    match update {
        Event::JobStarted { .. } => "JobStarted",
        Event::JobNote { .. } => "JobNote",
        Event::JobDone(_) => "JobDone",
        Event::Content { .. } => "Content",
        Event::Caption { .. } => "Caption",
        Event::CalendarWindow { .. } => "CalendarWindow",
        Event::CalendarBackfill { .. } => "CalendarBackfill",
        Event::Detail(_) => "Detail",
        Event::ViewerTitleLanguage(_) => "ViewerTitleLanguage",
        Event::TitlesChanged(_) => "TitlesChanged",
        Event::Status(_) => "Status",
        Event::Toast(_) => "Toast",
        Event::Providers(_) => "Providers",
        Event::MetaStatus(_) => "MetaStatus",
        Event::Settings(_) => "Settings",
        Event::ShowSettings(_) => "ShowSettings",
        Event::SourcesChanged => "SourcesChanged",
        Event::ProviderNote(_) => "ProviderNote",
        Event::ProviderPreference(_) => "ProviderPreference",
        Event::MatchChoices { .. } => "MatchChoices",
        Event::Sources { .. } => "Sources",
        Event::PlaybackVolume(_) => "PlaybackVolume",
        Event::ActiveProvider(_) => "ActiveProvider",
        Event::PlayerPlayEpisode { .. } => "PlayerPlayEpisode",
        Event::PlaybackKept => "PlaybackKept",
        Event::EpisodeLeft { .. } => "EpisodeLeft",
        Event::PlayerTranslation(_) => "PlayerTranslation",
        Event::PlayerUpscaling(_) => "PlayerUpscaling",
        Event::PlayerNightMode(_) => "PlayerNightMode",
        Event::PlayerToneMapping(_) => "PlayerToneMapping",
        Event::PrefetchNext { .. } => "PrefetchNext",
        Event::Playback { .. } => "Playback",
        Event::SkipAvailable { .. } => "SkipAvailable",
        Event::SkipCleared => "SkipCleared",
        Event::PlaybackSpeed(_) => "PlaybackSpeed",
        Event::PlaybackEnded { .. } => "PlaybackEnded",
        Event::PlaybackStaged { .. } => "PlaybackStaged",
        Event::Resumed { .. } => "Resumed",
        Event::PlayerOps { .. } => "PlayerOps",
        Event::PlayerUi { .. } => "PlayerUi",
        Event::Sync(_) => "Sync",
        Event::ProgressQueued => "ProgressQueued",
        Event::Conflicts(_) => "Conflicts",
        Event::Episodes { .. } => "Episodes",
        Event::Progress { .. } => "Progress",
        Event::Vpn(_) => "Vpn",
        Event::ArtworkReady { .. } => "ArtworkReady",
        Event::ImageFailed { .. } => "ImageFailed",
        Event::Downloads(_) => "Downloads",
        Event::Seeding(_) => "Seeding",
        Event::DeviceCode(_) => "DeviceCode",
        Event::LastAired(_) => "LastAired",
        Event::Stores(_) => "Stores",
        Event::StorePreview(_) => "StorePreview",
        Event::StoreJoined { .. } => "StoreJoined",
        Event::InviteLink(_) => "InviteLink",
        Event::StoreMembers { .. } => "StoreMembers",
        Event::LocalPlugins { .. } => "LocalPlugins",
    }
}

/// Every update's [`name`], in order.
pub fn names(updates: &[Event]) -> Vec<&'static str> {
    updates.iter().map(name).collect()
}

/// An update, twice: one for the app to apply and one for the test to read.
///
/// By hand because `Event` is not `Clone`, though everything it carries is. Exhaustive like
/// [`name`], so the two cannot drift apart.
fn copy(update: &Event) -> Event {
    match update {
        Event::JobStarted { job, activity } => {
            Event::JobStarted { job: *job, activity: activity.clone() }
        }
        Event::JobNote { job, note, wait } => {
            Event::JobNote { job: *job, note: note.clone(), wait: *wait }
        }
        Event::JobDone(job) => Event::JobDone(*job),
        Event::Content { view, content } => {
            Event::Content { view: view.clone(), content: content.clone() }
        }
        Event::Caption { view, text } => {
            Event::Caption { view: view.clone(), text: text.clone() }
        }
        Event::CalendarWindow { anchor, pages, more_past } => {
            Event::CalendarWindow { anchor: *anchor, pages: *pages, more_past: *more_past }
        }
        Event::CalendarBackfill { page, entries, more_past } => Event::CalendarBackfill {
            page: *page,
            entries: entries.clone(),
            more_past: *more_past,
        },
        Event::Detail(entry) => Event::Detail(entry.clone()),
        Event::ViewerTitleLanguage(language) => Event::ViewerTitleLanguage(*language),
        Event::TitlesChanged(language) => Event::TitlesChanged(*language),
        Event::Status(text) => Event::Status(text.clone()),
        Event::Toast(toast) => Event::Toast(toast.clone()),
        Event::Providers(rows) => Event::Providers(rows.clone()),
        Event::MetaStatus(rows) => Event::MetaStatus(rows.clone()),
        Event::Settings(view) => Event::Settings(view.clone()),
        Event::ShowSettings(view) => Event::ShowSettings(view.clone()),
        Event::SourcesChanged => Event::SourcesChanged,
        Event::ProviderNote(text) => Event::ProviderNote(text.clone()),
        Event::ProviderPreference(provider) => Event::ProviderPreference(provider.clone()),
        Event::MatchChoices { id, provider_id, candidates } => Event::MatchChoices {
            id: *id,
            provider_id: provider_id.clone(),
            candidates: candidates.clone(),
        },
        Event::Sources { id, episode, candidates } => {
            Event::Sources { id: *id, episode: episode.clone(), candidates: candidates.clone() }
        }
        Event::PlaybackVolume(volume) => Event::PlaybackVolume(*volume),
        Event::ActiveProvider(provider) => Event::ActiveProvider(provider.clone()),
        Event::PlayerPlayEpisode { episode, title } => {
            Event::PlayerPlayEpisode { episode: episode.clone(), title: title.clone() }
        }
        Event::PlaybackKept => Event::PlaybackKept,
        Event::EpisodeLeft { id, episode, position, duration, watched } => Event::EpisodeLeft {
            id: *id,
            episode: episode.clone(),
            position: *position,
            duration: *duration,
            watched: *watched,
        },
        Event::PlayerTranslation(translation) => Event::PlayerTranslation(*translation),
        Event::PlayerUpscaling(mode) => Event::PlayerUpscaling(mode.clone()),
        Event::PlayerNightMode(on) => Event::PlayerNightMode(*on),
        Event::PlayerToneMapping(curve) => Event::PlayerToneMapping(curve.clone()),
        Event::PrefetchNext { id, episode } => {
            Event::PrefetchNext { id: *id, episode: episode.clone() }
        }
        Event::Playback { position, duration, paused } => {
            Event::Playback { position: *position, duration: *duration, paused: *paused }
        }
        Event::SkipAvailable { kind, to } => Event::SkipAvailable { kind: *kind, to: *to },
        Event::SkipCleared => Event::SkipCleared,
        Event::PlaybackSpeed(speed) => Event::PlaybackSpeed(*speed),
        Event::PlaybackEnded { watched, ending, rolling_on } => Event::PlaybackEnded {
            watched: *watched,
            ending: ending.clone(),
            rolling_on: *rolling_on,
        },
        Event::PlaybackStaged { id, title, episode, episode_title, speed } => {
            Event::PlaybackStaged {
                id: *id,
                title: title.clone(),
                episode: episode.clone(),
                episode_title: episode_title.clone(),
                speed: *speed,
            }
        }
        Event::Resumed { position } => Event::Resumed { position: *position },
        Event::PlayerOps { playback, ops } => {
            Event::PlayerOps { playback: *playback, ops: ops.clone() }
        }
        Event::PlayerUi { playback, ui } => {
            Event::PlayerUi { playback: *playback, ui: ui.clone() }
        }
        Event::Sync(state) => Event::Sync(state.clone()),
        Event::ProgressQueued => Event::ProgressQueued,
        Event::Conflicts(rows) => Event::Conflicts(rows.clone()),
        Event::Episodes { id, rows } => Event::Episodes { id: *id, rows: rows.clone() },
        Event::Progress { id, progress, resume, episodes } => Event::Progress {
            id: *id,
            progress: *progress,
            resume: *resume,
            episodes: episodes.clone(),
        },
        Event::Vpn(guard) => Event::Vpn(guard.clone()),
        Event::ArtworkReady { url, bytes } => {
            Event::ArtworkReady { url: url.clone(), bytes: bytes.clone() }
        }
        Event::ImageFailed { url, offline } => {
            Event::ImageFailed { url: url.clone(), offline: *offline }
        }
        Event::Downloads(rows) => Event::Downloads(rows.clone()),
        Event::Seeding(rows) => Event::Seeding(rows.clone()),
        Event::DeviceCode(prompt) => Event::DeviceCode(prompt.clone()),
        Event::LastAired(rows) => Event::LastAired(rows.clone()),
        Event::Stores(rows) => Event::Stores(rows.clone()),
        Event::StorePreview(preview) => Event::StorePreview(preview.clone()),
        Event::StoreJoined { store } => Event::StoreJoined { store: store.clone() },
        Event::InviteLink(view) => Event::InviteLink(view.clone()),
        Event::StoreMembers { store, members, invites } => Event::StoreMembers {
            store: store.clone(),
            members: members.clone(),
            invites: invites.clone(),
        },
        Event::LocalPlugins { store, plugins } => {
            Event::LocalPlugins { store: store.clone(), plugins: plugins.clone() }
        }
    }
}
