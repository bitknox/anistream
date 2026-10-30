//! The engine on the test kit's scene, for screenshots and for trying the app with nothing set
//! up: the fake AniList on loopback with the fixture titles, signed in so the Library fills, and
//! four titles under way, as the phones' Fixtures builds show them. Every episode streams the
//! clip a run hands in. Nothing leaves the machine: the client cannot resolve a host name.

use std::{path::Path, sync::Arc};

use anistream_core::{
    config::NetworkConfig,
    media::Translation,
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
use anistream_store::{DownloadState, Store, WatchEvent};
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

/// Whether the app runs on the fixture scene, which has no network.
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the app runs on the fixture scene.
pub fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

/// The engine's runtime, for the scene's party server to run on.
static RUNTIME: std::sync::OnceLock<tokio::runtime::Handle> = std::sync::OnceLock::new();

/// Keep `handle`, the engine's runtime, for [`FakeParty::start`].
pub fn keep_runtime(handle: tokio::runtime::Handle) {
    ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = RUNTIME.set(handle);
}

/// A Syncplay server on loopback with members the scene plays, so a party can be joined,
/// steered and left with nothing on the network.
pub struct FakeParty {
    server: anistream_party::fake::FakeServer,
    members: std::cell::RefCell<
        std::collections::HashMap<String, anistream_party::fake::FakeMember>,
    >,
}

impl FakeParty {
    /// A server that takes every member, or refuses each with `refuse`.
    pub fn start(refuse: Option<&str>) -> Option<Self> {
        use anistream_party::fake::{FakeOptions, FakeServer};
        let handle = RUNTIME.get()?;
        let options =
            FakeOptions { refuse: refuse.map(str::to_owned), ..FakeOptions::default() };
        let server = handle
            .block_on(handle.spawn(FakeServer::start(options)))
            .ok()?
            .inspect_err(
                |e| tracing::warn!(error = %e, "the scene's party server did not start"),
            )
            .ok()?;
        Some(Self { server, members: Default::default() })
    }

    /// Where to join it, as `host:port`.
    pub fn server(&self) -> String {
        self.server.address()
    }

    fn with(&self, act: impl FnOnce(&anistream_party::fake::FakeServer)) {
        let Some(handle) = RUNTIME.get() else { return };
        let _entered = handle.enter();
        act(&self.server);
    }

    fn member(&self, name: &str, act: impl FnOnce(&anistream_party::fake::FakeMember)) {
        let Some(handle) = RUNTIME.get() else { return };
        let _entered = handle.enter();
        if let Some(member) = self.members.borrow().get(name) {
            act(member);
        }
    }

    /// `name` comes into `room`.
    pub fn add(&self, room: &str, name: &str) {
        let mut member = None;
        self.with(|server| member = Some(server.member(room, name)));
        if let Some(member) = member {
            self.members.borrow_mut().insert(name.to_owned(), member);
        }
    }

    /// The member opens `file`, `duration` seconds long.
    pub fn opens(&self, name: &str, file: &str, duration: f64) {
        self.member(name, |m| m.open(file, duration));
    }

    pub fn ready(&self, name: &str, ready: bool) {
        self.member(name, |m| m.ready(ready));
    }

    pub fn says(&self, name: &str, message: &str) {
        self.member(name, |m| m.chat(message));
    }

    /// The server goes away, as one lost does: every connection dropped and none taken.
    pub fn go_down(&self, down: bool) {
        self.with(|server| {
            server.set_down(down);
            if down {
                server.drop_connections();
            }
        });
    }
}

/// An address that takes a connection and never answers, for the form waiting on a room.
pub fn silent_server() -> Option<(std::net::TcpListener, String)> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    let address = listener.local_addr().ok()?.to_string();
    Some((listener, address))
}

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
    let (config, store, _) = open(&paths)?;
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
    let stores =
        plugin_stores::Stores::with_tokens(&config, &paths, tokens).map_err(|m| m.english())?;

    let mut options = EngineOptions::new(paths, config.clone(), http, store, theme());
    options.saved = Some(config);
    options.hosted_player = true;
    options.background = Background::none();
    let chain = registry(&server.art_base(), scene.media.as_deref());
    // A setting that changes which sources exist rebuilds the chain from the config, and the
    // scene's own source would be gone for every shot after it: it goes back in place.
    let (kept, providers) = (chain.clone(), chain.all());
    let rebuilt: Box<dyn Fn() + Send> = Box::new(move || kept.replace(providers.clone()));
    options.sources = Some(chain);
    options.parts = Some(Parts {
        meta,
        sync,
        stores,
        clock,
        fixture_exit: Some(anistream_providers::vpn::VpnGuard::fixture_egress()),
    });
    let (engine, events): (Engine, _) =
        Engine::start(options, host).await.map_err(|e| e.to_string())?;
    Ok(Started { engine, events, keep: Some(Box::new(server)), rebuilt: Some(rebuilt) })
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
    // Switched to the dub after episode 1, so its Continue row is marked "Dub".
    let dubbed = WatchEvent {
        duration_secs: Some(fixtures::runtime_secs(ONA)),
        watched_secs: 13.0 * 60.0 + 30.0,
        provider_id: Some(sources::PROVIDER_ID.to_owned()),
        translation: Some(Translation::Dub),
        completed: false,
        at: now - DAY,
        ..WatchEvent::new(ONA, "2", 13.0 * 60.0 + 30.0)
    };
    store.record_event(&dubbed).expect("seeding a dubbed episode");
    seed::watched(store, FINISHED, 6, now - 2 * HOUR);
    seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_leaves_four_titles_to_continue_with_the_newest_first() {
        let root = tempfile::tempdir().expect("a scratch directory");
        let (_config, store, _) = open(&paths_under(root.path())).expect("it opens");
        seed_history(&store, 1_800_000_000);
        seed_history(&store, 1_800_000_000);
        let ids: Vec<_> =
            store.continue_list(15).expect("the rail").iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL], "seeded once, newest first");
        let dubbed: Vec<_> =
            ids.iter().map(|id| store.last_translation(*id).expect("a read")).collect();
        let sub = Some(Translation::Sub);
        assert_eq!(dubbed, [sub, Some(Translation::Dub), sub, sub], "one watched dubbed");
    }
}
