//! What a front end asks of the trackers: a sync now, a sign-in, a list, and the edits queued
//! for the drain; and which trackers a finished episode is queued for.

use anistream_core::ids::AnilistId;
use anistream_model::{catalog::LibrarySegment, event::Event, stores::Secret};

use crate::{jobs::JobGuard, outbox::Outbox, services::Services, tracking};

/// Push what is queued to the trackers and pull their lists, now rather than at the next tick.
///
/// Not through [`spawn`]: sync work needs the tracker set, which `spawn` deliberately does not
/// move into its task.
pub fn sync_now(job: Option<JobGuard>, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    let (sync, tx) = (sync.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::drain_once(&sync, &tx).await;
        tracking::pull_once(&sync, &tx).await;
        let _ = tx.send(Event::Status(String::new()));
    });
}

/// Sign in to a tracker, off the UI thread: it opens a browser and waits.
pub fn connect(tracker: String, job: Option<JobGuard>, services: &Services, tx: &Outbox) {
    let Services { sync, host, .. } = services;
    let (sync, host, tx) = (sync.clone(), host.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::connect(sync, host, tracker, tx).await;
    });
}

/// Sign in to Kitsu with a username and password.
pub fn connect_kitsu(
    username: String,
    password: Secret,
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
) {
    let Services { sync, .. } = services;
    let (sync, tx) = (sync.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        tracking::connect_kitsu(sync, username, password.0, tx).await;
    });
}

/// Move a title to another of the trackers' lists, queued for the drain.
pub fn set_status(id: AnilistId, status: LibrarySegment, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    tracking::set_status(sync, id, status);
    // Queued, not sent — the drain owns the network. Reporting the new depth is what
    // makes the badge move immediately.
    for state in sync.state_after_enqueue() {
        let _ = tx.send(Event::Sync(Box::new(state)));
    }
}

/// Score a title on the trackers, queued for the drain.
pub fn set_score(id: AnilistId, score: f32, services: &Services, tx: &Outbox) {
    let Services { sync, .. } = services;
    tracking::set_score(sync, id, score);
    for state in sync.state_after_enqueue() {
        let _ = tx.send(Event::Sync(Box::new(state)));
    }
}

/// One of the trackers' lists, as the Library screen shows it.
pub fn load_library(
    segment: LibrarySegment,
    job: Option<JobGuard>,
    services: &Services,
    tx: &Outbox,
) {
    let Services { store, sync, .. } = services;
    let (sync, tx, store) = (sync.clone(), tx.clone(), store.clone());
    tokio::spawn(async move {
        let _job = job;
        let update = tracking::load_library(&sync, &store, segment).await;
        let _ = tx.send(update);
    });
}

/// The queues a finished episode should be appended to: signed-in trackers only.
///
/// Signed-in rather than enabled, because Kitsu is enabled out of the box — queueing for
/// an account that may never exist would grow a badge nobody can act on. A signed-in
/// tracker that is merely offline still queues, which is the case the outbox exists for;
/// and a tracker signed into *later* catches up anyway, since the first pull reconciles
/// progress out of the watch log.
pub fn signed_in_tracker_ids(sync: &tracking::Sync) -> Vec<String> {
    sync.trackers.iter().filter(|t| t.is_authenticated()).map(|t| t.id().to_owned()).collect()
}
