//! Finding what an episode plays from: its streams, and the context its playback records
//! history against. And the releases an episode can be played from, for the user to pick.

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::{event::Event, notice::Notice};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{
    jobs::JobGuard, now_epoch, outbox::Outbox, playback, services::Services, state::EngineState,
};

/// An episode's streams and playback context, as resolution produces them.
pub type Resolved = (Vec<anistream_core::stream::Stream>, playback::PlaybackContext);

/// Everything needed before mpv can be spawned: a stream, and the context history needs.
#[allow(clippy::too_many_arguments)]
pub async fn resolve_for_playback(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
    episode: &str,
    pick: Option<(String, String)>,
    playback: &anistream_core::config::PlaybackConfig,
    job: Option<&JobGuard>,
    tx: &Outbox,
) -> std::result::Result<(Vec<anistream_core::stream::Stream>, playback::PlaybackContext), String>
{
    // The step under way, on the job when there is one; the CLI has no activity row and reads
    // the status channel instead.
    let step = |text: String| match job {
        Some(job) => job.note(text, None),
        None => {
            let _ = tx.send(Event::Status(text));
        }
    };
    if registry.is_empty() {
        return Err("no sources configured — see the Providers screen".into());
    }
    // Phase-labelled, because the wait between Enter and mpv is where every tool in
    // this category earns its "is it frozen?" issues. The status line names the rung
    // being climbed; the eyecatch's own timer says how long it has taken.
    step("looking up the title".into());
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let now = now_epoch();

    // "Use this source for this series", applied *before* the title is matched.
    //
    // The order matters: a `ProviderKey` is only meaningful to the provider that issued it, so
    // matching against the whole chain and then resolving against a pinned source would hand one
    // provider's key to another and look like a missing episode.
    //
    // Skipped when the user picked an exact release from the Sources overlay. That pick is the
    // more immediate instruction of the two, it already names its own provider, and honouring the
    // remembered preference instead would refuse a release the user is looking straight at.
    //
    // A preference naming a provider that is not registered — a plugin removed, or one still
    // compiling in the background — falls back to the normal chain. Refusing to play because a
    // remembered choice is momentarily unresolvable would be a worse answer than playing.
    let preference =
        pick.is_none().then(|| store.provider_preference(id).ok().flatten()).flatten();
    let pinned = preference.as_deref().and_then(|id| registry.pinned(id));
    if let Some(name) = &preference
        && pinned.is_none()
    {
        tracing::debug!(provider = %name, "pinned source is not registered, using the chain");
    }
    let registry = pinned.as_ref().unwrap_or(registry);
    let target = media.match_target();

    step(format!("searching sources for ep {episode}"));
    let mut streams = match &pick {
        // A picked release goes straight to its provider, no failover: substituting a
        // different stream for the one the user chose would silently undo the choice. Matched
        // on that provider, whose key it is.
        Some((provider_id, source_id)) => {
            let source = registry
                .pinned(provider_id)
                .ok_or_else(|| format!("{provider_id} is not a source any more"))?;
            let resolution = anistream_providers::resolve(
                store,
                &source,
                id,
                &target,
                playback.translation,
                now,
            )
            .await;
            let key = resolution.key().cloned().ok_or_else(|| {
                format!("could not match this title: {}", resolution.explain())
            })?;
            source
                .resolve_source(
                    provider_id,
                    &key,
                    episode,
                    playback.translation,
                    source_id,
                    now,
                )
                .await
                .map_err(|e| format!("could not start that release: {e}"))?
        }
        None => {
            let answer = anistream_providers::first_answer(
                store,
                registry,
                id,
                &target,
                playback.translation,
                now,
                |source, key| async move {
                    source.resolve(&key, episode, playback.translation, now).await
                },
            )
            .await;
            match answer {
                anistream_providers::Answer::Answered { value, .. } => value,
                anistream_providers::Answer::Ambiguous { .. } => {
                    return Err(format!("could not match this title: {}", answer.explain()));
                }
                anistream_providers::Answer::Unanswered { .. } => return Err(answer.explain()),
            }
        }
    };

    // Toward the configured quality, preferring a step down over upscaling. The whole list is
    // returned rather than the winner: the sort is a preference, and playback falls over to
    // the next entry when a stream turns out not to decode.
    streams.sort_by_key(|s| s.quality_rank(playback.quality));
    if streams.is_empty() {
        return Err("no playable stream".into());
    }

    // Resume and skip data are both best-effort: a database hiccup or a missing MAL id must
    // not stop an episode from playing.
    let resume_at = store
        .resume_position(id, episode)
        .inspect_err(|e| tracing::warn!(error = %e, "could not read resume position"))
        .unwrap_or(None);
    let mal_id = store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id);

    let context = playback::PlaybackContext {
        anilist_id: id,
        mal_id,
        episode: episode.to_owned(),
        // The name the terminal shows the title by, so the player's top bar, the window and
        // the terminal agree. The match target's first title is whichever the matcher tries
        // first, which is often the romaji one.
        title: media.title.display().to_owned(),
        translation: playback.translation,
        resume_at,
        runtime_secs: media.duration.map(|minutes| minutes * 60),
        speed: playback.persist_speed.then_some(playback.persisted_speed).flatten(),
        volume: playback.persist_volume.then_some(playback.persisted_volume).flatten(),
        // Filled in by the caller, which has the episode table; headless play has none.
        next: None,
        episodes: Vec::new(),
    };
    Ok((streams, context))
}

/// The selectable releases for one episode, across every usable provider.
///
/// Shares the title-matching rung with playback: the slate is only meaningful for the
/// provider key the ladder would actually use.
async fn source_slate(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: anistream_core::ids::AnilistId,
    episode: &str,
    translation: anistream_core::media::Translation,
) -> std::result::Result<Vec<anistream_core::traits::SourceCandidate>, String> {
    if registry.is_empty() {
        return Err("no sources configured — see the Providers screen".into());
    }
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    let target = media.match_target();
    let now = now_epoch();
    // Each provider's releases under its own key: one provider's key means nothing to another.
    // A provider that cannot match the title has nothing to offer here, and the rest still do.
    let mut slate = Vec::new();
    let mut reasons = Vec::new();
    for provider_id in registry.usable_ids() {
        let Some(source) = registry.pinned(&provider_id) else { continue };
        let resolution =
            anistream_providers::resolve(store, &source, id, &target, translation, now).await;
        match resolution.key() {
            Some(key) => slate.extend(source.sources(key, episode, translation, now).await),
            None => reasons.push(format!("{provider_id}: {}", resolution.explain())),
        }
    }
    if slate.is_empty() && !reasons.is_empty() {
        return Err(format!("could not match this title: {}", reasons.join(", ")));
    }
    Ok(slate)
}

/// The releases an episode can be played from, for the Sources overlay.
pub fn load_sources(
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { meta, store, registry, .. } = services;
    let config = engine.config.effective();
    let (store, registry, meta, tx) =
        (store.clone(), registry.clone(), meta.clone(), tx.clone());
    let translation = config.playback.translation;
    tokio::spawn(async move {
        let _job = job;
        let update =
            match source_slate(&meta, &store, &registry, id, &episode, translation).await {
                Ok(candidates) => Event::Sources { id, episode, candidates },
                Err(reason) => Event::Toast(Notice::alert(reason)),
            };
        let _ = tx.send(update);
    });
}
