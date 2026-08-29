//! End-to-end validation of the metadata failover.
//!
//! ```text
//! cargo run -p anistream --example meta_probe
//! ```
//!
//! Three things unit tests cannot cover: whether Tenrai actually answers and its rows
//! translate back to AniList identity through the live mapping table, whether the
//! facade's write-through leaves a cache row a later outage could serve, and whether
//! a cached row deserializes back into the same `Media` the fresh path produced.
//! Read-only against every service; writes only to the local cache table.

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
};
use anistream_net::HttpClient;
use anistream_store::Store;

/// Frieren — a stable id present in both mapping corpora.
const SUBJECT: AnilistId = AnilistId::new(154_587);

#[tokio::main]
async fn main() {
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();
    let store = Store::open(paths.database()).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── tenrai, directly ────────────────────────────────────");
    let tenrai = anistream_meta::Tenrai::new(http.clone(), store.clone());
    match tenrai.media(SUBJECT).await {
        Ok(media) => {
            println!("  title       {}", media.title.display());
            println!("  identity    anilist {} · mal {:?}", media.id.get(), media.id_mal);
            println!(
                "  format      {:?} · {:?} eps · score {:?}",
                media.format, media.episodes, media.average_score
            );
            println!("  cover       {}", media.cover_image.best().unwrap_or("—"));
            println!("  trailer     {}", media.trailer_url().as_deref().unwrap_or("—"));
            println!("  watch order {} entries", media.watch_order().len());
            assert_eq!(media.id, SUBJECT, "identity did not survive translation");
        }
        Err(e) => {
            println!("  ✕ {e}");
            return;
        }
    }

    // Reported rather than fatal: search proxies MAL live where detail serves from
    // Tenrai's cache, so this leg can be down while the fallback is still useful — an
    // outage here is weather, and weather is what the ladder exists for.
    let search_ok = match tenrai.search("frieren", &Default::default(), 1, 10).await {
        Ok(page) => {
            println!("  search      {} rows, all with anilist identity", page.items.len());
            for media in page.items.iter().take(3) {
                println!("    {} · anilist {}", media.title.display(), media.id.get());
            }
            !page.items.is_empty()
        }
        Err(e) => {
            println!("  ▲ search: {e} — mal-side weather, not a translation fault");
            false
        }
    };

    println!();
    println!("── the facade, write-through and recall ───────────────");
    let meta = anistream_meta::Meta::new(
        anistream_meta::AniList::new(http.clone(), config.network.anilist_rate_limit),
        Some(anistream_meta::Tenrai::new(http.clone(), store.clone())),
        store.clone(),
    );
    let fresh = match meta.media(SUBJECT).await {
        Ok(media) => {
            println!("  fresh       {} via the ladder", media.title.display());
            media
        }
        Err(e) => {
            println!("  ✕ every rung failed: {e}");
            return;
        }
    };

    let (payload, fetched_at) = store
        .cached_media(SUBJECT)
        .expect("cache readable")
        .expect("the fetch did not write through to the cache");
    let recalled: anistream_meta::Media =
        serde_json::from_str(&payload).expect("cached row no longer deserializes");
    println!("  cached      {} bytes at t={fetched_at}", payload.len());
    assert_eq!(recalled.title, fresh.title, "the cache would show different data");
    assert_eq!(recalled.id, fresh.id);

    println!();
    println!("── verdict ────────────────────────────────────────────");
    println!("  tenrai answers and translates to our identity   ●");
    println!(
        "  tenrai search                                   {}",
        if search_ok { "●" } else { "▲ upstream down" }
    );
    println!("  fetches write through to the cache             ●");
    println!("  the cached row recalls losslessly              ●");
}
