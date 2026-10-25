//! Seeding a store: history, progress, mappings and downloads.
//!
//! Everything goes through the store's own API rather than SQL, so a seeded row is exactly
//! what the app would have written, and a schema change cannot leave the kit writing rows the
//! app no longer reads. Each helper panics on a store error: a test that cannot seed has
//! nothing to test.
//!
//! Times are passed in where the store takes them. Downloads are the exception: the store
//! stamps those with the real clock itself, so a golden must not render a download's age.

use anistream_core::{
    ids::{AnilistId, ProviderKey},
    media::Translation,
};
use anistream_store::{Download, DownloadState, MappingInput, Store, WatchEvent};

use crate::{fixtures, sources};

/// The size every seeded download claims: 350 MiB.
pub const DOWNLOAD_BYTES: u64 = 350 * 1024 * 1024;

/// Episodes `1..=upto` of a title watched to the end, an hour apart, the last at `at`.
///
/// Each is a completed event at full runtime, so the projection counts `upto` episodes done
/// and offers no resume: the Continue rail's next step is episode `upto + 1`.
pub fn watched(store: &Store, id: AnilistId, upto: u32, at: i64) {
    let runtime = fixtures::runtime_secs(id);
    for episode in 1..=upto {
        let event = WatchEvent {
            duration_secs: Some(runtime),
            watched_secs: runtime,
            provider_id: Some(sources::PROVIDER_ID.to_owned()),
            translation: Some(Translation::Sub),
            completed: true,
            at: at - i64::from(upto - episode) * 3_600,
            ..WatchEvent::new(id, episode.to_string(), runtime)
        };
        store.record_event(&event).expect("seeding a watched episode");
    }
}

/// One episode stopped `position_secs` in: an unfinished event, the resume point the Continue
/// rail and the resume prompt offer.
pub fn stopped_at(store: &Store, id: AnilistId, episode: &str, position_secs: f64, at: i64) {
    stopped_in(store, id, episode, position_secs, Translation::Sub, at);
}

/// [`stopped_at`], played in `translation`: what the title's Continue row then marks, a dub
/// being "Dub".
pub fn stopped_in(
    store: &Store,
    id: AnilistId,
    episode: &str,
    position_secs: f64,
    translation: Translation,
    at: i64,
) {
    let event = WatchEvent {
        duration_secs: Some(fixtures::runtime_secs(id)),
        watched_secs: position_secs,
        provider_id: Some(sources::PROVIDER_ID.to_owned()),
        translation: Some(translation),
        completed: false,
        at,
        ..WatchEvent::new(id, episode, position_secs)
    };
    store.record_event(&event).expect("seeding a stopped episode");
}

/// The mapping dataset row for one title: its AniList id beside its MAL id.
pub fn mapping(store: &Store, id: AnilistId, mal_id: u32) {
    let row = MappingInput { anilist_id: id.get(), mal_id: Some(mal_id), ..Default::default() };
    store
        .materialise_mapping("fixture", 0, &[row], None, fixtures::FIXTURE_NOW)
        .expect("seeding a mapping");
}

/// The mapping dataset for every fixture title that has a MAL id: what a refresh of the real
/// datasets would have left, so aniskip and the fallback can place the fixtures.
pub fn mappings(store: &Store) {
    let rows: Vec<MappingInput> = fixtures::ALL
        .iter()
        .filter_map(|id| {
            let mal_id = fixtures::media(*id).id_mal?;
            Some(MappingInput {
                anilist_id: id.get(),
                mal_id: Some(mal_id),
                ..Default::default()
            })
        })
        .collect();
    store
        .materialise_mapping("fixture", 0, &rows, None, fixtures::FIXTURE_NOW)
        .expect("seeding the mappings");
}

/// A title pinned to a provider's key by hand: the override the Fix Match overlay writes.
pub fn provider_match(
    store: &Store,
    id: AnilistId,
    provider_id: &str,
    key: &ProviderKey,
    at: i64,
) {
    store.set_override(id, provider_id, key, at).expect("seeding a provider match");
}

/// One download row in `state`, with the progress that state implies.
///
/// Queued has fetched nothing; active and paused are 40% in; done is whole, at
/// `downloads/<id>/<episode>.mkv`; failed stopped 10% in with a reason. The title is the
/// fixture's, so a row reads as it would in the app.
///
/// Each row is dated a second before [`fixtures::FIXTURE_NOW`] per row the store has made,
/// so seeded rows list in the order they were seeded, every run. Dated by the real clock they
/// listed that way only while seeding stayed within one second, and swapped when it crossed
/// one. The dates are before the real clock's, so a row the app queues during a test lists
/// above every seeded one.
pub fn download(store: &Store, id: AnilistId, episode: &str, state: DownloadState) -> Download {
    let title =
        fixtures::wire(id).map_or_else(|| "Untitled".to_owned(), |_| fixtures::title(id));
    let row = store
        .enqueue_download(id, episode, &title, &magnet(id, episode))
        .expect("seeding a download");
    store
        .date_download_request(row.id, fixtures::FIXTURE_NOW - row.id)
        .expect("dating a seeded download");
    let at = |percent: u64| DOWNLOAD_BYTES * percent / 100;
    let seeded = match state {
        DownloadState::Queued => Ok(()),
        DownloadState::Active => {
            store.update_download_progress(row.id, at(40), DOWNLOAD_BYTES, None)
        }
        DownloadState::Paused => store
            .update_download_progress(row.id, at(40), DOWNLOAD_BYTES, None)
            .and_then(|()| store.set_download_state(row.id, DownloadState::Paused)),
        DownloadState::Done => {
            let path = format!("downloads/{}/{episode:0>2}.mkv", id.get());
            store
                .update_download_progress(row.id, DOWNLOAD_BYTES, DOWNLOAD_BYTES, None)
                .and_then(|()| store.finish_download(row.id, Some(&path)))
        }
        DownloadState::Failed => store
            .update_download_progress(row.id, at(10), DOWNLOAD_BYTES, None)
            .and_then(|()| store.fail_download(row.id, "no peers answered")),
    };
    seeded.expect("seeding a download's progress");
    store
        .download_for(id, episode)
        .expect("reading a seeded download")
        .expect("a seeded download is there")
}

/// A magnet link that is well-formed and points nowhere: a made-up info hash, stable per
/// episode so two seeded rows never share one.
fn magnet(id: AnilistId, episode: &str) -> String {
    let fold = episode
        .bytes()
        .fold(0_u128, |acc, b| acc.wrapping_mul(131).wrapping_add(u128::from(b)));
    format!("magnet:?xt=urn:btih:{:08x}{fold:032x}&dn=fixture-{}-{episode}", id.get(), id.get())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_store::ResolutionRung;

    use crate::fixtures::{AIRING, FINISHED, FIXTURE_NOW, MOVIE, SEQUEL};

    #[test]
    fn watched_episodes_count_as_done_and_offer_no_resume() {
        let store = crate::store();
        watched(&store, FINISHED, 3, FIXTURE_NOW);
        let progress = store.progress(FINISHED).unwrap().expect("a projection row");
        assert_eq!(progress.episodes_done, 3);
        assert_eq!(progress.last_episode, "3");
        assert_eq!(progress.updated_at, FIXTURE_NOW);
        assert!(!progress.is_resumable(anistream_store::RESUME_CEILING));
        let mut done = store.completed_episodes(FINISHED).unwrap();
        done.sort();
        assert_eq!(done, ["1", "2", "3"]);
    }

    #[test]
    fn a_stopped_episode_is_offered_for_resume() {
        let store = crate::store();
        stopped_at(&store, AIRING, "2", 600.0, FIXTURE_NOW);
        let progress = store.progress(AIRING).unwrap().expect("a projection row");
        assert_eq!(progress.episodes_done, 0);
        assert!(progress.is_resumable(anistream_store::RESUME_CEILING));
        assert_eq!(store.resume_position(AIRING, "2").unwrap(), Some(600.0));
        assert_eq!(store.continue_list(10).unwrap()[0].anilist_id, AIRING);
    }

    #[test]
    fn mappings_place_the_fixtures_by_mal_id() {
        let store = crate::store();
        mappings(&store);
        let mal = fixtures::media(FINISHED).id_mal.unwrap();
        assert_eq!(store.mapping_for(FINISHED).unwrap().and_then(|m| m.mal_id), Some(mal));
        assert_eq!(store.anilist_id_for_mal(mal).unwrap(), Some(FINISHED));
        assert!(
            store.mapping_for(fixtures::AIRING_NO_MAL).unwrap().is_none(),
            "a title with no MAL id gets no row"
        );

        mapping(&store, MOVIE, 4_242);
        assert_eq!(store.anilist_id_for_mal(4_242).unwrap(), Some(MOVIE));
    }

    #[test]
    fn a_provider_match_is_an_override() {
        let store = crate::store();
        let key = sources::key(SEQUEL);
        provider_match(&store, FINISHED, sources::PROVIDER_ID, &key, FIXTURE_NOW);
        let found = store.lookup_mapping(FINISHED, sources::PROVIDER_ID).unwrap().unwrap();
        assert_eq!((found.provider_key, found.rung), (key, ResolutionRung::Override));
    }

    #[test]
    fn downloads_are_seeded_in_every_state() {
        let store = crate::store();
        for (episode, state) in [
            ("1", DownloadState::Queued),
            ("2", DownloadState::Active),
            ("3", DownloadState::Paused),
            ("4", DownloadState::Done),
            ("5", DownloadState::Failed),
        ] {
            let row = download(&store, FINISHED, episode, state);
            assert_eq!(row.state, state, "episode {episode}");
            assert_eq!(row.title, "Stardust Wayfarer");
        }
        let rows = store.downloads().unwrap();
        assert_eq!(rows.len(), 5);
        let magnets: std::collections::HashSet<_> = rows.iter().map(|r| &r.magnet).collect();
        assert_eq!(magnets.len(), 5, "every row has its own magnet");

        let done = store.download_for(FINISHED, "4").unwrap().unwrap();
        assert_eq!(done.path.as_deref(), Some("downloads/900001/04.mkv"));
        assert_eq!(done.fraction(), 1.0);
        let failed = store.download_for(FINISHED, "5").unwrap().unwrap();
        assert_eq!(failed.error.as_deref(), Some("no peers answered"));
        let active = store.download_for(FINISHED, "2").unwrap().unwrap();
        assert!((active.fraction() - 0.4).abs() < 1e-9);
    }
}
