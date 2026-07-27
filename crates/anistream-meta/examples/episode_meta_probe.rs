//! What do the streaming listings actually carry?
//!
//! `cargo run -p anistream-meta --example episode_meta_probe`

use anistream_meta::anilist::AniList;

#[tokio::main]
async fn main() {
    let http = anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
        .expect("http client");
    let anilist = AniList::new(http, 60);
    for (id, name) in [(154_587u32, "Frieren"), (21u32, "One Piece"), (176_301u32, "Dandadan S2")] {
        let id = anistream_core::ids::AnilistId::new(id);
        match anilist.media(id).await {
            Ok(media) => {
                println!("\n── {name} (anilist {}) ──", id.get());
                println!("  streamingEpisodes entries: {}", media.streaming_episodes.len());
                for listing in media.streaming_episodes.iter().take(4) {
                    println!(
                        "    title={:?}\n      -> number={:?} episode_title={:?} thumb={:?}",
                        listing.title,
                        listing.episode_number(),
                        listing.episode_title(),
                        listing.thumbnail.as_deref().map(|t| &t[..t.len().min(48)]),
                    );
                }
                let titles = media.episode_titles();
                let thumbs = media.episode_thumbnails();
                println!(
                    "  aligned titles: {} entries, keys {:?}..{:?}",
                    titles.len(),
                    titles.keys().next(),
                    titles.keys().next_back()
                );
                println!("  aligned thumbnails: {} entries", thumbs.len());
                println!("  entry episode count: {:?}", media.episodes);
                if let Some((n, t)) = titles.iter().next() {
                    println!("    first: ep {n} = {t:?}");
                }
            }
            Err(e) => println!("\n── {name}: FAILED: {e}"),
        }
    }
}
