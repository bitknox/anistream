//! Home on each of the Fixtures scenes the apps take their Home shots from: what the engine
//! sends there is the board's state the scene is named for.
#![cfg(feature = "testing")]

use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use anistream_core::ids::AnilistId;
use anistream_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use anistream_model::{
    command::Command,
    event::Event,
    home::{HomeAction, HomeView, Playable, SectionKind, SpotlightCase},
};

/// How long a test waits for what it is waiting on before failing: everything is on loopback.
const PATIENCE: Duration = Duration::from_secs(40);

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

/// The engine on `scene` with Home's world in it, in a home of its own that lasts as long as
/// the engine.
fn started(scene: FfiScene) -> (Arc<FfiEngine>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_home_scene_on(
        dirs,
        Arc::new(Quiet),
        scene,
        anistream_engine::clock::Clock::default(),
    )
    .expect("the engine starts");
    (engine, root)
}

/// The engine's events until one `until` accepts, polled on a thread of its own.
fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            let found = batch.iter().any(&until);
            let empty = batch.is_empty();
            read.extend(batch);
            if found || empty {
                let _ = tx.send((read, found));
                break;
            }
        }
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

fn homes(events: Vec<Event>) -> Vec<HomeView> {
    events
        .into_iter()
        .filter_map(|event| match event {
            Event::Home(home) => Some(*home),
            _ => None,
        })
        .collect()
}

/// Every Home sent until one has nothing left on its way.
fn settled(engine: &Arc<FfiEngine>) -> Vec<HomeView> {
    homes(read_until(engine, |e| matches!(e, Event::Home(h) if h.end.is_some())))
}

/// Home asked for, and every send until it settles.
fn load(engine: &Arc<FfiEngine>) -> Vec<HomeView> {
    engine.send(Command::LoadHome { refresh: false });
    settled(engine)
}

fn case(home: &HomeView) -> Option<(SpotlightCase, AnilistId)> {
    home.spotlight.as_ref().map(|s| (s.case, s.entry.id))
}

/// The kinds of Home's sections in order, leaving out Starting, which shows only in the two
/// weeks before a season.
fn kinds(home: &HomeView) -> Vec<SectionKind> {
    home.sections.iter().map(|s| s.kind).filter(|k| *k != SectionKind::Starting).collect()
}

/// The Home world's title `n`: see `anistream_testkit::home_world`.
fn world(n: u32) -> AnilistId {
    AnilistId::new(950_000 + n)
}

#[test]
fn watching_fills_every_shelf_the_board_draws_from_the_first_load() {
    let (engine, _home) = started(FfiScene::Watching);
    let home = load(&engine).pop().unwrap();
    assert_eq!(case(&home).map(|c| c.0), Some(SpotlightCase::Continuing), "{home:?}");
    assert_eq!(
        kinds(&home),
        [
            SectionKind::Continue,
            SectionKind::ThisWeek,
            SectionKind::UpNext,
            SectionKind::ThisSeason,
            SectionKind::Because,
            SectionKind::MoreGenre,
        ],
        "{home:?}"
    );
    assert_eq!(home.section(SectionKind::Continue).map(|s| s.total), Some(4), "as it was");
    for section in &home.sections {
        assert!(section.items.len() >= 4, "{:?} is thin: {section:?}", section.kind);
    }
    let first = |kind| home.section(kind).and_then(|s| s.items.first()).map(|i| i.entry.id);
    assert_eq!(first(SectionKind::UpNext), Some(world(12)), "The Ice Knight Season 2");
    assert_eq!(first(SectionKind::ThisSeason), Some(world(1)), "the most popular first");
    let because = home.section(SectionKind::Because).unwrap();
    assert!(because.title.english().contains("The Forest Courier"), "{because:?}");
    let genre = home.section(SectionKind::MoreGenre).unwrap();
    assert!(genre.title.english().contains("slice of life"), "{genre:?}");
}

#[test]
fn suggested_has_an_empty_continue_and_a_suggestion_in_the_spotlight() {
    let (engine, _home) = started(FfiScene::Suggested);
    let home = load(&engine).pop().unwrap();
    assert_eq!(case(&home).map(|c| c.0), Some(SpotlightCase::Suggested), "{home:?}");
    assert_eq!(case(&home).map(|c| c.1), Some(world(12)), "The Ice Knight Season 2");
    assert!(home.section(SectionKind::Continue).is_none());
    for kind in [SectionKind::UpNext, SectionKind::ThisSeason, SectionKind::Because] {
        assert!(home.section(kind).is_some(), "{kind:?}: {home:?}");
    }
}

#[test]
fn fresh_has_the_popular_pick_what_airs_today_and_a_full_season() {
    let (engine, _home) = started(FfiScene::Fresh);
    let home = load(&engine).pop().unwrap();
    assert_eq!(case(&home), Some((SpotlightCase::Popular, world(1))), "{home:?}");
    assert_eq!(kinds(&home), [SectionKind::AiringToday, SectionKind::ThisSeason], "{home:?}");
    assert!(home.section(SectionKind::AiringToday).unwrap().items.len() >= 3, "{home:?}");
    assert!(home.section(SectionKind::ThisSeason).unwrap().items.len() >= 8, "{home:?}");
}

#[test]
fn one_left_turns_from_the_first_case_to_the_popular_pick_once_its_episode_is_watched() {
    let (engine, _home) = started(FfiScene::OneLeft);
    let home = load(&engine).pop().unwrap();
    let (first, id) = case(&home).expect("a spotlight");
    assert_eq!(first, SpotlightCase::Continuing);
    assert_eq!(home.section(SectionKind::Continue).map(|s| s.total), Some(1));

    engine.send(Command::SetWatched { id, episodes: vec!["8".into()], watched: true });
    let then = homes(read_until(&engine, |e| {
        matches!(e, Event::Home(h) if h.spotlight.as_ref()
            .is_some_and(|s| s.case == SpotlightCase::Popular))
    }))
    .pop()
    .unwrap();
    assert_ne!(case(&then).map(|c| c.1), Some(id));
    assert!(then.section(SectionKind::Continue).is_none(), "{then:?}");
}

#[test]
fn continue_full_shows_twelve_of_fifteen_with_view_all() {
    let (engine, _home) = started(FfiScene::ContinueFull);
    let home = load(&engine).pop().unwrap();
    let shelf = home.section(SectionKind::Continue).expect("Continue");
    assert_eq!((shelf.items.len(), shelf.total), (12, 15));
    assert!(shelf.view_all.is_some());
    assert_eq!(case(&home).map(|c| c.0), Some(SpotlightCase::Continuing));
}

#[test]
fn not_found_says_no_source_has_the_spotlight_title() {
    let (engine, _home) = started(FfiScene::NotFound);
    let home = load(&engine).pop().unwrap();
    let (_, id) = case(&home).expect("a spotlight");
    engine.send(Command::ProbeTitle { id });
    // The answer, then Home again with it: often in one batch.
    let read = read_until(&engine, |e| {
        matches!(e, Event::Home(h) if h.spotlight.as_ref()
            .is_some_and(|s| s.playable == Playable::NotFound))
    });
    assert!(
        read.iter().any(|e| matches!(e,
            Event::TitlePlayable { id: probed, playable: Playable::NotFound } if *probed == id)),
        "{read:?}"
    );
    let home = homes(read).pop().unwrap();
    assert_eq!(home.spotlight.unwrap().action, HomeAction::AddToPlanning);
}

#[test]
fn slow_shows_continue_first_and_the_discovery_after_the_activity_line_is_due() {
    let (engine, _home) = started(FfiScene::Slow);
    let asked = Instant::now();
    let sent = load(&engine);
    let first = sent.iter().find(|h| h.section(SectionKind::Continue).is_some()).unwrap();
    assert!(!first.pending.is_empty(), "the discovery still on its way: {first:?}");
    assert!(asked.elapsed() > Duration::from_secs(4), "slower than the activity line's wait");
    let home = sent.last().unwrap();
    assert!(!home.failed && home.pending.is_empty(), "{home:?}");
}

#[test]
fn failed_shows_continue_and_one_failure() {
    let (engine, _home) = started(FfiScene::Failed);
    let sent = load(&engine);
    let home = sent.last().unwrap();
    assert!(sent.iter().any(|h| h.failed), "{sent:?}");
    assert!(home.section(SectionKind::Continue).is_some());
    assert!(home.section(SectionKind::ThisSeason).is_none(), "{home:?}");
}

#[test]
fn offline_has_nothing_saved_and_says_so() {
    let (engine, _home) = started(FfiScene::Offline);
    let home = load(&engine).pop().unwrap();
    assert!(home.offline && home.offline_since.is_none(), "{home:?}");
    assert!(home.sections.is_empty() && home.spotlight.is_none(), "{home:?}");
}

#[test]
fn a_scene_taken_offline_after_home_loaded_keeps_it_and_says_since_when() {
    let (engine, _home) = started(FfiScene::Watching);
    let shown = load(&engine).pop().unwrap();
    engine.testing_set_offline(true);
    // The host's own monitor says it is online: the scene stays offline all the same.
    engine.send(Command::NetworkPath { cellular: false, constrained: false, offline: false });
    let home =
        homes(read_until(&engine, |e| matches!(e, Event::Home(h) if h.offline))).pop().unwrap();
    assert!(home.offline_since.is_some(), "{home:?}");
    assert_eq!(home.sections.len(), shown.sections.len());
}
