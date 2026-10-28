//! Episode discussions: the AniList forum thread for an episode, found by the convention's
//! title, read a page at a time, and replied to, liked, edited and deleted as the viewer.
//!
//! An episode's thread is looked up once the episode counts as watched, quietly and behind
//! whatever someone is looking at ([`anistream_meta::in_background`]), and said with
//! [`Event::DiscussionFound`] when there is one. Nothing is looked up before: while spoiler
//! protection is on, a later episode's thread is neither fetched nor opened, and no thread title
//! is ever listed. Opening one costs AniList a request for the comments, and one more when the
//! thread was not looked up in the last hour; each further page, one. Every request goes through
//! the metadata client's budget, which every screen shares.
//!
//! One discussion is open at a time. Each change to it is said whole as [`Event::Discussion`].
//! The viewer's own posts show at once, marked as sending, and settle as AniList answers.
//!
//! Kept in the store's small values: each lookup, for an hour when a thread was found and for
//! ten minutes or a day when none was, by how lately the episode aired; and the comments read
//! last of each thread, for reading offline, for a week.

pub mod markdown;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, MutexGuard, atomic::Ordering},
};

use anistream_core::ids::AnilistId;
use anistream_meta::{
    AniList, AniListError, Meta,
    anilist::{
        Viewer,
        threads::{Thread, ThreadComment, shown_title},
    },
};
use anistream_model::{
    Msg,
    discussion::{CommentView, DiscussionState, DiscussionView, PostState},
    event::Event,
    notice::Notice,
};
use anistream_store::Store;
use anistream_track::AniListTracker;
use serde::{Deserialize, Serialize};

use crate::{outbox::Outbox, services::Services, state::EngineState};

/// A thread found is looked up again after this long.
const FOUND_FOR: i64 = 3_600;
/// No thread, for an episode that aired in the last two days or whose airing is unknown: the
/// thread may not be up yet.
const NONE_RECENT_FOR: i64 = 600;
/// No thread, for an older episode.
const NONE_FOR: i64 = 86_400;
/// How lately an episode aired for its missing thread to be asked about again soon.
const RECENT: i64 = 2 * 86_400;
/// How long comments are kept for reading offline.
const COMMENTS_KEPT: i64 = 7 * 86_400;
/// A wait on the budget shorter than this is no wait worth saying.
const WAIT_SAID_MS: u128 = 400;

/// The discussion open, and the episodes that crossed the watched point this session: shared
/// with the work that reads and posts.
#[derive(Clone, Default)]
pub struct Discussions {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// Counted up on every open and close, so work for a discussion since closed says nothing.
    generation: u64,
    open: Option<Open>,
    /// Episodes watched past the point they count as watched this session, which their
    /// discussion opens for whatever the progress on record says yet.
    unlocked: HashSet<(AnilistId, u32)>,
    /// The id the next comment on its way takes, counted down from the top so it is never
    /// one of AniList's.
    next_local: u32,
}

/// The open discussion.
struct Open {
    generation: u64,
    media: AnilistId,
    episode: u32,
    thread: Option<Thread>,
    /// Top-level comments, in AniList's order, each with its replies.
    comments: Vec<ThreadComment>,
    /// Pages read.
    pages: u32,
    phase: Phase,
    /// The viewer's own comments, edits, deletions and likes on their way, by comment.
    pending: HashMap<u32, Pending>,
    viewer: Option<Viewer>,
    signed_in: bool,
    load_images: bool,
    /// A page is being read.
    reading: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Ready {
        more: bool,
    },
    RateLimited {
        seconds: u32,
    },
    NoThread,
    Failed,
    /// Showing what was read at this moment.
    Offline {
        read_at: i64,
    },
}

struct Pending {
    state: PostState,
    kind: PendingKind,
}

enum PendingKind {
    /// A comment of the viewer's own, not yet on AniList, answering `parent`.
    New {
        parent: Option<u32>,
    },
    /// The viewer's comment with new text.
    Edit,
    Delete,
}

/// What the work runs with: its own handles.
#[derive(Clone)]
struct Lent {
    discussions: Discussions,
    meta: Meta,
    store: Store,
    tracker: Option<Arc<AniListTracker>>,
    offline: Arc<std::sync::atomic::AtomicBool>,
    clock: crate::clock::Clock,
    tx: Outbox,
}

impl Lent {
    fn of(engine: &EngineState, services: &Services, tx: &Outbox) -> Self {
        Self {
            discussions: engine.discussion.clone(),
            meta: services.meta.clone(),
            store: services.store.clone(),
            tracker: services.sync.anilist.clone(),
            offline: services.offline.clone(),
            clock: services.clock.clone(),
            tx: tx.clone(),
        }
    }

    /// The client the discussion asks through: the viewer's token when signed in, so AniList
    /// says which comments they like, on the metadata client's budget either way.
    fn client(&self) -> AniList {
        self.acting().unwrap_or_else(|| self.meta.anilist().clone())
    }

    /// The viewer's own client, for a write.
    fn acting(&self) -> Option<AniList> {
        self.tracker.as_ref().and_then(|tracker| tracker.acting_on(self.meta.anilist()))
    }

    fn offline(&self) -> bool {
        self.offline.load(Ordering::Relaxed)
    }

    /// Say the open discussion, when it is still the one `generation` opened.
    fn say(&self, generation: u64) {
        let view = {
            let inner = self.discussions.lock();
            let Some(open) = inner.open.as_ref().filter(|open| open.generation == generation)
            else {
                return;
            };
            view(open, self.offline(), self.clock.now())
        };
        let _ = self.tx.send(Event::Discussion(view));
    }

    /// Change the open discussion, when it is still the one `generation` opened.
    fn update(&self, generation: u64, change: impl FnOnce(&mut Open)) -> bool {
        let mut inner = self.discussions.lock();
        match inner.open.as_mut().filter(|open| open.generation == generation) {
            Some(open) => {
                change(open);
                true
            }
            None => false,
        }
    }
}

impl Discussions {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Whether the viewer may read `episode` of `media`'s discussion: spoiler protection off for
/// the show, or the episode at or before their progress, or watched past the point it counts
/// this session.
fn allowed(media: AnilistId, episode: u32, engine: &EngineState, services: &Services) -> bool {
    let config =
        crate::show_settings::for_show(engine.config.effective(), &services.store, media);
    if !config.playback.spoiler_protection {
        return true;
    }
    if engine.discussion.lock().unlocked.contains(&(media, episode)) {
        return true;
    }
    let here = services.store.tracker_progress(media).unwrap_or_default();
    let listed = engine.cache.done(media).unwrap_or_default();
    episode <= here.max(listed)
}

/// [`Command::OpenDiscussion`](anistream_model::command::Command::OpenDiscussion).
pub(crate) fn open(
    media: AnilistId,
    episode: u32,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let config = engine.config.effective();
    let on = config.meta.discussions;
    let load_images = config.meta.discussion_images;
    let signed_in = services.sync.anilist.as_ref().is_some_and(|t| {
        use anistream_core::traits::Tracker;
        t.is_authenticated()
    });
    let permitted = on && allowed(media, episode, engine, services);
    let lent = Lent::of(engine, services, tx);
    let generation = {
        let mut inner = engine.discussion.lock();
        inner.generation += 1;
        let generation = inner.generation;
        inner.open = Some(Open {
            generation,
            media,
            episode,
            thread: None,
            comments: Vec::new(),
            pages: 0,
            phase: if permitted { Phase::Loading } else { Phase::NoThread },
            pending: HashMap::new(),
            viewer: None,
            signed_in,
            load_images,
            reading: permitted,
        });
        generation
    };
    lent.say(generation);
    if permitted {
        tokio::spawn(async move { read_first(lent, generation, media, episode).await });
    }
}

/// [`Command::CloseDiscussion`](anistream_model::command::Command::CloseDiscussion).
pub(crate) fn close(engine: &EngineState) {
    let mut inner = engine.discussion.lock();
    inner.generation += 1;
    inner.open = None;
}

/// Read the open discussion's first page, finding its thread first.
async fn read_first(lent: Lent, generation: u64, media: AnilistId, episode: u32) {
    if lent.offline() {
        offline(&lent, generation, None);
        return;
    }
    say_wait(&lent, generation).await;
    let viewer = match &lent.tracker {
        Some(tracker) => tracker.signed_in_viewer().await,
        None => None,
    };
    lent.update(generation, |open| open.viewer = viewer);
    let thread = match thread(&lent, media, episode).await {
        Ok(thread) => thread,
        Err(error) => {
            tracing::warn!(%error, "could not look up the episode's discussion");
            failed(&lent, generation, None, &error);
            return;
        }
    };
    let Some(thread) = thread else {
        lent.update(generation, |open| {
            open.phase = Phase::NoThread;
            open.reading = false;
        });
        lent.say(generation);
        return;
    };
    lent.update(generation, |open| open.thread = Some(thread.clone()));
    say_wait(&lent, generation).await;
    match lent.client().thread_comments(thread.id, 1).await {
        Ok(page) => {
            keep_comments(&lent.store, thread.id, &page.items, lent.clock.now());
            lent.update(generation, |open| {
                open.comments = page.items;
                open.pages = 1;
                open.phase = Phase::Ready { more: page.has_next };
                open.reading = false;
            });
            lent.say(generation);
        }
        Err(error) => {
            tracing::warn!(%error, "could not read the discussion");
            failed(&lent, generation, Some(thread.id), &error);
        }
    }
}

/// Say a wait on AniList's budget as the discussion's state, then wait it out in the request.
async fn say_wait(lent: &Lent, generation: u64) {
    if let Some(wait) = lent.meta.rate_limit_wait().await
        && wait.as_millis() > WAIT_SAID_MS
    {
        let seconds = u32::try_from(wait.as_secs().max(1)).unwrap_or(u32::MAX);
        lent.update(generation, |open| open.phase = Phase::RateLimited { seconds });
        lent.say(generation);
    }
}

/// A read that failed: the comments kept for the thread when the network is the reason, as
/// offline reading; otherwise Failed.
fn failed(lent: &Lent, generation: u64, thread: Option<u32>, error: &AniListError) {
    if lent.offline() || matches!(error, AniListError::Network(_)) {
        offline(lent, generation, thread);
        return;
    }
    lent.update(generation, |open| {
        open.phase = Phase::Failed;
        open.reading = false;
    });
    lent.say(generation);
}

/// Offline: the comments kept, when there are any, with when they were read.
fn offline(lent: &Lent, generation: u64, thread: Option<u32>) {
    let (media, episode) = {
        let inner = lent.discussions.lock();
        match inner.open.as_ref().filter(|open| open.generation == generation) {
            Some(open) => (open.media, open.episode),
            None => return,
        }
    };
    let thread = thread.or_else(|| {
        kept_thread(&lent.store, media, episode).and_then(|(kept, _)| kept.map(|t| t.id))
    });
    let found = kept_thread(&lent.store, media, episode).and_then(|(kept, _)| kept);
    let comments = thread.and_then(|id| kept_comments(&lent.store, id));
    let now = lent.clock.now();
    lent.update(generation, |open| {
        if open.thread.is_none() {
            open.thread = found;
        }
        match comments {
            Some(kept) => {
                open.comments = kept.comments;
                open.phase = Phase::Offline { read_at: kept.at };
            }
            None => open.phase = Phase::Offline { read_at: now },
        }
        open.reading = false;
    });
    lent.say(generation);
}

/// [`Command::MoreComments`](anistream_model::command::Command::MoreComments).
pub(crate) fn more(engine: &EngineState, services: &Services, tx: &Outbox) {
    let lent = Lent::of(engine, services, tx);
    let asked = {
        let mut inner = engine.discussion.lock();
        let Some(open) = inner.open.as_mut() else { return };
        let Some(thread) = &open.thread else { return };
        if open.reading || open.phase != (Phase::Ready { more: true }) {
            return;
        }
        open.reading = true;
        (open.generation, thread.id, open.pages + 1)
    };
    let (generation, thread, page) = asked;
    tokio::spawn(async move {
        match lent.client().thread_comments(thread, page).await {
            Ok(found) => {
                let mut all = Vec::new();
                lent.update(generation, |open| {
                    // A comment posted since the last page was read may be on this one too.
                    let known: HashSet<u32> = open.comments.iter().map(|c| c.id).collect();
                    open.comments
                        .extend(found.items.into_iter().filter(|c| !known.contains(&c.id)));
                    open.pages = page;
                    open.phase = Phase::Ready { more: found.has_next };
                    open.reading = false;
                    all = open.comments.clone();
                });
                keep_comments(&lent.store, thread, &all, lent.clock.now());
            }
            Err(error) => {
                tracing::warn!(%error, "could not read more of the discussion");
                // The page is asked for again as the viewer scrolls.
                lent.update(generation, |open| open.reading = false);
            }
        }
        lent.say(generation);
    });
}

/// Toast that the viewer has to sign in first.
fn sign_in_first(tx: &Outbox) {
    let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeDiscussionSignIn)));
}

/// [`Command::PostComment`](anistream_model::command::Command::PostComment).
pub(crate) fn post(
    parent: Option<u32>,
    text: String,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return;
    }
    let lent = Lent::of(engine, services, tx);
    let Some(client) = lent.acting() else { return sign_in_first(tx) };
    let now = services.clock.now();
    let asked = {
        let mut guard = engine.discussion.lock();
        let inner = &mut *guard;
        let Some(open) = inner.open.as_mut() else { return };
        let Some(thread) = open.thread.as_ref().map(|t| t.id) else { return };
        // The same words again, where they failed before, try that comment again.
        let again = open.pending.iter().find_map(|(id, pending)| {
            let retry = pending.state == PostState::Failed
                && matches!(pending.kind, PendingKind::New { parent: p } if p == parent)
                && find(&open.comments, *id).is_some_and(|c| c.markdown == text);
            retry.then_some(*id)
        });
        let local = match again {
            Some(id) => id,
            None => {
                inner.next_local = inner.next_local.wrapping_sub(1);
                if inner.next_local == 0 {
                    inner.next_local = u32::MAX;
                }
                let id = inner.next_local;
                let (author, author_id) = open
                    .viewer
                    .as_ref()
                    .map(|v| (v.name.clone().unwrap_or_default(), v.id))
                    .unwrap_or_default();
                let comment = ThreadComment {
                    id,
                    author,
                    author_id,
                    avatar: None,
                    created_at: now,
                    markdown: text.clone(),
                    like_count: 0,
                    liked: false,
                    replies: Vec::new(),
                };
                match parent.and_then(|p| find_mut(&mut open.comments, p)) {
                    Some(under) => under.replies.push(comment),
                    None => open.comments.push(comment),
                }
                id
            }
        };
        open.pending.insert(
            local,
            Pending { state: PostState::Sending, kind: PendingKind::New { parent } },
        );
        (open.generation, thread, local)
    };
    let (generation, thread, local) = asked;
    lent.say(generation);
    tokio::spawn(async move {
        let saved = client.save_thread_comment(thread, parent, None, &text).await;
        lent.update(generation, |open| match saved {
            Ok(saved) => {
                if let Some(at) = find_mut(&mut open.comments, local) {
                    *at = ThreadComment { replies: std::mem::take(&mut at.replies), ..saved };
                }
                open.pending.remove(&local);
                if let Some(thread) = open.thread.as_mut() {
                    thread.reply_count += 1;
                }
            }
            Err(error) => {
                tracing::warn!(%error, "could not post the comment");
                if let Some(pending) = open.pending.get_mut(&local) {
                    pending.state = PostState::Failed;
                }
            }
        });
        lent.say(generation);
    });
}

/// [`Command::EditComment`](anistream_model::command::Command::EditComment).
pub(crate) fn edit(
    id: u32,
    text: String,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return;
    }
    let lent = Lent::of(engine, services, tx);
    let Some(client) = lent.acting() else { return sign_in_first(tx) };
    // A comment that never posted is posted with its new words.
    let unposted = {
        let mut inner = engine.discussion.lock();
        let Some(open) = inner.open.as_mut() else { return };
        match open.pending.get(&id) {
            Some(Pending { kind: PendingKind::New { parent }, state: PostState::Failed }) => {
                let parent = *parent;
                if let Some(comment) = find_mut(&mut open.comments, id) {
                    comment.markdown = text.clone();
                }
                Some(parent)
            }
            Some(_) => return,
            None => None,
        }
    };
    if let Some(parent) = unposted {
        return post(parent, text, engine, services, tx);
    }
    let asked = {
        let mut inner = engine.discussion.lock();
        let Some(open) = inner.open.as_mut() else { return };
        let Some(thread) = open.thread.as_ref().map(|t| t.id) else { return };
        let own = open.viewer.as_ref().map(|v| v.id);
        let Some(comment) = find_mut(&mut open.comments, id) else { return };
        if own != Some(comment.author_id) {
            return;
        }
        comment.markdown = text.clone();
        open.pending.insert(id, Pending { state: PostState::Sending, kind: PendingKind::Edit });
        (open.generation, thread)
    };
    let (generation, thread) = asked;
    lent.say(generation);
    tokio::spawn(async move {
        let saved = client.save_thread_comment(thread, None, Some(id), &text).await;
        lent.update(generation, |open| match saved {
            Ok(saved) => {
                if let Some(at) = find_mut(&mut open.comments, id) {
                    at.markdown = saved.markdown;
                }
                open.pending.remove(&id);
            }
            Err(error) => {
                tracing::warn!(%error, "could not edit the comment");
                if let Some(pending) = open.pending.get_mut(&id) {
                    pending.state = PostState::Failed;
                }
            }
        });
        lent.say(generation);
    });
}

/// [`Command::DeleteComment`](anistream_model::command::Command::DeleteComment).
pub(crate) fn delete(id: u32, engine: &EngineState, services: &Services, tx: &Outbox) {
    let lent = Lent::of(engine, services, tx);
    // One that never posted is only taken away.
    let (asked, unposted) = {
        let mut inner = engine.discussion.lock();
        let Some(open) = inner.open.as_mut() else { return };
        if matches!(open.pending.get(&id), Some(Pending { kind: PendingKind::New { .. }, .. }))
        {
            remove(&mut open.comments, id);
            open.pending.remove(&id);
            (open.generation, true)
        } else {
            if open.pending.contains_key(&id) {
                return;
            }
            let own = open.viewer.as_ref().map(|v| v.id);
            if find(&open.comments, id).is_none_or(|c| own != Some(c.author_id)) {
                return;
            }
            open.pending
                .insert(id, Pending { state: PostState::Sending, kind: PendingKind::Delete });
            (open.generation, false)
        }
    };
    if unposted {
        return lent.say(asked);
    }
    let Some(client) = lent.acting() else {
        lent.update(asked, |open| {
            open.pending.remove(&id);
        });
        return sign_in_first(tx);
    };
    lent.say(asked);
    tokio::spawn(async move {
        let deleted = client.delete_thread_comment(id).await;
        let gone = matches!(deleted, Ok(true));
        if let Err(error) = &deleted {
            tracing::warn!(%error, "could not delete the comment");
        }
        lent.update(asked, |open| {
            open.pending.remove(&id);
            if gone {
                let removed = remove(&mut open.comments, id);
                if let Some(thread) = open.thread.as_mut() {
                    thread.reply_count = thread.reply_count.saturating_sub(removed);
                }
            }
        });
        if !gone {
            let _ = lent.tx.send(Event::Toast(Notice::alert(Msg::NoticeCommentDeleteFailed)));
        }
        lent.say(asked);
    });
}

/// [`Command::LikeComment`](anistream_model::command::Command::LikeComment).
pub(crate) fn like(id: u32, engine: &EngineState, services: &Services, tx: &Outbox) {
    let lent = Lent::of(engine, services, tx);
    let Some(client) = lent.acting() else { return sign_in_first(tx) };
    let asked = {
        let mut inner = engine.discussion.lock();
        let Some(open) = inner.open.as_mut() else { return };
        if open.pending.contains_key(&id) {
            return;
        }
        let Some(comment) = find_mut(&mut open.comments, id) else { return };
        // As it will be, at once; put right by AniList's answer.
        comment.liked = !comment.liked;
        comment.like_count = if comment.liked {
            comment.like_count + 1
        } else {
            comment.like_count.saturating_sub(1)
        };
        (open.generation, comment.liked, comment.like_count)
    };
    let (generation, liked, count) = asked;
    lent.say(generation);
    tokio::spawn(async move {
        let answer = client.toggle_comment_like(id).await;
        lent.update(generation, |open| {
            if let Some(comment) = find_mut(&mut open.comments, id) {
                match &answer {
                    Ok(now) => {
                        comment.liked = now.liked;
                        comment.like_count = now.count;
                    }
                    Err(_) => {
                        comment.liked = !liked;
                        comment.like_count =
                            if liked { count.saturating_sub(1) } else { count + 1 };
                    }
                }
            }
        });
        if let Err(error) = &answer {
            tracing::warn!(%error, "could not like the comment");
            let _ = lent.tx.send(Event::Toast(Notice::alert(Msg::NoticeCommentLikeFailed)));
        }
        lent.say(generation);
    });
}

/// An episode counted as watched: its discussion opens from now on, and its thread is looked
/// up, quietly, to say whether there is one.
pub(crate) fn watched(
    media: AnilistId,
    episode: &str,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Ok(episode) = episode.parse::<u32>() else { return };
    engine.discussion.lock().unlocked.insert((media, episode));
    if !engine.config.effective().meta.discussions {
        return;
    }
    let lent = Lent::of(engine, services, tx);
    if lent.offline() {
        return;
    }
    tokio::spawn(async move {
        let found = anistream_meta::in_background(thread(&lent, media, episode)).await;
        match found {
            Ok(Some(thread)) => {
                let _ = lent.tx.send(Event::DiscussionFound {
                    media,
                    episode,
                    comments: thread.reply_count,
                });
            }
            Ok(None) => {}
            Err(error) => tracing::debug!(%error, "no word on the episode's discussion"),
        }
    });
}

/// The thread for `episode` of `media`: as kept, while that is fresh, or asked for and kept.
async fn thread(
    lent: &Lent,
    media: AnilistId,
    episode: u32,
) -> Result<Option<Thread>, AniListError> {
    let now = lent.clock.now();
    if let Some((kept, at)) = kept_thread(&lent.store, media, episode) {
        let fresh_for = match &kept {
            Some(_) => FOUND_FOR,
            None => none_for(&lent.store, media, episode, now),
        };
        if now - at < fresh_for {
            return Ok(kept);
        }
    }
    let found = lent.client().episode_thread(media, episode).await?;
    keep_thread(&lent.store, media, episode, found.as_ref(), now);
    Ok(found)
}

/// How long a lookup that found no thread holds: briefly while the episode is new, since the
/// thread may not be up yet, and a day after.
fn none_for(store: &Store, media: AnilistId, episode: u32, now: i64) -> i64 {
    match store.airing_time(media, episode).ok().flatten() {
        Some(aired) if now - aired > RECENT => NONE_FOR,
        _ => NONE_RECENT_FOR,
    }
}

#[derive(Serialize, Deserialize)]
struct KeptThread {
    at: i64,
    thread: Option<Thread>,
}

#[derive(Serialize, Deserialize)]
struct KeptComments {
    at: i64,
    comments: Vec<ThreadComment>,
}

fn thread_key(media: AnilistId, episode: u32) -> String {
    format!("discussion.thread.{}.{episode}", media.get())
}

fn comments_key(thread: u32) -> String {
    format!("discussion.comments.{thread}")
}

fn kept_thread(store: &Store, media: AnilistId, episode: u32) -> Option<(Option<Thread>, i64)> {
    let text = store.get_meta_text(&thread_key(media, episode)).ok()??;
    let kept: KeptThread = serde_json::from_str(&text).ok()?;
    Some((kept.thread, kept.at))
}

fn keep_thread(
    store: &Store,
    media: AnilistId,
    episode: u32,
    thread: Option<&Thread>,
    now: i64,
) {
    let kept = KeptThread { at: now, thread: thread.cloned() };
    if let Ok(text) = serde_json::to_string(&kept)
        && let Err(error) = store.set_meta_text(&thread_key(media, episode), &text)
    {
        tracing::warn!(%error, "could not keep the discussion lookup");
    }
}

fn kept_comments(store: &Store, thread: u32) -> Option<KeptComments> {
    let text = store.get_meta_text(&comments_key(thread)).ok()??;
    serde_json::from_str(&text).ok()
}

/// Keep a thread's comments for reading offline, and let go of what is older than a week.
fn keep_comments(store: &Store, thread: u32, comments: &[ThreadComment], now: i64) {
    let kept = KeptComments { at: now, comments: comments.to_vec() };
    if let Ok(text) = serde_json::to_string(&kept)
        && let Err(error) = store.set_meta_text(&comments_key(thread), &text)
    {
        tracing::warn!(%error, "could not keep the discussion's comments");
    }
    prune(store, now);
}

/// Let go of comments and lookups kept longer than a week.
fn prune(store: &Store, now: i64) {
    for prefix in ["discussion.comments.", "discussion.thread."] {
        let Ok(kept) = store.meta_texts_with_prefix(prefix) else { continue };
        for (key, text) in kept {
            let at = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value["at"].as_i64())
                .unwrap_or_default();
            if now - at > COMMENTS_KEPT {
                let _ = store.delete_meta(&key);
            }
        }
    }
}

fn find(comments: &[ThreadComment], id: u32) -> Option<&ThreadComment> {
    comments.iter().find_map(|c| if c.id == id { Some(c) } else { find(&c.replies, id) })
}

fn find_mut(comments: &mut [ThreadComment], id: u32) -> Option<&mut ThreadComment> {
    for comment in comments {
        if comment.id == id {
            return Some(comment);
        }
        if let Some(found) = find_mut(&mut comment.replies, id) {
            return Some(found);
        }
    }
    None
}

/// Take comment `id` and its replies out: how many comments went.
fn remove(comments: &mut Vec<ThreadComment>, id: u32) -> u32 {
    if let Some(at) = comments.iter().position(|c| c.id == id) {
        return comments.remove(at).count();
    }
    comments.iter_mut().map(|c| remove(&mut c.replies, id)).find(|n| *n > 0).unwrap_or(0)
}

/// The discussion as a front end is told it.
fn view(open: &Open, offline: bool, now: i64) -> DiscussionView {
    let state = match open.phase {
        Phase::Loading => DiscussionState::Loading,
        Phase::Ready { more } => DiscussionState::Ready { more },
        Phase::RateLimited { seconds } => DiscussionState::RateLimited { seconds },
        Phase::NoThread => DiscussionState::NoThread,
        Phase::Failed => DiscussionState::Failed,
        Phase::Offline { read_at } => DiscussionState::Offline {
            age_minutes: u32::try_from((now - read_at).max(0) / 60).unwrap_or(u32::MAX),
        },
    };
    let own = open.viewer.as_ref().map(|v| v.id);
    let comments = open
        .comments
        .iter()
        .map(|comment| {
            let replies = comment
                .replies
                .iter()
                .map(|reply| {
                    let mut deeper = Vec::new();
                    flatten(&reply.replies, &mut deeper);
                    deeper.sort_by_key(|c| (c.created_at, c.id));
                    let deeper: Vec<CommentView> = deeper
                        .into_iter()
                        .map(|c| comment_view(c, Vec::new(), open, own, now))
                        .collect();
                    let hidden = u32::try_from(deeper.len()).unwrap_or(u32::MAX);
                    CommentView {
                        hidden_replies: hidden,
                        ..comment_view(reply, deeper, open, own, now)
                    }
                })
                .collect();
            comment_view(comment, replies, open, own, now)
        })
        .collect();
    let url = match &open.thread {
        Some(thread) => thread.url.clone(),
        // The show's page on AniList, whose Forum tab lists its threads.
        None => format!("https://anilist.co/anime/{}", open.media.get()),
    };
    DiscussionView {
        media: open.media,
        episode: open.episode,
        title: open.thread.as_ref().map(|t| shown_title(&t.title)).unwrap_or_default(),
        comment_count: open.thread.as_ref().map_or(0, |t| t.reply_count),
        url,
        comments,
        state,
        can_post: open.signed_in && !offline && open.thread.is_some(),
        signed_in: open.signed_in,
        load_images: open.load_images,
    }
}

fn flatten<'a>(comments: &'a [ThreadComment], into: &mut Vec<&'a ThreadComment>) {
    for comment in comments {
        into.push(comment);
        flatten(&comment.replies, into);
    }
}

fn comment_view(
    comment: &ThreadComment,
    replies: Vec<CommentView>,
    open: &Open,
    own: Option<u32>,
    now: i64,
) -> CommentView {
    let pending = open.pending.get(&comment.id);
    CommentView {
        id: comment.id,
        author: comment.author.clone(),
        avatar: comment.avatar.clone(),
        created_at: comment.created_at,
        age: age(now - comment.created_at),
        body: markdown::blocks(&comment.markdown),
        like_count: comment.like_count,
        liked: comment.liked,
        own: own.is_some_and(|own| own == comment.author_id)
            || matches!(pending, Some(Pending { kind: PendingKind::New { .. }, .. })),
        pending: pending.map_or(PostState::Posted, |p| p.state),
        replies,
        hidden_replies: 0,
        markdown: comment.markdown.clone(),
    }
}

/// How long ago, as a timeline words it: "now", "5 min", "2 h", "3 d", "4 w", "1 y".
pub fn age(seconds: i64) -> Msg {
    let seconds = seconds.max(0);
    let (minutes, hours, days) = (seconds / 60, seconds / 3_600, seconds / 86_400);
    match () {
        () if minutes < 1 => Msg::DiscussionAgeNow,
        () if hours < 1 => Msg::DiscussionAgeMinutes { minutes },
        () if days < 1 => Msg::DiscussionAgeHours { hours },
        () if days < 7 => Msg::DiscussionAgeDays { days },
        () if days < 365 => Msg::DiscussionAgeWeeks { weeks: days / 7 },
        () => Msg::DiscussionAgeYears { years: days / 365 },
    }
}

#[cfg(test)]
mod tests;
