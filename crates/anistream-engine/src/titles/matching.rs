//! Which of a source's entries a title is: forgotten and matched again, chosen by the user, found
//! by a search they type, and the source to use for the title from now on.

use anistream_core::ids::{AnilistId, ProviderKey};
use anistream_model::{event::Event, notice::Notice};

use crate::{
    dispatch::Lent,
    jobs::JobGuard,
    now_epoch,
    outbox::Outbox,
    services::Services,
    state::EngineState,
    titles::episodes::{self, EpisodeLoad, load_episodes},
};

/// Forget the source match a title was given, and list its episodes as the ladder matches
/// them now.
pub async fn clear_match(lent: Lent, id: AnilistId) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let said = if let Err(e) = store.clear_title_match(id) {
        vec![Event::Toast(Notice::alert(format!("could not reset the match: {e}")))]
    } else {
        episodes::said(id, load_episodes(&meta, &store, &registry, id).await)
    };
    for event in said {
        let _ = tx.send(event);
    }
}

/// Keep the match the user chose for a title, and list its episodes from it.
pub async fn fix_match(lent: Lent, id: AnilistId, provider_id: String, key: ProviderKey) {
    let Lent { meta, store, registry, tx, .. } = lent;
    // Remembered as an override, which the ladder consults before anything else —
    // so the question is asked once, not on every visit.
    let said = if let Err(e) = store.set_override(id, &provider_id, &key, now_epoch()) {
        vec![Event::Toast(Notice::alert(format!("could not save that match: {e}")))]
    } else {
        match load_episodes(&meta, &store, &registry, id).await {
            // The override *is* the answer, so the ladder cannot come back
            // undecided; if it somehow does, say so rather than looping.
            Ok(EpisodeLoad::Choose { .. }) => vec![Event::Toast(Notice::alert(
                "that match did not stick — the source may have changed",
            ))],
            load => episodes::said(id, load),
        }
    };
    for event in said {
        let _ = tx.send(event);
    }
}

/// Search the sources for a title as the user typed it, for a match the ladder could not make.
pub fn manual_search(
    id: AnilistId,
    query: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, registry, .. } = services;
    let config = engine.config.effective();
    let (registry, meta, tx) = (registry.clone(), meta.clone(), tx.clone());
    let translation = config.playback.translation;
    tokio::spawn(async move {
        let _job = job;
        let now = now_epoch();
        let attempt = registry.search(&query, translation, now).await;
        let provider_id = attempt.provider.clone().unwrap_or_default();
        let update = match attempt.value {
            Some(hits) if !hits.is_empty() => match meta.media(id).await {
                // Ranked against the real title so the percentages mean the same
                // thing they mean when the ladder asks on its own.
                Ok(media) => {
                    let target = media.match_target();
                    let candidates = anistream_meta::title::rank(&target, &hits)
                        .into_iter()
                        .map(|scored| anistream_model::sources::MatchCandidate {
                            title: scored.hit.title.clone(),
                            key: scored.hit.key.clone(),
                            similarity: scored.score,
                            rejected: scored.rejected.map(str::to_owned),
                        })
                        .collect();
                    Event::MatchChoices { id, provider_id, candidates }
                }
                Err(e) => Event::Toast(Notice::alert(e.to_string())),
            },
            _ => Event::Toast(Notice::alert(format!("nothing found for {query:?}"))),
        };
        let _ = tx.send(update);
    });
}

/// Use this source for this title from now on, or go back to choosing one automatically.
pub fn set_provider_preference(
    id: AnilistId,
    provider: Option<String>,
    engine: &mut crate::state::EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { store, .. } = services;
    let now = now_epoch();
    let written = match &provider {
        Some(provider) => store.set_provider_preference(id, provider, now),
        None => store.clear_provider_preference(id),
    };
    let toast = match (written, &provider) {
        (Ok(()), Some(provider)) => {
            Notice::info(format!("{provider} will be used for this title"))
        }
        (Ok(()), None) => Notice::info("back to automatic source selection"),
        // The overlay has already moved its marker, so a failed write has to be said
        // out loud or the UI would be showing a choice that was never saved.
        (Err(e), _) => Notice::alert(format!("could not save that choice: {e}")),
    };
    // The settings first, with the shows that keep a source counted again: Settings says how
    // many keep one of their own under the source order.
    crate::show_settings::recount(engine, services, tx);
    let _ = tx.send(Event::Toast(toast));
}
