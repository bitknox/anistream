//! What a native app shows outside itself, as the testing engine answers it: the new-episode
//! check its background task runs, and the record its home-screen widgets read. On the Watching
//! scene, with the fake AniList at the real time and the clock let run on to when an episode
//! airs.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anistream_core::{ids::AnilistId, settings::SettingValue};
use anistream_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene, FfiStart};
use anistream_model::{
    command::Command,
    event::Event,
    notifications::NewEpisode,
    settings::SettingKey,
    show::ShowSettingKey,
    widgets::{WIDGET_DATA_VERSION, WidgetAiring, WidgetData},
};
use anistream_testkit::fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA};

/// How long a test waits for what it is waiting on before failing. Everything is on loopback,
/// so a wait this long is a bug.
const PATIENCE: Duration = Duration::from_secs(30);

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Past Kumo no Ue no Toshokan's episode 6, which airs 2 d 3 h 15 m after the scene's moment,
/// and before anything else does.
const PAST_EPISODE_6: i64 = 2 * DAY + 4 * HOUR;

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

/// The Watching scene, in a temporary directory it is kept beside: signed in to the fake
/// AniList, with four titles under way, two of them airing.
fn watching() -> (Arc<FfiEngine>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine =
        FfiEngine::start_scene(dirs, Arc::new(Quiet), FfiScene::Watching, FfiStart::default())
            .expect("the engine starts");
    (engine, root)
}

/// Every event until one `until` accepts, that one included, polled on a thread of its own as
/// the bindings poll it.
fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        let found = loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                break false;
            }
            let found = batch.iter().any(&until);
            read.extend(batch);
            if found {
                break true;
            }
        };
        let _ = tx.send((read, found));
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

/// What AniList was asked since `before` requests, by operation, the launch's lookup of the
/// viewer's title language left out: it lands whenever it lands.
fn asked_since(engine: &FfiEngine, before: usize) -> Vec<String> {
    engine.testing_anilist_requests()[before..]
        .iter()
        .filter(|operation| *operation != "Viewer")
        .cloned()
        .collect()
}

/// Turn new-episode notifications on or off, as the app's switch does, and wait for the
/// settings to say so: the launch's said them first, off.
fn notifications(engine: &Arc<FfiEngine>, on: bool) {
    let set = Command::SetSetting {
        key: SettingKey::EpisodeNotifications,
        value: SettingValue::Bool(on),
    };
    assert_eq!(engine.send(set), None, "a setting is not a job");
    let shown = if on { "on" } else { "off" };
    settled(engine, SettingKey::EpisodeNotifications, shown);
}

/// Read until the settings show `key` as `value`, among the native apps' rows or the rest.
fn settled(engine: &Arc<FfiEngine>, key: SettingKey, value: &'static str) {
    read_until(engine, move |event| {
        matches!(event, Event::Settings(view)
            if view.rows.iter().chain(&view.native_rows).any(|row| row.key == key && row.value == value))
    });
}

/// Set show `id`'s own notification switch, or with `None` have it follow Settings again.
fn notify_show(engine: &Arc<FfiEngine>, id: AnilistId, on: Option<bool>) -> String {
    let set = Command::SetShowSetting {
        id,
        key: ShowSettingKey::NotifyNewEpisodes,
        value: on.map(SettingValue::Bool),
    };
    assert_eq!(engine.send(set), None, "a show's setting is not a job");
    let read = read_until(engine, |event| matches!(event, Event::ShowSettings(_)));
    let sheet = read.iter().find_map(|event| match event {
        Event::ShowSettings(sheet) => Some(sheet),
        _ => None,
    });
    let row = sheet
        .and_then(|sheet| {
            sheet.rows.iter().find(|r| r.key == ShowSettingKey::NotifyNewEpisodes)
        })
        .expect("the sheet has the switch");
    assert_eq!(row.own, on.map(SettingValue::Bool));
    assert_eq!(row.default, "on", "it follows Settings, which have notifications on");
    row.value.clone()
}

/// Run a check: what it said, and what it asked AniList.
fn check(engine: &Arc<FfiEngine>) -> (Vec<NewEpisode>, Vec<String>) {
    let before = engine.testing_anilist_requests().len();
    assert_eq!(engine.send(Command::CheckNewEpisodes), None, "a check is not a job");
    let read = read_until(engine, |event| matches!(event, Event::NewEpisodes(_)));
    let said = read.into_iter().find_map(|event| match event {
        Event::NewEpisodes(said) => Some(said),
        _ => None,
    });
    (said.expect("the check's answer"), asked_since(engine, before))
}

/// Make the widgets' record: what it holds, and what it asked AniList.
fn snapshot(engine: &Arc<FfiEngine>) -> (WidgetData, Vec<String>) {
    let before = engine.testing_anilist_requests().len();
    assert_eq!(engine.send(Command::WidgetSnapshot), None, "a snapshot is not a job");
    let read = read_until(engine, |event| matches!(event, Event::WidgetSnapshot(_)));
    let data = read.into_iter().find_map(|event| match event {
        Event::WidgetSnapshot(data) => Some(data),
        _ => None,
    });
    let mut asked = asked_since(engine, before);
    asked.sort();
    (data.expect("the record"), asked)
}

/// When episode `episode` of `id` airs at the fixtures' moment.
fn fixture_airs(id: AnilistId, episode: u32) -> i64 {
    fixtures::schedule()
        .into_iter()
        .find(|b| b.media.id == id && b.episode == episode)
        .expect("a fixture broadcast")
        .airing_at
}

/// How far the scene moved the fixtures' broadcasts, to the real time it started at: read off
/// a title's page, whose schedule the fake moved.
fn shift(engine: &Arc<FfiEngine>) -> i64 {
    let job = engine.send(Command::LoadDetail(AIRING)).expect("a title's page is a job");
    let read =
        read_until(engine, move |event| matches!(event, Event::JobDone(done) if *done == job));
    let airs = read.iter().find_map(|event| match event {
        Event::Detail(entry) => {
            entry.detail.as_ref()?.schedule.iter().find(|b| b.episode == 6).map(|b| b.airs_at)
        }
        _ => None,
    });
    airs.expect("episode 6 on the page's schedule") - fixture_airs(AIRING, 6)
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

#[test]
fn the_first_check_only_notes_where_things_stand_and_asks_nothing() {
    let (engine, _root) = watching();
    notifications(&engine, true);

    // Natsukaze Relay's episode 8 aired 19 hours ago, and Kumo no Ue no Toshokan's episode 5
    // nearly five days ago: a backlog a first check must not announce.
    let (said, asked) = check(&engine);
    assert_eq!(said, [], "a first check says nothing");
    assert_eq!(asked, Vec::<String>::new(), "and needs no request to note where things stand");

    // Nor does the next, once it asks: nothing that aired before the first is ever said.
    engine.testing_advance_clock(10 * MINUTE);
    let (said, asked) = check(&engine);
    assert_eq!(said, []);
    assert_eq!(asked, ["AiredSince"]);
    engine.shutdown();
}

#[test]
fn an_episode_that_airs_later_is_said_once_for_one_request() {
    let (engine, _root) = watching();
    let shift = shift(&engine);
    notifications(&engine, true);
    assert_eq!(check(&engine).0, []);

    engine.testing_advance_clock(PAST_EPISODE_6);
    let (said, asked) = check(&engine);
    assert_eq!(asked, ["AiredSince"], "one request, for every show at once");
    let [episode] = &said[..] else { panic!("one episode: {said:?}") };
    assert_eq!(
        (episode.id, episode.title.as_str(), episode.episode, episode.aired_at),
        (AIRING, "The Library Above the Clouds", 6, fixture_airs(AIRING, 6) + shift)
    );
    let cover = episode.cover_url.as_deref().unwrap_or_default();
    assert!(cover.starts_with("http://127.0.0.1:"), "the fixture's cover, served: {cover}");

    // Asked again at once, the look a moment ago answers, and AniList is not asked.
    let (said, asked) = check(&engine);
    assert_eq!((said, asked), (vec![], vec![]));
    // Asked again later, it is, and the episode is not said a second time.
    engine.testing_advance_clock(10 * MINUTE);
    let (said, asked) = check(&engine);
    assert_eq!(said, [], "said once");
    assert_eq!(asked, ["AiredSince"]);
    engine.shutdown();
}

#[test]
fn a_show_not_being_watched_is_said_only_once_switched_on_for_itself() {
    for switched_on in [false, true] {
        let (engine, _root) = watching();
        notifications(&engine, true);
        // Taken off Continue, and on no list as watching: not followed, as a planned show is
        // not.
        assert_eq!(engine.send(Command::HideFromContinue { id: AIRING_NO_MAL }), None);
        if switched_on {
            assert_eq!(notify_show(&engine, AIRING_NO_MAL, Some(true)), "on");
        }
        assert_eq!(check(&engine).0, []);

        // A week on, Kumo no Ue no Toshokan's episode 6 has aired, and Natsukaze Relay's 9.
        engine.testing_advance_clock(7 * DAY);
        let said: Vec<_> = check(&engine).0.into_iter().map(|e| (e.id, e.episode)).collect();
        let expected =
            if switched_on { vec![(AIRING, 6), (AIRING_NO_MAL, 9)] } else { vec![(AIRING, 6)] };
        assert_eq!(said, expected, "switched on: {switched_on}");
        engine.shutdown();
    }
}

#[test]
fn a_show_switched_off_is_not_said_nor_what_aired_while_it_was() {
    let (engine, _root) = watching();
    notifications(&engine, true);
    assert_eq!(check(&engine).0, []);
    assert_eq!(notify_show(&engine, AIRING, Some(false)), "off");

    engine.testing_advance_clock(PAST_EPISODE_6);
    assert_eq!(check(&engine).0, [], "its episode 6 aired, and it is switched off");

    // Followed again from when it is switched back on: episode 6 aired before that.
    assert_eq!(notify_show(&engine, AIRING, None), "on");
    engine.testing_advance_clock(10 * MINUTE);
    assert_eq!(check(&engine).0, []);
    engine.shutdown();
}

#[test]
fn with_notifications_off_nothing_is_asked_or_said() {
    let (engine, _root) = watching();
    // Off until the viewer turns them on.
    let (said, asked) = check(&engine);
    assert_eq!((said, asked), (vec![], vec![]));
    engine.testing_advance_clock(PAST_EPISODE_6);
    let (said, asked) = check(&engine);
    assert_eq!((said, asked), (vec![], vec![]), "episode 6 has aired, and nothing is said");

    // Turned on now, the first check notes where things stand: episode 6 aired before it.
    notifications(&engine, true);
    let (said, asked) = check(&engine);
    assert_eq!((said, asked), (vec![], vec![]));

    // Turned off, what was noted is forgotten, so turned on again it is a first check again.
    notifications(&engine, false);
    assert_eq!(check(&engine).0, []);
    engine.testing_advance_clock(7 * DAY);
    notifications(&engine, true);
    let (said, asked) = check(&engine);
    assert_eq!(said, [], "Natsukaze Relay's episode 9 aired while they were off");
    assert_eq!(asked, Vec::<String>::new());
    engine.shutdown();
}

#[test]
fn the_title_is_in_the_form_the_title_setting_picks() {
    let (engine, _root) = watching();
    let romaji = Command::SetSetting {
        key: SettingKey::Titles,
        value: SettingValue::Str("romaji".into()),
    };
    assert_eq!(engine.send(romaji), None);
    settled(&engine, SettingKey::Titles, "Romaji");
    notifications(&engine, true);
    assert_eq!(check(&engine).0, []);

    engine.testing_advance_clock(PAST_EPISODE_6);
    let titles: Vec<_> = check(&engine).0.into_iter().map(|e| e.title).collect();
    assert_eq!(titles, ["Kumo no Ue no Toshokan"]);
    engine.shutdown();
}

#[test]
fn nothing_is_asked_while_the_cache_shows_nothing_can_have_aired() {
    let (engine, _root) = watching();
    // Continue fetched the four titles, and the cache keeps each one's next episode.
    let job = engine.send(Command::LoadContinue).expect("Continue is a job");
    read_until(&engine, move |event| matches!(event, Event::JobDone(done) if *done == job));
    notifications(&engine, true);
    assert_eq!(check(&engine).0, []);

    // An hour on, two have finished and the other two air days from now.
    engine.testing_advance_clock(HOUR);
    let (said, asked) = check(&engine);
    assert_eq!((said, asked), (vec![], vec![]), "nothing can have aired, so nothing is asked");

    // Past episode 6, the cache says it is due, so AniList is asked.
    engine.testing_advance_clock(PAST_EPISODE_6 - HOUR);
    let (said, asked) = check(&engine);
    assert_eq!(said.iter().map(|e| (e.id, e.episode)).collect::<Vec<_>>(), [(AIRING, 6)]);
    assert_eq!(asked, ["AiredSince"]);
    engine.shutdown();
}

/// The start of the day `at` is in, and of the next, in the zone the engine reads days in: the
/// system's.
fn day_of(at: i64) -> (i64, i64) {
    let zone = jiff::tz::TimeZone::system();
    let date = jiff::Timestamp::from_second(at).unwrap().to_zoned(zone.clone()).date();
    let start = |date: jiff::civil::Date| date.to_zoned(zone.clone()).unwrap().timestamp();
    (start(date).as_second(), start(date.tomorrow().unwrap()).as_second())
}

#[test]
fn the_widgets_record_for_the_watching_scene() {
    let (engine, _root) = watching();
    let shift = shift(&engine);
    // The middle of the day Natsukaze Relay's episode 9 airs on, so it is today's whatever
    // the hour the test runs at.
    let (day, next_day) = day_of(fixture_airs(AIRING_NO_MAL, 9) + shift);
    let midday = day + (next_day - day) / 2;
    engine.testing_advance_clock(midday - now());

    let (data, asked) = snapshot(&engine);
    assert_eq!(data.version, WIDGET_DATA_VERSION);
    assert!((data.generated_at - midday).abs() < MINUTE, "{} {midday}", data.generated_at);
    // Continue's four, most recent first, each where Continue has it.
    let continuing: Vec<_> = data
        .continue_watching
        .iter()
        .map(|row| {
            (row.id, row.title.as_str(), row.episode, row.watched, row.episodes, row.fraction)
        })
        .collect();
    assert_eq!(
        continuing,
        [
            (FINISHED, "Stardust Wayfarer", 7, 6, Some(12), Some(606.0 / 1_440.0)),
            (ONA, "The Steel Gardener", 2, 1, Some(10), Some(810.0 / 1_080.0)),
            (AIRING, "The Library Above the Clouds", 4, 3, Some(24), None),
            (AIRING_NO_MAL, "Summer Wind Relay", 8, 7, Some(12), None),
        ]
    );
    for row in &data.continue_watching {
        let cover = row.cover_url.as_deref().unwrap_or_default();
        assert!(cover.starts_with("http://127.0.0.1:"), "{}: {cover}", row.id);
    }

    // Today's broadcasts, in the order they air: every fixture broadcast that falls on it, each
    // of a show being watched.
    let today = |on_list: fn(AnilistId) -> bool| -> Vec<(AnilistId, String, u32, i64, bool)> {
        fixtures::schedule()
            .into_iter()
            .map(|b| (b.media.id, b.episode, b.airing_at + shift))
            .filter(|(_, _, at)| (day..next_day).contains(at))
            .map(|(id, episode, at)| (id, fixtures::title(id), episode, at, on_list(id)))
            .collect()
    };
    let shown = |rows: &[WidgetAiring]| -> Vec<(AnilistId, String, u32, i64, bool)> {
        rows.iter()
            .map(|row| (row.id, row.title.clone(), row.episode, row.airs_at, row.on_list))
            .collect()
    };
    let expected = today(|_| true);
    assert!(
        expected.iter().any(|row| (row.0, row.2) == (AIRING_NO_MAL, 9)),
        "episode 9 is today's: {expected:?}"
    );
    assert_eq!(shown(&data.airing_today), expected);
    assert!(data.airing_today.iter().all(|row| row.cover_url.is_some()));
    // The one title the cache lacked, and today's broadcasts: a request each.
    assert_eq!(asked, ["Many", "Schedule"]);

    // Taken off Continue, Natsukaze Relay is no longer followed. Asked again at once, the
    // record comes from the cache and today's broadcasts as they were just fetched.
    assert_eq!(engine.send(Command::HideFromContinue { id: AIRING_NO_MAL }), None);
    let (data, asked) = snapshot(&engine);
    let ids: Vec<_> = data.continue_watching.iter().map(|row| row.id).collect();
    assert_eq!(ids, [FINISHED, ONA, AIRING]);
    assert_eq!(shown(&data.airing_today), today(|id| id != AIRING_NO_MAL));
    assert_eq!(asked, Vec::<String>::new(), "nothing asked again within minutes");
    engine.shutdown();
}
