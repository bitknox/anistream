//! Finding what an episode plays from: its streams, and the context its playback records
//! history against. And the releases an episode can be played from, for the user to pick.

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::{Cause, Msg};
use anistream_model::{event::Event, notice::Notice, party::NoSourceWhy};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{
    jobs::JobGuard, now_epoch, outbox::Outbox, playback, services::Services, state::EngineState,
};

/// What a lookup says when there is no source to ask.
pub(crate) fn no_sources() -> Cause {
    Cause::NoSources
}

/// A reason as the toast or the dialog says it.
pub(crate) fn explained(cause: Cause) -> Msg {
    Msg::NoticeReason { reason: cause }
}

/// Why `episode` of `media` cannot be played yet, when its schedule says it has not aired.
///
/// Asked for, every source would be tried in turn for an episode none of them can have, and
/// the play would fail in the end as a missing one. Only an episode the schedule rules out: one
/// whose own broadcast is still ahead, said by its own time, or, past the broadcasts the title
/// carries, one after the next to air while that is still ahead. A label that is not a number,
/// or a title with no schedule, is looked up as ever.
pub(crate) fn not_aired(media: &anistream_meta::Media, episode: &str) -> Option<Msg> {
    not_aired_in(media, episode, &jiff::tz::TimeZone::system())
}

/// [`not_aired`], saying the time in `zone`.
fn not_aired_in(
    media: &anistream_meta::Media,
    episode: &str,
    zone: &jiff::tz::TimeZone,
) -> Option<Msg> {
    let number: u32 = episode.trim().parse().ok()?;
    let next = media.next_airing_episode.as_ref();
    // Now as the answer had it, so what is ahead and the day it falls on are counted by the
    // clock its countdown was; a title with nothing to come has only the wall clock.
    let now = next.map_or_else(now_epoch, |next| next.airing_at - next.time_until_airing);
    match media.air_times().get(&number) {
        Some(&at) if at > now => Some(Msg::NoticeNotAired {
            episode: i64::from(number),
            when: airs_when(at, now, zone)?,
        }),
        Some(_) => None,
        // Not among the broadcasts the title carries: ruled out only by coming after the next.
        None => {
            let next =
                next.filter(|next| next.time_until_airing > 0 && number > next.episode)?;
            let when = airs_when(next.airing_at, now, zone)?;
            Some(Msg::NoticeNotAiredYet {
                episode: i64::from(number),
                next: i64::from(next.episode),
                when,
            })
        }
    }
}

/// Why a play found nothing to play, as the lookup decided it: its kind, for a party's
/// no-source dialog to offer what fits, and its words, for a toast or the terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct Unplayable {
    pub why: NoSourceWhy,
    pub text: Msg,
}

impl Unplayable {
    /// No source is set up to ask: [`no_sources`].
    pub(crate) fn no_sources() -> Self {
        Self { why: NoSourceWhy::NoSources, text: explained(no_sources()) }
    }

    /// The schedule rules the episode out: [`not_aired`]'s words.
    pub(crate) fn not_aired(text: Msg) -> Self {
        Self { why: NoSourceWhy::NotAired, text }
    }
}

/// Any other failure: the sources were asked, and none of them played it.
impl From<Cause> for Unplayable {
    fn from(cause: Cause) -> Self {
        Self { why: NoSourceWhy::NotFound, text: explained(cause) }
    }
}

impl std::fmt::Display for Unplayable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl std::error::Error for Unplayable {}

/// When a broadcast at `at` is, as someone in `zone` says it at `now`: today, tomorrow, in so
/// many days within the week, and on its date after.
fn airs_when(at: i64, now: i64, zone: &jiff::tz::TimeZone) -> Option<Cause> {
    let at = jiff::Timestamp::from_second(at).ok()?.to_zoned(zone.clone());
    let today = jiff::Timestamp::from_second(now).ok()?.to_zoned(zone.clone()).date();
    let days = today.until(at.date()).ok()?.get_days();
    let time = at.strftime("%H:%M").to_string();
    Some(match days {
        ..=0 => Cause::AirsToday { time },
        1 => Cause::AirsTomorrow { time },
        2..=6 => Cause::AirsInDays { days: i64::from(days), time },
        _ => Cause::AirsOnDate { date: at.strftime("%Y-%m-%d").to_string(), time },
    })
}

/// An episode's streams and playback context, as resolution produces them.
pub type Resolved = (Vec<anistream_core::stream::Stream>, playback::PlaybackContext);

/// Everything needed before mpv can be spawned: a stream, and the context history needs.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
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
) -> std::result::Result<
    (Vec<anistream_core::stream::Stream>, playback::PlaybackContext),
    Unplayable,
> {
    // The step under way, on the job when there is one; the CLI has no activity row and reads
    // the status channel instead.
    let step = |text: Msg| match job {
        Some(job) => job.note(text, None),
        None => {
            let _ = tx.send(Event::Status(Some(text)));
        }
    };
    let began = std::time::Instant::now();
    tracing::info!(
        %id, episode, sources = ?registry.ids(), usable = ?registry.usable_ids(), picked = pick.is_some(),
        "resolving a play"
    );
    if registry.is_empty() && !registry.is_arriving() {
        tracing::warn!("no sources to resolve against");
        return Err(Unplayable::no_sources());
    }
    // Phase-labelled, because the wait between Enter and mpv is where every tool in
    // this category earns its "is it frozen?" issues. The status line names the rung
    // being climbed; the eyecatch's own timer says how long it has taken.
    step(Msg::StatusLookingUpTitle);
    let media = meta.media(id).await.map_err(|e| {
        tracing::warn!(error = %e, elapsed_ms = began.elapsed().as_millis() as u64, "could not look up the title");
        Cause::verbatim(e.to_string())
    })?;
    tracing::info!(elapsed_ms = began.elapsed().as_millis() as u64, "title looked up");
    if let Some(reason) = not_aired(&media, episode) {
        return Err(Unplayable::not_aired(reason));
    }
    // Plugins still compiling are part of the chain a moment from now. Looked up without
    // them, the episode would come from whichever source was ready first, or from none.
    crate::sources::arrived(registry, || step(Msg::StatusLoadingSources)).await;
    if registry.is_empty() {
        tracing::warn!("no sources to resolve against");
        return Err(Unplayable::no_sources());
    }
    let now = now_epoch();

    // "Use this source for this series", applied *before* the title is matched.
    //
    // The order matters: a `ProviderKey` is only meaningful to the provider that issued it, so
    // matching against the whole chain and then resolving against a pinned source would hand one
    // provider's key to another and look like a missing episode. Each source is matched on its
    // own, so the pinned one is simply first.
    //
    // A pin ranks a source; it does not remove the others. A pinned source with part of a season
    // hands the episodes it lacks to the next, as the automatic order does.
    //
    // Skipped when the user picked an exact release from the Sources overlay. That pick is the
    // more immediate instruction of the two, it already names its own provider, and honouring the
    // remembered preference instead would refuse a release the user is looking straight at.
    //
    // A preference naming a provider that is not registered, is switched off or is resting falls
    // back to the normal chain: refusing to play because a remembered choice is momentarily
    // unusable would be a worse answer than playing.
    let chain = pick.is_none().then(|| {
        match crate::titles::episodes::pin_of(store, registry, id) {
            crate::titles::episodes::Pin::Held(name) => {
                tracing::info!(provider = %name, "the series is pinned to a source");
            }
            crate::titles::episodes::Pin::Unavailable(name) => {
                tracing::info!(provider = %name, "pinned source is not usable, using the chain");
            }
            crate::titles::episodes::Pin::Automatic => {}
        }
        crate::titles::episodes::chain_for(store, registry, id)
    });
    let registry = chain.as_ref().unwrap_or(registry);
    let target = media.match_target();

    step(Msg::StatusSearchingSources { episode: episode.to_owned() });
    let mut streams = match &pick {
        // A picked release goes straight to its provider, no failover: substituting a
        // different stream for the one the user chose would silently undo the choice. Matched
        // on that provider, whose key it is.
        Some((provider_id, source_id)) => {
            let source = registry
                .pinned(provider_id)
                .ok_or_else(|| Cause::SourceGone { source: provider_id.clone() })?;
            let resolution = anistream_providers::resolve(
                store,
                &source,
                id,
                &target,
                playback.translation,
                now,
            )
            .await;
            let key = resolution
                .key()
                .cloned()
                .ok_or_else(|| Cause::TitleNoMatch { detail: resolution.explain() })?;
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
                .map_err(|e| Cause::ReleaseFailed { detail: e.to_string() })?
        }
        None => {
            let from_release = pinned_release(
                store,
                registry,
                id,
                &target,
                episode,
                playback.translation,
                now,
            )
            .await;
            if let Some(streams) = from_release {
                streams
            } else {
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
                        return Err(Cause::TitleNoMatch { detail: answer.explain() }.into());
                    }
                    anistream_providers::Answer::Unanswered { matched, .. } => {
                        return Err(no_source_has(episode, matched, &answer.explain()).into());
                    }
                }
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
        tracing::warn!(
            elapsed_ms = began.elapsed().as_millis() as u64,
            "the source answered with no stream"
        );
        return Err(Cause::NoPlayableStream.into());
    }
    if let Some(first) = streams.first() {
        tracing::info!(
            streams = streams.len(),
            provider = %first.provider_id,
            kind = ?first.kind,
            quality = ?first.quality,
            from = %crate::logged::address(&first.url),
            elapsed_ms = began.elapsed().as_millis() as u64,
            "streams found; the first is picked"
        );
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
        title: meta.titles().pick(&media.title).to_owned(),
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
        episode_total: media.episodes,
    };
    Ok((streams, context))
}

/// The streams of the release pinned for a title, when one is pinned, its source is in use and
/// the release holds `episode`. Otherwise `None`, and the episode is found as ever: a batch
/// that stops at episode 6 is no reason to refuse episode 7.
async fn pinned_release(
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    target: &anistream_meta::title::MatchTarget,
    episode: &str,
    translation: anistream_core::media::Translation,
    now: i64,
) -> Option<Vec<anistream_core::stream::Stream>> {
    let pin = store.release_preference(id).ok().flatten()?;
    if !registry.usable_ids().contains(&pin.provider_id) {
        return None;
    }
    let source = registry.pinned(&pin.provider_id)?;
    let resolution =
        anistream_providers::resolve(store, &source, id, target, translation, now).await;
    let key = resolution.key()?.clone();
    let offered = source.sources(&key, episode, translation, now).await;
    if !offered.iter().any(|candidate| candidate.id == pin.release_id) {
        tracing::info!(provider = %pin.provider_id, episode, "the pinned release does not hold this episode");
        return None;
    }
    match source
        .resolve_source(&pin.provider_id, &key, episode, translation, &pin.release_id, now)
        .await
    {
        Ok(streams) if !streams.is_empty() => Some(streams),
        Ok(_) => None,
        Err(error) => {
            tracing::info!(provider = %pin.provider_id, %error, "the pinned release did not play");
            None
        }
    }
}

/// What a play says when no source gave the episode: that none has it, when each source matched
/// the title and lacked the episode, and otherwise what each source said.
fn no_source_has(episode: &str, matched: bool, reasons: &str) -> Cause {
    if matched {
        Cause::NoSourceHasEpisode { episode: episode.to_owned(), detail: reasons.to_owned() }
    } else {
        Cause::verbatim(reasons)
    }
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
) -> std::result::Result<Vec<anistream_core::traits::SourceCandidate>, Msg> {
    if registry.is_empty() && !registry.is_arriving() {
        return Err(explained(no_sources()));
    }
    let media = meta.media(id).await.map_err(|e| Msg::verbatim(e.to_string()))?;
    if let Some(reason) = not_aired(&media, episode) {
        return Err(reason);
    }
    crate::sources::arrived(registry, || ()).await;
    if registry.is_empty() {
        return Err(explained(no_sources()));
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
        return Err(explained(Cause::TitleNoMatch { detail: reasons.join(", ") }));
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
                Err(msg) => Event::Toast(Notice::alert(msg)),
            };
        let _ = tx.send(update);
    });
}

#[cfg(test)]
mod tests {
    use anistream_core::config::NetworkConfig;
    use anistream_meta::AniList;
    use anistream_providers::MockProvider;

    use super::*;

    const HOUR: i64 = 3_600;
    const DAY: i64 = 86_400;

    /// What a play of `episode` of the cached title `media` fails with, against `sources`,
    /// offline.
    async fn failure(
        media: serde_json::Value,
        episode: &str,
        sources: Vec<MockProvider>,
    ) -> Unplayable {
        let store = Store::open_in_memory().unwrap();
        store.cache_media(AnilistId::new(900_201), &media.to_string(), now_epoch()).unwrap();
        let http = anistream_net::HttpClient::offline(&NetworkConfig::default()).unwrap();
        let meta = Meta::new(AniList::new(http, 30), None, store.clone());
        let registry =
            ProviderRegistry::new(sources.into_iter().map(MockProvider::arc).collect());
        let tx = Outbox::from(tokio::sync::mpsc::unbounded_channel().0);
        let playback = anistream_core::config::PlaybackConfig::default();
        let found = resolve_for_playback(
            &meta,
            &store,
            &registry,
            AnilistId::new(900_201),
            episode,
            None,
            &playback,
            None,
            &tx,
        )
        .await;
        found.expect_err("the play fails")
    }

    fn show(airing: bool) -> serde_json::Value {
        let mut media = serde_json::json!({
            "id": 900_201,
            "title": { "romaji": "Hoshi no Kakurega", "english": "The Hidden Star" },
            "format": "TV",
            "status": if airing { "RELEASING" } else { "FINISHED" },
            "episodes": 12,
        });
        if airing {
            media["nextAiringEpisode"] = serde_json::json!({ "episode": 6, "airingAt": now_epoch() + DAY, "timeUntilAiring": DAY });
        }
        media
    }

    #[tokio::test]
    async fn each_failure_says_its_kind_where_it_is_decided() {
        // No source to ask.
        let none = failure(show(false), "3", Vec::new()).await;
        assert_eq!(none.why, NoSourceWhy::NoSources);
        assert_eq!(none.text, explained(no_sources()));
        // The schedule rules the episode out before any source is asked.
        let unaired = failure(show(true), "8", vec![MockProvider::new("sample")]).await;
        assert_eq!(unaired.why, NoSourceWhy::NotAired);
        // Asked, and no source had it.
        let missing = failure(show(false), "3", vec![MockProvider::new("sample")]).await;
        assert_eq!(missing.why, NoSourceWhy::NotFound);
        assert!(
            missing.text.english().contains("sample"),
            "the source's answer, not the title's lookup: {}",
            missing.text.english()
        );
    }

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
            not_aired_in(&airing(next, now, in_secs), episode, &zone()).map(|msg| msg.english())
        };
        assert_eq!(
            said(4, 8 * HOUR + 1_800, "4").as_deref(),
            Some("Episode 4 airs today at 18:30.")
        );
        assert_eq!(
            said(4, 15 * HOUR, "4").as_deref(),
            Some("Episode 4 airs tomorrow at 01:00.")
        );
        let saturday = local("2026-10-24T18:30") - now;
        assert_eq!(
            said(4, saturday, "4").as_deref(),
            Some("Episode 4 airs in 3 days at 18:30.")
        );
        let fortnight = local("2026-11-02T18:30") - now;
        assert_eq!(
            said(4, fortnight, "4").as_deref(),
            Some("Episode 4 airs on 2026-11-02 at 18:30.")
        );
        // One after it has not aired either, and is told when the next one is.
        assert_eq!(
            said(4, saturday, " 6").as_deref(),
            Some("Episode 6 hasn't aired yet. Episode 4 airs in 3 days at 18:30.")
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
        let said = |episode| not_aired_in(&media, episode, &zone()).map(|msg| msg.english());
        assert_eq!(said("4").as_deref(), Some("Episode 4 airs in 3 days at 18:30."));
        assert_eq!(said("5").as_deref(), Some("Episode 5 airs on 2026-10-31 at 18:30."));
        assert_eq!(said("6").as_deref(), Some("Episode 6 airs on 2026-11-07 at 18:30."));
        // Past what the schedule carries, the next one is what can be said.
        assert_eq!(
            said("9").as_deref(),
            Some("Episode 9 hasn't aired yet. Episode 4 airs in 3 days at 18:30.")
        );
        // Out already.
        assert_eq!(said("3"), None);
    }

    #[test]
    fn an_episode_the_schedule_does_not_rule_out_is_looked_up() {
        let now = local("2026-10-21T10:00");
        let said = |next, in_secs, episode| {
            not_aired_in(&airing(next, now, in_secs), episode, &zone()).map(|msg| msg.english())
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
