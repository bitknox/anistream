//! The home-screen widgets' record: Continue's first titles and the week's broadcasts, for a
//! native app to write where its widgets read it. See `Command::WidgetSnapshot`.
//!
//! Made from what Continue and the calendar already compute: Continue's titles from history,
//! each one's progress and resume point as Continue counts them, and its title and art from the
//! metadata cache where the cache has it; and the broadcasts from the start of the device's day to
//! a week on, as the calendar fetches a window of them. A background task can ask for it, so it
//! asks AniList as little as it can.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anistream_core::ids::AnilistId;
use anistream_meta::{Media, Meta, anilist::AiringEntry};
use anistream_model::{
    event::Event,
    widgets::{
        WIDGET_DATA_VERSION, WidgetAiring, WidgetContinue, WidgetData, WidgetNextAiring,
    },
};
use anistream_store::Store;

use crate::{
    clock::Clock, data, new_episodes, outbox::Outbox, services::Services, state::EngineState,
};

/// How many titles to continue the widgets are given.
const CONTINUE_ROWS: u32 = 5;

/// How long the week's broadcasts, once fetched, answer for: AniList's answers are used again
/// for as long, `anistream_meta::source::REUSE_FOR`.
const SCHEDULE_FOR: Duration = anistream_meta::source::REUSE_FOR;

/// Pages of broadcasts at most, of fifty each: a week has around 130.
const SCHEDULE_PAGES: u32 = 4;

/// The longest either half may take, inside a background task's time.
const ASK_FOR: Duration = Duration::from_secs(15);

/// The week's broadcasts as the widgets' record last fetched them, answered again for
/// [`SCHEDULE_FOR`] while it is still the same day. Cheap to clone; clones share it.
#[derive(Debug, Clone, Default)]
pub struct Schedule(Arc<Mutex<Option<Fetched>>>);

#[derive(Debug)]
struct Fetched {
    /// The start of the day the week starts on.
    day: i64,
    at: Instant,
    rows: Vec<AiringEntry>,
}

impl Schedule {
    /// What was fetched for the week from `day`, when that was less than [`SCHEDULE_FOR`] ago.
    fn recent(&self, day: i64) -> Option<Vec<AiringEntry>> {
        let fetched = self.0.lock().ok()?;
        let fetched = fetched.as_ref()?;
        (fetched.day == day && fetched.at.elapsed() < SCHEDULE_FOR)
            .then(|| fetched.rows.clone())
    }

    fn keep(&self, day: i64, rows: &[AiringEntry]) {
        if let Ok(mut fetched) = self.0.lock() {
            *fetched = Some(Fetched { day, at: Instant::now(), rows: rows.to_vec() });
        }
    }
}

/// Make the record, off the actor, and say it as [`Event::WidgetSnapshot`]: once, with what
/// could not be fetched left out.
pub(crate) fn snapshot(engine: &EngineState, services: &Services, tx: &Outbox) {
    let tracker = services
        .sync
        .trackers
        .first()
        .filter(|tracker| tracker.is_authenticated())
        .map(|tracker| tracker.id().to_owned());
    let make = Make {
        store: services.store.clone(),
        meta: services.meta.clone(),
        tracker,
        clock: services.clock.clone(),
        schedule: engine.widget_schedule.clone(),
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let _ = tx.send(Event::WidgetSnapshot(make.run().await));
    });
}

/// What making the record works with.
struct Make {
    store: Store,
    meta: Meta,
    /// The Library's tracker, when it is signed in.
    tracker: Option<String>,
    clock: Clock,
    schedule: Schedule,
}

impl Make {
    async fn run(&self) -> WidgetData {
        let now = self.clock.now();
        let days = Days::of(now);
        let (continue_watching, broadcasts) =
            tokio::join!(self.continuing(now), self.broadcasts(&days));
        let followed: HashSet<AnilistId> =
            new_episodes::followed(&self.store, self.tracker.is_some(), now, true)
                .into_iter()
                .collect();
        let rows = self.airing(&broadcasts, &followed);
        let next_airing = self.next_airing(&rows, &followed, now);
        let (airing_today, next_days) =
            rows.into_iter().partition(|row| row.airs_at < days.tomorrow);
        WidgetData {
            version: WIDGET_DATA_VERSION,
            generated_at: now,
            continue_watching,
            airing_today,
            signed_in: self.tracker.is_some(),
            tracker: self.tracker.clone(),
            next_airing,
            next_days,
        }
    }

    /// Continue's first titles, most recently watched first, as Continue counts where each
    /// stands. Titles and art from the cache, and only the titles it lacks asked for.
    async fn continuing(&self, now: i64) -> Vec<WidgetContinue> {
        let ids: Vec<AnilistId> = self
            .store
            .continue_list(CONTINUE_ROWS)
            .unwrap_or_default()
            .into_iter()
            .map(|progress| progress.anilist_id)
            .collect();
        let mut media: HashMap<AnilistId, Media> =
            ids.iter().filter_map(|id| Some((*id, self.cached(*id)?))).collect();
        let missing: Vec<AnilistId> =
            ids.iter().filter(|id| !media.contains_key(id)).copied().collect();
        if !missing.is_empty() {
            match tokio::time::timeout(ASK_FOR, self.meta.media_many(&missing)).await {
                Ok(Ok(found)) => media.extend(found.into_iter().map(|m| (m.id, m))),
                Ok(Err(error)) => tracing::warn!(%error, "the widgets' titles were not found"),
                Err(_) => tracing::warn!("gave up waiting for the widgets' titles"),
            }
        }
        ids.iter()
            .filter_map(|id| media.get(id))
            .map(|media| self.to_continue(media, now))
            .collect()
    }

    /// One title to continue, as Continue has it: the episode it starts, where that was left
    /// and how much of it is left, and when the next one airs.
    fn to_continue(&self, media: &Media, now: i64) -> WidgetContinue {
        let entry = data::entry_from(media, Some(&self.store));
        let episode = entry.progress.map_or(1, |progress| progress.next);
        let position = entry.resume.map(|resume| resume.position);
        let duration = self.length(media, episode);
        let left = duration.map(|duration| (duration - position.unwrap_or(0.0)).max(0.0));
        let next = media.next_airing_episode.filter(|next| next.airing_at > now);
        WidgetContinue {
            id: media.id,
            title: self.meta.titles().pick(&media.title).to_owned(),
            episode,
            watched: entry.progress.map_or(0, |progress| progress.done),
            episodes: entry.episodes,
            fraction: entry.resume.and_then(|resume| resume.fraction),
            cover_url: entry.cover_url,
            episode_title: media.episode_titles().remove(&episode),
            resume: position.is_some(),
            position_secs: position.map(|secs| secs.round() as u32),
            duration_secs: duration.map(|secs| secs.round() as u32),
            minutes_left: left.map(|secs| (secs / 60.0).ceil() as u32),
            next_airing: next.map(|next| next.airing_at),
            next_airing_episode: next.map(|next| next.episode),
            banner_url: media.banner_image.clone(),
            cover_color: media.cover_image.color.clone(),
        }
    }

    /// How long `episode` of `media` is, in seconds: as its player last found it, or AniList's
    /// length for the title's episodes.
    fn length(&self, media: &Media, episode: u32) -> Option<f64> {
        let episode = episode.to_string();
        let played = self.store.events_for(media.id, 50).ok().and_then(|events| {
            events.into_iter().find(|e| e.episode == episode).and_then(|e| e.duration_secs)
        });
        played
            .filter(|secs| *secs > 0.0)
            .or(media.duration.map(|minutes| f64::from(minutes) * 60.0))
    }

    /// A title as the cache last had it, from any fetch that wrote it.
    fn cached(&self, id: AnilistId) -> Option<Media> {
        let (payload, _) = self.store.cached_media(id).ok().flatten()?;
        serde_json::from_str(&payload).ok()
    }

    /// The broadcasts from the start of today to a week on, aired or not, oldest first: the
    /// calendar's window, a few pages of it, used again for a few minutes.
    async fn broadcasts(&self, days: &Days) -> Vec<AiringEntry> {
        if let Some(rows) = self.schedule.recent(days.today) {
            return rows;
        }
        let mut rows = Vec::new();
        let walked = tokio::time::timeout(ASK_FOR, async {
            for page in 1..=SCHEDULE_PAGES {
                // `airingAt_greater` is strict, and a broadcast at midnight is the new day's.
                let asked = self.meta.airing_between_sorted(
                    days.today - 1,
                    days.week_end,
                    page,
                    50,
                    false,
                );
                let fetched = asked.await?;
                rows.extend(fetched.items);
                if !fetched.has_next {
                    break;
                }
            }
            Ok::<_, anistream_meta::source::MetaError>(())
        })
        .await;
        match walked {
            // Kept only whole, so a week cut short is asked for again next time.
            Ok(Ok(())) => self.schedule.keep(days.today, &rows),
            Ok(Err(error)) => {
                tracing::warn!(%error, "the week's broadcasts were not all found")
            }
            Err(_) => tracing::warn!("gave up waiting for the week's broadcasts"),
        }
        rows
    }

    /// The broadcasts as the widgets show them, each marked by whether its show is followed and
    /// whether the episode has been watched here.
    fn airing(
        &self,
        broadcasts: &[AiringEntry],
        followed: &HashSet<AnilistId>,
    ) -> Vec<WidgetAiring> {
        let titles = self.meta.titles();
        let mut watched: HashMap<AnilistId, Vec<String>> = HashMap::new();
        broadcasts
            .iter()
            .map(|row| {
                let id = row.media.id;
                let done = watched
                    .entry(id)
                    .or_insert_with(|| self.store.completed_episodes(id).unwrap_or_default());
                WidgetAiring {
                    id,
                    title: titles.pick(&row.media.title).to_owned(),
                    episode: row.episode,
                    airs_at: row.airing_at,
                    on_list: followed.contains(&id),
                    cover_url: row.media.cover_image.best().map(str::to_owned),
                    watched: done.iter().any(|e| e.trim().parse::<u32>() == Ok(row.episode)),
                    translation: None,
                    banner_url: row.media.banner_image.clone(),
                    cover_color: row.media.cover_image.color.clone(),
                }
            })
            .collect()
    }

    /// The soonest broadcast still to come of a show followed: in the week's, or past them the
    /// soonest next episode the metadata cache has for one.
    fn next_airing(
        &self,
        rows: &[WidgetAiring],
        followed: &HashSet<AnilistId>,
        now: i64,
    ) -> Option<WidgetNextAiring> {
        let this_week = rows
            .iter()
            .filter(|row| row.on_list && row.airs_at > now)
            .min_by_key(|row| row.airs_at);
        if let Some(row) = this_week {
            return Some(WidgetNextAiring {
                id: row.id,
                title: row.title.clone(),
                episode: row.episode,
                airs_at: row.airs_at,
            });
        }
        followed
            .iter()
            .filter_map(|id| {
                let media = self.cached(*id)?;
                let next = media.next_airing_episode.filter(|next| next.airing_at > now)?;
                Some(WidgetNextAiring {
                    id: *id,
                    title: self.meta.titles().pick(&media.title).to_owned(),
                    episode: next.episode,
                    airs_at: next.airing_at,
                })
            })
            .min_by_key(|next| (next.airs_at, next.id.get()))
    }
}

/// The days the record is of, in the device's time zone as the system has it: the start of
/// today, of tomorrow, and of the day a week on. A day is 23 or 25 hours where the clocks change,
/// and 24 hours from `now` when the zone cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Days {
    today: i64,
    tomorrow: i64,
    week_end: i64,
}

impl Days {
    fn of(now: i64) -> Self {
        let zone = jiff::tz::TimeZone::system();
        let start = |date: jiff::civil::Date| {
            date.to_zoned(zone.clone()).ok().map(|zoned| zoned.timestamp().as_second())
        };
        let bounds = jiff::Timestamp::from_second(now).ok().and_then(|at| {
            let today = at.to_zoned(zone.clone()).date();
            let mut week_end = today;
            for _ in 0..7 {
                week_end = week_end.tomorrow().ok()?;
            }
            Some(Self {
                today: start(today)?,
                tomorrow: start(today.tomorrow().ok()?)?,
                week_end: start(week_end)?,
            })
        });
        bounds.unwrap_or(Self { today: now, tomorrow: now + DAY, week_end: now + 7 * DAY })
    }
}

const DAY: i64 = 24 * 3_600;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_days_are_the_zones_from_midnight_to_midnight() {
        // 2026-10-14 12:00:30 UTC, the fixtures' moment.
        let now = 1_791_979_230;
        let days = Days::of(now);
        assert!(days.today <= now && now < days.tomorrow, "{days:?}");
        let hours = |secs: i64| secs as f64 / 3_600.0;
        assert!((23.0..=25.0).contains(&hours(days.tomorrow - days.today)), "{days:?}");
        assert!((167.0..=169.0).contains(&hours(days.week_end - days.today)), "{days:?}");
    }
}
