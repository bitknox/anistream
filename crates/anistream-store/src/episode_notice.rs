//! What the new-episode check has said of each show, and how far it has looked: see
//! `0009_episode_notices`. Beside it, the reads the check makes of the rest of the database to
//! know which shows are followed.

use anistream_core::ids::AnilistId;
use rusqlite::OptionalExtension;

use crate::{Result, Store};

/// Where `app_state` keeps the moment the new-episode check has looked as far as.
const CHECKED_THROUGH: &str = "episode_check_through";

/// What the check has said of one show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EpisodeNotice {
    pub anilist_id: AnilistId,
    /// The newest episode said, or 0 before any.
    pub episode: u32,
    /// When the show was first followed: nothing that aired before is said.
    pub since: i64,
}

impl Store {
    /// Every show the check has followed, with what it has said of each.
    pub fn episode_notices(&self) -> Result<Vec<EpisodeNotice>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id, episode, since FROM episode_notice ORDER BY anilist_id",
            )?;
            let rows = statement.query_map([], |r| {
                Ok(EpisodeNotice {
                    anilist_id: AnilistId::new(r.get(0)?),
                    episode: r.get(1)?,
                    since: r.get(2)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// How far the check has looked: the moment its last look reached, or `None` before the
    /// first.
    pub fn episode_check_through(&self) -> Result<Option<i64>> {
        self.with_conn(|c| {
            let raw: Option<String> = c
                .query_row(
                    "SELECT value FROM app_state WHERE key = ?1",
                    [CHECKED_THROUGH],
                    |r| r.get(0),
                )
                .optional()?;
            Ok(raw.and_then(|value| value.parse().ok()))
        })
    }

    /// Keep what a check found, `notices` for the shows it looked at, and that it looked as far
    /// as `through`, together or not at all: a check cut off between the two would say its
    /// episodes again.
    pub fn keep_episode_check(
        &self,
        notices: &[EpisodeNotice],
        through: i64,
        at: i64,
    ) -> Result<()> {
        self.with_tx(|tx| {
            let mut upsert = tx.prepare(
                "INSERT INTO episode_notice (anilist_id, episode, since, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id) DO UPDATE SET episode = excluded.episode,
                                                      since = excluded.since,
                                                      updated_at = excluded.updated_at",
            )?;
            for notice in notices {
                upsert.execute(rusqlite::params![
                    notice.anilist_id.get(),
                    notice.episode,
                    notice.since,
                    at
                ])?;
            }
            tx.execute(
                "INSERT INTO app_state (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![CHECKED_THROUGH, through.to_string()],
            )?;
            Ok(())
        })
    }

    /// Forget what the check has said of `shows`, which it follows no longer: one followed
    /// again starts over, from when it is.
    pub fn forget_episode_notices(&self, shows: &[AnilistId]) -> Result<()> {
        self.with_tx(|tx| {
            let mut delete = tx.prepare("DELETE FROM episode_notice WHERE anilist_id = ?1")?;
            for show in shows {
                delete.execute([show.get()])?;
            }
            Ok(())
        })
    }

    /// Forget everything the check has said and how far it looked, as notifications are turned
    /// off: the next check after they are turned on again is a first one.
    pub fn forget_episode_check(&self) -> Result<()> {
        self.with_tx(|tx| {
            tx.execute("DELETE FROM episode_notice", [])?;
            tx.execute("DELETE FROM app_state WHERE key = ?1", [CHECKED_THROUGH])?;
            Ok(())
        })
    }

    /// The titles watched here since `since`, newest first, but those taken off Continue.
    pub fn watched_since(&self, since: i64) -> Result<Vec<AnilistId>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id FROM watch_progress
                  WHERE hidden = 0 AND updated_at >= ?1
                  ORDER BY updated_at DESC",
            )?;
            let rows = statement.query_map([since], |r| Ok(AnilistId::new(r.get(0)?)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// Every title on the list, with where it is on it, as the tracker words it.
    pub fn library_statuses(&self) -> Result<Vec<(AnilistId, String)>> {
        self.with_conn(|c| {
            let mut statement =
                c.prepare("SELECT anilist_id, status FROM library_status ORDER BY anilist_id")?;
            let rows =
                statement.query_map([], |r| Ok((AnilistId::new(r.get(0)?), r.get(1)?)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    /// The shows that set `setting` to `value` for themselves.
    pub fn shows_setting(&self, setting: &str, value: &str) -> Result<Vec<AnilistId>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT anilist_id FROM show_setting WHERE setting = ?1 AND value = ?2
                  ORDER BY anilist_id",
            )?;
            let rows =
                statement.query_map([setting, value], |r| Ok(AnilistId::new(r.get(0)?)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WatchEvent;

    const ONE: AnilistId = AnilistId::new(900_002);
    const TWO: AnilistId = AnilistId::new(900_006);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    #[test]
    fn a_check_is_kept_with_how_far_it_looked_and_forgotten_whole() {
        let store = store();
        assert!(store.episode_notices().unwrap().is_empty());
        assert_eq!(store.episode_check_through().unwrap(), None, "no check yet");

        let first = [
            EpisodeNotice { anilist_id: ONE, episode: 0, since: 100 },
            EpisodeNotice { anilist_id: TWO, episode: 8, since: 100 },
        ];
        store.keep_episode_check(&first, 100, 100).unwrap();
        assert_eq!(store.episode_notices().unwrap(), first);
        assert_eq!(store.episode_check_through().unwrap(), Some(100));

        // A later check moves one show on and leaves the other as it was.
        let later = [EpisodeNotice { anilist_id: ONE, episode: 6, since: 100 }];
        store.keep_episode_check(&later, 200, 200).unwrap();
        assert_eq!(store.episode_notices().unwrap(), [later[0], first[1]]);
        assert_eq!(store.episode_check_through().unwrap(), Some(200));

        store.forget_episode_notices(&[TWO]).unwrap();
        assert_eq!(store.episode_notices().unwrap(), later);

        store.forget_episode_check().unwrap();
        assert!(store.episode_notices().unwrap().is_empty());
        assert_eq!(store.episode_check_through().unwrap(), None, "a first check again");
    }

    #[test]
    fn what_is_followed_is_read_from_history_the_list_and_the_shows_settings() {
        let store = store();
        let mut event = WatchEvent::new(ONE, "3", 1_400.0);
        event.duration_secs = Some(1_440.0);
        event.completed = true;
        event.at = 1_000;
        store.record_event(&event).unwrap();
        let mut old = WatchEvent::new(TWO, "1", 1_400.0);
        old.at = 10;
        store.record_event(&old).unwrap();
        assert_eq!(store.watched_since(500).unwrap(), [ONE]);
        store.hide_from_continue(ONE).unwrap();
        assert!(store.watched_since(500).unwrap().is_empty(), "taken off Continue");

        store
            .replace_library_statuses("anilist", &[(TWO, "CURRENT"), (ONE, "PLANNING")], 5)
            .unwrap();
        assert_eq!(
            store.library_statuses().unwrap(),
            [(ONE, "PLANNING".to_owned()), (TWO, "CURRENT".to_owned())]
        );

        store.set_show_setting(ONE, "notify_new_episodes", Some("false"), 1).unwrap();
        store.set_show_setting(TWO, "notify_new_episodes", Some("true"), 1).unwrap();
        assert_eq!(store.shows_setting("notify_new_episodes", "false").unwrap(), [ONE]);
    }
}
