//! The calendar: one timeline of what recently aired and what is coming, and the pages further
//! back that a scroll toward its top asks for.

use std::collections::{HashMap, HashSet};

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::{
    catalog::{AiringStatus, ContentFor, ContentList, Entry},
    event::Event,
};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{
    data,
    dispatch::Lent,
    now_epoch,
    titles::dub::{Dubs, route},
};

/// The calendar's first load: what recently aired and the week ahead, with the window a
/// backfill continues from.
pub async fn load_calendar(lent: Lent, dubs: Dubs) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let anchor = now_epoch();
    let update = match calendar_timeline(&meta, &store, anchor).await {
        Ok(mut load) => {
            mark_dubbed(&mut load.entries, &dubs, &store, &registry);
            // Sent before the content, so the reducer knows how to continue the
            // list at the moment the list exists — a backfill triggered by the
            // very first keypress has everything it needs.
            let _ = tx.send(Event::CalendarWindow {
                anchor,
                pages: load.pages,
                more_past: load.more_past,
            });
            Event::Content {
                view: ContentFor::Calendar,
                content: ContentList::Entries(load.entries),
            }
        }
        Err(e) => {
            Event::Content { view: ContentFor::Calendar, content: ContentList::Failed(e) }
        }
    };
    let _ = tx.send(update);
}

/// One page further back in the calendar, for a scroll toward its top.
pub async fn load_calendar_past(lent: Lent, dubs: Dubs, anchor: i64, page: u32) {
    let Lent { meta, store, registry, tx, .. } = lent;
    let update = match calendar_past_page(&meta, &store, anchor, page).await {
        Ok((mut entries, more_past)) => {
            mark_dubbed(&mut entries, &dubs, &store, &registry);
            Event::CalendarBackfill { page: Some(page), entries, more_past }
        }
        // Not a toast. The user never asked for this page by name — they scrolled —
        // and the calendar in front of them is unchanged and still correct. The
        // reducer clears its in-flight flag so the next scroll tries again.
        Err(e) => {
            tracing::warn!(%e, page, "calendar backfill failed");
            Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true }
        }
    };
    let _ = tx.send(update);
}

/// The calendar's first load: the timeline, plus what the backfill needs to continue it.
struct CalendarLoad {
    entries: Vec<anistream_model::catalog::Entry>,
    /// Pages of past actually fetched, so the backfill knows which page is next.
    pages: u32,
    /// Whether AniList reported broadcasts older than the ones loaded.
    more_past: bool,
}

/// The calendar as one timeline: what recently aired, then what is coming.
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
async fn calendar_timeline(
    meta: &Meta,
    store: &Store,
    anchor: i64,
) -> std::result::Result<CalendarLoad, String> {
    let (past, future) = tokio::join!(
        calendar_past(meta, store, anchor, CALENDAR_INITIAL_PAST_PAGES),
        calendar_future(meta, store, anchor),
    );

    // Either half failing alone is still a usable screen, so only report a failure when both
    // are empty — a calendar showing just the upcoming week beats an error page.
    let past_error = past.as_ref().err().cloned();
    // A past half that failed outright still leaves `more_past` set: there is no evidence the
    // history is empty, only that this attempt did not reach it, and the backfill should retry.
    let (mut entries, pages, more_past) = past.unwrap_or((Vec::new(), 0, true));
    let boundary = entries.len();

    match future {
        Ok(upcoming) => entries.extend(upcoming),
        Err(e) => {
            if entries.is_empty() {
                return Err(past_error.unwrap_or(e));
            }
        }
    }
    if entries.is_empty() {
        return Err(past_error.unwrap_or_else(|| "nothing airing in this window".into()));
    }
    let now = now_epoch();
    let guessed = estimated(&entries, now, anchor + CALENDAR_FUTURE, |id, episode| {
        store.airing_time(id, episode).ok().flatten()
    });
    if !guessed.is_empty() {
        tracing::info!(count = guessed.len(), "calendar rows estimated");
        entries.extend(guessed);
        // Back into time order: the estimates belong among the week ahead.
        entries.sort_by_key(|entry| entry.airing_in.unwrap_or(i64::MAX));
    }

    tracing::info!(
        recent = boundary,
        upcoming = entries.len() - boundary,
        pages,
        more_past,
        "calendar timeline"
    );
    Ok(CalendarLoad { entries, pages, more_past })
}

/// One page of past broadcasts, in chronological order.
///
/// The window's *upper* bound is the anchor and the sort is descending, so page numbering means
/// something stable for as long as the anchor holds: page three is the same fifty broadcasts
/// whether it is fetched with the first load or ten minutes later. That is what lets the backfill
/// ask for "the next page" without re-fetching what is on screen, and without a broadcast slipping
/// across a moving boundary and appearing twice. Re-reading the clock per page would break both.
async fn calendar_past_page(
    meta: &Meta,
    store: &Store,
    anchor: i64,
    page: u32,
) -> std::result::Result<(Vec<anistream_model::catalog::Entry>, bool), String> {
    let page = meta
        .airing_between_sorted(
            anchor - CALENDAR_MAX_PAST,
            anchor,
            page,
            CALENDAR_PER_PAGE,
            true,
        )
        .await
        .map_err(|e| e.to_string())?;

    let now = now_epoch();
    // Reversed back into chronological order, so the list reads downward through time.
    let entries = data::entries_from_airings(page.items.iter().rev(), now, store);
    Ok((entries, page.has_next))
}

/// Walk back through pages of history until `pages` are spent or AniList runs out.
///
/// Returns the entries chronologically, how many pages were actually fetched, and whether there
/// is more behind them. Pages are fetched newest-first and each is internally reversed, so the
/// *blocks* have to be reversed too — page two sits above page one on a timeline reading down.
async fn calendar_past(
    meta: &Meta,
    store: &Store,
    anchor: i64,
    pages: u32,
) -> std::result::Result<(Vec<anistream_model::catalog::Entry>, u32, bool), String> {
    let mut blocks: Vec<Vec<anistream_model::catalog::Entry>> = Vec::new();
    let mut loaded = 0;
    let mut more = false;
    let mut failure = None;

    for page in 1..=pages.max(1) {
        match calendar_past_page(meta, store, anchor, page).await {
            Ok((entries, has_next)) => {
                loaded = page;
                more = has_next;
                blocks.push(entries);
                if !has_next {
                    break;
                }
            }
            // A page failing part-way still leaves a usable calendar. Keep what arrived, leave
            // `more` set so the backfill retries this page, and only surface the error if
            // nothing came back at all.
            Err(e) => {
                more = loaded > 0;
                failure = Some(e);
                break;
            }
        }
    }

    if blocks.is_empty() {
        return Err(failure.unwrap_or_else(|| "nothing aired in this window".into()));
    }
    blocks.reverse();
    Ok((blocks.concat(), loaded, more))
}

/// The upcoming half, walked the same way.
///
/// Ascending, so pages concatenate as they arrive with no reversing. There is no backfill for
/// this direction: the far end of the window is a fixed week out rather than open-ended history,
/// and these pages cover it.
async fn calendar_future(
    meta: &Meta,
    store: &Store,
    anchor: i64,
) -> std::result::Result<Vec<anistream_model::catalog::Entry>, String> {
    let mut entries: Vec<anistream_model::catalog::Entry> = Vec::new();
    let mut failure = None;

    for page in 1..=CALENDAR_FUTURE_PAGES {
        match meta
            .airing_between_sorted(
                anchor,
                anchor + CALENDAR_FUTURE,
                page,
                CALENDAR_PER_PAGE,
                false,
            )
            .await
        {
            Ok(fetched) => {
                let now = now_epoch();
                entries.extend(data::entries_from_airings(&fetched.items, now, store));
                if !fetched.has_next {
                    break;
                }
            }
            Err(e) => {
                failure = Some(e.to_string());
                break;
            }
        }
    }

    match failure {
        Some(e) if entries.is_empty() => Err(e),
        _ => Ok(entries),
    }
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
/// deliberate journey rather than an accident.
const CALENDAR_MAX_PAST: i64 = 90 * 86_400;
/// How far forward the calendar reaches. A week covers a full broadcast cycle, so every airing
/// show you follow has exactly one upcoming episode in view.
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
