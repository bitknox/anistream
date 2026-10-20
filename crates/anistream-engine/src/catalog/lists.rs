//! The lists: what you were watching, from local history, and this season's most popular, each
//! followed by when its releasing titles last aired.

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::{
    catalog::{Content, ContentFor},
    event::Event,
    notice::Notice,
};
use anistream_store::Store;

use crate::{data, dispatch::Lent, now_epoch, outbox::Outbox, services::Services};

/// What you were watching, as the CONTINUE rail lists it, or the empty that says nothing has
/// been watched yet.
pub async fn load_continue(lent: Lent) {
    let Lent { meta, store, tx, .. } = lent;
    let update = match continue_entries(&meta, &store).await {
        // Nothing in the history at all is a different empty from a history with
        // nothing left to continue — everything hidden with `x` — and only the first
        // is "nothing watched yet". `Empty` says it is the first.
        Ok(entries) if entries.is_empty() && store.stats().is_ok_and(|s| s.titles == 0) => {
            Event::Content { view: ContentFor::Continue, content: Content::Empty }
        }
        Ok(entries) => {
            publish_list(&meta, &tx, ContentFor::Continue, entries, now_epoch()).await;
            return;
        }
        Err(e) => Event::Content { view: ContentFor::Continue, content: Content::Failed(e) },
    };
    let _ = tx.send(update);
}

/// This season's most popular titles, captioned with which season it is.
pub async fn load_seasonal(lent: Lent) {
    let Lent { meta, store, tx, .. } = lent;
    const PAGE: u32 = 40;
    let (season, year) = current_season();
    let update = match meta.seasonal(season, year, &Default::default(), 1, PAGE).await {
        Ok(page) => {
            // Which season this is, which the screen never said, and that one
            // page is not all of it when it is not.
            let season = format!("{} {year}", season.as_str().to_lowercase());
            let text = if page.has_next {
                format!("{season} · the {PAGE} most popular")
            } else {
                season
            };
            let _ = tx.send(Event::Caption { view: ContentFor::Seasonal, text });
            let entries =
                page.items.iter().map(|m| data::entry_from(m, Some(&store))).collect();
            publish_list(&meta, &tx, ContentFor::Seasonal, entries, now_epoch()).await;
            return;
        }
        Err(e) => Event::Content {
            view: ContentFor::Seasonal,
            content: Content::Failed(e.to_string()),
        },
    };
    let _ = tx.send(update);
}

/// Take a title off the CONTINUE rail for good.
pub fn hide_from_continue(id: AnilistId, services: &Services, tx: &Outbox) {
    let Services { store, .. } = services;
    // The reducer has already dropped the row from the screen; failing to persist
    // that would resurrect it on the next reload, so the failure is named.
    if let Err(e) = store.hide_from_continue(id) {
        let _ = tx
            .send(Event::Toast(Notice::alert(format!("could not remove from continue: {e}"))));
    }
}

/// What you were watching, most recent first.
///
/// Trending and continuing are different questions, and this screen used to answer the wrong one:
/// it fetched the current season, which made the section labelled CONTINUE a discovery screen and
/// left an episode abandoned halfway through — the single most likely reason to open the app —
/// with nowhere to appear. Discovery has three sections of its own; this one is personal.
///
/// Built from local history, so it works with no account at all.
///
/// **Ordering is the feature.** `continue_list` orders by `updated_at DESC` and
/// [`AniList::media_many`] preserves the order it was asked in — which it has to, because AniList
/// returns `id_in` results in its own order and letting that through would shuffle the rail into
/// something arbitrary. Together they are what put the thing you last watched at the top.
///
/// An empty result is a real answer here, not a failure to paper over: the screen says so and
/// points at where to find something, rather than silently showing a different kind of list.
async fn continue_entries(
    meta: &Meta,
    store: &Store,
) -> std::result::Result<Vec<anistream_model::catalog::Entry>, String> {
    let continuing = store.continue_list(CONTINUE_ROWS).unwrap_or_default();
    let ids: Vec<anistream_core::ids::AnilistId> =
        continuing.iter().map(|p| p.anilist_id).collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    meta.media_many(&ids)
        .await
        .map(|media| media.iter().map(|m| data::entry_from(m, Some(store))).collect())
        .map_err(|e| e.to_string())
}

/// Titles kept in the CONTINUE rail. Long enough to cover what you are actually mid-way through,
/// short enough that it stays a shortlist rather than a second library.
const CONTINUE_ROWS: u32 = 15;

/// Publish a list, then follow up with the broadcast dates it needs a second request for.
///
/// Two sends rather than one: the last-aired query depends on which titles came back, so it
/// cannot be folded into the list query. The list renders as soon as it arrives and gains the
/// broadcast line a moment later, rather than the whole screen waiting on an annotation.
pub async fn publish_list(
    meta: &Meta,
    tx: &Outbox,
    view: ContentFor,
    entries: Vec<anistream_model::catalog::Entry>,
    now: i64,
) {
    // Releasing titles only. A finished show has no broadcast to report, and including it would
    // spend rows of a capped response on titles that cannot use them.
    let ids: Vec<anistream_core::ids::AnilistId> =
        entries.iter().filter(|e| e.airing_in.is_some()).map(|e| e.id).collect();
    let _ = tx.send(Event::Content { view, content: Content::Entries(entries) });
    if ids.is_empty() {
        return;
    }
    match meta.last_aired(&ids).await {
        Ok(rows) => {
            let _ = tx.send(Event::LastAired(
                rows.into_iter()
                    // Aired rows only. AniList has answered this query with rows whose air
                    // time is still ahead, and naming next week's episode as "out just now"
                    // is exactly the lie the broadcast line exists not to tell.
                    .filter(|r| r.airing_at <= now)
                    .map(|r| (r.media_id, r.episode, now.saturating_sub(r.airing_at)))
                    .collect(),
            ));
        }
        // Not worth a toast: the list is already on screen and complete, and this only adds a
        // line to it. Logged so the Logs overlay can still explain a missing line.
        Err(e) => tracing::warn!(%e, "last-aired lookup failed"),
    }
}

/// The season containing today, derived from the epoch without a calendar dependency.
pub fn current_season() -> (anistream_meta::anilist::Season, u16) {
    use anistream_meta::anilist::Season;
    let days = now_epoch() / 86_400;
    // Civil-from-days, the standard algorithm.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    (Season::of_month(month as u8), year as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_season_is_plausible() {
        let (_, year) = current_season();
        assert!((2024..2100).contains(&year), "derived year {year} is not plausible");
    }

    #[test]
    fn season_derivation_matches_known_dates() {
        use anistream_meta::anilist::Season;
        // Verified against the civil-from-days algorithm.
        let cases = [
            (1_704_067_200_i64, Season::Winter, 2024u16), // 2024-01-01
            (1_719_792_000, Season::Summer, 2024),        // 2024-07-01
            (1_727_740_800, Season::Fall, 2024),          // 2024-10-01
        ];
        for (epoch, season, year) in cases {
            let days = epoch / 86_400;
            let z = days + 719_468;
            let era = z.div_euclid(146_097);
            let doe = z.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let derived_year = if month <= 2 { y + 1 } else { y };
            assert_eq!(Season::of_month(month as u8), season, "for epoch {epoch}");
            assert_eq!(derived_year as u16, year, "for epoch {epoch}");
        }
    }
}
