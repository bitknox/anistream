//! Exercise a configured indexer against the live service.
//!
//! anistream ships no indexer, so this probe takes yours:
//!
//! ```sh
//! cargo run -p anistream-providers --example torrent_probe -- \
//!     'https://your-indexer.example/?page=rss&q={query}' \
//!     'https://your-curation.example/api?alID={anilist_id}'   # optional
//! ```
//!
//! Unit tests cover parsing against captured fixtures; this checks what they cannot — that
//! a real feed has the shape assumed, and how the release parser copes with titles nobody
//! wrote by hand. The parser is the fiddly part of the torrent path, so seeing it run over
//! a hundred real titles is worth more than any number of invented cases.

use anistream_providers::torrent::{curation, indexer};

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(rss_template) = args.next() else {
        eprintln!(
            "usage: torrent_probe <rss-url-with-{{query}}> [curation-url-with-{{anilist_id}}]"
        );
        eprintln!("anistream ships no indexer; supply your own endpoint.");
        std::process::exit(2);
    };
    let curation_template = args.next();

    let client = reqwest::Client::builder()
        .user_agent("anistream/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("client");

    println!("── indexer rss ────────────────────────────────────────");
    let url = indexer::search_url(&rss_template, "Frieren", Some(1080));
    println!("  {url}");

    let items = match client.get(&url).send().await {
        Ok(response) => {
            println!("  HTTP {}", response.status().as_u16());
            let body = response.text().await.unwrap_or_default();
            let items = indexer::parse_feed(&body);
            println!("  {} items parsed from {} bytes", items.len(), body.len());
            items
        }
        Err(e) => {
            println!("  FAILED: {e}");
            Vec::new()
        }
    };

    println!();
    println!("── release parsing over real titles ───────────────────");
    let mut with_episode = 0;
    let mut batches = 0;
    let mut with_quality = 0;
    let mut with_group = 0;

    for item in items.iter().take(12) {
        let r = &item.release;
        println!(
            "  {:>4}s  {:<10} ep {:<7} {:<7} {}",
            item.seeders,
            truncate(r.group.as_deref().unwrap_or("-"), 10),
            r.episode.map_or_else(
                || r.batch.map_or("-".into(), |(a, b)| format!("{a}-{b}")),
                |e| e.to_string()
            ),
            r.quality.map_or("-".into(), |q| format!("{q}p")),
            truncate(&item.title, 58),
        );
    }
    for item in &items {
        with_episode += usize::from(item.release.episode.is_some());
        batches += usize::from(item.release.is_batch());
        with_quality += usize::from(item.release.quality.is_some());
        with_group += usize::from(item.release.group.is_some());
    }
    if !items.is_empty() {
        println!();
        println!("  coverage over {} titles:", items.len());
        println!("    group recognised    {with_group}/{}", items.len());
        println!("    quality recognised  {with_quality}/{}", items.len());
        println!("    single episode      {with_episode}");
        println!("    batches             {batches}");
        println!("    unclassified        {}", items.len() - with_episode - batches);
    }

    println!();
    println!("── magnet construction ────────────────────────────────");
    let trackers: Vec<String> = std::env::var("ANISTREAM_TRACKERS")
        .map(|v| v.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    match items.iter().find_map(|item| item.magnet(&trackers)) {
        Some(magnet) => println!("  {}", truncate(&magnet, 100)),
        None => println!("  no item reported an info hash"),
    }

    println!();
    println!("── curation, keyed on anilist id ──────────────────────");
    // Frieren. Keyed directly on the AniList id, with no mapping step.
    let Some(curation_template) = curation_template.as_deref() else {
        println!("  (no curation url given — skipped)");
        return;
    };
    let indexer_host = rss_template
        .split_once("://")
        .and_then(|(_, rest)| rest.split(['/', '?', '#']).next())
        .map(str::to_owned);
    let curation_url = curation::query_url(curation_template, 154_587);
    match client.get(&curation_url).send().await {
        Ok(response) => {
            println!("  HTTP {}", response.status().as_u16());
            let body = response.text().await.unwrap_or_default();
            let releases = curation::parse(&body, indexer_host.as_deref());
            println!("  {} public release(s)", releases.len());
            for r in &releases {
                println!(
                    "    {:<12} best={:<5} dual={:<5} {}",
                    truncate(&r.group, 12),
                    r.is_best,
                    r.dual_audio,
                    r.url
                );
            }
            if let Some(best) = curation::best(&releases, true) {
                println!("  curated pick: {} → {}", best.group, best.url);
                if let Some(notes) = &best.notes {
                    println!("  notes: {}", truncate(notes, 70));
                }
            }
        }
        Err(e) => println!("  FAILED: {e}"),
    }

    println!();
    println!("── end-to-end selection ───────────────────────────────");
    for episode in [1u32, 12] {
        match indexer::best(&items, episode, None, 1080, false) {
            Some(pick) => println!(
                "  ep {episode:<3} → {:>4} seeders  {}",
                pick.seeders,
                truncate(&pick.title, 68)
            ),
            None => println!("  ep {episode:<3} → nothing in this feed covers it"),
        }
    }

    // The parser is the component most likely to drift as release conventions change.
    let unclassified = items.len().saturating_sub(with_episode + batches);
    if !items.is_empty() && unclassified * 2 > items.len() {
        println!();
        println!(
            "  ⚠ {unclassified}/{} titles were unclassified — the release parser may need \
             extending against current conventions",
            items.len()
        );
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}
