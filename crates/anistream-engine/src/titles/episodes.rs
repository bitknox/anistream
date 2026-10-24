//! A title's episodes: listed from the source playback would use, and marked watched or not.

use anistream_core::{ids::AnilistId, media::Translation};
use anistream_meta::Meta;
use anistream_model::{episodes::EpisodeRow, event::Event, notice::Notice};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{
    data, dispatch::Lent, now_epoch, outbox::Outbox, services::Services, state::EngineState,
};

/// What loading a title's episodes produced.
pub enum EpisodeLoad {
    Rows(Vec<EpisodeRow>),
    /// No source lists the title, so its episodes are from AniList's data alone: see
    /// [`EpisodeRow::metadata_only`]. With why, when a source was asked and could not list it;
    /// with none set up, nothing is said until a play asks for one.
    Unlisted {
        rows: Vec<EpisodeRow>,
        reason: Option<String>,
    },
    /// The ladder found candidates but could not choose. The user decides.
    Choose {
        provider_id: String,
        candidates: Vec<anistream_model::sources::MatchCandidate>,
    },
}

/// Resolve a title to a provider and list its episodes.
///
/// A title no source lists still has the episodes AniList knows of, marked as such, so its
/// page shows them and they can be marked watched: with no source set up, and when the
/// sources there are could not list it.
pub async fn load_episodes(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
) -> std::result::Result<EpisodeLoad, String> {
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    // Numbered by the source a play would use, which may be one still compiling.
    crate::sources::arrived(registry, || ()).await;
    let now = now_epoch();
    let from_metadata = || {
        let rows = data::metadata_rows(&media, store, id, now);
        dated(rows, &media, data::Numbering::Same, store, id, now)
    };
    if registry.is_empty() {
        return Ok(EpisodeLoad::Unlisted { rows: from_metadata(), reason: None });
    }
    let answer = listed(&media, store, registry, id, Translation::Sub, now).await;

    let mut episodes = match answer {
        anistream_providers::Answer::Answered { value, .. } => value,
        // Candidates exist, none confidently enough to pick. That is a question, not a
        // failure: the user can see which of these is their show, and the answer is
        // remembered as an override so it is asked once.
        anistream_providers::Answer::Ambiguous { provider_id, candidates } => {
            return Ok(EpisodeLoad::Choose {
                provider_id,
                candidates: candidates
                    .iter()
                    .map(|candidate| anistream_model::sources::MatchCandidate {
                        title: candidate.hit.title.clone(),
                        key: candidate.hit.key.clone(),
                        similarity: candidate.score,
                        rejected: candidate.rejected.map(str::to_owned),
                    })
                    .collect(),
            });
        }
        anistream_providers::Answer::Unanswered { .. } => {
            return Ok(EpisodeLoad::Unlisted {
                rows: from_metadata(),
                reason: Some(answer.explain()),
            });
        }
    };

    // A torrent source knows an episode exists but not what it is called, and some sources
    // list nothing but the numbers. The metadata we already fetched above knows more, so it
    // fills in what they lack before they reach the table.
    let numbering = data::enrich_episodes(&mut episodes, &media, now);
    let rows = data::episode_rows(&episodes, store, id);
    Ok(EpisodeLoad::Rows(dated(rows, &media, numbering, store, id, now)))
}

/// `rows`, dated by when each aired or airs, so an episode not out yet can be told from one
/// that is, and marked with when one the schedule has moved later was to air before: each by
/// the number AniList gives it, as `numbering` has the rows' numbers stand to AniList's.
fn dated(
    mut rows: Vec<EpisodeRow>,
    media: &anistream_meta::anilist::Media,
    numbering: data::Numbering,
    store: &Store,
    id: AnilistId,
    now: i64,
) -> Vec<EpisodeRow> {
    data::date_episodes(&mut rows, &numbering.rekey(&media.air_times()));
    data::mark_delays(&mut rows, numbering, store, id, now);
    rows
}

/// What a load of title `id`'s episodes says, in order: the table, always, so the screen
/// leaves its loading state whatever happened, then why it is empty or from AniList alone when
/// that needs saying; or the candidates to choose between.
pub(crate) fn said(
    id: AnilistId,
    load: std::result::Result<EpisodeLoad, String>,
) -> Vec<Event> {
    match load {
        Ok(EpisodeLoad::Rows(rows)) => vec![Event::Episodes { id, rows }],
        // Quietly: the table is there, and a play says what is missing when one is asked for.
        Ok(EpisodeLoad::Unlisted { rows, reason }) => {
            let notice = reason.map(|reason| {
                Event::Toast(Notice::info(format!("no source could list this show: {reason}")))
            });
            std::iter::once(Event::Episodes { id, rows }).chain(notice).collect()
        }
        Ok(EpisodeLoad::Choose { provider_id, candidates }) => {
            vec![Event::MatchChoices { id, provider_id, candidates }]
        }
        // The Episodes screen shows its own empty state, so the reason belongs in a toast
        // rather than replacing the whole view.
        Err(reason) => {
            vec![Event::Episodes { id, rows: Vec::new() }, Event::Toast(Notice::alert(reason))]
        }
    }
}

/// A title's episodes in `translation`, from the source playback would use: the one pinned for
/// this title, or else the first in the order that has it, so the table's numbering is the one
/// Enter plays.
async fn listed(
    media: &anistream_meta::anilist::Media,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    translation: Translation,
    now: i64,
) -> anistream_providers::Answer<Vec<anistream_core::media::Episode>> {
    let pinned = store.provider_preference(id).ok().flatten().and_then(|p| registry.pinned(&p));
    let registry = pinned.as_ref().unwrap_or(registry);
    anistream_providers::first_answer(
        store,
        registry,
        id,
        &media.match_target(),
        translation,
        now,
        |source, key| async move { source.episodes(&key, translation, now).await },
    )
    .await
}

/// Title `id`'s episodes in the dub, from the source a play would use, once the sources on
/// their way have arrived: `None` when no source answered. What the new-episode check reads, for
/// a show watched dubbed.
pub(crate) async fn dub_listing(
    media: &anistream_meta::anilist::Media,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    now: i64,
) -> Option<Vec<anistream_core::media::Episode>> {
    match dub_answer(media, store, registry, id, now).await.answer {
        anistream_providers::Answer::Answered { value, .. } => Some(value),
        _ => None,
    }
}

/// What a listing of a title in the dub came to: see [`dub_answer`].
pub(crate) struct DubAnswer {
    pub answer: anistream_providers::Answer<Vec<anistream_core::media::Episode>>,
    /// No source answered, and that is because each matched the title and found nothing of it
    /// in the dub, rather than because one failed or could not match it: it has no dub there.
    pub none: bool,
}

/// What the source a dubbed play would use answers for title `id`'s episodes in the dub, once
/// the sources on their way have arrived: [`dub_listing`], and the title page's dub line.
pub(crate) async fn dub_answer(
    media: &anistream_meta::anilist::Media,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    now: i64,
) -> DubAnswer {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    crate::sources::arrived(registry, || ()).await;
    let pinned = store.provider_preference(id).ok().flatten().and_then(|p| registry.pinned(&p));
    let registry = pinned.as_ref().unwrap_or(registry);
    // How many sources were listed, and whether each found nothing: a source that failed says
    // nothing of whether there is a dub.
    let (asked, nothing) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicBool::new(true)));
    // A source the ladder cannot match confidently in the dub is not asked about: the question
    // of which title is meant belongs to the episode table, which asks it in the sub.
    let answer = anistream_providers::first_answer(
        store,
        registry,
        id,
        &media.match_target(),
        Translation::Dub,
        now,
        |source, key| {
            let (asked, nothing) = (asked.clone(), nothing.clone());
            async move {
                let attempt = source.episodes(&key, Translation::Dub, now).await;
                asked.fetch_add(1, Ordering::SeqCst);
                let not_found = attempt.failures.iter().all(|(_, error)| {
                    matches!(error, anistream_core::error::ProviderError::NotFound)
                });
                if attempt.value.is_some() || !not_found {
                    nothing.store(false, Ordering::SeqCst);
                }
                attempt
            }
        },
    )
    .await;
    let none = match &answer {
        // One reason for each source tried: each was matched and listed, and found nothing.
        anistream_providers::Answer::Unanswered { reasons, matched: true } => {
            !reasons.is_empty()
                && asked.load(Ordering::SeqCst) == reasons.len()
                && nothing.load(Ordering::SeqCst)
        }
        _ => false,
    };
    DubAnswer { answer, none }
}

/// A title's episodes, from the source playback would use, or the candidates to choose
/// between when the ladder could not choose.
pub async fn list_episodes(lent: Lent, id: AnilistId) {
    let Lent { meta, store, registry, tx, .. } = lent;
    for event in said(id, load_episodes(&meta, &store, &registry, id).await) {
        let _ = tx.send(event);
    }
}

/// Mark episodes watched, or forget that they were: in local history, and on the signed-in
/// trackers when marking. Then the record of the titles is moved on by what the store now
/// holds, and the title said whole, as the answer to the mark.
///
/// Said whether the record holds the title or not. A mark sent before any list or page showed
/// it, as a native app can, was written and left unsaid, and the screen caught up only at the
/// next load; such a title's standing is read from the store.
pub fn set_watched(
    id: AnilistId,
    episodes: &[String],
    watched: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { store, sync, .. } = services;
    let now = anistream_store::now();
    for episode in episodes {
        if watched {
            let mut event = anistream_store::WatchEvent::new(id, episode.clone(), 0.0);
            event.completed = true;
            let _ = store.record_event(&event);
        } else {
            let _ = store.forget_episode(id, episode);
        }
    }
    // Project to trackers only when marking: progress there is monotonic, so an
    // unmark keeps the remote high-water mark and only local history forgets.
    if watched {
        let reached = store.tracker_progress(id).unwrap_or(0);
        if reached > 0 {
            let op = anistream_core::traits::TrackOp::SetProgress {
                anilist_id: id,
                episode: reached,
            };
            // Signed-in trackers only, like a finished episode: the default Kitsu
            // entry has no account behind it until someone signs in.
            for tracker in sync.trackers.iter().filter(|t| t.is_authenticated()) {
                let _ = store.enqueue(tracker.id(), &op, now);
            }
        }
    }
    // Read back rather than assumed: a mark the store did not take is not one to show. When
    // the store cannot say, each is taken as asked, which is what the interface showed.
    let held = store.completed_episodes(id).ok();
    let held =
        |episode: &str| held.as_ref().map_or(watched, |held| held.iter().any(|e| e == episode));
    let marked = engine.cache.marked(id, episodes, watched, held);
    let said = marked.and_then(|id| engine.cache.progress(id)).unwrap_or_else(|| {
        // Without its episode count, which only its metadata has: Continue's episode is
        // counted on from where history stands, as it is for a title of unknown length.
        let (progress, resume) = data::standing(store, id, None);
        engine.cache.standing(id, progress, resume)
    });
    let _ = tx.send(said);
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_core::media::Episode;
    use anistream_meta::Media;
    use anistream_testkit::fixtures::{self, AIRING, FINISHED, FIXTURE_NOW};

    use super::*;
    use crate::data::Numbering;

    fn store() -> Store {
        Store::open_in_memory().unwrap()
    }

    /// Title `id`'s table from AniList's data alone, dated as a load dates it.
    fn from_metadata(media: &Media, store: &Store) -> Vec<EpisodeRow> {
        let rows = data::metadata_rows(media, store, media.id, FIXTURE_NOW);
        dated(rows, media, Numbering::Same, store, media.id, FIXTURE_NOW)
    }

    fn numbers(rows: &[EpisodeRow]) -> Vec<&str> {
        rows.iter().map(|row| row.number.as_str()).collect()
    }

    /// A made-up title from AniList's wire shape: `fields` over an id.
    fn media(fields: serde_json::Value) -> Media {
        let mut wire = serde_json::json!({ "id": 1 });
        wire.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
        serde_json::from_value(wire).unwrap()
    }

    /// AniList's streaming listings for episodes `numbers`, each titled and with a still.
    fn listings(numbers: std::ops::RangeInclusive<u32>) -> serde_json::Value {
        numbers
            .map(|n| {
                serde_json::json!({
                    "title": format!("Episode {n} - Chapter {n}"),
                    "thumbnail": format!("https://img.example/ep/1/{n}.jpg"),
                })
            })
            .collect()
    }

    /// A source's bare episodes, numbered `numbers`.
    fn bare(numbers: std::ops::RangeInclusive<u32>) -> Vec<Episode> {
        numbers.map(Episode::new).collect()
    }

    /// A season of twelve, finished, with every episode listed and its weekly broadcasts.
    fn finished_season() -> Media {
        let aired: Vec<_> = (1..=12)
            .map(|n| serde_json::json!({ "episode": n, "airingAt": 1_000_000 + i64::from(n) }))
            .collect();
        media(serde_json::json!({
            "status": "FINISHED", "episodes": 12, "duration": 24,
            "streamingEpisodes": listings(1..=12), "aired": aired,
        }))
    }

    /// A season of twelve on the air: five out, the sixth next.
    fn airing_season() -> Media {
        media(serde_json::json!({
            "status": "RELEASING", "episodes": 12, "duration": 24,
            "streamingEpisodes": listings(1..=5),
            "aired": (1..=5).map(|n| serde_json::json!({ "episode": n, "airingAt": 100 * n }))
                .collect::<Vec<_>>(),
            "nextAiringEpisode": { "episode": 6, "airingAt": 600, "timeUntilAiring": 50 },
        }))
    }

    #[test]
    fn with_no_source_a_finished_show_has_every_episode_its_count_says() {
        let store = store();
        let media = fixtures::media(FINISHED);
        let rows = from_metadata(&media, &store);
        let expected: Vec<String> = (1..=12).map(|n| n.to_string()).collect();
        assert_eq!(numbers(&rows), expected);
        assert!(rows.iter().all(|row| row.metadata_only), "every row is AniList's alone");
        // Named and given stills by the streaming listings, as long as AniList says.
        assert_eq!(rows[2].title.as_deref(), Some("The Ferryman's Ledger"));
        assert_eq!(rows[2].thumbnail.as_deref(), Some("https://img.example/ep/900001/3.jpg"));
        assert_eq!(rows[2].duration_secs, Some(24 * 60));
        // And dated by the broadcasts the title carries, all of them out.
        assert!(rows.iter().all(|row| row.airs_at.is_some_and(|at| at <= FIXTURE_NOW)));
    }

    #[test]
    fn with_no_source_an_airing_show_has_what_aired_and_the_next_to_come() {
        let store = store();
        let rows = from_metadata(&fixtures::media(AIRING), &store);
        assert_eq!(numbers(&rows), ["1", "2", "3", "4", "5", "6"]);
        let (next, out) = rows.split_last().unwrap();
        assert!(out.iter().all(|row| row.airs_at.is_some_and(|at| at <= FIXTURE_NOW)));
        assert!(next.airs_at.is_some_and(|at| at > FIXTURE_NOW), "the next one is not out yet");
        assert!(rows.iter().all(|row| row.metadata_only));
    }

    #[test]
    fn a_show_whose_count_and_schedule_say_nothing_has_no_episodes() {
        let store = store();
        // Still to finish and no schedule: how many are out is not known, so none are said to
        // be. Nor for one that finished without a count.
        for status in ["RELEASING", "FINISHED", "HIATUS"] {
            let unknown = media(serde_json::json!({ "status": status, "duration": 24 }));
            assert!(from_metadata(&unknown, &store).is_empty(), "{status}");
        }
        // The schedule alone is enough, without a count.
        let counted_by_schedule = media(serde_json::json!({
            "status": "RELEASING",
            "nextAiringEpisode": { "episode": 3, "airingAt": 600, "timeUntilAiring": 50 },
        }));
        assert_eq!(numbers(&from_metadata(&counted_by_schedule, &store)), ["1", "2", "3"]);
    }

    #[test]
    fn a_mark_shows_on_a_row_from_anilist_by_its_number() {
        let store = store();
        let mut event = anistream_store::WatchEvent::new(FINISHED, "3", 0.0);
        event.completed = true;
        store.record_event(&event).unwrap();
        let rows = from_metadata(&fixtures::media(FINISHED), &store);
        let done: Vec<_> = rows.iter().filter(|row| row.completed).map(|r| &r.number).collect();
        assert_eq!(done, ["3"]);
    }

    #[test]
    fn a_table_from_anilist_is_said_quietly_and_why_only_when_a_source_was_asked() {
        let id = FINISHED;
        let rows = from_metadata(&fixtures::media(FINISHED), &store());
        let none_set_up =
            said(id, Ok(EpisodeLoad::Unlisted { rows: rows.clone(), reason: None }));
        assert!(matches!(&none_set_up[..], [Event::Episodes { rows, .. }] if rows.len() == 12));
        let reason = Some("fixture found no matches".to_owned());
        let unlisted = said(id, Ok(EpisodeLoad::Unlisted { rows, reason }));
        let [Event::Episodes { .. }, Event::Toast(notice)] = &unlisted[..] else {
            panic!("the table, then why: {unlisted:?}")
        };
        assert_eq!(
            notice.kind,
            anistream_model::notice::NoticeKind::Info,
            "quiet, not an alert"
        );
        assert_eq!(notice.text, "no source could list this show: fixture found no matches");
    }

    #[test]
    fn a_source_numbered_as_anilist_is_named_by_its_own_numbers() {
        let media = finished_season();
        for numbers in [1..=12, 1..=3, 5..=12] {
            let mut episodes = bare(numbers.clone());
            assert_eq!(data::enrich_episodes(&mut episodes, &media, 0), Numbering::Same);
            let first = *numbers.start();
            assert_eq!(episodes[0].title, Some(format!("Chapter {first}")), "{numbers:?}");
        }
    }

    #[test]
    fn a_season_numbered_on_from_the_one_before_lines_up_by_its_count_and_schedule() {
        let store = store();
        // A finished season the source has whole, as 13 to 24.
        let media = finished_season();
        let mut episodes = bare(13..=24);
        let numbering = data::enrich_episodes(&mut episodes, &media, 0);
        assert_eq!(numbering, Numbering::After(12));
        assert_eq!(episodes[0].title.as_deref(), Some("Chapter 1"));
        assert_eq!(episodes[11].thumbnail.as_deref(), Some("https://img.example/ep/1/12.jpg"));
        // Dated by AniList's numbers through the same offset, and kept by the source's.
        let rows = dated(
            data::episode_rows(&episodes, &store, media.id),
            &media,
            numbering,
            &store,
            media.id,
            0,
        );
        assert_eq!(rows[0].number, "13");
        assert_eq!(rows[0].airs_at, Some(1_000_001));
        assert_eq!(rows[11].airs_at, Some(1_000_012));

        // One on the air the source is up to date with, as 13 to 17.
        let mut episodes = bare(13..=17);
        let numbering = data::enrich_episodes(&mut episodes, &airing_season(), 0);
        assert_eq!(numbering, Numbering::After(12));
        assert_eq!(episodes[4].title.as_deref(), Some("Chapter 5"));
    }

    #[test]
    fn a_numbering_that_reads_either_way_or_neither_names_nothing() {
        let store = store();
        // Three to seven of a season five episodes in: its own 3 to 7, ahead of the schedule,
        // or 1 to 5 counted on from two before. Either could be.
        let media = airing_season();
        let mut episodes = bare(3..=7);
        assert_eq!(data::enrich_episodes(&mut episodes, &media, 0), Numbering::Unknown);
        assert!(episodes.iter().all(|e| e.title.is_none() && e.thumbnail.is_none()));
        let rows = dated(
            data::episode_rows(&episodes, &store, media.id),
            &media,
            Numbering::Unknown,
            &store,
            media.id,
            0,
        );
        assert!(rows.iter().all(|row| row.airs_at.is_none()), "nor dated by a guess");
        // Eight past the end of a season of twelve: not its own numbers, and not all of it.
        let mut episodes = bare(13..=20);
        assert_eq!(
            data::enrich_episodes(&mut episodes, &finished_season(), 0),
            Numbering::Unknown
        );
        assert!(episodes.iter().all(|e| e.title.is_none()));
        // The length needs no number, so it is filled in all the same.
        assert!(episodes.iter().all(|e| e.duration == Some(Duration::from_secs(24 * 60))));
    }

    #[test]
    fn what_a_source_says_stands_and_anilist_fills_only_what_it_left_out() {
        let media = finished_season();
        let with = |title: Option<&str>, duration: Option<u64>| Episode {
            title: title.map(str::to_owned),
            duration: duration.map(Duration::from_secs),
            ..Episode::new(3)
        };
        let mut episodes = vec![
            with(Some("Director's Cut"), Some(1_401)),
            with(Some("Episode 3 - Embers in the Snow"), None),
            with(Some("Ep. 3"), Some(0)),
            with(Some("  "), None),
        ];
        data::enrich_episodes(&mut episodes, &media, 0);
        let said: Vec<_> = episodes
            .iter()
            .map(|e| (e.title.as_deref(), e.duration.map(|d| d.as_secs())))
            .collect();
        assert_eq!(
            said,
            [
                (Some("Director's Cut"), Some(1_401)),
                // The numbering is not part of a title, whoever put it there.
                (Some("Embers in the Snow"), Some(1_440)),
                // Numbering alone, or nothing, is no title: AniList's stands in.
                (Some("Chapter 3"), Some(1_440)),
                (Some("Chapter 3"), Some(1_440)),
            ]
        );
    }
}
