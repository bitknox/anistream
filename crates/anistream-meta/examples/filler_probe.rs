//! Does the filler parser survive the real page?
//!
//! ```text
//! cargo run -p anistream-meta --example filler_probe
//! ```
//!
//! AnimeFillerList is parsed HTML with its own slugs, so this checks two things a unit test on
//! recorded markup cannot: that the slug guess resolves for real shows, and that the live markup
//! still has the shape the parser expects. A silent shape change would show up here as a show with
//! zero classified episodes rather than as an error.

use anistream_meta::filler;

/// Long-runners where filler actually matters, plus a control that should have none.
const SHOWS: &[&str] = &["One Piece", "Naruto: Shippuden", "Bleach", "Sousou no Frieren"];

#[tokio::main]
async fn main() {
    let client = reqwest::Client::builder()
        // The site serves a different page to something that looks like a bot.
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("client");

    println!(
        "{:<22} {:>6} {:>10} {:>7} {:>7} {:>7}  slug",
        "title", "http", "classified", "filler", "mixed", "canon"
    );
    println!("{}", "─".repeat(88));

    // The index first, because a derived slug is not good enough: AnimeFillerList indexes by
    // English title while AniList's primary title is romaji.
    let index_html = client
        .get(filler::INDEX_URL)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .ok();
    let index = match index_html {
        Some(response) => filler::parse_index(&response.text().await.unwrap_or_default()),
        None => Vec::new(),
    };
    println!("index: {} shows from {}", index.len(), filler::INDEX_URL);

    // Romaji titles, exactly as AniList would hand them over. A derived slug gets all of these
    // wrong; the index gets them right.
    println!();
    println!("── romaji → slug, via the index ───────────────────────");
    for romaji in [
        "Boku no Hero Academia",
        "Tate no Yuusha no Nariagari",
        "Toaru Majutsu no Index",
        "One Piece",
        "Sousou no Frieren",
    ] {
        let matched = filler::match_index(&[romaji.to_string()], &index);
        let derived = filler::slug_for(romaji);
        let via_index = matched.map_or("—", |e| e.slug.as_str());
        let agree = via_index == derived;
        println!(
            "  {romaji:<30} index={via_index:<34} derived={derived:<30} {}",
            if matched.is_none() {
                "not covered"
            } else if agree {
                "(same)"
            } else {
                "← derived would have failed"
            }
        );
    }

    println!();
    println!("── episode classification ─────────────────────────────");
    for title in SHOWS {
        // Index match first, slug derivation only as the fallback.
        let slug = filler::match_index(&[(*title).to_string()], &index)
            .map(|e| e.slug.clone())
            .unwrap_or_else(|| filler::slug_for(title));
        let url = filler::show_url(&slug);

        let (status, list) = match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                (status, filler::parse(&body))
            }
            Err(e) => {
                println!("{title:<22} ✕ {e}");
                continue;
            }
        };

        println!(
            "{title:<22} {status:>6} {:>10} {:>7} {:>7} {:>7}  {slug}",
            list.classified(),
            list.filler.len(),
            list.mixed.len(),
            list.manga_canon.len() + list.anime_canon.len(),
        );

        // A 200 with nothing classified means the markup moved and the parser is now silently
        // useless — the failure mode worth surfacing loudly.
        if status == 200 && list.is_empty() {
            println!("  ▲ 200 but nothing parsed — the page shape has probably changed");
        }
        if !list.filler.is_empty() {
            let sample: Vec<String> = list.filler.iter().take(8).map(u32::to_string).collect();
            println!("      filler: {} …", sample.join(", "));
        }
        // The distinction that matters most: mixed episodes must never be offered as skippable.
        if let Some(first_mixed) = list.mixed.iter().next() {
            println!(
                "      ep {first_mixed} is mixed → skippable: {} (must be false)",
                list.is_skippable(*first_mixed)
            );
        }
    }
}
