//! Durable per-tracker sync queue.
//!
//! Deliberately a table and not an in-memory channel. Progress recorded on a plane, or
//! while AniList is having an outage, has to survive the process exiting — otherwise
//! "watched six episodes offline" silently evaporates, which is the single worst thing a
//! tracking client can do.
//!
//! Two properties make this safe to retry aggressively:
//!
//! - **Coalescing.** Progress is monotonic, so only the highest pending episode per title
//!   is worth sending. Bingeing twelve episodes offline leaves one queued op, not twelve.
//! - **Per-tracker cursors.** Each tracker drains independently, so a failing MAL push
//!   cannot stall AniList.

use anistream_core::{ids::AnilistId, traits::TrackOp};

use crate::{Result, Store, StoreError};

/// Base delay for the retry backoff, in seconds.
const BACKOFF_BASE_SECS: i64 = 30;

/// Ceiling on the backoff. An hour is long enough to ride out an outage without leaving
/// a recovered tracker waiting for days.
const BACKOFF_MAX_SECS: i64 = 3_600;

/// A queued operation.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxEntry {
    pub id: i64,
    pub tracker_id: String,
    pub op: TrackOp,
    pub attempts: u32,
    pub created_at: i64,
}

/// Delay before the next attempt after `attempts` failures.
///
/// Exponential with a cap, so a persistently broken tracker stops generating traffic but
/// still recovers on its own once it comes back.
pub fn backoff_secs(attempts: u32) -> i64 {
    BACKOFF_BASE_SECS.saturating_mul(1_i64 << attempts.min(10)).min(BACKOFF_MAX_SECS)
}

impl Store {
    /// Queue an operation for a tracker.
    ///
    /// `SetProgress` coalesces against any pending op for the same title: because progress
    /// only ever moves forward, a queued "episode 12" makes a queued "episode 7"
    /// meaningless. A lower episode is dropped outright rather than queued behind a
    /// higher one, which would otherwise walk a tracker's progress backwards.
    pub fn enqueue(&self, tracker_id: &str, op: &TrackOp, at: i64) -> Result<Option<i64>> {
        let anilist_id = op.anilist_id().get();
        let payload = serde_json::to_string(op)
            .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;

        self.with_tx(|tx| {
            if let TrackOp::SetProgress { episode, .. } = op {
                let existing: Option<(i64, String)> = tx
                    .query_row(
                        "SELECT id, op FROM sync_outbox
                          WHERE tracker_id = ?1 AND anilist_id = ?2
                            AND json_extract(op, '$.op') = 'set_progress'
                          LIMIT 1",
                        rusqlite::params![tracker_id, anilist_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();

                if let Some((existing_id, existing_json)) = existing {
                    let pending: TrackOp = serde_json::from_str(&existing_json)
                        .map_err(|source| StoreError::Encode { what: "TrackOp", source })?;
                    let pending_episode = match pending {
                        TrackOp::SetProgress { episode, .. } => episode,
                        _ => 0,
                    };
                    if *episode <= pending_episode {
                        // Already covered by a queued op at or beyond this episode.
                        return Ok(None);
                    }
                    tx.execute(
                        "UPDATE sync_outbox
                            SET op = ?1, created_at = ?2, attempts = 0, next_retry = 0,
                                last_error = NULL
                          WHERE id = ?3",
                        rusqlite::params![&payload, at, existing_id],
                    )?;
                    return Ok(Some(existing_id));
                }
            }

            tx.execute(
                "INSERT INTO sync_outbox (tracker_id, op, anilist_id, created_at, next_retry)
                 VALUES (?1, ?2, ?3, ?4, 0)",
                rusqlite::params![tracker_id, &payload, anilist_id, at],
            )?;
            Ok(Some(tx.last_insert_rowid()))
        })
    }

    /// Operations due for sending, oldest first.
    pub fn claim_ready(
        &self,
        tracker_id: &str,
        now: i64,
        limit: u32,
    ) -> Result<Vec<OutboxEntry>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT id, tracker_id, op, attempts, created_at
                   FROM sync_outbox
                  WHERE tracker_id = ?1 AND next_retry <= ?2
                  ORDER BY created_at ASC, id ASC
                  LIMIT ?3",
            )?;
            let rows = stmt.query_map(rusqlite::params![tracker_id, now, limit], |r| {
                let json: String = r.get(2)?;
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    json,
                    r.get::<_, u32>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?;

            let mut out = Vec::new();
            for row in rows {
                let (id, tracker, json, attempts, created_at) = row?;
                match serde_json::from_str(&json) {
                    Ok(op) => out.push(OutboxEntry {
                        id,
                        tracker_id: tracker,
                        op,
                        attempts,
                        created_at,
                    }),
                    // A row we can no longer decode would block the queue head forever.
                    // Log and skip rather than wedge every later op behind it.
                    Err(e) => {
                        tracing::error!(id, error = %e, "undecodable outbox row, skipping");
                    }
                }
            }
            Ok(out)
        })
    }

    /// Remove a successfully-sent operation.
    pub fn complete(&self, id: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute("DELETE FROM sync_outbox WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// Record a failure and schedule a retry.
    pub fn fail(&self, id: i64, error: &str, now: i64) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE sync_outbox
                    SET attempts = attempts + 1,
                        last_error = ?1,
                        next_retry = ?2 + ?3
                  WHERE id = ?4",
                rusqlite::params![
                    error,
                    now,
                    // Backoff for the attempt count *after* this failure.
                    c.query_row(
                        "SELECT attempts + 1 FROM sync_outbox WHERE id = ?1",
                        [id],
                        |r| r.get::<_, u32>(0)
                    )
                    .map(backoff_secs)
                    .unwrap_or(BACKOFF_BASE_SECS),
                    id
                ],
            )?;
            Ok(())
        })
    }

    /// Number of pending operations, for the `⇅` badge in the status line.
    pub fn outbox_depth(&self, tracker_id: Option<&str>) -> Result<u32> {
        self.with_conn(|c| {
            let depth = match tracker_id {
                Some(t) => c.query_row(
                    "SELECT COUNT(*) FROM sync_outbox WHERE tracker_id = ?1",
                    [t],
                    |r| r.get(0),
                )?,
                None => c.query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))?,
            };
            Ok(depth)
        })
    }

    /// Highest queued progress for a title, if any. Used when reconciling with a remote
    /// value so an in-flight local push is not treated as absent.
    pub fn pending_progress(
        &self,
        tracker_id: &str,
        anilist_id: AnilistId,
    ) -> Result<Option<u32>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT MAX(CAST(json_extract(op, '$.episode') AS INTEGER))
                   FROM sync_outbox
                  WHERE tracker_id = ?1 AND anilist_id = ?2
                    AND json_extract(op, '$.op') = 'set_progress'",
                rusqlite::params![tracker_id, anilist_id.get()],
                |r| r.get::<_, Option<u32>>(0),
            )?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::traits::WatchStatus;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    fn progress(episode: u32) -> TrackOp {
        TrackOp::SetProgress { anilist_id: FRIEREN, episode }
    }

    #[test]
    fn queued_ops_survive_and_are_claimable() {
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(11), 1_000).unwrap().unwrap();

        let ready = store.claim_ready("anilist", 1_000, 10).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].op, progress(11));
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
    }

    #[test]
    fn a_binge_offline_coalesces_into_one_pending_push() {
        // Twelve episodes watched with no network should not produce twelve queued ops;
        // progress is monotonic so only the furthest point matters.
        let store = Store::open_in_memory().unwrap();
        for ep in 1..=12 {
            store.enqueue("anilist", &progress(ep), 1_000 + i64::from(ep)).unwrap();
        }
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
        let ready = store.claim_ready("anilist", 2_000, 10).unwrap();
        assert_eq!(ready[0].op, progress(12));
    }

    #[test]
    fn progress_never_walks_backwards() {
        // A stale lower episode arriving after a higher one must be dropped, not queued
        // behind it — otherwise the tracker would end up regressing.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(12), 1_000).unwrap();
        let dropped = store.enqueue("anilist", &progress(7), 1_100).unwrap();
        assert_eq!(dropped, None, "lower episode should be discarded");
        assert_eq!(store.claim_ready("anilist", 2_000, 10).unwrap()[0].op, progress(12));
    }

    #[test]
    fn status_ops_are_not_coalesced_with_progress() {
        // Status is not monotonic and carries its own timestamp, so it must queue
        // separately rather than being folded into a progress update.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(12), 1_000).unwrap();
        store
            .enqueue(
                "anilist",
                &TrackOp::SetStatus {
                    anilist_id: FRIEREN,
                    status: WatchStatus::Completed,
                    at: 1_100,
                },
                1_100,
            )
            .unwrap();
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 2);
    }

    #[test]
    fn trackers_drain_independently() {
        // A broken MAL push must not stall AniList.
        let store = Store::open_in_memory().unwrap();
        store.enqueue("anilist", &progress(11), 1_000).unwrap();
        store.enqueue("mal", &progress(11), 1_000).unwrap();

        let mal = store.claim_ready("mal", 1_000, 10).unwrap();
        store.fail(mal[0].id, "503 from MAL", 1_000).unwrap();

        // MAL is now backed off, AniList is untouched and still ready.
        assert!(store.claim_ready("mal", 1_000, 10).unwrap().is_empty());
        assert_eq!(store.claim_ready("anilist", 1_000, 10).unwrap().len(), 1);
    }

    #[test]
    fn failures_back_off_exponentially_then_become_ready_again() {
        let store = Store::open_in_memory().unwrap();
        let id = store.enqueue("anilist", &progress(11), 0).unwrap().unwrap();

        store.fail(id, "network down", 0).unwrap();
        assert!(
            store.claim_ready("anilist", 10, 10).unwrap().is_empty(),
            "should be waiting out the backoff"
        );

        let ready = store.claim_ready("anilist", backoff_secs(1) + 1, 10).unwrap();
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].attempts, 1);
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff_secs(0), 30);
        assert_eq!(backoff_secs(1), 60);
        assert_eq!(backoff_secs(2), 120);
        assert_eq!(backoff_secs(30), BACKOFF_MAX_SECS, "must not overflow or run away");
        // Monotonic up to the cap.
        for a in 0..8 {
            assert!(backoff_secs(a) <= backoff_secs(a + 1));
        }
    }

    #[test]
    fn completing_removes_the_op() {
        let store = Store::open_in_memory().unwrap();
        let id = store.enqueue("anilist", &progress(11), 0).unwrap().unwrap();
        store.complete(id).unwrap();
        assert_eq!(store.outbox_depth(None).unwrap(), 0);
    }

    #[test]
    fn pending_progress_reports_the_highest_queued_episode() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.pending_progress("anilist", FRIEREN).unwrap(), None);
        store.enqueue("anilist", &progress(5), 0).unwrap();
        store.enqueue("anilist", &progress(9), 0).unwrap();
        assert_eq!(store.pending_progress("anilist", FRIEREN).unwrap(), Some(9));
    }

    #[test]
    fn ops_round_trip_through_json_unchanged() {
        // The outbox stores serialised ops, so a representation change would silently
        // orphan queued rows.
        let store = Store::open_in_memory().unwrap();
        let ops = [
            progress(3),
            TrackOp::SetStatus { anilist_id: FRIEREN, status: WatchStatus::Paused, at: 7 },
            TrackOp::SetScore { anilist_id: FRIEREN, score: 8.5, at: 9 },
        ];
        for (i, op) in ops.iter().enumerate() {
            store.enqueue("t", op, i as i64).unwrap();
        }
        let claimed = store.claim_ready("t", 100, 10).unwrap();
        assert_eq!(claimed.len(), 3);
        for (expected, got) in ops.iter().zip(claimed.iter()) {
            assert_eq!(&got.op, expected);
        }
    }
}
