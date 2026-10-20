//! A title's progress, after a mark and as a playback goes: what the engine says of it, and
//! what the table, the title and Continue show once it is done.
//!
//! Marked on the table, an episode is written and the title moves on with it: its count, the
//! episode Continue offers, and the row. The engine answers a mark with `Progress`, the title
//! whole. A playback moves the row's meter and the resume point as it plays, and the count and
//! Continue by how it ended, each said with `Progress` just ahead of the event that moved it.
//! Only the title and the episode the playback is of, whatever else is on screen; and nothing
//! after it has ended.

use anistream_core::ids::AnilistId;
use anistream_engine::cache::PlayKind;
use anistream_model::{
    catalog::{Entry, ResumePoint},
    event::Event,
    playback::Ending,
};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::{
    fixtures::{self, FINISHED, FIXTURE_NOW, SEQUEL, runtime_secs, title},
    seed, sources,
};
use anistream_tui::{app::NowPlaying, keymap::Action};

use super::toasts;
use crate::harness::{Harness, Options, names};

/// Three episodes of [`FINISHED`] watched two days ago and the fourth stopped five minutes in,
/// an hour ago: Home has it to continue.
pub(super) fn seed_partway(h: &Harness) {
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - 2 * 86_400);
    seed::stopped_at(h.store(), FINISHED, "4", 300.0, FIXTURE_NOW - 3_600);
}

/// Home loaded, and the first title on it opened, as Enter on a row with nothing to resume
/// opens it: Enter itself plays a row with something to resume.
pub(super) async fn opened(h: &mut Harness) {
    h.reload();
    h.settle().await;
    h.press(Action::Open);
    let task = h.app_mut().open_selected().expect("Home has a title to open");
    h.send(task);
    h.settle().await;
}

/// The App on [`FINISHED`]'s episode table, reached from Home, its cursor on episode 4.
async fn at_the_table() -> Harness {
    at_the_table_with(Options::default()).await
}

/// [`at_the_table`], set up as `options` say.
pub(super) async fn at_the_table_with(options: Options) -> Harness {
    let mut h = Harness::with(options).await;
    seed_partway(&h);
    opened(&mut h).await;
    h.press(Action::ShowEpisodes);
    h.settle().await;
    assert_eq!(cursor(&h), "4", "the table opens on the first episode not finished");
    h
}

/// The episode under the table's cursor.
pub(super) fn cursor(h: &Harness) -> String {
    let app = h.app();
    app.episodes[app.episode_selected].number.clone()
}

/// Each row of the table that is finished, by number.
fn finished(h: &Harness) -> Vec<String> {
    h.app().episodes.iter().filter(|r| r.completed).map(|r| r.number.clone()).collect()
}

/// An episode's meter on the table.
fn meter(h: &Harness, episode: &str) -> f64 {
    h.app().episodes.iter().find(|r| r.number == episode).expect("a row").watched
}

/// Every entry of `id` on screen: the title's page, and its rows in the list under it.
fn entries(h: &Harness, id: AnilistId) -> Vec<&Entry> {
    let app = h.app();
    app.detail.iter().chain(app.content.entries()).filter(|e| e.id == id).collect()
}

/// What every entry of `id` on screen says of it: episodes done and the one Continue offers,
/// and where Continue resumes it. One answer, however many places show it.
#[track_caller]
fn standing(h: &Harness, id: AnilistId) -> (Option<(u32, u32)>, Option<f64>) {
    let all: Vec<_> =
        entries(h, id).iter().map(|e| (e.progress, e.resume.map(|r| r.position))).collect();
    assert!(!all.is_empty(), "{id:?} is on screen");
    assert!(all.windows(2).all(|pair| pair[0] == pair[1]), "one answer everywhere: {all:?}");
    all[0]
}

/// Now Playing as the App stages it for `episode` of [`FINISHED`]: these tests have no player,
/// so nothing else would stage it.
fn stage(h: &mut Harness, episode: &str) {
    stage_of(h, FINISHED, episode);
}

/// [`stage`], for `episode` of `id`.
pub(super) fn stage_of(h: &mut Harness, id: AnilistId, episode: &str) {
    h.app_mut().playing = Some(NowPlaying {
        id: Some(id),
        title: title(id),
        episode: episode.into(),
        speed: 1.0,
        ..NowPlaying::default()
    });
}

/// The playback the tests play episode 4 in.
pub(super) const PLAYBACK: u64 = 1_000_001;

/// A tick of the playhead from [`PLAYBACK`].
pub(super) fn at(h: &mut Harness, position: f64) {
    let duration = Some(runtime_secs(FINISHED));
    let tick = Event::Playback { position, duration, paused: false };
    h.playback_says(PLAYBACK, FINISHED, "4", tick);
}

#[tokio::test]
async fn a_mark_on_the_table_moves_the_row_the_title_and_continue() {
    let mut h = at_the_table().await;
    assert_eq!(standing(&h, FINISHED), (Some((3, 4)), Some(300.0)));

    // Written, then answered with where the title now stands.
    h.press(Action::ToggleWatched);
    assert!(h.app().awaiting_mark(), "the App waits for the engine's word");
    assert_eq!(names(&h.settle().await), ["Progress"]);
    assert!(!h.app().awaiting_mark());
    assert_eq!(finished(&h), ["1", "2", "3", "4"]);
    assert_eq!(meter(&h, "4"), 1.0, "a marked episode reads as finished");
    assert_eq!(standing(&h, FINISHED), (Some((4, 5)), None), "Continue moves past it");

    // Marked again, it is not watched after all: Continue goes back to it, with nothing to
    // resume, since the store forgot where it was left too.
    h.press(Action::ToggleWatched);
    assert_eq!(names(&h.settle().await), ["Progress"]);
    assert_eq!(finished(&h), ["1", "2", "3"]);
    assert_eq!(meter(&h, "4"), 0.0);
    assert_eq!(standing(&h, FINISHED), (Some((3, 4)), None));

    // Caught up to episode 7: everything before it marked, in one go.
    for _ in 0..3 {
        h.press(Action::Down);
    }
    assert_eq!(cursor(&h), "7");
    h.press(Action::MarkAllPrevious);
    assert_eq!(names(&h.settle().await), ["Progress"]);
    assert_eq!(finished(&h), ["1", "2", "3", "4", "5", "6"]);
    assert_eq!(standing(&h, FINISHED), (Some((6, 7)), None));
    let mut held = h.store().completed_episodes(FINISHED).unwrap();
    held.sort();
    assert_eq!(held, ["1", "2", "3", "4", "5", "6"], "and written");
}

#[tokio::test]
async fn a_playback_moves_the_table_and_continue_as_it_plays_and_as_it_ends() {
    let mut h = at_the_table().await;
    stage(&mut h, "4");
    h.playback_says(PLAYBACK, FINISHED, "4", Event::Resumed { position: 300.0 });
    at(&mut h, 720.0);
    // Where the title stands is said ahead of each tick that moves it.
    assert_eq!(names(&h.settle().await), ["Resumed", "Progress", "Playback"]);
    assert_eq!(meter(&h, "4"), 0.5, "the row follows the playhead while detached");
    assert_eq!(standing(&h, FINISHED), (Some((3, 4)), Some(720.0)), "and so does Continue");

    at(&mut h, 1_300.0);
    let ended = Event::playback_ended(true, Ending::Closed);
    h.playback_says(PLAYBACK, FINISHED, "4", ended);
    // And ahead of the ending, so the screen it drops back to shows the episode finished.
    assert_eq!(names(&h.settle().await), ["Progress", "Playback", "Progress", "PlaybackEnded"]);
    assert_eq!(finished(&h), ["1", "2", "3", "4"]);
    assert_eq!(meter(&h, "4"), 1.0, "finished reads as finished, whatever the last tick said");
    assert_eq!(standing(&h, FINISHED), (Some((4, 5)), None));

    // A tick that straggles in after the ending is not a playback coming back.
    at(&mut h, 100.0);
    assert_eq!(names(&h.settle().await), ["Playback"]);
    assert_eq!(finished(&h), ["1", "2", "3", "4"]);
    assert_eq!(standing(&h, FINISHED), (Some((4, 5)), None));
}

#[tokio::test]
async fn a_window_handed_over_mid_episode_leaves_its_watch_where_it_was() {
    let mut h = at_the_table().await;
    stage(&mut h, "4");
    at(&mut h, 400.0);
    h.settle().await;

    // Episode 5 is about to take the window; 4 says where it was as it hands over.
    let left = Event::EpisodeLeft {
        id: FINISHED,
        episode: "4".into(),
        position: 600.0,
        duration: Some(runtime_secs(FINISHED)),
        watched: false,
    };
    h.playback_says(PLAYBACK, FINISHED, "4", left);
    assert_eq!(names(&h.settle().await), ["Progress", "EpisodeLeft"]);
    assert!((meter(&h, "4") - 600.0 / runtime_secs(FINISHED)).abs() < 1e-9);
    assert_eq!(standing(&h, FINISHED), (Some((3, 4)), Some(600.0)));
    let resume = entries(&h, FINISHED)[0].resume.expect("a resume point");
    assert_eq!(resume, ResumePoint { position: 600.0, fraction: Some(600.0 / 1_440.0) });
}

#[tokio::test]
async fn a_watch_from_home_moves_only_the_row_it_is_of() {
    // Two titles on Home; the one played has no table open, and the other is not touched.
    let mut h = Harness::new().await;
    seed_partway(&h);
    seed::watched(h.store(), SEQUEL, 2, FIXTURE_NOW - 7_200);
    h.reload();
    h.settle().await;
    assert_eq!(standing(&h, SEQUEL), (Some((2, 3)), None));

    stage(&mut h, "4");
    at(&mut h, 900.0);
    let ended = Event::playback_ended(true, Ending::Stopped);
    h.playback_says(PLAYBACK, FINISHED, "4", ended);
    assert_eq!(names(&h.settle().await), ["Progress", "Playback", "Progress", "PlaybackEnded"]);
    assert_eq!(standing(&h, FINISHED), (Some((4, 5)), None));
    assert_eq!(standing(&h, SEQUEL), (Some((2, 3)), None), "another show took the watch");
}

/// The fixture source with its titles and their episodes listed, and no stream for any of
/// them: a play is looked up and fails there, with no player to start.
pub(super) fn streamless() -> ProviderRegistry {
    let mut provider = MockProvider::new(sources::PROVIDER_ID).with_hits(sources::hits());
    for id in fixtures::ALL {
        provider = provider.with_episodes_for(sources::key(id), sources::episodes(id));
    }
    ProviderRegistry::new(vec![provider.arc()])
}

#[tokio::test]
async fn an_ending_that_rolls_on_plays_the_next_episode_under_the_name_it_played_under() {
    let mut h =
        at_the_table_with(Options { registry: Some(streamless()), ..Options::default() }).await;
    stage(&mut h, "4");
    at(&mut h, 1_400.0);
    let ended = Event::playback_ended(true, Ending::Complete { next: Some("5".into()) });
    h.playback_says(PLAYBACK, FINISHED, "4", ended);
    let updates = h.settle().await;

    // Episode 5 is started and staged, and the lookup finds nothing to play.
    let looked_up = ["JobNote", "JobNote", "Toast", "PlaybackEnded", "Status", "JobDone"];
    let finished_4 = ["Progress", "Playback", "Progress", "PlaybackEnded", "PlaybackStaged"];
    assert_eq!(names(&updates), [&finished_4[..], &looked_up].concat());
    assert_eq!(finished(&h), ["1", "2", "3", "4"]);
    assert_eq!(standing(&h, FINISHED), (Some((4, 5)), None));
    assert!(h.app().playing.is_none(), "nothing is left staged");
}

#[tokio::test]
async fn an_episode_picked_in_the_player_is_asked_for_by_the_title_playing() {
    let mut h =
        at_the_table_with(Options { registry: Some(streamless()), ..Options::default() }).await;
    stage(&mut h, "4");
    at(&mut h, 200.0);
    let picked = Event::PlayerPlayEpisode {
        episode: "6".into(),
        title: Some("Rain over Kiriya Pass".into()),
    };
    h.playback_says(PLAYBACK, FINISHED, "4", picked);
    let updates = h.settle().await;

    // Started and staged, as the player asked, and looked up; there is nothing to play it from.
    let looked_up = ["JobNote", "JobNote", "Toast", "PlaybackEnded", "Status", "JobDone"];
    let picked = ["Progress", "Playback", "PlayerPlayEpisode", "PlaybackStaged"];
    assert_eq!(names(&updates), [&picked[..], &looked_up].concat());
    assert!(
        toasts(&updates).iter().any(|toast| toast.starts_with("alert: ")),
        "the lookup says why: {:?}",
        toasts(&updates)
    );
}

#[tokio::test]
async fn the_next_episode_steps_over_filler_on_a_table_that_marks_it() {
    // SEQUEL's source says episode 7 is filler: with filler skipped, what follows 6 is 8, for
    // the Up Next card as for auto-next.
    let mut h = Harness::with(Options {
        config: Some("[playback]\nskip_filler = true\n".into()),
        ..Options::default()
    })
    .await;
    seed::watched(h.store(), SEQUEL, 2, FIXTURE_NOW - 7_200);
    opened(&mut h).await;
    assert_eq!(h.app().detail.as_ref().map(|d| d.id), Some(SEQUEL));
    h.press(Action::ShowEpisodes);
    h.settle().await;
    let filler = sources::FILLER_EPISODE.to_string();
    let play = h.title_cache().await.hints(SEQUEL, "6", true, PlayKind::Staged);
    assert_eq!(play.next.map(|n| n.number).as_deref(), Some("8"), "{filler} is stepped over");
    // Shown in the Episodes menu as what it is.
    let marked: Vec<_> = play.episodes.iter().filter(|e| e.filler).map(|e| &e.number).collect();
    assert_eq!(marked, [&filler]);
}
