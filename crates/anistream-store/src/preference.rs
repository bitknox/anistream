//! Which source to use for one series.
//!
//! Deliberately separate from [`crate::mapping`], because the two answer different questions.
//! A mapping override says *which title a provider means*; a provider preference says *which
//! provider to ask at all*. Conflating them would make "use the torrent for this show" and
//! "this show is that entry in the catalogue" the same setting, and clearing one would clear
//! the other.
//!
//! The preference is per title rather than global for a reason that shows up immediately in
//! practice: a web source often carries a series the indexer has no seeded release for, and a
//! torrent is frequently the only thing carrying an older season. `providers.order` sets the
//! default; this overrides it where the default is wrong.

use anistream_core::ids::AnilistId;

use crate::{Result, Store};

impl Store {
    /// The provider pinned for a title, if any.
    ///
    /// The caller is expected to check that the id still names a registered provider — see
    /// [`Self::set_provider_preference`] for why this does not do that itself.
    pub fn provider_preference(&self, anilist_id: AnilistId) -> Result<Option<String>> {
        self.with_conn(|c| {
            let found = c
                .query_row(
                    "SELECT provider_id FROM provider_preference WHERE anilist_id = ?1",
                    [anilist_id.get()],
                    |row| row.get::<_, String>(0),
                )
                .ok();
            Ok(found)
        })
    }

    /// Pin a provider for a title. Idempotent, and overwrites any prior choice.
    ///
    /// The provider id is not validated against the registry here, and that is deliberate:
    /// plugins load in the background, so a write during startup could otherwise be refused
    /// for a provider that is merely a few hundred milliseconds away from existing. An id
    /// that names nothing is inert at read time rather than harmful.
    pub fn set_provider_preference(
        &self,
        anilist_id: AnilistId,
        provider_id: &str,
        at: i64,
    ) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO provider_preference (anilist_id, provider_id, created_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(anilist_id) DO UPDATE SET
                    provider_id = excluded.provider_id,
                    created_at  = excluded.created_at",
                rusqlite::params![anilist_id.get(), provider_id, at],
            )?;
            Ok(())
        })
    }

    /// Return a title to automatic source selection.
    pub fn clear_provider_preference(&self, anilist_id: AnilistId) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM provider_preference WHERE anilist_id = ?1",
                [anilist_id.get()],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    fn id(v: u32) -> AnilistId {
        AnilistId::new(v)
    }

    #[test]
    fn no_preference_is_the_default() {
        assert_eq!(store().provider_preference(id(21)).unwrap(), None);
    }

    #[test]
    fn a_preference_round_trips() {
        let store = store();
        store.set_provider_preference(id(21), "web", 100).unwrap();
        assert_eq!(store.provider_preference(id(21)).unwrap(), Some("web".into()));
    }

    #[test]
    fn setting_twice_replaces_rather_than_conflicts() {
        // The primary key is the title alone, so a second choice must overwrite the first
        // instead of failing the insert.
        let store = store();
        store.set_provider_preference(id(21), "web", 100).unwrap();
        store.set_provider_preference(id(21), "torrent", 200).unwrap();
        assert_eq!(store.provider_preference(id(21)).unwrap(), Some("torrent".into()));
    }

    #[test]
    fn a_preference_is_scoped_to_one_title() {
        let store = store();
        store.set_provider_preference(id(21), "web", 100).unwrap();
        assert_eq!(store.provider_preference(id(9999)).unwrap(), None);
    }

    #[test]
    fn clearing_returns_a_title_to_automatic() {
        let store = store();
        store.set_provider_preference(id(21), "web", 100).unwrap();
        store.clear_provider_preference(id(21)).unwrap();
        assert_eq!(store.provider_preference(id(21)).unwrap(), None);
    }

    #[test]
    fn clearing_a_title_with_no_preference_is_not_an_error() {
        // The "reset to automatic" gesture must be safe to press twice.
        assert!(store().clear_provider_preference(id(21)).is_ok());
    }

    #[test]
    fn a_preference_does_not_disturb_the_title_mapping() {
        // The two overrides are independent: clearing the match must not un-pin the source,
        // which is the whole reason this lives in its own table.
        use anistream_core::ids::ProviderKey;
        let store = store();
        store.set_provider_preference(id(21), "web", 100).unwrap();
        store.set_override(id(21), "web", &ProviderKey::new("one-piece"), 100).unwrap();

        store.clear_title_match(id(21)).unwrap();

        assert_eq!(
            store.provider_preference(id(21)).unwrap(),
            Some("web".into()),
            "clearing the match must leave the pinned source alone"
        );
    }
}
