//! What a playback leads to on its own, which the engine starts: the next episode an ending
//! rolls on to, past filler when the settings skip it; an episode picked in the player, or
//! stepped to with the terminal's n and N; the episode restarted in the other translation.
//! And what does not roll on, a play asked for here, and quitting while a play is found.
//!
//! Played from a source with no stream, so a play is started, staged, looked up, and fails
//! before any player starts. What was started is what the engine staged: the
//! `PlaybackStaged` that follows the event that led to it.

use anistream_core::{ids::AnilistId, media::Translation};
use anistream_model::{
    command::Command, command::PlayerCommand, event::Event, playback::Ending,
};
use anistream_testkit::{
    fixtures::{FINISHED, FIXTURE_NOW, SEQUEL, runtime_secs, title},
    seed,
};
use anistream_tui::{keymap::Action, nav::StageView};

use super::{
    progress::{
        PLAYBACK, at_the_table_with, cursor, opened, seed_partway, stage_of, streamless,
    },
    toasts,
};
use crate::harness::{HEADLESS_MPV, Harness, Options, mpv_here, names};

/// What the lookup of a play that has no stream says, once the play is staged.
const LOOKED_UP: [&str; 6] =
    ["JobNote", "JobNote", "Toast", "PlaybackEnded", "Status", "JobDone"];

/// What the engine staged: whose, under what name, which episode, and the episode's own name.
/// The one `PlaybackStaged` among `updates`, at the speed nothing remembered.
#[track_caller]
fn staged(updates: &[Event]) -> (AnilistId, String, String, Option<String>) {
    let mut staged = updates.iter().filter_map(|update| match update {
        Event::PlaybackStaged { id, title, episode, episode_title, speed } => {
            assert_eq!(*speed, 1.0, "no speed was remembered");
            Some((*id, title.clone(), episode.clone(), episode_title.clone()))
        }
        _ => None,
    });
    let first = staged.next().expect("a play was staged");
    assert!(staged.next().is_none(), "one play staged: {:?}", names(updates));
    first
}

/// Whether each ending among `updates` rolled on, as the engine says, in order.
fn rolled_on(updates: &[Event]) -> Vec<bool> {
    updates
        .iter()
        .filter_map(|update| match update {
            Event::PlaybackEnded { rolling_on, .. } => Some(*rolling_on),
            _ => None,
        })
        .collect()
}

/// The App on [`FINISHED`]'s table, as `progress` has it, with a source that has no stream.
async fn at_the_table() -> Harness {
    at_the_table_with(Options { registry: Some(streamless()), ..Options::default() }).await
}

/// Episode `episode` of [`FINISHED`] on Now Playing, playing in [`PLAYBACK`] and `position`
/// seconds in.
async fn playing(h: &mut Harness, episode: &str, position: f64) {
    stage_of(h, FINISHED, episode);
    let tick =
        Event::Playback { position, duration: Some(runtime_secs(FINISHED)), paused: false };
    h.playback_says(PLAYBACK, FINISHED, episode, tick);
    h.settle().await;
}

/// The ending of an episode mpv played to its end, naming the episode the playback kept its
/// window for, if it kept it.
fn complete(next: Option<&str>) -> Event {
    Event::playback_ended(true, Ending::Complete { next: next.map(Into::into) })
}

#[tokio::test]
async fn an_ending_that_rolls_on_stages_the_next_episode_under_the_name_it_played_under() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 1_400.0).await;
    h.playback_says(PLAYBACK, FINISHED, "4", complete(Some("5")));
    let updates = h.settle().await;

    let rolled = ["Progress", "PlaybackEnded", "PlaybackStaged"];
    assert_eq!(names(&updates), [&rolled[..], &LOOKED_UP].concat());
    assert_eq!(rolled_on(&updates), [true, false], "4 rolls on; 5, found nothing, does not");
    let what =
        (FINISHED, title(FINISHED), "5".into(), Some("What the Beacons Remember".into()));
    assert_eq!(staged(&updates), what);
    assert!(!toasts(&updates).iter().any(|t| t.contains("filler")), "nothing was skipped");
    assert!(h.app().playing.is_none(), "found nothing, so nothing is left staged");
}

/// The App on [`SEQUEL`]'s table, reached from Home, on `config`, its fifth episode watched:
/// the source says its seventh is filler.
async fn at_the_sequel(config: Option<&str>) -> Harness {
    let options = Options {
        config: config.map(Into::into),
        registry: Some(streamless()),
        ..Options::default()
    };
    let mut h = Harness::with(options).await;
    seed::watched(h.store(), SEQUEL, 5, FIXTURE_NOW - 7_200);
    opened(&mut h).await;
    assert_eq!(h.app().detail.as_ref().map(|d| d.id), Some(SEQUEL));
    h.press(Action::ShowEpisodes);
    h.settle().await;
    h
}

/// Episode 6 of [`SEQUEL`] played to its end, with the window kept for 7: as a playback says
/// it when it learned its list after it started, which names the episode after it as it comes.
async fn sixth_finished(h: &mut Harness) -> Vec<Event> {
    stage_of(h, SEQUEL, "6");
    let tick = Event::Playback {
        position: 1_400.0,
        duration: Some(runtime_secs(SEQUEL)),
        paused: false,
    };
    h.playback_says(PLAYBACK, SEQUEL, "6", tick);
    h.playback_says(PLAYBACK, SEQUEL, "6", complete(Some("7")));
    h.settle().await
}

#[tokio::test]
async fn filler_is_stepped_over_when_the_settings_skip_it() {
    let mut h = at_the_sequel(Some("[playback]\nskip_filler = true\n")).await;
    let updates = sixth_finished(&mut h).await;
    assert_eq!(staged(&updates), (SEQUEL, title(SEQUEL), "8".into(), None));
    assert_eq!(rolled_on(&updates), [true, false]);
    // Said before the staging it explains.
    let toast = updates.iter().position(|u| matches!(u, Event::Toast(_))).expect("a toast");
    let stage = updates.iter().position(|u| matches!(u, Event::PlaybackStaged { .. }));
    assert!(Some(toast) < stage, "{:?}", names(&updates));
    assert_eq!(toasts(&updates)[0], "info: skipped filler — ep 8 next");
}

#[tokio::test]
async fn filler_is_played_when_the_settings_do_not_skip_it() {
    let mut h = at_the_sequel(None).await;
    let updates = sixth_finished(&mut h).await;
    assert_eq!(staged(&updates), (SEQUEL, title(SEQUEL), "7".into(), None));
    assert!(!toasts(&updates).iter().any(|t| t.contains("filler")));
}

/// An ending that names no next episode: the playback kept no window, whether because Up Next
/// was cancelled, auto-next is off, or nothing follows. Nothing is staged, and Now Playing
/// comes down with a word that the episode is done.
#[track_caller]
fn the_binge_ends_here(h: &Harness, updates: &[Event]) {
    assert_eq!(names(updates), ["Progress", "PlaybackEnded"]);
    assert_eq!(rolled_on(updates), [false]);
    let said = |text: &str| h.app().toasts.iter().any(|toast| toast.notice.text == text);
    assert!(said("episode finished"), "the screen says the episode is done");
    assert!(h.app().playing.is_none());
    assert_ne!(h.app().nav.current(), &StageView::NowPlaying);
}

#[tokio::test]
async fn a_cancelled_up_next_ends_the_binge_at_this_episode() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 1_400.0).await;
    h.playback_says(PLAYBACK, FINISHED, "4", complete(None));
    let updates = h.settle().await;
    the_binge_ends_here(&h, &updates);
}

#[tokio::test]
async fn nothing_rolls_on_from_the_last_episode() {
    let mut h = at_the_table().await;
    playing(&mut h, "12", 1_400.0).await;
    h.playback_says(PLAYBACK, FINISHED, "12", complete(None));
    let updates = h.settle().await;
    the_binge_ends_here(&h, &updates);
}

#[tokio::test]
async fn an_episode_picked_in_the_player_goes_by_the_tables_name_when_its_list_has_none() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 200.0).await;
    let picked = Event::PlayerPlayEpisode { episode: "6".into(), title: None };
    h.playback_says(PLAYBACK, FINISHED, "4", picked);
    let updates = h.settle().await;
    assert_eq!(
        names(&updates),
        [&["PlayerPlayEpisode", "PlaybackStaged"][..], &LOOKED_UP].concat()
    );
    let what =
        (FINISHED, title(FINISHED), "6".into(), Some("The Cartographer's Daughter".into()));
    assert_eq!(staged(&updates), what);
}

/// The terminal's `action` on Now Playing, which asks the playback for the episode beside
/// it; and the playback's answer, which these tests give for it, having no player.
async fn step(action: Action, delta: i64, answer: &str) -> Vec<Event> {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 200.0).await;
    let task = h.app_mut().handle(action, 20);
    assert_eq!(
        task,
        Some(Command::Player(PlayerCommand::Step(delta))),
        "asked of the playback"
    );
    h.send(task.expect("a step"));
    let picked = Event::PlayerPlayEpisode { episode: answer.into(), title: None };
    h.playback_says(PLAYBACK, FINISHED, "4", picked);
    let updates = h.settle().await;
    assert_eq!(staged(&updates).2, answer);
    updates
}

#[tokio::test]
async fn n_steps_to_the_next_episode_of_the_title_playing() {
    step(Action::NextEpisode, 1, "5").await;
}

#[tokio::test]
async fn shift_n_steps_back_to_the_episode_before() {
    let updates = step(Action::PreviousEpisode, -1, "3").await;
    assert_eq!(staged(&updates).3.as_deref(), Some("The Ferryman's Ledger"));
}

#[tokio::test]
async fn sub_or_dub_picked_in_the_player_restarts_the_episode_in_it() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 600.0).await;
    h.playback_says(PLAYBACK, FINISHED, "4", Event::PlayerTranslation(Translation::Dub));
    let updates = h.settle().await;

    // The notice and the restart, then the session's choice of dub said whole.
    let restarted = ["PlayerTranslation", "Toast", "PlaybackStaged", "Settings"];
    assert_eq!(names(&updates), [&restarted[..], &LOOKED_UP].concat());
    assert_eq!(toasts(&updates)[0], "info: now using dub");
    let what = (FINISHED, title(FINISHED), "4".into(), Some("Embers in the Snow".into()));
    assert_eq!(staged(&updates), what);
    assert_eq!(h.app().settings.translation, Translation::Dub);
}

#[tokio::test]
async fn quitting_while_the_next_episode_is_found_does_not_wait() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 1_400.0).await;
    h.playback_says(PLAYBACK, FINISHED, "4", complete(Some("5")));
    let updates = h.settle_until(|e| matches!(e, Event::PlaybackStaged { .. })).await;
    assert_eq!(staged(&updates).2, "5");
    // Nothing is under way: 4 has ended, and 5 is still being found.
    let took = h.quit().await;
    assert!(took < std::time::Duration::from_millis(500), "waited {took:?}");
}

#[tokio::test]
async fn quitting_partway_through_an_episode_has_one_under_way() {
    let mut h = at_the_table().await;
    playing(&mut h, "4", 600.0).await;
    // Under way, as the engine and the loop both say; with no player to stop, nothing waits.
    let took = h.quit().await;
    assert!(took < std::time::Duration::from_millis(500), "waited {took:?}");
}

#[tokio::test]
async fn a_play_asked_for_on_the_table_is_staged_alike_here_and_by_the_engine() {
    let mut h = at_the_table().await;
    assert_eq!(cursor(&h), "4");
    h.press_to_play(Action::Open);
    let updates = h.settle().await;
    assert_eq!(names(&updates), [&["PlaybackStaged"][..], &LOOKED_UP].concat());
    let what = (FINISHED, title(FINISHED), "4".into(), Some("Embers in the Snow".into()));
    assert_eq!(staged(&updates), what);
}

#[tokio::test]
async fn a_play_resumed_from_continue_is_staged_alike_here_and_by_the_engine() {
    // Enter on Home resumes the episode left partway, by the name the row has.
    let mut h =
        Harness::with(Options { registry: Some(streamless()), ..Options::default() }).await;
    seed_partway(&h);
    h.reload();
    h.settle().await;
    h.app_mut().nav.focus_stage();
    h.press_to_play(Action::Open);
    let updates = h.settle().await;
    let what = (FINISHED, title(FINISHED), "4".into(), None);
    assert_eq!(staged(&updates), what, "no table is open to name the episode");
}

#[tokio::test]
async fn a_played_ending_after_a_pick_is_not_rolled_on_from() {
    // Episode 6 was picked while 4 played, and 4 reaches its end before 6 is found: the screen
    // has moved on to 6, so 4's ending takes nothing down and starts nothing.
    let mut h = at_the_table().await;
    playing(&mut h, "4", 1_400.0).await;
    let picked = Event::PlayerPlayEpisode { episode: "6".into(), title: None };
    h.playback_says(PLAYBACK, FINISHED, "4", picked);
    h.playback_says(PLAYBACK, FINISHED, "4", complete(Some("5")));
    let updates = h.settle().await;
    assert_eq!(staged(&updates).2, "6");
    assert!(!rolled_on(&updates)[0], "{:?}", names(&updates));
}

/// Every episode staged, in order.
fn staged_episodes(updates: &[Event]) -> Vec<String> {
    updates
        .iter()
        .filter_map(|update| match update {
            Event::PlaybackStaged { episode, .. } => Some(episode.clone()),
            _ => None,
        })
        .collect()
}

/// How many player windows were opened.
fn windows(updates: &[Event]) -> usize {
    updates.iter().filter(|u| matches!(u, Event::Status(s) if s == "starting mpv…")).count()
}

/// [`FINISHED`]'s table with ten episodes watched, to play for real: mpv with no window and no
/// sound, and the fixture source's clip for every episode. `None` where there is no mpv.
async fn before_the_finale_for_real() -> Option<Harness> {
    if !mpv_here().await {
        return None;
    }
    let options = Options { config: Some(HEADLESS_MPV.into()), ..Options::default() };
    let mut h = Harness::with(options).await;
    seed::watched(h.store(), FINISHED, 10, FIXTURE_NOW - 86_400);
    opened(&mut h).await;
    h.press(Action::ShowEpisodes);
    h.settle().await;
    assert_eq!(cursor(&h), "11");
    Some(h)
}

#[tokio::test]
async fn a_binge_rolls_on_in_the_window_the_last_episode_left() {
    let Some(mut h) = before_the_finale_for_real().await else { return };
    h.press_to_play(Action::Open);
    let updates = h.settle().await;

    // 11 plays to its end and rolls on to 12, the finale, which ends the binge: one window.
    assert_eq!(staged_episodes(&updates), ["11", "12"]);
    assert_eq!(rolled_on(&updates), [true, false], "{:?}", names(&updates));
    assert_eq!(windows(&updates), 1, "{:?}", names(&updates));
    let finished = h.app().episodes.iter().filter(|row| row.completed).count();
    assert_eq!(finished, 12, "both recorded as watched");
    assert!(h.app().playing.is_none());
    assert!(h.app().toasts.iter().any(|toast| toast.notice.text == "episode finished"));
}

#[tokio::test]
async fn n_hands_the_window_to_the_next_episode() {
    let Some(mut h) = before_the_finale_for_real().await else { return };
    h.press_to_play(Action::Open);
    // Once 11 is playing, n asks its playback for the episode after it.
    let started =
        h.settle_until(|e| matches!(e, Event::Playback { duration: Some(_), .. })).await;
    assert_eq!(windows(&started), 1);
    h.press(Action::NextEpisode);
    let updates = h.settle().await;

    assert_eq!(staged_episodes(&updates), ["12"]);
    let left = updates
        .iter()
        .any(|u| matches!(u, Event::EpisodeLeft { episode, .. } if episode == "11"));
    assert!(left, "11 handed its window over: {:?}", names(&updates));
    assert_eq!(windows(&updates), 0, "in the same window");
    assert_eq!(rolled_on(&updates), [false], "12 is the finale");
    assert!(h.app().playing.is_none());
}
