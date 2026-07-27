//! The AniList [`Tracker`] implementation.
//!
//! The reference tracker, and the one the rest of the app is keyed on — AniList media ids are
//! the primary key everywhere, so this needs no mapping layer at all. Every other tracker will
//! have to go through `anistream-meta::mapping` to translate ids; this one does not, which is
//! why it lands first.

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};
use anistream_meta::anilist::{AniList, AniListError, LibraryEntry};

/// AniList's `MediaListStatus` enum, as strings.
///
/// Kept as an explicit pair of functions rather than serde attributes because the mapping is
/// part of a wire contract with someone else's API — a rename on our side must not silently
/// start writing a different status to somebody's list.
pub const fn status_to_anilist(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Current => "CURRENT",
        WatchStatus::Planning => "PLANNING",
        WatchStatus::Completed => "COMPLETED",
        WatchStatus::Paused => "PAUSED",
        WatchStatus::Dropped => "DROPPED",
        WatchStatus::Repeating => "REPEATING",
    }
}

/// Parse AniList's status back.
///
/// An unrecognised value maps to `Current` rather than failing: AniList adding a status we have
/// never heard of should not blank someone's library.
pub fn status_from_anilist(status: &str) -> WatchStatus {
    match status {
        "PLANNING" => WatchStatus::Planning,
        "COMPLETED" => WatchStatus::Completed,
        "PAUSED" => WatchStatus::Paused,
        "DROPPED" => WatchStatus::Dropped,
        "REPEATING" => WatchStatus::Repeating,
        _ => WatchStatus::Current,
    }
}

/// The AniList tracker.
pub struct AniListTracker {
    client: AniList,
    /// Resolved on first use and cached — the library query needs a user id, and asking for it
    /// on every pull would waste one of thirty requests a minute.
    viewer: tokio::sync::Mutex<Option<u32>>,
    /// Set when the user signs out.
    ///
    /// The client holds its token from construction, and clearing the *stored* token does not
    /// revoke it at AniList — so without this the tracker would keep reporting itself connected and
    /// keep pushing with a credential the user had just withdrawn. Signing out has to mean
    /// something immediately, not at the next restart.
    signed_out: std::sync::atomic::AtomicBool,
}

impl AniListTracker {
    /// `client` must already carry the token; an unauthenticated one degrades to local-only.
    pub fn new(client: AniList) -> Self {
        Self {
            client,
            viewer: tokio::sync::Mutex::new(None),
            signed_out: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The full library, with media attached, for the Library screen.
    ///
    /// [`Tracker::pull_library`] returns only the syncable projection; this returns everything
    /// AniList sent, so the screen does not need a second pass of lookups.
    pub async fn library(&self) -> Result<Vec<LibraryEntry>, anistream_core::Error> {
        let viewer = self.viewer_id().await?;
        self.client.library(viewer).await.map_err(to_core)
    }

    /// Remove an entry from the user's list.
    ///
    /// Takes the `MediaList` entry id from [`anistream_meta::anilist::LibraryEntry::entry_id`].
    /// Not part of the [`Tracker`] trait: deleting is not something sync ever does — the outbox
    /// only ever moves progress forward — so it stays off the interface every tracker must
    /// implement.
    pub async fn delete(&self, entry_id: u32) -> Result<bool, anistream_core::Error> {
        self.client.delete_entry(entry_id).await.map_err(to_core)
    }

    async fn viewer_id(&self) -> Result<u32, anistream_core::Error> {
        let mut cached = self.viewer.lock().await;
        if let Some(id) = *cached {
            return Ok(id);
        }
        let id = self.client.viewer_id().await.map_err(to_core)?;
        *cached = Some(id);
        Ok(id)
    }
}

#[async_trait::async_trait]
impl Tracker for AniListTracker {
    fn id(&self) -> &str {
        "anilist"
    }

    fn is_authenticated(&self) -> bool {
        !self.signed_out.load(std::sync::atomic::Ordering::Relaxed)
            && self.client.is_authenticated()
    }

    async fn accept_credentials(
        &self,
        access: &str,
        _refresh: Option<&str>,
        _expires_at: Option<i64>,
    ) {
        self.client.set_token(Some(access.to_owned()));
        self.signed_out.store(false, std::sync::atomic::Ordering::Relaxed);
        // The cached viewer id belonged to whoever was signed in before, and a library pull keyed on
        // it would fetch the wrong account's list.
        *self.viewer.lock().await = None;
    }

    async fn forget_credentials(&self) {
        self.signed_out.store(true, std::sync::atomic::Ordering::Relaxed);
        // Drop the cached viewer id too, so signing into a *different* account cannot inherit the
        // previous one's list.
        *self.viewer.lock().await = None;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
        Ok(self
            .library()
            .await?
            .into_iter()
            .map(|entry| TrackedEntry {
                anilist_id: entry.media.id,
                progress: entry.progress,
                status: status_from_anilist(&entry.status),
                score: entry.score,
            })
            .collect())
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
        // Checked per push, not just at startup: the outbox drains on a timer, and a queued op must
        // not reach an account the user has signed out of.
        if !self.is_authenticated() {
            return Err(anistream_core::Error::Auth("signed out".into()));
        }
        for entry in coalesce(ops) {
            self.client
                .save_entry(entry.anilist_id, entry.progress, entry.status, entry.score)
                .await
                .map_err(to_core)?;
        }
        Ok(())
    }
}

/// One title's worth of changes, as a single mutation.
#[derive(Debug, Clone, PartialEq)]
pub struct Mutation {
    pub anilist_id: AnilistId,
    /// `None` fields are omitted from the mutation rather than sent as null, because AniList
    /// treats null as *clear this* — a progress push would otherwise wipe the user's score.
    pub progress: Option<u32>,
    pub status: Option<&'static str>,
    pub score: Option<f32>,
}

/// Fold a batch of ops into one mutation per title.
///
/// `SaveMediaListEntry` sets whichever fields are present, so three ops for one show are one
/// request. Sending them separately would burn three of thirty requests a minute for no benefit.
/// Extracted so the batching rules are testable without a network.
pub fn coalesce(ops: &[TrackOp]) -> Vec<Mutation> {
    let mut batched: Vec<Mutation> = Vec::new();

    for op in ops {
        let id = op.anilist_id();
        let slot = match batched.iter_mut().find(|m| m.anilist_id == id) {
            Some(slot) => slot,
            None => {
                batched.push(Mutation {
                    anilist_id: id,
                    progress: None,
                    status: None,
                    score: None,
                });
                batched.last_mut().expect("just pushed")
            }
        };
        match op {
            // Highest wins within a batch: progress is monotonic, and a later op carrying a
            // lower episode would otherwise undo the earlier one.
            TrackOp::SetProgress { episode, .. } => {
                slot.progress = Some(slot.progress.map_or(*episode, |c| c.max(*episode)));
            }
            TrackOp::SetStatus { status, .. } => slot.status = Some(status_to_anilist(*status)),
            TrackOp::SetScore { score, .. } => slot.score = Some(*score),
        }
    }
    batched
}

/// Map an AniList failure onto the core error, preserving whether it is worth retrying.
fn to_core(error: AniListError) -> anistream_core::Error {
    match error {
        // A bad or expired token will never succeed on retry, so it must not sit in the outbox
        // burning backoff — the user has to re-authorise.
        AniListError::Unauthenticated => {
            anistream_core::Error::Auth("anilist rejected the token".into())
        }
        other => anistream_core::Error::Tracker {
            tracker: "anilist".into(),
            message: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn every_status_survives_a_round_trip() {
        // These strings go into someone's list. A mismatch would quietly mark a show dropped.
        for status in [
            WatchStatus::Current,
            WatchStatus::Planning,
            WatchStatus::Completed,
            WatchStatus::Paused,
            WatchStatus::Dropped,
            WatchStatus::Repeating,
        ] {
            assert_eq!(status_from_anilist(status_to_anilist(status)), status);
        }
    }

    #[test]
    fn the_wire_strings_are_anilists_and_not_ours() {
        // Pinned deliberately: these are someone else's enum, so a local rename must not
        // change what goes over the wire.
        assert_eq!(status_to_anilist(WatchStatus::Current), "CURRENT");
        assert_eq!(status_to_anilist(WatchStatus::Repeating), "REPEATING");
    }

    #[test]
    fn an_unknown_status_degrades_rather_than_failing() {
        // If AniList adds a status, a library pull should still work.
        assert_eq!(status_from_anilist("SOMETHING_NEW"), WatchStatus::Current);
        assert_eq!(status_from_anilist(""), WatchStatus::Current);
    }

    fn mutation(
        id: AnilistId,
        progress: Option<u32>,
        status: Option<&'static str>,
        score: Option<f32>,
    ) -> Mutation {
        Mutation { anilist_id: id, progress, status, score }
    }

    #[test]
    fn three_ops_for_one_title_become_one_mutation() {
        // Thirty requests a minute is the whole budget, so a per-op mutation would make a
        // resync of twenty titles take a minute.
        let ops = [
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 },
            TrackOp::SetStatus { anilist_id: FRIEREN, status: WatchStatus::Completed, at: 1 },
            TrackOp::SetScore { anilist_id: FRIEREN, score: 9.0, at: 1 },
        ];
        let batched = coalesce(&ops);
        assert_eq!(batched.len(), 1);
        assert_eq!(batched[0], mutation(FRIEREN, Some(12), Some("COMPLETED"), Some(9.0)));
    }

    #[test]
    fn batching_keeps_the_highest_progress() {
        // Two progress ops in one batch must not let the later, lower one win.
        let ops = [
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 12 },
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 4 },
        ];
        assert_eq!(
            coalesce(&ops)[0].progress,
            Some(12),
            "progress walked backwards inside a batch"
        );
    }

    #[test]
    fn different_titles_stay_separate() {
        let other = AnilistId::new(1);
        let ops = [
            TrackOp::SetProgress { anilist_id: FRIEREN, episode: 3 },
            TrackOp::SetProgress { anilist_id: other, episode: 7 },
        ];
        let batched = coalesce(&ops);
        assert_eq!(batched.len(), 2);
        assert_eq!(batched[0], mutation(FRIEREN, Some(3), None, None));
        assert_eq!(batched[1], mutation(other, Some(7), None, None));
    }

    #[test]
    fn a_progress_only_push_leaves_status_and_score_untouched() {
        // The reason unset fields are omitted rather than nulled: AniList treats null as
        // "clear this", so a progress push would wipe the user's score.
        let ops = [TrackOp::SetProgress { anilist_id: FRIEREN, episode: 5 }];
        assert_eq!(coalesce(&ops)[0], mutation(FRIEREN, Some(5), None, None));
    }

    #[test]
    fn an_empty_batch_sends_nothing() {
        assert!(coalesce(&[]).is_empty());
    }

    #[test]
    fn a_rejected_token_is_an_auth_error_not_a_retryable_one() {
        // It must not sit in the outbox retrying forever — nothing but re-authorising fixes it.
        assert!(matches!(
            to_core(AniListError::Unauthenticated),
            anistream_core::Error::Auth(_)
        ));
        assert!(matches!(
            to_core(AniListError::Network("timeout".into())),
            anistream_core::Error::Tracker { .. }
        ));
    }
}
