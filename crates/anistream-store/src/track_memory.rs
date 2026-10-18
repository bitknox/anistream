//! The tracks a viewer chose for one series, carried from episode to episode.
//!
//! The choice is the player's to make and to describe (`anistream_player::tracks`); this only
//! keeps it. One row per title *and* translation, because a viewer who watches a series dubbed
//! wants signs over their own language and the same viewer watching it subbed wants full
//! subtitles over Japanese — one slot would have each overwrite the other.

use anistream_core::{AnilistId, Translation, track_memory::TrackMemory};
use rusqlite::OptionalExtension;

use crate::{Result, Store, StoreError, now};

impl Store {
    /// The remembered choices for a title in one translation, if there are any.
    ///
    /// A row that no longer parses reads as no memory rather than an error. It is a
    /// convenience — the player's own heuristic is a perfectly good answer — and refusing to
    /// play an episode over a stale preference would be the wrong way round. The next choice
    /// the viewer makes overwrites it.
    pub fn track_memory(
        &self,
        anilist_id: AnilistId,
        translation: Translation,
    ) -> Result<Option<TrackMemory>> {
        let raw: Option<String> = self.with_conn(|c| {
            Ok(c.query_row(
                "SELECT memory FROM track_memory WHERE anilist_id = ?1 AND translation = ?2",
                rusqlite::params![anilist_id.get(), translation.as_str()],
                |r| r.get(0),
            )
            .optional()?)
        })?;
        Ok(raw.and_then(|json| match serde_json::from_str(&json) {
            Ok(memory) => Some(memory),
            Err(error) => {
                tracing::warn!(%anilist_id, %translation, %error, "unreadable track memory");
                None
            }
        }))
    }

    /// Keep the viewer's choices for a title in one translation, replacing what was there.
    ///
    /// The whole memory is written each time, not merged: the player holds the current state
    /// and hands over all of it, so what is stored is always one coherent set of choices.
    pub fn remember_tracks(
        &self,
        anilist_id: AnilistId,
        translation: Translation,
        memory: &TrackMemory,
    ) -> Result<()> {
        let json = serde_json::to_string(memory)
            .map_err(|source| StoreError::Encode { what: "track memory", source })?;
        self.with_conn(|c| {
            c.execute(
                "INSERT INTO track_memory (anilist_id, translation, memory, updated_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(anilist_id, translation) DO UPDATE SET
                    memory     = excluded.memory,
                    updated_at = excluded.updated_at",
                rusqlite::params![anilist_id.get(), translation.as_str(), json, now()],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::track_memory::{SubtitleChoice, SubtitleMemory, TrackPrint};

    use super::*;

    const TITLE: AnilistId = AnilistId::new(21);

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    fn full_subtitles() -> TrackMemory {
        TrackMemory {
            audio: None,
            subtitles: Some(SubtitleMemory {
                choice: SubtitleChoice::Track(TrackPrint {
                    lang: Some("eng".into()),
                    title: "full subtitles".into(),
                    codec: Some("ass".into()),
                    ..TrackPrint::default()
                }),
                audio_lang: Some("jpn".into()),
            }),
            sub_delay: Some(0.4),
        }
    }

    #[test]
    fn nothing_is_remembered_by_default() {
        assert_eq!(store().track_memory(TITLE, Translation::Sub).unwrap(), None);
    }

    #[test]
    fn a_memory_round_trips() {
        let store = store();
        store.remember_tracks(TITLE, Translation::Sub, &full_subtitles()).unwrap();
        assert_eq!(
            store.track_memory(TITLE, Translation::Sub).unwrap(),
            Some(full_subtitles())
        );
    }

    #[test]
    fn remembering_again_replaces_rather_than_conflicts() {
        // The key is title and translation, so a second write must overwrite the first rather
        // than fail the insert — the player writes on every choice the viewer makes.
        let store = store();
        store.remember_tracks(TITLE, Translation::Sub, &full_subtitles()).unwrap();
        let off = TrackMemory {
            subtitles: Some(SubtitleMemory { choice: SubtitleChoice::Off, audio_lang: None }),
            ..TrackMemory::default()
        };
        store.remember_tracks(TITLE, Translation::Sub, &off).unwrap();
        assert_eq!(store.track_memory(TITLE, Translation::Sub).unwrap(), Some(off));
    }

    #[test]
    fn the_sub_and_the_dub_keep_separate_choices() {
        // Signs over the dub must not overwrite full subtitles over the original audio, or
        // switching translation would always start from the other one's choice.
        let store = store();
        store.remember_tracks(TITLE, Translation::Sub, &full_subtitles()).unwrap();
        let dub = TrackMemory { sub_delay: Some(-1.0), ..TrackMemory::default() };
        store.remember_tracks(TITLE, Translation::Dub, &dub).unwrap();

        assert_eq!(
            store.track_memory(TITLE, Translation::Sub).unwrap(),
            Some(full_subtitles())
        );
        assert_eq!(store.track_memory(TITLE, Translation::Dub).unwrap(), Some(dub));
    }

    #[test]
    fn a_memory_is_scoped_to_one_title() {
        let store = store();
        store.remember_tracks(TITLE, Translation::Sub, &full_subtitles()).unwrap();
        assert_eq!(store.track_memory(AnilistId::new(9999), Translation::Sub).unwrap(), None);
    }

    #[test]
    fn an_unreadable_row_reads_as_no_memory() {
        // A row from some other build that no longer parses must not stop the episode from
        // playing; the heuristic stands in, and the next choice overwrites the row.
        let store = store();
        store
            .with_conn(|c| {
                c.execute(
                    "INSERT INTO track_memory (anilist_id, translation, memory, updated_at)
                     VALUES (21, 'sub', '{\"audio\": 7}', 0)",
                    [],
                )?;
                Ok(())
            })
            .unwrap();
        assert_eq!(store.track_memory(TITLE, Translation::Sub).unwrap(), None);

        store.remember_tracks(TITLE, Translation::Sub, &full_subtitles()).unwrap();
        assert_eq!(
            store.track_memory(TITLE, Translation::Sub).unwrap(),
            Some(full_subtitles())
        );
    }
}
