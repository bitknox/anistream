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

/// What a lookup says when there is no source to ask.
const NO_SOURCES: &str = "no sources configured — see the Providers screen";

/// Why `episode` of `media` cannot be played yet, when its schedule says it has not aired.
///
/// Asked for, every source would be tried in turn for an episode none of them can have, and
/// the play would fail in the end as a missing one. Only an episode the schedule rules out: one
/// whose own broadcast is still ahead, said by its own time, or, past the broadcasts the title
/// carries, one after the next to air while that is still ahead. A label that is not a number,
/// or a title with no schedule, is looked up as ever.
pub(crate) fn not_aired(media: &anistream_meta::Media, episode: &str) -> Option<String> {
    not_aired_in(media, episode, &jiff::tz::TimeZone::system())
}

/// [`not_aired`], saying the time in `zone`.
fn not_aired_in(
    media: &anistream_meta::Media,
    episode: &str,
    zone: &jiff::tz::TimeZone,
) -> Option<String> {
    let number: u32 = episode.trim().parse().ok()?;
    let next = media.next_airing_episode.as_ref();
    // Now as the answer had it, so what is ahead and the day it falls on are counted by the
    // clock its countdown was; a title with nothing to come has only the wall clock.
    let now = next.map_or_else(now_epoch, |next| next.airing_at - next.time_until_airing);
    match media.air_times().get(&number) {
        Some(&at) if at > now => {
            Some(format!("episode {number} airs {}", airs_when(at, now, zone)?))
        }
        Some(_) => None,
        // Not among the broadcasts the title carries: ruled out only by coming after the next.
        None => {
            let next =
                next.filter(|next| next.time_until_airing > 0 && number > next.episode)?;
            let when = airs_when(next.airing_at, now, zone)?;
            Some(format!(
                "episode {number} hasn't aired yet; episode {} airs {when}",
                next.episode
            ))
        }
    }
}

/// When a broadcast at `at` is, as someone in `zone` says it at `now`: "today 18:30",
/// "tomorrow 18:30", "Saturday 18:30" within the week and "Saturday 2 November 18:30" after.
fn airs_when(at: i64, now: i64, zone: &jiff::tz::TimeZone) -> Option<String> {
    let at = jiff::Timestamp::from_second(at).ok()?.to_zoned(zone.clone());
    let today = jiff::Timestamp::from_second(now).ok()?.to_zoned(zone.clone()).date();
    let days = today.until(at.date()).ok()?.get_days();
    let time = at.strftime("%H:%M");
    Some(match days {
        ..=0 => format!("today {time}"),
        1 => format!("tomorrow {time}"),
        2..=6 => format!("{} {time}", at.strftime("%A")),
        _ => format!("{} {time}", at.strftime("%A %-d %B")),
    })
}

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
    if registry.is_empty() && !registry.is_arriving() {
        return Err(NO_SOURCES.into());
    }
    // Phase-labelled, because the wait between Enter and mpv is where every tool in
    // this category earns its "is it frozen?" issues. The status line names the rung
    // being climbed; the eyecatch's own timer says how long it has taken.
    step("looking up the title".into());
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    if let Some(reason) = not_aired(&media, episode) {
        return Err(reason);
    }
    // Plugins still compiling are part of the chain a moment from now. Looked up without
    // them, the episode would come from whichever source was ready first, or from none.
    crate::sources::arrived(registry, || step("loading sources".into())).await;
    if registry.is_empty() {
        return Err(NO_SOURCES.into());
    }
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
    // Then, within one quality, the servers that keep failing after the ones that play. Never a
    // release the user picked, which plays as it was picked.
    playback::health::order(store, &mut streams, playback.quality, pick.is_some(), now);
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
        cover_url: media.cover_image.best().map(str::to_owned),
        banner_url: media.banner_image.clone(),
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
    if registry.is_empty() && !registry.is_arriving() {
        return Err(NO_SOURCES.into());
    }
    let media = meta.media(id).await.map_err(|e| e.to_string())?;
    if let Some(reason) = not_aired(&media, episode) {
        return Err(reason);
    }
    crate::sources::arrived(registry, || ()).await;
    if registry.is_empty() {
        return Err(NO_SOURCES.into());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600;

    /// A title whose episode `next` airs `in_secs` after the moment `now` it was asked for.
    fn airing(next: u32, now: i64, in_secs: i64) -> anistream_meta::Media {
        serde_json::from_value(serde_json::json!({
            "id": 154_587,
            "nextAiringEpisode": {
                "episode": next,
                "airingAt": now + in_secs,
                "timeUntilAiring": in_secs,
            },
        }))
        .unwrap()
    }

    /// Two hours east, as the reader is.
    fn zone() -> jiff::tz::TimeZone {
        jiff::tz::TimeZone::fixed(jiff::tz::offset(2))
    }

    /// Unix seconds for a local time in [`zone`].
    fn local(at: &str) -> i64 {
        let civil: jiff::civil::DateTime = at.parse().unwrap();
        civil.to_zoned(zone()).unwrap().timestamp().as_second()
    }

    #[test]
    fn an_episode_yet_to_air_is_said_by_when_it_airs_where_the_reader_is() {
        // Wednesday 21 October 2026, ten in the morning.
        let now = local("2026-10-21T10:00");
        let said = |next, in_secs, episode| {
            not_aired_in(&airing(next, now, in_secs), episode, &zone())
        };
        assert_eq!(
            said(4, 8 * HOUR + 1_800, "4").as_deref(),
            Some("episode 4 airs today 18:30")
        );
        assert_eq!(said(4, 15 * HOUR, "4").as_deref(), Some("episode 4 airs tomorrow 01:00"));
        let saturday = local("2026-10-24T18:30") - now;
        assert_eq!(said(4, saturday, "4").as_deref(), Some("episode 4 airs Saturday 18:30"));
        let fortnight = local("2026-11-02T18:30") - now;
        assert_eq!(
            said(4, fortnight, "4").as_deref(),
            Some("episode 4 airs Monday 2 November 18:30")
        );
        // One after it has not aired either, and is told when the next one is.
        assert_eq!(
            said(4, saturday, " 6").as_deref(),
            Some("episode 6 hasn't aired yet; episode 4 airs Saturday 18:30")
        );
    }

    #[test]
    fn an_episode_further_out_is_said_by_its_own_broadcast() {
        let now = local("2026-10-21T10:00");
        let saturday = local("2026-10-24T18:30");
        // The next is episode 4 on Saturday; 5 and 6 are on the schedule the weeks after.
        let media: anistream_meta::Media = serde_json::from_value(serde_json::json!({
            "id": 154_587,
            "nextAiringEpisode": {
                "episode": 4,
                "airingAt": saturday,
                "timeUntilAiring": saturday - now,
            },
            "upcoming": { "nodes": [
                { "episode": 4, "airingAt": saturday },
                { "episode": 5, "airingAt": saturday + 7 * 24 * HOUR },
                { "episode": 6, "airingAt": saturday + 14 * 24 * HOUR },
            ] },
            "aired": [{ "episode": 3, "airingAt": saturday - 7 * 24 * HOUR }],
        }))
        .unwrap();
        let said = |episode| not_aired_in(&media, episode, &zone());
        assert_eq!(said("4").as_deref(), Some("episode 4 airs Saturday 18:30"));
        assert_eq!(said("5").as_deref(), Some("episode 5 airs Saturday 31 October 18:30"));
        assert_eq!(said("6").as_deref(), Some("episode 6 airs Saturday 7 November 18:30"));
        // Past what the schedule carries, the next one is what can be said.
        assert_eq!(
            said("9").as_deref(),
            Some("episode 9 hasn't aired yet; episode 4 airs Saturday 18:30")
        );
        // Out already.
        assert_eq!(said("3"), None);
    }

    #[test]
    fn an_episode_the_schedule_does_not_rule_out_is_looked_up() {
        let now = local("2026-10-21T10:00");
        let said = |next, in_secs, episode| {
            not_aired_in(&airing(next, now, in_secs), episode, &zone())
        };
        // Aired already, or due by now with the schedule not yet moved on.
        assert_eq!(said(4, HOUR, "3"), None);
        assert_eq!(said(4, 0, "4"), None);
        assert_eq!(said(4, -HOUR, "5"), None);
        // Not a number the schedule counts.
        assert_eq!(said(4, HOUR, "4.5"), None);
        assert_eq!(said(4, HOUR, "OVA"), None);
        // No schedule at all: finished, or not announced.
        let finished: anistream_meta::Media =
            serde_json::from_value(serde_json::json!({ "id": 1 })).unwrap();
        assert_eq!(not_aired_in(&finished, "13", &zone()), None);
    }
}
