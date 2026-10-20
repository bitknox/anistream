//! The app with its effects, driven as the event loop drives them, for the contract tests.
//!
//! [`dispatch`] and the loop's reactions to what it sends back are the layer
//! that turns a [`Command`] into work and the work's answers into [`Event`]s. Nothing tested it:
//! the reducer's tests stop at the task it returns, and the end-to-end tests start mpv. The
//! contract tests pin what each task sends back and in what order, so that moving all of it
//! into an engine can be shown to change nothing.
//!
//! So the tests are written against this, and this alone: [`Harness::send`] a task, then
//! [`Harness::settle`] for every update it led to, in the order the app applied them. Nothing
//! about `dispatch`'s parameters reaches a test. When the effects move behind
//! `Engine::send`, the insides of this file change and the tests do not.
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
//!   own `reqwest` client: it is only ever given loopback addresses here.
//!
//! What it does not stand up: a torrent session, a VPN guard, mpv or a browser. A config that
//! asks for torrents is refused, and so are the tasks that would start a player or open a page.

use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
    time::Duration,
};

use anistream_core::config::{Config, NetworkConfig, Paths};
use anistream_engine::{
    data,
    outbox::{Envelope, Internal, Outbox, WeakOutbox},
    playback::{prefetch::Prefetch, slot::PlayerSlot},
    plugin_stores,
    services::Services,
    sources, tracking,
};
use anistream_meta::Meta;
use anistream_model::{command::Command, event::Event, jobs::JobId, notice::Notice};
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_testkit::{FakeAniList, fixtures::FIXTURE_NOW};
use anistream_track::{
    AniListTracker,
    secret::{Backend, TokenStore},
};
use anistream_tui::{
    Clock, Zone,
    app::App,
    image::ImageEngine,
    keymap::{Action, Keymap},
    screens, theme,
};
use tokio::sync::mpsc;

use crate::{
    ConfigSeam, command, covers, dispatch, engine_state::EngineState, handle_internal, receive,
    show_config_problem,
};

/// How long [`Harness::settle`] waits for work to finish before failing the test.
///
/// Every request goes to loopback with a five-second timeout, so a wait this long is a task
/// that never ends — a bug the failure names.
const PATIENCE: Duration = Duration::from_secs(30);

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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            config: None,
            signed_in: Vec::new(),
            registry: None,
            size: (120, 34),
            now: FIXTURE_NOW,
        }
    }
}

/// The app, its effects, and a fake AniList, in a temporary directory.
pub struct Harness {
    app: App,
    server: FakeAniList,
    tokens: TokenStore,
    size: (u16, u16),
    channels: Channels,
    // The handles the event loop holds, held the same way.
    services: Services,
    engine: EngineState,
    seam: ConfigSeam,
    jobs: Jobs,
    /// The last settle, by name: see [`Harness::trace`].
    trace: Vec<&'static str>,
    _arrived: mpsc::UnboundedReceiver<(u64, Result<sources::TorrentUp, String>)>,
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
        // `build_meta`, pointed at the fake.
        let meta = Meta::new(
            server.anilist(&http),
            config.meta.fallback.then(|| server.tenrai(&http, &store)),
            store.clone(),
        );

        let tokens = TokenStore::with_backend(root.path().join("tokens"), Backend::File);
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
        let mut app =
            App::with_images(config.clone(), palette, keymap, ImageEngine::disabled());
        app.clock = Clock::fixed(options.now, zone());
        for problem in problems {
            app.push_toast(Notice::alert(format!("keybinding not applied: {problem}")));
        }
        app.saved_config = Config::load(&paths).unwrap_or_default();
        if let Some(problem) = &problem {
            show_config_problem(&mut app, &paths, problem);
        }
        match stores.rows() {
            Ok(rows) => app.apply(Event::Stores(rows)),
            Err(e) => app.apply(Event::Toast(Notice::alert(e))),
        }
        // As `--preview` has them: the rows the plugin load's reorder puts up a moment in.
        app.apply(Event::Providers(data::provider_rows(&registry)));
        for state in sync.initial_states() {
            app.apply(Event::Sync(Box::new(state)));
        }

        let (arrivals, arrived) = mpsc::unbounded_channel();
        Self {
            engine: EngineState::new(config, arrivals),
            seam: ConfigSeam::default(),
            jobs: Jobs::default(),
            trace: Vec::new(),
            app,
            server,
            tokens,
            size: options.size,
            channels: Channels::default(),
            // Assembled from the parts above rather than built: `Services::build` would point
            // the metadata source at AniList itself, and keep tokens wherever the user's are.
            services: Services {
                paths,
                http,
                store,
                meta,
                registry,
                sync,
                stores,
                slot: PlayerSlot::default(),
                prefetch: Prefetch::default(),
                player_theme: theme::player_theme(),
            },
            _arrived: arrived,
            _root: root,
        }
    }

    /// Run a task as the event loop runs one: its job started, then dispatched. Returns the
    /// job, as the engine side numbered it, for work someone waits on.
    ///
    /// Nothing is applied until [`Self::settle`]. Tasks sent one after another without a
    /// settle between them are dispatched one after another, as keys pressed in quick
    /// succession are, and whatever one of them writes inline is there for the next.
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
        self.channels.hold();
        self.dispatch(task)
    }

    /// Press a key, as the event loop does: the reducer's answer, sent.
    pub fn press(&mut self, action: Action) -> Option<JobId> {
        let task = self.app.handle(action, self.rows())?;
        self.send(task)
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
    /// The event loop's reactions included, each where the loop has it: a metadata notice as
    /// the status rows and a toast just before the update it arrived with, a queued progress
    /// drained at once, and work the reducer queued dispatched before the next update is read.
    /// What the engine side left itself to do is done in its place among them, as the loop does
    /// it: the chain put in a new order and its rows sent, or the sources rebuilt and
    /// `SourcesChanged` sent ahead of what the rebuild reports. Covers are fetched
    /// as the loop fetches them, which with images off is never.
    ///
    /// Done means done, not quiet for a while: every piece of work dispatched holds a sender,
    /// so the updates end exactly when the last of the work does. A job still open then is a
    /// job that never ended, and fails the test by name. The one piece of work that holds no
    /// sender is a prefetch, which reports nothing, and is not waited for.
    pub async fn settle(&mut self) -> Vec<Event> {
        self.channels.held = None;
        self.trace.clear();
        let deadline = tokio::time::Instant::now() + PATIENCE;
        let mut seen = Vec::new();
        loop {
            // Auto-next and the reducer's other follow-ups, as at the top of the loop.
            if let Some(task) = self.app.take_pending() {
                self.dispatch(task);
            }
            for url in self.app.visible_artwork(self.rows()) {
                self.dispatch(Command::FetchArtwork { url });
            }
            // And whatever moved the config since, before the reactions read it.
            self.use_config();
            let Some(updates) = self.channels.open.front_mut() else { break };
            let wait = deadline.saturating_duration_since(tokio::time::Instant::now());
            match tokio::time::timeout(wait, updates.recv()).await {
                Ok(Some(Envelope::Event(update))) => self.react(update, &mut seen).await,
                Ok(Some(Envelope::Internal(message))) => self.handle(message, &mut seen).await,
                Ok(None) => {
                    self.channels.open.pop_front();
                }
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

    /// What the last [`Self::settle`] went through, by name and in order: every event it
    /// returned, and between them each message the engine side left itself and kept, where
    /// the loop handles it. So `SettingsSaved` and `SourcesReordered` are in it, though no
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
        &self.services.store
    }

    /// Where the app's files are: all of them under one temporary directory.
    pub fn paths(&self) -> &Paths {
        &self.services.paths
    }

    /// The live source chain.
    pub fn registry(&self) -> &ProviderRegistry {
        &self.services.registry
    }

    /// Where tracker and plugin store tokens are kept: files in the temporary directory.
    pub fn tokens(&self) -> &TokenStore {
        &self.tokens
    }

    /// The frame as text: one line per row, trailing spaces trimmed, as `--preview` prints it.
    pub fn screen(&self) -> String {
        let (width, height) = self.size;
        let buffer = screens::render_to_buffer(&self.app, width, height);
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

    /// A command sent as the loop sends one: the config ahead of it when it moved, then the
    /// command, its job started.
    fn dispatch(&mut self, task: Command) -> Option<JobId> {
        let tx = self.channels.sender();
        let job =
            command(&self.app, &mut self.seam, task, &mut self.engine, &self.services, &tx);
        self.jobs.started.extend(job);
        job
    }

    /// `use_config`, as the loop calls it before its reactions. A channel is opened only to say
    /// something: one opened on every pass would keep the settle from ever ending.
    fn use_config(&mut self) {
        if let Some(config) = self.seam.moved(&self.app) {
            let tx = self.channels.sender();
            dispatch(config, None, &mut self.engine, &self.services, &tx);
        }
    }

    /// One update, as the loop's `rx.recv()` arm handles it.
    async fn react(&mut self, update: Event, seen: &mut Vec<Event>) {
        // First, the loop hands an invite link to the terminal's clipboard. There is no
        // terminal here, so nothing marks the link copied.

        // The engine side's part: a queued progress drained now, and the metadata ladder's
        // notice ahead of the update.
        let channels = &mut self.channels;
        for said in receive(&update, &self.services, || channels.sender()) {
            self.apply(said, seen);
        }
        // The loop asks for the terminal's focus back after an ending; there is no terminal.
        self.apply(update, seen);
    }

    /// What the engine side left itself to do, as the loop handles it: what that says applied
    /// at once, and the message itself in the trace after it, where the App
    /// applied it when it rode the event channel. Unless it was passed on, and is there as
    /// the event already.
    async fn handle(&mut self, message: Internal, seen: &mut Vec<Event>) {
        assert!(
            message != Internal::SourcesChanged
                || !sources::torrent_wanted(&self.engine.config),
            "the harness has no torrent session or VPN guard to bring up"
        );
        let tx = self.channels.sender();
        for said in handle_internal(message, &mut self.engine, &self.services, &tx).await {
            self.apply(said, seen);
        }
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

/// The jobs the harness's commands started, held to what the work says of them.
#[derive(Default)]
struct Jobs {
    /// Numbered by a command, and not yet announced: in the order the commands were sent.
    started: VecDeque<JobId>,
    /// Announced, and not yet ended.
    running: HashSet<JobId>,
}

impl Jobs {
    fn check(&mut self, update: &Event, seen: &[Event]) {
        match update {
            Event::JobStarted { job, .. } => {
                let expected = self.started.pop_front();
                assert_eq!(
                    expected,
                    Some(*job),
                    "a job announced out of turn, or by another number than its send returned, \
                     having seen {:?}",
                    names(seen)
                );
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

/// The channels the work answers on.
///
/// One at a time is open, and work is handed a sender to it; the harness keeps only a weak
/// one. So a channel closes exactly when the last work holding a sender ends, and an empty,
/// closed channel is the end of the work — no waiting for a quiet moment. Work dispatched
/// after the open one has closed gets a new one, read after it: everything on the old one was
/// sent before the new one existed.
#[derive(Default)]
struct Channels {
    open: VecDeque<mpsc::UnboundedReceiver<Envelope>>,
    newest: Option<WeakOutbox>,
    /// Holds the newest channel open between a [`Harness::send`] and the settle after it,
    /// when nothing else may yet be holding it.
    held: Option<Outbox>,
}

impl Channels {
    /// A sender to the open channel, or to a new one when it has closed.
    fn sender(&mut self) -> Outbox {
        if let Some(tx) = self.newest.as_ref().and_then(WeakOutbox::upgrade) {
            return tx;
        }
        let (tx, rx) = Outbox::inbox();
        self.newest = Some(tx.downgrade());
        self.open.push_back(rx);
        tx
    }

    fn hold(&mut self) {
        if self.held.is_none() {
            self.held = Some(self.sender());
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
        Event::Status(_) => "Status",
        Event::Toast(_) => "Toast",
        Event::Providers(_) => "Providers",
        Event::MetaStatus(_) => "MetaStatus",
        Event::SettingWritten { .. } => "SettingWritten",
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
        Event::Resumed { .. } => "Resumed",
        Event::Sync(_) => "Sync",
        Event::ProgressQueued => "ProgressQueued",
        Event::Conflicts(_) => "Conflicts",
        Event::Episodes { .. } => "Episodes",
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
        Event::Status(text) => Event::Status(text.clone()),
        Event::Toast(toast) => Event::Toast(toast.clone()),
        Event::Providers(rows) => Event::Providers(rows.clone()),
        Event::MetaStatus(rows) => Event::MetaStatus(rows.clone()),
        Event::SettingWritten { on_disk, failed } => {
            Event::SettingWritten { on_disk: on_disk.clone(), failed: *failed }
        }
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
        Event::PlaybackEnded { watched, ending } => {
            Event::PlaybackEnded { watched: *watched, ending: ending.clone() }
        }
        Event::Resumed { position } => Event::Resumed { position: *position },
        Event::Sync(state) => Event::Sync(state.clone()),
        Event::ProgressQueued => Event::ProgressQueued,
        Event::Conflicts(rows) => Event::Conflicts(rows.clone()),
        Event::Episodes { id, rows } => Event::Episodes { id: *id, rows: rows.clone() },
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
