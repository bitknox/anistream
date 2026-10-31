//! Which AniList entry each series on a server is.
//!
//! In order of trust: the AniList id the server's metadata carries; the mapping dataset from
//! AniDB, TVDB or TMDB ids; and last the series' name against AniList's search, accepted only
//! when exactly one result clears the same confidence floor the sources use. A series that none
//! of these settle stays unmatched rather than guessed.

use anistream_core::ids::{AnilistId, ExternalIds};
use anistream_core::media::SearchHit;
use anistream_jellyfin::{
    Jellyfin, JellyfinError, SeriesMatch,
    model::{Library, Series},
    source::ids_of,
};
use anistream_meta::{CONFIDENCE_FLOOR, Meta, anilist::BrowseFilter, rank};
use anistream_model::servers::LibraryChoice;
use anistream_store::Store;

use super::record::Record;

/// The most name lookups one sync makes: AniList allows 30 requests a minute, shared with the
/// rest of the app. Series left over are matched on the next sync.
const NAME_LOOKUPS: usize = 40;

fn is_shows(library: &Library) -> bool {
    matches!(library.collection_type.as_deref(), Some("tvshows") | None)
}

/// A library looks like anime when its name says so, or when most of its series carry an AniList
/// or AniDB id.
fn looks_like_anime(library: &Library, series: &[Series]) -> bool {
    if library.name.to_lowercase().contains("anime") {
        return true;
    }
    if series.is_empty() {
        return false;
    }
    let with_ids = series
        .iter()
        .filter(|s| s.provider_id("AniList").is_some() || s.provider_id("AniDB").is_some())
        .count();
    with_ids * 2 > series.len()
}

/// The libraries to choose from. Ones already chosen keep their choice; on first sight the
/// anime-looking shows libraries start ticked.
pub async fn choices(
    client: &Jellyfin,
    record: &Record,
) -> Result<Vec<LibraryChoice>, JellyfinError> {
    let mut out = Vec::new();
    for library in client.libraries().await? {
        let kept = record.libraries.iter().find(|l| l.id == library.id);
        let ticked = match kept {
            Some(kept) if record.libraries_chosen => kept.ticked,
            _ if is_shows(&library) => {
                looks_like_anime(&library, &client.series(&library.id).await?)
            }
            _ => false,
        };
        out.push(LibraryChoice {
            id: library.id,
            name: library.name,
            kind: library.collection_type,
            ticked,
        });
    }
    Ok(out)
}

/// Match every series of the ticked libraries. Returns the matches and how many series there were.
pub async fn match_all(
    client: &Jellyfin,
    record: &Record,
    meta: &Meta,
    store: &Store,
) -> Result<(Vec<SeriesMatch>, u32), JellyfinError> {
    let mut matches: Vec<SeriesMatch> = Vec::new();
    let mut total = 0u32;
    let mut lookups = 0usize;
    for library in record.libraries.iter().filter(|l| l.ticked) {
        for series in client.series(&library.id).await? {
            total += 1;
            let ids = ids_of(&series);
            let mut found = by_ids(client, store, &series, &ids).await?;
            if found.is_empty() && lookups < NAME_LOOKUPS {
                lookups += 1;
                found = by_name(meta, &series).await;
            }
            for (anilist, season) in found {
                matches.push(SeriesMatch {
                    series: series.id.clone(),
                    name: series.name.clone(),
                    anilist,
                    ids: ExternalIds { anilist: Some(anilist), ..ids },
                    season,
                });
            }
        }
    }
    Ok((matches, total))
}

/// The entries the server's own ids name. When the dataset names several entries for one
/// series (one per season) they are paired with the series' seasons in AniList id order, but
/// only when the counts agree; otherwise just the first entry is kept, as season one.
async fn by_ids(
    client: &Jellyfin,
    store: &Store,
    series: &Series,
    ids: &ExternalIds,
) -> Result<Vec<(AnilistId, Option<u32>)>, JellyfinError> {
    if let Some(id) = ids.anilist {
        return Ok(vec![(id, None)]);
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut entries: Vec<AnilistId> = store
        .anilist_entries_for_ids(ids)
        .unwrap_or_default()
        .into_iter()
        .map(|e| e.anilist_id)
        .collect();
    entries.sort();
    entries.dedup();
    match entries.len() {
        0 => Ok(Vec::new()),
        1 => Ok(vec![(entries[0], None)]),
        n => {
            let all = client.episodes(&series.id).await?;
            let mut seasons: Vec<u32> =
                all.iter().filter_map(|e| e.season).filter(|s| *s > 0).collect();
            seasons.sort();
            seasons.dedup();
            if seasons.len() == n {
                Ok(entries.into_iter().zip(seasons).map(|(a, s)| (a, Some(s))).collect())
            } else {
                Ok(vec![(entries[0], Some(1))])
            }
        }
    }
}

/// The one AniList entry whose title the series' name is, when exactly one is clearly it.
async fn by_name(meta: &Meta, series: &Series) -> Vec<(AnilistId, Option<u32>)> {
    let Ok(found) = meta.find(&series.name, &BrowseFilter::default(), 1, 5).await else {
        return Vec::new();
    };
    let mut hit = SearchHit::new(series.id.as_str(), series.name.as_str());
    hit.year = series.production_year;
    let mut confident = Vec::new();
    for media in &found.page.items {
        let target = media.match_target();
        // Not the episode count or format: the server's series has neither to compare.
        let target =
            anistream_meta::MatchTarget { episode_count: None, format: None, ..target };
        let hits = [hit.clone()];
        if let Some(best) = rank(&target, &hits).first()
            && best.rejected.is_none()
            && best.score >= CONFIDENCE_FLOOR
        {
            confident.push((media.id, best.score));
        }
    }
    match confident.as_slice() {
        [(id, _)] => vec![(*id, None)],
        _ => Vec::new(),
    }
}
