//! The new-episode check: which episodes of the shows followed aired since they were last looked
//! at, each said once, for a native app to notify of. See `Command::CheckNewEpisodes`.
//!
//! What the check keeps between runs is in the database (`0009_episode_notices`): for each show,
//! the newest episode said and when the show was first followed, and how far the check has
//! looked. A show is first followed silently, from the moment it is, which is all a first check
//! does: nothing that aired before it is ever said, so turning notifications on, or adding a
//! show to the list, never fires a backlog.
//!
//! It is written for a background task's few seconds, on an engine that does nothing else:
//!
//! - the shows followed are read from the database, the list as its last pull left it, and
//!   history, so no tracker is asked;
//! - AniList is asked once at most, for every show at once, and not at all when the last look
//!   was minutes ago or the metadata cache shows nothing can have aired since;
//! - the ask is cut off at [`ASK_FOR`], and a check cut off says nothing and keeps its place, so
//!   the next one looks again from there.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use anistream_core::{ids::AnilistId, media::MediaStatus};
use anistream_meta::{Media, Meta, anilist::AiringEntry, titles::Titles};
use anistream_model::{event::Event, notifications::NewEpisode, show::ShowSettingKey};
use anistream_store::{EpisodeNotice, Store};

use crate::{clock::Clock, outbox::Outbox, services::Services, state::EngineState};

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

/// The tracker statuses of a show followed: watching, rewatching or planning.
const FOLLOWED: [&str; 3] = ["CURRENT", "REPEATING", "PLANNING"];

/// The statuses that say a show is not being watched, whatever history says.
const SET_ASIDE: [&str; 2] = ["DROPPED", "PAUSED"];

/// Run a check, off the actor, and say what it found as [`Event::NewEpisodes`]: once, empty
/// when there is nothing to say. With notifications off it asks nothing, and forgets what it
/// kept, so the first check after they are turned on again is a first check.
pub(crate) fn check(engine: &EngineState, services: &Services, tx: &Outbox) {
    let on = engine.config.effective().notifications.episodes;
    let check = Check {
        store: services.store.clone(),
        meta: services.meta.clone(),
        signed_in: list_signed_in(services),
        clock: services.clock.clone(),
    };
    let (turn, tx) = (Arc::clone(&engine.episode_check), tx.clone());
    tokio::spawn(async move {
        let _turn = turn.lock().await;
        let found = if on {
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

/// The shows followed, as of `now`: on the list as watching, rewatching or planning, when the
/// list is a signed-in tracker's, and watched here in the last [`WATCHED_WITHIN`], unless the
/// list has them dropped or paused. The list's first, then history's, each once.
pub(crate) fn followed(store: &Store, signed_in: bool, now: i64) -> Vec<AnilistId> {
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
        .filter(|(_, status)| FOLLOWED.contains(&status.as_str()))
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

/// What one check works with.
struct Check {
    store: Store,
    meta: Meta,
    signed_in: bool,
    clock: Clock,
}

impl Check {
    /// What aired since the last look, for the shows followed and not switched off: see the
    /// module's docs. What it finds is kept before it is said.
    async fn run(&self) -> Vec<NewEpisode> {
        let now = self.clock.now();
        let off: HashSet<AnilistId> = self
            .store
            .shows_setting(ShowSettingKey::NotifyNewEpisodes.name(), "false")
            .unwrap_or_default()
            .into_iter()
            .collect();
        let shows: Vec<AnilistId> = followed(&self.store, self.signed_in, now)
            .into_iter()
            .filter(|id| !off.contains(id))
            .collect();

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
            .map(|id| EpisodeNotice { anilist_id: *id, episode: 0, since: now })
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
        if ids.is_empty() || now - through < REUSE_FOR || self.quiet(&ids, through, now) {
            self.keep(&fresh, through, now);
            return Vec::new();
        }

        let from = (through - SLACK).max(now - LOOKBACK);
        // `airingAt_lesser` is strict, and an episode airing this very second has aired.
        let asked = tokio::time::timeout(ASK_FOR, self.meta.aired_since(&ids, from, now + 1));
        let page = match asked.await {
            Ok(Ok(page)) => page,
            // Not said and not kept: the next check looks again from the same place.
            Ok(Err(error)) => {
                tracing::warn!(%error, "the new-episode check could not ask AniList");
                self.keep(&fresh, through, now);
                return Vec::new();
            }
            Err(_) => {
                tracing::warn!("the new-episode check gave up waiting for AniList");
                self.keep(&fresh, through, now);
                return Vec::new();
            }
        };

        let mut notices: HashMap<AnilistId, EpisodeNotice> =
            following.iter().map(|notice| (notice.anilist_id, *notice)).collect();
        let said = self.said(&page.items, &mut notices, now);
        // A full page is carried on from its last broadcast, so the ones after it are looked at
        // next time. A page that never got past the last look is let go, so a check can never
        // be held on the same page.
        let reached = match page.items.last() {
            Some(last) if page.has_next && last.airing_at > through => last.airing_at,
            _ => now,
        };
        let changed: Vec<EpisodeNotice> = notices
            .into_values()
            .filter(|notice| !following.contains(notice))
            .chain(fresh)
            .collect();
        self.keep(&changed, reached, now);
        said
    }

    /// The episodes among `aired`, oldest first, that are new for their show: aired by `now`,
    /// after the show was first followed, and after the newest one said of it. Each moves its
    /// show's notice on, said or not: one already watched here is counted and not said.
    fn said(
        &self,
        aired: &[AiringEntry],
        notices: &mut HashMap<AnilistId, EpisodeNotice>,
        now: i64,
    ) -> Vec<NewEpisode> {
        let titles: &Titles = self.meta.titles();
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
            notice.episode = row.episode;
            if self.watched(id, row.episode) {
                continue;
            }
            said.push(NewEpisode {
                id,
                title: titles.pick(&row.media.title).to_owned(),
                episode: row.episode,
                aired_at: row.airing_at,
                cover_url: row.media.cover_image.best().map(str::to_owned),
            });
        }
        said
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

        // The list's, then history's, a show dropped on the list left out whatever history says.
        assert_eq!(followed(&store, true, NOW), [LISTED, PLANNED, DONE, WATCHED]);
        // Signed out, the list kept here is not the viewer's any more.
        assert_eq!(followed(&store, false, NOW), [DROPPED, DONE, WATCHED]);
    }

    fn broadcast(id: AnilistId, episode: u32, airing_at: i64) -> AiringEntry {
        let media = serde_json::json!({
            "id": id.get(),
            "title": { "romaji": "Kumo no Ue no Toshokan", "english": "The Library Above the Clouds" },
            "coverImage": { "large": "https://img.example/library-poster.png" },
        });
        AiringEntry { episode, airing_at, media: serde_json::from_value(media).unwrap() }
    }

    #[test]
    fn an_episode_is_said_once_after_its_show_was_followed_and_not_once_watched() {
        let store = Store::open_in_memory().unwrap();
        let http = anistream_net::HttpClient::offline(&NetworkConfig::default()).unwrap();
        let meta = Meta::new(AniList::new(http, 30), None, store.clone());
        let check =
            Check { store: store.clone(), meta, signed_in: false, clock: Clock::default() };
        let since = NOW - DAY;
        let mut notices = HashMap::from([
            (LISTED, EpisodeNotice { anilist_id: LISTED, episode: 5, since }),
            (WATCHED, EpisodeNotice { anilist_id: WATCHED, episode: 0, since }),
        ]);
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

    const MINUTE: i64 = 60;
}
