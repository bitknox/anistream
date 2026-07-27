//! The drain engine.
//!
//! Local SQLite is the source of truth and this pushes a projection of it outward. Two loops,
//! deliberately separate:
//!
//! - **Drain** takes whatever the outbox holds and sends it. Runs often, cheap when idle.
//! - **Pull** fetches the remote library and reconciles. Runs rarely, costs a request.
//!
//! The engine's whole job is to be boring under failure. Nothing here may block playback,
//! nothing may lose a queued op, and a tracker that is down must not turn into a stream of
//! error toasts.

use std::sync::Arc;

use anistream_core::traits::Tracker;
use anistream_store::{Store, outbox::OutboxEntry};

use crate::merge::{self, LocalState, Merged};

/// How many ops one drain pass sends per tracker.
///
/// Bounded so a large backlog cannot monopolise a rate limit that other parts of the app share.
/// The remainder simply waits for the next pass.
const DRAIN_BATCH: u32 = 25;

/// What one drain pass did, for the status line and the Accounts overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrainReport {
    pub sent: u32,
    pub failed: u32,
    /// Set when the tracker rejected our credentials. The user has to re-authorise; retrying
    /// cannot help.
    pub needs_reauth: bool,
    /// Ops still queued after this pass.
    pub remaining: u32,
}

impl DrainReport {
    pub fn did_nothing(&self) -> bool {
        self.sent == 0 && self.failed == 0 && !self.needs_reauth
    }
}

/// Send what the outbox holds for one tracker.
///
/// Ops are sent as a batch, and the batch's fate is applied to every op in it. That is safe
/// precisely because tracker pushes are idempotent — a partial success followed by a retry
/// re-sends something already applied, which by contract is a no-op.
pub async fn drain(
    store: &Store,
    tracker: &dyn Tracker,
    now: i64,
) -> Result<DrainReport, anistream_store::StoreError> {
    let id = tracker.id();
    let mut report = DrainReport::default();

    if !tracker.is_authenticated() {
        // Not an error. Watching with no account is a supported way to use this, so the queue
        // simply waits — and keeps waiting across restarts, because it is a table.
        report.remaining = store.outbox_depth(Some(id))?;
        return Ok(report);
    }

    let claimed = store.claim_ready(id, now, DRAIN_BATCH)?;
    if claimed.is_empty() {
        return Ok(report);
    }

    let ops: Vec<_> = claimed.iter().map(|entry| entry.op.clone()).collect();
    match tracker.push(&ops).await {
        Ok(()) => {
            for entry in &claimed {
                store.complete(entry.id)?;
            }
            report.sent = claimed.len() as u32;
            tracing::info!(tracker = id, sent = report.sent, "outbox drained");
        }
        Err(anistream_core::Error::Auth(message)) => {
            // Deliberately *not* counted as a failure: incrementing attempts would push the
            // backoff toward an hour for something no amount of waiting fixes, and the queue
            // has to still be intact when the user re-authorises.
            report.needs_reauth = true;
            tracing::warn!(tracker = id, %message, "tracker rejected our credentials");
        }
        Err(error) => {
            let message = error.to_string();
            for entry in &claimed {
                store.fail(entry.id, &message, now)?;
            }
            report.failed = claimed.len() as u32;
            tracing::warn!(tracker = id, %message, "outbox push failed; backing off");
        }
    }

    report.remaining = store.outbox_depth(Some(id))?;
    Ok(report)
}

/// What a library pull produced.
#[derive(Debug, Clone, Default)]
pub struct PullReport {
    /// Titles seen on the remote list.
    pub seen: u32,
    /// Ops queued so the remote catches up with local.
    pub queued: u32,
    /// Titles where the remote was ahead and local adopted its value.
    pub adopted: u32,
    /// Disagreements needing the user, for the Conflicts overlay.
    pub conflicts: Vec<merge::Conflict>,
}

/// Pull the remote library and reconcile it against local history.
///
/// Reconciliation is one-directional in effect: anything local can prove goes out through the
/// outbox, anything the remote is ahead on is adopted locally, and anything genuinely
/// contradictory is reported rather than resolved.
pub async fn pull(
    store: &Store,
    tracker: &dyn Tracker,
    now: i64,
    last_pull_at: i64,
) -> Result<PullReport, anistream_core::Error> {
    let id = tracker.id();
    let mut report = PullReport::default();

    if !tracker.is_authenticated() {
        return Ok(report);
    }

    let remote = tracker.pull_library().await?;
    report.seen = remote.len() as u32;

    for entry in &remote {
        let local = local_state(store, entry.anilist_id)
            .map_err(|e| anistream_core::Error::Store(e.to_string()))?;
        let pending = store
            .pending_progress(id, entry.anilist_id)
            .map_err(|e| anistream_core::Error::Store(e.to_string()))?;

        let merged = merge::reconcile_at(&local, entry, pending, last_pull_at);
        apply(store, id, now, &merged, &mut report)
            .map_err(|e| anistream_core::Error::Store(e.to_string()))?;
    }

    tracing::info!(
        tracker = id,
        seen = report.seen,
        queued = report.queued,
        adopted = report.adopted,
        conflicts = report.conflicts.len(),
        "library reconciled"
    );
    Ok(report)
}

/// Put a merge result into effect.
fn apply(
    store: &Store,
    tracker_id: &str,
    now: i64,
    merged: &Merged,
    report: &mut PullReport,
) -> Result<(), anistream_store::StoreError> {
    for op in &merged.push {
        if store.enqueue(tracker_id, op, now)?.is_some() {
            report.queued += 1;
        }
    }
    if merged.adopt_progress.is_some() {
        report.adopted += 1;
    }
    report.conflicts.extend(merged.conflicts.iter().cloned());
    Ok(())
}

/// Build the local view of one title from the watch log.
///
/// Progress is derived from completed episodes rather than a stored counter, so rewatching
/// cannot inflate it — the same rule the projection uses.
pub fn local_state(
    store: &Store,
    anilist_id: anistream_core::ids::AnilistId,
) -> Result<LocalState, anistream_store::StoreError> {
    Ok(LocalState {
        progress: store.completed_episode_count(anilist_id)?,
        // Status and score are not yet set anywhere but the list-status overlay, which writes
        // them straight to the outbox. Until that state is persisted locally, the merge sees
        // "local has no opinion" — which is correct, and means the remote is left alone.
        ..LocalState::default()
    })
}

/// Queue progress for every configured tracker.
///
/// Called when an episode crosses the commit threshold. Failure to queue is logged, never
/// propagated: a database hiccup must not interrupt playback.
pub fn queue_progress(
    store: &Store,
    trackers: &[Arc<dyn Tracker>],
    anilist_id: anistream_core::ids::AnilistId,
    now: i64,
) {
    let ids: Vec<String> = trackers.iter().map(|t| t.id().to_owned()).collect();
    queue_progress_for(store, &ids, anilist_id, now);
}

/// Queue progress against tracker *ids*.
///
/// Takes ids rather than trackers because the caller is the playback loop, which has no reason
/// to hold the tracker set — and this runs on a blocking thread, where moving a list of strings
/// is simpler than threading trait objects through.
pub fn queue_progress_for(
    store: &Store,
    trackers: &[String],
    anilist_id: anistream_core::ids::AnilistId,
    now: i64,
) {
    let episodes = match store.completed_episode_count(anilist_id) {
        Ok(0) => return,
        Ok(count) => count,
        Err(e) => {
            tracing::warn!(error = %e, "could not count completed episodes");
            return;
        }
    };

    let op = anistream_core::traits::TrackOp::SetProgress { anilist_id, episode: episodes };
    for tracker in trackers {
        // Queued even for an unauthenticated tracker: the whole point of a durable outbox is
        // that watching now and signing in later still syncs.
        if let Err(e) = store.enqueue(tracker, &op, now) {
            tracing::warn!(tracker, error = %e, "could not queue progress");
        }
    }
}

/// Total queued ops across every tracker, for the `⇅` badge.
pub fn depth(store: &Store) -> u32 {
    store.outbox_depth(None).unwrap_or(0)
}

/// Claimed ops, for the Accounts overlay's "what is waiting" list.
pub fn pending(store: &Store, tracker_id: &str, now: i64) -> Vec<OutboxEntry> {
    store.claim_ready(tracker_id, now, 50).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::{
        ids::AnilistId,
        traits::{TrackOp, TrackedEntry, WatchStatus},
    };
    use anistream_store::WatchEvent;
    use std::sync::Mutex;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    /// A tracker that records what it was asked to do and fails how the test tells it to.
    struct MockTracker {
        authenticated: bool,
        /// Set to make `push` fail.
        failure: Option<anistream_core::Error>,
        remote: Vec<TrackedEntry>,
        pushed: Mutex<Vec<Vec<TrackOp>>>,
        pulls: Mutex<u32>,
    }

    impl MockTracker {
        fn new() -> Self {
            Self {
                authenticated: true,
                failure: None,
                remote: Vec::new(),
                pushed: Mutex::new(Vec::new()),
                pulls: Mutex::new(0),
            }
        }

        fn failing(error: anistream_core::Error) -> Self {
            Self { failure: Some(error), ..Self::new() }
        }

        fn with_remote(remote: Vec<TrackedEntry>) -> Self {
            Self { remote, ..Self::new() }
        }

        fn unauthenticated() -> Self {
            Self { authenticated: false, ..Self::new() }
        }

        fn batches(&self) -> Vec<Vec<TrackOp>> {
            self.pushed.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Tracker for MockTracker {
        fn id(&self) -> &str {
            "mock"
        }

        fn is_authenticated(&self) -> bool {
            self.authenticated
        }

        async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
            *self.pulls.lock().unwrap() += 1;
            Ok(self.remote.clone())
        }

        async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
            self.pushed.lock().unwrap().push(ops.to_vec());
            match &self.failure {
                Some(anistream_core::Error::Auth(m)) => {
                    Err(anistream_core::Error::Auth(m.clone()))
                }
                Some(e) => Err(anistream_core::Error::Tracker {
                    tracker: "mock".into(),
                    message: e.to_string(),
                }),
                None => Ok(()),
            }
        }
    }

    fn store_with_completed(episodes: u32) -> Store {
        let store = Store::open_in_memory().unwrap();
        for ep in 1..=episodes {
            store
                .record_event(&WatchEvent {
                    duration_secs: Some(1440.0),
                    completed: true,
                    at: 1_000 + i64::from(ep),
                    ..WatchEvent::new(FRIEREN, ep.to_string(), 1400.0)
                })
                .unwrap();
        }
        store
    }

    #[tokio::test]
    async fn a_successful_drain_empties_the_queue() {
        let store = Store::open_in_memory().unwrap();
        store
            .enqueue("mock", &TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }, 0)
            .unwrap();

        let tracker = MockTracker::new();
        let report = drain(&store, &tracker, 100).await.unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(report.remaining, 0);
        assert_eq!(tracker.batches().len(), 1);
    }

    #[tokio::test]
    async fn a_failed_push_leaves_the_op_queued_and_backs_off() {
        // Losing a queued op on a transient failure is the worst thing this code could do.
        let store = Store::open_in_memory().unwrap();
        store
            .enqueue("mock", &TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }, 0)
            .unwrap();

        let tracker = MockTracker::failing(anistream_core::Error::Network("503".into()));
        let report = drain(&store, &tracker, 100).await.unwrap();

        assert_eq!(report.failed, 1);
        assert_eq!(report.remaining, 1, "the op must survive a failure");
        assert!(
            store.claim_ready("mock", 101, 10).unwrap().is_empty(),
            "should be waiting out a backoff rather than hammering"
        );
    }

    #[tokio::test]
    async fn a_rejected_token_does_not_burn_the_backoff() {
        // A bad token fails identically forever. Counting it as an attempt would push the
        // retry delay to an hour, so a user who re-authorises would sit and wait for nothing.
        let store = Store::open_in_memory().unwrap();
        store
            .enqueue("mock", &TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }, 0)
            .unwrap();

        let tracker = MockTracker::failing(anistream_core::Error::Auth("bad token".into()));
        let report = drain(&store, &tracker, 100).await.unwrap();

        assert!(report.needs_reauth);
        assert_eq!(report.failed, 0, "auth failure must not count as a retryable failure");
        assert_eq!(
            store.claim_ready("mock", 100, 10).unwrap().len(),
            1,
            "the op must be ready to send the moment the user re-authorises"
        );
    }

    #[tokio::test]
    async fn an_unauthenticated_tracker_keeps_the_queue_without_sending() {
        // Watching with no account is supported, so the queue accumulates quietly and drains
        // whenever an account appears.
        let store = Store::open_in_memory().unwrap();
        store
            .enqueue("mock", &TrackOp::SetProgress { anilist_id: FRIEREN, episode: 4 }, 0)
            .unwrap();

        let tracker = MockTracker::unauthenticated();
        let report = drain(&store, &tracker, 100).await.unwrap();

        assert!(report.did_nothing());
        assert_eq!(report.remaining, 1);
        assert!(tracker.batches().is_empty(), "must not call an unauthenticated tracker");
    }

    #[tokio::test]
    async fn an_empty_queue_costs_no_request() {
        let store = Store::open_in_memory().unwrap();
        let tracker = MockTracker::new();
        assert!(drain(&store, &tracker, 100).await.unwrap().did_nothing());
        assert!(tracker.batches().is_empty());
    }

    #[tokio::test]
    async fn a_binge_offline_drains_as_one_op_on_reconnect() {
        // The whole offline story, end to end: twelve episodes with no network coalesce in the
        // outbox and become a single push.
        let store = Store::open_in_memory().unwrap();
        for ep in 1..=12 {
            store
                .enqueue("mock", &TrackOp::SetProgress { anilist_id: FRIEREN, episode: ep }, 0)
                .unwrap();
        }
        let tracker = MockTracker::new();
        let report = drain(&store, &tracker, 100).await.unwrap();

        assert_eq!(report.sent, 1);
        assert_eq!(
            tracker.batches()[0],
            vec![TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }]
        );
    }

    #[tokio::test]
    async fn pushing_the_same_progress_twice_is_harmless() {
        // The idempotence the batch-fate logic depends on.
        let store = Store::open_in_memory().unwrap();
        let op = TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 };
        store.enqueue("mock", &op, 0).unwrap();

        let tracker = MockTracker::new();
        drain(&store, &tracker, 100).await.unwrap();
        store.enqueue("mock", &op, 200).unwrap();
        let second = drain(&store, &tracker, 300).await.unwrap();

        assert_eq!(second.sent, 1);
        assert_eq!(tracker.batches().len(), 2);
    }

    #[tokio::test]
    async fn a_pull_queues_local_progress_when_local_is_ahead() {
        let store = store_with_completed(12);
        let tracker = MockTracker::with_remote(vec![TrackedEntry {
            anilist_id: FRIEREN,
            progress: 7,
            status: WatchStatus::Current,
            score: None,
        }]);

        let report = pull(&store, &tracker, 5_000, 0).await.unwrap();
        assert_eq!(report.seen, 1);
        assert_eq!(report.queued, 1);
        assert_eq!(store.outbox_depth(Some("mock")).unwrap(), 1);
    }

    #[tokio::test]
    async fn a_pull_adopts_the_remote_when_it_is_ahead() {
        // "Watched two episodes on my phone" — no prompt, no conflict.
        let store = store_with_completed(3);
        let tracker = MockTracker::with_remote(vec![TrackedEntry {
            anilist_id: FRIEREN,
            progress: 9,
            status: WatchStatus::Current,
            score: None,
        }]);

        let report = pull(&store, &tracker, 5_000, 0).await.unwrap();
        assert_eq!(report.adopted, 1);
        assert_eq!(report.queued, 0, "must not push a lower local value");
        assert_eq!(store.outbox_depth(Some("mock")).unwrap(), 0);
    }

    #[tokio::test]
    async fn a_pull_with_matching_progress_is_silent() {
        let store = store_with_completed(12);
        let tracker = MockTracker::with_remote(vec![TrackedEntry {
            anilist_id: FRIEREN,
            progress: 12,
            status: WatchStatus::Current,
            score: None,
        }]);

        let report = pull(&store, &tracker, 5_000, 0).await.unwrap();
        assert_eq!(report.queued, 0);
        assert_eq!(report.adopted, 0);
        assert!(report.conflicts.is_empty());
    }

    #[tokio::test]
    async fn a_pull_does_not_re_queue_something_already_pending() {
        // Pull-then-pull with no drain in between must not stack duplicate ops.
        let store = store_with_completed(12);
        let tracker = MockTracker::with_remote(vec![TrackedEntry {
            anilist_id: FRIEREN,
            progress: 7,
            status: WatchStatus::Current,
            score: None,
        }]);

        pull(&store, &tracker, 5_000, 0).await.unwrap();
        let second = pull(&store, &tracker, 6_000, 0).await.unwrap();

        assert_eq!(second.queued, 0, "re-queued an op that was already pending");
        assert_eq!(store.outbox_depth(Some("mock")).unwrap(), 1);
    }

    #[tokio::test]
    async fn an_unauthenticated_pull_is_a_no_op() {
        let store = store_with_completed(5);
        let tracker = MockTracker::unauthenticated();
        let report = pull(&store, &tracker, 100, 0).await.unwrap();
        assert_eq!(report.seen, 0);
        assert_eq!(*tracker.pulls.lock().unwrap(), 0);
    }

    #[test]
    fn progress_is_queued_from_completed_episodes_not_a_counter() {
        // Rewatching must not inflate progress, so the count comes from distinct completed
        // episodes in the log.
        let store = store_with_completed(12);
        let tracker: Arc<dyn Tracker> = Arc::new(MockTracker::new());
        queue_progress(&store, &[tracker], FRIEREN, 9_000);

        let queued = store.claim_ready("mock", 9_000, 10).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].op, TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 });

        // Rewatching episode 1 adds an event but not an episode.
        store
            .record_event(&WatchEvent {
                duration_secs: Some(1440.0),
                completed: true,
                at: 20_000,
                ..WatchEvent::new(FRIEREN, "1", 1400.0)
            })
            .unwrap();
        queue_progress(&store, &[], FRIEREN, 21_000);
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 12);
    }

    #[test]
    fn nothing_watched_queues_nothing() {
        // Opening a title to read the synopsis must not touch anyone's list.
        let store = Store::open_in_memory().unwrap();
        let tracker: Arc<dyn Tracker> = Arc::new(MockTracker::new());
        queue_progress(&store, &[tracker], FRIEREN, 100);
        assert_eq!(store.outbox_depth(None).unwrap(), 0);
    }

    #[test]
    fn progress_is_queued_for_every_tracker_independently() {
        // Per-tracker cursors are what stop a failing MAL push stalling AniList.
        struct Named(&'static str);
        #[async_trait::async_trait]
        impl Tracker for Named {
            fn id(&self) -> &str {
                self.0
            }
            fn is_authenticated(&self) -> bool {
                true
            }
            async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
                Ok(Vec::new())
            }
            async fn push(&self, _: &[TrackOp]) -> Result<(), anistream_core::Error> {
                Ok(())
            }
        }

        let store = store_with_completed(4);
        let trackers: Vec<Arc<dyn Tracker>> =
            vec![Arc::new(Named("anilist")), Arc::new(Named("mal"))];
        queue_progress(&store, &trackers, FRIEREN, 9_000);

        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
        assert_eq!(store.outbox_depth(Some("mal")).unwrap(), 1);
        assert_eq!(depth(&store), 2);
    }
}
