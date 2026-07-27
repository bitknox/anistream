//! Exercise the AniList client against the live API.
//!
//! Unit tests cover decoding against recorded payloads; this checks the parts they cannot —
//! that the queries are actually valid GraphQL, that the field names match the live schema,
//! and that the rate limiter reads real headers.
//!
//! `cargo run -p anistream-meta --example anilist_probe`

use anistream_core::{config::NetworkConfig, ids::AnilistId};
use anistream_meta::anilist::{AniList, BrowseFilter, Season};
use anistream_net::HttpClient;

#[tokio::main]
async fn main() {
    let http = HttpClient::new(&NetworkConfig::default()).expect("http client");
    let client = AniList::new(http, 30);

    println!("── search ─────────────────────────────────────────────");
    match client.search("frieren", 1, 3).await {
        Ok(page) => {
            for m in &page.items {
                println!(
                    "  {:<7} {:<42} {:>3} ep  {:?}",
                    m.id.get(),
                    truncate(m.title.display(), 42),
                    m.episodes.unwrap_or(0),
                    m.format
                );
            }
            println!("  has_next={}", page.has_next);
        }
        Err(e) => println!("  FAILED: {e}"),
    }

    println!();
    println!("── title detail (relations, links, deep links) ─────────");
    match client.media(AnilistId::new(154_587)).await {
        Ok(m) => {
            println!("  {}", m.title.display());
            println!("  mal_id       {:?}  (needed for aniskip)", m.id_mal);
            println!("  score        {:?}", m.average_score);
            println!("  genres       {}", m.genres.join(", "));
            println!("  cover        {}", m.cover_image.best().unwrap_or("none"));
            println!("  banner       {}", m.banner_image.as_deref().unwrap_or("none"));
            println!("  synopsis     {}", truncate(&m.plain_description(), 60));
            println!("  streaming on {}", {
                let s: Vec<&str> =
                    m.streaming_services().iter().map(|l| l.site.as_str()).collect();
                if s.is_empty() { "nothing listed".into() } else { s.join(", ") }
            });
            println!("  deep links   {} episodes", m.streaming_episodes.len());
            if let Some(ep) = m.episode_link(1) {
                println!("    ep 1 → {}", ep.url.as_deref().unwrap_or("?"));
            }
            let t = m.match_target();
            println!(
                "  match gates  {} titles, {:?} eps, {:?}, {:?}",
                t.titles.len(),
                t.episode_count,
                t.year,
                t.format
            );
        }
        Err(e) => println!("  FAILED: {e}"),
    }

    println!();
    println!("── seasonal, server-side filtered ─────────────────────");
    let filter = BrowseFilter {
        genres: vec!["Fantasy".into()],
        min_score: Some(70),
        ..Default::default()
    };
    match client.seasonal(Season::Summer, 2026, &filter, 1, 5).await {
        Ok(page) => {
            for m in &page.items {
                println!(
                    "  {:<44} {:>3}  {}",
                    truncate(m.title.display(), 44),
                    m.average_score.unwrap_or(0),
                    m.genres.join("/")
                );
            }
        }
        Err(e) => println!("  FAILED: {e}"),
    }

    println!();
    println!("── airing calendar (next 24h) ─────────────────────────");
    let now =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()
            as i64;
    match client.airing_between(now, now + 86_400, 1, 5).await {
        Ok(page) => {
            for e in &page.items {
                let mins = (e.airing_at - now) / 60;
                println!(
                    "  in {:>4}m  ep {:<4} {}",
                    mins,
                    e.episode,
                    truncate(e.media.title.display(), 44)
                );
            }
            if page.items.is_empty() {
                println!("  (nothing airing in the next 24h)");
            }
        }
        Err(e) => println!("  FAILED: {e}"),
    }

    println!();
    println!("── rate limiting: 8 rapid requests ────────────────────");
    let start = std::time::Instant::now();
    let mut ok = 0;
    for _ in 0..8 {
        if client.search("a", 1, 1).await.is_ok() {
            ok += 1;
        }
    }
    println!("  {ok}/8 succeeded in {:?} (limiter paced them)", start.elapsed());
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_owned()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}
