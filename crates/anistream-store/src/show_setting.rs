//! What a show sets for itself over the Settings screen's own.
//!
//! Kept as the model's names and text for each setting (`anistream_model::show`), which this
//! does not read: a setting the store held to a type of its own would need a migration each
//! time the model grew one. One row per setting a show sets; one it does not set has no row and
//! follows Settings.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

impl Store {
    /// What show `anilist_id` sets for itself: each setting's name and its value as text, by
    /// name.
    pub fn show_settings(&self, anilist_id: AnilistId) -> Result<Vec<(String, String)>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT setting, value FROM show_setting WHERE anilist_id = ?1 ORDER BY setting",
            )?;
            let rows = statement
                .query_map([anilist_id.get()], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<_>>()?;
            Ok(rows)
        })
    }

    /// Have show `anilist_id` set `setting` to `value`, in place of what it set before, or with
    /// `None` follow Settings again.
    pub fn set_show_setting(
        &self,
        anilist_id: AnilistId,
        setting: &str,
        value: Option<&str>,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            match value {
                Some(value) => c.execute(
                    "INSERT INTO show_setting (anilist_id, setting, value, updated_at)
                     VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(anilist_id, setting) DO UPDATE SET
                        value      = excluded.value,
                        updated_at = excluded.updated_at",
                    rusqlite::params![anilist_id.get(), setting, value, at],
                )?,
                None => c.execute(
                    "DELETE FROM show_setting WHERE anilist_id = ?1 AND setting = ?2",
                    rusqlite::params![anilist_id.get(), setting],
                )?,
            };
            Ok(())
        })
    }

    /// How many shows set each setting for themselves, by name: what "Overridden for 3 shows"
    /// counts. The source a show keeps is counted with them, under `source`.
    pub fn show_setting_counts(&self) -> Result<Vec<(String, u32)>> {
        self.with_conn(|c| {
            let mut statement = c.prepare(
                "SELECT setting, COUNT(*) FROM show_setting GROUP BY setting
                 UNION ALL
                 SELECT 'source', COUNT(*) FROM provider_preference
                 ORDER BY 1",
            )?;
            let rows = statement
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<(String, u32)>>>()?;
            Ok(rows.into_iter().filter(|(_, shows)| *shows > 0).collect())
        })
    }

    /// Put show `anilist_id` back on Settings: what it sets for itself, the source it keeps and
    /// the tracks remembered for it, all forgotten together or none of them.
    pub fn reset_show(&self, anilist_id: AnilistId) -> Result<()> {
        self.with_tx(|tx| {
            for table in ["show_setting", "provider_preference", "track_memory"] {
                tx.execute(
                    &format!("DELETE FROM {table} WHERE anilist_id = ?1"),
                    [anilist_id.get()],
                )?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::{Translation, track_memory::TrackMemory};

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_001);
    const OTHER: AnilistId = AnilistId::new(900_002);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    #[test]
    fn a_show_sets_what_it_sets_and_follows_settings_for_the_rest() {
        let store = store();
        assert!(store.show_settings(SHOW).unwrap().is_empty());
        store.set_show_setting(SHOW, "translation", Some("dub"), 1).unwrap();
        store.set_show_setting(SHOW, "skip_opening", Some("false"), 1).unwrap();
        // Again, in place of the first: one row per setting.
        store.set_show_setting(SHOW, "translation", Some("sub"), 2).unwrap();
        assert_eq!(
            store.show_settings(SHOW).unwrap(),
            [("skip_opening".into(), "false".into()), ("translation".into(), "sub".into())]
        );
        store.set_show_setting(SHOW, "translation", None, 3).unwrap();
        assert_eq!(
            store.show_settings(SHOW).unwrap(),
            [("skip_opening".into(), "false".into())]
        );
        assert!(store.show_settings(OTHER).unwrap().is_empty(), "one show's alone");
    }

    #[test]
    fn the_counts_are_of_shows_with_the_source_among_them() {
        let store = store();
        assert!(
            store.show_setting_counts().unwrap().is_empty(),
            "nothing set, nothing counted"
        );
        store.set_show_setting(SHOW, "translation", Some("dub"), 1).unwrap();
        store.set_show_setting(OTHER, "translation", Some("dub"), 1).unwrap();
        store.set_show_setting(OTHER, "skip_filler", Some("true"), 1).unwrap();
        store.set_provider_preference(SHOW, "yamabiko", 1).unwrap();
        assert_eq!(
            store.show_setting_counts().unwrap(),
            [("skip_filler".into(), 1), ("source".into(), 1), ("translation".into(), 2)]
        );
    }

    #[test]
    fn a_reset_forgets_the_show_and_only_the_show() {
        let store = store();
        let memory = TrackMemory { sub_delay: Some(0.5), ..TrackMemory::default() };
        for show in [SHOW, OTHER] {
            store.set_show_setting(show, "quality", Some("720"), 1).unwrap();
            store.set_provider_preference(show, "yamabiko", 1).unwrap();
            store.remember_tracks(show, Translation::Sub, &memory).unwrap();
            store.remember_tracks(show, Translation::Dub, &memory).unwrap();
        }
        store.reset_show(SHOW).unwrap();

        assert!(store.show_settings(SHOW).unwrap().is_empty());
        assert_eq!(store.provider_preference(SHOW).unwrap(), None);
        assert_eq!(store.track_memory(SHOW, Translation::Sub).unwrap(), None);
        assert_eq!(store.track_memory(SHOW, Translation::Dub).unwrap(), None);
        assert_eq!(store.show_settings(OTHER).unwrap().len(), 1);
        assert_eq!(store.provider_preference(OTHER).unwrap().as_deref(), Some("yamabiko"));
        assert!(store.track_memory(OTHER, Translation::Dub).unwrap().is_some());
    }
}
