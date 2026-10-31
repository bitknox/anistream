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
    Failure, FakeAniList, Operation,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    home_world::{HomeClock, HomeWorld},
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

/// The environment variable that gives the scene Home's world: invented titles dated by the
/// clock, enough of them airing this season to fill every shelf. The same one the phones'
/// Fixtures builds read.
pub const HOME_WORLD_VAR: &str = "ANISTREAM_FIXTURES_HOME_WORLD";

/// Set by the offline scene, so the network path says offline as the device would.
pub static OFFLINE_SCENE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// How long a slow scene holds each of Home's discovery answers: long enough for the shots to
/// catch the placeholders and the activity line after four seconds, inside the wait the slow
/// scene gives the client.
const SLOW_ANSWER: std::time::Duration = std::time::Duration::from_secs(10);
/// How long the client waits for an answer: twice the slow answer in the slow scene.
const fn client_wait(scene: HomeScene) -> u64 {
    if matches!(scene, HomeScene::Slow) { 20 } else { 5 }
}

/// What Home and the app around it start from: the board's states, as the phones' Fixtures scenes
/// name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeScene {
    /// Signed in, four titles under way: case 1, Continue at 4.
    #[default]
    Watching,
    /// Signed out, nothing watched: case 3, Continue at 0.
    Fresh,
    /// Signed in, the one title finished: case 2, Continue at 0.
    Suggested,
    /// Signed out with one title under way: case 1, Continue at 1.
    OneLeft,
    /// Fifteen titles under way: twelve shown, View all and the count.
    ContinueFull,
    /// Signed in, nothing watched, and a source that has nothing: Add to Planning.
    NotFound,
    /// Watching with every discovery answer held past the activity line.
    Slow,
    /// Watching with every discovery request failing.
    Failed,
    /// Nothing watched and no network.
    Offline,
    /// Watching with no source installed.
    Sourceless,
}

impl HomeScene {
    pub fn named(name: &str) -> Option<Self> {
        Some(match name {
            "watching" => Self::Watching,
            "fresh" => Self::Fresh,
            "suggested" => Self::Suggested,
            "one-left" => Self::OneLeft,
            "continue-full" => Self::ContinueFull,
            "not-found" => Self::NotFound,
            "slow" => Self::Slow,
            "failed" => Self::Failed,
            "offline" => Self::Offline,
            "sourceless" => Self::Sourceless,
            _ => return None,
        })
    }

    fn signed_in(self) -> bool {
        !matches!(self, Self::Fresh | Self::OneLeft | Self::Offline)
    }

    /// The world's own history: finished and caught-up shows, none of them in Continue.
    fn world_history(self) -> bool {
        matches!(
            self,
            Self::Watching | Self::Suggested | Self::Slow | Self::Failed | Self::Sourceless
        )
    }
}

/// What the scene starts with beyond its history.
#[derive(Debug, Clone, Default)]
pub struct Scene {
    /// A file every episode streams, for the player.
    pub media: Option<String>,
    /// An episode downloading, so closing the window asks first.
    pub downloading: bool,
    /// The state Home starts in.
    pub home: HomeScene,
}

/// Whether the environment asks for Home's world.
fn world_asked() -> bool {
    std::env::var(HOME_WORLD_VAR).is_ok_and(|v| v == "1")
}

/// Start the engine on the scene, its files under `root`.
pub async fn start(root: &Path, scene: &Scene, host: Arc<dyn Host>) -> Result<Started, String> {
    let paths = paths_under(root);
    let signed_in = scene.home.signed_in();
    if signed_in {
        write_config(&paths.config_file)?;
    }
    let (config, store, _) = open(&paths)?;
    let clock = Clock::default();
    let now = clock.now();
    let world = world_asked().then(|| HomeWorld::at(HomeClock::in_zone(now, &clock.zone())));
    seed_scene(&store, scene.home, now);
    if let Some(world) = world.as_ref().filter(|_| scene.home.world_history()) {
        world.seed_history(&store, now);
        let mut statuses = fixtures::library_statuses();
        statuses.extend(world.library_statuses());
        store
            .replace_library_statuses("anilist", &statuses, now)
            .map_err(|e| format!("seeding the list's statuses: {e}"))?;
    }
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
    if signed_in {
        tokens.set("anilist", "fixture-anilist-token").map_err(|e| e.to_string())?;
    }

    let server = FakeAniList::start().await;
    server.set_now(now);
    server.add_titles(fixtures::long_franchise());
    server.serve_art();
    // Home's world: this season's titles, the rest in the browse, and their broadcasts.
    if let Some(world) = &world {
        let (season, rest): (Vec<_>, Vec<_>) = world
            .titles
            .iter()
            .cloned()
            .partition(|t| world.this_season.iter().any(|id| t["id"] == id.get()));
        server.add_season(season);
        server.add_catalogue(rest);
        server.add_broadcasts(world.broadcasts.iter().copied());
        if scene.home.world_history() {
            server.add_entries(world.entries.iter().cloned());
        }
    }
    match scene.home {
        HomeScene::Slow => {
            for operation in DISCOVERY {
                server.answer_slowly(operation, SLOW_ANSWER);
            }
        }
        HomeScene::Failed => {
            for operation in DISCOVERY {
                server.fail_operation(operation, Failure::Status(500));
            }
        }
        HomeScene::Offline => {
            OFFLINE_SCENE.store(true, std::sync::atomic::Ordering::Relaxed);
            server.fail_always(Failure::Disconnect)
        }
        _ => {}
    }
    let http = HttpClient::offline(&NetworkConfig {
        timeout_secs: client_wait(scene.home),
        ..config.network.clone()
    })
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
    let provided = world.as_ref().map(HomeWorld::provided).unwrap_or_default();
    let chain = match scene.home {
        HomeScene::Sourceless => ProviderRegistry::new(Vec::new()),
        HomeScene::NotFound => ProviderRegistry::new(vec![
            anistream_providers::MockProvider::new(sources::PROVIDER_ID)
                .with_hits(Vec::new())
                .arc(),
        ]),
        _ => registry(&server.art_base(), scene.media.as_deref(), &provided),
    };
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
        windows: None,
    });
    let (engine, events): (Engine, _) =
        Engine::start(options, host).await.map_err(|e| e.to_string())?;
    Ok(Started { engine, events, keep: Some(Box::new(server)), rebuilt: Some(rebuilt) })
}

/// What Home asks AniList for beyond its queue's titles and their broadcasts: the season's
/// lists, the genre shelf, recommendations and the season walk. The slow and failing scenes hold
/// these up.
const DISCOVERY: [Operation; 6] = [
    Operation::Browse,
    Operation::Search,
    Operation::Seasonal,
    Operation::Recommended,
    Operation::Relations,
    Operation::Schedule,
];

/// The fixture provider, its stills from the fake, every episode streaming `media` when given.
/// `provided` is Home's world's titles with the episodes out of each, which it has too.
fn registry(
    art: &str,
    media: Option<&str>,
    provided: &[(anistream_meta::Media, u32)],
) -> ProviderRegistry {
    let mut provider =
        sources::with_long_franchise_and(sources::provider_with_art(art), provided);
    if let Some(media) = media {
        let stream = Stream {
            provider_id: sources::PROVIDER_ID.into(),
            ..Stream::new(media, StreamKind::Mp4)
        };
        let world = provided.iter().map(|p| p.0.id);
        for id in fixtures::ALL.into_iter().chain(world) {
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

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: HomeScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        HomeScene::Fresh | HomeScene::NotFound | HomeScene::Offline => {}
        HomeScene::Suggested => {
            let all = fixtures::media(FINISHED).episodes.unwrap_or(1);
            seed::watched(store, FINISHED, all, now - HOUR);
        }
        HomeScene::OneLeft => seed::watched(store, AIRING_NO_MAL, 7, now - DAY),
        HomeScene::ContinueFull => {
            let long = fixtures::long_franchise_media();
            let shows = fixtures::ALL.into_iter().chain(long.iter().map(|m| m.id)).take(15);
            // Oldest first, an hour apart, each stopped five minutes into episode 1.
            for (n, id) in shows.collect::<Vec<_>>().into_iter().rev().enumerate() {
                seed::stopped_at(store, id, "1", 5.0 * 60.0, now - (15 - n as i64) * HOUR);
            }
        }
        HomeScene::Watching | HomeScene::Slow | HomeScene::Failed | HomeScene::Sourceless => {
            seed_history(store, now)
        }
    }
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
