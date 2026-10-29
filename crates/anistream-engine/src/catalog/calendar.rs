//! The calendar: one timeline of what recently aired and what is coming, the pages further back
//! that a scroll toward its top asks for, and the pages further ahead that a step forward asks
//! for.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anistream_core::ids::AnilistId;
use anistream_meta::{Meta, anilist::AiringEntry};
use anistream_model::{
    catalog::{AiringStatus, ContentFor, ContentList, Entry},
    event::Event,
};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{
    clock::Clock,
    data,
    dispatch::Lent,
    now_epoch,
    titles::dub::{Dubs, route},
};

/// One page of broadcasts as AniList answered it.
#[derive(Debug, Clone)]
struct Block {
    /// In the order they were fetched: newest first for the past, oldest first for the future.
    items: Vec<AiringEntry>,
    has_next: bool,
}

/// The pages the calendar has loaded, numbered against one anchor.
#[derive(Debug, Clone, Default)]
struct Window {
    anchor: i64,
    past: BTreeMap<u32, Block>,
    ahead: BTreeMap<u32, Block>,
}

impl Window {
    /// How many pages from the first are held without a gap, and whether AniList has more
    /// behind them. With none held there is more to find.
    fn run(pages: &BTreeMap<u32, Block>) -> (u32, bool) {
        let mut count = 0;
        let mut more = true;
        for (page, block) in pages {
            if *page != count + 1 {
                break;
            }
            count = *page;
            more = block.has_next;
        }
        (count, more)
    }

    /// The window the front end is told of.
    fn event(&self) -> Event {
        let (pages, more_past) = Self::run(&self.past);
        let (ahead_pages, more_ahead) = Self::run(&self.ahead);
        Event::CalendarWindow {
            anchor: self.anchor,
            pages,
            more_past,
            floor: self.anchor - CALENDAR_MAX_PAST,
            ceiling: self.anchor + CALENDAR_CEILING,
            ahead_pages,
            more_ahead,
        }
    }

    /// Every broadcast held, in time order: the past pages with the oldest first, then the
    /// future.
    fn broadcasts(&self) -> Vec<&AiringEntry> {
        let (pages, _) = Self::run(&self.past);
        let (ahead_pages, _) = Self::run(&self.ahead);
        let past = self.past.range(..=pages).rev().flat_map(|(_, b)| b.items.iter().rev());
        let ahead = self.ahead.range(..=ahead_pages).flat_map(|(_, b)| b.items.iter());
        past.chain(ahead).collect()
    }
}

/// The last window, kept for [`REMEMBERED_FOR`], so a return to the calendar asks AniList
/// nothing. Cheap to clone; every clone is the same memory.
#[derive(Clone, Default)]
pub struct CalendarMemory {
    inner: Arc<Mutex<Option<Remembered>>>,
}

struct Remembered {
    /// The engine's clock when the window was fetched.
    at: i64,
    window: Window,
}

impl CalendarMemory {
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Remembered>> {
        self.inner.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The window kept, if it was fetched less than ten minutes before `now`.
    fn fresh(&self, now: i64) -> Option<Window> {
        let kept = self.lock();
        let kept = kept.as_ref()?;
        (now - kept.at < REMEMBERED_FOR && now >= kept.at).then(|| kept.window.clone())
    }

    fn keep(&self, at: i64, window: Window) {
        *self.lock() = Some(Remembered { at, window });
    }

    /// Add pages a job fetched to the window they belong to, if it is still the one kept.
    fn add(&self, anchor: i64, ahead: bool, pages: &[(u32, Block)]) {
        let mut kept = self.lock();
        if let Some(kept) = kept.as_mut().filter(|k| k.window.anchor == anchor) {
            let into = if ahead { &mut kept.window.ahead } else { &mut kept.window.past };
            into.extend(pages.iter().cloned());
        }
    }
}

/// The calendar's first load: what recently aired and what is coming, with the window a
/// backfill or a step forward continues from. Answered from memory when the last load is
/// fresh.
pub async fn load_calendar(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    clock: Clock,
    remembered: bool,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let started = clock.now();
    let (window, failure) = match memory.fresh(started).filter(|_| remembered) {
        Some(window) => (window, None),
        None => {
            let (window, failure) = fetch_window(&meta, now_epoch()).await;
            // A window with a half missing is shown, but not kept: the next visit asks again.
            if failure.is_none() {
                memory.keep(started, window.clone());
            }
            (window, failure)
        }
    };
    let update = match timeline(&window, &store) {
        Some(mut entries) => {
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            // Sent before the content, so the reducer knows how to continue the
            // list at the moment the list exists — a backfill triggered by the
            // very first keypress has everything it needs.
            let _ = tx.send(window.event());
            Event::Content {
                view: ContentFor::Calendar,
                content: ContentList::Entries(entries),
            }
        }
        None => {
            let error = failure.unwrap_or_else(|| "nothing airing in this window".into());
            Event::Content { view: ContentFor::Calendar, content: ContentList::Failed(error) }
        }
    };
    let _ = tx.send(update);
}

/// Pages further back in the calendar, for a scroll toward its top: `page`, and with `through`
/// the pages after it until a broadcast at or before that instant is in.
pub async fn load_calendar_past(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    anchor: i64,
    page: u32,
    through: Option<i64>,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let page = page.max(1);
    let last = if through.is_some() { page + CALENDAR_JOB_PAGES - 1 } else { page };
    let (blocks, failure) = walk(
        page,
        last,
        |page| fetch_past(&meta, anchor, page),
        // Newest first, so the last is the oldest.
        |b| through.is_some_and(|t| b.items.last().is_some_and(|a| a.airing_at <= t)),
    )
    .await;
    memory.add(anchor, false, &blocks);
    let update = match blocks.last() {
        Some((last, block)) => {
            let mut airings: Vec<&AiringEntry> = Vec::new();
            for (_, block) in blocks.iter().rev() {
                airings.extend(block.items.iter().rev());
            }
            let mut entries = data::entries_from_airings(airings, now_epoch(), &store);
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            Event::CalendarBackfill {
                page: Some(*last),
                entries,
                more_past: failure.is_some() || block.has_next,
            }
        }
        // Not a toast. The user never asked for this page by name — they scrolled —
        // and the calendar in front of them is unchanged and still correct. The
        // reducer clears its in-flight flag so the next scroll tries again.
        None => {
            tracing::warn!(error = ?failure, page, "calendar backfill failed");
            Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true }
        }
    };
    let _ = tx.send(update);
}

/// Pages further ahead in the calendar: `page`, and with `through` the pages after it until a
/// broadcast at or after that instant is in.
pub async fn load_calendar_ahead(
    lent: Lent,
    dubs: Dubs,
    memory: CalendarMemory,
    anchor: i64,
    page: u32,
    through: Option<i64>,
) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let page = page.max(1);
    let last = if through.is_some() { page + CALENDAR_JOB_PAGES - 1 } else { page };
    let (blocks, failure) = walk(
        page,
        last,
        |page| fetch_ahead(&meta, anchor, page),
        |b| through.is_some_and(|t| b.items.last().is_some_and(|a| a.airing_at >= t)),
    )
    .await;
    memory.add(anchor, true, &blocks);
    let update = match blocks.last() {
        Some((last, block)) => {
            let airings = blocks.iter().flat_map(|(_, b)| b.items.iter());
            let mut entries = data::entries_from_airings(airings, now_epoch(), &store);
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            Event::CalendarAhead {
                page: Some(*last),
                entries,
                more_ahead: failure.is_some() || block.has_next,
            }
        }
        None => {
            tracing::warn!(error = ?failure, page, "calendar ahead failed");
            Event::CalendarAhead { page: None, entries: Vec::new(), more_ahead: true }
        }
    };
    let _ = tx.send(update);
}

/// One page of past broadcasts, newest first.
///
/// The window's *upper* bound is the anchor and the sort is descending, so page numbering means
/// something stable for as long as the anchor holds: page three is the same fifty broadcasts
/// whether it is fetched with the first load or ten minutes later. That is what lets the backfill
/// ask for "the next page" without re-fetching what is on screen, and without a broadcast slipping
/// across a moving boundary and appearing twice. Re-reading the clock per page would break both.
async fn fetch_past(meta: &Meta, anchor: i64, page: u32) -> Result<Block, String> {
    let fetched = meta
        .airing_between_sorted(
            anchor - CALENDAR_MAX_PAST,
            anchor,
            page,
            CALENDAR_PER_PAGE,
            true,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(Block { items: fetched.items, has_next: fetched.has_next })
}

/// One page of future broadcasts, oldest first, from the anchor to the ceiling. Numbered
/// against a fixed upper bound for the same reason the past is.
async fn fetch_ahead(meta: &Meta, anchor: i64, page: u32) -> Result<Block, String> {
    let fetched = meta
        .airing_between_sorted(
            anchor,
            anchor + CALENDAR_CEILING,
            page,
            CALENDAR_PER_PAGE,
            false,
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(Block { items: fetched.items, has_next: fetched.has_next })
}

/// Fetch pages `first` to `last`, stopping early where AniList has no more or `covered` says
/// enough has come. Returns the pages that arrived, in order, and the error that stopped the
/// walk, if one did. A page failing part-way still leaves a usable calendar: what arrived is
/// kept, and the front end asks for the failed page again.
async fn walk<F, Fut>(
    first: u32,
    last: u32,
    mut fetch: F,
    covered: impl Fn(&Block) -> bool,
) -> (Vec<(u32, Block)>, Option<String>)
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<Block, String>>,
{
    let mut blocks = Vec::new();
    for page in first..=last {
        match fetch(page).await {
            Ok(block) => {
                let done = !block.has_next || covered(&block);
                blocks.push((page, block));
                if done {
                    break;
                }
            }
            Err(e) => return (blocks, Some(e)),
        }
    }
    (blocks, None)
}

/// The calendar's first load: the pages of past and of the days ahead that cover the week
/// either side, beside each other, numbered against `anchor`.
///
/// "Is there a tab for latest releases?" was a fair question with no good answer — the calendar
/// only looked forward, so the episodes you could actually watch right now appeared nowhere.
///
/// Two directions rather than one, and not for want of trying: a single ascending query over the
/// whole fortnight returns its page *oldest first*, and the page is exhausted long before it
/// reaches today. So the recent half is fetched newest-first and reversed.
///
/// **Both halves page.** The window was never the real limit — a single page is. Measured against
/// live data there are around 127 broadcasts in a week, and AniList caps `perPage` at 50 whatever
/// you ask for, so one page of past is a day and a half and one page of future is barely two days.
/// A seven-day window served by one page each was a seven-day window in name only.
///
/// Either half failing alone is still a usable screen. The failure is returned beside the window
/// so the caller can tell a whole window from half of one. A half that failed outright still
/// leaves its `more` set: there is no evidence it is empty, only that this attempt did not
/// reach it, and the front end should ask for it again.
async fn fetch_window(meta: &Meta, anchor: i64) -> (Window, Option<String>) {
    let past = walk(1, CALENDAR_INITIAL_PAST_PAGES, |p| fetch_past(meta, anchor, p), |_| false);
    let ahead = walk(1, CALENDAR_FUTURE_PAGES, |p| fetch_ahead(meta, anchor, p), |_| false);
    let ((past, past_error), (ahead, ahead_error)) = tokio::join!(past, ahead);
    let window =
        Window { anchor, past: past.into_iter().collect(), ahead: ahead.into_iter().collect() };
    (window, past_error.or(ahead_error))
}

/// The window as one timeline of rows: what recently aired, then what is coming, with the
/// rows the calendar estimates among the week ahead. `None` when it holds no broadcast.
fn timeline(window: &Window, store: &Store) -> Option<Vec<Entry>> {
    let now = now_epoch();
    let mut entries = data::entries_from_airings(window.broadcasts(), now, store);
    if entries.is_empty() {
        return None;
    }
    let boundary = entries.iter().filter(|e| e.airing_in.is_some_and(|t| t <= 0)).count();
    let guessed = estimated(&entries, now, window.anchor + CALENDAR_FUTURE, |id, episode| {
        store.airing_time(id, episode).ok().flatten()
    });
    if !guessed.is_empty() {
        tracing::info!(count = guessed.len(), "calendar rows estimated");
        entries.extend(guessed);
        // Back into time order: the estimates belong among the week ahead.
        entries.sort_by_key(|entry| entry.airing_in.unwrap_or(i64::MAX));
    }
    tracing::info!(recent = boundary, upcoming = entries.len() - boundary, "calendar timeline");
    Some(entries)
}

/// Mark each row of `entries` whose episode is out in the dub, as far as `dubs` already knows,
/// asking nothing: see [`Entry::airing_dubbed`]. Only rows that have aired, of titles whose dub
/// some page found, and only as far as the sources a dubbed play would use list it now.
fn mark_dubbed(entries: &mut [Entry], dubs: &Dubs, store: &Store, registry: &ProviderRegistry) {
    let mut through: HashMap<AnilistId, Option<u32>> = HashMap::new();
    dubbed_rows(entries, |id| {
        *through.entry(id).or_insert_with(|| {
            dubs.knows(id)
                .then(|| dubs.aired_through(id, &route(store, registry, id)))
                .flatten()
        })
    });
}

/// Mark the aired rows of `entries` whose episode `through` says the title's dub has reached.
fn dubbed_rows(entries: &mut [Entry], mut through: impl FnMut(AnilistId) -> Option<u32>) {
    for entry in entries {
        let (Some(airing_in), Some(episode)) = (entry.airing_in, entry.airing_episode) else {
            continue;
        };
        if airing_in > 0 || entry.airing_approximate {
            continue;
        }
        entry.airing_dubbed = through(entry.id).is_some_and(|through| episode <= through);
    }
}

/// The rows the calendar estimates, each with "~" for its time: for a show still airing whose
/// schedule AniList has stopped giving, its next episode, a week on from the last.
///
/// AniList dates every broadcast it lists, and a calendar of only its rows loses a show the
/// moment its schedule lapses, which is the week its viewers look for it. So a show is given
/// one estimated row when all of these hold, from what the calendar already loaded and what
/// this device saw of the schedule before (`aired_at`), asking nothing more:
///
/// - AniList says it is releasing, and has no next broadcast for it.
/// - Its latest broadcast here has aired, and no row of it is still to come.
/// - That broadcast came a week after the one before it, within an hour: a weekly show, since
///   a sequel's first episodes can air half an hour apart and the next eight days later.
/// - The count AniList gives, if any, has episodes left.
/// - A week on from the last broadcast is still within the calendar's week ahead.
fn estimated(
    entries: &[Entry],
    now: i64,
    until: i64,
    aired_at: impl Fn(AnilistId, u32) -> Option<i64>,
) -> Vec<Entry> {
    // Each show's latest aired row, and the shows with a row still to come.
    let mut latest: HashMap<AnilistId, &Entry> = HashMap::new();
    let mut coming: HashSet<AnilistId> = HashSet::new();
    for entry in entries {
        let (Some(airing_in), Some(episode)) = (entry.airing_in, entry.airing_episode) else {
            continue;
        };
        if airing_in > 0 {
            coming.insert(entry.id);
        } else if latest.get(&entry.id).is_none_or(|e| e.airing_episode < Some(episode)) {
            latest.insert(entry.id, entry);
        }
    }
    let at = |entry: &Entry| entry.airing_in.map(|airing_in| now + airing_in);
    let mut guessed: Vec<Entry> = latest
        .into_values()
        .filter(|last| !coming.contains(&last.id))
        .filter(|last| {
            last.status == Some(AiringStatus::Releasing) && last.next_episode.is_none()
        })
        .filter_map(|last| {
            let episode = last.airing_episode?;
            if last.episodes.is_some_and(|total| episode >= total) || episode < 2 {
                return None;
            }
            let aired = at(last)?;
            let before = entries
                .iter()
                .find(|e| e.id == last.id && e.airing_episode == Some(episode - 1))
                .and_then(at)
                .or_else(|| aired_at(last.id, episode - 1))?;
            if (aired - before - WEEK).abs() > WEEKLY_WITHIN {
                return None;
            }
            let next = aired + WEEK;
            (next > now && next <= until).then(|| Entry {
                airing_episode: Some(episode + 1),
                airing_in: Some(next - now),
                delayed_from: None,
                airing_approximate: true,
                ..last.clone()
            })
        })
        .collect();
    guessed.sort_by_key(|entry| (entry.airing_in, entry.id.get()));
    guessed
}

/// A week, which an estimated row is a week on from its show's last broadcast.
const WEEK: i64 = 7 * 86_400;
/// How far from a week apart a show's last two broadcasts may be and still count as weekly:
/// AniList nudges a slot by minutes as a broadcaster pins it down.
const WEEKLY_WITHIN: i64 = 3_600;

/// The furthest back the calendar will ever reach, and so the floor of every past query.
///
/// It bounds the *backfill*, not the first load: because the sort is descending from the anchor,
/// page one is the fifty most recent broadcasts whatever this is set to. All it decides is when
/// scrolling up finally runs out. Three months is far enough back that reaching the end is a
/// journey rather than an accident.
const CALENDAR_MAX_PAST: i64 = 90 * 86_400;
/// How far ahead the calendar will ever reach, and so the ceiling of every future query: four
/// weeks from the anchor, fixed so that page numbers stay stable.
const CALENDAR_CEILING: i64 = 28 * 86_400;
/// How far forward the first load promises. A week covers a full broadcast cycle, so every
/// airing show you follow has exactly one upcoming episode in view. Estimated rows stay within
/// it.
const CALENDAR_FUTURE: i64 = 7 * 86_400;
/// Rows per request. AniList's ceiling: ask for 75 or 100 and the response says `perPage: 50`
/// and returns fifty. Depth is bought in pages, never here.
const CALENDAR_PER_PAGE: u32 = 50;
/// Pages of history fetched before the screen is first drawn. At roughly 127 broadcasts a week
/// three pages covers a full week and change, which is what the seven-day window always claimed
/// to offer. Scrolling past them backfills further without being asked.
const CALENDAR_INITIAL_PAST_PAGES: u32 = 3;
/// And forward, covering the week ahead by the same measure.
const CALENDAR_FUTURE_PAGES: u32 = 3;
/// The most pages one job walks to reach its `through`. About a week and a half of broadcasts
/// is three pages, so this is several weeks; a job that stops short says there is more.
const CALENDAR_JOB_PAGES: u32 = 12;
/// How long the last window is kept (seconds), so a return to the calendar asks AniList nothing.
const REMEMBERED_FOR: i64 = 600;

#[cfg(test)]
mod tests {
    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_101);
    const NOW: i64 = 1_800_000_000;
    const HOUR: i64 = 3_600;
    const DAY: i64 = 86_400;
    const UNTIL: i64 = NOW + 7 * DAY;

    /// A row of `SHOW` about `episode`, airing `at` seconds from now, AniList calling the show
    /// releasing with no broadcast to come.
    fn row(episode: u32, at: i64) -> Entry {
        let mut entry = Entry::new(SHOW, "Invented Show");
        entry.airing_episode = Some(episode);
        entry.airing_in = Some(at);
        entry.status = Some(AiringStatus::Releasing);
        entry.episodes = Some(12);
        entry
    }

    fn never(_: AnilistId, _: u32) -> Option<i64> {
        None
    }

    #[test]
    fn a_weekly_show_with_no_schedule_left_gets_its_next_episode_a_week_on() {
        let rows = [row(4, -8 * DAY), row(5, -DAY)];
        let guessed = estimated(&rows, NOW, UNTIL, never);
        assert_eq!(guessed.len(), 1);
        let next = &guessed[0];
        assert_eq!(next.airing_episode, Some(6));
        assert_eq!(next.airing_in, Some(6 * DAY));
        assert!(next.airing_approximate);
        assert!(!rows[1].airing_approximate, "AniList's own rows are not estimates");
    }

    #[test]
    fn the_week_before_can_come_from_what_this_device_saw() {
        let rows = [row(5, -DAY)];
        let seen = |id: AnilistId, episode: u32| {
            (id == SHOW && episode == 4).then_some(NOW - 8 * DAY + 20 * 60)
        };
        assert_eq!(estimated(&rows, NOW, UNTIL, seen)[0].airing_episode, Some(6));
        assert!(estimated(&rows, NOW, UNTIL, never).is_empty(), "no cadence, no guess");
    }

    #[test]
    fn nothing_is_guessed_where_anilist_still_has_a_say() {
        // A row still to come.
        assert!(
            estimated(&[row(4, -8 * DAY), row(5, -DAY), row(6, 6 * DAY)], NOW, UNTIL, never)
                .is_empty()
        );
        // A next broadcast AniList has, past the week: a break.
        let mut on_break = row(5, -DAY);
        on_break.next_episode = Some(6);
        assert!(estimated(&[row(4, -8 * DAY), on_break], NOW, UNTIL, never).is_empty());
        // Finished, or at its last episode.
        let mut done = row(5, -DAY);
        done.status = Some(AiringStatus::Finished);
        assert!(estimated(&[row(4, -8 * DAY), done], NOW, UNTIL, never).is_empty());
        let mut last = row(12, -DAY);
        last.episodes = Some(12);
        assert!(estimated(&[row(11, -8 * DAY), last], NOW, UNTIL, never).is_empty());
    }

    #[test]
    fn broadcasts_not_a_week_apart_are_no_cadence() {
        // Half an hour apart, as a sequel's first two can be.
        assert!(
            estimated(&[row(4, -DAY - HOUR / 2), row(5, -DAY)], NOW, UNTIL, never).is_empty()
        );
        // Eight days apart.
        assert!(estimated(&[row(4, -9 * DAY), row(5, -DAY)], NOW, UNTIL, never).is_empty());
        // A nudge of minutes is still weekly.
        assert_eq!(
            estimated(&[row(4, -8 * DAY - 10 * 60), row(5, -DAY)], NOW, UNTIL, never).len(),
            1
        );
    }

    #[test]
    fn an_aired_row_is_marked_dubbed_as_far_as_the_dub_is_known_to_go() {
        let mut rows = [row(4, -8 * DAY), row(5, -DAY), row(6, 6 * DAY)];
        dubbed_rows(&mut rows, |id| (id == SHOW).then_some(4));
        let marked: Vec<bool> = rows.iter().map(|row| row.airing_dubbed).collect();
        assert_eq!(marked, [true, false, false], "to episode 4, and nothing yet to air");

        let mut rows = [row(4, -8 * DAY)];
        dubbed_rows(&mut rows, |_| None);
        assert!(!rows[0].airing_dubbed, "nothing known, nothing marked");
    }

    #[test]
    fn a_guess_past_the_week_ahead_or_already_due_is_left_out() {
        // Aired ten days ago: a week on is already past.
        assert!(
            estimated(&[row(4, -17 * DAY), row(5, -10 * DAY)], NOW, UNTIL, never).is_empty()
        );
        // The calendar's week ends sooner than a week on.
        assert!(
            estimated(&[row(4, -8 * DAY), row(5, -DAY)], NOW, NOW + 5 * DAY, never).is_empty()
        );
    }
}
