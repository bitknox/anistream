//! The home-screen widgets' record: Continue's first titles and today's broadcasts, for a
//! native app to write where its widgets read it. See `Command::WidgetSnapshot`.
//!
//! Made from what Continue and the calendar already compute: Continue's titles from history,
//! each one's progress as Continue counts it, and its title and cover from the metadata cache
//! where the cache has it; and the broadcasts of the device's day, as the calendar fetches a
//! window of them. A background task can ask for it, so it asks AniList as little as it can.

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anistream_core::ids::AnilistId;
use anistream_meta::{Media, Meta, anilist::AiringEntry};
use anistream_model::{
    event::Event,
    widgets::{WIDGET_DATA_VERSION, WidgetAiring, WidgetContinue, WidgetData},
};
use anistream_store::Store;

use crate::{
    clock::Clock, data, new_episodes, outbox::Outbox, services::Services, state::EngineState,
};

/// How many titles to continue the widgets are given.
const CONTINUE_ROWS: u32 = 4;

/// How long today's broadcasts, once fetched, answer for: AniList's answers are used again for as
/// long, `anistream_meta::source::REUSE_FOR`.
const TODAY_FOR: Duration = anistream_meta::source::REUSE_FOR;

/// The longest either half may take, inside a background task's time.
const ASK_FOR: Duration = Duration::from_secs(15);

/// Today's broadcasts as the widgets' record last fetched them, answered again for
/// [`TODAY_FOR`] while it is still the same day. Cheap to clone; clones share it.
#[derive(Debug, Clone, Default)]
pub struct Today(Arc<Mutex<Option<Fetched>>>);

#[derive(Debug)]
struct Fetched {
    /// The start of the day they are of.
    day: i64,
    at: Instant,
    rows: Vec<AiringEntry>,
}

impl Today {
    /// What was fetched for `day`, when that was less than [`TODAY_FOR`] ago.
    fn recent(&self, day: i64) -> Option<Vec<AiringEntry>> {
        let fetched = self.0.lock().ok()?;
        let fetched = fetched.as_ref()?;
        (fetched.day == day && fetched.at.elapsed() < TODAY_FOR).then(|| fetched.rows.clone())
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
    let make = Make {
        store: services.store.clone(),
        meta: services.meta.clone(),
        signed_in: new_episodes::list_signed_in(services),
        clock: services.clock.clone(),
        today: engine.airing_today.clone(),
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
    signed_in: bool,
    clock: Clock,
    today: Today,
}

impl Make {
    async fn run(&self) -> WidgetData {
        let now = self.clock.now();
        let (continue_watching, airing_today) =
            tokio::join!(self.continuing(), self.airing_today(now));
        WidgetData {
            version: WIDGET_DATA_VERSION,
            generated_at: now,
            continue_watching,
            airing_today,
        }
    }

    /// Continue's first titles, most recently watched first, as Continue counts where each
    /// stands. Titles and covers from the cache, and only the titles it lacks asked for.
    async fn continuing(&self) -> Vec<WidgetContinue> {
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
        let titles = self.meta.titles();
        ids.iter()
            .filter_map(|id| media.get(id))
            .map(|media| {
                let entry = data::entry_from(media, Some(&self.store));
                WidgetContinue {
                    id: media.id,
                    title: titles.pick(&media.title).to_owned(),
                    episode: entry.progress.map_or(1, |progress| progress.next),
                    watched: entry.progress.map_or(0, |progress| progress.done),
                    episodes: entry.episodes,
                    fraction: entry.resume.and_then(|resume| resume.fraction),
                    cover_url: entry.cover_url,
                }
            })
            .collect()
    }

    /// A title as the cache last had it, from any fetch that wrote it.
    fn cached(&self, id: AnilistId) -> Option<Media> {
        let (payload, _) = self.store.cached_media(id).ok().flatten()?;
        serde_json::from_str(&payload).ok()
    }

    /// The broadcasts of the device's day, aired or not, in the order they air, each marked by
    /// whether its show is followed. One page of the calendar's window, the day's whole.
    async fn airing_today(&self, now: i64) -> Vec<WidgetAiring> {
        let (day, next_day) = day_of(now);
        let rows = match self.today.recent(day) {
            Some(rows) => rows,
            None => {
                // `airingAt_greater` is strict, and a broadcast at midnight is the new day's.
                let asked = self.meta.airing_between_sorted(day - 1, next_day, 1, 50, false);
                match tokio::time::timeout(ASK_FOR, asked).await {
                    Ok(Ok(page)) => {
                        self.today.keep(day, &page.items);
                        page.items
                    }
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "today's broadcasts were not found");
                        return Vec::new();
                    }
                    Err(_) => {
                        tracing::warn!("gave up waiting for today's broadcasts");
                        return Vec::new();
                    }
                }
            }
        };
        let followed: HashSet<AnilistId> =
            new_episodes::followed(&self.store, self.signed_in, now).into_iter().collect();
        let titles = self.meta.titles();
        rows.iter()
            .map(|row| WidgetAiring {
                id: row.media.id,
                title: titles.pick(&row.media.title).to_owned(),
                episode: row.episode,
                airs_at: row.airing_at,
                on_list: followed.contains(&row.media.id),
                cover_url: row.media.cover_image.best().map(str::to_owned),
            })
            .collect()
    }
}

/// The start of the day `now` is in, in the device's time zone as the system has it, and of
/// the next: a day of 23 or 25 hours where the clocks change. A day from `now` when the zone
/// cannot be read.
fn day_of(now: i64) -> (i64, i64) {
    let zone = jiff::tz::TimeZone::system();
    let start = |date: jiff::civil::Date| {
        date.to_zoned(zone.clone()).ok().map(|zoned| zoned.timestamp().as_second())
    };
    let bounds = jiff::Timestamp::from_second(now).ok().and_then(|at| {
        let today = at.to_zoned(zone.clone()).date();
        Some((start(today)?, start(today.tomorrow().ok()?)?))
    });
    bounds.unwrap_or((now, now + 24 * 3_600))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_is_the_zones_from_midnight_to_midnight() {
        // 2026-10-14 12:00:30 UTC, the fixtures' moment.
        let now = 1_791_979_230;
        let (start, end) = day_of(now);
        assert!(start <= now && now < end, "{start} {now} {end}");
        assert!((23 * 3_600..=25 * 3_600).contains(&(end - start)), "{}", end - start);
    }
}
