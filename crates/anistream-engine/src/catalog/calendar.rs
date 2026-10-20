//! The calendar: one timeline of what recently aired and what is coming, and the pages further
//! back that a scroll toward its top asks for.

use anistream_meta::Meta;
use anistream_model::{
    catalog::{Content, ContentFor},
    event::Event,
};
use anistream_store::Store;

use crate::{data, dispatch::Lent, now_epoch};

/// The calendar's first load: what recently aired and the week ahead, with the window a
/// backfill continues from.
pub async fn load_calendar(lent: Lent) {
    let Lent { meta, store, tx, .. } = lent;
    let anchor = now_epoch();
    let update = match calendar_timeline(&meta, &store, anchor).await {
        Ok(load) => {
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
                content: Content::Entries(load.entries),
            }
        }
        Err(e) => Event::Content { view: ContentFor::Calendar, content: Content::Failed(e) },
    };
    let _ = tx.send(update);
}

/// One page further back in the calendar, for a scroll toward its top.
pub async fn load_calendar_past(lent: Lent, anchor: i64, page: u32) {
    let Lent { meta, store, tx, .. } = lent;
    let update = match calendar_past_page(&meta, &store, anchor, page).await {
        Ok((entries, more_past)) => {
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
    let entries =
        page.items.iter().rev().map(|a| data::entry_from_airing(a, now, Some(store))).collect();
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
                entries.extend(
                    fetched.items.iter().map(|a| data::entry_from_airing(a, now, Some(store))),
                );
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
