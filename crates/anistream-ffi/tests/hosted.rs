//! The hosted player's contract: the engine driving a player the app hosts, as a native app's
//! libmpv will be driven, with the test as that player.
//!
//! The test reads `Event::PlayerOps` and `Event::PlayerUi` off the engine, as an app does, and
//! answers with `Command::PlayerReport`: the file loading, its properties in mpv's own form, a
//! playhead, and the end. What the viewer does in the player goes back as player commands. Run
//! in any build, with the desktop feature or without: the engine starts on a fake AniList and the
//! test kit's sources, with the app's hosted player whichever it is built with.
#![cfg(feature = "testing")]

use std::{
    io::{Read, Write},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use anistream_ffi::{FfiDirs, FfiEngine, FfiHost};
use anistream_model::{
    catalog::ResumePoint,
    command::{AppState, Command, PlayerCommand},
    event::Event,
    hosted::{
        MpvOption, MpvValue, PlayerChoice, PlayerEnd, PlayerOp, PlayerReport, PlayerUi,
        TrackKind,
    },
    playback::{Ending, PlaybackId, SkipKind},
    progress::TitleProgress,
};
use anistream_providers::ProviderRegistry;
use anistream_testkit::{
    fixtures::{AIRING, FINISHED},
    sources,
};
use serde_json::json;

/// How long a test waits for what it is waiting on. Everything is on loopback, so only the
/// stall prompt takes long, and it takes eight seconds.
const PATIENCE: Duration = Duration::from_secs(30);

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
    /// Everything read so far, as it prints, for a failure to show.
    read: Vec<String>,
    /// Every op the player was told so far, in order, with its playback.
    ops: Vec<(PlaybackId, PlayerOp)>,
    /// Each title's episode table and progress, as an app's title cache keeps them: a table
    /// for every title loaded, moved on by what the engine says of the title's episodes.
    titles: AppTitles,
    root: tempfile::TempDir,
}

/// What an app holds of the titles it was told of, applied as the Mac and Android apps apply
/// it: `Episodes` replaces a title's table, and `Progress` sets the title's standing and the
/// state of each row it names. A title's table is loaded once and kept.
#[derive(Default)]
struct AppTitles {
    tables: std::collections::HashMap<AnilistId, Vec<anistream_model::episodes::EpisodeRow>>,
    progress: std::collections::HashMap<AnilistId, Option<TitleProgress>>,
}

impl AppTitles {
    fn apply(&mut self, event: &Event) {
        match event {
            Event::Episodes { id, rows } => {
                self.tables.insert(*id, rows.clone());
            }
            Event::Progress { id, progress, episodes, .. } => {
                self.progress.insert(*id, *progress);
                let Some(table) = self.tables.get_mut(id) else { return };
                for said in episodes {
                    if let Some(row) = table.iter_mut().find(|row| row.number == said.number) {
                        row.watched = said.watched;
                        row.completed = said.completed;
                    }
                }
            }
            _ => {}
        }
    }

    /// The episodes of `id`'s table that read as watched.
    fn watched(&self, id: AnilistId) -> Vec<&str> {
        let rows = self.tables.get(&id).map(Vec::as_slice).unwrap_or_default();
        rows.iter().filter(|row| row.completed).map(|row| row.number.as_str()).collect()
    }
}

/// A file the player was told to load.
struct Load {
    playback: PlaybackId,
    url: String,
    options: Vec<MpvOption>,
}

impl Load {
    fn option(&self, name: &str) -> Option<&str> {
        self.options.iter().find(|o| o.name == name).map(|o| o.value.as_str())
    }
}

impl App {
    /// The engine on `sources`, with `config` as its config file.
    fn start(sources: ProviderRegistry, config: &str) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        std::fs::create_dir_all(root.path().join("config")).unwrap();
        std::fs::write(root.path().join("config/config.toml"), config).unwrap();
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
        Self {
            engine,
            events,
            read: Vec::new(),
            ops: Vec::new(),
            titles: AppTitles::default(),
            root,
        }
    }

    /// The engine on the test kit's sources: one stream for each episode.
    fn fixtures(config: &str) -> Self {
        Self::start(sources::registry(), config)
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
            self.titles.apply(&event);
            if let Event::PlayerOps { playback, ops } = &event {
                self.ops.extend(ops.iter().map(|op| (*playback, op.clone())));
            }
            if let Some(found) = want(&event) {
                return found;
            }
        }
    }

    /// The next op `want` accepts.
    fn op(&mut self, what: &str, want: impl Fn(&PlayerOp) -> bool) -> (PlaybackId, PlayerOp) {
        self.until(what, |event| match event {
            Event::PlayerOps { playback, ops } => {
                ops.iter().find(|op| want(op)).map(|op| (*playback, op.clone()))
            }
            _ => None,
        })
    }

    /// The next thing for the player's own interface that `want` accepts.
    fn ui(&mut self, what: &str, want: impl Fn(&PlayerUi) -> bool) -> (PlaybackId, PlayerUi) {
        self.until(what, |event| match event {
            Event::PlayerUi { playback, ui } if want(ui) => Some((*playback, ui.clone())),
            _ => None,
        })
    }

    /// The next file the player is told to load.
    fn load(&mut self) -> Load {
        let (playback, op) =
            self.op("a file to load", |op| matches!(op, PlayerOp::Load { .. }));
        let PlayerOp::Load { url, options } = op else { unreachable!() };
        Load { playback, url, options }
    }

    /// How the playback that ends next ended.
    fn ended(&mut self) -> Ending {
        self.until("the playback's ending", |event| match event {
            Event::PlaybackEnded { ending, .. } => Some(ending.clone()),
            _ => None,
        })
    }

    fn play(&self, id: AnilistId, episode: &str) {
        self.engine.send(Command::play(id, episode));
    }

    fn control(&self, command: PlayerCommand) {
        self.engine.send(Command::Player(command));
    }

    /// A command aimed at `playback`, as a hosted player sends what its interface shows.
    fn aim(&self, playback: PlaybackId, command: PlayerCommand) {
        self.engine.send(Command::Control { playback, command });
    }

    fn report(&self, playback: PlaybackId, report: PlayerReport) {
        assert_eq!(self.engine.send(Command::PlayerReport { playback, report }), None);
    }

    fn property(&self, playback: PlaybackId, name: &str, value: serde_json::Value) {
        let report = PlayerReport::Property { name: name.into(), json: value.to_string() };
        self.report(playback, report);
    }

    /// The file loaded, as libmpv reports it: its tracks, with the English dub picked ahead of
    /// the Japanese, its length, and its chapters.
    fn opened(&self, playback: PlaybackId, chapters: &[(&str, f64)]) {
        self.report(playback, PlayerReport::FileLoaded);
        self.property(
            playback,
            "track-list",
            json!([
                { "type": "video", "id": 1, "selected": true },
                { "type": "audio", "id": 1, "lang": "eng", "selected": true },
                { "type": "audio", "id": 2, "lang": "jpn" },
                { "type": "sub", "id": 1, "lang": "eng", "title": "Full" },
            ]),
        );
        self.property(playback, "duration", json!(RUNTIME));
        let chapters: Vec<_> = chapters
            .iter()
            .map(|(title, time)| json!({ "title": title, "time": time }))
            .collect();
        self.property(playback, "chapter-list", json!(chapters));
    }

    /// The playhead from `from` to `to`, a second at a time.
    fn tick(&self, playback: PlaybackId, from: u32, to: u32) {
        for second in from..=to {
            self.property(playback, "time-pos", json!(f64::from(second)));
        }
    }

    /// The viewer stops what plays: the player is told to quit, ends the file and closes.
    fn stop(&mut self) {
        self.control(PlayerCommand::Stop);
        let (playback, _) = self.op("the player told to quit", |op| *op == PlayerOp::Quit);
        self.report(playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
        self.report(playback, PlayerReport::Closed);
        assert_eq!(self.ended(), Ending::Stopped);
    }

    /// The history the engine wrote for `episode` of the finished fixture title, positions
    /// only, earliest first.
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

    /// Whether the player has been told `op` so far.
    fn told(&self, op: &PlayerOp) -> bool {
        self.ops.iter().any(|(_, told)| told == op)
    }

    /// How the playback that ends next ended, or the playback a player was shown for first,
    /// whichever comes first.
    fn ended_or_shown(&mut self) -> Result<Ending, PlaybackId> {
        self.until("an ending, or a player shown", |event| match event {
            Event::PlaybackEnded { ending, .. } => Some(Ok(ending.clone())),
            Event::PlayerOps { playback, ops }
                if ops.iter().any(|op| matches!(op, PlayerOp::Open { .. })) =>
            {
                Some(Err(*playback))
            }
            _ => None,
        })
    }

    /// Read whatever the engine says for a moment, for what should not come to have the chance.
    fn settle(&mut self) {
        let until = Instant::now() + Duration::from_millis(750);
        while let Ok(event) =
            self.events.recv_timeout(until.saturating_duration_since(Instant::now()))
        {
            self.read.push(format!("{event:?}"));
            self.titles.apply(&event);
            if let Event::PlayerOps { playback, ops } = &event {
                self.ops.extend(ops.iter().map(|op| (*playback, op.clone())));
            }
        }
    }

    /// The players shown so far.
    fn shown(&self) -> usize {
        self.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Open { .. })).count()
    }

    /// The viewer closes the player showing `playback`'s file, as the app closes it: the file
    /// ended by quitting, the player closed, and what is being found is stopped, aimed at the
    /// playback it showed, which is all the app knows of.
    fn close(&self, playback: PlaybackId) {
        self.report(playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
        self.report(playback, PlayerReport::Closed);
        self.aim(playback, PlayerCommand::Stop);
    }

    fn shutdown(self) {
        self.engine.shutdown();
    }
}

/// How long each file runs, in seconds.
const RUNTIME: f64 = 120.0;

/// The finished fixture title with `streams` for every episode, in that order.
fn with_streams(streams: Vec<Stream>) -> ProviderRegistry {
    let mut provider = sources::provider();
    provider = provider.with_streams_for(sources::key(FINISHED), streams);
    ProviderRegistry::new(vec![provider.arc()])
}

fn stream(url: &str, kind: StreamKind) -> Stream {
    Stream { provider_id: sources::PROVIDER_ID.into(), ..Stream::new(url, kind) }
}

/// Whether `url` is the fixture image at `path` as the testing engine's fake AniList serves it:
/// on loopback, in place of the fixtures' own `img.example`, so an app shows real art.
fn served(url: Option<&str>, path: &str) -> bool {
    url.is_some_and(|url| {
        url.starts_with("http://127.0.0.1:") && url.ends_with(&format!("/art/{path}"))
    })
}

#[test]
fn the_player_is_told_what_to_play_and_reports_the_episode_to_its_end() {
    let mut app = App::fixtures("[playback]\nskip_opening = false\n");
    // The finale: nothing follows it, so the episode is the whole playback.
    app.play(FINISHED, "12");

    let (_, open) = app.op("the player shown", |op| matches!(op, PlayerOp::Open { .. }));
    assert_eq!(open, PlayerOp::Open { fullscreen: false, speed: None, volume: None });
    let (_, starting) =
        app.ui("the starting screen", |ui| matches!(ui, PlayerUi::Starting { .. }));
    // The title and its art, as its lookup found them, so the screen needs nothing more.
    let PlayerUi::Starting { id, title, episode, episode_title, cover_url, banner_url } =
        starting
    else {
        unreachable!()
    };
    assert_eq!(
        (id, title.as_str(), episode.as_str(), episode_title),
        (FINISHED, "Stardust Wayfarer", "12", None)
    );
    assert!(served(cover_url.as_deref(), "cover/900001-xl.jpg"), "{cover_url:?}");
    assert!(served(banner_url.as_deref(), "banner/900001.jpg"), "{banner_url:?}");
    let load = app.load();
    assert_eq!(load.url, sources::clip().url);
    assert_eq!(load.option("force-media-title"), Some("Stardust Wayfarer · Episode 12"));
    assert_eq!(load.option("alang"), Some("ja,jpn,jp"));

    // Read by mpv's own parsers: the Japanese audio picked in place of the dub, and the opening
    // offered from the file's own chapters, then cleared once past.
    let playback = load.playback;
    app.opened(playback, &[("Opening", 0.0), ("Part A", 30.0), ("Ending", 100.0)]);
    let (_, aid) = app.op(
        "the audio corrected",
        |op| matches!(op, PlayerOp::Set { property, .. } if property == "aid"),
    );
    assert_eq!(aid, PlayerOp::Set { property: "aid".into(), value: MpvValue::Int(2) });
    app.tick(playback, 0, 119);
    let (_, skip) = app.ui("a skip offered", |ui| matches!(ui, PlayerUi::Skip { .. }));
    assert_eq!(skip, PlayerUi::Skip { kind: SkipKind::Opening, to: 30.0 });
    app.ui("the skip withdrawn", |ui| *ui == PlayerUi::SkipCleared);
    app.report(playback, PlayerReport::Ended { reason: PlayerEnd::Eof });

    assert_eq!(app.ended(), Ending::Complete { next: None });
    assert!(app.told(&PlayerOp::Close), "the player closed: {:?}", app.ops);
    // History every ten seconds of the playhead, as for mpv: a row where the episode counts
    // as watched, 85% of the way in, the tens from there, and where it ended.
    let history = app.history("12");
    let tens: Vec<f64> = (1..=10).map(|n| f64::from(n) * 10.0).collect();
    assert_eq!(history[..10], tens[..], "{history:?}");
    assert_eq!(history[10..], [102.0, 112.0, 119.0], "{history:?}");
    app.shutdown();
}

#[test]
fn a_file_opens_where_the_viewer_left_it() {
    let app = App::fixtures("");
    {
        let store =
            anistream_store::Store::open(app.root.path().join("data/anistream.db")).unwrap();
        let left = anistream_store::WatchEvent {
            duration_secs: Some(1_440.0),
            watched_secs: 600.0,
            ..anistream_store::WatchEvent::new(FINISHED, "12", 600.0)
        };
        store.record_event(&left).unwrap();
    }
    let mut app = app;
    app.play(FINISHED, "12");
    let load = app.load();
    assert_eq!(load.option("start"), Some("600"));
    let (playback, resumed) =
        app.ui("where it resumed", |ui| matches!(ui, PlayerUi::Resumed { .. }));
    assert_eq!(resumed, PlayerUi::Resumed { position: 600.0 });
    assert_eq!(playback, load.playback);
    app.opened(playback, &[]);
    app.tick(playback, 600, 603);
    app.stop();
    app.shutdown();
}

#[test]
fn a_play_from_the_beginning_opens_at_the_start_and_is_left_where_it_stops() {
    let app = App::fixtures("[playback]\nauto_next = false\n");
    let store =
        anistream_store::Store::open(app.root.path().join("data/anistream.db")).unwrap();
    let mut left = anistream_store::WatchEvent {
        duration_secs: Some(1_440.0),
        watched_secs: 600.0,
        ..anistream_store::WatchEvent::new(FINISHED, "12", 600.0)
    };
    // An hour ago, so the new playback's history is plainly the later.
    left.at -= 3_600;
    store.record_event(&left).unwrap();
    let mut app = app;

    // "Play from the beginning", on an episode left at ten minutes.
    let from_start = Command::Play { id: FINISHED, episode: "12".into(), from_start: true };
    app.engine.send(from_start);
    let load = app.load();
    assert_eq!(load.option("start"), None, "{:?}", load.options);
    let playback = load.playback;
    app.opened(playback, &[]);
    app.tick(playback, 0, 45);
    app.stop();
    let resumed = app.read.iter().filter(|said| said.contains("Resumed")).count();
    assert_eq!(resumed, 0, "nothing says it resumed:\n{}", app.read.join("\n"));

    // The stored position followed the playback: the next plain play resumes where it stopped.
    assert_eq!(store.resume_position(FINISHED, "12").unwrap(), Some(45.0));
    app.play(FINISHED, "12");
    assert_eq!(app.load().option("start"), Some("45"));
    app.stop();
    app.shutdown();
}

#[test]
fn an_opening_skipped_by_itself_can_be_undone() {
    // Skipping the opening is on by default.
    let mut app = App::fixtures("");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[("Opening", 0.0), ("Part A", 30.0)]);
    app.tick(playback, 1, 1);

    let skip = |op: &PlayerOp| matches!(op, PlayerOp::Seek { absolute: true, .. });
    let (_, seek) = app.op("the opening skipped", skip);
    assert_eq!(seek, PlayerOp::Seek { seconds: 30.0, absolute: true });
    let (_, skipped) =
        app.ui("the undo offered", |ui| matches!(ui, PlayerUi::AutoSkipped { .. }));
    assert_eq!(skipped, PlayerUi::AutoSkipped { kind: SkipKind::Opening, undo_to: 1.0 });

    app.control(PlayerCommand::UndoSkip { to: 1.0 });
    let (_, back) = app.op("the skip undone", skip);
    assert_eq!(back, PlayerOp::Seek { seconds: 1.0, absolute: true });
    // Declined: the opening plays on without being skipped again.
    app.tick(playback, 2, 40);
    app.report(playback, PlayerReport::Ended { reason: PlayerEnd::Eof });
    app.ended();
    let seeks = app.ops.iter().filter(|(_, op)| skip(op)).count();
    assert_eq!(seeks, 2, "{:?}", app.ops);
    app.shutdown();
}

/// Play the first episode until its Up Next card names the second: the list of episodes is
/// found after the playback starts, as for a play from Continue.
fn up_next(app: &mut App) -> PlaybackId {
    app.play(FINISHED, "1");
    let playback = app.load().playback;
    app.opened(playback, &[("Opening", 0.0), ("Part A", 30.0), ("Ending", 100.0)]);
    // The playhead starts as the file plays, and with it the length the ending runs to.
    app.tick(playback, 0, 0);
    // Once both the list and the file's own ending are known, whichever came first.
    let (_, card) =
        app.ui("Up Next", |ui| matches!(ui, PlayerUi::UpNext { ending_at: Some(_), .. }));
    let PlayerUi::UpNext { next, auto, ending_at, countdown, still_watching_after } = card
    else {
        unreachable!()
    };
    assert_eq!((next.number.as_str(), auto, ending_at), ("2", true, Some(100.0)));
    // The app's countdown and when it asks whether anyone is still watching, both the engine's.
    assert_eq!((countdown, still_watching_after), (10, 3));
    assert_eq!(next.title.as_deref(), Some("A Town Without Night"));
    assert_eq!(next.thumbnail.as_deref(), Some("https://img.example/ep/900001/2.jpg"));
    playback
}

#[test]
fn play_next_hands_the_player_to_the_next_episode() {
    let mut app = App::fixtures("");
    let first = up_next(&mut app);
    app.tick(first, 31, 40);
    // Up Next's button, or the app's countdown reaching zero.
    app.control(PlayerCommand::PlayNext);

    let (_, starting) = app.ui(
        "the next episode starting",
        |ui| matches!(ui, PlayerUi::Starting { episode, .. } if episode == "2"),
    );
    let PlayerUi::Starting { id, banner_url, .. } = starting else { unreachable!() };
    assert_eq!(id, FINISHED);
    assert!(served(banner_url.as_deref(), "banner/900001.jpg"), "{banner_url:?}");
    app.op("the episode silenced", |op| *op == PlayerOp::Stop);
    let next = app.load();
    assert_ne!(next.playback, first, "the next episode is a playback of its own");
    assert_eq!(next.option("force-media-title"), Some("Stardust Wayfarer · Episode 2"));
    let opened = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Open { .. })).count();
    assert_eq!(opened, 1, "the same player");
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);
    app.stop();
    app.shutdown();
}

#[test]
fn a_late_command_for_the_episode_before_is_let_go() {
    let mut app = App::fixtures("");
    let first = up_next(&mut app);
    app.tick(first, 31, 40);
    app.aim(first, PlayerCommand::PlayNext);
    let next = app.load();
    assert_ne!(next.playback, first);
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);

    // A second Play Now from the card the first episode showed, crossing the roll-on: it was
    // for the first episode, and the second plays on.
    app.aim(first, PlayerCommand::PlayNext);
    app.aim(first, PlayerCommand::TogglePause);
    // Aimed at the episode playing, it lands.
    app.aim(next.playback, PlayerCommand::TogglePause);
    let (playback, _) = app.op("the pause", |op| matches!(op, PlayerOp::Cycle { .. }));
    assert_eq!(playback, next.playback);
    let pauses = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Cycle { .. })).count();
    assert_eq!(pauses, 1, "only the aimed one: {:?}", app.ops);
    assert!(!app.ops.iter().any(|(p, op)| *p == next.playback && *op == PlayerOp::Stop));
    app.stop();
    app.shutdown();
}

/// The engine with sources that are still loading, so a play waits for them for as long as the
/// test says: a lookup the viewer can walk away from. The chain, for the test to fill.
fn finding() -> (App, ProviderRegistry) {
    let chain = ProviderRegistry::new(Vec::new());
    chain.expect_arrivals();
    (App::start(chain.clone(), ""), chain)
}

/// Wait until the play under way says it is waiting for its sources.
fn waiting(app: &mut App) {
    app.until("the wait, said", |event| match event {
        Event::JobNote { note, .. } if note == "loading sources" => Some(()),
        _ => None,
    });
}

#[test]
fn a_stop_while_the_episode_is_found_shows_no_player() {
    // The viewer closing the starting screen of a first play, which has nothing to aim at:
    // read only once the file loaded, the Stop let the player come up first.
    let (mut app, chain) = finding();
    app.play(FINISHED, "12");
    waiting(&mut app);
    app.control(PlayerCommand::Stop);
    assert_eq!(app.ended_or_shown(), Ok(Ending::Stopped), "{}", app.read.join("\n"));

    // The sources arriving after it change nothing.
    chain.extend(vec![sources::provider().arc()]);
    chain.arrivals_done();
    app.settle();
    assert_eq!(app.shown(), 0, "{:?}", app.ops);
    assert!(
        !app.ops.iter().any(|(_, op)| matches!(op, PlayerOp::Load { .. })),
        "{:?}",
        app.ops
    );
    app.shutdown();
}

#[test]
fn a_stop_aimed_at_the_playback_before_still_stops_the_one_being_found() {
    // What an app aims at when it has only heard of the playback before: the one it last
    // played, whose player it closed.
    let (mut app, chain) = finding();
    chain.extend(vec![sources::provider().arc()]);
    chain.arrivals_done();
    app.play(FINISHED, "12");
    let before = app.load().playback;
    app.opened(before, &[]);
    app.stop();

    chain.expect_arrivals();
    app.play(FINISHED, "11");
    waiting(&mut app);
    app.aim(before, PlayerCommand::Stop);
    assert_eq!(app.ended_or_shown(), Ok(Ending::Stopped), "{}", app.read.join("\n"));
    chain.arrivals_done();
    app.settle();
    assert_eq!(app.shown(), 1, "only the first play's: {:?}", app.ops);
    app.shutdown();
}

/// The viewer closes the player on the next episode's starting screen, while it is found:
/// after Play Now, or a pick from the Episodes menu. `ask` is what the first episode's player
/// sent for the next.
fn closed_while_the_next_is_found(ask: PlayerCommand, next: &str) {
    let chain = sources::registry();
    let mut app = App::start(chain.clone(), "");
    let first = up_next(&mut app);
    app.tick(first, 31, 40);
    // The next episode's lookup takes as long as the test says.
    chain.expect_arrivals();
    app.aim(first, ask);
    app.until("the next episode staged", |event| match event {
        Event::PlaybackStaged { episode, .. } if episode == next => Some(()),
        _ => None,
    });
    waiting(&mut app);
    app.close(first);

    // The first episode ends as its player closed (or as stopped, the Stop reaching it first),
    // and the next as stopped, without a player of its own.
    for _ in 0..2 {
        let ending = app.ended_or_shown();
        assert!(
            matches!(ending, Ok(Ending::Closed | Ending::Stopped)),
            "{ending:?}: {}",
            app.read.join("\n")
        );
    }
    chain.arrivals_done();
    app.settle();
    assert_eq!(app.shown(), 1, "only the first episode's player: {:?}", app.ops);
    let loads: Vec<_> =
        app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Load { .. })).collect();
    assert_eq!(loads.len(), 1, "only the first episode's file: {loads:?}");
    app.shutdown();
}

#[test]
fn closing_the_player_after_play_next_shows_no_other() {
    closed_while_the_next_is_found(PlayerCommand::PlayNext, "2");
}

#[test]
fn closing_the_player_after_an_episode_is_picked_shows_no_other() {
    closed_while_the_next_is_found(
        PlayerCommand::Choose(PlayerChoice::Episode("3".into())),
        "3",
    );
}

#[test]
fn a_viewer_closing_the_player_ends_the_playback_as_closed() {
    // The first of the two ways a hosted player closes: nobody told it to.
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[]);
    app.tick(playback, 0, 5);
    app.report(playback, PlayerReport::Ended { reason: PlayerEnd::Quit });
    app.report(playback, PlayerReport::Closed);
    assert_eq!(app.ended(), Ending::Closed);
    assert!(!app.told(&PlayerOp::Quit), "nothing told it to quit: {:?}", app.ops);
    app.shutdown();
}

#[test]
fn the_engine_stopping_the_player_ends_the_playback_as_stopped() {
    // The second: the front end asked, the player was told to quit, and said so.
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[]);
    app.tick(playback, 0, 5);
    app.stop();
    assert!(app.told(&PlayerOp::Quit));
    app.shutdown();
}

#[test]
fn a_track_picked_in_the_player_is_selected_and_kept_for_the_series() {
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[]);
    app.op(
        "the audio corrected",
        |op| matches!(op, PlayerOp::Set { property, .. } if property == "aid"),
    );
    app.tick(playback, 0, 1);

    let pick = PlayerCommand::ChooseTrack { kind: TrackKind::Subtitle, id: None };
    app.aim(playback, pick);
    let off = PlayerOp::Set { property: "sid".into(), value: MpvValue::String("no".into()) };
    app.op("the subtitles off", |op| *op == off);
    app.aim(playback, PlayerCommand::SubtitleDelay { seconds: 0.5 });
    let (_, delay) = app.op(
        "the delay",
        |op| matches!(op, PlayerOp::Set { property, .. } if property == "sub-delay"),
    );
    assert_eq!(
        delay,
        PlayerOp::Set { property: "sub-delay".into(), value: MpvValue::Double(0.5) }
    );
    app.stop();

    // Kept for the series: the subtitles off at once, without the player having said a word
    // of it, and the delay once the episode left it where it was.
    let store =
        anistream_store::Store::open(app.root.path().join("data/anistream.db")).unwrap();
    let deadline = Instant::now() + PATIENCE;
    let memory = loop {
        let memory = store.track_memory(FINISHED, Translation::Sub).unwrap();
        if memory.as_ref().is_some_and(|m| m.sub_delay.is_some()) || Instant::now() > deadline {
            break memory.expect("kept for the series");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let subtitles = memory.subtitles.expect("a subtitle choice");
    assert_eq!(subtitles.choice, anistream_core::track_memory::SubtitleChoice::Off);
    assert_eq!(memory.sub_delay, Some(0.5));
    app.shutdown();
}

#[test]
fn full_screen_is_the_hosts_window_and_never_an_mpv_property() {
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[]);
    app.aim(playback, PlayerCommand::Fullscreen);
    app.op("full screen", |op| *op == PlayerOp::ToggleFullscreen);
    assert!(
        !app.ops.iter().any(|(_, op)| matches!(op, PlayerOp::Cycle { property } | PlayerOp::Set { property, .. } if property == "fullscreen")),
        "{:?}",
        app.ops
    );
    app.stop();
    app.shutdown();
}

#[test]
fn stats_for_nerds_are_read_while_open_worded_by_the_engine_and_copied_without_an_address() {
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    // The TV's summary is said with the panel shut, from what the player observes anyway, with
    // the decoding that runs rather than the one asked for.
    app.report(playback, PlayerReport::FileLoaded);
    app.property(playback, "hwdec-current", json!("no"));
    app.property(
        playback,
        "video-params",
        json!({"w": 1280, "h": 720, "dw": 1280, "dh": 720, "pixelformat": "yuv420p"}),
    );
    app.property(playback, "current-tracks/video", json!({"codec": "h264"}));
    app.tick(playback, 0, 2);
    // The engine says its summary after each figure it learns, so the first line may lack the
    // codec the last property brings: wait for the line that has it.
    let summary = app.until("the summary", |event| match event {
        Event::PlayerStats { stats, .. }
            if stats.summary.first().is_some_and(|line| line.contains("H.264")) =>
        {
            Some(stats.clone())
        }
        _ => None,
    });
    assert!(summary.rows.is_empty(), "no rows with the panel shut");
    assert_eq!(summary.summary[0], "720p · H.264 · software decoding");
    assert!(!app.ops.iter().any(|(_, op)| matches!(op, PlayerOp::Sample { .. })));

    // Open: the player is asked for its figures, and the rows follow from its answers.
    app.aim(playback, PlayerCommand::Stats(true));
    let (_, sample) =
        app.op("the figures asked for", |op| matches!(op, PlayerOp::Sample { .. }));
    let PlayerOp::Sample { properties } = sample else { unreachable!() };
    assert!(properties.iter().any(|p| p == "cache-speed"), "{properties:?}");
    app.property(playback, "container-fps", json!(23.976));
    app.property(playback, "avsync", json!(0.25));
    app.property(playback, "demuxer-cache-duration", json!(30.0));
    let rows = app.until("the rows", |event| match event {
        Event::PlayerStats { stats, .. }
            if stats.rows.iter().any(|r| r.label == "A/V sync") =>
        {
            Some(stats.rows.clone())
        }
        _ => None,
    });
    let row = |label: &str| rows.iter().find(|r| r.label == label).cloned();
    assert_eq!(row("Video").unwrap().value, "1280 × 720 · H.264 · 23.976 fps");
    assert_eq!(row("Decoding").unwrap().value, "Software");
    assert_eq!(row("HDR"), None, "an SDR picture");
    let sync = row("A/V sync").unwrap();
    assert!(sync.warn && sync.value == "+0.250 s", "{sync:?}");
    // Asked again a second on, and not before.
    let asked = Instant::now();
    app.op("the figures asked for again", |op| matches!(op, PlayerOp::Sample { .. }));
    assert!(asked.elapsed() > Duration::from_millis(400), "{:?}", asked.elapsed());

    app.aim(
        playback,
        PlayerCommand::CopyStats {
            about: anistream_model::hosted::StatsAbout {
                app_version: "2.4.0".into(),
                system: "macOS 26.1".into(),
                device: "Mac15,3".into(),
            },
        },
    );
    let text = app.until("the copy", |event| match event {
        Event::Clipboard { text } => Some(text.clone()),
        _ => None,
    });
    assert!(text.starts_with("Stats for nerds\nVideo: 1280 × 720"), "{text}");
    assert!(
        text.ends_with("App: anistream 2.4.0\nSystem: macOS 26.1\nDevice: Mac15,3"),
        "{text}"
    );
    assert!(!text.contains("://") && !text.contains("127.0.0.1"), "no address: {text}");

    // Shut: nothing more is asked of the player.
    app.aim(playback, PlayerCommand::Stats(false));
    app.until("the panel shut", |event| {
        matches!(event, Event::PlayerStats { stats, .. } if stats.rows.is_empty()).then_some(())
    });
    let before = app.ops.len();
    app.settle();
    app.settle();
    assert!(
        !app.ops[before..].iter().any(|(_, op)| matches!(op, PlayerOp::Sample { .. })),
        "{:?}",
        &app.ops[before..]
    );
    app.stop();
    app.shutdown();
}

#[test]
fn a_reload_opens_the_same_file_again_where_it_plays() {
    // What follows a loopback server bound again after the background.
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let first = app.load();
    app.opened(first.playback, &[]);
    app.tick(first.playback, 0, 30);
    // Once the playback has read its playhead there: reports and commands come by two roads.
    app.until("the playhead at 30", |event| match event {
        Event::Playback { position, .. } if *position >= 30.0 => Some(()),
        _ => None,
    });
    app.control(PlayerCommand::Reload);
    let again = app.load();
    assert_eq!((again.playback, again.url.as_str()), (first.playback, first.url.as_str()));
    assert_eq!(again.option("start"), Some("30"));
    app.stop();
    app.shutdown();
}

#[test]
fn coming_back_with_nothing_taken_opens_nothing_again() {
    let mut app = App::fixtures("[playback]\nauto_next = false\n");
    app.play(FINISHED, "12");
    let playback = app.load().playback;
    app.opened(playback, &[]);
    app.tick(playback, 0, 5);
    let phase = |phase| Command::AppPhase { phase };
    app.engine.send(phase(AppState::Background { playing: true }));
    app.engine.send(Command::NetworkPath {
        cellular: true,
        constrained: false,
        offline: false,
    });
    app.engine.send(phase(AppState::Foreground));
    // Every server still answers: the playback carries on as it was.
    app.control(PlayerCommand::TogglePause);
    app.op("the pause", |op| matches!(op, PlayerOp::Cycle { .. }));
    let loads = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Load { .. })).count();
    assert_eq!(loads, 1, "{:?}", app.ops);
    app.stop();
    app.shutdown();
}

#[test]
fn a_cancelled_up_next_ends_with_the_episode() {
    let mut app = App::fixtures("");
    let playback = up_next(&mut app);
    app.control(PlayerCommand::CancelNext);
    // Controls are read in order: once this one is, so is the cancel before it.
    app.control(PlayerCommand::TogglePause);
    app.op("the pause", |op| matches!(op, PlayerOp::Cycle { property } if property == "pause"));
    app.tick(playback, 31, 119);
    app.report(playback, PlayerReport::Ended { reason: PlayerEnd::Eof });

    assert_eq!(app.ended(), Ending::Complete { next: None });
    assert!(app.told(&PlayerOp::Close), "the player closed: {:?}", app.ops);
    let loads = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Load { .. })).count();
    assert_eq!(loads, 1, "nothing rolls on: {:?}", app.ops);
    app.shutdown();
}

#[test]
fn the_next_episode_is_found_ahead_and_rolls_on_in_the_same_player() {
    let mut app = App::fixtures("");
    let first = up_next(&mut app);
    // Into the ending: the next episode's streams are looked for ahead.
    app.tick(first, 31, 101);
    app.until("the next episode found ahead", |event| match event {
        Event::PrefetchNext { episode, .. } if episode == "2" => Some(()),
        _ => None,
    });
    app.tick(first, 102, 119);
    app.report(first, PlayerReport::Ended { reason: PlayerEnd::Eof });

    assert_eq!(app.ended(), Ending::Complete { next: Some("2".into()) });
    let next = app.load();
    assert_ne!(next.playback, first);
    assert_eq!(next.option("force-media-title"), Some("Stardust Wayfarer · Episode 2"));
    let opened = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Open { .. })).count();
    assert_eq!(opened, 1, "the same player");
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);
    app.stop();
    app.shutdown();
}

/// Episode `playback` of a binge played to its end, as libmpv reports it: the file loaded with
/// its chapters, the Up Next card shown once the list is in, the playhead to the last second
/// and the end of the file. Returns the next episode's playback, which the ending rolls on to.
fn played_through(app: &mut App, playback: PlaybackId, next: &str) -> PlaybackId {
    app.opened(playback, &[("Opening", 0.0), ("Part A", 30.0), ("Ending", 100.0)]);
    app.tick(playback, 0, 0);
    app.ui("Up Next", |ui| matches!(ui, PlayerUi::UpNext { ending_at: Some(_), .. }));
    app.tick(playback, 1, 119);
    app.report(playback, PlayerReport::Ended { reason: PlayerEnd::Eof });
    assert_eq!(app.ended(), Ending::Complete { next: Some(next.into()) });
    let rolled = app.load();
    assert_ne!(rolled.playback, playback, "{next} is a playback of its own");
    rolled.playback
}

#[test]
fn a_binge_marks_every_episode_watched_on_a_table_loaded_before_another_titles() {
    let mut app = App::fixtures("");
    // Five of the airing title's twelve episodes are out. The app keeps both tables.
    airing_open_then_another(&mut app);
    assert_eq!(app.titles.tables[&AIRING].len(), 5);

    // Episodes 1 to 3 in one sitting, each rolling on to the next.
    app.play(AIRING, "1");
    let mut playback = app.load().playback;
    for next in ["2", "3", "4"] {
        playback = played_through(&mut app, playback, next);
    }
    app.opened(playback, &[]);
    app.tick(playback, 0, 2);
    app.stop();
    app.settle();

    assert_eq!(app.titles.watched(AIRING), ["1", "2", "3"], "each episode played is watched");
    assert_eq!(app.titles.progress[&AIRING], Some(TitleProgress { done: 3, next: 4 }));
    assert!(app.titles.watched(FINISHED).is_empty(), "the other title watched nothing");

    // Continue offers episode 4, which is out: the next to air is 6.
    app.engine.send(Command::LoadContinue);
    // A list sent before the last change may still be on its way: wait for the row that has it.
    let row = app.until("Continue", |event| match event {
        Event::Content {
            view: anistream_model::catalog::ContentFor::Continue,
            content: anistream_model::catalog::ContentList::Entries(entries),
        } => entries
            .iter()
            .find(|entry| {
                entry.id == AIRING && entry.progress == Some(TitleProgress { done: 3, next: 4 })
            })
            .cloned(),
        _ => None,
    });
    assert_eq!(row.progress, Some(TitleProgress { done: 3, next: 4 }));
    assert_eq!(row.next_episode, Some(6), "4 has aired, so Continue is not caught up");
    app.shutdown();
}

/// The airing title's page and table open, then another title's table loaded, as Continue or
/// Downloads loads one: the airing title's table is no longer the one the engine holds.
fn airing_open_then_another(app: &mut App) {
    app.engine.send(Command::LoadDetail(AIRING));
    app.until("the page", |event| matches!(event, Event::Detail(_)).then_some(()));
    app.engine.send(Command::LoadEpisodes(AIRING));
    app.until("its table", |event| {
        matches!(event, Event::Episodes { id, .. } if *id == AIRING).then_some(())
    });
    app.engine.send(Command::LoadEpisodes(FINISHED));
    app.until("the other table", |event| {
        matches!(event, Event::Episodes { id, .. } if *id == FINISHED).then_some(())
    });
}

/// The answer to a mark of the airing title's `episodes`: episodes done and the one Continue
/// offers, and the rows that read as watched.
fn mark(app: &mut App, episodes: &[&str], watched: bool) -> ((u32, u32), Vec<String>) {
    let episodes = episodes.iter().map(|e| (*e).to_owned()).collect();
    app.engine.send(Command::SetWatched { id: AIRING, episodes, watched });
    let progress = app.until("the mark's answer", |event| match event {
        Event::Progress { id, progress, .. } if *id == AIRING => Some(*progress),
        _ => None,
    });
    let progress = progress.map(|p| (p.done, p.next)).expect("a standing");
    (progress, app.titles.watched(AIRING).into_iter().map(str::to_owned).collect())
}

#[test]
fn a_mark_out_of_order_or_undone_moves_the_count_with_the_rows() {
    let mut app = App::fixtures("");
    airing_open_then_another(&mut app);
    assert_eq!(mark(&mut app, &["3"], true), ((1, 4), vec!["3".to_owned()]));
    // Before the episode Continue offers: counted all the same, and Continue goes back to 2.
    assert_eq!(mark(&mut app, &["1"], true), ((2, 2), vec!["1".to_owned(), "3".to_owned()]));
    assert_eq!(mark(&mut app, &["3"], false), ((1, 2), vec!["1".to_owned()]));

    // As the store has it: the count and Continue read from history agree.
    app.engine.send(Command::LoadContinue);
    // A list sent before the last change may still be on its way: wait for the row that has it.
    let row = app.until("Continue", |event| match event {
        Event::Content {
            view: anistream_model::catalog::ContentFor::Continue,
            content: anistream_model::catalog::ContentList::Entries(entries),
        } => entries
            .iter()
            .find(|entry| {
                entry.id == AIRING && entry.progress == Some(TitleProgress { done: 1, next: 2 })
            })
            .cloned(),
        _ => None,
    });
    assert_eq!(row.progress, Some(TitleProgress { done: 1, next: 2 }));
    app.shutdown();
}

#[test]
fn a_mark_undone_leaves_the_episode_where_it_was_left() {
    // Ten minutes into episode 2 of the airing title.
    let app = App::fixtures("");
    {
        let store =
            anistream_store::Store::open(app.root.path().join("data/anistream.db")).unwrap();
        let left = anistream_store::WatchEvent {
            duration_secs: Some(1_440.0),
            watched_secs: 600.0,
            provider_id: Some(sources::PROVIDER_ID.to_owned()),
            ..anistream_store::WatchEvent::new(AIRING, "2", 600.0)
        };
        store.record_event(&left).unwrap();
    }
    let mut app = app;
    app.engine.send(Command::LoadDetail(AIRING));
    app.until("the page", |event| matches!(event, Event::Detail(_)).then_some(()));
    app.engine.send(Command::LoadEpisodes(AIRING));
    app.until("its table", |event| {
        matches!(event, Event::Episodes { id, .. } if *id == AIRING).then_some(())
    });
    let meter = |app: &App| {
        let rows = &app.titles.tables[&AIRING];
        rows.iter().find(|row| row.number == "2").map(|row| (row.watched, row.completed))
    };
    let before = meter(&app);
    assert_eq!(before, Some((600.0 / 1_440.0, false)), "partway through");

    // Marked watched, then the Undo: the mark taken back.
    let standing = |app: &mut App, watched| {
        let episodes = vec!["2".to_owned()];
        app.engine.send(Command::SetWatched { id: AIRING, episodes, watched });
        app.until("the mark's answer", |event| match event {
            Event::Progress { id, progress, resume, .. } if *id == AIRING => {
                Some((progress.map(|p| (p.done, p.next)), *resume))
            }
            _ => None,
        })
    };
    assert_eq!(standing(&mut app, true), (Some((1, 3)), None));
    assert_eq!(meter(&app), Some((1.0, true)));
    let resume = Some(ResumePoint { position: 600.0, fraction: Some(600.0 / 1_440.0) });
    assert_eq!(standing(&mut app, false), (Some((0, 2)), resume), "Continue resumes it");
    assert_eq!(meter(&app), before, "the row as it was");

    // And a play of it carries on from there.
    app.play(AIRING, "2");
    assert_eq!(app.load().option("start"), Some("600"));
    app.shutdown();
}

#[test]
fn a_stall_offers_another_stream_or_to_wait() {
    let first = "https://cdn.example/stardust-wayfarer/12/a.mp4";
    let second = "https://cdn.example/stardust-wayfarer/12/b.mp4";
    let mut app = App::start(
        with_streams(vec![stream(first, StreamKind::Mp4), stream(second, StreamKind::Mp4)]),
        "",
    );
    app.play(FINISHED, "12");
    let load = app.load();
    assert_eq!(load.url, first);
    let playback = load.playback;
    app.opened(playback, &[]);
    app.tick(playback, 0, 5);
    // Held waiting for data, mid-episode: after eight seconds, the prompt.
    app.property(playback, "paused-for-cache", json!(true));
    let (_, stall) = app.ui("the stall prompt", |ui| matches!(ui, PlayerUi::Stall { .. }));
    let PlayerUi::Stall { seconds } = stall else { unreachable!() };
    assert!(seconds >= 8, "{seconds}");

    app.control(PlayerCommand::StallWait);
    app.ui("the prompt down", |ui| *ui == PlayerUi::StallCleared);
    app.control(PlayerCommand::StallSwitch);
    let switched = app.load();
    assert_eq!(switched.url, second);
    assert_eq!(switched.option("start"), Some("5"), "where the first left off");
    app.opened(switched.playback, &[]);
    app.tick(switched.playback, 5, 7);
    app.stop();
    app.shutdown();
}

#[test]
fn a_stream_that_shows_nothing_falls_over_to_the_next_in_the_same_player() {
    let first = "https://cdn.example/stardust-wayfarer/12/silent.mp4";
    let second = "https://cdn.example/stardust-wayfarer/12/b.mp4";
    let mut app = App::start(
        with_streams(vec![stream(first, StreamKind::Mp4), stream(second, StreamKind::Mp4)]),
        "",
    );
    app.play(FINISHED, "12");
    let silent = app.load();
    assert_eq!(silent.url, first);
    // Loaded, then over without a single frame.
    app.report(silent.playback, PlayerReport::FileLoaded);
    app.report(silent.playback, PlayerReport::Ended { reason: PlayerEnd::Error });

    let next = app.load();
    assert_eq!(next.url, second);
    let opened = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Open { .. })).count();
    assert_eq!(opened, 1, "the same player");
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 119);
    app.report(next.playback, PlayerReport::Ended { reason: PlayerEnd::Eof });
    assert_eq!(app.ended(), Ending::Complete { next: None });
    app.shutdown();
}

/// A stream that does not load at all, as libmpv says when the first request fails: its file
/// ends in an error before it has loaded. The next stream is loaded at once, with no wait.
fn a_stream_that_fails_to_load_falls_over_at_once(kind: StreamKind, upstream: &str) {
    let first = format!("{upstream}/stardust-wayfarer/12/first");
    let second = format!("{upstream}/stardust-wayfarer/12/second");
    let mut app =
        App::start(with_streams(vec![stream(&first, kind), stream(&second, kind)]), "");
    app.play(FINISHED, "12");
    let failed = app.load();
    // Never loaded: no FileLoaded, only the end, as libmpv says "loading failed".
    let ended = Instant::now();
    app.report(failed.playback, PlayerReport::Ended { reason: PlayerEnd::Error });
    let next = app.load();
    let waited = ended.elapsed();
    assert!(waited < Duration::from_secs(1), "the next stream waited {waited:?}");
    assert_ne!(next.url, failed.url, "the next stream, not the same one again");
    let opened = app.ops.iter().filter(|(_, op)| matches!(op, PlayerOp::Open { .. })).count();
    assert_eq!(opened, 1, "the same player");
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);
    app.stop();
    app.shutdown();
}

/// An upstream on loopback that answers every request `502 Bad Gateway`, as a dead CDN does.
fn bad_gateway() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for mut connection in listener.incoming().flatten() {
            let mut request = [0_u8; 4096];
            let _ = connection.read(&mut request);
            let _ = connection.write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    format!("http://{address}")
}

/// A subtitle host on loopback that takes every connection and answers after `delay`, or
/// never when there is none.
fn subtitle_host(delay: Option<Duration>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for mut connection in listener.incoming().flatten() {
            thread::spawn(move || {
                let mut request = [0_u8; 4096];
                let _ = connection.read(&mut request);
                let Some(delay) = delay else {
                    // Held open and never answered, until the test is over.
                    thread::sleep(Duration::from_secs(600));
                    return;
                };
                thread::sleep(delay);
                let body = "WEBVTT\n\n00:01.000 --> 00:02.000\nLate.\n";
                let _ = write!(
                    connection,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    format!("http://{address}")
}

/// A stream at `url` listing a dozen subtitle languages, all at `host`.
fn with_subtitles(url: &str, host: &str) -> Stream {
    let subtitles = (0..12)
        .map(|n| Subtitle {
            language: format!("l{n}"),
            url: format!("{host}/sub-{n}.vtt"),
            hard: false,
            format: Some("vtt".into()),
        })
        .collect();
    Stream { subtitles, ..stream(url, StreamKind::Mp4) }
}

#[test]
fn the_next_stream_opens_without_waiting_out_a_subtitle_host_that_never_answers() {
    // What a minute between streams was: every round of a dozen subtitle fetches timing out
    // before the next stream's file was opened.
    let host = subtitle_host(None);
    let first = "https://cdn.example/stardust-wayfarer/12/first.mp4";
    let second = "https://cdn.example/stardust-wayfarer/12/second.mp4";
    let mut app = App::start(
        with_streams(vec![stream(first, StreamKind::Mp4), with_subtitles(second, &host)]),
        "",
    );
    app.play(FINISHED, "12");
    let failed = app.load();
    let ended = Instant::now();
    app.report(failed.playback, PlayerReport::Ended { reason: PlayerEnd::Error });
    let next = app.load();
    let waited = ended.elapsed();
    assert_eq!(next.url, second);
    assert!(waited < Duration::from_secs(3), "the next stream waited {waited:?}");
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);
    app.stop();
    app.shutdown();
}

#[test]
fn a_subtitle_that_arrives_after_the_file_opened_is_added_to_it() {
    let host = subtitle_host(Some(Duration::from_secs(3)));
    let url = "https://cdn.example/stardust-wayfarer/12/a.mp4";
    let mut app = App::start(with_streams(vec![with_subtitles(url, &host)]), "");
    app.play(FINISHED, "12");
    let started = Instant::now();
    let load = app.load();
    assert!(started.elapsed() < Duration::from_secs(3), "the file did not wait for them");
    app.opened(load.playback, &[]);
    let (_, added) = app.op("a late subtitle", |op| matches!(op, PlayerOp::AddSubtitle { .. }));
    let PlayerOp::AddSubtitle { url, .. } = added else { unreachable!() };
    assert!(!url.starts_with("http"), "its local copy: {url}");
    app.stop();
    app.shutdown();
}

#[test]
fn a_mended_stream_whose_upstream_answers_502_falls_over_at_once() {
    a_stream_that_fails_to_load_falls_over_at_once(StreamKind::Hls, &bad_gateway());
}

#[test]
fn a_file_that_will_not_load_falls_over_at_once() {
    a_stream_that_fails_to_load_falls_over_at_once(StreamKind::Mp4, "https://cdn.example");
}

/// Serve one WebVTT track on loopback, for as long as the test runs, and say where.
fn subtitle_server() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    thread::spawn(move || {
        for connection in listener.incoming().flatten() {
            let mut connection = connection;
            let mut request = [0_u8; 4_096];
            let _ = connection.read(&mut request);
            let body = "WEBVTT\n\n00:00.000 --> 00:02.000\nThe star road\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/vtt\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = connection.write_all(response.as_bytes());
        }
    });
    format!("http://{address}/12.en.vtt")
}

#[test]
fn a_web_stream_comes_mended_and_its_subtitles_fetched_ahead() {
    let subtitles = subtitle_server();
    let hls = Stream {
        subtitles: vec![Subtitle {
            language: "en".into(),
            url: subtitles.clone(),
            hard: false,
            format: Some("vtt".into()),
        }],
        ..stream("https://cdn.example/stardust-wayfarer/12/master.m3u8", StreamKind::Hls)
    };
    let mut app = App::start(with_streams(vec![hls]), "");
    app.play(FINISHED, "12");

    // The thumbnailer's own entry to the mender, then the mender's address for the stream.
    let (_, previews) =
        app.ui("the previews' address", |ui| matches!(ui, PlayerUi::Thumbnails { .. }));
    let PlayerUi::Thumbnails { url: Some(previews) } = previews else {
        panic!("a mended stream has its previews read through the mender: {previews:?}")
    };
    assert!(previews.starts_with("http://127.0.0.1:"), "{previews}");
    let load = app.load();
    assert!(load.url.starts_with("http://127.0.0.1:"), "on loopback: {}", load.url);
    assert_ne!(load.url, previews);

    // Once loaded, the track fetched ahead, from where the engine wrote it.
    app.report(load.playback, PlayerReport::FileLoaded);
    let (_, added) = app.op("the subtitles", |op| matches!(op, PlayerOp::AddSubtitle { .. }));
    let PlayerOp::AddSubtitle { url, language, .. } = added else { unreachable!() };
    assert_eq!(language.as_deref(), Some("en"));
    assert!(!url.starts_with("http"), "a local file, not the address: {url}");
    assert!(std::path::Path::new(&url).is_file(), "written out: {url}");
    assert!(std::fs::read_to_string(&url).unwrap().contains("The star road"));
    app.tick(load.playback, 0, 2);
    app.stop();
    app.shutdown();
}

#[test]
fn an_episode_yet_to_air_fails_at_once_without_asking_a_source() {
    let provider = sources::provider();
    let asked = provider.call_count();
    let mut app = App::start(ProviderRegistry::new(vec![provider.arc()]), "");
    // Its table open, as a play is asked for from: what aired, which the player's menu shows.
    app.engine.send(Command::LoadEpisodes(AIRING));
    app.until("the table", |event| matches!(event, Event::Episodes { .. }).then_some(()));
    let listed = asked.load(std::sync::atomic::Ordering::SeqCst);
    // The airing fixture's next episode is 6, two days out, and 7 is on its schedule after it.
    // Past the schedule, the next one is what can be said.
    let picked = Command::PlaySource {
        id: AIRING,
        episode: "7".into(),
        provider_id: sources::PROVIDER_ID.into(),
        source_id: "a".into(),
    };
    for (play, said) in [
        (Command::play(AIRING, "6"), "episode 6 airs "),
        (picked, "episode 7 airs "),
        (Command::play(AIRING, "99"), "episode 99 hasn't aired yet; episode 6 airs "),
    ] {
        app.engine.send(play);
        let refusal = app.until("the refusal", |event| match event {
            Event::Toast(notice) if notice.text.contains("airs") => Some(notice.text.clone()),
            _ => None,
        });
        assert!(refusal.starts_with(said), "{refusal}");
        assert_eq!(app.ended(), Ending::Failed);
    }
    let looked_up = asked.load(std::sync::atomic::Ordering::SeqCst) - listed;
    assert_eq!(looked_up, 0, "no source was asked for either");
    assert!(!app.ops.iter().any(|(_, op)| matches!(op, PlayerOp::Load { .. })));
    app.shutdown();
}

#[test]
fn a_play_while_sources_are_still_loading_waits_for_them() {
    // Plugins compiling at launch: none of the chain is in it yet.
    let chain = ProviderRegistry::new(Vec::new());
    chain.expect_arrivals();
    let mut app = App::start(chain.clone(), "");
    app.play(FINISHED, "1");
    app.until("the wait, said", |event| match event {
        Event::JobNote { note, .. } if note == "loading sources" => Some(()),
        _ => None,
    });

    // They join, and the play goes on with them rather than failing for want of a source.
    chain.extend(vec![sources::provider().arc()]);
    chain.arrivals_done();
    let load = app.load();
    app.report(load.playback, PlayerReport::FileLoaded);
    app.stop();
    app.shutdown();
}

/// A stream of episode 12 from a server that keeps failing, and one from a server that plays.
const FAILING: &str = "https://cdn-a.example/stardust-wayfarer/12.mp4";
const HEALTHY: &str = "https://cdn-b.example/stardust-wayfarer/12.mp4";

/// The two, as the source lists them: the failing one first.
fn two_servers() -> Vec<Stream> {
    vec![stream(FAILING, StreamKind::Mp4), stream(HEALTHY, StreamKind::Mp4)]
}

/// Play episode 12 with its first stream failing before a frame and the next one playing.
fn fails_then_plays(app: &mut App) {
    app.play(FINISHED, "12");
    let first = app.load();
    assert_eq!(first.url, FAILING);
    app.report(first.playback, PlayerReport::FileLoaded);
    app.report(first.playback, PlayerReport::Ended { reason: PlayerEnd::Error });
    let next = app.load();
    assert_eq!(next.url, HEALTHY);
    app.opened(next.playback, &[]);
    app.tick(next.playback, 0, 2);
    app.stop();
}

#[test]
fn a_server_that_keeps_failing_is_tried_after_the_one_that_plays() {
    let mut app = App::start(with_streams(two_servers()), "");
    fails_then_plays(&mut app);
    fails_then_plays(&mut app);

    // Failed twice running: the third play tries the server that has been playing first, and
    // the Sources menu says why the other is second.
    app.play(FINISHED, "12");
    let (_, menus) = app.ui("the menus", |ui| matches!(ui, PlayerUi::Menus { .. }));
    let PlayerUi::Menus { sources, .. } = menus else { unreachable!() };
    let rows: Vec<(Option<&str>, Option<&str>)> =
        sources.iter().map(|s| (s.detail.as_deref(), s.health.as_deref())).collect();
    assert_eq!(
        rows,
        [
            (Some("direct · cdn-b"), None),
            (Some("direct · cdn-a"), Some("failed the last 2 times · just now")),
        ]
    );
    let first = app.load();
    assert_eq!(first.url, HEALTHY);
    app.opened(first.playback, &[]);
    app.tick(first.playback, 0, 2);

    // Picked from the menu, the failing server plays all the same, and playing gives it its
    // place back.
    app.control(PlayerCommand::Choose(PlayerChoice::Source("1".into())));
    let picked = app.load();
    assert_eq!(picked.url, FAILING);
    app.opened(picked.playback, &[]);
    app.tick(picked.playback, 2, 4);
    app.stop();

    app.play(FINISHED, "12");
    assert_eq!(app.load().url, FAILING, "one success restores it");
    app.stop();
    app.shutdown();
}

#[test]
fn a_release_picked_by_hand_plays_its_servers_as_it_lists_them() {
    let provider = sources::provider()
        .with_streams_for(sources::key(FINISHED), two_servers())
        .with_source_streams("release", two_servers());
    let mut app = App::start(ProviderRegistry::new(vec![provider.arc()]), "");
    fails_then_plays(&mut app);
    fails_then_plays(&mut app);

    app.engine.send(Command::PlaySource {
        id: FINISHED,
        episode: "12".into(),
        provider_id: sources::PROVIDER_ID.into(),
        source_id: "release".into(),
    });
    assert_eq!(app.load().url, FAILING, "never reordered, however its servers have done");
    app.stop();
    app.shutdown();
}
