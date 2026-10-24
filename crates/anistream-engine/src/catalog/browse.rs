//! Search's start, and what its filters choose from.
//!
//! The rails come in one request, kept as ids so the next visit shows them at once from the
//! metadata cache, and asked again only once they are half an hour old. The facets change a
//! few times a year, so they are asked for once a week.

use anistream_core::ids::AnilistId;
use anistream_meta::{Facets, Media, Meta};
use anistream_model::{
    catalog::Season,
    event::Event,
    search::{
        ADULT_GENRE, ADULT_TAG_CATEGORY, BrowseRail, FacetTag, GENRES, SearchFacets,
        SearchQuery, SearchSort, TagGroup, season_label,
    },
};
use anistream_store::Store;
use serde::{Deserialize, Serialize};

use crate::{catalog::search::browse_filter, data, dispatch::Lent};

/// Titles a rail holds.
const RAIL: u32 = 20;
/// How long the rails serve before they are asked again.
const RAILS_FOR: i64 = 30 * 60;
/// How long the facets serve.
const FACETS_FOR: i64 = 7 * 24 * 60 * 60;
const RAILS_KEY: &str = "search:rails";
const FACETS_KEY: &str = "search:facets";

/// The rails, as kept: what was asked, under which setting, when, and the ids found.
#[derive(Serialize, Deserialize, PartialEq)]
struct KeptRails {
    at: i64,
    adult: bool,
    queries: Vec<SearchQuery>,
    ids: Vec<Vec<u32>>,
}

#[derive(Serialize, Deserialize)]
struct KeptFacets {
    at: i64,
    facets: Facets,
}

/// The rails for the season `now` is in: what is trending, this season and the next, the most
/// popular of all time and the best rated.
pub fn rails(season: Season, year: u16) -> Vec<(String, Option<String>, SearchQuery)> {
    let (next, next_year) = match season {
        Season::Winter => (Season::Spring, year),
        Season::Spring => (Season::Summer, year),
        Season::Summer => (Season::Fall, year),
        Season::Fall => (Season::Winter, year + 1),
    };
    let named = |season: Season, year: u16| Some(format!("{} {year}", season_label(season)));
    vec![
        ("Trending now".into(), None, SearchQuery::sorted(SearchSort::Trending)),
        ("Popular this season".into(), named(season, year), SearchQuery::season(season, year)),
        (
            "Upcoming next season".into(),
            named(next, next_year),
            SearchQuery::season(next, next_year),
        ),
        ("All-time popular".into(), None, SearchQuery::sorted(SearchSort::Popularity)),
        ("Top rated".into(), None, SearchQuery::sorted(SearchSort::Score)),
    ]
}

/// The season the clock is in, in the model's words.
pub fn this_season(now: i64) -> (Season, u16) {
    let (season, year) = crate::catalog::lists::season_at(now);
    let season = match season {
        anistream_meta::Season::Winter => Season::Winter,
        anistream_meta::Season::Spring => Season::Spring,
        anistream_meta::Season::Summer => Season::Summer,
        anistream_meta::Season::Fall => Season::Fall,
    };
    (season, year)
}

/// Search's start: at once from what was kept, and again once AniList answers when that is
/// old or was for another season or setting.
pub async fn load_browse(lent: Lent, adult: bool, now: i64) {
    let Lent { meta, store, tx, .. } = lent;
    let (season, year) = this_season(now);
    let rails = rails(season, year);
    let queries: Vec<SearchQuery> = rails.iter().map(|(_, _, q)| q.clone()).collect();
    let kept = store
        .get_meta_text(RAILS_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<KeptRails>(&json).ok())
        .filter(|kept| kept.adult == adult && kept.queries == queries);
    let mut shown = false;
    if let Some(kept) = &kept {
        let media: Vec<Vec<Media>> = kept
            .ids
            .iter()
            .map(|ids| {
                meta.cached(&ids.iter().map(|id| AnilistId::new(*id)).collect::<Vec<_>>())
            })
            .collect();
        if media.iter().any(|m| !m.is_empty()) {
            let _ = tx.send(Event::Browse(publish(&rails, &media, &store)));
            shown = true;
            if now - kept.at < RAILS_FOR {
                return;
            }
        }
    }
    let filters: Vec<_> = queries.iter().map(|q| browse_filter(q, adult)).collect();
    let asked: Vec<(&str, &anistream_meta::BrowseFilter)> =
        filters.iter().map(|f| ("", f)).collect();
    match meta.browse_many(&asked, RAIL).await {
        Ok(pages) => {
            let media: Vec<Vec<Media>> = pages.into_iter().map(|p| p.items).collect();
            let ids = media.iter().map(|m| m.iter().map(|m| m.id.get()).collect()).collect();
            let kept = KeptRails { at: now, adult, queries, ids };
            if let Ok(json) = serde_json::to_string(&kept)
                && let Err(e) = store.set_meta_text(RAILS_KEY, &json)
            {
                tracing::warn!(error = %e, "could not keep Search's rails");
            }
            let _ = tx.send(Event::Browse(publish(&rails, &media, &store)));
        }
        Err(e) => {
            // The rails are a way in, not the only one: the field and the filters still
            // work, so a failure is logged and the start shows without them.
            tracing::warn!(error = %e, "Search's rails failed");
            if !shown {
                let _ = tx.send(Event::Browse(Vec::new()));
            }
        }
    }
}

fn publish(
    rails: &[(String, Option<String>, SearchQuery)],
    media: &[Vec<Media>],
    store: &Store,
) -> Vec<BrowseRail> {
    rails
        .iter()
        .zip(media)
        .filter(|(_, media)| !media.is_empty())
        .map(|((title, detail, query), media)| BrowseRail {
            title: title.clone(),
            detail: detail.clone(),
            query: query.clone(),
            entries: media.iter().map(|m| data::entry_from(m, Some(store))).collect(),
        })
        .collect()
}

/// What the filters choose from: kept for a week, and AniList's genres alone when it has
/// never answered.
pub async fn load_facets(lent: Lent, adult: bool, now: i64) {
    let Lent { meta, store, tx, .. } = lent;
    let kept = store
        .get_meta_text(FACETS_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<KeptFacets>(&json).ok());
    if let Some(kept) = &kept
        && now - kept.at < FACETS_FOR
    {
        let _ = tx.send(Event::Facets(facets(&kept.facets, adult)));
        return;
    }
    let answer = fetch(&meta, &store, now).await;
    let facets = match (answer, kept) {
        (Some(fresh), _) => facets(&fresh, adult),
        (None, Some(kept)) => facets(&kept.facets, adult),
        (None, None) => SearchFacets {
            genres: GENRES.iter().map(|g| (*g).to_owned()).collect(),
            ..SearchFacets::default()
        },
    };
    let _ = tx.send(Event::Facets(facets));
}

async fn fetch(meta: &Meta, store: &Store, now: i64) -> Option<Facets> {
    match meta.facets().await {
        Ok(facets) => {
            let kept = KeptFacets { at: now, facets };
            if let Ok(json) = serde_json::to_string(&kept)
                && let Err(e) = store.set_meta_text(FACETS_KEY, &json)
            {
                tracing::warn!(error = %e, "could not keep the search facets");
            }
            Some(kept.facets)
        }
        Err(e) => {
            tracing::warn!(error = %e, "the search facets failed");
            None
        }
    }
}

/// AniList's facets as the filters offer them: tags by the first part of their category,
/// the common categories first and each one's tags by name, and the adult ones only when
/// adult titles are shown.
pub fn facets(from: &Facets, adult: bool) -> SearchFacets {
    const ORDER: [&str; 5] = ["Theme", "Setting", "Cast", "Demographic", "Technical"];
    let genres =
        from.genres.iter().filter(|g| adult || g.as_str() != ADULT_GENRE).cloned().collect();
    let mut tag_groups: Vec<TagGroup> = Vec::new();
    for tag in &from.tags {
        if !adult && (tag.adult || tag.category == ADULT_TAG_CATEGORY) {
            continue;
        }
        let category = tag.category.split('-').next().unwrap_or_default().trim();
        let category = if category.is_empty() { "Other" } else { category };
        let facet = FacetTag {
            name: tag.name.clone(),
            description: tag.description.clone(),
            spoiler: tag.spoiler,
        };
        match tag_groups.iter_mut().find(|g| g.category == category) {
            Some(group) => group.tags.push(facet),
            None => {
                tag_groups.push(TagGroup { category: category.to_owned(), tags: vec![facet] })
            }
        }
    }
    for group in &mut tag_groups {
        group.tags.sort_by_key(|tag| tag.name.to_lowercase());
    }
    tag_groups.sort_by_key(|g| {
        (ORDER.iter().position(|c| *c == g.category).unwrap_or(ORDER.len()), g.category.clone())
    });
    SearchFacets { genres, tag_groups, streaming_sites: from.streaming.clone() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_meta::FacetTag as Tag;

    #[test]
    fn the_rails_follow_the_season_into_the_next_year() {
        let rails = rails(Season::Fall, 2026);
        let titles: Vec<_> = rails.iter().map(|(t, d, _)| (t.as_str(), d.as_deref())).collect();
        assert_eq!(
            titles,
            [
                ("Trending now", None),
                ("Popular this season", Some("Fall 2026")),
                ("Upcoming next season", Some("Winter 2027")),
                ("All-time popular", None),
                ("Top rated", None),
            ]
        );
        assert_eq!(rails[2].2, SearchQuery::season(Season::Winter, 2027));
    }

    #[test]
    fn adult_genres_and_tags_show_only_when_asked_for() {
        let tag = |name: &str, category: &str, adult| Tag {
            name: name.into(),
            category: category.into(),
            adult,
            ..Tag::default()
        };
        let from = Facets {
            genres: vec!["Mecha".into(), "Hentai".into()],
            tags: vec![
                tag("Time Travel", "Theme-Sci-Fi", false),
                tag("Nudity", "Sexual Content", true),
                tag("Androids", "Cast-Traits", false),
                tag("Aliens", "Theme-Sci-Fi", false),
            ],
            streaming: vec!["Hikari Stream".into()],
        };
        let shown = facets(&from, false);
        assert_eq!(shown.genres, ["Mecha"]);
        let groups: Vec<(&str, Vec<&str>)> = shown
            .tag_groups
            .iter()
            .map(|g| (g.category.as_str(), g.tags.iter().map(|t| t.name.as_str()).collect()))
            .collect();
        assert_eq!(
            groups,
            [("Theme", vec!["Aliens", "Time Travel"]), ("Cast", vec!["Androids"])]
        );
        let all = facets(&from, true);
        assert_eq!(all.genres, ["Mecha", "Hentai"]);
        assert_eq!(all.tag_groups.len(), 3);
    }
}
