//! Why "play" does nothing: the episode-listing chain, stage by stage.
//!
//! ```text
//! cargo run -p anistream --example episodes_probe            # a default title
//! cargo run -p anistream --example episodes_probe -- 154587  # any AniList id
//! ```
//!
//! Reported from real use: pressing play produced neither playback nor an error. Every stage
//! between a title and a playable stream can come back *empty* rather than failing — an empty
//! provider list, an empty episode list, an empty stream list — and an empty result is not an
//! error, so nothing is raised and the screen simply sits there. This probe prints each stage's
//! count so the silent one is visible.

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    media::Translation,
};
use anistream_net::HttpClient;
use anistream_store::Store;

/// Frieren — present in both mapping corpora and covered by curation.
const DEFAULT: u32 = 154_587;

#[tokio::main]
async fn main() {
    let id =
        AnilistId::new(std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(DEFAULT));
    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).expect("config");
    let store = Store::open(paths.database()).expect("store");
    let http = HttpClient::new(&config.network).expect("http");
    let anilist =
        anistream_meta::anilist::AniList::new(http.clone(), config.network.anilist_rate_limit);

    println!("── registry ───────────────────────────────────────────");
    let (registry, guard, note) =
        anistream::sources::build_registry(&config, &http, &paths).await;
    println!("  providers    {:?}", registry.ids());
    println!("  vpn guard    {}", if guard.is_some() { "running" } else { "not running" });
    if let Some(note) = &note {
        println!("  note         {note}");
    }
    if registry.is_empty() {
        println!("  ✕ nothing registered — the UI would say 'no sources configured'");
        return;
    }

    println!();
    println!("── title ──────────────────────────────────────────────");
    let media = match anilist.media(id).await {
        Ok(m) => m,
        Err(e) => {
            println!("  ✕ {e}");
            return;
        }
    };
    let target = media.match_target();
    println!("  {}", media.title.display());
    println!("  episodes     {:?}", media.episodes);
    println!("  match on     {:?}", target.titles);

    println!();
    println!("── mapping ────────────────────────────────────────────");
    let now = anistream_store::now();
    let resolution =
        anistream_providers::resolve(&store, &registry, id, &target, Translation::Sub, now)
            .await;
    println!("  rung         {}", resolution.explain());
    let Some(key) = resolution.key().cloned() else {
        println!("  ✕ unmatched — the UI raises 'could not match this title'");
        return;
    };
    println!("  provider key {key:?}");

    println!();
    println!("── episodes ───────────────────────────────────────────");
    let attempt = registry.episodes(&key, Translation::Sub, now).await;
    println!("  summary      {}", attempt.summary());
    let episodes = match attempt.value {
        Some(list) => list,
        None => {
            println!("  ✕ every provider failed — this one *does* raise a toast");
            return;
        }
    };
    println!("  count        {}", episodes.len());
    if episodes.is_empty() {
        println!();
        println!("  ✕ THIS IS THE SILENT CASE.");
        println!("    An empty list is a successful answer, so nothing is raised: the table");
        println!("    renders its empty state and Enter has no row to act on.");
        return;
    }
    for episode in episodes.iter().take(5) {
        println!("    ep {:>4}  {:?}", episode.number.as_str(), episode.title);
    }

    println!();
    println!("── resolve (first episode) ────────────────────────────");
    let first = &episodes[0];
    let attempt = registry.resolve(&key, first.number.as_str(), Translation::Sub, now).await;
    println!("  summary      {}", attempt.summary());
    match attempt.value {
        Some(streams) if streams.is_empty() => {
            println!("  ✕ resolved to an empty stream list — silent for the same reason");
        }
        Some(streams) => {
            println!("  streams      {}", streams.len());
            for stream in streams.iter().take(3) {
                println!("    {:?}  {:?}  {}", stream.kind, stream.quality, stream.url);
            }
            println!();
            println!("  ● the chain is intact end to end");
        }
        None => println!("  ✕ every provider failed to resolve"),
    }
}
