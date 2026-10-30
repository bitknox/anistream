//! Quitting with an episode playing, quitting behind a flood of commands, and next-episode
//! presses faster than the sources answer: the engine as an app with a hosted player drives it,
//! the test playing the part of that player.
#![cfg(feature = "testing")]

use std::{
    collections::HashMap,
    sync::{Arc, Barrier, mpsc},
    thread,
    time::{Duration, Instant},
};

use anistream_core::{
    error::ProviderError,
    ids::ProviderKey,
    media::{Episode, SearchHit, Translation},
    stream::Stream,
    traits::{Provider, ProviderManifest, SourceCandidate},
};
use anistream_ffi::{FfiDirs, FfiEngine, FfiHost};
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
    hosted::{PlayerEnd, PlayerOp, PlayerReport},
    playback::PlaybackId,
};
use anistream_providers::ProviderRegistry;
use anistream_testkit::{fixtures::FINISHED, sources};
use serde_json::json;

/// How long a test waits for what it is waiting on. Everything is on loopback.
const PATIENCE: Duration = Duration::from_secs(30);

/// How long each file runs, in seconds.
const RUNTIME: f64 = 120.0;

/// A host that opens nothing, shows nothing and keeps no secrets.
struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

/// The engine, and the test as the player it drives.
struct App {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    /// Everything read so far, for a failure to show.
    read: Vec<String>,
    root: tempfile::TempDir,
}

/// What the player was told while the test drove it.
#[derive(Default, Debug)]
struct Seen {
    /// The players shown.
    opens: Vec<PlaybackId>,
    /// Each file loaded: its playback, its title, and when, in milliseconds from the start.
    loads: Vec<(PlaybackId, String, u128)>,
}

impl Seen {
    fn titles(&self) -> Vec<&str> {
        self.loads.iter().map(|(_, title, _)| title.as_str()).collect()
    }
}

impl App {
    fn start(sources: ProviderRegistry) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine = FfiEngine::start_testing_with(dirs, Arc::new(Quiet), sources)
            .expect("the engine starts");
        // Read on a plain thread with an executor that is not tokio's, as an app reads.
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self { engine, events, read: Vec::new(), root }
    }

    /// Read until `want` finds what it wants in an event, and return that.
    fn until<T>(&mut self, what: &str, mut want: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(event) = self.events.recv_timeout(left) else {
                panic!("no {what} in time; the engine said:\n{}", self.read.join("\n"))
            };
            self.read.push(format!("{event:?}"));
            if let Some(found) = want(&event) {
                return found;
            }
        }
    }

    /// The next file the player is told to load.
    fn load(&mut self) -> PlaybackId {
        self.until("file to load", |event| match event {
            Event::PlayerOps { playback, ops }
                if ops.iter().any(|op| matches!(op, PlayerOp::Load { .. })) =>
            {
                Some(*playback)
            }
            _ => None,
        })
    }

    /// Until the engine says the playhead is at `position`.
    fn at(&mut self, position: f64) {
        self.until("playhead there", |event| {
            matches!(event, Event::Playback { position: at, .. } if *at == position)
                .then_some(())
        });
    }

    fn play(&self, episode: &str) {
        self.engine.send(Command::play(FINISHED, episode));
    }

    fn report(&self, playback: PlaybackId, report: PlayerReport) {
        assert_eq!(self.engine.send(Command::PlayerReport { playback, report }), None);
    }

    fn property(&self, playback: PlaybackId, name: &str, value: serde_json::Value) {
        let report = PlayerReport::Property { name: name.into(), json: value.to_string() };
        self.report(playback, report);
    }

    /// The file loaded, as libmpv reports it.
    fn opened(&self, playback: PlaybackId) {
        self.report(playback, PlayerReport::FileLoaded);
        self.property(playback, "duration", json!(RUNTIME));
    }

    /// The playhead from `from` to `to`, a second at a time.
    fn tick(&self, playback: PlaybackId, from: u32, to: u32) {
        for second in from..=to {
            self.property(playback, "time-pos", json!(f64::from(second)));
        }
    }

    /// Read for `span`, as an app's player: every file loaded is opened and ticked a little,
    /// and every quit is answered with the file ended and the player closed.
    fn drive(&mut self, span: Duration) -> Seen {
        let mut seen = Seen::default();
        let began = Instant::now();
        let until = began + span;
        while let Ok(event) =
            self.events.recv_timeout(until.saturating_duration_since(Instant::now()))
        {
            self.read.push(format!("{event:?}"));
            let Event::PlayerOps { playback, ops } = &event else { continue };
            for op in ops {
                match op {
                    PlayerOp::Open { .. } => seen.opens.push(*playback),
                    PlayerOp::Load { options, .. } => {
                        let title = options
                            .iter()
                            .find(|o| o.name == "force-media-title")
                            .map(|o| o.value.clone())
                            .unwrap_or_default();
                        seen.loads.push((*playback, title, began.elapsed().as_millis()));
                        self.opened(*playback);
                        self.tick(*playback, 0, 3);
                    }
                    PlayerOp::Quit => {
                        self.report(*playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
                        self.report(*playback, PlayerReport::Closed);
                    }
                    _ => {}
                }
            }
        }
        seen
    }

    /// The positions the engine wrote for `episode` of the finished fixture title, earliest
    /// first.
    fn history(&self, episode: &str) -> Vec<f64> {
        let store = anistream_store::Store::open(self.root.path().join("data/anistream.db"))
            .expect("the engine's database");
        let mut positions: Vec<f64> = store
            .events_for(FINISHED, 1_000)
            .unwrap()
            .into_iter()
            .filter(|event| event.episode == episode)
            .map(|event| event.position_secs)
            .collect();
        positions.sort_by(f64::total_cmp);
        positions
    }
}

/// The fixture source, each episode's streams taking as long to find as `delays` says.
struct Slow {
    inner: Arc<dyn Provider>,
    delays: HashMap<String, Duration>,
}

#[async_trait::async_trait]
impl Provider for Slow {
    fn manifest(&self) -> &ProviderManifest {
        self.inner.manifest()
    }

    async fn search(&self, q: &str, t: Translation) -> Result<Vec<SearchHit>, ProviderError> {
        self.inner.search(q, t).await
    }

    async fn episodes(
        &self,
        key: &ProviderKey,
        t: Translation,
    ) -> Result<Vec<Episode>, ProviderError> {
        self.inner.episodes(key, t).await
    }

    async fn resolve(
        &self,
        key: &ProviderKey,
        episode: &str,
        t: Translation,
    ) -> Result<Vec<Stream>, ProviderError> {
        if let Some(delay) = self.delays.get(episode) {
            tokio::time::sleep(*delay).await;
        }
        self.inner.resolve(key, episode, t).await
    }

    async fn sources(
        &self,
        key: &ProviderKey,
        episode: &str,
        t: Translation,
    ) -> Result<Vec<SourceCandidate>, ProviderError> {
        self.inner.sources(key, episode, t).await
    }
}

fn slow(episode: &str, delay: Duration) -> ProviderRegistry {
    let delays = HashMap::from([(episode.to_owned(), delay)]);
    ProviderRegistry::new(vec![Arc::new(Slow { inner: sources::provider().arc(), delays })])
}

/// Episode 1 playing in the one player.
fn first_playing(app: &mut App) {
    app.play("1");
    let playback = app.load();
    app.opened(playback);
    app.tick(playback, 0, 5);
}

/// Episode 2 is slow to find and 3 is quick: the viewer asks for 2, then 3 a moment later.
/// Only 3 loads, at once, into the window episode 1 had.
fn asked_past(slow_by: Duration) -> Seen {
    let mut app = App::start(slow("2", slow_by));
    first_playing(&mut app);
    app.play("2");
    thread::sleep(Duration::from_millis(50));
    app.play("3");
    let seen = app.drive(slow_by + Duration::from_secs(3));
    app.engine.shutdown();
    seen
}

#[test]
fn an_episode_asked_past_while_it_is_found_gives_way_to_the_next_at_once() {
    let seen = asked_past(Duration::from_secs(4));
    assert!(seen.opens.is_empty(), "no second player window: {seen:#?}");
    assert_eq!(seen.titles(), ["Stardust Wayfarer · Episode 3"], "{seen:#?}");
    assert!(seen.loads[0].2 < 2_000, "3 loaded at once: {seen:#?}");
}

/// Past the ten seconds a new playback waits for the window, the episode asked past used to
/// open a second window, then load the stale episode after the one chosen.
#[test]
fn an_episode_asked_past_opens_no_second_window_however_slow_it_is() {
    let seen = asked_past(Duration::from_secs(13));
    assert!(seen.opens.is_empty(), "no second player window: {seen:#?}");
    assert_eq!(seen.titles(), ["Stardust Wayfarer · Episode 3"], "{seen:#?}");
}

#[test]
fn presses_faster_than_anything_loads_end_on_the_last_in_one_window() {
    let mut app = App::start(sources::registry());
    first_playing(&mut app);
    for episode in 2..=9 {
        app.play(&episode.to_string());
    }
    let seen = app.drive(Duration::from_secs(8));
    assert!(seen.opens.is_empty(), "no second window: {seen:#?}");
    assert_eq!(seen.titles().last(), Some(&"Stardust Wayfarer · Episode 9"), "{seen:#?}");
    app.engine.shutdown();
}

/// Episode 12 playing to 45 seconds, then the app shuts the engine down from `threads` threads
/// at once while the test answers the player's quit: the positions written.
fn quit_from(threads: usize) -> Vec<f64> {
    let mut app = App::start(sources::registry());
    app.play("12");
    let playback = app.load();
    app.opened(playback);
    app.tick(playback, 0, 45);
    app.at(45.0);
    let barrier = Arc::new(Barrier::new(threads));
    let quitting: Vec<_> = (0..threads)
        .map(|_| {
            let (engine, barrier) = (Arc::clone(&app.engine), Arc::clone(&barrier));
            thread::spawn(move || {
                barrier.wait();
                engine.shutdown();
            })
        })
        .collect();
    app.drive(Duration::from_secs(3));
    for quit in quitting {
        quit.join().expect("shutdown does not panic");
    }
    app.history("12")
}

/// The FFI let go of the engine before its last save, so the player's report of its end was
/// dropped and every quit lost up to ten seconds of the resume point.
#[test]
fn quitting_with_an_episode_playing_keeps_where_it_was_left() {
    let history = quit_from(1);
    assert_eq!(history.last(), Some(&45.0), "{history:?}");
}

#[test]
fn two_quits_at_once_keep_where_it_was_left_too() {
    for _ in 0..3 {
        let history = quit_from(2);
        assert_eq!(history.last(), Some(&45.0), "{history:?}");
    }
}

/// The quit waited behind every command sent before it, which a host sending fast made a
/// minute and more.
#[test]
fn a_quit_behind_a_flood_of_commands_is_over_in_seconds() {
    let app = App::start(sources::registry());
    let flooding = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let senders: Vec<_> = (0..4)
        .map(|i| {
            let (engine, flooding) = (Arc::clone(&app.engine), Arc::clone(&flooding));
            thread::spawn(move || {
                let mut n = 0u32;
                while flooding.load(std::sync::atomic::Ordering::Relaxed) {
                    let command = match (i + n) % 3 {
                        0 => Command::play(FINISHED, (n % 12 + 1).to_string()),
                        1 => Command::Player(PlayerCommand::Stop),
                        _ => Command::Player(PlayerCommand::TogglePause),
                    };
                    let _ = engine.send(command);
                    n += 1;
                }
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(500));
    let began = Instant::now();
    app.engine.shutdown();
    let took = began.elapsed();
    flooding.store(false, std::sync::atomic::Ordering::Relaxed);
    for sender in senders {
        sender.join().expect("no panic");
    }
    assert_eq!(app.engine.send(Command::play(FINISHED, "1")), None);
    assert!(futures::executor::block_on(app.engine.next_events()).is_empty());
    assert!(took < Duration::from_secs(6), "the quit took {took:?}");
}
