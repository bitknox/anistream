//! The new-episode check: which episodes of the shows followed aired since they were last looked
//! at, each said once, for a native app to notify of. See `Command::CheckNewEpisodes`.
//!
//! What the check keeps between runs is in the database (`0009_episode_notices`): for each show,
//! the newest episode said, the newest dub said of one watched dubbed, and when the show was
//! first followed, and how far the check has looked. A show is first followed silently, from the moment it is, which is all a first check
//! does: nothing that aired before it is ever said, so turning notifications on, or adding a
//! show to the list, never fires a backlog.
//!
//! The shows it follows are those being watched: on the list as watching or rewatching, or
//! watched here lately. A show switched on for itself is followed whatever the list says of it,
//! one planned among them, and one switched off is not. With Settings' switch off, only the
//! shows switched on are followed.
//!
//! A show watched dubbed is said when its dub is out. AniList dates only the original broadcast,
//! so for such a show a broadcast only moves on how far the show has aired, and the dub is said
//! once the source a play would use lists it: each check lists the shows whose broadcast is
//! ahead of the dub said, one listing each, and none for a show whose dub has caught up.
//!
//! It is written for a background task's few seconds, on an engine that does nothing else:
//!
//! - the shows followed are read from the database, the list as its last pull left it, and
//!   history, so no tracker is asked;
//! - AniList is asked once at most, for every show at once, and not at all when the last look
//!   was minutes ago or the metadata cache shows nothing can have aired since;
//! - the ask is cut off at [`ASK_FOR`], and a check cut off says nothing and keeps its place, so
//!   the next one looks again from there;
//! - the dubs are listed all at once, cut off at [`DUB_FOR`], and a show's cut off is listed
//!   again by the next check.
//!
//! Each episode said carries how far behind the viewer is, the translation it is out in and the
//! show's banner, for the notification's words and picture.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use anistream_core::{
    config::Config,
    ids::AnilistId,
    media::{Episode, MediaStatus, Translation},
};
use anistream_meta::{Media, Meta, anilist::AiringEntry, titles::Titles};
use anistream_model::{event::Event, notifications::NewEpisode, show::ShowSettingKey};
use anistream_providers::ProviderRegistry;
use anistream_store::{EpisodeNotice, Store};

use crate::{
    clock::Clock, data, outbox::Outbox, services::Services, show_settings, state::EngineState,
    titles::episodes::dub_listing,
};

const HOUR: i64 = 3_600;
const DAY: i64 = 24 * HOUR;

/// How recently a show has to have been watched here to be followed without being on the list.
pub(crate) const WATCHED_WITHIN: i64 = 30 * DAY;

/// The furthest back a check looks, however long since the last one: after a long time away an
/// episode a week old is old news, and a pile of notifications for them would be noise.
const LOOKBACK: i64 = 7 * DAY;

/// How far before the last look the next one starts, for a broadcast AniList moved earlier after
/// that look. Saying one twice is prevented by the episode kept for each show, not by this.
const SLACK: i64 = HOUR;

/// A look this recent answers for now as well: the time AniList's answers are used again for,
/// `anistream_meta::source::REUSE_FOR`. A front end that checks on every launch and again from
/// its background task spends one request, not two.
const REUSE_FOR: i64 = anistream_meta::source::REUSE_FOR.as_secs() as i64;

/// The longest the ask may take, inside a background task's thirty seconds with room to spare.
const ASK_FOR: Duration = Duration::from_secs(15);

/// The longest a show's dub may take to list, after the ask: the listings run at once, so this
/// is all of them together, and the check still answers inside a background task's time.
const DUB_FOR: Duration = Duration::from_secs(8);

/// The tracker statuses of a show being watched: watching or rewatching.
const WATCHING: [&str; 2] = ["CURRENT", "REPEATING"];

/// The status of a show planned, which the widgets count as on the list and the check does not
/// follow unless the show is switched on for itself.
const PLANNING: &str = "PLANNING";

/// The statuses that say a show is not being watched, whatever history says.
const SET_ASIDE: [&str; 2] = ["DROPPED", "PAUSED"];

/// Run a check, off the actor, and say what it found as [`Event::NewEpisodes`]: once, empty
/// when there is nothing to say. With notifications off it follows only the shows switched on
/// for themselves; with none of those either, it asks nothing and forgets what it kept, so the
/// first check after one is turned on again is a first check.
pub(crate) fn check(engine: &EngineState, services: &Services, tx: &Outbox) {
    let check = Check {
        store: services.store.clone(),
        meta: services.meta.clone(),
        registry: services.registry.clone(),
        signed_in: list_signed_in(services),
        enabled: engine.config.effective().notifications.episodes,
        clock: services.clock.clone(),
        config: engine.config.effective().clone(),
    };
    let (turn, tx) = (Arc::clone(&engine.episode_check), tx.clone());
    tokio::spawn(async move {
        let _turn = turn.lock().await;
        let found = if check.enabled || !switched(&check.store, "true").is_empty() {
            check.run().await
        } else {
            if let Err(error) = check.store.forget_episode_check() {
                tracing::warn!(%error, "could not forget the new-episode check");
            }
            Vec::new()
        };
        let _ = tx.send(Event::NewEpisodes(found));
    });
}

/// Whether the Library's tracker is signed in, whose list the database keeps a copy of.
pub(crate) fn list_signed_in(services: &Services) -> bool {
    services.sync.trackers.first().is_some_and(|tracker| tracker.is_authenticated())
}

/// The shows followed, as of `now`: on the list as watching or rewatching, and as planning too
/// when `planning` says so, when the list is a signed-in tracker's; and watched here in the last
/// [`WATCHED_WITHIN`], unless the list has them dropped or paused. The list's first, then
/// history's, each once.
pub(crate) fn followed(
    store: &Store,
    signed_in: bool,
    now: i64,
    planning: bool,
) -> Vec<AnilistId> {
    let statuses = if signed_in {
        store.library_statuses().unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read the list for the new-episode check");
            Vec::new()
        })
    } else {
        Vec::new()
    };
    let set_aside: HashSet<AnilistId> = statuses
        .iter()
        .filter(|(_, status)| SET_ASIDE.contains(&status.as_str()))
        .map(|(id, _)| *id)
        .collect();
    let listed = statuses
        .iter()
        .filter(|(_, status)| {
            WATCHING.contains(&status.as_str()) || (planning && status == PLANNING)
        })
        .map(|(id, _)| *id);
    let watched = store.watched_since(now - WATCHED_WITHIN).unwrap_or_else(|error| {
        tracing::warn!(%error, "could not read history for the new-episode check");
        Vec::new()
    });
    let mut seen = HashSet::new();
    listed
        .chain(watched.into_iter().filter(|id| !set_aside.contains(id)))
        .filter(|id| seen.insert(*id))
        .collect()
}

/// The shows that switched notifications on, or off, for themselves: `value` is the switch's
/// stored word, `"true"` or `"false"`.
pub(crate) fn switched(store: &Store, value: &str) -> Vec<AnilistId> {
    store.shows_setting(ShowSettingKey::NotifyNewEpisodes.name(), value).unwrap_or_else(
        |error| {
            tracing::warn!(%error, "could not read the shows' own notification switches");
            Vec::new()
        },
    )
}

/// The shows a check looks at, as of `now`: those [`followed`], planned ones left out, and every
/// show switched on for itself, whatever the list says of it; but none switched off. With
/// Settings' switch off, `enabled` false, only those switched on.
fn notified(store: &Store, signed_in: bool, now: i64, enabled: bool) -> Vec<AnilistId> {
    let on = switched(store, "true");
    if !enabled {
        return on;
    }
    let off: HashSet<AnilistId> = switched(store, "false").into_iter().collect();
    let mut seen = HashSet::new();
    followed(store, signed_in, now, false)
        .into_iter()
        .chain(on)
        .filter(|id| !off.contains(id) && seen.insert(*id))
        .collect()
}

/// The translation the viewer watches `id` in: its own setting, or Settings'.
fn translation(config: &Config, store: &Store, id: AnilistId) -> Translation {
    show_settings::for_show(config, store, id).playback.translation
}

/// The highest whole episode a listing reaches: 6 for one that ends at "6" or "6.5", `None` for
/// one with no episode numbered.
fn reach(episodes: &[Episode]) -> Option<u32> {
    episodes
        .iter()
        .filter_map(|episode| episode.number.as_number())
        .filter(|number| *number >= 1.0)
        .map(|number| number.floor() as u32)
        .max()
}

/// What one check works with.
struct Check {
    store: Store,
    meta: Meta,
    /// The sources, for the dub of a show watched dubbed.
    registry: ProviderRegistry,
    signed_in: bool,
    /// Whether Settings' switch is on, which the shows being watched follow.
    enabled: bool,
    clock: Clock,
    /// Settings, for the translation each show is watched in.
    config: Config,
}

impl Check {
    /// What aired since the last look, for the shows followed and not switched off: see the
    /// module's docs. What it finds is kept before it is said.
    async fn run(&self) -> Vec<NewEpisode> {
        let now = self.clock.now();
        let shows = notified(&self.store, self.signed_in, now, self.enabled);

        let kept: HashMap<AnilistId, EpisodeNotice> = match self.store.episode_notices() {
            Ok(kept) => kept.into_iter().map(|notice| (notice.anilist_id, notice)).collect(),
            Err(error) => {
                tracing::warn!(%error, "could not read what the new-episode check said");
                return Vec::new();
            }
        };
        // A show followed no longer is forgotten, so one followed again starts from then.
        let gone: Vec<AnilistId> =
            kept.keys().filter(|id| !shows.contains(id)).copied().collect();
        if let Err(error) = self.store.forget_episode_notices(&gone) {
            tracing::warn!(%error, "could not forget the shows no longer followed");
        }
        // A show followed for the first time is followed from now, and nothing is said of it.
        let fresh: Vec<EpisodeNotice> = shows
            .iter()
            .filter(|id| !kept.contains_key(id))
            .map(|id| EpisodeNotice { anilist_id: *id, episode: 0, since: now, dubbed: None })
            .collect();
        let following: Vec<EpisodeNotice> =
            shows.iter().filter_map(|id| kept.get(id).copied()).collect();

        let through = match self.store.episode_check_through() {
            Ok(through) => through,
            Err(error) => {
                tracing::warn!(%error, "could not read how far the new-episode check looked");
                return Vec::new();
            }
        };
        let Some(through) = through else {
            // The first check: where things stand, kept, and nothing said.
            self.keep(&fresh, now, now);
            return Vec::new();
        };
        let ids: Vec<AnilistId> = following.iter().map(|notice| notice.anilist_id).collect();
        if ids.is_empty() || now - through < REUSE_FOR {
            self.keep(&fresh, through, now);
            return Vec::new();
        }

        let mut notices: HashMap<AnilistId, EpisodeNotice> =
            following.iter().map(|notice| (notice.anilist_id, *notice)).collect();
        // Where the check has looked as far as stays put when AniList is not asked, or does not
        // answer: the next check looks again from the same place.
        let (mut said, reached) = if self.quiet(&ids, through, now) {
            (Vec::new(), through)
        } else {
            match self.ask(&ids, through, now).await {
                Some(page) => {
                    let said = self.said(&page.items, &mut notices, now);
                    // A full page is carried on from its last broadcast, so the ones after it
                    // are looked at next time. A page that never got past the last look is let
                    // go, so a check can never be held on the same page.
                    let reached = match page.items.last() {
                        Some(last) if page.has_next && last.airing_at > through => {
                            last.airing_at
                        }
                        _ => now,
                    };
                    (said, reached)
                }
                None => (Vec::new(), through),
            }
        };
        // The dubs out since, of the shows watched dubbed, whatever AniList said.
        said.extend(self.dubbed(&mut notices, now).await);
        let changed: Vec<EpisodeNotice> = notices
            .into_values()
            .filter(|notice| !following.contains(notice))
            .chain(fresh)
            .collect();
        self.keep(&changed, reached, now);
        said
    }

    /// What AniList says aired for `ids` since the last look, `through`, cut off at
    /// [`ASK_FOR`]: `None` when it could not say.
    async fn ask(
        &self,
        ids: &[AnilistId],
        through: i64,
        now: i64,
    ) -> Option<anistream_meta::anilist::Page<AiringEntry>> {
        let from = (through - SLACK).max(now - LOOKBACK);
        // `airingAt_lesser` is strict, and an episode airing this very second has aired.
        let asked = tokio::time::timeout(ASK_FOR, self.meta.aired_since(ids, from, now + 1));
        match asked.await {
            Ok(Ok(page)) => Some(page),
            Ok(Err(error)) => {
                tracing::warn!(%error, "the new-episode check could not ask AniList");
                None
            }
            Err(_) => {
                tracing::warn!("the new-episode check gave up waiting for AniList");
                None
            }
        }
    }

    /// The episodes among `aired`, oldest first, that are new for their show: aired by `now`,
    /// after the show was first followed, and after the newest one said of it. Each moves its
    /// show's notice on, said or not: one already watched here is counted and not said, and one
    /// of a show watched dubbed is left to [`Self::dubbed`], to say once its dub is out.
    fn said(
        &self,
        aired: &[AiringEntry],
        notices: &mut HashMap<AnilistId, EpisodeNotice>,
        now: i64,
    ) -> Vec<NewEpisode> {
        let mut said = Vec::new();
        for row in aired {
            let id = row.media.id;
            let Some(notice) = notices.get_mut(&id) else { continue };
            if row.airing_at > now
                || row.airing_at <= notice.since
                || row.episode <= notice.episode
            {
                continue;
            }
            let translation = translation(&self.config, &self.store, id);
            if translation == Translation::Dub && notice.dubbed.is_none() {
                // The first broadcast since the show was followed dubbed: its dub is said from
                // this one on, or from the one after the last said in the sub.
                let before = if notice.episode > 0 {
                    notice.episode
                } else {
                    row.episode.saturating_sub(1)
                };
                notice.dubbed = Some(before);
            }
            notice.episode = row.episode;
            if translation == Translation::Dub || self.watched(id, row.episode) {
                continue;
            }
            said.push(self.episode(id, &row.media, row.episode, row.airing_at, translation));
        }
        said
    }

    /// The dubs out since the last check, of the shows in `notices` watched dubbed, oldest
    /// first: for each show with a broadcast ahead of the dub said, the source a play would use
    /// is listed in the dub, all at once and cut off at [`DUB_FOR`], and the episodes it lists
    /// up to that broadcast are said, each once. A show watched subtitled lets its dub go.
    async fn dubbed(
        &self,
        notices: &mut HashMap<AnilistId, EpisodeNotice>,
        now: i64,
    ) -> Vec<NewEpisode> {
        let mut behind: Vec<EpisodeNotice> = Vec::new();
        for notice in notices.values_mut() {
            if translation(&self.config, &self.store, notice.anilist_id) != Translation::Dub {
                notice.dubbed = None;
            } else if notice.dubbed.is_some_and(|dubbed| notice.episode > dubbed) {
                behind.push(*notice);
            }
        }
        if behind.is_empty() {
            return Vec::new();
        }
        behind.sort_by_key(|notice| notice.anilist_id);
        let listings = behind.iter().map(|notice| {
            tokio::time::timeout(DUB_FOR, self.dub_reach(notice.anilist_id, now))
        });
        let reaches = futures::future::join_all(listings).await;

        let mut said = Vec::new();
        for (notice, reach) in behind.iter().zip(reaches) {
            let id = notice.anilist_id;
            let (media, reach) = match reach {
                Ok(Some((media, Some(reach)))) => (media, reach),
                // Not listed in the dub yet, or not at all: asked again next time.
                Ok(_) => continue,
                Err(_) => {
                    tracing::warn!(%id, "the new-episode check gave up waiting for a dub");
                    continue;
                }
            };
            let from = notice.dubbed.unwrap_or_default();
            // Up to the broadcast seen: the dub of an episode that aired before the show was
            // followed is never said.
            let to = reach.min(notice.episode);
            if to <= from {
                continue;
            }
            for episode in from + 1..=to {
                if !self.watched(id, episode) {
                    said.push(self.episode(id, &media, episode, now, Translation::Dub));
                }
            }
            if let Some(kept) = notices.get_mut(&id) {
                kept.dubbed = Some(to);
            }
        }
        said
    }

    /// How far `id`'s dub goes on the source a play would use: the show, from the metadata
    /// cache where it is, and the highest whole episode it lists in the dub, `None` when it
    /// lists none. `None` altogether when there is no source, or none answered.
    async fn dub_reach(&self, id: AnilistId, now: i64) -> Option<(Media, Option<u32>)> {
        if self.registry.is_empty() && !self.registry.is_arriving() {
            return None;
        }
        let cached = self.store.cached_media(id).ok().flatten();
        let media = match cached.and_then(|(payload, _)| serde_json::from_str(&payload).ok()) {
            Some(media) => media,
            None => self.meta.media(id).await.ok()?,
        };
        let listed = dub_listing(&media, &self.store, &self.registry, id, now).await?;
        Some((media, reach(&listed)))
    }

    /// Episode `episode` of `id` as a notification says it, out at `at` in `translation`.
    fn episode(
        &self,
        id: AnilistId,
        media: &Media,
        episode: u32,
        at: i64,
        translation: Translation,
    ) -> NewEpisode {
        let titles: &Titles = self.meta.titles();
        // The episode Continue offers next, when it is before this one.
        let next = data::standing(&self.store, id, media.episodes).0.map(|p| p.next);
        NewEpisode {
            id,
            title: titles.pick(&media.title).to_owned(),
            episode,
            aired_at: at,
            cover_url: media.cover_image.best().map(str::to_owned),
            next_for_you: next.filter(|next| *next < episode),
            translation: Some(translation),
            banner_url: media.banner_image.clone(),
        }
    }

    /// Whether episode `episode` of `id` has been watched here already.
    fn watched(&self, id: AnilistId, episode: u32) -> bool {
        let completed = self.store.completed_episodes(id).unwrap_or_default();
        completed.iter().any(|done| done.trim().parse::<u32>() == Ok(episode))
    }

    /// Whether the metadata cache shows that nothing can have aired for any of `ids` since
    /// `through`: each one cached no later than that, and either finished then, or with its
    /// next episode still to come. A show it does not have, or has from after `through`, which
    /// may be after an episode aired, is asked about.
    fn quiet(&self, ids: &[AnilistId], through: i64, now: i64) -> bool {
        ids.iter().all(|id| {
            let Some((payload, fetched_at)) = self.store.cached_media(*id).ok().flatten()
            else {
                return false;
            };
            let Ok(media) = serde_json::from_str::<Media>(&payload) else { return false };
            fetched_at <= through
                && match media.next_airing_episode {
                    Some(next) => next.airing_at > now,
                    None => matches!(
                        media.status,
                        Some(MediaStatus::Finished | MediaStatus::Cancelled)
                    ),
                }
        })
    }

    /// Keep `notices` and how far the check looked, together.
    fn keep(&self, notices: &[EpisodeNotice], through: i64, now: i64) {
        if let Err(error) = self.store.keep_episode_check(notices, through, now) {
            tracing::warn!(%error, "could not keep the new-episode check");
        }
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::config::NetworkConfig;
    use anistream_meta::AniList;
    use anistream_store::WatchEvent;

    use super::*;

    const LISTED: AnilistId = AnilistId::new(900_002);
    const PLANNED: AnilistId = AnilistId::new(900_003);
    const DROPPED: AnilistId = AnilistId::new(900_004);
    const DONE: AnilistId = AnilistId::new(900_001);
    const WATCHED: AnilistId = AnilistId::new(900_006);
    const NOW: i64 = 1_800_000_000;

    fn watch(store: &Store, id: AnilistId, episode: u32, at: i64) {
        let event = WatchEvent {
            duration_secs: Some(1_440.0),
            completed: true,
            at,
            ..WatchEvent::new(id, episode.to_string(), 1_440.0)
        };
        store.record_event(&event).unwrap();
    }

    #[test]
    fn the_shows_followed_are_the_lists_and_what_was_watched_lately() {
        let store = Store::open_in_memory().unwrap();
        let list = [
            (LISTED, "CURRENT"),
            (PLANNED, "PLANNING"),
            (DROPPED, "DROPPED"),
            (DONE, "COMPLETED"),
        ];
        store.replace_library_statuses("anilist", &list, NOW).unwrap();
        watch(&store, DROPPED, 3, NOW - DAY);
        watch(&store, DONE, 12, NOW - 2 * DAY);
        watch(&store, WATCHED, 8, NOW - 3 * DAY);
        // Watched, but too long ago to count as following it.
        watch(&store, AnilistId::new(900_005), 1, NOW - WATCHED_WITHIN - DAY);

        // The list's being watched, then history's, a show dropped on the list left out
        // whatever history says. A show planned is followed only where planning counts.
        assert_eq!(followed(&store, true, NOW, false), [LISTED, DONE, WATCHED]);
        assert_eq!(followed(&store, true, NOW, true), [LISTED, PLANNED, DONE, WATCHED]);
        // Signed out, the list kept here is not the viewer's any more.
        assert_eq!(followed(&store, false, NOW, false), [DROPPED, DONE, WATCHED]);

        // A check leaves planned shows out, but takes in any show switched on for itself, and
        // leaves out any switched off.
        assert_eq!(notified(&store, true, NOW, true), [LISTED, DONE, WATCHED]);
        assert_eq!(notified(&store, true, NOW, false), [], "Settings' switch off, none on");
        let name = ShowSettingKey::NotifyNewEpisodes.name();
        store.set_show_setting(PLANNED, name, Some("true"), NOW).unwrap();
        store.set_show_setting(DROPPED, name, Some("true"), NOW).unwrap();
        store.set_show_setting(WATCHED, name, Some("false"), NOW).unwrap();
        assert_eq!(notified(&store, true, NOW, true), [LISTED, DONE, PLANNED, DROPPED]);
        // With Settings' switch off, a show switched on for itself is still followed, and only
        // those are.
        assert_eq!(notified(&store, true, NOW, false), [PLANNED, DROPPED]);
    }

    fn broadcast(id: AnilistId, episode: u32, airing_at: i64) -> AiringEntry {
        let media = serde_json::json!({
            "id": id.get(),
            "title": { "romaji": "Kumo no Ue no Toshokan", "english": "The Library Above the Clouds" },
            "coverImage": { "large": "https://img.example/library-poster.png" },
        });
        AiringEntry { episode, airing_at, media: serde_json::from_value(media).unwrap() }
    }

    /// A check on `store`, offline, with `registry` for the dubs.
    fn checking(store: &Store, registry: ProviderRegistry) -> Check {
        let http = anistream_net::HttpClient::offline(&NetworkConfig::default()).unwrap();
        Check {
            store: store.clone(),
            meta: Meta::new(AniList::new(http, 30), None, store.clone()),
            registry,
            signed_in: false,
            enabled: true,
            clock: Clock::default(),
            config: Config::default(),
        }
    }

    #[test]
    fn an_episode_is_said_once_after_its_show_was_followed_and_not_once_watched() {
        let store = Store::open_in_memory().unwrap();
        let check = checking(&store, ProviderRegistry::new(Vec::new()));
        let since = NOW - DAY;
        let notice =
            |id, episode| EpisodeNotice { anilist_id: id, episode, since, dubbed: None };
        let mut notices =
            HashMap::from([(LISTED, notice(LISTED, 5)), (WATCHED, notice(WATCHED, 0))]);
        watch(&store, WATCHED, 9, NOW - HOUR);
        let aired = [
            // Aired before the show was followed.
            broadcast(WATCHED, 8, since - HOUR),
            // Said already.
            broadcast(LISTED, 5, NOW - 2 * HOUR),
            broadcast(LISTED, 6, NOW - 2 * HOUR),
            // Watched here already, as it aired.
            broadcast(WATCHED, 9, NOW - 90 * MINUTE),
            // Not a show this check follows.
            broadcast(DONE, 3, NOW - HOUR),
            // Still to come, though AniList listed it.
            broadcast(LISTED, 7, NOW + MINUTE),
        ];

        let said = check.said(&aired, &mut notices, NOW);
        let said: Vec<_> = said.iter().map(|e| (e.id, e.episode, e.title.as_str())).collect();
        assert_eq!(said, [(LISTED, 6, "The Library Above the Clouds")]);
        // Each show's notice moved to the newest episode it has aired, said or not.
        assert_eq!(notices[&LISTED].episode, 6);
        assert_eq!(notices[&WATCHED].episode, 9);
        assert_eq!(check.said(&aired, &mut notices, NOW), [], "and none is said twice");
    }

    const DUBBED: AnilistId = AnilistId::new(900_007);

    /// The show watched dubbed, to episode 4 two days ago, and as the metadata cache keeps it:
    /// airing, its episode 7 a week after `now`, so the cache shows nothing more can have aired
    /// and AniList is not asked.
    fn dubbed_show(store: &Store, now: i64) {
        watch(store, DUBBED, 4, now - 2 * DAY);
        let media = serde_json::json!({
            "id": DUBBED.get(),
            "title": { "romaji": "Kaze no Tou no Ryouriban", "english": "The Cook of the Wind Tower" },
            "format": "TV",
            "status": "RELEASING",
            "episodes": 12,
            "seasonYear": 2026,
            "coverImage": { "large": "https://img.example/tower-poster.png" },
            "bannerImage": "https://img.example/tower-banner.jpg",
            "nextAiringEpisode": { "episode": 7, "airingAt": now + 7 * DAY, "timeUntilAiring": 7 * DAY },
        });
        store.cache_media(DUBBED, &media.to_string(), now - 2 * DAY).unwrap();
        let translation = ShowSettingKey::Translation.name();
        store.set_show_setting(DUBBED, translation, Some("dub"), now - 2 * DAY).unwrap();
    }

    /// A source that lists the show in the dub up to `dubbed`, and how many times it is asked.
    fn dub_source(dubbed: u32) -> (ProviderRegistry, Arc<std::sync::atomic::AtomicUsize>) {
        use anistream_core::{ids::ProviderKey, media::SearchHit};
        let key = ProviderKey::new("tower");
        let hit = SearchHit {
            key: key.clone(),
            title: "Kaze no Tou no Ryouriban".into(),
            synonyms: vec!["The Cook of the Wind Tower".into()],
            episode_count: Some(12),
            year: Some(2026),
            format: Some(anistream_core::media::MediaFormat::Tv),
        };
        let episodes = (1..=dubbed).map(Episode::new).collect();
        let source = anistream_providers::MockProvider::new("dubs")
            .with_hits(vec![hit])
            .with_episodes_for(key, episodes);
        let calls = source.call_count();
        (ProviderRegistry::new(vec![source.arc()]), calls)
    }

    /// The broadcast of episode 6 has moved the show on, with its dub said to episode 5.
    fn aired_ahead_of_its_dub(store: &Store, now: i64) {
        let notice = EpisodeNotice {
            anilist_id: DUBBED,
            episode: 6,
            since: now - 3 * DAY,
            dubbed: Some(5),
        };
        store.keep_episode_check(&[notice], now - DAY, now - DAY).unwrap();
    }

    #[test]
    fn a_broadcast_of_a_show_watched_dubbed_is_not_said_but_sets_where_its_dub_is_said_from() {
        let store = Store::open_in_memory().unwrap();
        dubbed_show(&store, NOW);
        let check = checking(&store, ProviderRegistry::new(Vec::new()));
        let since = NOW - 3 * DAY;
        let mut notices = HashMap::from([(
            DUBBED,
            EpisodeNotice { anilist_id: DUBBED, episode: 0, since, dubbed: None },
        )]);
        // Episode 5 aired before the show was followed; 6 after.
        let aired = [broadcast(DUBBED, 5, since - HOUR), broadcast(DUBBED, 6, NOW - HOUR)];
        assert_eq!(check.said(&aired, &mut notices, NOW), [], "not at the broadcast");
        let notice = notices[&DUBBED];
        assert_eq!((notice.episode, notice.dubbed), (6, Some(5)), "its dub said from 6 on");
    }

    #[tokio::test]
    async fn a_dub_is_said_once_the_source_lists_it_and_only_once() {
        let store = Store::open_in_memory().unwrap();
        let now = Clock::default().now();
        dubbed_show(&store, now);
        aired_ahead_of_its_dub(&store, now);

        // Episode 6 has aired, and the dub stops at 5: nothing to say yet.
        let (behind, asked) = dub_source(5);
        assert_eq!(checking(&store, behind).run().await, []);
        assert!(asked.load(std::sync::atomic::Ordering::SeqCst) > 0, "the dub was listed");
        let kept = store.episode_notices().unwrap();
        assert_eq!(kept[0].dubbed, Some(5), "and is listed again next time");

        // Once the source lists episode 6 in the dub, it is said, dubbed.
        let (out, asked) = dub_source(6);
        let said = checking(&store, out.clone()).run().await;
        let [episode] = &said[..] else { panic!("one episode: {said:?}") };
        assert_eq!(
            (episode.id, episode.episode, episode.translation),
            (DUBBED, 6, Some(Translation::Dub))
        );
        assert_eq!(episode.title, "The Cook of the Wind Tower");
        assert_eq!(episode.next_for_you, Some(5), "watched to episode 4");
        assert_eq!(episode.banner_url.as_deref(), Some("https://img.example/tower-banner.jpg"));
        assert!(episode.aired_at >= now, "when the check found it, which AniList cannot say");

        // And never again: caught up with the broadcast, the dub is not even listed.
        let listed = asked.load(std::sync::atomic::Ordering::SeqCst);
        assert_eq!(checking(&store, out).run().await, []);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), listed, "not listed");
    }

    #[tokio::test]
    async fn a_dub_already_watched_is_counted_and_not_said() {
        let store = Store::open_in_memory().unwrap();
        let now = Clock::default().now();
        dubbed_show(&store, now);
        aired_ahead_of_its_dub(&store, now);
        watch(&store, DUBBED, 6, now - HOUR);
        let (out, _) = dub_source(6);
        assert_eq!(checking(&store, out).run().await, []);
        assert_eq!(store.episode_notices().unwrap()[0].dubbed, Some(6));
    }

    #[tokio::test]
    async fn with_settings_off_only_a_show_switched_on_is_checked() {
        let store = Store::open_in_memory().unwrap();
        let now = Clock::default().now();
        dubbed_show(&store, now);
        aired_ahead_of_its_dub(&store, now);
        // Watched lately, and so followed while Settings' switch is on, but not while it is off.
        let (out, _) = dub_source(6);
        let off = Check { enabled: false, ..checking(&store, out.clone()) };
        assert_eq!(off.run().await, []);
        assert!(
            store.episode_notices().unwrap().is_empty(),
            "no longer followed, so forgotten"
        );

        // Switched on for itself, as if it had been when episode 6 aired, it is checked whatever
        // Settings says.
        let name = ShowSettingKey::NotifyNewEpisodes.name();
        store.set_show_setting(DUBBED, name, Some("true"), now).unwrap();
        aired_ahead_of_its_dub(&store, now);
        let off = Check { enabled: false, ..checking(&store, out) };
        let said: Vec<_> = off.run().await.into_iter().map(|e| (e.id, e.episode)).collect();
        assert_eq!(said, [(DUBBED, 6)]);
    }

    #[test]
    fn a_listing_reaches_its_highest_whole_episode() {
        let listed = |numbers: &[&str]| -> Vec<Episode> {
            numbers.iter().map(|n| Episode::new(*n)).collect()
        };
        assert_eq!(reach(&listed(&["1", "2", "12", "3"])), Some(12));
        assert_eq!(reach(&listed(&["5", "5.5"])), Some(5));
        assert_eq!(reach(&listed(&["OVA", "Special"])), None);
        assert_eq!(reach(&[]), None);
    }

    const MINUTE: i64 = 60;
}
