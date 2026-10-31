//! Home: the spotlight, Continue, the week and the discovery shelves, as the engine composes
//! them for every app.
//!
//! Every title here is invented (`fixtures::invented`), named `Invented {id}`, and told apart by
//! id. The engine's clock is set to a day in 2030 or 2031, read in UTC, so the seasons asked for
//! are never the one the fake answers with the fixtures' own (the real clock's).
//!
//! A title numbered 9400xx is one the viewer watched, 9401xx one of the autumn 2030 season,
//! 9402xx one part way through, 9403xx a sequel, 9404xx one of the winter 2031 season.

use std::sync::{Arc, atomic::Ordering};
use std::time::Duration;

use anistream_core::{ids::AnilistId, media::SearchHit};
use anistream_engine::clock::Clock;
use anistream_model::{
    Msg,
    catalog::Season,
    command::Command,
    event::Event,
    home::{
        HiddenTitle, HomeAction, HomeEnd, HomeLink, HomeView, Playable, SectionKind,
        SpotlightCase,
    },
    present::SeasonYear,
    up_next::UpNextReason,
};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::{Operation, anilist::Failure, fixtures::invented};
use serde_json::{Value, json};

use super::ends;
use crate::harness::{Harness, Options};

const DAY: i64 = 86_400;

/// Watched to the end long ago: no sequel, nothing recommended.
const FINISHED: u32 = 940_001;
/// The prequel of `SEQUEL`, never watched.
const PREQUEL: u32 = 940_002;
/// Autumn 2030, most popular first: a sequel of an unwatched title, one not out yet, then two
/// airing.
const POP_SEQUEL: u32 = 940_100;
const POP_UNAIRED: u32 = 940_101;
const POP_ONE: u32 = 940_102;
const POP_TWO: u32 = 940_103;
/// Part way through, an episode waiting.
const WATCHING: u32 = 940_201;
const WATCHING_TOO: u32 = 940_202;
/// Airing, every episode out watched.
const CAUGHT_UP: u32 = 940_203;
/// The new season of `FINISHED`, airing this autumn.
const NEW_SEASON: u32 = 940_301;
/// Winter 2031: one announced, one airing.
const WINTER_UNAIRED: u32 = 940_401;
const WINTER_AIRING: u32 = 940_402;

fn id(n: u32) -> AnilistId {
    AnilistId::new(n)
}

/// Noon UTC on a day.
fn noon(year: i16, month: i8, day: i8) -> i64 {
    jiff::civil::date(year, month, day)
        .at(12, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .unwrap()
        .timestamp()
        .as_second()
}

/// 15 November 2030: autumn, with winter six weeks off.
fn autumn() -> i64 {
    noon(2030, 11, 15)
}

/// The app, its clock and the fake's stopped at `now`, days read in UTC.
async fn at(now: i64, registry: Option<ProviderRegistry>) -> Harness {
    let clock = Clock::in_zone(jiff::tz::TimeZone::UTC);
    clock.advance(now - anistream_store::now());
    Harness::with(Options { now, clock: Some(clock), registry, ..Options::default() }).await
}

/// An invented title of a season, as popular as `popularity`.
fn show(
    n: u32,
    status: &str,
    (season, year): (&str, u16),
    popularity: u32,
    relations: &[(&str, u32)],
) -> Value {
    let month = match season {
        "WINTER" => 1,
        "SPRING" => 4,
        "SUMMER" => 7,
        _ => 10,
    };
    let mut title = invented(n, status, 12, (year, month, 6), relations, &[]);
    title["season"] = json!(season);
    title["seasonYear"] = json!(year);
    title["popularity"] = json!(popularity);
    title
}

/// The autumn 2030 season and the titles watched, on the fake AniList.
fn season(h: &Harness, extra: impl IntoIterator<Item = Value>) {
    const FALL: (&str, u16) = ("FALL", 2030);
    const SUMMER: (&str, u16) = ("SUMMER", 2030);
    let mut titles = vec![
        show(FINISHED, "FINISHED", SUMMER, 10, &[]),
        show(PREQUEL, "FINISHED", SUMMER, 5, &[("SEQUEL", POP_SEQUEL)]),
        show(POP_SEQUEL, "RELEASING", FALL, 1_000, &[("PREQUEL", PREQUEL)]),
        show(POP_UNAIRED, "NOT_YET_RELEASED", FALL, 950, &[]),
        show(POP_ONE, "RELEASING", FALL, 900, &[]),
        show(POP_TWO, "RELEASING", FALL, 800, &[]),
        show(WATCHING, "FINISHED", SUMMER, 20, &[]),
        show(WATCHING_TOO, "FINISHED", SUMMER, 30, &[]),
        show(CAUGHT_UP, "RELEASING", FALL, 40, &[]),
    ];
    titles.extend(extra);
    h.server().add_catalogue(titles);
}

/// Watch `episodes` of a show, the last `age` seconds before `now`.
fn watch(h: &Harness, show: u32, episodes: std::ops::RangeInclusive<u32>, now: i64, age: i64) {
    let runtime = 24.0 * 60.0;
    let last = *episodes.end();
    for episode in episodes {
        let event = anistream_store::WatchEvent {
            duration_secs: Some(runtime),
            watched_secs: runtime,
            completed: true,
            at: now - age - i64::from(last - episode) * 3_600,
            ..anistream_store::WatchEvent::new(id(show), episode.to_string(), runtime)
        };
        h.store().record_event(&event).expect("a watched episode");
    }
}

/// Every Home sent among `updates`, in order.
fn homes(updates: &[Event]) -> Vec<HomeView> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::Home(home) => Some(home.as_ref().clone()),
            _ => None,
        })
        .collect()
}

/// Ask for Home and return every Home it sent, the last complete.
async fn load(h: &mut Harness) -> Vec<HomeView> {
    let job = h.send(Command::LoadHome { refresh: false });
    let updates = h.settle().await;
    ends(&updates, job);
    let sent = homes(&updates);
    assert!(!sent.is_empty(), "Home was sent");
    sent
}

async fn last(h: &mut Harness) -> HomeView {
    load(h).await.pop().unwrap()
}

fn spotlit(home: &HomeView) -> Option<(u32, SpotlightCase)> {
    home.spotlight.as_ref().map(|s| (s.entry.id.get(), s.case))
}

fn kinds(home: &HomeView) -> Vec<SectionKind> {
    home.sections.iter().map(|s| s.kind).collect()
}

fn shelf(home: &HomeView, kind: SectionKind) -> Vec<u32> {
    home.section(kind)
        .map(|s| s.items.iter().map(|i| i.entry.id.get()).collect())
        .unwrap_or_default()
}

fn everywhere(home: &HomeView) -> Vec<u32> {
    home.ids().map(|id| id.get()).collect()
}

#[tokio::test]
async fn a_show_with_an_episode_waiting_is_the_spotlight_and_first_in_continue() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, WATCHING, 1..=3, now, 600);
    watch(&h, WATCHING_TOO, 1..=2, now, 7_200);

    let sent = load(&mut h).await;
    let home = sent.last().unwrap();

    let spotlight = home.spotlight.as_ref().expect("a spotlight");
    assert_eq!(
        (spotlight.entry.id.get(), spotlight.case),
        (WATCHING, SpotlightCase::Continuing)
    );
    assert_eq!(spotlight.episode, 4);
    assert_eq!(spotlight.action, HomeAction::Play);
    assert_eq!(spotlight.label, Msg::HomeSpotlightContinuing);
    assert_eq!(kinds(home)[0], SectionKind::Continue);
    let cont = home.section(SectionKind::Continue).unwrap();
    assert_eq!(shelf(home, SectionKind::Continue), [WATCHING, WATCHING_TOO]);
    assert_eq!(cont.total, 2);
    assert_eq!(cont.view_all, Some(HomeLink::Watching));
    assert_eq!(cont.items[0].line, Some(Msg::HomeEpisodeOut { episode: 4 }));
    assert_eq!(spotlight.line, Some(Msg::HomeEpisodeOut { episode: 4 }));
    // Never a spotlight that is then replaced by another as the parts land.
    let mut shown: Vec<_> = sent.iter().filter_map(spotlit).collect();
    shown.dedup();
    assert_eq!(shown, [(WATCHING, SpotlightCase::Continuing)]);
    assert_eq!(home.end, Some(HomeEnd::SignIn));
    assert!(home.pending.is_empty());
}

#[tokio::test]
async fn a_caught_up_show_is_never_the_spotlight() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    // Episode 1 is out and watched; episode 2 airs tomorrow.
    watch(&h, CAUGHT_UP, 1..=1, now, 600);

    let home = last(&mut h).await;

    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    assert!(home.section(SectionKind::Continue).is_none(), "nothing waits: {:?}", kinds(&home));
}

#[tokio::test]
async fn with_nothing_waiting_a_suggestion_comes_before_the_popular_pick() {
    let now = autumn();
    let mut h = at(now, None).await;
    let finished = show(FINISHED, "FINISHED", ("SUMMER", 2030), 10, &[("SEQUEL", NEW_SEASON)]);
    season(&h, [show(NEW_SEASON, "RELEASING", ("FALL", 2030), 60, &[("PREQUEL", FINISHED)])]);
    // The finished title again, now with its new season.
    h.server().add_titles([finished]);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let sent = load(&mut h).await;
    let home = sent.last().unwrap();

    let spotlight = home.spotlight.as_ref().expect("a spotlight");
    assert_eq!(
        (spotlight.entry.id.get(), spotlight.case),
        (NEW_SEASON, SpotlightCase::Suggested)
    );
    assert_eq!(spotlight.reason, Some(UpNextReason::NewSeason));
    assert_eq!(
        spotlight.label,
        Msg::HomeSpotlightNewSeason { title: format!("Invented {FINISHED}") }
    );
    assert_eq!(spotlight.action, HomeAction::Play);
    assert_eq!(spotlight.line, None);
    // The spotlight's title is on no shelf below it: not Up next for you, not This season.
    assert!(
        !everywhere(home).iter().skip(1).any(|n| *n == NEW_SEASON),
        "{:?}",
        everywhere(home)
    );
    let mut shown: Vec<_> = sent.iter().filter_map(spotlit).collect();
    shown.dedup();
    assert_eq!(shown, [(NEW_SEASON, SpotlightCase::Suggested)], "decided once");
}

#[tokio::test]
async fn the_popular_pick_is_the_most_popular_title_out_that_may_be_offered() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let home = last(&mut h).await;

    // The sequel of an unwatched title is passed over, and so is the one not out yet.
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    let spotlight = home.spotlight.as_ref().unwrap();
    assert_eq!(spotlight.label, Msg::HomeSpotlightPopular);
    assert_eq!(spotlight.action, HomeAction::Play);
    let this = shelf(&home, SectionKind::ThisSeason);
    assert_eq!(this, [POP_UNAIRED, POP_ONE, POP_TWO, CAUGHT_UP]);
    // The spoiler rule holds on every shelf.
    assert!(!everywhere(&home).contains(&POP_SEQUEL));
    // What is not out yet offers Planning, with its start.
    let this_season = &home.section(SectionKind::ThisSeason).unwrap().items;
    assert_eq!(this_season[1].line, Some(Msg::HomeEpisodeOutShort { episode: 1 }));
    let unaired = &this_season[0];
    assert_eq!(unaired.line, None, "the app says when it starts");
    assert_eq!(unaired.action, HomeAction::AddToPlanning);
    assert!(unaired.starts.is_some());
    assert_eq!(
        home.section(SectionKind::ThisSeason).unwrap().view_all,
        Some(HomeLink::SeasonalView)
    );
    // The season beside the heading, for the apps to word with their own season names.
    assert_eq!(home.season, Some(SeasonYear { season: Season::Fall, year: 2030 }));
}

#[tokio::test]
async fn once_the_prequel_is_finished_the_sequel_is_suggested() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    let home = last(&mut h).await;
    assert!(!everywhere(&home).contains(&POP_SEQUEL));

    // The prequel watched through: the next command routed finds the history moved.
    watch(&h, PREQUEL, 1..=12, now, 60);
    h.send(Command::LoadRecentSearches);
    let updates = h.settle().await;
    let home = homes(&updates).pop().expect("Home sent again as the history moved");

    assert_eq!(spotlit(&home), Some((POP_SEQUEL, SpotlightCase::Suggested)));
    assert_eq!(home.spotlight.unwrap().reason, Some(UpNextReason::NewSeason));
}

#[tokio::test]
async fn from_continue_to_the_popular_pick_the_spotlight_changes_once() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    watch(&h, WATCHING, 1..=3, now, 600);
    let home = last(&mut h).await;
    assert_eq!(spotlit(&home), Some((WATCHING, SpotlightCase::Continuing)));

    h.send(Command::HideFromContinue { id: id(WATCHING) });
    let updates = h.settle().await;
    let sent = homes(&updates);

    assert!(!sent.is_empty(), "Home is sent again in the same update");
    let mut shown: Vec<_> = sent.iter().map(spotlit).collect();
    shown.dedup();
    assert_eq!(shown, [Some((POP_ONE, SpotlightCase::Popular))]);
    assert!(sent.last().unwrap().section(SectionKind::Continue).is_none());
}

#[tokio::test]
async fn the_season_turns_on_its_start_date_and_the_pick_waits_for_an_episode() {
    let now = noon(2030, 12, 31);
    let mut h = at(now, None).await;
    season(&h, [show(WINTER_UNAIRED, "NOT_YET_RELEASED", ("WINTER", 2031), 2_000, &[])]);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let home = last(&mut h).await;
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    // The day before winter: its shelf, first after Continue would be.
    let starting = home.section(SectionKind::Starting).expect("the next season's shelf");
    assert_eq!(starting.title, Msg::HomeStartingWinter);
    assert_eq!(shelf(&home, SectionKind::Starting), [WINTER_UNAIRED]);

    // 1 January: winter, with nothing of it out. The pick stays with the autumn.
    h.advance_engine_clock(DAY);
    h.server().advance(DAY);
    let home = last(&mut h).await;
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    assert_eq!(shelf(&home, SectionKind::ThisSeason), [WINTER_UNAIRED]);
    assert!(home.section(SectionKind::Starting).is_none(), "spring is three months off");
}

#[tokio::test]
async fn on_the_first_day_of_a_season_with_an_episode_out_the_pick_is_new() {
    let now = noon(2031, 1, 1);
    let mut h = at(now, None).await;
    season(
        &h,
        [
            show(WINTER_UNAIRED, "NOT_YET_RELEASED", ("WINTER", 2031), 2_000, &[]),
            show(WINTER_AIRING, "RELEASING", ("WINTER", 2031), 100, &[]),
        ],
    );
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let home = last(&mut h).await;

    assert_eq!(spotlit(&home), Some((WINTER_AIRING, SpotlightCase::Popular)));
    // The season before was not asked for.
    let asked = h.server().requests_for(Operation::Search);
    assert!(asked.is_empty(), "no search for the autumn: {}", asked.len());
}

#[tokio::test]
async fn a_return_within_the_half_hour_asks_nothing() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    watch(&h, WATCHING, 1..=3, now, 600);

    let first = last(&mut h).await;
    let cold = h.server().requests().len();
    let again = last(&mut h).await;
    let warm = h.server().requests().len() - cold;

    assert!(cold <= 6, "a cold Home asks {cold} times");
    assert_eq!(warm, 0, "a warm Home asks nothing");
    assert_eq!(again, first, "and is the same Home");
}

#[tokio::test]
async fn a_failure_says_so_and_what_was_shown_stays() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    let shown = last(&mut h).await;
    assert!(!shown.failed);

    h.advance_engine_clock(31 * 60);
    h.server().fail_always(Failure::Status(500));
    let sent = load(&mut h).await;
    let home = sent.last().unwrap();

    assert!(sent.iter().any(|home| home.failed), "a send says a part failed");
    assert_eq!(shelf(home, SectionKind::ThisSeason), shelf(&shown, SectionKind::ThisSeason));
    assert_eq!(spotlit(home), spotlit(&shown));
}

#[tokio::test]
async fn a_cold_failure_still_ends_and_says_so() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    h.server().fail_always(Failure::Status(500));

    let sent = load(&mut h).await;
    let home = sent.last().unwrap();

    assert!(sent.iter().any(|home| home.failed));
    assert!(home.pending.is_empty(), "nothing is left loading");
    assert!(home.end.is_some());
    assert_eq!(home.season, None, "no This season, so no season beside it");
}

#[tokio::test]
async fn offline_shows_what_was_kept_and_since_when_and_asks_nothing() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    let shown = last(&mut h).await;
    assert!(!shown.offline);

    h.send(Command::NetworkPath { cellular: false, constrained: false, offline: true });
    h.settle().await;
    h.advance_engine_clock(3 * 3_600);
    let before = h.server().requests().len();
    let home = last(&mut h).await;

    assert!(home.offline);
    let since = home.offline_since.expect("when what is shown was fetched");
    assert!((since - now).abs() < 60, "{since} against {now}");
    assert_eq!(shelf(&home, SectionKind::ThisSeason), shelf(&shown, SectionKind::ThisSeason));
    assert_eq!(h.server().requests().len(), before, "nothing asked offline");
}

#[tokio::test]
async fn offline_says_when_the_oldest_part_shown_was_fetched() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    let first = last(&mut h).await;
    assert!(first.section(SectionKind::ThisSeason).is_some());

    // Twenty minutes on, the history moves: the suggestions are fetched again, the season kept.
    h.advance_engine_clock(20 * 60);
    watch(&h, PREQUEL, 1..=12, now + 20 * 60, 60);
    h.send(Command::LoadRecentSearches);
    let updates = h.settle().await;
    let moved = homes(&updates).pop().expect("Home sent again as the history moved");
    assert_eq!(spotlit(&moved), Some((POP_SEQUEL, SpotlightCase::Suggested)));

    h.send(Command::NetworkPath { cellular: false, constrained: false, offline: true });
    h.settle().await;
    let home = last(&mut h).await;
    let since = home.offline_since.expect("when what is shown was fetched");
    assert!((since - now).abs() < 60, "the season's time, not the newer suggestions': {since}");
}

#[tokio::test]
async fn not_interested_takes_a_title_off_home_and_undo_puts_it_back() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    let home = last(&mut h).await;
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    h.send(Command::LoadHiddenTitles);
    let updates = h.settle().await;
    assert!(updates.iter().any(|u| matches!(u, Event::HiddenTitles(rows) if rows.is_empty())));
    let before = h.server().requests().len();

    h.send(Command::DismissUpNext { id: id(POP_ONE) });
    let updates = h.settle().await;
    let home = homes(&updates).pop().expect("Home sent again");
    assert_eq!(spotlit(&home), Some((POP_TWO, SpotlightCase::Popular)));
    assert!(!everywhere(&home).contains(&POP_ONE));
    let hidden: Vec<HiddenTitle> = updates
        .iter()
        .find_map(|u| match u {
            Event::HiddenTitles(rows) => Some(rows.clone()),
            _ => None,
        })
        .expect("Settings, Hidden titles told");
    assert_eq!(hidden.iter().map(|r| r.entry.id.get()).collect::<Vec<_>>(), [POP_ONE]);
    assert!(!hidden[0].until_airs, "an airing title is hidden for good");
    assert_eq!(h.server().requests().len(), before, "taken off from what is kept");

    h.send(Command::RestoreUpNext { id: id(POP_ONE) });
    let updates = h.settle().await;
    let home = homes(&updates).pop().expect("Home sent again");
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    assert!(updates.iter().any(|u| matches!(u, Event::HiddenTitles(rows) if rows.is_empty())));
}

#[tokio::test]
async fn a_title_dismissed_before_it_airs_is_hidden_until_it_does() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    last(&mut h).await;
    h.send(Command::LoadHiddenTitles);
    h.settle().await;

    h.send(Command::DismissUpNext { id: id(POP_UNAIRED) });
    let updates = h.settle().await;

    let home = homes(&updates).pop().expect("Home sent again");
    assert!(!everywhere(&home).contains(&POP_UNAIRED));
    let until_airs = updates.iter().find_map(|u| match u {
        Event::HiddenTitles(rows) => rows.first().map(|r| r.until_airs),
        _ => None,
    });
    assert_eq!(until_airs, Some(true));
}

/// The fixture source's search, answered by one made-up source.
fn source(
    hits: Vec<SearchHit>,
    latency: Duration,
) -> (ProviderRegistry, Arc<std::sync::atomic::AtomicUsize>) {
    let provider = MockProvider::new("source.example").with_hits(hits).with_latency(latency);
    let calls = provider.call_count();
    (ProviderRegistry::new(vec![provider.arc()]), calls)
}

fn probed(updates: &[Event], n: u32) -> Option<Playable> {
    updates.iter().find_map(|u| match u {
        Event::TitlePlayable { id, playable } if id.get() == n => Some(*playable),
        _ => None,
    })
}

#[tokio::test]
async fn a_title_no_source_has_is_marked_and_offers_planning_instead() {
    let now = autumn();
    let (registry, calls) = source(Vec::new(), Duration::ZERO);
    let mut h = at(now, Some(registry)).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    last(&mut h).await;

    h.send(Command::ProbeTitle { id: id(POP_ONE) });
    let updates = h.settle().await;

    assert_eq!(probed(&updates, POP_ONE), Some(Playable::NotFound));
    let home = homes(&updates).pop().expect("Home sent again with the miss");
    let spotlight = home.spotlight.unwrap();
    assert_eq!(spotlight.entry.id.get(), POP_ONE);
    assert_eq!(spotlight.playable, Playable::NotFound);
    assert_eq!(spotlight.action, HomeAction::AddToPlanning);

    // Kept for a day: asked again a minute later, the sources are not.
    let asked = calls.load(Ordering::SeqCst);
    h.advance_engine_clock(61);
    h.send(Command::ProbeTitle { id: id(POP_ONE) });
    let updates = h.settle().await;
    assert_eq!(probed(&updates, POP_ONE), Some(Playable::NotFound));
    assert_eq!(calls.load(Ordering::SeqCst), asked);
}

#[tokio::test]
async fn a_source_that_has_the_title_keeps_its_match() {
    let now = autumn();
    let hit = SearchHit {
        episode_count: Some(12),
        year: Some(2030),
        ..SearchHit::new("invented-940102", format!("Invented {POP_ONE}"))
    };
    let (registry, _) = source(vec![hit], Duration::ZERO);
    let mut h = at(now, Some(registry)).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    last(&mut h).await;

    h.send(Command::ProbeTitle { id: id(POP_ONE) });
    let updates = h.settle().await;

    assert_eq!(probed(&updates, POP_ONE), Some(Playable::Found));
    let kept = h.store().lookup_mapping(id(POP_ONE), "source.example").unwrap();
    assert!(kept.is_some(), "the match is kept for the play after");
}

#[tokio::test]
async fn a_source_that_does_not_answer_in_time_marks_nothing() {
    let now = autumn();
    let (registry, _) = source(Vec::new(), Duration::from_secs(30));
    let mut h = at(now, Some(registry)).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);
    last(&mut h).await;

    let started = std::time::Instant::now();
    h.send(Command::ProbeTitle { id: id(POP_ONE) });
    let updates = h.settle_until(|u| matches!(u, Event::TitlePlayable { .. })).await;

    assert_eq!(probed(&updates, POP_ONE), Some(Playable::Unknown));
    assert!(started.elapsed() < Duration::from_secs(6));
}

#[tokio::test]
async fn no_title_is_on_two_discovery_shelves() {
    let now = noon(2030, 12, 25);
    let mut h = at(now, None).await;
    let mut finished =
        show(FINISHED, "FINISHED", ("SUMMER", 2030), 10, &[("SEQUEL", NEW_SEASON)]);
    finished["averageScore"] = json!(82);
    season(
        &h,
        [
            show(NEW_SEASON, "RELEASING", ("FALL", 2030), 60, &[("PREQUEL", FINISHED)]),
            show(WINTER_UNAIRED, "NOT_YET_RELEASED", ("WINTER", 2031), 2_000, &[]),
        ],
    );
    h.server().add_titles([finished]);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let home = last(&mut h).await;

    let discovery: Vec<u32> = home
        .sections
        .iter()
        .filter(|s| s.kind != SectionKind::Continue && s.kind != SectionKind::ThisWeek)
        .flat_map(|s| s.items.iter().map(|i| i.entry.id.get()))
        .collect();
    let mut unique = discovery.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), discovery.len(), "{discovery:?}");
}

#[tokio::test]
async fn a_first_run_signed_out_has_what_airs_today_in_place_of_the_week() {
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);

    let home = last(&mut h).await;

    assert!(home.section(SectionKind::ThisWeek).is_none());
    assert!(home.section(SectionKind::Continue).is_none());
    assert_eq!(spotlit(&home), Some((POP_ONE, SpotlightCase::Popular)));
    assert_eq!(home.end, Some(HomeEnd::SignIn));
}

#[tokio::test]
async fn this_week_has_the_followed_shows_from_today_in_order() {
    use anistream_testkit::fixtures::AIRING;
    let now = autumn();
    let mut h = at(now, None).await;
    season(&h, []);
    anistream_testkit::seed::watched(h.store(), AIRING, 1, now - 600);

    let home = last(&mut h).await;

    let week = home.section(SectionKind::ThisWeek).expect("the week");
    assert_eq!(week.view_all, Some(HomeLink::CalendarView));
    assert!(week.items.iter().all(|i| i.entry.id == AIRING));
    let start = now - now.rem_euclid(DAY);
    let times: Vec<i64> = week.items.iter().map(|i| i.airs_at.unwrap()).collect();
    assert!(times.windows(2).all(|w| w[0] <= w[1]), "in order: {times:?}");
    assert!(times.iter().all(|t| *t >= start && *t < start + 7 * DAY), "{times:?}");
    for item in &week.items {
        let day = ((item.airs_at.unwrap() - start) / DAY) as u32;
        assert_eq!(item.day, Some(day));
        let out = item.airs_at.unwrap() <= now;
        assert_eq!(item.action == HomeAction::Play, out);
        assert_eq!(matches!(item.line, Some(Msg::HomeOutAgo { .. })), out);
    }
}

#[tokio::test]
async fn with_no_source_the_closing_line_says_to_add_one() {
    let now = autumn();
    let mut h = at(now, Some(ProviderRegistry::new(Vec::new()))).await;
    season(&h, []);
    watch(&h, FINISHED, 1..=12, now, 30 * DAY);

    let home = last(&mut h).await;

    assert_eq!(home.end, Some(HomeEnd::NoSources));
}
