//! The handles every effect shares, and how they are built.

use anistream_core::config::Config;
use anistream_meta::{Meta, Tenrai, anilist::AniList};
use anistream_net::HttpClient;
use anistream_store::Store;

/// The metadata source: AniList first, Tenrai when it fails, the cache when both do.
///
/// Built identically at every entry point — the TUI, `--search`, `--play`, previews —
/// so "anistream said X" never depends on which door was used to ask.
pub fn build_meta(config: &Config, http: &HttpClient, store: &Store) -> Meta {
    let anilist = AniList::new(http.clone(), config.network.anilist_rate_limit);
    let tenrai = config.meta.fallback.then(|| Tenrai::new(http.clone(), store.clone()));
    Meta::new(anilist, tenrai, store.clone())
}
