//! The engine on the test kit's scene, for screenshots and for trying the app with nothing set
//! up: the fake AniList on loopback with the fixture titles, signed in so the Library fills, and
//! four titles under way, as the phones' Fixtures builds show them. Every episode streams the
//! clip a run hands in. Nothing leaves the machine: the client cannot resolve a host name.

use std::{path::Path, sync::Arc};

use anistream_core::{
    config::NetworkConfig,
    stream::{Stream, StreamKind},
};
use anistream_engine::{
    clock::Clock,
    engine::Engine,
    host::Host,
    options::{AppClients, Background, EngineOptions, Parts},
    plugin_stores, tracking,
};
use anistream_meta::Meta;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::{DownloadState, Store};
use anistream_testkit::{
    FakeAniList,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    seed, sources,
};
use anistream_track::{
    AniListTracker,
    secret::{Backend, TokenStore},
};

use crate::engine::{Started, open, paths_under, theme};

/// What the scene starts with beyond its history.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// A file every episode streams, for the player.
    pub media: Option<String>,
    /// An episode downloading, so closing the window asks first.
    pub downloading: bool,
}

/// Start the engine on the scene, its files under `root`.
pub async fn start(root: &Path, scene: &Scene, host: Arc<dyn Host>) -> Result<Started, String> {
    let paths = paths_under(root);
    write_config(&paths.config_file)?;
    let (config, store) = open(&paths)?;
    let clock = Clock::default();
    let now = clock.now();
    seed_history(&store, now);
    if scene.downloading && store.download_for(AIRING, "4").ok().flatten().is_none() {
        // A queue with a row in every state.
        seed::download(&store, FINISHED, "1", DownloadState::Done);
        seed::download(&store, FINISHED, "2", DownloadState::Done);
        seed::download(&store, AIRING, "4", DownloadState::Active);
        seed::download(&store, AIRING, "5", DownloadState::Queued);
        seed::download(&store, ONA, "3", DownloadState::Paused);
        seed::download(&store, AIRING_NO_MAL, "2", DownloadState::Failed);
    }
    let tokens = TokenStore::with_backend(paths.data_dir.join("tokens"), Backend::File);
    tokens.set("anilist", "fixture-anilist-token").map_err(|e| e.to_string())?;

    let server = FakeAniList::start().await;
    server.set_now(now);
    server.add_titles(fixtures::long_franchise());
    server.serve_art();
    let http =
        HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..config.network.clone() })
            .map_err(|e| e.to_string())?;
    let meta = Meta::new(server.anilist(&http), None, store.clone());
    let tracked = tracking::native::with_app_clients(&config, &AppClients::default());
    let mut sync = tracking::Sync::with_tokens(&tracked, &store, &http, tokens.clone());
    if let Some(at) = sync.trackers.iter().position(|t| t.id() == "anilist") {
        let client = server.anilist(&http).with_token(tokens.get("anilist").ok());
        let tracker = Arc::new(AniListTracker::new(client));
        sync.trackers[at] = tracker.clone();
        sync.anilist = Some(tracker);
    }
    let stores = plugin_stores::Stores::with_tokens(&config, &paths, tokens)?;

    let mut options = EngineOptions::new(paths, config.clone(), http, store, theme());
    options.saved = Some(config);
    options.hosted_player = true;
    options.background = Background::none();
    options.sources = Some(registry(&server.art_base(), scene.media.as_deref()));
    options.parts = Some(Parts { meta, sync, stores, clock });
    let (engine, events): (Engine, _) =
        Engine::start(options, host).await.map_err(|e| e.to_string())?;
    Ok(Started { engine, events, keep: Some(Box::new(server)) })
}

/// The fixture provider, its stills from the fake, every episode streaming `media` when given.
fn registry(art: &str, media: Option<&str>) -> ProviderRegistry {
    let mut provider = sources::with_long_franchise(sources::provider_with_art(art));
    if let Some(media) = media {
        let stream = Stream {
            provider_id: sources::PROVIDER_ID.into(),
            ..Stream::new(media, StreamKind::Mp4)
        };
        for id in fixtures::ALL {
            provider = provider.with_streams_for(sources::key(id), vec![stream.clone()]);
        }
    }
    ProviderRegistry::new(vec![provider.arc()])
}

/// AniList on, as a signed-in user has it, tokens in files, and skips offered rather than taken.
fn write_config(file: &Path) -> Result<(), String> {
    if file.exists() {
        return Ok(());
    }
    let dir = file.parent().ok_or("the config has no folder")?;
    std::fs::create_dir_all(dir)
        .and_then(|()| {
            std::fs::write(
                file,
                "[trackers]\nenabled = [\"anilist\"]\ntoken_storage = \"file\"\n\n\
                 [trackers.anilist]\nclient_id = \"fixture\"\nclient_secret = \"fixture\"\n\n\
                 [playback]\nskip_opening = false\nupscaling = \"off\"\n",
            )
        })
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// The phones' Watching scene: four titles under way, the newest an hour before `now`.
fn seed_history(store: &Store, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    if store.continue_list(1).is_ok_and(|rows| !rows.is_empty()) {
        return;
    }
    seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
    seed::watched(store, AIRING, 3, now - 2 * DAY);
    seed::watched(store, ONA, 1, now - DAY - HOUR);
    seed::stopped_at(store, ONA, "2", 13.0 * 60.0 + 30.0, now - DAY);
    seed::watched(store, FINISHED, 6, now - 2 * HOUR);
    seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_leaves_four_titles_to_continue_with_the_newest_first() {
        let root = tempfile::tempdir().expect("a scratch directory");
        let (_config, store) = open(&paths_under(root.path())).expect("it opens");
        seed_history(&store, 1_800_000_000);
        seed_history(&store, 1_800_000_000);
        let ids: Vec<_> =
            store.continue_list(15).expect("the rail").iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL], "seeded once, newest first");
    }
}
