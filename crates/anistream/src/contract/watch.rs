//! Watching by hand, and the trackers: what is written, what is queued, and for whom.
//!
//! Marks, statuses and scores are written inline and queued for signed-in trackers only —
//! Kitsu is on by default with no account behind it, and a queue for nobody would grow a badge
//! nobody can clear. The queue goes out when the drain runs, never from the task itself.

use anistream_model::{
    catalog::LibrarySegment, command::Command, event::Event, progress::TitleProgress,
    sync::SyncState,
};
use anistream_testkit::{
    Operation,
    fixtures::{AIRING, FINISHED, FIXTURE_NOW},
    seed,
};

use super::{WITH_ANILIST, content, ends, ids, toasts};
use crate::harness::{Harness, Options, names};

/// AniList and Kitsu on, and only AniList signed in.
async fn anilist_signed_in() -> Harness {
    Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await
}

fn mark(episodes: &[&str], watched: bool) -> Command {
    Command::SetWatched {
        id: FINISHED,
        episodes: episodes.iter().map(|e| (*e).to_owned()).collect(),
        watched,
    }
}

fn state(update: &Event) -> &SyncState {
    match update {
        Event::Sync(state) => state,
        other => panic!("not a tracker's state: {other:?}"),
    }
}

fn completed(h: &Harness) -> Vec<String> {
    let mut done = h.store().completed_episodes(FINISHED).unwrap();
    done.sort();
    done
}

#[tokio::test]
async fn marking_watched_queues_progress_for_signed_in_trackers_only() {
    let mut h = anilist_signed_in().await;
    let job = h.send(mark(&["1", "2"], true));
    let updates = h.settle().await;

    assert!(job.is_none(), "written as it is asked");
    assert_eq!(names(&updates), ["Progress"], "answered, though nothing showed the title");
    assert_eq!(completed(&h), ["1", "2"]);
    assert_eq!(h.store().pending_progress("anilist", FINISHED).unwrap(), Some(2));
    assert_eq!(h.store().outbox_depth(Some("kitsu")).unwrap(), 0, "no account to queue for");
}

#[tokio::test]
async fn marking_watched_signed_out_records_history_and_queues_nothing() {
    let mut h = Harness::new().await;
    h.send(mark(&["1", "2"], true));
    assert_eq!(names(&h.settle().await), ["Progress"]);

    assert_eq!(completed(&h), ["1", "2"]);
    assert_eq!(h.store().outbox_depth(None).unwrap(), 0);
}

#[tokio::test]
async fn unmarking_forgets_history_and_never_walks_progress_back() {
    let mut h = anilist_signed_in().await;
    h.send(mark(&["1", "2", "3"], true));
    h.send(mark(&["3"], false));
    assert_eq!(names(&h.settle().await), ["Progress", "Progress"]);

    assert_eq!(completed(&h), ["1", "2"]);
    assert_eq!(h.store().pending_progress("anilist", FINISHED).unwrap(), Some(3));
    assert_eq!(h.store().outbox_depth(Some("anilist")).unwrap(), 1, "one row, coalesced");
}

#[tokio::test]
async fn a_mark_sent_before_anything_loaded_says_where_the_title_now_stands() {
    // As a native app can: marked from a screen of its own before any list or page did.
    let mut h = Harness::new().await;
    h.send(mark(&["1", "2"], true));
    let updates = h.settle().await;
    let [Event::Progress { id, progress, resume, episodes }] = &updates[..] else {
        panic!("the title's standing alone: {:?}", names(&updates))
    };
    assert_eq!(*id, FINISHED);
    assert_eq!(*progress, Some(TitleProgress { done: 2, next: 3 }));
    assert_eq!((resume, episodes.len()), (&None, 0), "no table loaded to say episodes of");

    // And an unmark the same, from what the store now holds.
    h.send(mark(&["2"], false));
    let updates = h.settle().await;
    let [Event::Progress { progress, .. }] = &updates[..] else {
        panic!("the title's standing alone: {:?}", names(&updates))
    };
    assert_eq!(*progress, Some(TitleProgress { done: 1, next: 2 }));
}

#[tokio::test]
async fn a_mark_is_in_the_table_the_next_task_loads() {
    let mut h = Harness::new().await;
    h.send(mark(&["1", "2", "3"], true));
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Progress", "Episodes", "TitleMatch", "JobDone"]);
    let Event::Episodes { rows, .. } = &updates[1] else { unreachable!() };
    let done: Vec<&str> =
        rows.iter().filter(|r| r.completed).map(|r| r.number.as_str()).collect();
    assert_eq!(done, ["1", "2", "3"]);
    ends(&updates, job);
}

#[tokio::test]
async fn a_title_taken_off_continue_stays_off_for_the_next_load() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 7_200);
    seed::watched(h.store(), AIRING, 2, FIXTURE_NOW - 600);
    let hidden = h.send(Command::HideFromContinue { id: AIRING });
    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;

    assert!(hidden.is_none());
    // Nothing from the hiding itself, and no broadcast dates for the one airing title left out.
    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert_eq!(ids(content(&updates)), [FINISHED]);
    ends(&updates, job);
}

#[tokio::test]
async fn a_status_or_a_score_moves_the_badge_of_each_signed_in_tracker() {
    let mut h = anilist_signed_in().await;
    let job = h.send(Command::SetStatus { id: FINISHED, status: LibrarySegment::Completed });
    let updates = h.settle().await;
    assert!(job.is_none());
    assert_eq!(names(&updates), ["Sync"]);
    let badge = state(&updates[0]);
    assert_eq!(
        (badge.tracker.as_str(), badge.outbox, badge.last.as_deref()),
        ("anilist", 1, Some("queued"))
    );

    h.send(Command::SetScore { id: FINISHED, score: 8.5 });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Sync"]);
    assert_eq!(state(&updates[0]).outbox, 2);
    assert_eq!(h.store().outbox_depth(Some("kitsu")).unwrap(), 0);

    // Signed out, there is no badge to move and nothing is queued.
    let mut h = Harness::new().await;
    h.send(Command::SetStatus { id: FINISHED, status: LibrarySegment::Completed });
    h.send(Command::SetScore { id: FINISHED, score: 8.5 });
    assert_eq!(names(&h.settle().await), Vec::<&str>::new());
    assert_eq!(h.store().outbox_depth(None).unwrap(), 0);
}

#[tokio::test]
async fn syncing_now_sends_the_queue_then_pulls_the_library() {
    let mut h = anilist_signed_in().await;
    h.send(mark(&["1", "2"], true));
    h.settle().await;
    let job = h.send(Command::SyncNow);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Sync", "Sync", "JobDone"]);
    let (sent, pulled) = (state(&updates[0]), state(&updates[1]));
    assert_eq!((sent.tracker.as_str(), sent.last.as_deref()), ("anilist", Some("sent 1")));
    assert_eq!(pulled.last.as_deref(), Some("pulled 3 titles"));
    ends(&updates, job);

    let pushed = h.server().requests_for(Operation::SaveEntry);
    assert_eq!(pushed.len(), 1);
    assert_eq!(pushed[0].variables["mediaId"], FINISHED.get());
    assert_eq!(pushed[0].variables["progress"], 2);
    assert_eq!(h.store().outbox_depth(Some("anilist")).unwrap(), 0);
}

#[tokio::test]
async fn keeping_your_side_of_a_conflict_queues_it() {
    let mut h = anilist_signed_in().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - 600);
    h.send(Command::ResolveConflict { id: FINISHED, keep_local: false });
    assert_eq!(names(&h.settle().await), Vec::<&str>::new());
    assert_eq!(h.store().pending_progress("anilist", FINISHED).unwrap(), None, "theirs stands");

    h.send(Command::ResolveConflict { id: FINISHED, keep_local: true });
    assert_eq!(names(&h.settle().await), Vec::<&str>::new());
    assert_eq!(h.store().pending_progress("anilist", FINISHED).unwrap(), Some(3));
}

#[tokio::test]
async fn signing_out_says_what_was_kept_then_republishes_every_tracker() {
    let mut h = anilist_signed_in().await;
    h.send(mark(&["1"], true));
    h.settle().await;
    let job = h.send(Command::Disconnect { tracker: "anilist".into() });
    let updates = h.settle().await;

    assert!(job.is_none());
    assert_eq!(names(&updates), ["Toast", "Sync", "Sync"]);
    assert_eq!(
        toasts(&updates),
        ["info: Signed out of AniList. One queued change is kept for the next sign-in."]
    );
    let states: Vec<(&str, bool)> =
        updates[1..].iter().map(state).map(|s| (s.tracker.as_str(), s.connected)).collect();
    assert_eq!(states, [("anilist", false), ("kitsu", false)]);
    assert!(!h.tokens().has("anilist"), "the token is gone");
    assert_eq!(h.store().outbox_depth(Some("anilist")).unwrap(), 1, "the queue is kept");
}
