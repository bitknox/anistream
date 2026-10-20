//! Search, and browsing by filter when nothing is typed.

use anistream_model::{
    catalog::{Content, ContentFor, SearchFilters},
    event::Event,
};

use crate::{catalog::lists::publish_list, data, dispatch::Lent, now_epoch};

/// A search, or a browse when the query is empty, captioned when one page did not hold it.
pub async fn search(lent: Lent, query: String, filter: SearchFilters) {
    let Lent { meta, store, tx, .. } = lent;
    let view = ContentFor::Search { query: query.clone(), filter: filter.clone() };
    let browse = anistream_meta::anilist::BrowseFilter {
        genres: filter.genre.iter().cloned().collect(),
        format: filter.format.clone(),
        status: filter.status.clone(),
        min_score: None,
        sort: filter.sort.clone(),
        season: filter.season.clone(),
        year: filter.year,
    };
    const PAGE: u32 = 30;
    let update = match meta.search(&query, &browse, 1, PAGE).await {
        Ok(page) => {
            // Sent every time, empty when the page held everything, so a caption
            // from an earlier run of the same search cannot outlive its answer.
            let text =
                if page.has_next { format!("showing the first {PAGE}") } else { String::new() };
            let _ = tx.send(Event::Caption { view: view.clone(), text });
            let entries =
                page.items.iter().map(|m| data::entry_from(m, Some(&store))).collect();
            publish_list(&meta, &tx, view, entries, now_epoch()).await;
            return;
        }
        Err(e) => Event::Content { view, content: Content::Failed(e.to_string()) },
    };
    let _ = tx.send(update);
}
