//! The notifications' week: the broadcasts of the next seven days that will notify, and those a
//! show's own switch leaves out, for a native app that schedules its notifications ahead to fire
//! at air time, and lists them in its Notifications pane. See `Command::UpcomingNotices`.
//!
//! The shows are those the new-episode check follows (`crate::new_episodes`): being watched, or
//! switched on for themselves, and not switched off. The broadcasts are the week the widgets'
//! record fetches (`crate::widgets::week`), fetched once for both.

use std::collections::{HashMap, HashSet};

use anistream_core::ids::AnilistId;
use anistream_meta::{anilist::AiringEntry, titles::Titles};
use anistream_model::{
    event::Event,
    notifications::{NoticeReason, UpcomingNotice, UpcomingNotices},
    show::ShowSettingKey,
};
use anistream_store::Store;

use crate::{data, new_episodes, outbox::Outbox, services::Services, state::EngineState};

/// Make the week, off the actor, and say it as [`Event::UpcomingNotices`]: once, with only the
/// shows' switches and none of the week when AniList could not be asked.
pub(crate) fn answer(engine: &EngineState, services: &Services, tx: &Outbox) {
    let enabled = engine.config.effective().notifications.episodes;
    let signed_in = new_episodes::list_signed_in(services);
    let (store, meta, clock) =
        (services.store.clone(), services.meta.clone(), services.clock.clone());
    let schedule = engine.widget_schedule.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        let now = clock.now();
        let (broadcasts, end) =
            crate::widgets::week(store.clone(), meta.clone(), clock, schedule, now).await;
        let week = Week { store: &store, titles: meta.titles(), signed_in, now, end };
        let _ = tx.send(Event::UpcomingNotices(week.notices(enabled, &broadcasts)));
    });
}

/// What the week is made from.
struct Week<'a> {
    store: &'a Store,
    titles: &'a Titles,
    /// Whether the Library's tracker is signed in, whose list counts.
    signed_in: bool,
    now: i64,
    /// The end of the week: the start of the day a week from today.
    end: i64,
}

impl Week<'_> {
    fn notices(&self, enabled: bool, broadcasts: &[AiringEntry]) -> UpcomingNotices {
        let name = ShowSettingKey::NotifyNewEpisodes.name();
        let switched = |value| -> HashSet<AnilistId> {
            self.store
                .shows_setting(name, value)
                .unwrap_or_else(|error| {
                    tracing::warn!(%error, "could not read the shows' own notification switches");
                    Vec::new()
                })
                .into_iter()
                .collect()
        };
        let (on, off) = (switched("true"), switched("false"));
        let watching = new_episodes::followed(self.store, self.signed_in, self.now, false);
        let followed: HashSet<AnilistId> = watching.iter().copied().collect();

        let mut next: HashMap<AnilistId, u32> = HashMap::new();
        let airings = broadcasts
            .iter()
            .filter(|row| self.now < row.airing_at && row.airing_at < self.end)
            .filter_map(|row| {
                let id = row.media.id;
                let reason = if off.contains(&id) {
                    NoticeReason::SwitchedOff
                } else if on.contains(&id) {
                    NoticeReason::SwitchedOn
                } else if followed.contains(&id) {
                    NoticeReason::Watching
                } else {
                    return None;
                };
                let next_to_watch = *next.entry(id).or_insert_with(|| {
                    let (progress, _) = data::standing(self.store, id, row.media.episodes);
                    progress.map_or(1, |progress| progress.next)
                });
                // Watched here already, ahead of the broadcast: nothing to say of it.
                (row.episode >= next_to_watch).then(|| UpcomingNotice {
                    id,
                    title: self.titles.pick(&row.media.title).to_owned(),
                    episode: row.episode,
                    airs_at: row.airing_at,
                    reason,
                    next_to_watch,
                    cover_url: row.media.cover_image.best().map(str::to_owned),
                    banner_url: row.media.banner_image.clone(),
                })
            })
            .collect();
        UpcomingNotices {
            enabled,
            generated_at: self.now,
            airings,
            watching,
            overrides: (on.len() + off.len()) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use anistream_store::WatchEvent;

    use super::*;

    const WATCHED: AnilistId = AnilistId::new(900_002);
    const PLANNED: AnilistId = AnilistId::new(900_003);
    const SWITCHED_OFF: AnilistId = AnilistId::new(900_004);
    const STRANGER: AnilistId = AnilistId::new(900_005);
    const NOW: i64 = 1_800_000_000;
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;

    fn broadcast(id: AnilistId, episode: u32, airing_at: i64) -> AiringEntry {
        let media = serde_json::json!({
            "id": id.get(),
            "episodes": 12,
            "title": { "romaji": format!("Show {}", id.get()), "english": format!("The Show {}", id.get()) },
            "coverImage": { "large": format!("https://img.example/{}.png", id.get()) },
            "bannerImage": (id == WATCHED).then(|| format!("https://img.example/{}-banner.jpg", id.get())),
        });
        AiringEntry { episode, airing_at, media: serde_json::from_value(media).unwrap() }
    }

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
    fn the_week_holds_what_notifies_and_what_its_switch_leaves_out() {
        let store = Store::open_in_memory().unwrap();
        let list = [(WATCHED, "CURRENT"), (PLANNED, "PLANNING"), (SWITCHED_OFF, "CURRENT")];
        store.replace_library_statuses("anilist", &list, NOW).unwrap();
        // Episodes 1 to 3 watched, so 4 is next: behind when 6 airs.
        for episode in 1..=3 {
            watch(&store, WATCHED, episode, NOW - DAY);
        }
        let name = ShowSettingKey::NotifyNewEpisodes.name();
        store.set_show_setting(PLANNED, name, Some("true"), NOW).unwrap();
        store.set_show_setting(SWITCHED_OFF, name, Some("false"), NOW).unwrap();

        let titles = Titles::default();
        let week = Week {
            store: &store,
            titles: &titles,
            signed_in: true,
            now: NOW,
            end: NOW + 7 * DAY,
        };
        let broadcasts = [
            // Aired already: not the week's to schedule.
            broadcast(WATCHED, 5, NOW - HOUR),
            broadcast(WATCHED, 6, NOW + 2 * HOUR),
            broadcast(PLANNED, 2, NOW + DAY),
            broadcast(SWITCHED_OFF, 9, NOW + 2 * DAY),
            // Not followed at all.
            broadcast(STRANGER, 1, NOW + 3 * DAY),
            // Past the week.
            broadcast(WATCHED, 7, NOW + 7 * DAY + HOUR),
        ];
        let notices = week.notices(false, &broadcasts);

        assert!(!notices.enabled);
        assert_eq!(notices.generated_at, NOW);
        let rows: Vec<_> = notices
            .airings
            .iter()
            .map(|row| (row.id, row.episode, row.reason, row.next_to_watch))
            .collect();
        assert_eq!(
            rows,
            [
                (WATCHED, 6, NoticeReason::Watching, 4),
                (PLANNED, 2, NoticeReason::SwitchedOn, 1),
                (SWITCHED_OFF, 9, NoticeReason::SwitchedOff, 1),
            ]
        );
        let first = &notices.airings[0];
        assert_eq!(first.title, "The Show 900002", "in the form the title setting picks");
        assert_eq!(first.airs_at, NOW + 2 * HOUR);
        assert_eq!(first.cover_url.as_deref(), Some("https://img.example/900002.png"));
        assert_eq!(first.banner_url.as_deref(), Some("https://img.example/900002-banner.jpg"));
        // Being watched: the list's, a show switched off among them, and not a planned one.
        assert_eq!(notices.watching, [WATCHED, SWITCHED_OFF]);
        assert_eq!(notices.overrides, 2);
    }

    #[test]
    fn an_episode_watched_ahead_of_its_broadcast_is_left_out() {
        let store = Store::open_in_memory().unwrap();
        for episode in 1..=6 {
            watch(&store, WATCHED, episode, NOW - HOUR);
        }
        let titles = Titles::default();
        let week = Week {
            store: &store,
            titles: &titles,
            signed_in: false,
            now: NOW,
            end: NOW + 7 * DAY,
        };
        let broadcasts = [broadcast(WATCHED, 6, NOW + HOUR), broadcast(WATCHED, 7, NOW + DAY)];
        let notices = week.notices(true, &broadcasts);
        let rows: Vec<_> =
            notices.airings.iter().map(|row| (row.episode, row.next_to_watch)).collect();
        assert_eq!(rows, [(7, 7)], "watched here lately, so followed signed out too");
        assert_eq!(notices.watching, [WATCHED]);
    }
}
