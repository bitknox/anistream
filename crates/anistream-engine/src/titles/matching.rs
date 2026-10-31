//! Which of a source's entries a title is: forgotten and matched again, chosen by the user, found
//! by a search they type, and the source to use for the title from now on.

use anistream_core::{
    error::ProviderError,
    ids::{AnilistId, ProviderKey},
    media::SearchHit,
};
use anistream_meta::title::Rejection;
use anistream_model::{
    Cause, Msg,
    event::Event,
    notice::Notice,
    search::MediaFormat,
    sources::{MatchCandidate, MatchQuestion},
};
use anistream_providers::ProviderRegistry;

use crate::{
    dispatch::Lent,
    jobs::JobGuard,
    now_epoch,
    outbox::Outbox,
    services::Services,
    state::EngineState,
    titles::episodes::{self, EpisodeLoad},
};

/// Forget the source match a title was given, and list its episodes as the ladder matches
/// them now.
pub async fn clear_match(lent: Lent, id: AnilistId) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let said = if let Err(e) = store.clear_title_match(id) {
        vec![Event::Toast(Notice::alert(Msg::NoticeMatchResetFailed { reason: e.to_string() }))]
    } else {
        let load = episodes::load_episodes_matched(&meta, &store, &registry, id).await;
        let (load, current) = match load {
            Ok((load, current)) => (Ok(load), current),
            Err(reason) => (Err(reason), None),
        };
        episodes::said_matched(id, load, current)
    };
    for event in said {
        let _ = tx.send(event);
    }
}

/// Keep the match the user chose for a title, and list its episodes from it.
///
/// A pick that could not be used says so with [`Event::MatchFixFailed`], ahead of any table it
/// still produced, so a sheet waiting on it can say why rather than wait for good.
pub async fn fix_match(
    lent: Lent,
    id: AnilistId,
    provider_id: String,
    key: ProviderKey,
    title: Option<String>,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    // Remembered as an override, which the ladder consults before anything else —
    // so the question is asked once, not on every visit.
    let saved =
        store.set_override_titled(id, &provider_id, &key, title.as_deref(), now_epoch());
    let said = if let Err(e) = saved {
        let reason = Msg::NoticeMatchSaveFailed { reason: e.to_string() };
        vec![Event::MatchFixFailed { id, reason }]
    } else {
        match episodes::load_episodes_matched(&meta, &store, &registry, id).await {
            // The override *is* the answer, so the ladder cannot come back
            // undecided; if it somehow does, say so rather than looping.
            Ok((EpisodeLoad::Choose { rows, .. }, _)) => vec![
                Event::MatchFixFailed { id, reason: Msg::NoticeMatchDidNotStick },
                Event::Episodes { id, rows, failed: None },
                Event::TitleMatch { id, current: None },
            ],
            // Said once, by the sheet's failure, rather than again in a toast.
            Err(reason) => {
                let failed = Msg::NoticeEpisodesFailed { reason };
                vec![
                    Event::MatchFixFailed { id, reason: failed.clone() },
                    Event::Episodes { id, rows: Vec::new(), failed: Some(failed) },
                ]
            }
            Ok((load, current)) => episodes::said_matched(id, Ok(load), current),
        }
    };
    for event in said {
        let _ = tx.send(event);
    }
}

/// A source's entry as a front end offers it: with the facts it was ranked on, and why it was
/// ruled out in the catalogue's words.
pub(crate) fn candidate(
    hit: &SearchHit,
    similarity: f64,
    rejected: Option<Rejection>,
) -> MatchCandidate {
    MatchCandidate {
        title: hit.title.clone(),
        key: hit.key.clone(),
        similarity,
        rejected: rejected.map(ruled_out),
        year: hit.year,
        episodes: hit.episode_count,
        format: hit.format.and_then(format_of),
    }
}

/// Why the ladder ruled an entry out, as the catalogue says it.
fn ruled_out(rejection: Rejection) -> Msg {
    match rejection {
        Rejection::Format => Msg::MatchRuledOutFormat,
        Rejection::TooManyEpisodes => Msg::MatchRuledOutEpisodes,
        Rejection::Year => Msg::MatchRuledOutYear,
    }
}

/// A source's format as the front ends name formats. One the catalogue has no name for is left
/// out, as the source not saying.
fn format_of(format: anistream_core::media::MediaFormat) -> Option<MediaFormat> {
    use anistream_core::media::MediaFormat as Core;
    Some(match format {
        Core::Tv => MediaFormat::Tv,
        Core::TvShort => MediaFormat::TvShort,
        Core::Movie => MediaFormat::Movie,
        Core::Special => MediaFormat::Special,
        Core::Ova => MediaFormat::Ova,
        Core::Ona => MediaFormat::Ona,
        Core::Music => MediaFormat::Music,
        _ => return None,
    })
}

/// A source's name, by its id.
fn source_name(registry: &ProviderRegistry, provider_id: &str) -> String {
    registry
        .get(provider_id)
        .map_or_else(|| provider_id.to_owned(), |p| p.manifest().display_name.clone())
}

/// Why a search found nothing to offer, when it was not that the sources had no such title.
/// `None` when every source asked answered that it had none: an empty answer, not a failure.
fn search_failure(failures: &[(String, ProviderError)]) -> Option<Msg> {
    if failures.is_empty() {
        return Some(Msg::NoticeReason { reason: Cause::NoSources });
    }
    if failures.iter().all(|(_, e)| matches!(e, ProviderError::NotFound)) {
        return None;
    }
    let timed_out =
        |e: &ProviderError| e.to_string().to_ascii_lowercase().contains("timed out");
    if failures.iter().any(|(_, e)| timed_out(e)) {
        return Some(Msg::MatchSearchTimedOut);
    }
    let said = failures
        .iter()
        .filter(|(_, e)| !matches!(e, ProviderError::NotFound))
        .map(|(_, e)| e.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Some(Msg::verbatim(said))
}

/// Search the sources for a title as the user typed it, for a match the ladder could not make:
/// the source `provider_id` names, or the first in the order that answers.
///
/// Answered with [`Event::MatchChoices`] for the search, with no candidates when the source has
/// nothing by those words, or [`Event::MatchSearchFailed`] when no source answered.
pub fn manual_search(
    id: AnilistId,
    query: String,
    provider_id: Option<String>,
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
        let failed = |reason| Event::MatchSearchFailed { id, query: query.clone(), reason };
        let chain = match provider_id.as_deref() {
            None => Some(registry.clone()),
            Some(provider_id) => registry.pinned(provider_id),
        };
        let Some(chain) = chain else {
            let _ = tx.send(failed(Msg::MatchSearchSourceOff));
            return;
        };
        let attempt = chain.search(&query, translation, now_epoch()).await;
        let answered = |provider_id: String, candidates| Event::MatchChoices {
            id,
            source: source_name(&registry, &provider_id),
            provider_id,
            candidates,
            answers: MatchQuestion::Search { query: query.clone() },
        };
        let update = match attempt.value {
            Some(hits) if !hits.is_empty() => match meta.media(id).await {
                // Ranked against the real title so the percentages mean the same
                // thing they mean when the ladder asks on its own.
                Ok(media) => {
                    let target = media.match_target();
                    let candidates = anistream_meta::title::rank(&target, &hits)
                        .into_iter()
                        .map(|scored| candidate(scored.hit, scored.score, scored.rejected))
                        .collect();
                    answered(attempt.provider.clone().unwrap_or_default(), candidates)
                }
                Err(e) => failed(Msg::verbatim(e.to_string())),
            },
            Some(_) => answered(attempt.provider.clone().unwrap_or_default(), Vec::new()),
            None => match search_failure(&attempt.failures) {
                Some(reason) => failed(reason),
                // The source the search named, or the first that said it had nothing.
                None => {
                    let asked = provider_id
                        .clone()
                        .or_else(|| attempt.failures.first().map(|(id, _)| id.clone()))
                        .unwrap_or_default();
                    answered(asked, Vec::new())
                }
            },
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
            Notice::info(Msg::NoticeSourcePinned { source: provider.clone() })
        }
        (Ok(()), None) => Notice::info(Msg::NoticeSourceAutomatic),
        // The overlay has already moved its marker, so a failed write has to be said
        // out loud or the UI would be showing a choice that was never saved.
        (Err(e), _) => Notice::alert(Msg::NoticeSourceChoiceFailed { reason: e.to_string() }),
    };
    // The settings first, with the shows that keep a source counted again: Settings says how
    // many keep one of their own under the source order.
    crate::show_settings::recount(engine, services, tx);
    let _ = tx.send(Event::Toast(toast));
    // Then the show's sheet, which says the source it keeps: the sheet the change came from
    // shows the pin, or automatic again, as the other rows do theirs.
    crate::show_settings::load_show_settings(id, engine, services, tx);
}
