//! The recent searches: kept by the engine with their filters, so every front end on the
//! device lists the same ones, newest first.

use anistream_model::{event::Event, search::SearchQuery};
use anistream_store::Store;

use crate::{outbox::Outbox, services::Services};

/// Where they are kept.
const KEY: &str = "search:recent";
/// How many are kept.
const KEEP: usize = 8;

/// The recent searches, newest first. None kept, or unreadable, is none.
pub fn recent(store: &Store) -> Vec<SearchQuery> {
    store
        .get_meta_text(KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save(recent: &[SearchQuery], services: &Services, tx: &Outbox) {
    match serde_json::to_string(recent) {
        Ok(json) => {
            if let Err(e) = services.store.set_meta_text(KEY, &json) {
                tracing::warn!(error = %e, "could not keep the recent searches");
            }
        }
        Err(e) => tracing::warn!(error = %e, "could not write the recent searches"),
    }
    let _ = tx.send(Event::RecentSearches(recent.to_vec()));
}

/// Answer with the recent searches.
pub fn publish(services: &Services, tx: &Outbox) {
    let _ = tx.send(Event::RecentSearches(recent(&services.store)));
}

/// Put `query` first, once: searched again, it moves up rather than being listed twice.
pub fn remember(query: SearchQuery, services: &Services, tx: &Outbox) {
    let query = SearchQuery { text: query.trimmed_text().to_owned(), ..query };
    if query.is_blank() {
        return;
    }
    let mut kept = recent(&services.store);
    kept.retain(|q| *q != query);
    kept.insert(0, query);
    kept.truncate(KEEP);
    save(&kept, services, tx);
}

/// Take one search off the list.
pub fn forget(query: SearchQuery, services: &Services, tx: &Outbox) {
    let query = SearchQuery { text: query.trimmed_text().to_owned(), ..query };
    let mut kept = recent(&services.store);
    kept.retain(|q| *q != query);
    save(&kept, services, tx);
}

/// Take every search off the list.
pub fn clear(services: &Services, tx: &Outbox) {
    save(&[], services, tx);
}
