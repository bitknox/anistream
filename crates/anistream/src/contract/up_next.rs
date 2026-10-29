//! "Up next for you": the suggestions under a short Continue.
//!
//! Every title here is invented (`fixtures::invented`), named `Invented {id}`, and its shows
//! are told apart by id. A title numbered 93000x is a show the viewer finished, 9301xx one they
//! are part way through, 9302xx a recommendation, 9303xx a title on the Planning list.

use anistream_core::ids::AnilistId;
use anistream_engine::clock::Clock;
use anistream_model::{
    catalog::{AiringStatus, FuzzyDate},
    command::Command,
    event::Event,
    up_next::{UpNextAction, UpNextItem, UpNextReason},
};
use anistream_testkit::{
    COMPLEXITY_LIMIT, Operation, complexity,
    fixtures::{AIRING, AIRING_NO_MAL, FIXTURE_NOW, MOVIE, ONA, SEQUEL, invented},
    seed,
};

use super::{WITH_ANILIST, ends};
use crate::harness::{Harness, Options};

const FINISHED_ONE: u32 = 930_001;
const NEW_SEASON_AIRING: u32 = 930_002;
const NEW_SEASON_ANNOUNCED: u32 = 930_003;
/// Part way through, with a sequel that must not be suggested yet.
const UNFINISHED: u32 = 930_101;
const UNFINISHED_SEQUEL: u32 = 930_102;
const RECOMMENDED_ONE: u32 = 930_201;
const RECOMMENDED_TWO: u32 = 930_202;
/// Recommended, but a sequel of `UNFINISHED`.
const RECOMMENDED_LATE: u32 = 930_203;
const PLANNED_AIRING: u32 = 930_301;
const PLANNED_ANNOUNCED: u32 = 930_302;
const PLANNED_FINISHED: u32 = 930_303;
/// On Planning, but a sequel of `UNFINISHED`.
const PLANNED_LATE: u32 = 930_304;

fn id(n: u32) -> AnilistId {
    AnilistId::new(n)
}

fn signed_in() -> Options {
    Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    }
}

/// The invented shows, on the fake AniList.
fn invent(h: &Harness) {
    let finished = |n, relations: &[(&str, u32)], recommends: &[u32]| {
        invented(n, "FINISHED", 12, (2024, 4, 6), relations, recommends)
    };
    h.server().add_titles([
        finished(
            FINISHED_ONE,
            &[("SEQUEL", NEW_SEASON_AIRING), ("SEQUEL", NEW_SEASON_ANNOUNCED)],
            &[RECOMMENDED_ONE, RECOMMENDED_LATE, RECOMMENDED_TWO],
        ),
        invented(
            NEW_SEASON_AIRING,
            "RELEASING",
            12,
            (2026, 10, 4),
            &[("PREQUEL", FINISHED_ONE)],
            &[],
        ),
        invented(
            NEW_SEASON_ANNOUNCED,
            "NOT_YET_RELEASED",
            12,
            (2027, 1, 9),
            &[("PREQUEL", FINISHED_ONE)],
            &[],
        ),
        finished(UNFINISHED, &[("SEQUEL", UNFINISHED_SEQUEL)], &[]),
        invented(
            UNFINISHED_SEQUEL,
            "RELEASING",
            12,
            (2026, 10, 4),
            &[("PREQUEL", UNFINISHED)],
            &[],
        ),
        finished(RECOMMENDED_ONE, &[], &[]),
        invented(RECOMMENDED_TWO, "RELEASING", 12, (2026, 10, 4), &[], &[]),
        finished(RECOMMENDED_LATE, &[("PREQUEL", UNFINISHED)], &[]),
        invented(PLANNED_AIRING, "RELEASING", 12, (2026, 10, 4), &[], &[]),
        invented(PLANNED_ANNOUNCED, "NOT_YET_RELEASED", 12, (2027, 4, 1), &[], &[]),
        finished(PLANNED_FINISHED, &[], &[]),
        finished(PLANNED_LATE, &[("PREQUEL", UNFINISHED)], &[]),
    ]);
}

/// Watch `n` of the shows' episodes, the latest at `FIXTURE_NOW - age` seconds.
fn watch(h: &Harness, show: u32, episodes: u32, age: i64) {
    let runtime = 24.0 * 60.0;
    for episode in 1..=episodes {
        let event = anistream_store::WatchEvent {
            duration_secs: Some(runtime),
            watched_secs: runtime,
            completed: true,
            at: FIXTURE_NOW - age - i64::from(episodes - episode) * 3_600,
            ..anistream_store::WatchEvent::new(id(show), episode.to_string(), runtime)
        };
        h.store().record_event(&event).expect("a watched episode");
    }
}

fn list(h: &Harness, statuses: &[(u32, &str)]) {
    let rows: Vec<(AnilistId, &str)> = statuses.iter().map(|(n, s)| (id(*n), *s)).collect();
    h.store().replace_library_statuses("anilist", &rows, FIXTURE_NOW).unwrap();
}

/// Send the load and return what it answered with.
async fn load(h: &mut Harness) -> Vec<UpNextItem> {
    let job = h.send(Command::LoadUpNext);
    let updates = h.settle().await;
    ends(&updates, job);
    let mut sent = updates.into_iter().filter_map(|u| match u {
        Event::UpNext(items) => Some(items),
        _ => None,
    });
    let items = sent.next().expect("Up next was sent");
    assert!(sent.next().is_none(), "sent once");
    items
}

fn ids_of(items: &[UpNextItem]) -> Vec<u32> {
    items.iter().map(|i| i.entry.id.get()).collect()
}

#[tokio::test]
async fn new_seasons_come_first_then_planning_then_recommendations() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(
        &h,
        &[
            (FINISHED_ONE, "COMPLETED"),
            (PLANNED_FINISHED, "PLANNING"),
            (PLANNED_ANNOUNCED, "PLANNING"),
            (PLANNED_AIRING, "PLANNING"),
        ],
    );

    let items = load(&mut h).await;

    // Airing, then announced, for the new seasons; the same for Planning, where a title that
    // finished airing is last; then what the finished show's fans rated highest.
    assert_eq!(
        ids_of(&items),
        [
            NEW_SEASON_AIRING,
            NEW_SEASON_ANNOUNCED,
            PLANNED_AIRING,
            PLANNED_ANNOUNCED,
            PLANNED_FINISHED,
            RECOMMENDED_ONE,
        ]
    );
    let reasons: Vec<_> = items.iter().map(|i| i.reason).collect();
    assert_eq!(
        reasons,
        [
            UpNextReason::NewSeason,
            UpNextReason::NewSeason,
            UpNextReason::Planning,
            UpNextReason::Planning,
            UpNextReason::Planning,
            UpNextReason::Recommended,
        ]
    );
    // What each does, and when the ones that have not started will.
    let actions: Vec<_> = items.iter().map(|i| i.action).collect();
    assert_eq!(
        actions,
        [
            UpNextAction::PlayFirstEpisode,
            UpNextAction::AddToPlanning,
            UpNextAction::PlayFirstEpisode,
            UpNextAction::Nothing,
            UpNextAction::PlayFirstEpisode,
            UpNextAction::PlayFirstEpisode,
        ]
    );
    assert_eq!(items[0].starts, None, "it airs already");
    assert_eq!(items[1].starts, Some(FuzzyDate { year: 2027, month: Some(1), day: Some(9) }));
    assert_eq!(items[1].entry.status, Some(AiringStatus::NotYetReleased));
    assert_eq!(items[5].because.as_deref(), Some("Invented 930001"));
}

#[tokio::test]
async fn never_a_title_whose_prequel_is_unfinished() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    // Five of twelve watched: its sequel, a planned sequel and a recommended one stay away.
    watch(&h, UNFINISHED, 5, 600);
    watch(&h, FINISHED_ONE, 12, 3_600);
    list(&h, &[(FINISHED_ONE, "COMPLETED"), (PLANNED_LATE, "PLANNING")]);

    let items = load(&mut h).await;

    let shown = ids_of(&items);
    for late in [UNFINISHED_SEQUEL, PLANNED_LATE, RECOMMENDED_LATE] {
        assert!(
            !shown.contains(&late),
            "{late} follows a show that is not finished: {shown:?}"
        );
    }
    assert!(shown.contains(&NEW_SEASON_AIRING), "the finished show's sequel is fine");
}

#[tokio::test]
async fn the_queue_leaves_room_for_eight_less_two_a_row_and_none_at_four() {
    // Eight titles on Planning, all airing; queue rows are shows part watched.
    let planned: Vec<u32> = (930_310..930_318).collect();
    for (rows, room) in [(1usize, 6usize), (2, 4), (3, 2), (4, 0), (5, 0)] {
        let mut h = Harness::with(signed_in()).await;
        h.server().add_titles(
            planned.iter().map(|n| invented(*n, "RELEASING", 12, (2026, 10, 4), &[], &[])),
        );
        let queue = [AIRING, AIRING_NO_MAL, ONA, SEQUEL, MOVIE];
        for (n, show) in queue.iter().take(rows).enumerate() {
            seed::stopped_at(h.store(), *show, "1", 300.0, FIXTURE_NOW - 60 * n as i64);
        }
        let statuses: Vec<(u32, &str)> = planned.iter().map(|n| (*n, "PLANNING")).collect();
        list(&h, &statuses);

        let items = load(&mut h).await;
        assert_eq!(items.len(), room, "with {rows} queue rows");
    }
}

#[tokio::test]
async fn not_interested_hides_it_until_it_airs_and_for_good_once_it_does() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);
    assert!(
        ids_of(&load(&mut h).await).starts_with(&[NEW_SEASON_AIRING, NEW_SEASON_ANNOUNCED])
    );

    // One that airs, and one that is announced.
    h.send(Command::DismissUpNext { id: id(NEW_SEASON_AIRING) });
    h.send(Command::DismissUpNext { id: id(NEW_SEASON_ANNOUNCED) });
    h.settle().await;
    let shown = ids_of(&load(&mut h).await);
    assert!(!shown.contains(&NEW_SEASON_AIRING));
    assert!(!shown.contains(&NEW_SEASON_ANNOUNCED));

    // The announced one starts airing: it is back. The one that already aired stays away.
    h.server().add_titles([invented(
        NEW_SEASON_ANNOUNCED,
        "RELEASING",
        12,
        (2027, 1, 9),
        &[("PREQUEL", FINISHED_ONE)],
        &[],
    )]);
    h.advance_engine_clock(31 * 60);
    let shown = ids_of(&load(&mut h).await);
    assert!(shown.contains(&NEW_SEASON_ANNOUNCED), "it airs now: {shown:?}");
    assert!(!shown.contains(&NEW_SEASON_AIRING), "it aired when it was dismissed: {shown:?}");
}

#[tokio::test]
async fn signed_out_there_is_no_planning_and_the_list_does_not_count() {
    let mut h = Harness::new().await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    // The tracker list is a copy of a signed-in answer: signed out, it is not read.
    list(&h, &[(PLANNED_AIRING, "PLANNING"), (PLANNED_FINISHED, "COMPLETED")]);

    let items = load(&mut h).await;

    let shown = ids_of(&items);
    assert!(shown.starts_with(&[NEW_SEASON_AIRING, NEW_SEASON_ANNOUNCED]), "{shown:?}");
    assert!(items.iter().all(|i| i.reason != UpNextReason::Planning), "{shown:?}");
    assert!(!shown.contains(&PLANNED_AIRING));
}

#[tokio::test]
async fn a_title_completed_on_the_list_counts_as_finished() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    // Something watched here, so this is no new viewer; the finished show is only on the list.
    watch(&h, UNFINISHED, 2, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);

    let items = load(&mut h).await;

    assert!(ids_of(&items).starts_with(&[NEW_SEASON_AIRING, NEW_SEASON_ANNOUNCED]));
}

#[tokio::test]
async fn with_nothing_of_their_own_it_falls_back_to_what_airs_today() {
    // Days are read in a zone whose midnight is twenty hours before now, so the one broadcast
    // the fixtures have near now (nineteen hours before it) is today whatever the hour.
    let now = anistream_store::now();
    let midnight = now - 20 * 3_600;
    let east = (86_400 - midnight.rem_euclid(86_400)).rem_euclid(86_400);
    let east = if east > 12 * 3_600 { east - 86_400 } else { east };
    let zone = jiff::tz::TimeZone::fixed(jiff::tz::Offset::from_seconds(east as i32).unwrap());
    let mut h =
        Harness::with(Options { now, clock: Some(Clock::in_zone(zone)), ..Options::default() })
            .await;
    // One web series watched through: finished, with no sequel and nothing recommended.
    seed::watched(h.store(), ONA, 10, now - 600);

    let items = load(&mut h).await;

    assert_eq!(ids_of(&items), [AIRING_NO_MAL.get()]);
    assert_eq!(items[0].reason, UpNextReason::AiringToday);
    assert_eq!(items[0].action, UpNextAction::PlayFirstEpisode);
    // It says it is the fallback, and when the episode airs.
    assert!(items[0].fallback);
    let airs_at = items[0].airs_at.expect("the broadcast time");
    assert!(airs_at > now - 20 * 3_600 && airs_at < now, "today: {airs_at}");
}

#[tokio::test]
async fn personal_suggestions_are_not_the_fallback_and_carry_no_air_time() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);

    let items = load(&mut h).await;

    assert!(!items.is_empty());
    assert!(items.iter().all(|i| !i.fallback && i.airs_at.is_none()), "{items:?}");
}

/// The suggestions an `Event::UpNext` among `updates` carries, the last of them.
fn sent(updates: Vec<Event>) -> Option<Vec<UpNextItem>> {
    updates
        .into_iter()
        .filter_map(|u| match u {
            Event::UpNext(items) => Some(items),
            _ => None,
        })
        .next_back()
}

#[tokio::test]
async fn restoring_a_dismissed_title_brings_it_back_in_a_fresh_event() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);
    h.send(Command::DismissUpNext { id: id(NEW_SEASON_AIRING) });
    h.settle().await;
    assert!(!ids_of(&load(&mut h).await).contains(&NEW_SEASON_AIRING));

    h.send(Command::RestoreUpNext { id: id(NEW_SEASON_AIRING) });
    let items = sent(h.settle().await).expect("a fresh Up next");

    assert_eq!(ids_of(&items)[0], NEW_SEASON_AIRING);
}

#[tokio::test]
async fn a_change_to_the_queue_sends_up_next_again_cut_to_the_room() {
    let mut h = Harness::with(signed_in()).await;
    let planned: Vec<u32> = (930_310..930_318).collect();
    h.server().add_titles(
        planned.iter().map(|n| invented(*n, "RELEASING", 12, (2026, 10, 4), &[], &[])),
    );
    let statuses: Vec<(u32, &str)> = planned.iter().map(|n| (*n, "PLANNING")).collect();
    list(&h, &statuses);
    seed::stopped_at(h.store(), AIRING, "1", 300.0, FIXTURE_NOW - 60);
    seed::stopped_at(h.store(), AIRING_NO_MAL, "1", 300.0, FIXTURE_NOW - 120);
    assert_eq!(load(&mut h).await.len(), 4, "two rows leave room for four");

    // A row removed: one row leaves room for six, and nobody asked again.
    h.send(Command::HideFromContinue { id: AIRING });
    let items = sent(h.settle().await).expect("sent without being asked");
    assert_eq!(items.len(), 6);

    // Another show started: back to four.
    seed::stopped_at(h.store(), ONA, "1", 300.0, FIXTURE_NOW - 30);
    h.send(Command::HideFromContinue { id: id(930_999) });
    let items = sent(h.settle().await).expect("sent for a show that started");
    assert_eq!(items.len(), 4);
}

#[tokio::test]
async fn up_next_is_not_sent_by_itself_before_it_was_asked_for() {
    let mut h = Harness::with(signed_in()).await;
    seed::stopped_at(h.store(), AIRING, "1", 300.0, FIXTURE_NOW - 60);
    h.send(Command::HideFromContinue { id: AIRING });
    assert!(sent(h.settle().await).is_none());
}

#[tokio::test]
async fn nothing_watched_sends_nothing_and_asks_nothing() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    list(&h, &[(PLANNED_AIRING, "PLANNING")]);

    assert!(load(&mut h).await.is_empty(), "the welcome is the app's");
    for asked in [Operation::Many, Operation::Recommended, Operation::Schedule] {
        assert!(h.server().requests_for(asked).is_empty(), "{asked:?} was asked");
    }
}

#[tokio::test]
async fn what_was_found_is_kept_for_half_an_hour() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);
    let first = load(&mut h).await;
    let asked = h.server().requests().len();

    let again = load(&mut h).await;
    assert_eq!(ids_of(&again), ids_of(&first), "the same list");
    let reasons = |items: &[UpNextItem]| items.iter().map(|i| i.reason).collect::<Vec<_>>();
    assert_eq!(reasons(&again), reasons(&first));
    assert_eq!(h.server().requests().len(), asked, "from what was kept, asking nothing");

    h.advance_engine_clock(31 * 60);
    load(&mut h).await;
    assert!(h.server().requests().len() > asked, "asked again once it is old");
}

#[tokio::test]
async fn the_recommendations_query_stays_well_inside_the_complexity_limit() {
    let mut h = Harness::with(signed_in()).await;
    invent(&h);
    watch(&h, FINISHED_ONE, 12, 600);
    list(&h, &[(FINISHED_ONE, "COMPLETED")]);
    load(&mut h).await;

    let asked = h.server().requests_for(Operation::Recommended);
    assert_eq!(asked.len(), 1, "one request for every finished show");
    let cost = complexity(&asked[0].query, &asked[0].variables);
    println!("recommendations query, one title: {cost} of {COMPLEXITY_LIMIT}");
    assert!(cost > 0 && cost < COMPLEXITY_LIMIT / 4, "costs {cost}");

    // At the most titles it ever asks about.
    let most = anistream_meta::anilist::query::RECOMMENDED_FROM;
    let vars = serde_json::json!({ "ids": (0..most).collect::<Vec<_>>(), "perPage": most });
    let cost = complexity(&anistream_meta::anilist::query::recommended_from(), &vars);
    println!("recommendations query, {most} titles: {cost} of {COMPLEXITY_LIMIT}");
    assert!(cost < COMPLEXITY_LIMIT / 2, "costs {cost}");
}
