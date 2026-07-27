//! Refresh the mapping datasets for real and report what it cost.
//!
//! Verifies the parts unit tests cannot: that both corpora still parse despite their
//! differing field typing, that the ETag round trip actually produces a zero-byte `304`,
//! and that merging them yields the coverage the design assumed.
//!
//! `cargo run -p anistream-meta --example dataset_probe`

use anistream_core::{config::NetworkConfig, ids::AnilistId};
use anistream_meta::dataset::{MAPPING_DATASETS, RefreshOutcome, refresh, refresh_all};
use anistream_net::HttpClient;
use anistream_store::Store;

#[tokio::main]
async fn main() {
    let http = HttpClient::new(&NetworkConfig::default()).expect("http client");
    let store = Store::open_in_memory().expect("store");
    let now = 1_800_000_000_i64;

    println!("── first refresh (cold, no ETag) ──────────────────────");
    let started = std::time::Instant::now();
    for (name, outcome) in refresh_all(&store, &http, now, false).await {
        println!("  {name:<12} {outcome:?}");
    }
    println!("  elapsed {:?}", started.elapsed());
    println!("  merged mapping rows: {}", store.mapping_count().unwrap());

    println!();
    println!("── second refresh, forced (ETag should give 304) ──────");
    for spec in MAPPING_DATASETS {
        let started = std::time::Instant::now();
        let outcome = refresh(&store, &http, spec, now, true).await;
        let note = match &outcome {
            RefreshOutcome::Unchanged => "no bytes transferred",
            RefreshOutcome::Updated { .. } => "changed since first fetch",
            _ => "",
        };
        println!("  {:<12} {outcome:?} in {:?}  {note}", spec.name, started.elapsed());
    }

    println!();
    println!("── cadence gating (unforced, immediately after) ───────");
    for (name, outcome) in refresh_all(&store, &http, now, false).await {
        println!("  {name:<12} {outcome:?}");
    }

    println!();
    println!("── merged coverage ────────────────────────────────────");
    let frieren = AnilistId::new(154_587);
    match store.mapping_for(frieren).unwrap() {
        Some(m) => {
            println!("  Frieren (anilist {})", m.anilist_id);
            println!("    mal_id         {:?}   \u{2190} aniskip needs this", m.mal_id);
            println!("    kitsu_id       {:?}", m.kitsu_id);
            println!("    anidb_id       {:?}", m.anidb_id);
            println!("    tvdb_id        {:?}", m.tvdb_id);
            println!("    tmdb_id        {:?}", m.tmdb_id);
            println!("    episode_offset {:?}", m.episode_offset);
        }
        None => println!("  Frieren not present — merge produced nothing"),
    }

    // A title that only exists in one corpus proves the merge is a union, not a
    // replacement: whichever source ran last must not have erased the other.
    println!();
    println!("── reverse lookup ─────────────────────────────────────");
    println!("  mal 52991 → {:?}", store.anilist_id_for_mal(52_991).unwrap());

    println!();
    println!("── entries carrying an episode offset (Fribb only) ────");
    let with_offset = store.mappings_with_offset_count().unwrap_or(0);
    println!("  {with_offset} rows (split-cour / continuous numbering cases)");
}
