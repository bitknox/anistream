//! A title's episodes: listed from the source playback would use, and marked watched or not.

use anistream_core::ids::AnilistId;
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
    /// The ladder found candidates but could not choose. The user decides.
    Choose {
        provider_id: String,
        candidates: Vec<anistream_model::sources::MatchCandidate>,
    },
}

/// Resolve a title to a provider and list its episodes.
pub async fn load_episodes(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
) -> std::result::Result<EpisodeLoad, String> {
    if registry.is_empty() {
        return Err("no sources configured".into());
    }
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let target = media.match_target();
    let now = now_epoch();

    // From the source playback would use: the one pinned for this title, or else the first in
    // the order that has it, so the table's numbering is the one Enter plays.
    let pinned = store.provider_preference(id).ok().flatten().and_then(|p| registry.pinned(&p));
    let registry = pinned.as_ref().unwrap_or(registry);
    let answer = anistream_providers::first_answer(
        store,
        registry,
        id,
        &target,
        anistream_core::media::Translation::Sub,
        now,
        |source, key| async move {
            source.episodes(&key, anistream_core::media::Translation::Sub, now).await
        },
    )
    .await;

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
            return Err(format!("could not list episodes: {}", answer.explain()));
        }
    };

    // A torrent source knows an episode exists but not what it is called. The metadata we
    // already fetched above does, so name them before they reach the table.
    data::name_episodes(&mut episodes, &media.episode_titles(), &media.episode_thumbnails());

    // And when each aired or airs, so an episode not out yet can be told from one that is.
    let mut rows = data::episode_rows(&episodes, store, id);
    data::date_episodes(&mut rows, &media.air_times());
    Ok(EpisodeLoad::Rows(rows))
}

/// A title's episodes, from the source playback would use, or the candidates to choose
/// between when the ladder could not choose.
pub async fn list_episodes(lent: Lent, id: AnilistId) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let update = match load_episodes(&meta, &store, &registry, id).await {
        Ok(EpisodeLoad::Rows(rows)) => Event::Episodes { id, rows },
        Ok(EpisodeLoad::Choose { provider_id, candidates }) => {
            Event::MatchChoices { id, provider_id, candidates }
        }
        Err(reason) => {
            // The Episodes screen shows its own empty state, so the reason belongs
            // in a toast rather than replacing the whole view.
            let _ = tx.send(Event::Episodes { id, rows: Vec::new() });
            Event::Toast(Notice::alert(reason))
        }
    };
    let _ = tx.send(update);
}

/// Mark episodes watched, or forget that they were: in local history, and on the signed-in
/// trackers when marking. Then the record of the titles is moved on by what the store now
/// holds, and the title said whole, as the answer to the mark.
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
    if let Some(progress) = marked.and_then(|id| engine.cache.progress(id)) {
        let _ = tx.send(progress);
    }
}
