//! When each episode airs, as this device last saw AniList's schedule, and when it was to air
//! before it moved: see `0010_airing_times`.

use std::collections::HashMap;

use anistream_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store};

/// How much later than first seen an episode must stand to count as moved: AniList nudges a
/// time by a minute or two as a broadcaster's slot is pinned down, which is no delay.
const MOVED_SECS: i64 = 5 * 60;
/// How long an episode's time is kept after it was last seen: past the calendar's three months.
const KEPT_SECS: i64 = 120 * 86_400;

impl Store {
    /// Note when each of `airings` airs, as `(title, episode, airs_at)` in Unix seconds, and say
    /// for each that the schedule has moved later the time it stood at before: "Delayed (was
    /// 23:00)". Moved again, it is still delayed from that time.
    ///
    /// A nudge of a few minutes is no delay, and becomes the time it stands at. An episode back
    /// at that time, or earlier, is no longer delayed. One seen for the first time cannot be
    /// known to have moved.
    pub fn remember_airings(
        &self,
        airings: &[(AnilistId, u32, i64)],
        now: i64,
    ) -> Result<HashMap<(AnilistId, u32), i64>> {
        self.with_tx(|tx| {
            let mut read = tx.prepare(
                "SELECT airs_at, was FROM airing_time WHERE anilist_id = ?1 AND episode = ?2",
            )?;
            let mut write = tx.prepare(
                "INSERT INTO airing_time (anilist_id, episode, airs_at, was, seen_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(anilist_id, episode) DO UPDATE SET
                    airs_at = excluded.airs_at,
                    was     = excluded.was,
                    seen_at = excluded.seen_at",
            )?;
            let mut moved = HashMap::new();
            for &(id, episode, airs_at) in airings {
                let kept: Option<(i64, Option<i64>)> = read
                    .query_row(rusqlite::params![id.get(), episode], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .optional()?;
                let was = kept.and_then(|(before, was)| {
                    let first = was.unwrap_or(before);
                    (airs_at > first + MOVED_SECS).then_some(first)
                });
                write.execute(rusqlite::params![id.get(), episode, airs_at, was, now])?;
                if let Some(was) = was {
                    moved.insert((id, episode), was);
                }
            }
            tx.execute("DELETE FROM airing_time WHERE seen_at < ?1", [now - KEPT_SECS])?;
            Ok(moved)
        })
    }

    /// When `episode` of `id` airs, or aired, as this device last saw AniList's schedule, in
    /// Unix seconds: `None` for an episode no calendar here has listed.
    pub fn airing_time(&self, id: AnilistId, episode: u32) -> Result<Option<i64>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT airs_at FROM airing_time WHERE anilist_id = ?1 AND episode = ?2",
                rusqlite::params![id.get(), episode],
                |r| r.get(0),
            )
            .optional()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_001);
    const HOUR: i64 = 3_600;
    const SLOT: i64 = 1_792_000_000;

    #[test]
    fn an_episode_seen_says_when_it_airs_and_one_never_seen_says_nothing() {
        let store = Store::open_in_memory().unwrap();
        store.remember_airings(&[(SHOW, 3, SLOT)], 1).unwrap();
        assert_eq!(store.airing_time(SHOW, 3).unwrap(), Some(SLOT));
        assert_eq!(store.airing_time(SHOW, 4).unwrap(), None);
        store.remember_airings(&[(SHOW, 3, SLOT + HOUR)], 2).unwrap();
        assert_eq!(store.airing_time(SHOW, 3).unwrap(), Some(SLOT + HOUR), "as last seen");
    }

    #[test]
    fn an_episode_moved_later_says_when_it_was_to_air_until_it_is_back() {
        let store = Store::open_in_memory().unwrap();
        let seen = |airs_at, now| store.remember_airings(&[(SHOW, 8, airs_at)], now).unwrap();

        assert!(seen(SLOT, 1).is_empty(), "first seen: nothing to have moved from");
        assert!(seen(SLOT, 2).is_empty(), "the same time again");
        let nudged = SLOT + 120;
        assert!(seen(nudged, 3).is_empty(), "a nudge is no delay");
        assert_eq!(seen(SLOT + 45 * 60, 4), HashMap::from([((SHOW, 8), nudged)]));
        // Moved again: still delayed from where it stood before it first moved.
        assert_eq!(seen(SLOT + HOUR * 24, 5), HashMap::from([((SHOW, 8), nudged)]));
        assert!(seen(nudged, 6).is_empty(), "back where it was");
        assert!(seen(SLOT - HOUR, 7).is_empty(), "earlier is no delay");
    }

    #[test]
    fn episodes_are_told_apart_and_old_ones_let_go() {
        let store = Store::open_in_memory().unwrap();
        store.remember_airings(&[(SHOW, 1, SLOT), (SHOW, 2, SLOT + 7 * 86_400)], 0).unwrap();
        let moved = store
            .remember_airings(&[(SHOW, 1, SLOT), (SHOW, 2, SLOT + 8 * 86_400)], 1)
            .unwrap();
        assert_eq!(moved, HashMap::from([((SHOW, 2), SLOT + 7 * 86_400)]));

        // Not seen for longer than the calendar reaches: forgotten, so first seen again.
        let later = 1 + KEPT_SECS + 1;
        store.remember_airings(&[(SHOW, 3, SLOT)], later).unwrap();
        let moved = store.remember_airings(&[(SHOW, 2, SLOT + 9 * 86_400)], later).unwrap();
        assert!(moved.is_empty(), "{moved:?}");
    }
}
