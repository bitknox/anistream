//! Episode discussions: an episode's AniList thread found, read, and written to as the viewer.
//!
//! The fake AniList has no threads of its own: each test makes up the one it reads, with a
//! decoy beside it that AniList's loose search would also find, so picking the thread by its
//! title is tested every time. Opening says the discussion whole as `Discussion`, first loading
//! and then read. A later episode's thread is never asked for while spoiler protection holds it
//! back; an episode watched past the point it counts is looked up behind the playback and said
//! with `DiscussionFound`.

use anistream_model::{
    Msg,
    command::Command,
    discussion::{DiscussionState, DiscussionView, PostState},
    event::Event,
};
use anistream_testkit::{
    anilist::{Failure, Operation, comment},
    fixtures::{FINISHED, FIXTURE_NOW},
    seed,
};

use super::WITH_ANILIST;
use crate::harness::{Harness, Options};

const DAY: i64 = 86_400;

/// Episode 3's thread of [`FINISHED`], two days old by the engine's clock, which is the
/// system's: two top-level comments, the first with a
/// reply that has a reply of its own. Beside it, episode 13's, more replied to, which a search
/// for "Episode 3" also finds. Returns episode 3's id.
fn threads(h: &Harness) -> u32 {
    let at = anistream_engine::now_epoch() - 2 * DAY;
    let decoys = (1..=6).map(|n| comment(n, "decoy", "the wrong episode", at, Vec::new()));
    h.server().add_thread(FINISHED, "Lantern Harbour Episode 13 Discussion", decoys.collect());
    h.server().add_thread(
        FINISHED,
        "Lantern Harbour Episode 3 Discussion [Spoilers]",
        vec![
            comment(
                11,
                "hoshi",
                "**What** a finale",
                at,
                vec![comment(
                    12,
                    "kumo",
                    "agreed",
                    at + 60,
                    vec![comment(13, "sora", "same here", at + 120, Vec::new())],
                )],
            ),
            comment(14, "sora", "~!the lighthouse was the key!~", at + 300, Vec::new()),
        ],
    )
}

/// Every discussion said, in order.
fn said(events: &[Event]) -> Vec<DiscussionView> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::Discussion(view) => Some(view.clone()),
            _ => None,
        })
        .collect()
}

/// The last discussion said.
#[track_caller]
fn last(events: &[Event]) -> DiscussionView {
    said(events).pop().expect("a discussion is said")
}

fn asked(h: &Harness, operation: Operation) -> usize {
    h.server().requests_for(operation).len()
}

fn open(h: &mut Harness, episode: u32) {
    h.send(Command::OpenDiscussion { media: FINISHED, episode });
}

/// Signed in to AniList, with three episodes of [`FINISHED`] watched and its threads up, and
/// episode 3's discussion read.
async fn signed_in_and_read() -> (Harness, u32) {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    let thread = threads(&h);
    open(&mut h, 3);
    let view = last(&h.settle().await);
    assert_eq!(view.state, DiscussionState::Ready { more: false });
    assert!(view.can_post && view.signed_in);
    (h, thread)
}

#[tokio::test]
async fn a_watched_episode_reads_its_own_thread_with_replies_one_step_in() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    let thread = threads(&h);

    open(&mut h, 3);
    let views = said(&h.settle().await);
    assert_eq!(views.first().map(|v| &v.state), Some(&DiscussionState::Loading));
    let view = views.last().expect("read");
    assert_eq!(view.state, DiscussionState::Ready { more: false });
    assert_eq!(view.title, "Lantern Harbour Episode 3 Discussion", "the spoiler tag left out");
    assert_eq!(view.comment_count, 4);
    assert_eq!(view.url, format!("https://anilist.co/forum/thread/{thread}"));
    assert!(!view.can_post && !view.signed_in, "signed out, it reads and does not post");

    let ids: Vec<u32> = view.comments.iter().map(|c| c.id).collect();
    assert_eq!(ids, [11, 14]);
    let first = &view.comments[0];
    assert_eq!(first.author, "hoshi");
    assert!(first.body[0].spans[0].bold, "the markdown is read into spans");
    assert_eq!(first.age, Msg::DiscussionAgeDays { days: 2 });
    let reply = &first.replies[0];
    assert_eq!((reply.id, reply.hidden_replies), (12, 1), "the reply's own reply waits");
    assert_eq!(reply.replies[0].author, "sora");
    assert!(view.comments[1].body[0].spans.iter().all(|s| s.spoiler));
    assert!(view.comments.iter().all(|c| !c.own && c.pending == PostState::Posted));
    assert_eq!(
        (asked(&h, Operation::EpisodeThreads), asked(&h, Operation::ThreadComments)),
        (1, 1)
    );

    // Opened again within the hour, the thread is not looked up again; the comments are read.
    h.send(Command::CloseDiscussion);
    open(&mut h, 3);
    assert_eq!(last(&h.settle().await).comments.len(), 2);
    assert_eq!(
        (asked(&h, Operation::EpisodeThreads), asked(&h, Operation::ThreadComments)),
        (1, 2)
    );
}

#[tokio::test]
async fn a_later_episode_is_held_back_without_a_word_to_anilist() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - DAY);
    threads(&h);

    open(&mut h, 3);
    let view = last(&h.settle().await);
    assert_eq!(view.state, DiscussionState::NoThread);
    assert!(view.comments.is_empty() && view.title.is_empty());
    assert_eq!(asked(&h, Operation::EpisodeThreads) + asked(&h, Operation::ThreadComments), 0);
}

#[tokio::test]
async fn with_spoiler_protection_off_any_episode_reads() {
    let mut h = Harness::with(Options {
        config: Some("[playback]\nspoiler_protection = false\n".into()),
        ..Options::default()
    })
    .await;
    threads(&h);
    open(&mut h, 3);
    let view = last(&h.settle().await);
    assert_eq!(view.state, DiscussionState::Ready { more: false });
    assert_eq!(view.comments.len(), 2);
}

#[tokio::test]
async fn switched_off_nothing_is_asked() {
    let mut h = Harness::with(Options {
        config: Some("[meta]\ndiscussions = false\n".into()),
        ..Options::default()
    })
    .await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    threads(&h);
    open(&mut h, 3);
    assert_eq!(last(&h.settle().await).state, DiscussionState::NoThread);
    assert_eq!(asked(&h, Operation::EpisodeThreads), 0);

    // Watched past the point it counts, no lookup either.
    h.playback_says(1_000_001, FINISHED, "4", Event::ProgressQueued);
    let events = h.settle().await;
    assert!(!events.iter().any(|e| matches!(e, Event::DiscussionFound { .. })));
    assert_eq!(asked(&h, Operation::EpisodeThreads), 0);
}

#[tokio::test]
async fn an_episode_nobody_opened_a_thread_for_says_so() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    open(&mut h, 3);
    let view = last(&h.settle().await);
    assert_eq!(view.state, DiscussionState::NoThread);
    assert_eq!(
        view.url,
        format!("https://anilist.co/anime/{}", FINISHED.get()),
        "the show's page, whose forum tab lists its threads"
    );
    assert_eq!(asked(&h, Operation::ThreadComments), 0);
}

#[tokio::test]
async fn a_failed_read_says_so_and_offline_shows_what_was_read() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    threads(&h);

    h.server().fail_always(Failure::GraphQl("Internal Server Error".into()));
    open(&mut h, 3);
    assert_eq!(last(&h.settle().await).state, DiscussionState::Failed);

    h.server().heal();
    open(&mut h, 3);
    assert_eq!(last(&h.settle().await).comments.len(), 2);

    // The network gone: the comments read a moment ago, said as read then.
    h.server().fail_always(Failure::Disconnect);
    open(&mut h, 3);
    let view = last(&h.settle().await);
    assert_eq!(view.state, DiscussionState::Offline { age_minutes: 0 });
    assert_eq!(view.comments.len(), 2);
    assert_eq!(view.title, "Lantern Harbour Episode 3 Discussion");
}

#[tokio::test]
async fn crossing_the_watched_point_looks_the_thread_up_and_opens_it() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - DAY);
    threads(&h);

    h.playback_says(1_000_001, FINISHED, "3", Event::ProgressQueued);
    let events = h.settle().await;
    let found: Vec<&Event> =
        events.iter().filter(|e| matches!(e, Event::DiscussionFound { .. })).collect();
    assert!(
        matches!(found[..], [Event::DiscussionFound { media, episode: 3, comments: 4 }] if *media == FINISHED),
        "{found:?}"
    );
    assert_eq!(asked(&h, Operation::ThreadComments), 0, "the comments wait to be opened");

    // Open, though the progress on record has not moved yet; the lookup is not asked again.
    open(&mut h, 3);
    assert_eq!(last(&h.settle().await).state, DiscussionState::Ready { more: false });
    assert_eq!(asked(&h, Operation::EpisodeThreads), 1);
}

#[tokio::test]
async fn signed_out_a_reply_asks_to_sign_in() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - DAY);
    threads(&h);
    open(&mut h, 3);
    h.settle().await;

    h.send(Command::PostComment { parent: None, text: "hello".into() });
    let events = h.settle().await;
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Toast(notice) if notice.msg == Some(Msg::NoticeDiscussionSignIn)
    )));
    assert_eq!(asked(&h, Operation::SaveThreadComment), 0);
}

#[tokio::test]
async fn a_reply_that_fails_is_kept_and_the_same_words_try_it_again() {
    let (mut h, thread) = signed_in_and_read().await;
    let reads = h.server().requests_for(Operation::ThreadComments);
    assert!(
        reads.iter().all(|r| r.authorization.is_some()),
        "read as the viewer, for their likes"
    );

    h.server().fail_next(Failure::GraphQl("Validation".into()));
    let text = "The harbour scene, though";
    h.send(Command::PostComment { parent: Some(11), text: text.into() });
    let views = said(&h.settle().await);
    let states: Vec<PostState> = views
        .iter()
        .map(|v| v.comments[0].replies.last().expect("the reply").pending)
        .collect();
    assert_eq!(states, [PostState::Sending, PostState::Failed]);
    let kept = views.last().unwrap().comments[0].replies.last().unwrap().clone();
    assert!(kept.own && kept.markdown == text);

    h.send(Command::PostComment { parent: Some(11), text: text.into() });
    let view = last(&h.settle().await);
    let replies = &view.comments[0].replies;
    assert_eq!(replies.len(), 2, "tried again in its place, not posted twice");
    let posted = &replies[1];
    assert_eq!(posted.pending, PostState::Posted);
    assert!(posted.own && posted.id != kept.id, "AniList's own id now");
    assert_eq!(posted.author, "fixture-viewer");
    assert_eq!(view.comment_count, 5);

    let on_anilist = h.server().thread_comments(thread);
    let under = on_anilist[0]["childComments"].as_array().unwrap();
    assert_eq!(under.len(), 2);
    assert_eq!(under[1]["comment"], text);
    let saves = h.server().requests_for(Operation::SaveThreadComment);
    assert!(saves.iter().all(|r| r.authorization.is_some()));
}

#[tokio::test]
async fn a_like_shows_at_once_and_is_put_right_when_it_fails() {
    let (mut h, thread) = signed_in_and_read().await;

    h.send(Command::LikeComment { id: 11 });
    let views = said(&h.settle().await);
    assert!(views.iter().all(|v| v.comments[0].liked && v.comments[0].like_count == 1));
    assert_eq!(h.server().thread_comments(thread)[0]["isLiked"], true);

    h.server().fail_next(Failure::GraphQl("Validation".into()));
    h.send(Command::LikeComment { id: 11 });
    let events = h.settle().await;
    let views = said(&events);
    assert!(!views[0].comments[0].liked, "taken back at once");
    let put_right = views.last().unwrap();
    assert!(put_right.comments[0].liked && put_right.comments[0].like_count == 1);
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Toast(notice) if notice.msg == Some(Msg::NoticeCommentLikeFailed)
    )));
}

#[tokio::test]
async fn the_viewer_edits_and_deletes_their_own_and_only_their_own() {
    let (mut h, thread) = signed_in_and_read().await;
    h.send(Command::PostComment { parent: None, text: "first thoughts".into() });
    let posted = last(&h.settle().await).comments.last().unwrap().id;

    h.send(Command::EditComment { id: posted, text: "second thoughts".into() });
    let view = last(&h.settle().await);
    assert_eq!(view.comments.last().unwrap().markdown, "second thoughts");
    assert_eq!(
        h.server().thread_comments(thread).last().unwrap()["comment"],
        "second thoughts"
    );

    // Someone else's: nothing is sent.
    h.send(Command::DeleteComment { id: 11 });
    h.send(Command::EditComment { id: 11, text: "not mine".into() });
    h.settle().await;
    assert_eq!(asked(&h, Operation::DeleteThreadComment), 0);
    assert_eq!(h.server().thread_comments(thread)[0]["comment"], "**What** a finale");

    h.send(Command::DeleteComment { id: posted });
    let view = last(&h.settle().await);
    assert!(view.comments.iter().all(|c| c.id != posted));
    assert_eq!(view.comment_count, 4);
    assert_eq!(h.server().thread_comments(thread).len(), 2);
}
