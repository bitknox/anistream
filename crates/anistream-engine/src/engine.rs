//! The engine as a front end holds it: started, sent commands, read for what happened, and shut
//! down, with one task behind it that owns everything else.
//!
//! [`Engine::start`] stands the engine up and spawns its actor: one task that owns the engine
//! side's state and every handle the work shares, and reads, in one place, the commands
//! [`Engine::send`] hands it, the inbox the work answers on, and the torrent source reporting in.
//! The front end reads what happened from the [`EventStream`], in the order the actor applied
//! it. Nothing else of the engine is the front end's to call, and nothing of the front end's
//! reaches the engine except as a command.
//!
//! Shaped for a front end on the other side of an FFI. Events are pulled, in batches, rather
//! than pushed through a callback; the one callback interface is the [`Host`]; and [`send`]
//! only puts the command on a channel, so a UI thread that is not async can call it.
//!
//! [`send`]: Engine::send

use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    jobs::JobId,
    notice::{Notice, NoticeHint},
};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use crate::{
    background::{datasets::keep_datasets_current, digest, prune},
    data,
    dispatch::{dispatch, handle_internal, receive},
    error::EngineError,
    host::Host,
    jobs::JobGuard,
    options::EngineOptions,
    outbox::{Envelope, Internal, Outbox, Played, WeakOutbox},
    playback::{
        autonext,
        player::{launch_notice, player},
        prefetch::prefetch,
        start::play_following,
    },
    plugin_stores,
    services::Services,
    settings,
    sources::{self, TorrentUp},
    state::EngineState,
    tracking, updates,
};

/// How long the exit waits for a playback to record where it was left.
const EXIT_FLUSH: Duration = Duration::from_secs(2);

/// How long the exit gives the torrent session to stand down. librqbit's stop ends in a fixed
/// one-second wait after the work that matters, so this is what a quit with torrents costs.
const TORRENT_CLOSE: Duration = Duration::from_secs(1);

/// The engine, as a front end holds it: where its commands go.
///
/// Dropping it, or [`Self::shutdown`], ends the actor. What the actor had started runs on until
/// the runtime it runs on ends, as it always has when the app quits.
pub struct Engine {
    commands: mpsc::UnboundedSender<Message>,
    /// Jobs numbered so far, here as well as in the actor so [`Self::send`] can say at once
    /// which number the actor will announce. The actor numbers the jobs it starts on its own
    /// from the same count: see [`Jobs`].
    jobs: Jobs,
    /// Commands sent so far, which an idle engine's word is read against: see `Traced::Idle`.
    #[cfg(feature = "testing")]
    sent: AtomicU64,
}

/// What the front end hands the actor.
enum Message {
    /// A command, with the job [`Engine::send`] numbered for it, and its activity.
    Command { job: Option<(JobId, String)>, command: Command },
    /// The app is quitting: see [`Engine::shutdown`].
    Shutdown { playing: bool, done: oneshot::Sender<()> },
    /// An event, as the work would send it: see [`Engine::inject`].
    #[cfg(feature = "testing")]
    Inject(Event),
    /// An event, as a playback would send it: see [`Engine::inject_from`].
    #[cfg(feature = "testing")]
    InjectFrom(Played, Event),
    /// The engine's record of the titles, as it stands: see [`Engine::title_cache`].
    #[cfg(feature = "testing")]
    TitleCache(oneshot::Sender<crate::cache::TitleCache>),
    /// Whether a playback is under way, as the engine sees it: see [`Engine::under_way`].
    #[cfg(feature = "testing")]
    UnderWay(oneshot::Sender<bool>),
}

/// The count jobs are numbered by, from one: shared by [`Engine::send`], which numbers the
/// jobs of the commands sent, and the actor, which numbers the jobs it starts on its own.
#[derive(Clone, Default)]
struct Jobs(Arc<AtomicU64>);

impl Jobs {
    fn next(&self) -> JobId {
        JobId::new(self.0.fetch_add(1, Ordering::Relaxed) + 1)
    }
}

impl Engine {
    /// Stand the engine up, and start its actor.
    ///
    /// Does what the app's launch did on the engine side, in the same order: the sweeps of old
    /// files, the source chain and its plugins loading behind it, the handles the work shares,
    /// the update check and the airing digest, the plugin load's outcome said when it comes, the
    /// joined stores asked for their lists, the trackers' sync loops, the mapping datasets, the
    /// torrent source brought up, and the check of the player.
    ///
    /// What was true at launch is queued at the head of the [`EventStream`], for the front end
    /// to apply before its first frame: the settings, the joined stores, the chain's rows when
    /// it was given whole, the trackers' states, and what the check of the player found. That check runs
    /// mpv, so it is not waited for here: the front end can send its first commands while it
    /// runs, as the app did, and [`EventStream::launch`] waits for it. Everything else the
    /// engine says follows, in the order the actor applied it.
    ///
    /// Spawns on the tokio runtime it is called from, which has to outlive the engine.
    pub async fn start(
        options: EngineOptions,
        host: Arc<dyn Host>,
    ) -> Result<(Self, EventStream), EngineError> {
        let EngineOptions {
            paths,
            config,
            saved,
            http,
            store,
            player_theme,
            sources: given,
            background,
            #[cfg(feature = "testing")]
            parts,
        } = options;

        if background {
            // Old logs, stale covers and the player's leftovers, off the critical path.
            {
                let paths = paths.clone();
                tokio::task::spawn_blocking(move || prune::sweep_at_launch(&paths));
            }
            // With torrents off nothing else would ever prune what an earlier run streamed. With
            // them on, the session prunes on its way up, before it writes anything.
            if !config.providers.torrent.enabled {
                let (dir, limit) = (
                    paths.cache_dir.join("torrents"),
                    config.providers.torrent.cache_limit_bytes(),
                );
                let store = store.clone();
                tokio::spawn(async move {
                    prune::sweep_torrent_cache(dir, limit, &store, Default::default()).await;
                });
            }
        }

        let own_chain = given.is_none();
        let (registry, plugins_loading) = match given {
            Some(registry) => (registry, None),
            None => {
                // Without the torrent source, which comes up behind the interface once the VPN
                // guard passes: see `TorrentTasks::bring_up`.
                let registry = sources::build_registry_deferred(&config, &http);
                tracing::info!(providers = ?registry.ids(), "provider registry ready");
                // Plugins join the chain when they have finished compiling. Nothing waits on
                // this: the first frame is worth more than a source nobody has asked for yet.
                let loading = sources::spawn_plugin_load(
                    registry.clone(),
                    config.clone(),
                    http.clone(),
                    paths.clone(),
                );
                (registry, Some(loading))
            }
        };

        // What every task is lent, built around that chain, or from a test's parts.
        #[cfg(feature = "testing")]
        let trace = parts.is_some();
        #[cfg(feature = "testing")]
        let services = match parts {
            Some(parts) => Services {
                paths,
                http,
                store,
                meta: parts.meta,
                registry,
                sync: parts.sync,
                stores: parts.stores,
                slot: Default::default(),
                prefetch: Default::default(),
                player_theme,
                host,
            },
            None => Services::build(&config, paths, http, store, registry, player_theme, host)
                .map_err(|reason| EngineError::PluginStores { reason })?,
        };
        #[cfg(not(feature = "testing"))]
        let services =
            Services::build(&config, paths, http, store, registry, player_theme, host)
                .map_err(|reason| EngineError::PluginStores { reason })?;

        // Where the torrent source reports in once it is up, tagged with the bring-up it answers.
        let (arrivals_tx, arrivals) = mpsc::unbounded_channel();
        // The engine side's own state, on the config the app was launched with.
        let mut state = EngineState::new(config, saved, &services.paths, arrivals_tx);
        let mut launch = VecDeque::new();
        // The settings first: everything after is drawn under the config they say.
        launch.push_back(Event::Settings(state.config.view()));

        // Joined stores as they were left, so Providers can count them on the first frame.
        let joined = match services.stores.rows() {
            Ok(rows) => {
                let joined = !rows.is_empty();
                launch.push_back(Event::Stores(rows));
                joined
            }
            Err(e) => {
                launch.push_back(Event::Toast(Notice::alert(e)));
                false
            }
        };
        // A chain given whole is final, so its rows are what launch says of it.
        if !own_chain {
            launch.push_back(Event::Providers(data::provider_rows(&services.registry)));
        }

        // Background work reports through this, each piece holding its own sender, and the
        // engine side's messages to itself ride it too, each in its place among the events.
        let mut inbox = Inbox::default();

        if background {
            // The daily update check: one cached request, one quiet toast when a release exists.
            updates::spawn_check(
                state.config.effective().updates.check,
                &services.paths.cache_dir,
                &services.http,
                &inbox.sender(),
            );
            // The airing digest: what aired for shows you watch since the last launch. Same
            // philosophy as the update check — one line, only when there is something to say.
            digest::spawn(
                state.config.effective().notifications.airing_digest,
                state.config.effective().notifications.desktop,
                services.store.clone(),
                services.meta.clone(),
                services.host.clone(),
                inbox.sender(),
            );
        }
        if let Some(loading) = plugins_loading {
            say_plugin_load(loading, inbox.sender());
        }
        if background && joined {
            refresh_joined(services.stores.clone(), inbox.sender());
        }

        for tracker in services.sync.initial_states() {
            launch.push_back(Event::Sync(Box::new(tracker)));
        }
        if background {
            tracking::spawn_loops(services.sync.clone(), inbox.sender());
            // Keep the mapping datasets current without ever blocking startup.
            tokio::spawn(keep_datasets_current(
                services.store.clone(),
                services.http.clone(),
                inbox.sender(),
            ));
        }

        // The torrent stack's live parts, held so a settings change can tear them down and
        // stand them back up without a restart. Both are replaced together: the download
        // manager and the guard's watcher are the two things holding a session alive, and
        // leaving either behind would mean traffic outliving the source it belongs to. A chain
        // given whole has its torrent source in it already, or none.
        if own_chain {
            let tx = inbox.sender();
            let EngineState { config, torrent_tasks, arrivals, .. } = &mut state;
            torrent_tasks.bring_up(config.effective(), &services, &tx, arrivals);
        }

        // The player is checked last, as launch always has, and not waited for.
        let checking = background.then(|| {
            let config = state.config.effective();
            let mpv = player(config, &services.paths, &services.player_theme);
            let binary = config.playback.mpv_binary.clone();
            tokio::spawn(async move { launch_notice(&mpv, &binary).await.map(Event::Toast) })
        });

        let (commands_tx, commands) = mpsc::unbounded_channel();
        let (out, live) = mpsc::unbounded_channel();
        let jobs = Jobs::default();
        let actor = Actor {
            state,
            services,
            inbox,
            commands,
            arrivals,
            out,
            jobs: jobs.clone(),
            #[cfg(feature = "testing")]
            routed: 0,
            #[cfg(feature = "testing")]
            trace,
            #[cfg(feature = "testing")]
            injected: Default::default(),
        };
        tokio::spawn(actor.run());

        let engine = Self {
            commands: commands_tx,
            jobs,
            #[cfg(feature = "testing")]
            sent: AtomicU64::new(0),
        };
        Ok((engine, EventStream { launch, checking, live }))
    }

    /// Hand the engine a command, and say which job it starts, for work someone waits on.
    ///
    /// Only puts it on the actor's channel, so any thread may call this, async or not. The job
    /// is numbered here, once each, from one, and the actor announces it with
    /// [`Event::JobStarted`] before the command is routed, so the announcement comes ahead of
    /// everything the work says. Commands are routed one at a time in the order they are sent,
    /// and whatever one of them does before the next is routed, such as a setting written, is
    /// done for the next to see.
    ///
    /// `None` too once the engine has stopped, when nothing would announce the job.
    pub fn send(&self, command: Command) -> Option<JobId> {
        let job = command.activity().map(|activity| (self.jobs.next(), activity));
        let id = job.as_ref().map(|(id, _)| *id);
        #[cfg(feature = "testing")]
        self.sent.fetch_add(1, Ordering::Relaxed);
        self.commands.send(Message::Command { job, command }).ok()?;
        id
    }

    /// Stop as the app quits, after everything sent before.
    ///
    /// A playback under way is stopped, the way `x` stops it, and given a moment to record
    /// where it was left: it writes its last history row before it reports its ending, which
    /// is what this waits for. The tracker writes a row every ten seconds of playback, so
    /// quitting without this lost up to that much of the resume point. `playing` is whether
    /// one is under way: started, not still finding its source, which has nothing to save and
    /// whose wait would only delay the exit. The front end says so for now, since what is on
    /// screen is its own until P4 gives the engine its own record of the playback.
    ///
    /// Then the torrent session, if there is one, is stood down in order: each torrent paused
    /// so the next launch knows how far it got, then its tasks and connections stopped. That
    /// is bounded at a second, and with torrents off there is nothing to wait for.
    pub async fn shutdown(self, playing: bool) {
        let (done, stopped) = oneshot::channel();
        if self.commands.send(Message::Shutdown { playing, done }).is_ok() {
            let _ = stopped.await;
        }
    }

    /// Commands sent so far: what [`Traced::Idle`] is read against.
    #[cfg(feature = "testing")]
    pub fn sent(&self) -> u64 {
        self.sent.load(Ordering::Relaxed)
    }

    /// Hand the engine an event as its work would send it, after everything sent before: what a
    /// playback says, for a test that has no player. Counted as a command, so the engine falls
    /// idle only once it has done its part with the event and passed it on.
    #[cfg(feature = "testing")]
    pub fn inject(&self, event: Event) {
        self.sent.fetch_add(1, Ordering::Relaxed);
        let _ = self.commands.send(Message::Inject(event));
    }

    /// [`Self::inject`], as a playback of `episode` of `id` would send it, named with that
    /// playback: `playback` numbers it, from a million up so that no playback the engine
    /// starts is mistaken for it. The first event under a number starts it.
    #[cfg(feature = "testing")]
    pub fn inject_from(
        &self,
        playback: u64,
        id: anistream_core::ids::AnilistId,
        episode: &str,
        event: Event,
    ) {
        self.sent.fetch_add(1, Ordering::Relaxed);
        let played = Played { serial: playback, id, episode: episode.to_owned() };
        let _ = self.commands.send(Message::InjectFrom(played, event));
    }

    /// The engine's record of the titles, as it stands when the engine reads the request:
    /// after every command sent before it is routed. For a test that has settled, everything
    /// they led to is in it.
    #[cfg(feature = "testing")]
    pub async fn title_cache(&self) -> crate::cache::TitleCache {
        let (tx, rx) = oneshot::channel();
        let _ = self.commands.send(Message::TitleCache(tx));
        rx.await.expect("the engine answers while it runs")
    }

    /// Whether the engine sees a playback under way, as the exit would: the one Now Playing
    /// shows, started and not ended. Read after every command sent before it is routed.
    #[cfg(feature = "testing")]
    pub async fn under_way(&self) -> bool {
        let (tx, rx) = oneshot::channel();
        let _ = self.commands.send(Message::UnderWay(tx));
        rx.await.expect("the engine answers while it runs")
    }
}

/// Say whatever the background plugin load concluded, once. A plugin that failed to load must
/// not be silent: the Providers screen is where a missing source gets explained.
fn say_plugin_load(loading: JoinHandle<Option<String>>, tx: Outbox) {
    tokio::spawn(async move {
        if let Ok(outcome) = loading.await {
            match outcome {
                Some(reason) => {
                    let _ = tx.send(Event::ProviderNote(reason));
                }
                // Ranked again by the order as it stands now, since the user may have moved a
                // source while these compiled, and the Providers screen refreshed so late
                // arrivals actually appear on it.
                None => {
                    let _ = tx.internal(Internal::SourcesReordered);
                }
            }
        }
    });
}

/// Ask each joined store for its current list, once, in the background. Updates are said once
/// at launch, the way a new release is, and never installed without being asked. Asked of the
/// rows launch said, which the front end was given whole.
fn refresh_joined(stores: plugin_stores::Stores, tx: Outbox) {
    tokio::spawn(async move {
        if let Ok(rows) = stores.refresh().await {
            let _ = tx.send(Event::Stores(rows));
        }
        let updates = stores.pending_updates();
        // Where to install them is the front end's to say: the terminal's Providers, Enter.
        if !updates.is_empty() {
            let notice = Notice::info(format!(
                "plugin {} ready: {}",
                if updates.len() == 1 { "update" } else { "updates" },
                updates.join(", ")
            ));
            let _ = tx.send(Event::Toast(notice.with_hint(NoticeHint::OpenStores)));
        }
    });
}

/// What happened, as a front end reads it: what launch found, then every event in the order
/// the engine applied it.
///
/// Pulled rather than pushed, so a front end reads it when it is ready to apply what it reads,
/// on its own thread: [`Self::next_events`] waits for a batch, [`Self::try_next_events`] takes
/// what is there, and [`Self::recv`] takes one at a time, for a loop that selects over it.
pub struct EventStream {
    /// What launch found, not yet taken.
    launch: VecDeque<Event>,
    /// The check of the player, the last of what launch finds, until it is taken.
    checking: Option<JoinHandle<Option<Event>>>,
    /// Everything since, from the actor.
    live: mpsc::UnboundedReceiver<Out>,
}

/// What the actor puts on the stream.
enum Out {
    /// For the front end.
    Event(Event),
    /// A message the engine side left itself was handled here, after what it said: see
    /// [`Traced::Handled`].
    #[cfg(feature = "testing")]
    Handled(Internal),
    /// Nothing left to do: see [`Traced::Idle`].
    #[cfg(feature = "testing")]
    Idle(u64),
}

impl Out {
    /// The event, for a front end: what only a test wants to hear is passed over.
    fn event(self) -> Option<Event> {
        match self {
            Self::Event(event) => Some(event),
            #[cfg(feature = "testing")]
            Self::Handled(_) | Self::Idle(_) => None,
        }
    }
}

impl EventStream {
    /// What was true at launch, whole: the joined stores, the chain's rows when it was given
    /// whole, the trackers' states, and what the check of the player found, which this waits
    /// for. For the front end to apply before its first frame, so the first frame shows the
    /// app as it was launched. Nothing said since is in it; empty once taken.
    pub async fn launch(&mut self) -> Vec<Event> {
        if let Some(checking) = self.checking.take()
            && let Ok(Some(notice)) = checking.await
        {
            self.launch.push_back(notice);
        }
        self.launch.drain(..).collect()
    }

    /// The next event, waiting for one: launch's first, then everything since. `None` once the
    /// engine has stopped.
    ///
    /// Cancel-safe, for a loop that selects over it beside its own events: an event is taken
    /// only when it is returned.
    pub async fn recv(&mut self) -> Option<Event> {
        if let Some(event) = self.launch.pop_front() {
            return Some(event);
        }
        if let Some(checking) = &mut self.checking {
            let found = checking.await;
            self.checking = None;
            if let Ok(Some(event)) = found {
                return Some(event);
            }
        }
        while let Some(out) = self.live.recv().await {
            if let Some(event) = out.event() {
                return Some(event);
            }
        }
        None
    }

    /// The events there are, waiting for at least one: a batch for a front end to apply in
    /// one go. Empty once the engine has stopped.
    pub async fn next_events(&mut self) -> Vec<Event> {
        let Some(first) = self.recv().await else { return Vec::new() };
        let mut events = vec![first];
        events.extend(self.try_next_events());
        events
    }

    /// The events there are, without waiting: what launch found, as far as it is known, and
    /// then everything since. Nothing since launch comes before the check of the player has
    /// been taken, so the order holds however this is called.
    pub fn try_next_events(&mut self) -> Vec<Event> {
        let mut events: Vec<Event> = self.launch.drain(..).collect();
        if let Some(checking) = &mut self.checking {
            if !checking.is_finished() {
                return events;
            }
            if let Some(Ok(Some(event))) = futures::FutureExt::now_or_never(checking) {
                events.push(event);
            }
            self.checking = None;
        }
        while let Ok(out) = self.live.try_recv() {
            events.extend(out.event());
        }
        events
    }

    /// The next thing the engine says, with the two things only a test wants to hear: each
    /// message the engine side left itself, in the place it was handled, and the moment it had
    /// nothing left to do. Only an engine started with [`crate::options::Parts`] says them.
    #[cfg(feature = "testing")]
    pub async fn next_traced(&mut self) -> Option<Traced> {
        if let Some(event) = self.launch.pop_front() {
            return Some(Traced::Event(event));
        }
        Some(match self.live.recv().await? {
            Out::Event(event) => Traced::Event(event),
            Out::Handled(message) => Traced::Handled(message),
            Out::Idle(routed) => Traced::Idle { routed },
        })
    }
}

/// What an engine started from a test's parts says: its events, and two things beside them.
#[cfg(feature = "testing")]
pub enum Traced {
    /// An event, as any front end reads it.
    Event(Event),
    /// A message the engine side left itself, handled after everything it said was passed on,
    /// and before anything after it.
    Handled(Internal),
    /// The engine had nothing left to do once it had routed this many commands: none queued,
    /// nothing in its inbox, and no work holding a way to answer. So everything those commands
    /// led to has been said, ahead of this; once `routed` is every command sent, there is no
    /// more to wait for. Said each time the engine falls idle.
    Idle { routed: u64 },
}

/// The engine's own task: the state, the handles, and everything it reads.
struct Actor {
    state: EngineState,
    services: Services,
    inbox: Inbox,
    commands: mpsc::UnboundedReceiver<Message>,
    arrivals: mpsc::UnboundedReceiver<(u64, Result<TorrentUp, String>)>,
    out: mpsc::UnboundedSender<Out>,
    /// The count the jobs it starts on its own are numbered by: see [`Jobs`].
    jobs: Jobs,
    /// Commands routed so far.
    #[cfg(feature = "testing")]
    routed: u64,
    /// Whether to say what only a test wants to hear: see [`Traced`].
    #[cfg(feature = "testing")]
    trace: bool,
    /// The playbacks a test has spoken for, by number: see [`Engine::inject_from`].
    #[cfg(feature = "testing")]
    injected: std::collections::HashSet<u64>,
}

impl Actor {
    /// Read and act until the front end lets go of the engine or shuts it down.
    ///
    /// Commands first, then the inbox, then the torrent source: a command is routed as soon as
    /// it is sent, as the loop dispatched it in the same pass that produced it, and what a
    /// bring-up says on its way up is in the inbox before its arrival is.
    async fn run(mut self) {
        loop {
            // Each pass follows one thing done, so this is said once each time the engine
            // falls idle.
            #[cfg(feature = "testing")]
            if self.trace && self.idle() {
                let _ = self.out.send(Out::Idle(self.routed));
            }
            tokio::select! {
                biased;
                message = self.commands.recv() => match message {
                    Some(Message::Command { job, command }) => self.route(job, command),
                    Some(Message::Shutdown { playing, done }) => {
                        self.stop(playing).await;
                        let _ = done.send(());
                        return;
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::Inject(event)) => {
                        let _ = self.inbox.sender().send(event);
                        self.routed += 1;
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::InjectFrom(played, event)) => {
                        if self.injected.insert(played.serial) {
                            self.started(&played);
                        }
                        let _ = self.inbox.sender().for_playback(played).send(event);
                        self.routed += 1;
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::TitleCache(answer)) => {
                        let _ = answer.send(self.state.cache.clone());
                    }
                    #[cfg(feature = "testing")]
                    Some(Message::UnderWay(answer)) => {
                        let _ = answer.send(self.under_way());
                    }
                    // Every handle on the engine has gone.
                    None => return,
                },
                envelope = self.inbox.recv(), if !self.inbox.is_empty() => {
                    if let Some(envelope) = envelope {
                        self.act_on(envelope).await;
                    }
                }
                Some((generation, outcome)) = self.arrivals.recv() => {
                    self.arrived(generation, outcome);
                }
            }
        }
    }

    /// No command queued, nothing in the inbox, no work holding a way to answer, and no torrent
    /// source reporting in.
    #[cfg(feature = "testing")]
    fn idle(&self) -> bool {
        self.inbox.is_empty() && self.commands.is_empty() && self.arrivals.is_empty()
    }

    /// Route a command, its job announced first: everything the command does before it returns
    /// is done before the next is routed.
    fn route(&mut self, job: Option<(JobId, String)>, command: Command) {
        let tx = self.inbox.sender();
        let job = job.map(|(id, activity)| JobGuard::announce(id, activity, &tx));
        dispatch(command, job, &mut self.state, &self.services, &tx);
        #[cfg(feature = "testing")]
        {
            self.routed += 1;
        }
    }

    /// One item of the inbox, in its place.
    async fn act_on(&mut self, envelope: Envelope) {
        match envelope {
            Envelope::Event(event) => self.forward(event, None),
            Envelope::Played(played, event) => self.forward(event, Some(played)),
            // What the engine side left itself to do, after everything sent before it and
            // before anything sent after.
            Envelope::Internal(message) => {
                let tx = self.inbox.sender();
                let said = handle_internal(message, &mut self.state, &self.services, &tx).await;
                said.into_iter().for_each(|said| self.pass_on(said));
                #[cfg(feature = "testing")]
                if self.trace {
                    let _ = self.out.send(Out::Handled(message));
                }
            }
        }
    }

    /// An event on its way to the front end, from the playback `played` when one said it: the
    /// engine side's part in it, and what it has to say ahead of it.
    ///
    /// A title the event moves on — a tick of the playhead, an ending, a window handed over —
    /// is said whole just before it, so that whatever the event brings up, the screen a
    /// finished episode drops back to among them, shows the title as it now stands.
    fn forward(&mut self, mut event: Event, played: Option<Played>) {
        let Self { services, inbox, .. } = self;
        let said = receive(&event, services, || inbox.sender());
        said.into_iter().for_each(|said| self.pass_on(said));
        // What it says of a title, taken into the record, and what that moved.
        let moved = self.state.cache.observe(&event, played.as_ref());
        if let Some(progress) = moved.and_then(|id| self.state.cache.progress(id)) {
            self.pass_on(progress);
        }
        // What the player changed of the config, kept and saved, with what the save has to say
        // coming after the event, in its place in the inbox.
        let Self { services, inbox, state, .. } = self;
        settings::remember(&event, state, services, || inbox.sender());
        // What it leads to on its own, started now and said right after it, so nothing the
        // front end reads comes between an ending and the episode it rolls on to.
        let after = self.follow(&mut event, played.as_ref());
        self.pass_on(event);
        after.into_iter().for_each(|said| self.pass_on(said));
    }

    /// Start what a playback's event leads to — the next episode an ending rolls on to, an
    /// episode picked in the player, the episode restarted in the other translation, the next
    /// episode's streams found ahead — and return what to say of it after the event: see
    /// [`autonext`]. An ending that rolls on is said to.
    ///
    /// A play started here is a job like one the front end asks for, numbered from the same
    /// count, and staged like one: it is announced, then said as `PlaybackStaged`.
    fn follow(&mut self, event: &mut Event, played: Option<&Played>) -> Vec<Event> {
        let finished = self.state.staging.observe(event, played);
        if let Event::PrefetchNext { id, episode } = event {
            prefetch(*id, episode.clone(), &self.state, &self.services);
            return Vec::new();
        }
        let skip_filler = self.state.config.effective().playback.skip_filler;
        let (mut said, follow) = autonext::follow(
            event,
            finished,
            self.state.staging.front(),
            &self.state.cache,
            skip_filler,
        );
        let Some(follow) = follow else { return said };
        if let Event::PlaybackEnded { rolling_on, .. } = event {
            *rolling_on = true;
        }
        let tx = self.inbox.sender();
        let play = Command::Play { id: follow.id, episode: follow.episode.clone() };
        let job =
            play.activity().map(|activity| JobGuard::announce(self.jobs.next(), activity, &tx));
        said.push(play_following(follow, job, &mut self.state, &self.services, &tx));
        said
    }

    /// Whether a playback is under way: the one Now Playing shows has started playing, and
    /// has not ended. One still finding its stream has nothing to record.
    // Read by the cross-check alone until the exit asks the engine, not the front end.
    #[cfg_attr(not(feature = "testing"), allow(dead_code))]
    fn under_way(&self) -> bool {
        let EngineState { staging, cache, .. } = &self.state;
        staging.front().is_some_and(|front| cache.under_way(front.serial))
    }

    /// A playback a test speaks for has started, as a play the front end asked for starts
    /// one: the record takes what it says from now, and Now Playing shows it.
    #[cfg(feature = "testing")]
    fn started(&mut self, played: &Played) {
        use crate::{cache::PlayKind, playback::autonext::Staged};
        let EngineState { cache, staging, .. } = &mut self.state;
        cache.started(played);
        let title = cache.name_for(played.id, PlayKind::Staged);
        cache.announce(played.id, title.clone());
        staging.staged(Staged {
            serial: played.serial,
            id: played.id,
            episode: played.episode.clone(),
            title,
            episode_title: cache.episode_title(played.id, &played.episode),
        });
    }

    /// The torrent source is up, or will not come up; only the latest bring-up counts.
    fn arrived(&mut self, generation: u64, outcome: Result<TorrentUp, String>) {
        let tx = self.inbox.sender();
        let EngineState { torrent_tasks, vpn_guard, shared_config, .. } = &mut self.state;
        torrent_tasks.arrived(
            generation,
            outcome,
            vpn_guard,
            &self.services,
            shared_config,
            &tx,
        );
    }

    fn pass_on(&self, event: Event) {
        let _ = self.out.send(Out::Event(event));
    }

    /// Everything the app does on its way out: the playback's last record, then the torrents.
    async fn stop(&mut self, playing: bool) {
        self.flush_playback(playing).await;
        self.close_torrents().await;
    }

    /// Stand the torrent session down within [`TORRENT_CLOSE`], then let the guard go.
    async fn close_torrents(&mut self) {
        let closing = self.state.torrent_tasks.close();
        if tokio::time::timeout(TORRENT_CLOSE, closing).await.is_err() {
            tracing::debug!("the torrent session was still winding down as the app quit");
        }
        self.state.vpn_guard = None;
    }

    /// Stop what is playing, if it is under way, and wait briefly for it to record where it
    /// ended: everything the inbox holds meanwhile is let go, unread, as the app quits.
    async fn flush_playback(&mut self, playing: bool) {
        let Some(controls) = self.state.player_tx.as_ref().filter(|_| playing) else { return };
        if controls.send(PlayerCommand::Stop).is_err() {
            return;
        }
        let ended = async {
            while let Some(envelope) = self.inbox.recv().await {
                if matches!(
                    envelope,
                    Envelope::Event(Event::PlaybackEnded { .. })
                        | Envelope::Played(_, Event::PlaybackEnded { .. })
                ) {
                    break;
                }
            }
        };
        if tokio::time::timeout(EXIT_FLUSH, ended).await.is_err() {
            tracing::warn!("the player did not stop in time; its last position may be lost");
        }
    }
}

/// The engine side's inbox: the channels the work answers on, read in the order they opened.
///
/// The actor holds no sender of its own between one thing and the next, only a weak one, and
/// lends a strong one to each piece of work. So a channel closes exactly when the last work
/// holding it ends, and an inbox with every channel closed and read is an engine with nothing
/// outstanding: what [`Traced::Idle`] says, with no waiting for a quiet moment. Work started
/// after a channel closed is lent a new one, read after it: everything on the old one was sent
/// before the new one existed, so the order holds.
#[derive(Default)]
struct Inbox {
    open: VecDeque<mpsc::UnboundedReceiver<Envelope>>,
    newest: Option<WeakOutbox>,
}

impl Inbox {
    /// A way to answer: onto the newest channel while it is open, or onto a new one.
    fn sender(&mut self) -> Outbox {
        if let Some(tx) = self.newest.as_ref().and_then(WeakOutbox::upgrade) {
            return tx;
        }
        let (tx, rx) = Outbox::inbox();
        self.newest = Some(tx.downgrade());
        self.open.push_back(rx);
        tx
    }

    /// Every channel closed and read: no work holds a way to answer, and nothing is unread.
    fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// The next item, from the oldest channel still open or unread. `None` once there is none.
    ///
    /// Cancel-safe: a channel is let go only once it is closed and read.
    async fn recv(&mut self) -> Option<Envelope> {
        while let Some(oldest) = self.open.front_mut() {
            match oldest.recv().await {
                Some(envelope) => return Some(envelope),
                None => {
                    self.open.pop_front();
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::config::{Config, Paths};
    use anistream_model::playback::PlayerTheme;
    use anistream_net::HttpClient;
    use anistream_providers::ProviderRegistry;

    use crate::host::DesktopHost;

    /// An engine on the defaults, in `dir`, with no sources, doing nothing it is not asked to,
    /// and keeping its tokens in files there rather than in the keychain.
    async fn quiet_engine(dir: &std::path::Path) -> (Engine, EventStream) {
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let mut config = Config::default();
        config.trackers.token_storage = "file".into();
        let http = HttpClient::offline(&config.network).unwrap();
        let store = anistream_store::Store::open_in_memory().unwrap();
        let theme = PlayerTheme {
            text: "#E4E0D6".into(),
            dim: "#9298B5".into(),
            obi: "#F2A64B".into(),
            state: "#7FC7D9".into(),
            alert: "#E06B54".into(),
            ground: "#161A2E".into(),
            font: "sans-serif".into(),
        };
        let mut options = EngineOptions::new(paths, config, http, store, theme);
        options.sources = Some(ProviderRegistry::new(Vec::new()));
        options.background = false;
        Engine::start(options, Arc::new(DesktopHost)).await.unwrap()
    }

    #[tokio::test]
    async fn launch_says_the_settings_the_stores_the_chain_and_the_trackers_in_that_order() {
        let dir = tempfile::tempdir().unwrap();
        let (_engine, mut events) = quiet_engine(dir.path()).await;
        let launch: Vec<&str> = events
            .launch()
            .await
            .iter()
            .map(|event| match event {
                Event::Settings(_) => "settings",
                Event::Stores(_) => "stores",
                Event::Providers(_) => "providers",
                Event::Sync(_) => "sync",
                _ => "other",
            })
            .collect();
        // Kitsu is the one tracker the defaults build.
        assert_eq!(launch, ["settings", "stores", "providers", "sync"]);
        assert!(events.launch().await.is_empty(), "taken once");
    }

    #[tokio::test]
    async fn a_job_is_numbered_at_once_and_announced_before_its_work_says_anything() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        let score =
            Command::SetScore { id: anistream_core::ids::AnilistId::new(1), score: 7.0 };
        assert_eq!(engine.send(score), None, "nothing to wait on, so no job");
        let first = engine.send(Command::LoadDownloads);
        let second = engine.send(Command::CheckProviders).expect("a job");
        assert_eq!(first, None, "the downloads are read at once");
        assert_eq!(second, JobId::new(1), "numbered from one");

        let mut seen = Vec::new();
        while !matches!(seen.last(), Some(Event::JobDone(_))) {
            seen.extend(events.next_events().await);
        }
        let Some(Event::JobStarted { job, activity }) =
            seen.iter().find(|e| matches!(e, Event::JobStarted { .. }))
        else {
            panic!("announced")
        };
        assert_eq!((*job, activity.as_str()), (second, "checking providers"));
        let started = seen.iter().position(|e| matches!(e, Event::JobStarted { .. }));
        let answered = seen.iter().position(|e| matches!(e, Event::Providers(_)));
        assert!(started < answered, "announced before the answer");
        // The downloads, sent first, are answered first.
        assert!(matches!(seen[0], Event::Downloads(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_thread_that_is_not_async_can_send() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        let engine = Arc::new(engine);
        let sender = Arc::clone(&engine);
        let job =
            std::thread::spawn(move || sender.send(Command::CheckProviders)).join().unwrap();
        assert_eq!(job, Some(JobId::new(1)));
        let mut done = false;
        while !done {
            done = events.next_events().await.iter().any(|e| matches!(e, Event::JobDone(_)));
        }
    }

    /// Events until one `done` says is the last, or a second of nothing.
    async fn until(events: &mut EventStream, done: impl Fn(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        while !seen.last().is_some_and(&done) {
            match tokio::time::timeout(Duration::from_secs(1), events.next_events()).await {
                Ok(batch) if !batch.is_empty() => seen.extend(batch),
                _ => panic!("nothing more, having seen {seen:?}"),
            }
        }
        seen
    }

    #[tokio::test]
    async fn a_step_asked_for_is_put_in_effect_written_and_said_whole() {
        use anistream_model::settings::SettingKey;
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        engine.send(Command::StepSetting { key: SettingKey::AutoNext, delta: 1 });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(saved)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        let row = view.rows.iter().find(|row| row.key == SettingKey::AutoNext).unwrap();
        assert_eq!(row.value, "off");
        assert_eq!(view.refused, None);
        assert_eq!(saved.text, "saved · auto-play next");
        let file = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert_eq!(file, "[playback]\nauto_next = false\n");
    }

    #[tokio::test]
    async fn a_refused_write_is_said_with_its_setting_put_back() {
        use anistream_model::{notice::NoticeKind, settings::SettingKey};
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        // Torrents with no VPN configured: refused by the config's own rules.
        engine.send(Command::StepSetting { key: SettingKey::Torrents, delta: 1 });
        let seen = until(&mut events, |e| matches!(e, Event::Toast(_))).await;
        let [Event::Settings(view), Event::Toast(refusal)] = &seen[..] else {
            panic!("the settings, then the notice: {seen:?}")
        };
        assert_eq!(view.refused, Some(SettingKey::Torrents));
        let row = view.rows.iter().find(|row| row.key == SettingKey::Torrents).unwrap();
        assert_eq!(row.value, "off", "put back to what the file holds");
        assert!(!view.sources.torrent);
        assert_eq!(refusal.kind, NoticeKind::Alert);
        assert!(refusal.text.starts_with("not saved: torrents need"), "{}", refusal.text);
        assert!(!dir.path().join("config.toml").exists(), "nothing written");
    }

    #[tokio::test]
    async fn sub_or_dub_for_the_session_is_put_in_effect_and_not_written() {
        use anistream_core::media::Translation;
        use anistream_model::settings::SettingKey;
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;

        engine.send(Command::SetTranslation { translation: Translation::Dub });
        let seen = until(&mut events, |e| matches!(e, Event::Settings(_))).await;
        let [Event::Settings(view)] = &seen[..] else { panic!("the settings alone: {seen:?}") };
        assert_eq!(view.translation, Translation::Dub);
        let row = view.rows.iter().find(|row| row.key == SettingKey::Translation).unwrap();
        assert_eq!(row.value, "sub · dub this session");
        assert!(!dir.path().join("config.toml").exists(), "the file is the saved choice's");
    }

    #[tokio::test]
    async fn quitting_with_nothing_playing_does_not_wait() {
        let dir = tempfile::tempdir().unwrap();
        let (engine, mut events) = quiet_engine(dir.path()).await;
        events.launch().await;
        // Said under way, but no playback was ever started: there is nothing to stop.
        tokio::time::timeout(Duration::from_millis(500), engine.shutdown(true))
            .await
            .expect("no wait for a player that is not there");
        assert!(events.recv().await.is_none(), "the engine has stopped");
    }
}
