//! Reconciling local state with a tracker's.
//!
//! Pure functions over plain values, because this is the code that decides whether someone's
//! list gets overwritten. Every rule here is reachable from a test, and the interesting cases
//! are the ones where the two sides genuinely disagree.
//!
//! Three fields, three different rules, and conflating them is the usual bug:
//!
//! | Field | Rule | Why |
//! |---|---|---|
//! | progress | `max(local, remote)` | Monotonic. "Watched two on my phone" needs no ceremony. |
//! | status | last write wins, by timestamp | Not monotonic — `Dropped` → `Current` is a real edit. |
//! | score | last write wins, by timestamp | Same, and a score is deliberate rather than derived. |
//!
//! The asymmetry is the point. Progress can be merged without asking because moving it forward
//! is never destructive. Status and score can be genuinely contradictory, so when the two sides
//! disagree and we cannot tell which is newer, the divergence is *surfaced* rather than
//! resolved — silently overwriting a status the user set on the website is exactly the kind of
//! thing that makes people stop trusting a sync client.

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, WatchStatus},
};

/// Local state for one title, as the merge sees it.
///
/// Timestamps are `None` when local never set the field, which is the common case: progress
/// comes from watching, but status and score are only ever set deliberately.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocalState {
    pub progress: u32,
    pub status: Option<WatchStatus>,
    /// When the local status was last set.
    pub status_at: Option<i64>,
    pub score: Option<f32>,
    pub score_at: Option<i64>,
}

/// What reconciling one title produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Merged {
    /// Operations to push, so the remote catches up with local.
    pub push: Vec<TrackOp>,
    /// The value local should adopt, when the remote is ahead.
    pub adopt_progress: Option<u32>,
    /// Disagreements the user has to settle.
    pub conflicts: Vec<Conflict>,
}

impl Merged {
    pub fn is_empty(&self) -> bool {
        self.push.is_empty() && self.adopt_progress.is_none() && self.conflicts.is_empty()
    }
}

/// A disagreement that cannot be resolved without asking.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub anilist_id: AnilistId,
    pub field: Field,
    pub local: String,
    pub remote: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Status,
    Score,
}

impl Field {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Score => "score",
        }
    }
}

/// Scores differing by less than this are the same score.
///
/// AniList stores scores as floats across several user-selectable scales (100-point, 10-point,
/// 5-star), so a round-trip can perturb the last bit. Treating that as a conflict would prompt
/// the user about nothing.
const SCORE_EPSILON: f32 = 0.01;

/// Reconcile one title.
///
/// `pending_progress` is the highest episode already queued in the outbox. Without it a push
/// that has not drained yet looks like the remote being ahead, and the merge would helpfully
/// walk local progress *backwards* to the stale remote value.
pub fn reconcile(
    local: &LocalState,
    remote: &TrackedEntry,
    pending_progress: Option<u32>,
) -> Merged {
    let mut merged = Merged::default();

    // Progress: monotonic, so the higher value simply wins and nobody needs to be asked.
    let local_effective = local.progress.max(pending_progress.unwrap_or(0));
    match local_effective.cmp(&remote.progress) {
        std::cmp::Ordering::Greater => {
            // Only queue if it is not already queued — the outbox coalesces, but skipping the
            // call keeps the drain quiet on a no-op resync.
            if pending_progress != Some(local_effective) {
                merged.push.push(TrackOp::SetProgress {
                    anilist_id: remote.anilist_id,
                    episode: local_effective,
                });
            }
        }
        std::cmp::Ordering::Less => merged.adopt_progress = Some(remote.progress),
        std::cmp::Ordering::Equal => {}
    }

    // Status: last write wins, and an unknowable ordering is a conflict rather than a guess.
    match (local.status, local.status_at) {
        (Some(status), _) if status == remote.status => {}
        (Some(status), Some(at)) => {
            // We know when local changed but never when the remote did, so a local edit is
            // only allowed to win if it is *newer than the pull*. The caller passes the pull
            // time as the remote's timestamp; see `reconcile_at`.
            merged.push.push(TrackOp::SetStatus { anilist_id: remote.anilist_id, status, at });
        }
        (Some(status), None) => {
            // Local has an opinion with no idea when it was formed. Refusing to guess is the
            // whole reason the Conflicts overlay exists.
            merged.conflicts.push(Conflict {
                anilist_id: remote.anilist_id,
                field: Field::Status,
                local: format!("{status:?}"),
                remote: format!("{:?}", remote.status),
            });
        }
        (None, _) => {}
    }

    // Score: same shape as status.
    match (local.score, local.score_at) {
        (Some(score), _) if scores_match(Some(score), remote.score) => {}
        (Some(score), Some(at)) => {
            merged.push.push(TrackOp::SetScore { anilist_id: remote.anilist_id, score, at });
        }
        (Some(score), None) => merged.conflicts.push(Conflict {
            anilist_id: remote.anilist_id,
            field: Field::Score,
            local: format!("{score}"),
            remote: remote.score.map_or_else(|| "—".into(), |s| format!("{s}")),
        }),
        (None, _) => {}
    }

    merged
}

/// Reconcile with knowledge of when the remote value was last observed.
///
/// This is the version worth using when a previous pull recorded a timestamp: a local status
/// older than the last successful pull has already been superseded remotely, so pushing it
/// would undo whatever the user did on the website.
pub fn reconcile_at(
    local: &LocalState,
    remote: &TrackedEntry,
    pending_progress: Option<u32>,
    remote_observed_at: i64,
) -> Merged {
    let mut merged = reconcile(local, remote, pending_progress);

    // Drop any push whose local timestamp predates the remote observation, and record the
    // disagreement instead. Progress is exempt: it is monotonic, so it is never destructive.
    let mut kept = Vec::with_capacity(merged.push.len());
    for op in merged.push.drain(..) {
        let (at, field, local_repr, remote_repr) = match &op {
            TrackOp::SetProgress { .. } => {
                kept.push(op);
                continue;
            }
            TrackOp::SetStatus { status, at, .. } => {
                (*at, Field::Status, format!("{status:?}"), format!("{:?}", remote.status))
            }
            TrackOp::SetScore { score, at, .. } => (
                *at,
                Field::Score,
                format!("{score}"),
                remote.score.map_or_else(|| "—".into(), |s| format!("{s}")),
            ),
        };

        if at >= remote_observed_at {
            kept.push(op);
        } else {
            merged.conflicts.push(Conflict {
                anilist_id: remote.anilist_id,
                field,
                local: local_repr,
                remote: remote_repr,
            });
        }
    }
    merged.push = kept;
    merged
}

/// Whether a title present locally but absent from the remote list should be pushed.
///
/// Only if something was actually watched. Opening a title to read the synopsis must not add it
/// to someone's list — a sync client that silently populates your AniList with everything you
/// glanced at is worse than one that does nothing.
pub fn should_add_to_remote(local: &LocalState) -> bool {
    local.progress > 0 || local.status.is_some() || local.score.is_some()
}

fn scores_match(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => (x - y).abs() < SCORE_EPSILON,
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    fn remote(progress: u32, status: WatchStatus, score: Option<f32>) -> TrackedEntry {
        TrackedEntry { anilist_id: FRIEREN, progress, status, score }
    }

    #[test]
    fn the_higher_progress_wins_in_either_direction() {
        // The whole reason progress needs no prompting: it only ever moves forward, so the
        // maximum is always right.
        let ahead = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&ahead, &remote(7, WatchStatus::Current, None), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }]
        );
        assert_eq!(merged.adopt_progress, None);

        let behind = LocalState { progress: 3, ..Default::default() };
        let merged = reconcile(&behind, &remote(9, WatchStatus::Current, None), None);
        assert!(merged.push.is_empty());
        assert_eq!(merged.adopt_progress, Some(9), "watched elsewhere should be adopted");
    }

    #[test]
    fn equal_progress_produces_no_work() {
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&local, &remote(12, WatchStatus::Current, None), None);
        assert!(merged.is_empty(), "a no-op resync must not generate traffic: {merged:?}");
    }

    #[test]
    fn a_queued_push_is_not_mistaken_for_the_remote_being_ahead() {
        // The bug this prevents: local shows 3 because the projection has not caught up, the
        // outbox already holds 12, and the remote still says 7. Without the pending value the
        // merge would adopt 7 and walk progress backwards.
        let local = LocalState { progress: 3, ..Default::default() };
        let merged = reconcile(&local, &remote(7, WatchStatus::Current, None), Some(12));
        assert_eq!(merged.adopt_progress, None, "adopted a stale remote over a queued push");
        assert!(merged.push.is_empty(), "12 is already queued; queueing it again is noise");
    }

    #[test]
    fn a_queued_push_below_local_still_pushes_local() {
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&local, &remote(2, WatchStatus::Current, None), Some(5));
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }]
        );
    }

    #[test]
    fn a_matching_status_is_left_alone() {
        let local = LocalState {
            progress: 5,
            status: Some(WatchStatus::Current),
            status_at: Some(1_000),
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(5, WatchStatus::Current, None), None);
        assert!(merged.is_empty());
    }

    #[test]
    fn a_timestamped_local_status_is_pushed() {
        let local = LocalState {
            status: Some(WatchStatus::Completed),
            status_at: Some(2_000),
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, None), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetStatus {
                anilist_id: FRIEREN,
                status: WatchStatus::Completed,
                at: 2_000
            }]
        );
        assert!(merged.conflicts.is_empty());
    }

    #[test]
    fn a_status_with_no_timestamp_is_surfaced_rather_than_guessed() {
        // Status is not monotonic, so with no way to order the two edits, picking one would
        // be overwriting someone's deliberate choice on a coin flip.
        let local = LocalState {
            status: Some(WatchStatus::Dropped),
            status_at: None,
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, None), None);
        assert!(merged.push.is_empty(), "must not push an unorderable status");
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!(merged.conflicts[0].field, Field::Status);
        assert_eq!(merged.conflicts[0].local, "Dropped");
        assert_eq!(merged.conflicts[0].remote, "Current");
    }

    #[test]
    fn a_local_status_older_than_the_last_pull_loses() {
        // The user set it to Dropped locally, then changed it to Current on the website. The
        // website edit is newer, so pushing the local value would undo it.
        let local = LocalState {
            status: Some(WatchStatus::Dropped),
            status_at: Some(1_000),
            ..Default::default()
        };
        let merged = reconcile_at(&local, &remote(0, WatchStatus::Current, None), None, 5_000);
        assert!(merged.push.is_empty(), "a stale local status overwrote a newer remote one");
        assert_eq!(merged.conflicts.len(), 1, "the divergence still has to be visible");
    }

    #[test]
    fn a_local_status_newer_than_the_last_pull_wins() {
        let local = LocalState {
            status: Some(WatchStatus::Completed),
            status_at: Some(9_000),
            ..Default::default()
        };
        let merged = reconcile_at(&local, &remote(0, WatchStatus::Current, None), None, 5_000);
        assert_eq!(merged.push.len(), 1);
        assert!(merged.conflicts.is_empty());
    }

    #[test]
    fn progress_is_never_held_back_by_the_pull_timestamp() {
        // Progress is monotonic, so it is exempt from last-write-wins entirely. Making it
        // wait for a fresher timestamp would lose episodes for no benefit.
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile_at(&local, &remote(7, WatchStatus::Current, None), None, 9_999);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 }]
        );
    }

    #[test]
    fn scores_that_only_differ_by_float_noise_are_the_same_score() {
        // AniList stores scores as floats across several scales, so a round-trip can perturb
        // the last bit. Prompting about that would be prompting about nothing.
        let local =
            LocalState { score: Some(8.5), score_at: Some(1_000), ..Default::default() };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, Some(8.500_001)), None);
        assert!(merged.is_empty(), "float noise was treated as a conflict: {merged:?}");
    }

    #[test]
    fn a_genuinely_different_score_is_pushed() {
        let local =
            LocalState { score: Some(9.0), score_at: Some(1_000), ..Default::default() };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, Some(6.0)), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetScore { anilist_id: FRIEREN, score: 9.0, at: 1_000 }]
        );
    }

    #[test]
    fn a_score_only_on_the_remote_is_left_alone() {
        // Local has no opinion, so there is nothing to push and nothing to resolve.
        let merged = reconcile(
            &LocalState::default(),
            &remote(0, WatchStatus::Current, Some(7.0)),
            None,
        );
        assert!(merged.is_empty());
    }

    #[test]
    fn glancing_at_a_title_does_not_add_it_to_your_list() {
        // The distinction that keeps a sync client trustworthy: opening a title to read the
        // synopsis writes nothing anywhere.
        assert!(!should_add_to_remote(&LocalState::default()));
        assert!(should_add_to_remote(&LocalState { progress: 1, ..Default::default() }));
        assert!(should_add_to_remote(&LocalState {
            status: Some(WatchStatus::Planning),
            ..Default::default()
        }));
        assert!(should_add_to_remote(&LocalState { score: Some(8.0), ..Default::default() }));
    }

    #[test]
    fn every_op_produced_is_addressed_to_the_right_title() {
        // A merge that emitted an op for the wrong id would corrupt an unrelated entry.
        let local = LocalState {
            progress: 12,
            status: Some(WatchStatus::Completed),
            status_at: Some(2_000),
            score: Some(9.0),
            score_at: Some(2_000),
        };
        let merged = reconcile(&local, &remote(1, WatchStatus::Current, Some(3.0)), None);
        assert_eq!(merged.push.len(), 3);
        for op in &merged.push {
            assert_eq!(op.anilist_id(), FRIEREN);
        }
    }
}
