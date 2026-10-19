//! `anistream --search`: AniList searched from the shell, as a table or as JSON.

use anistream_core::config::Config;
use anistream_net::HttpClient;
use anistream_store::Store;
use anyhow::Result;

use crate::build_meta;

/// Search AniList from the shell.
///
/// Titles are remembered as they pass through, so a later `--stats` or sync conflict can name a
/// show you have only searched for.
pub async fn search_cli(
    config: &Config,
    http: &HttpClient,
    store: &Store,
    query: &str,
    json: bool,
) -> Result<()> {
    let meta = build_meta(config, http, store);
    let page = meta.search(query, &Default::default(), 1, 20).await?;

    for media in &page.items {
        let _ = store.remember_title(media.id, media.title.display());
    }

    if json {
        // Built explicitly rather than serialising `Media`: this is a published surface, and
        // leaking the AniList response shape would make every upstream field change a breaking one.
        let rows: Vec<serde_json::Value> = page
            .items
            .iter()
            .map(|m| {
                serde_json::json!({
                    "anilist_id": m.id.get(),
                    "title": m.title.display(),
                    "romaji": m.title.romaji,
                    "english": m.title.english,
                    // AniList's own names, `TV_SHORT` included: a stable machine value, apart
                    // from however the interface labels it.
                    "format": m.format.and_then(|f| serde_json::to_value(f).ok()),
                    "episodes": m.episodes,
                    "year": m.season_year,
                    "score": m.average_score,
                    "progress": store.progress(m.id).ok().flatten().map(|p| p.episodes_done),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }

    println!("{:<9} {:>4} {:>5} {:>4}  title", "id", "eps", "year", "%");
    println!("{}", "─".repeat(70));
    for media in &page.items {
        println!(
            "{:<9} {:>4} {:>5} {:>4}  {}",
            media.id.get(),
            media.episodes.map_or("—".into(), |e| e.to_string()),
            media.season_year.map_or("—".into(), |y| y.to_string()),
            media.average_score.map_or("—".into(), |s| s.to_string()),
            media.title.display()
        );
    }
    Ok(())
}
