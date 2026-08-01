//! Watch history: the append-only log and the projection the UI reads.
//!
//! Two tables, two jobs. `watch_event` is an immutable log of everything that happened —
//! position, how long was actually watched, which provider served it, which translation.
//! `watch_progress` is a denormalised projection of the latest state per title, so the
//! `CONTINUE` rail and resume prompt are a single indexed read rather than an aggregate
//! over the log.
//!
//! Keeping both is deliberate. The log holds detail no tracker can represent, and the
//! projection keeps the hot path fast. Deriving the projection on every read would make
//! startup scale with how much you have ever watched.

use anistream_core::{ids::AnilistId, media::Translation};
use rusqlite::OptionalExtension;

use crate::{Result, Store, now};

/// One playback observation.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchEvent {
    pub anilist_id: AnilistId,
    pub episode: String,
    /// Playhead position in seconds.
    pub position_secs: f64,
    /// Total runtime, when mpv has reported it.
    pub duration_secs: Option<f64>,
    /// Seconds actually watched during this session — distinct from `position_secs`,
    /// which a single seek to the end would satisfy.
    pub watched_secs: f64,
    pub provider_id: Option<String>,
    pub translation: Option<Translation>,
    /// Whether this observation crossed the commit threshold.
    pub completed: bool,
    pub at: i64,
}

impl WatchEvent {
    pub fn new(anilist_id: AnilistId, episode: impl Into<String>, position_secs: f64) -> Self {
        Self {
            anilist_id,
            episode: episode.into(),
            position_secs,
            duration_secs: None,
            watched_secs: 0.0,
            provider_id: None,
            translation: None,
            completed: false,
            at: now(),
        }
    }
}

/// Current state for one title, as shown in the `CONTINUE` rail.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub anilist_id: AnilistId,
    pub last_episode: String,
    pub last_position: f64,
    pub last_duration: Option<f64>,
    /// Count of distinct episodes that crossed the commit threshold.
    pub episodes_done: u32,
    pub updated_at: i64,
}

impl Progress {
    /// Fraction of the last episode watched, for the progress meter.
    pub fn fraction(&self) -> f64 {
        match self.last_duration {
            Some(d) if d > 0.0 => (self.last_position / d).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Whether resuming is worth offering.
    ///
    /// Suppressed at both ends: the first few seconds are indistinguishable from starting
    /// fresh, and a position past the commit threshold means the episode is effectively
    /// finished, so the right offer is the *next* episode rather than the last 90 seconds
    /// of this one.
    pub fn is_resumable(&self, threshold: f64) -> bool {
        self.last_position > MIN_RESUME_SECS && self.fraction() < threshold
    }
}

/// Below this, resuming is indistinguishable from starting fresh, and the prompt is noise.
pub const MIN_RESUME_SECS: f64 = 30.0;

/// Above this fraction, an episode is close enough to over that resuming would land in the credits.
///
/// Its own constant rather than the sync threshold, and the distinction is the whole point:
/// `playback.commit_threshold` decides when an episode *counts* and gets pushed to a tracker, which
/// is a deliberately conservative 85% so that opening one to check the subtitles does not mark it
/// seen. Whether you can *pick an episode back up* is an unrelated question — a viewer who quit
/// halfway through fully intends to return, and answering it with the sync threshold made the
/// single most wanted action on the home screen the one thing that never appeared.
///
/// Passing the sync threshold in also had a trap: `is_complete` compares `>=`, so a caller trying
/// to disable the ceiling by passing `0.0` marked *everything* complete and got no resume at all.
pub const RESUME_CEILING: f64 = 0.95;

/// Whether a position counts as having watched the episode.
///
/// Never true at zero: opening an episode to check the subtitles must not mark it seen
/// and push progress to a tracker. Without a known duration we cannot judge, so we say no
/// — under-reporting is recoverable, a wrongly-completed episode is not.
pub fn is_complete(position_secs: f64, duration_secs: Option<f64>, threshold: f64) -> bool {
    match duration_secs {
        Some(d) if d > 0.0 => position_secs / d >= threshold,
        _ => false,
    }
}

impl Store {
    /// Append an event and refresh the projection, in one transaction.
    ///
    /// `episodes_done` counts *distinct* completed episodes from the log rather than
    /// incrementing a counter, so rewatching an episode cannot inflate it.
    pub fn record_event(&self, event: &WatchEvent) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "INSERT INTO watch_event
                   (anilist_id, episode, position_secs, duration_secs, watched_secs,
                    provider_id, translation, completed, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    event.anilist_id.get(),
                    &event.episode,
                    event.position_secs,
                    event.duration_secs,
                    event.watched_secs,
                    event.provider_id.as_deref(),
                    event.translation.map(|t| t.as_str()),
                    event.completed as i32,
                    event.at,
                ],
            )?;

            let episodes_done: u32 = tx.query_row(
                "SELECT COUNT(DISTINCT episode) FROM watch_event
                  WHERE anilist_id = ?1 AND completed = 1",
                [event.anilist_id.get()],
                |r| r.get(0),
            )?;

            // The resume columns move only for an event at least as new as the one they
            // already describe. They used to move for *any* event, so a write carrying an
            // older `at` — an out-of-order arrival, a backwards clock step, a synthetic
            // event stamped in the past — dragged the projection back to a position the log
            // had already superseded, and reordered the CONTINUE rail with it.
            //
            // `episodes_done` is unconditional on purpose: it is recounted from the log
            // above rather than taken from the event, so it is already correct whatever
            // order events arrive in.
            tx.execute(
                "INSERT INTO watch_progress
                   (anilist_id, last_episode, last_position, last_duration,
                    episodes_done, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(anilist_id) DO UPDATE SET
                    last_episode  = CASE WHEN excluded.updated_at >= watch_progress.updated_at
                                         THEN excluded.last_episode
                                         ELSE watch_progress.last_episode END,
                    last_position = CASE WHEN excluded.updated_at >= watch_progress.updated_at
                                         THEN excluded.last_position
                                         ELSE watch_progress.last_position END,
                    last_duration = CASE WHEN excluded.updated_at >= watch_progress.updated_at
                                         THEN excluded.last_duration
                                         ELSE watch_progress.last_duration END,
                    updated_at    = MAX(excluded.updated_at, watch_progress.updated_at),
                    episodes_done = excluded.episodes_done,
                    hidden        = 0",
                rusqlite::params![
                    event.anilist_id.get(),
                    &event.episode,
                    event.position_secs,
                    event.duration_secs,
                    episodes_done,
                    event.at,
                ],
            )?;
            Ok(())
        })
    }

    /// Adopt a remote progress figure: episodes `1..=upto` count as watched.
    ///
    /// **The resume point is left alone.** A tracker reports a count, not a position, so
    /// adopting one says nothing about where the viewer stopped — and writing ordinary events
    /// to record it would drag `last_episode`/`last_position` onto a synthetic episode at 0.0
    /// and lose the real one. Only the count moves.
    ///
    /// Episodes already recorded as completed are skipped, so this is idempotent and cannot
    /// inflate the count. Returns how many episodes were newly marked.
    pub fn adopt_progress(&self, anilist_id: AnilistId, upto: u32, at: i64) -> Result<u32> {
        self.with_tx(|tx| {
            // One statement over a generated series rather than a query per episode: a
            // catch-up can be hundreds of episodes, and the per-episode form spent a
            // round trip on each one.
            let adopted = tx.execute(
                "INSERT INTO watch_event
                   (anilist_id, episode, position_secs, duration_secs, watched_secs,
                    provider_id, translation, completed, at)
                 WITH RECURSIVE series(n) AS (
                     SELECT 1 UNION ALL SELECT n + 1 FROM series WHERE n < ?2
                 )
                 SELECT ?1, CAST(n AS TEXT), 0.0, NULL, 0.0, 'tracker', NULL, 1, ?3
                   FROM series
                  WHERE NOT EXISTS (
                     SELECT 1 FROM watch_event
                      WHERE anilist_id = ?1 AND episode = CAST(n AS TEXT) AND completed = 1
                  )",
                rusqlite::params![anilist_id.get(), upto, at],
            )? as u32;

            let episodes_done: u32 = tx.query_row(
                "SELECT COUNT(DISTINCT episode) FROM watch_event
                  WHERE anilist_id = ?1 AND completed = 1",
                [anilist_id.get()],
                |r| r.get(0),
            )?;

            // Update in place where a row exists — that row's resume columns are the thing
            // being protected. Where none does, there is no resume point to keep, so the
            // required NOT NULL columns are filled from what was adopted.
            let updated = tx.execute(
                "UPDATE watch_progress SET episodes_done = ?2, updated_at = MAX(updated_at, ?3)
                  WHERE anilist_id = ?1",
                rusqlite::params![anilist_id.get(), episodes_done, at],
            )?;
            if updated == 0 && episodes_done > 0 {
                tx.execute(
                    "INSERT INTO watch_progress
                       (anilist_id, last_episode, last_position, last_duration,
                        episodes_done, updated_at)
                     VALUES (?1, ?2, 0.0, NULL, ?3, ?4)",
                    rusqlite::params![anilist_id.get(), upto.to_string(), episodes_done, at],
                )?;
            }
            Ok(adopted)
        })
    }

    /// Erase one episode's events and refresh the projection — the unmark half of a
    /// manual toggle. Local history is the source of truth, so forgetting is allowed;
    /// trackers are monotonic and simply keep their high-water mark.
    pub fn forget_episode(&self, anilist_id: AnilistId, episode: &str) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute(
                "DELETE FROM watch_event WHERE anilist_id = ?1 AND episode = ?2",
                rusqlite::params![anilist_id.get(), episode],
            )?;
            let episodes_done: u32 = tx.query_row(
                "SELECT COUNT(DISTINCT episode) FROM watch_event
                  WHERE anilist_id = ?1 AND completed = 1",
                [anilist_id.get()],
                |r| r.get(0),
            )?;
            // The projection has to stop describing history that no longer exists. It named
            // the erased episode and its position, so the CONTINUE rail went on offering
            // "episode 6, 42%" for a title with nothing behind it and no resume point to
            // return to. Rebuilt from the newest surviving event, or dropped when the last
            // one is gone.
            let newest: Option<(String, f64, Option<f64>, i64)> = tx
                .query_row(
                    "SELECT episode, position_secs, duration_secs, at FROM watch_event
                      WHERE anilist_id = ?1 ORDER BY at DESC, id DESC LIMIT 1",
                    [anilist_id.get()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;

            match newest {
                Some((last_episode, last_position, last_duration, at)) => {
                    tx.execute(
                        "UPDATE watch_progress
                            SET last_episode = ?2, last_position = ?3, last_duration = ?4,
                                episodes_done = ?5, updated_at = ?6
                          WHERE anilist_id = ?1",
                        rusqlite::params![
                            anilist_id.get(),
                            last_episode,
                            last_position,
                            last_duration,
                            episodes_done,
                            at,
                        ],
                    )?;
                }
                None => {
                    tx.execute(
                        "DELETE FROM watch_progress WHERE anilist_id = ?1",
                        [anilist_id.get()],
                    )?;
                }
            }
            Ok(())
        })
    }

    /// Current progress for one title.
    pub fn progress(&self, anilist_id: AnilistId) -> Result<Option<Progress>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT anilist_id, last_episode, last_position, last_duration,
                        episodes_done, updated_at
                   FROM watch_progress WHERE anilist_id = ?1",
            )?;
            let mut rows = stmt.query_map([anilist_id.get()], row_to_progress)?;
            Ok(rows.next().transpose()?)
        })
    }

    /// Where to resume one specific episode, if resuming is worth offering.
    ///
    /// Per-episode rather than off `watch_progress`, which only remembers the last episode
    /// touched — going back to finish episode 3 after starting 4 has to land in the right place.
    /// Returns `None` for an episode that was finished or barely started, so the caller never
    /// has to decide what "resumable" means.
    ///
    /// Takes no threshold, deliberately. It used to, and every caller passed
    /// `playback.commit_threshold` — which quietly tied "can I carry on watching this" to "should
    /// this be reported as watched", two questions with different right answers. See
    /// [`RESUME_CEILING`].
    pub fn resume_position(&self, anilist_id: AnilistId, episode: &str) -> Result<Option<f64>> {
        self.with_conn(|c| {
            let row: Option<(f64, Option<f64>, i64)> = c
                .query_row(
                    "SELECT position_secs, duration_secs, completed FROM watch_event
                      WHERE anilist_id = ?1 AND episode = ?2
                      ORDER BY at DESC LIMIT 1",
                    rusqlite::params![anilist_id.get(), episode],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;

            let Some((position, duration, completed)) = row else { return Ok(None) };
            // Resuming a finished episode would drop you at the credits.
            if completed != 0 || is_complete(position, duration, RESUME_CEILING) {
                return Ok(None);
            }
            Ok((position >= MIN_RESUME_SECS).then_some(position))
        })
    }

    /// Most recently watched titles, newest first — the `CONTINUE` rail.
    ///
    /// Filtered to titles there is genuinely something to continue: either an episode is part-way
    /// through, or at least one is finished and the next one is waiting. Without this, opening a
    /// title and closing it three seconds later puts it at the top of the rail forever — measured
    /// on a real database, where a three-second row outranked an episode left at 61%.
    pub fn continue_list(&self, limit: u32) -> Result<Vec<Progress>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT anilist_id, last_episode, last_position, last_duration,
                        episodes_done, updated_at
                   FROM watch_progress
                  WHERE hidden = 0 AND (episodes_done > 0 OR last_position >= ?2)
                  ORDER BY updated_at DESC
                  LIMIT ?1",
            )?;
            let rows =
                stmt.query_map(rusqlite::params![limit, MIN_RESUME_SECS], row_to_progress)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Take a title off the CONTINUE rail without touching its history.
    ///
    /// A dismissal, not a deletion: every watch event and the progress projection survive,
    /// so the meter and resume point are intact wherever else they show. The next recorded
    /// watch clears the flag — a dismissed title earns its way back by being watched.
    pub fn hide_from_continue(&self, anilist_id: AnilistId) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "UPDATE watch_progress SET hidden = 1 WHERE anilist_id = ?1",
                [anilist_id.get()],
            )?;
            Ok(())
        })
    }

    /// Distinct completed episode count. This is the value pushed to trackers.
    pub fn completed_episode_count(&self, anilist_id: AnilistId) -> Result<u32> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT COUNT(DISTINCT episode) FROM watch_event
                  WHERE anilist_id = ?1 AND completed = 1",
                [anilist_id.get()],
                |r| r.get(0),
            )?)
        })
    }

    /// Full event log for a title, newest first. Used by the stats view.
    pub fn events_for(&self, anilist_id: AnilistId, limit: u32) -> Result<Vec<WatchEvent>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT anilist_id, episode, position_secs, duration_secs, watched_secs,
                        provider_id, translation, completed, at
                   FROM watch_event
                  WHERE anilist_id = ?1
                  ORDER BY at DESC
                  LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![anilist_id.get(), limit], |r| {
                Ok(WatchEvent {
                    anilist_id: AnilistId::new(r.get::<_, u32>(0)?),
                    episode: r.get(1)?,
                    position_secs: r.get(2)?,
                    duration_secs: r.get(3)?,
                    watched_secs: r.get(4)?,
                    provider_id: r.get(5)?,
                    translation: r.get::<_, Option<String>>(6)?.and_then(|s| s.parse().ok()),
                    completed: r.get::<_, i32>(7)? != 0,
                    at: r.get(8)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }
}

fn row_to_progress(r: &rusqlite::Row<'_>) -> rusqlite::Result<Progress> {
    Ok(Progress {
        anilist_id: AnilistId::new(r.get::<_, u32>(0)?),
        last_episode: r.get(1)?,
        last_position: r.get(2)?,
        last_duration: r.get(3)?,
        episodes_done: r.get(4)?,
        updated_at: r.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRIEREN: AnilistId = AnilistId::new(154_587);
    const THRESHOLD: f64 = 0.85;

    fn event(ep: &str, position: f64, duration: f64, at: i64) -> WatchEvent {
        WatchEvent {
            completed: is_complete(position, Some(duration), THRESHOLD),
            duration_secs: Some(duration),
            at,
            ..WatchEvent::new(FRIEREN, ep, position)
        }
    }

    #[test]
    fn adopting_remote_progress_leaves_the_resume_point_alone() {
        // A tracker reports a count, not a position — so adopting one must not touch where
        // the viewer stopped. Recording it as ordinary events would drag the projection onto
        // a synthetic episode at 0.0 and lose the real one, which is why this has a method of
        // its own rather than a loop over `record_event`.
        //
        // Episode 13 is mid-watch and beyond what the remote claims, so nothing about it
        // should move.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("13", 720.0, 1440.0, 5_000)).unwrap();

        let adopted = store.adopt_progress(FRIEREN, 12, 6_000).unwrap();

        assert_eq!(adopted, 12, "every episode up to the remote's figure");
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 12);
        assert_eq!(
            store.resume_position(FRIEREN, "13").unwrap(),
            Some(720.0),
            "the twelve-minute mark of episode 13 survived the adoption"
        );
        let progress = store.progress(FRIEREN).unwrap().expect("a row");
        assert_eq!(progress.last_episode, "13", "the rail still points where the viewer is");
        assert_eq!(progress.last_position, 720.0);
    }

    #[test]
    fn adopting_covers_an_episode_that_was_only_part_watched() {
        // The other direction, and it is deliberate: the remote says twelve episodes are
        // watched, so episode 9 is watched — a half-finished local view of it is superseded
        // rather than preserved, and it stops being offered for resume.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("9", 720.0, 1440.0, 5_000)).unwrap();

        store.adopt_progress(FRIEREN, 12, 6_000).unwrap();

        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 12);
        assert_eq!(
            store.resume_position(FRIEREN, "9").unwrap(),
            None,
            "an episode the remote counts as watched is not half-watched any more"
        );
    }

    #[test]
    fn adopting_the_same_progress_twice_changes_nothing() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.adopt_progress(FRIEREN, 5, 1_000).unwrap(), 5);
        assert_eq!(store.adopt_progress(FRIEREN, 5, 2_000).unwrap(), 0, "already adopted");
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 5);
    }

    #[test]
    fn an_older_event_does_not_drag_the_projection_backwards() {
        // Events do not always arrive in order — an out-of-order write, or a clock that
        // stepped back — and the projection used to take whichever landed last. That moved
        // the CONTINUE rail to a position the log had already superseded.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("3", 900.0, 1440.0, 5_000)).unwrap();
        store.record_event(&event("3", 10.0, 1440.0, 1_000)).unwrap();

        let progress = store.progress(FRIEREN).unwrap().expect("a row");
        assert_eq!(progress.last_position, 900.0, "the newer observation still stands");
        assert_eq!(progress.updated_at, 5_000);
    }

    #[test]
    fn forgetting_an_episode_stops_the_rail_describing_it() {
        // The projection named the erased episode and its position, so CONTINUE went on
        // offering a title at "episode 6, 42%" with no history behind it and nothing to
        // resume — the row outlived the only events that justified it.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("5", 300.0, 1440.0, 1_000)).unwrap();
        store.record_event(&event("6", 600.0, 1440.0, 2_000)).unwrap();

        store.forget_episode(FRIEREN, "6").unwrap();

        let progress = store.progress(FRIEREN).unwrap().expect("episode 5 is still watched");
        assert_eq!(progress.last_episode, "5", "the rail describes surviving history");
        assert_eq!(progress.last_position, 300.0);

        // And with the last event gone, the row goes too rather than lingering empty.
        store.forget_episode(FRIEREN, "5").unwrap();
        assert!(store.progress(FRIEREN).unwrap().is_none());
        assert!(store.continue_list(10).unwrap().is_empty());
    }

    #[test]
    fn resume_is_per_episode_not_per_title() {
        // Going back to finish episode 3 after starting 4 has to land in episode 3's place.
        // `watch_progress` only remembers the last episode touched, so reading from there
        // would resume 3 at 4's timestamp.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("3", 400.0, 1440.0, 1_000)).unwrap();
        store.record_event(&event("4", 900.0, 1440.0, 2_000)).unwrap();

        assert_eq!(store.resume_position(FRIEREN, "3").unwrap(), Some(400.0));
        assert_eq!(store.resume_position(FRIEREN, "4").unwrap(), Some(900.0));
    }

    #[test]
    fn resume_uses_the_latest_observation_for_an_episode() {
        // Positions are recorded every ten seconds, so the log holds many rows per episode.
        let store = Store::open_in_memory().unwrap();
        for (i, position) in [100.0, 200.0, 300.0].into_iter().enumerate() {
            store.record_event(&event("5", position, 1440.0, 1_000 + i as i64)).unwrap();
        }
        assert_eq!(store.resume_position(FRIEREN, "5").unwrap(), Some(300.0));
    }

    #[test]
    fn a_finished_episode_is_not_offered_for_resume() {
        // Resuming past the commit threshold would drop the viewer at the credits. The right
        // offer there is the *next* episode.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("6", 1440.0 * 0.9, 1440.0, 1_000)).unwrap();
        assert_eq!(store.resume_position(FRIEREN, "6").unwrap(), None);
    }

    #[test]
    fn the_first_few_seconds_are_not_worth_resuming() {
        // Indistinguishable from starting fresh, and the prompt would be noise.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("7", 12.0, 1440.0, 1_000)).unwrap();
        assert_eq!(store.resume_position(FRIEREN, "7").unwrap(), None);
    }

    #[test]
    fn an_unwatched_episode_has_nothing_to_resume() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.resume_position(FRIEREN, "1").unwrap(), None);
    }

    #[test]
    fn an_episode_with_no_known_duration_still_resumes() {
        // Torrent streams often report no duration until enough has been fetched. Refusing to
        // resume there would silently lose the position on exactly the source that needs it.
        let store = Store::open_in_memory().unwrap();
        store
            .record_event(&WatchEvent { at: 1_000, ..WatchEvent::new(FRIEREN, "8", 600.0) })
            .unwrap();
        assert_eq!(store.resume_position(FRIEREN, "8").unwrap(), Some(600.0));
    }

    #[test]
    fn threshold_does_not_commit_below_the_line_and_does_above_it() {
        // 84% must not commit and 85% must — the exact boundary from the design.
        let runtime = 1440.0;
        assert!(!is_complete(runtime * 0.84, Some(runtime), THRESHOLD));
        assert!(is_complete(runtime * 0.85, Some(runtime), THRESHOLD));
    }

    #[test]
    fn opening_an_episode_never_marks_it_watched() {
        assert!(!is_complete(0.0, Some(1440.0), THRESHOLD));
        assert!(!is_complete(3.0, Some(1440.0), THRESHOLD));
    }

    #[test]
    fn unknown_duration_never_commits() {
        // We cannot judge completion without a runtime, and wrongly marking an episode
        // watched is not recoverable — it pushes to trackers.
        assert!(!is_complete(9_999.0, None, THRESHOLD));
        assert!(!is_complete(100.0, Some(0.0), THRESHOLD));
    }

    #[test]
    fn recording_events_updates_the_projection() {
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("011", 600.0, 1440.0, 1_000)).unwrap();

        let p = store.progress(FRIEREN).unwrap().unwrap();
        assert_eq!(p.last_episode, "011");
        assert_eq!(p.last_position, 600.0);
        assert_eq!(p.episodes_done, 0, "600/1440 is below the threshold");
        assert!((p.fraction() - 600.0 / 1440.0).abs() < 1e-9);
        assert!(p.is_resumable(THRESHOLD));
    }

    #[test]
    fn rewatching_an_episode_does_not_inflate_the_completed_count() {
        // The count is DISTINCT over the log rather than an incrementing counter, so
        // watching episode 1 three times still means one episode done.
        let store = Store::open_in_memory().unwrap();
        for at in [1_000, 2_000, 3_000] {
            store.record_event(&event("001", 1400.0, 1440.0, at)).unwrap();
        }
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 1);

        store.record_event(&event("002", 1400.0, 1440.0, 4_000)).unwrap();
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 2);
        assert_eq!(store.progress(FRIEREN).unwrap().unwrap().episodes_done, 2);
    }

    #[test]
    fn the_log_is_append_only_and_keeps_every_observation() {
        let store = Store::open_in_memory().unwrap();
        for (i, pos) in [100.0, 400.0, 900.0].into_iter().enumerate() {
            store.record_event(&event("011", pos, 1440.0, 1_000 + i as i64)).unwrap();
        }
        let events = store.events_for(FRIEREN, 10).unwrap();
        assert_eq!(events.len(), 3, "history is a log, not a mutable row");
        // Newest first.
        assert_eq!(events[0].position_secs, 900.0);
    }

    #[test]
    fn resume_is_suppressed_at_both_ends() {
        let store = Store::open_in_memory().unwrap();

        // Barely started: offering to resume 12 seconds in is noise.
        store.record_event(&event("001", 12.0, 1440.0, 1_000)).unwrap();
        assert!(!store.progress(FRIEREN).unwrap().unwrap().is_resumable(THRESHOLD));

        // Effectively finished: the useful offer is the next episode, not the last 60s.
        store.record_event(&event("001", 1400.0, 1440.0, 2_000)).unwrap();
        assert!(!store.progress(FRIEREN).unwrap().unwrap().is_resumable(THRESHOLD));
    }

    #[test]
    fn continue_list_is_ordered_by_recency() {
        let store = Store::open_in_memory().unwrap();
        let dandadan = AnilistId::new(185_660);
        store.record_event(&event("011", 600.0, 1440.0, 1_000)).unwrap();
        store
            .record_event(&WatchEvent {
                duration_secs: Some(1440.0),
                at: 5_000,
                ..WatchEvent::new(dandadan, "004", 300.0)
            })
            .unwrap();

        let list = store.continue_list(10).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].anilist_id, dandadan, "most recent first");
        assert_eq!(list[1].anilist_id, FRIEREN);
    }

    #[test]
    fn a_hidden_title_leaves_the_continue_list_but_keeps_its_history() {
        // Dismissal, not deletion: the rail row goes, the watch log and resume point stay.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("011", 600.0, 1440.0, 1_000)).unwrap();
        assert_eq!(store.continue_list(10).unwrap().len(), 1);

        store.hide_from_continue(FRIEREN).unwrap();
        assert!(store.continue_list(10).unwrap().is_empty());
        assert_eq!(store.resume_position(FRIEREN, "011").unwrap(), Some(600.0));
        assert!(store.progress(FRIEREN).unwrap().is_some(), "the projection survives");
    }

    #[test]
    fn watching_a_hidden_title_brings_it_back() {
        // The undo is the watch itself: there is no key to unhide, so a new event has to
        // clear the flag or a dismissed show could never return to the rail.
        let store = Store::open_in_memory().unwrap();
        store.record_event(&event("011", 600.0, 1440.0, 1_000)).unwrap();
        store.hide_from_continue(FRIEREN).unwrap();
        assert!(store.continue_list(10).unwrap().is_empty());

        store.record_event(&event("012", 90.0, 1440.0, 2_000)).unwrap();
        let list = store.continue_list(10).unwrap();
        assert_eq!(list.len(), 1, "a new watch must resurface the title");
    }

    #[test]
    fn provider_and_translation_survive_the_round_trip() {
        // This detail is the reason local history cannot be replaced by a tracker: no
        // tracker records which source served an episode or in which translation.
        let store = Store::open_in_memory().unwrap();
        store
            .record_event(&WatchEvent {
                provider_id: Some("torrent".into()),
                translation: Some(Translation::Dub),
                duration_secs: Some(1440.0),
                ..WatchEvent::new(FRIEREN, "011", 700.0)
            })
            .unwrap();
        let ev = &store.events_for(FRIEREN, 1).unwrap()[0];
        assert_eq!(ev.provider_id.as_deref(), Some("torrent"));
        assert_eq!(ev.translation, Some(Translation::Dub));
    }

    #[test]
    fn unknown_title_has_no_progress() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.progress(AnilistId::new(1)).unwrap().is_none());
        assert_eq!(store.completed_episode_count(AnilistId::new(1)).unwrap(), 0);
    }
}
