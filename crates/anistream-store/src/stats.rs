//! Watch statistics, and the export that makes them portable.
//!
//! Cheap to provide because the history log already holds everything: this is aggregation over a
//! table that exists, not new bookkeeping.
//!
//! One rule shapes the numbers. **`watched_secs` is per-event and cumulative within a session, so
//! it must never be summed across events for the same episode.** Positions are recorded every ten
//! seconds, each row carrying the session total to that point — adding them would report roughly
//! fifty times the real figure. The queries below take the maximum per episode instead, which is
//! the last observation of that session.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

/// Aggregate figures over the whole history.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    /// Distinct titles with at least one recorded event.
    pub titles: u32,
    /// Distinct episodes that crossed the commit threshold.
    pub episodes_completed: u32,
    /// Distinct episodes touched at all, finished or not.
    pub episodes_started: u32,
    /// Real time spent watching, in seconds.
    pub watched_secs: f64,
    /// Unix seconds of the first and most recent event.
    pub first_at: Option<i64>,
    pub last_at: Option<i64>,
    /// Which provider served the most episodes, and how many.
    pub top_provider: Option<(String, u32)>,
}

impl Stats {
    /// Watch time as a human phrase — `"3d 4h"`, `"5h 12m"`, `"18m"`.
    ///
    /// Two units at most. `"3d 4h 12m 6s"` is a stopwatch reading, not an answer to "how much anime
    /// have I watched".
    pub fn watched_human(&self) -> String {
        let total = self.watched_secs.max(0.0) as u64;
        let (days, hours, minutes) =
            (total / 86_400, (total % 86_400) / 3_600, (total % 3_600) / 60);
        match (days, hours, minutes) {
            (0, 0, m) => format!("{m}m"),
            (0, h, m) => format!("{h}h {m}m"),
            (d, h, _) => format!("{d}d {h}h"),
        }
    }
}

/// One title's row in an export.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExportedTitle {
    pub anilist_id: u32,
    /// Whatever title was last seen for this id, when one is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub episodes_completed: u32,
    pub last_episode: String,
    pub last_position_secs: f64,
    pub watched_secs: f64,
    pub updated_at: i64,
}

/// A whole history export.
///
/// Deliberately a *projection*, not a database dump: the point is something a person can read,
/// diff, and load into another tool. `version` is there so a future format change can be detected
/// rather than misparsed.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Export {
    pub version: u32,
    pub exported_at: i64,
    pub titles: Vec<ExportedTitle>,
}

/// The current export format.
pub const EXPORT_VERSION: u32 = 1;

impl Store {
    /// Aggregate statistics over the whole log.
    pub fn stats(&self) -> Result<Stats> {
        self.with_conn(|c| {
            let mut stats = Stats::default();

            (stats.titles, stats.first_at, stats.last_at) = c.query_row(
                "SELECT COUNT(DISTINCT anilist_id), MIN(at), MAX(at) FROM watch_event",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?;

            stats.episodes_completed = c.query_row(
                "SELECT COUNT(*) FROM (
                     SELECT DISTINCT anilist_id, episode FROM watch_event WHERE completed = 1
                 )",
                [],
                |r| r.get(0),
            )?;

            stats.episodes_started = c.query_row(
                "SELECT COUNT(*) FROM (SELECT DISTINCT anilist_id, episode FROM watch_event)",
                [],
                |r| r.get(0),
            )?;

            // MAX per episode, then summed — never SUM over rows. Each row carries the session
            // total to that point, so summing them would report ~50× the real figure.
            stats.watched_secs = c
                .query_row(
                    "SELECT COALESCE(SUM(best), 0) FROM (
                         SELECT MAX(watched_secs) AS best FROM watch_event
                          GROUP BY anilist_id, episode
                     )",
                    [],
                    |r| r.get::<_, f64>(0),
                )
                .unwrap_or(0.0);

            stats.top_provider = c
                .query_row(
                    "SELECT provider_id, COUNT(*) AS n FROM (
                         SELECT DISTINCT anilist_id, episode, provider_id FROM watch_event
                          WHERE provider_id IS NOT NULL
                     ) GROUP BY provider_id ORDER BY n DESC LIMIT 1",
                    [],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
                )
                .ok();

            Ok(stats)
        })
    }

    /// Every title with recorded history, newest first.
    pub fn export(&self, now: i64) -> Result<Export> {
        let titles = self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT p.anilist_id, t.title, p.episodes_done, p.last_episode,
                        p.last_position, p.updated_at,
                        COALESCE((
                            SELECT SUM(best) FROM (
                                SELECT MAX(watched_secs) AS best FROM watch_event e
                                 WHERE e.anilist_id = p.anilist_id
                                 GROUP BY e.episode
                            )
                        ), 0)
                   FROM watch_progress p
                   LEFT JOIN title_index t ON t.anilist_id = p.anilist_id
                  ORDER BY p.updated_at DESC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(ExportedTitle {
                    anilist_id: r.get(0)?,
                    title: r.get(1)?,
                    episodes_completed: r.get(2)?,
                    last_episode: r.get(3)?,
                    last_position_secs: r.get(4)?,
                    updated_at: r.get(5)?,
                    watched_secs: r.get(6)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })?;

        Ok(Export { version: EXPORT_VERSION, exported_at: now, titles })
    }

    /// Merge an export back in.
    ///
    /// Additive and monotonic: an imported title raises local progress but never lowers it. That
    /// makes importing the same file twice a no-op, and importing an *older* backup harmless —
    /// both of which someone will do, and neither should cost them episodes.
    ///
    /// Returns how many titles were actually advanced.
    pub fn import(&self, export: &Export, now: i64) -> Result<u32> {
        if export.version > EXPORT_VERSION {
            return Err(crate::StoreError::UnsupportedFormat(format!(
                "export is version {} but this build understands {EXPORT_VERSION}",
                export.version
            )));
        }

        let mut advanced = 0;
        for title in &export.titles {
            let id = AnilistId::new(title.anilist_id);
            let existing = self.progress(id)?;
            let local_done = existing.as_ref().map_or(0, |p| p.episodes_done);

            // Nothing to do when local already knows as much. Checked rather than relying on the
            // write being idempotent, so the return count means something.
            if title.episodes_completed <= local_done {
                continue;
            }

            // Written as a synthetic completed event per newly-known episode, so the log stays the
            // source of truth and the projection is derived as usual rather than patched.
            for episode in (local_done + 1)..=title.episodes_completed {
                self.record_event(&crate::WatchEvent {
                    completed: true,
                    duration_secs: None,
                    watched_secs: 0.0,
                    provider_id: Some("import".into()),
                    at: now,
                    ..crate::WatchEvent::new(id, episode.to_string(), 0.0)
                })?;
            }
            if let Some(name) = &title.title {
                let _ = self.remember_title(id, name);
            }
            advanced += 1;
        }
        Ok(advanced)
    }

    /// A random title from local history, for when you cannot decide.
    ///
    /// `SQLite`'s `RANDOM()` rather than shuffling in Rust: the selection happens where the rows
    /// are, and it stays one query as history grows.
    pub fn random_watched(&self) -> Result<Option<AnilistId>> {
        self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT anilist_id FROM watch_progress ORDER BY RANDOM() LIMIT 1",
                [],
                |r| r.get::<_, u32>(0).map(AnilistId::new),
            )
            .ok())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WatchEvent;

    const FRIEREN: AnilistId = AnilistId::new(154_587);
    const DANDADAN: AnilistId = AnilistId::new(171_018);

    /// A session's worth of rows for one episode: positions recorded every ten seconds, each
    /// carrying the session total so far. This shape is why the queries take a maximum.
    fn record_session(store: &Store, id: AnilistId, episode: u32, watched: f64, at: i64) {
        let steps = 5;
        for step in 1..=steps {
            let fraction = f64::from(step) / f64::from(steps);
            store
                .record_event(&WatchEvent {
                    duration_secs: Some(1_440.0),
                    watched_secs: watched * fraction,
                    provider_id: Some("torrent".into()),
                    completed: step == steps,
                    at: at + i64::from(step),
                    ..WatchEvent::new(id, episode.to_string(), 1_400.0 * fraction)
                })
                .unwrap();
        }
    }

    #[test]
    fn watch_time_is_not_multiplied_by_the_number_of_rows() {
        // The defect this module is written around. Each of the five rows per episode carries the
        // session total to that point; summing them would report 3× the truth here, and ~50× in a
        // real session recorded every ten seconds.
        let store = Store::open_in_memory().unwrap();
        record_session(&store, FRIEREN, 1, 1_400.0, 1_000);

        let stats = store.stats().unwrap();
        assert_eq!(stats.watched_secs, 1_400.0, "watch time was summed across rows");
        assert_eq!(stats.episodes_started, 1);
        assert_eq!(stats.episodes_completed, 1);
        assert_eq!(stats.titles, 1);
    }

    #[test]
    fn stats_aggregate_across_titles_and_episodes() {
        let store = Store::open_in_memory().unwrap();
        record_session(&store, FRIEREN, 1, 1_400.0, 1_000);
        record_session(&store, FRIEREN, 2, 1_400.0, 2_000);
        record_session(&store, DANDADAN, 1, 700.0, 3_000);

        let stats = store.stats().unwrap();
        assert_eq!(stats.titles, 2);
        assert_eq!(stats.episodes_completed, 3);
        assert_eq!(stats.watched_secs, 3_500.0);
        assert_eq!(stats.first_at, Some(1_001));
        assert_eq!(stats.last_at, Some(3_005));
        assert_eq!(stats.top_provider, Some(("torrent".to_string(), 3)));
    }

    #[test]
    fn rewatching_does_not_inflate_the_episode_count() {
        // Distinct episodes, not events — the same rule the tracker push depends on.
        let store = Store::open_in_memory().unwrap();
        record_session(&store, FRIEREN, 1, 1_400.0, 1_000);
        record_session(&store, FRIEREN, 1, 1_400.0, 9_000);
        assert_eq!(store.stats().unwrap().episodes_completed, 1);
    }

    #[test]
    fn an_empty_history_produces_zeroes_rather_than_an_error() {
        // The first-run case, and it must not look like a failure.
        let stats = Store::open_in_memory().unwrap().stats().unwrap();
        assert_eq!(stats, Stats::default());
        assert_eq!(stats.watched_human(), "0m");
    }

    #[test]
    fn watch_time_reads_as_a_phrase_with_two_units_at_most() {
        let at = |secs: f64| Stats { watched_secs: secs, ..Stats::default() }.watched_human();
        assert_eq!(at(0.0), "0m");
        assert_eq!(at(59.0), "0m");
        assert_eq!(at(1_080.0), "18m");
        assert_eq!(at(18_720.0), "5h 12m");
        assert_eq!(at(273_600.0), "3d 4h");
        // Never a stopwatch reading.
        assert!(!at(273_666.0).contains('s'));
    }

    #[test]
    fn an_export_round_trips_into_an_empty_store() {
        let source = Store::open_in_memory().unwrap();
        record_session(&source, FRIEREN, 1, 1_400.0, 1_000);
        record_session(&source, FRIEREN, 2, 1_400.0, 2_000);
        source.remember_title(FRIEREN, "Sousou no Frieren").unwrap();

        let export = source.export(5_000).unwrap();
        assert_eq!(export.version, EXPORT_VERSION);
        assert_eq!(export.titles.len(), 1);
        assert_eq!(export.titles[0].episodes_completed, 2);
        assert_eq!(export.titles[0].title.as_deref(), Some("Sousou no Frieren"));

        let target = Store::open_in_memory().unwrap();
        assert_eq!(target.import(&export, 6_000).unwrap(), 1);
        assert_eq!(target.completed_episode_count(FRIEREN).unwrap(), 2);
        assert_eq!(target.cached_title(FRIEREN).unwrap().as_deref(), Some("Sousou no Frieren"));
    }

    #[test]
    fn importing_twice_changes_nothing_the_second_time() {
        // Someone will do this. It must not double anything.
        let store = Store::open_in_memory().unwrap();
        let export = Export {
            version: EXPORT_VERSION,
            exported_at: 0,
            titles: vec![ExportedTitle {
                anilist_id: FRIEREN.get(),
                title: None,
                episodes_completed: 4,
                last_episode: "4".into(),
                last_position_secs: 0.0,
                watched_secs: 0.0,
                updated_at: 0,
            }],
        };
        assert_eq!(store.import(&export, 100).unwrap(), 1);
        assert_eq!(
            store.import(&export, 200).unwrap(),
            0,
            "the second import advanced something"
        );
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 4);
    }

    #[test]
    fn importing_an_older_backup_never_loses_episodes() {
        // Progress is monotonic. Restoring a stale backup over newer local history must not undo
        // what you watched since.
        let store = Store::open_in_memory().unwrap();
        for episode in 1..=8 {
            record_session(&store, FRIEREN, episode, 1_400.0, 1_000 + i64::from(episode) * 10);
        }
        let stale = Export {
            version: EXPORT_VERSION,
            exported_at: 0,
            titles: vec![ExportedTitle {
                anilist_id: FRIEREN.get(),
                title: None,
                episodes_completed: 3,
                last_episode: "3".into(),
                last_position_secs: 0.0,
                watched_secs: 0.0,
                updated_at: 0,
            }],
        };
        assert_eq!(store.import(&stale, 9_000).unwrap(), 0);
        assert_eq!(store.completed_episode_count(FRIEREN).unwrap(), 8, "episodes were lost");
    }

    #[test]
    fn a_newer_export_format_is_refused_rather_than_misread() {
        let store = Store::open_in_memory().unwrap();
        let future = Export { version: EXPORT_VERSION + 1, exported_at: 0, titles: Vec::new() };
        let err = store.import(&future, 0).unwrap_err().to_string();
        assert!(err.contains("version"), "{err}");
    }

    #[test]
    fn an_export_is_json_that_survives_a_round_trip() {
        // The point of an export is that another tool can read it.
        let store = Store::open_in_memory().unwrap();
        record_session(&store, FRIEREN, 1, 1_400.0, 1_000);
        let export = store.export(5_000).unwrap();

        let json = serde_json::to_string_pretty(&export).unwrap();
        assert_eq!(serde_json::from_str::<Export>(&json).unwrap(), export);
        // A title with no known name omits the field rather than emitting null.
        assert!(!json.contains("\"title\": null"), "{json}");
    }

    #[test]
    fn a_random_pick_comes_from_history_or_is_absent() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.random_watched().unwrap(), None, "nothing watched yet");

        record_session(&store, FRIEREN, 1, 100.0, 1_000);
        record_session(&store, DANDADAN, 1, 100.0, 2_000);
        for _ in 0..20 {
            let picked = store.random_watched().unwrap().expect("a pick");
            assert!(picked == FRIEREN || picked == DANDADAN, "picked {picked:?}");
        }
    }
}
