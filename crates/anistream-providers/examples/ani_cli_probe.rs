//! Read-only provider metadata probe. Usage: cargo run -p anistream-providers --example ani_cli_probe -- QUERY
use anistream_core::{config::NetworkConfig, media::Translation, traits::Provider};
use anistream_net::HttpClient;
use anistream_providers::ani_cli::AniCliProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = std::env::args().nth(1).ok_or("supply a search query")?;
    let provider = AniCliProvider::new(HttpClient::new(&NetworkConfig::default())?);
    let hits = provider.search(&query, Translation::Sub).await?;
    println!("search results: {}", hits.len());
    if let Some(hit) = hits.first() {
        let episodes = provider.episodes(&hit.key, Translation::Sub).await?;
        println!("first result episode count: {}", episodes.len());
    }
    Ok(())
}
