//! Read-only provider probe: QUERY [--resolve] [--dub].
//! Resolution fetches HLS playlists only, never video segments.
use anistream_core::{config::NetworkConfig, media::Translation, traits::Provider};
use anistream_net::HttpClient;
use anistream_providers::ani_cli::AniCliProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = std::env::args().nth(1).ok_or("supply a search query")?;
    let http = HttpClient::new(&NetworkConfig::default())?;
    let provider = AniCliProvider::new(http.clone());
    let hits = provider.search(&query, Translation::Sub).await?;
    println!("search results: {}", hits.len());
    let hit = hits.first().ok_or("no search results")?;
    let episodes = provider.episodes(&hit.key, Translation::Sub).await?;
    println!("first result episode count: {}", episodes.len());
    let episode = episodes.first().ok_or("no episodes")?;
    if !std::env::args().any(|arg| arg == "--resolve") {
        return Ok(());
    }
    let translation = if std::env::args().any(|arg| arg == "--dub") {
        Translation::Dub
    } else {
        Translation::Sub
    };
    let streams = provider
        .resolve(&hit.key, episode.number.as_str(), translation)
        .await
        .map_err(|error| format!("{translation:?} resolution failed: {error}"))?;
    println!("{translation:?}: {} streams", streams.len());
    if streams.is_empty() {
        return Err("no streams".into());
    }
    for stream in streams {
        let mut request = http.plain().get(&stream.url);
        for (name, value) in &stream.headers {
            request = request.header(name, value);
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        println!(
            "playlist HTTP {status}, HLS header: {}, subtitles: {}",
            body.trim_start().starts_with("#EXTM3U"),
            stream.subtitles.len()
        );
        if !status.is_success() || !body.trim_start().starts_with("#EXTM3U") {
            return Err("stream did not return an HLS playlist".into());
        }
    }
    Ok(())
}
