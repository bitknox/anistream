//! Whether the sources have a title, asked when someone shows interest in one on Home.
//!
//! Every usable source is searched for the title at once, within [`PROBE_WAIT`]. A confident
//! match on any of them is a find, kept as that source's mapping so the play after it asks
//! nothing. A miss is only a miss when every source answered, and none had it; a source that
//! failed or ran out of time leaves the answer unknown, which Home never marks. A miss is kept
//! for a day, so the same title is not asked again each time it is looked at.

use std::time::Duration;

use anistream_core::{error::ProviderError, ids::AnilistId, media::Translation};
use anistream_meta::{Meta, title::rank};
use anistream_model::{event::Event, home::Playable};
use anistream_providers::ProviderRegistry;
use anistream_store::{ResolutionRung, Store};

use crate::{
    catalog::home::{Probed, probe_key},
    outbox::Outbox,
};

/// How long the check as a whole may take.
pub const PROBE_WAIT: Duration = Duration::from_secs(4);
const DAY: i64 = 86_400;

/// What the sources say of one title, as far as one source can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Said {
    Has,
    HasNot,
    CouldNotTell,
}

/// Ask the sources whether they have `id`, and send what they said. A find or a miss kept from
/// less than a day ago is sent again without asking.
pub async fn probe(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    tx: &Outbox,
    id: AnilistId,
    translation: Translation,
    now: i64,
) -> Playable {
    let playable = check(meta, store, registry, id, translation, now).await;
    let _ = tx.send(Event::TitlePlayable { id, playable });
    playable
}

async fn check(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    translation: Translation,
    now: i64,
) -> Playable {
    let key = probe_key(id);
    if let Some(kept) = store
        .get_meta_text(&key)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Probed>(&json).ok())
        && now - kept.at < DAY
        && now >= kept.at
    {
        return if kept.found { Playable::Found } else { Playable::NotFound };
    }
    let ids = registry.usable_ids();
    if ids.is_empty() {
        return Playable::Unknown;
    }
    // A mapping already kept for any source is a find without asking.
    if ids.iter().any(|p| store.lookup_mapping(id, p).ok().flatten().is_some()) {
        return Playable::Found;
    }
    let Some(media) = meta.cached(&[id]).into_iter().next() else {
        return Playable::Unknown;
    };
    let target = media.match_target();
    let Some(query) = target.titles.first().cloned() else {
        return Playable::Unknown;
    };
    // Each source has until the same moment: one that hangs leaves the others' answers standing.
    let deadline = tokio::time::Instant::now() + PROBE_WAIT;
    let asked = ids.iter().map(|provider_id| {
        let (target, query) = (&target, &query);
        let one = async move {
            let Some(source) = registry.pinned(provider_id) else {
                return Said::CouldNotTell;
            };
            let attempt = source.search(query, translation, now).await;
            match attempt.value {
                Some(hits) if hits.is_empty() => Said::HasNot,
                Some(hits) => {
                    let ranked = rank(target, &hits);
                    match ranked.first() {
                        Some(best) if best.is_confident() => {
                            let _ = store.cache_resolution(
                                id,
                                provider_id,
                                &best.hit.key,
                                best.score,
                                ResolutionRung::ProviderSearch,
                                now,
                            );
                            Said::Has
                        }
                        // Something like it, but not sure: not a miss to mark.
                        Some(_) => Said::CouldNotTell,
                        None => Said::HasNot,
                    }
                }
                None if !attempt.failures.is_empty()
                    && attempt.failures.iter().all(|(_, e)| matches!(e, ProviderError::NotFound)) =>
                {
                    Said::HasNot
                }
                None => Said::CouldNotTell,
            }
        };
        async move { tokio::time::timeout_at(deadline, one).await.unwrap_or(Said::CouldNotTell) }
    });
    let said = futures::future::join_all(asked).await;
    let playable = if said.contains(&Said::Has) {
        Playable::Found
    } else if said.iter().all(|s| *s == Said::HasNot) {
        Playable::NotFound
    } else {
        Playable::Unknown
    };
    if playable != Playable::Unknown {
        let kept = Probed { at: now, found: playable == Playable::Found };
        if let Ok(json) = serde_json::to_string(&kept)
            && let Err(e) = store.set_meta_text(&key, &json)
        {
            tracing::warn!(error = %e, "could not keep what the sources said");
        }
    }
    playable
}
