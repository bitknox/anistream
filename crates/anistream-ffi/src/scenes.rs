//! The engine started on a scene: the test kit's fixtures with a history already in the
//! database, and the fake AniList answering at the real time, for the Apple app's Fixtures build
//! to show its screens filled.

use std::sync::Arc;

use anistream_core::media::{Episode, MediaFormat, SearchHit, Translation};
use anistream_core::stream::{Stream, StreamKind};
use anistream_engine::clock::Clock;
use anistream_meta::Media;
use anistream_model::Msg;
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_store::Store;
use anistream_testkit::{
    Failure, FakeAniList, Operation,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    home_world::{HomeClock, HomeWorld},
    seed, sources,
};
use anistream_track::secret::{Backend, TokenStore};

use crate::{
    engine::{FfiEngine, Opened, RemoteHome, runtime},
    host::{Adapter, FfiHost},
    start::{FfiDirs, FfiError, FfiStart, SecretsMode},
    testing,
};

/// What a scene puts in the database before the engine starts on it.
///
/// Every scene's fake AniList also has the test kit's long franchise
/// ([`fixtures::LONG_FRANCHISE_SEASONS`], twelve seasons of Cloudcutter Airship) and the short one
/// ([`fixtures::FRANCHISE_SEASONS`], five seasons of Lantern Wood), in no list, search or
/// schedule: their pages open by id.
///
/// With the environment variable [`HOME_WORLD_VAR`] set to `1` when a scene starts, the scene
/// has Home's world too ([`HomeWorld`], ids 950001 to 950036): invented titles dated by the
/// scene's clock, fourteen of them airing this season, so This season, Airing today, Search,
/// Seasonal and the Calendar have a season's worth whatever the date. The signed-in scenes with
/// a history ([`Self::Watching`], its kin and [`Self::Suggested`]) also have the world's
/// history: four shows finished here and Completed on the list, The Forest Courier last, three
/// caught up and Watching, and two on Planning. None of it is in Continue, so Home's shelves
/// fill as the design board draws them and Continue is as it was. It is for Home's shots: the
/// caught-up shows and what airs today change the new-episode notices and the widgets' record,
/// which the apps' tests read from the scenes as they are without it.
///
/// With the environment variable `ANISTREAM_FIXTURES_CATALOGUE` set to `1` when a scene starts,
/// eight more invented titles ([`fixtures::catalogue_titles`], ids 900101 to 900108) join the
/// six in Search and browse, for a grid of fourteen: no scene's own data changes, and with the
/// variable unset or anything else the six-title world is exactly as it was.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScene {
    /// Nothing: a first launch, as [`FfiEngine::start_testing`] starts.
    Fresh,
    /// Signed in to the fake AniList, whose library then fills the Library, and four titles
    /// under way, newest first, as Continue shows them:
    ///
    /// - Hoshikuzu no Tabibito, episodes 1 to 6 watched and episode 7 left at 10:06 of 24:00;
    /// - Hagane no Niwashi, episode 1 watched and episode 2 left at 13:30 of 18:00;
    /// - Kumo no Ue no Toshokan, episodes 1 to 3 watched, so episode 4 is next;
    /// - Natsukaze Relay, episodes 1 to 7 watched, and episode 8 out since 19 hours ago.
    Watching,
    /// [`Self::Watching`] with no source set up: every title's episodes come from AniList's own
    /// data, marked so, and a page offers to add a source in place of Play.
    Sourceless,
    /// A scene that plays: Hoshikuzu no Tabibito's episodes 1 to 6 watched, so Continue starts
    /// episode 7, and every episode of every title streams `media`, a local file or URL the
    /// app's own player can open (the test kit's own clip is a lavfi source, which the app's
    /// libmpv has no device for). Skipping the opening is off, so the skip is offered rather
    /// than taken, unless the config says otherwise. An address ending `.m3u8` is a web stream:
    /// HLS behind a referer lock, as provider CDNs have them, which goes through the engine's
    /// mender with its header ([`SCENE_REFERER`]).
    Playing { media: String },
    /// [`Self::Watching`], with every episode streaming `media` as [`Self::Playing`]'s do: a TV
    /// on a fixture resumes episode 7 at once from a file it can seek, where the test kit's clip
    /// is a test pattern mpv must play through to the resume point.
    WatchingMedia { media: String },
    /// [`Self::Watching`] with a second source ahead of the fixture one, Hotaru Open Archive,
    /// which lists four titles for Hoshikuzu no Tabibito and is sure of none: two could be it
    /// and two are ruled out, one by its year and one by its format. So its page asks for the
    /// match. A search there for "Stardust Wayfarer" finds the show itself, whose episodes play,
    /// and its specials, ruled out. Every other title it has nothing for, so the fixture source
    /// answers for them as in [`Self::Watching`].
    Unsure,
    /// Home's second spotlight case: signed in, every episode of Hoshikuzu no Tabibito watched
    /// and nothing under way, so Continue is empty and the spotlight is from Up next for you.
    /// With the Home world on, that is The Ice Knight Season 2, a new season airing now of a
    /// show finished here.
    Suggested,
    /// Signed out, with one show in Continue: Natsukaze Relay's episodes 1 to 7 watched and
    /// episode 8 out, the spotlight's first case. Once episode 8 is watched or marked so, the
    /// show is caught up and the spotlight turns to the popular pick: the change from the first
    /// case to the third, with Continue at one and then at none.
    OneLeft,
    /// Signed in, with fifteen shows under way, each stopped part way into episode 1, so
    /// Continue shows twelve with View all and its count.
    ContinueFull,
    /// Signed in with nothing watched, and the one source has none of the titles: every title
    /// Home asks after is not found in your sources, and its main action is Add to Planning.
    NotFound,
    /// [`Self::Watching`] with AniList slow for everything but the queue's own titles: each of
    /// Home's discovery requests is answered after [`SLOW_ANSWER`], so the local sections show
    /// at once, the placeholders come in and the activity line after them.
    Slow,
    /// [`Self::Watching`] with AniList failing everything but the queue's own titles and their
    /// broadcasts: Continue shows, the discovery shelves are left out and one notice says so.
    Failed,
    /// [`Self::Watching`] with three watch party rooms kept, as Together shows them: Natsukaze
    /// Relay's with two friends an hour ago, Stardust Wayfarer's with one a day ago, and
    /// Cloudcutter Airship's alone a week ago. The party lobby states are reached by joining the
    /// fake party server ([`FfiEngine::start_fake_party`]) and steering its members: Natsukaze
    /// Relay's episode 9 has not aired (Out on a day), Stardust Wayfarer's episodes play, and
    /// [`Self::Sourceless`] and [`Self::NotFound`] show the other two sources lines.
    Together,
    /// Nothing watched and offline from the start: the fake AniList hangs up on every request
    /// and the engine is told the path is offline, whatever the host says, so Home has nothing
    /// saved to show. [`FfiEngine::testing_set_offline`] does the same to any other scene once
    /// Home has loaded, for the offline line over what it last loaded.
    Offline,
}

/// The environment variable that puts Home's world in a scene: see [`FfiScene`].
pub const HOME_WORLD_VAR: &str = "ANISTREAM_FIXTURES_HOME_WORLD";

/// Whether the environment asks for Home's world.
fn home_world_asked() -> bool {
    std::env::var(HOME_WORLD_VAR).is_ok_and(|v| v == "1")
}

/// How long [`FfiScene::Slow`]'s fake AniList holds each of Home's discovery answers: past the
/// four seconds after which Home's activity line shows, within the five the engine's client
/// waits for an answer.
pub const SLOW_ANSWER: std::time::Duration = std::time::Duration::from_millis(4_500);

/// What Home asks AniList for beyond its queue's titles and their broadcasts: the season's
/// lists, the genre shelf, recommendations and the season walk. [`FfiScene::Slow`] and
/// [`FfiScene::Failed`] hold these up.
const DISCOVERY: [Operation; 6] = [
    Operation::Browse,
    Operation::Search,
    Operation::Seasonal,
    Operation::Recommended,
    Operation::Relations,
    Operation::Schedule,
];

impl FfiScene {
    /// Whether the scene has the Home world's history and list entries beside its own.
    fn has_world_history(&self) -> bool {
        matches!(
            self,
            Self::Watching
                | Self::WatchingMedia { .. }
                | Self::Sourceless
                | Self::Suggested
                | Self::Slow
                | Self::Failed
        )
    }

    /// Whether the scene starts signed in to the fake AniList.
    fn signed_in(&self) -> bool {
        matches!(
            self,
            Self::Watching
                | Self::WatchingMedia { .. }
                | Self::Sourceless
                | Self::Unsure
                | Self::Suggested
                | Self::ContinueFull
                | Self::NotFound
                | Self::Slow
                | Self::Failed
        )
    }
}

#[uniffi::export]
impl FfiEngine {
    /// [`FfiEngine::start_testing`] on `scene`, with the fake AniList answering at the time
    /// the engine starts rather than the fixtures' own, so that what aired and what airs next
    /// is counted from now, as the engine counts it. `start` is taken as `start_testing` takes
    /// it: tokens are files under `dirs.data` whatever its `secrets` says.
    ///
    /// Only in a build with the `testing` feature, which an app never turns on.
    #[uniffi::constructor(default(start))]
    pub fn start_scene(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        start: FfiStart,
    ) -> Result<Arc<Self>, FfiError> {
        let world = home_world_asked();
        Self::start_scene_from(dirs, host, scene, &start, Clock::default(), world)
    }
}

impl FfiEngine {
    /// [`FfiEngine::start_scene`] on `clock` rather than the system's own: the scene seeded, and
    /// the fake AniList answering, as of the moment it reads, and today and the week read in its
    /// zone. For a Rust test that reads days in a zone of its own (`Clock::in_zone`), so that its
    /// today and its week come out the same on any date, at any hour and in any zone it runs in;
    /// `testing_advance_clock` moves the clock on. Not in the bindings.
    pub fn start_scene_on(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        clock: Clock,
    ) -> Result<Arc<Self>, FfiError> {
        let world = home_world_asked();
        Self::start_scene_from(dirs, host, scene, &FfiStart::default(), clock, world)
    }

    /// [`FfiEngine::start_scene_on`] with Home's world in the scene, whatever the environment
    /// says: see [`FfiScene`]. For Home's own scene tests. Not in the bindings.
    pub fn start_home_scene_on(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        clock: Clock,
    ) -> Result<Arc<Self>, FfiError> {
        Self::start_scene_from(dirs, host, scene, &FfiStart::default(), clock, true)
    }

    fn start_scene_from(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        start: &FfiStart,
        clock: Clock,
        home_world: bool,
    ) -> Result<Arc<Self>, FfiError> {
        if scene.signed_in() {
            sign_in_config(&dirs)?;
        }
        if matches!(scene, FfiScene::Playing { .. }) {
            offer_skips(&dirs)?;
        }
        let opened = Opened::open(&dirs)?;
        let now = clock.now();
        let world = home_world.then(|| HomeWorld::at(HomeClock::in_zone(now, &clock.zone())));
        seed_scene(&opened.store, &scene, now);
        if let Some(world) = world.as_ref().filter(|_| scene.has_world_history()) {
            world.seed_history(&opened.store, now);
            library_statuses(&opened.store, world, now);
        }
        if scene.signed_in() {
            // Where the testing engine keeps tokens: files under the data directory.
            TokenStore::with_backend(opened.paths.data_dir.join("tokens"), Backend::File)
                .set("anilist", "fixture-anilist-token")
                .map_err(|e| FfiError::engine(Msg::StartFailed { reason: e.to_string() }))?;
        }
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let home = RemoteHome::of(&host, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let server = FakeAniList::start().await;
            server.set_now(now);
            // The long franchise beside the six, for a season picker of twelve: a page opens on
            // it by id, as no list has it.
            server.add_titles(fixtures::long_franchise());
            // And the short one, five seasons with a film, an OVA and a web series beside them,
            // for a season row that fits across the page with Related beside it.
            server.add_titles(fixtures::franchise());
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
                if scene.has_world_history() {
                    server.add_entries(world.entries.iter().cloned());
                }
            }
            // Eight more titles in the browse, for a grid of fourteen, only when asked for.
            if std::env::var("ANISTREAM_FIXTURES_CATALOGUE").is_ok_and(|v| v == "1") {
                server.add_catalogue(fixtures::catalogue_titles());
            }
            match scene {
                FfiScene::Slow => {
                    for operation in DISCOVERY {
                        server.answer_slowly(operation, SLOW_ANSWER);
                    }
                }
                FfiScene::Failed => {
                    for operation in DISCOVERY {
                        server.fail_operation(operation, Failure::Status(500));
                    }
                }
                _ => {}
            }
            let provided = world.as_ref().map(HomeWorld::provided).unwrap_or_default();
            let sources = Some(scene_sources(&scene, &server.art_base(), &provided));
            let testing = testing::Testing::new(server, clock);
            let options = testing::options(opened, &testing, start, sources)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, testing))
        });
        let engine = match started {
            Ok((started, testing)) => {
                Self::hold(runtime, Ok(started), Some(Box::new(testing)), home)
            }
            Err(e) => Self::hold(runtime, Err(e), None, home),
        }?;
        if scene == FfiScene::Offline {
            engine.testing_set_offline(true);
        }
        Ok(engine)
    }
}

/// Downloads for the Downloads screen to show, seeded into the database at `dirs` before a scene
/// starts on it: episode 6 of the finished fixture downloaded, its file `clip` (a real file on
/// the device, which plays offline), episode 7 of it 40% in, and episode 3 of the airing one
/// waiting. The engine's downloads are torrents, which no fixture has, so this is how a build
/// shows them without one.
///
/// Only in a build with the `testing` feature, which an app never turns on.
#[uniffi::export]
pub fn seed_fixture_downloads(dirs: FfiDirs, clip: String) -> Result<(), FfiError> {
    let opened = Opened::open(&dirs)?;
    let size = std::fs::metadata(&clip)
        .map_err(|e| FfiError::dirs(Msg::verbatim(format!("{clip}: {e}"))))?
        .len();
    seed_downloads(&opened.store, &clip, size)
        .map_err(|e| FfiError::store(Msg::verbatim(format!("seeding the downloads: {e}"))))
}

/// The three rows [`seed_fixture_downloads`] seeds, the finished one at `clip`, `size` bytes.
fn seed_downloads(store: &Store, clip: &str, size: u64) -> anistream_store::Result<()> {
    use anistream_store::DownloadState;
    let done = seed::download(store, FINISHED, "6", DownloadState::Done);
    store.update_download_progress(done.id, size, size, Some(clip))?;
    store.finish_download(done.id, Some(clip))?;
    seed::download(store, FINISHED, "7", DownloadState::Active);
    seed::download(store, AIRING, "3", DownloadState::Queued);
    Ok(())
}

/// The scene's sources: the test kit's fixture provider, its stills under `art`, with the long
/// franchise's titles beside the six, which the scene's fake AniList has too, and `provided`,
/// Home's world's titles with the episodes out of each, when the scene has it. Every episode
/// streams the test kit's episode-long clip, or `media` in a scene that plays. None at all for
/// [`FfiScene::Sourceless`].
fn scene_sources(scene: &FfiScene, art: &str, provided: &[(Media, u32)]) -> ProviderRegistry {
    if *scene == FfiScene::Sourceless {
        return ProviderRegistry::new(Vec::new());
    }
    if *scene == FfiScene::NotFound {
        // A source that answers every search with nothing: a miss for every title.
        return ProviderRegistry::new(vec![
            MockProvider::new(sources::PROVIDER_ID).with_hits(Vec::new()).arc(),
        ]);
    }
    let franchise =
        || sources::with_long_franchise_and(sources::provider_with_art(art), provided);
    let (FfiScene::Playing { media } | FfiScene::WatchingMedia { media }) = scene else {
        let clip = sources::episode_clip();
        let mut provider = franchise();
        for id in fixtures::ALL {
            provider = provider.with_streams_for(sources::key(id), vec![clip.clone()]);
        }
        let others = fixtures::long_franchise_media()
            .into_iter()
            .chain(provided.iter().map(|p| p.0.clone()));
        for media in others {
            provider = provider.with_streams_for(sources::key(media.id), vec![clip.clone()]);
        }
        if *scene == FfiScene::Unsure {
            return ProviderRegistry::new(vec![
                unsure_source(art, &clip).arc(),
                provider.arc(),
            ]);
        }
        return ProviderRegistry::new(vec![provider.arc()]);
    };
    let web = media.starts_with("http")
        && media.split('?').next().is_some_and(|path| path.ends_with(".m3u8"));
    let stream = Stream {
        provider_id: sources::PROVIDER_ID.into(),
        headers: if web { vec![("Referer".into(), SCENE_REFERER.into())] } else { Vec::new() },
        ..Stream::new(media, if web { StreamKind::Hls } else { StreamKind::Mp4 })
    };
    let mut provider = franchise();
    for id in fixtures::ALL {
        provider = provider.with_streams_for(sources::key(id), vec![stream.clone()]);
    }
    ProviderRegistry::new(vec![provider.arc()])
}

/// The source [`FfiScene::Unsure`] puts first: an invented archive that lists four titles for
/// Hoshikuzu no Tabibito, none close enough to choose, and the show itself for a search by its
/// English title. The show's episodes are the fixture's, streaming `clip`.
fn unsure_source(art: &str, clip: &Stream) -> MockProvider {
    let hit =
        |key: &str, title: &str, episodes: u32, year: u16, format: MediaFormat| SearchHit {
            episode_count: Some(episodes),
            year: Some(year),
            format: Some(format),
            ..SearchHit::new(format!("{UNSURE_SOURCE}:{key}"), title)
        };
    let show = format!("{UNSURE_SOURCE}:stardust-wayfarer");
    let episodes = sources::episodes(FINISHED)
        .into_iter()
        .map(|e| Episode {
            thumbnail: e.thumbnail.map(|t| t.replacen(fixtures::ART_HOST, art, 1)),
            ..e
        })
        .collect();
    let clip = Stream { provider_id: UNSURE_SOURCE.into(), ..clip.clone() };
    MockProvider::new(UNSURE_SOURCE)
        .named("Hotaru Open Archive")
        .with_hits_for(
            "Hoshikuzu no Tabibito",
            vec![
                hit("hoshi-tabi", "Hoshi Tabi Diaries", 12, 2024, MediaFormat::Tv),
                hit("tabibito-hoshikuzu", "Tabibito to Hoshikuzu", 13, 2025, MediaFormat::Tv),
                hit("hoshikuzu-2019", "Hoshikuzu no Tabibito", 10, 2019, MediaFormat::Tv),
                hit("hoshikuzu-ova", "Hoshikuzu no Tabibito: Yoru", 1, 2024, MediaFormat::Ova),
            ],
        )
        .with_hits_for(
            "Stardust Wayfarer",
            vec![
                hit("stardust-wayfarer", "Stardust Wayfarer", 12, 2024, MediaFormat::Tv),
                hit(
                    "stardust-specials",
                    "Stardust Wayfarer Specials",
                    2,
                    2024,
                    MediaFormat::Special,
                ),
            ],
        )
        .with_episodes_for(show.clone(), episodes)
        .with_streams_for(show, vec![clip])
}

/// The id of [`FfiScene::Unsure`]'s first source.
pub const UNSURE_SOURCE: &str = "hotaru-open-archive";

/// The referer a Playing scene's web stream is locked to: an invented source's.
pub const SCENE_REFERER: &str = "https://lantern-archive.example/";

/// A config with AniList enabled, as a signed-in user's has it: the fixtures' client, and tokens
/// in files. Left alone when there is a config already.
fn sign_in_config(dirs: &FfiDirs) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    if file.exists() {
        return Ok(());
    }
    let written = std::fs::create_dir_all(&dirs.config).and_then(|()| {
        std::fs::write(
            &file,
            "[trackers]\nenabled = [\"anilist\"]\ntoken_storage = \"file\"\n\n\
             [trackers.anilist]\nclient_id = \"fixture\"\nclient_secret = \"fixture\"\n",
        )
    });
    written.map_err(|e| FfiError::config(Msg::verbatim(format!("{}: {e}", file.display()))))
}

/// A config that offers skips rather than taking them, written where there is none yet.
fn offer_skips(dirs: &FfiDirs) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    if file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&dirs.config)
        .and_then(|()| std::fs::write(&file, "[playback]\nskip_opening = false\n"))
        .map_err(|e| {
            FfiError::dirs(Msg::verbatim(format!("{} cannot be written: {e}", file.display())))
        })
}

/// The fake library's statuses as the last sync left them: the fixture library's and the Home
/// world's. A scene starts with them, so what Home leaves out or suggests from the list is
/// there from the first load rather than after the engine's first sync.
fn library_statuses(store: &Store, world: &HomeWorld, now: i64) {
    let mut statuses = fixtures::library_statuses();
    statuses.extend(world.library_statuses());
    store
        .replace_library_statuses("anilist", &statuses, now)
        .expect("seeding the list's statuses");
}

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: &FfiScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        FfiScene::Fresh | FfiScene::NotFound | FfiScene::Offline => {}
        FfiScene::Playing { .. } => seed::watched(store, FINISHED, 6, now - HOUR),
        FfiScene::Suggested => {
            let all = fixtures::media(FINISHED).episodes.unwrap_or(1);
            seed::watched(store, FINISHED, all, now - HOUR);
        }
        FfiScene::OneLeft => seed::watched(store, AIRING_NO_MAL, 7, now - DAY),
        FfiScene::ContinueFull => {
            let long = fixtures::long_franchise_media();
            let shows = fixtures::ALL.into_iter().chain(long.iter().map(|m| m.id)).take(15);
            // Oldest first, an hour apart, each stopped five minutes into episode 1.
            for (n, id) in shows.collect::<Vec<_>>().into_iter().rev().enumerate() {
                let at = now - (15 - n as i64) * HOUR;
                seed::stopped_at(store, id, "1", 5.0 * 60.0, at);
            }
        }
        FfiScene::Watching
        | FfiScene::Together
        | FfiScene::WatchingMedia { .. }
        | FfiScene::Sourceless
        | FfiScene::Unsure
        | FfiScene::Slow
        | FfiScene::Failed => {
            // Oldest first, since each watch moves its title to the top.
            seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
            seed::watched(store, AIRING, 3, now - 2 * DAY);
            seed::watched(store, ONA, 1, now - DAY - HOUR);
            // Switched to the dub after episode 1, so its row is marked "Dub".
            seed::stopped_in(store, ONA, "2", 13.0 * 60.0 + 30.0, Translation::Dub, now - DAY);
            seed::watched(store, FINISHED, 6, now - 2 * HOUR);
            seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
            if *scene == FfiScene::Together {
                seed_rooms(store, now);
            }
        }
    }
}

/// The three rooms [`FfiScene::Together`] keeps.
fn seed_rooms(store: &Store, now: i64) {
    use anistream_store::PartyRoom;
    const HOUR: i64 = 3_600;
    let rooms = [
        (AIRING_NO_MAL, "kite-7qm4-xr9d", "8", &["Mika", "Ren"][..], now - HOUR),
        (FINISHED, "ember-2hv8-kd4p", "3", &["Mika"][..], now - 24 * HOUR),
        (ONA, "ember-9wz3-tn6c", "1", &[][..], now - 7 * 24 * HOUR),
    ];
    for (id, room, episode, names, last_at) in rooms {
        let media = fixtures::media(id);
        store
            .remember_party_room(&PartyRoom {
                server: "syncplay.pl:8995".into(),
                room: room.into(),
                anilist_id: id,
                title: media.title.display().to_owned(),
                episode: episode.into(),
                names: names.iter().map(|n| (*n).to_owned()).collect(),
                cover_url: None,
                last_at,
            })
            .expect("seeding a party room");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Home's world at the tests' moment, read in UTC.
    fn world() -> HomeWorld {
        HomeWorld::at(HomeClock::in_zone(1_800_000_000, &jiff::tz::TimeZone::UTC))
    }

    #[test]
    fn the_worlds_history_leaves_continue_as_it_was_and_the_list_in_the_store() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        seed_scene(&store, &FfiScene::Watching, 1_800_000_000);
        let world = world();
        world.seed_history(&store, 1_800_000_000);
        library_statuses(&store, &world, 1_800_000_000);
        let continuing = store.continue_list(15).expect("the rail");
        let ids: Vec<_> = continuing.iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL]);
        assert_eq!(store.watched_titles(50).expect("the history").len(), 4 + 7);
        let completed = store.library_with_status("COMPLETED").expect("the list");
        assert_eq!(completed.len(), 1 + 4, "{completed:?}");
        assert_eq!(store.library_with_status("PLANNING").expect("the list").len(), 1 + 2);
    }

    #[test]
    fn watching_leaves_four_titles_to_continue_with_the_newest_first() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        seed_scene(&store, &FfiScene::Watching, 1_800_000_000);

        let continuing = store.continue_list(15).expect("the rail");
        let ids: Vec<_> = continuing.iter().map(|p| p.anilist_id).collect();
        assert_eq!(ids, [FINISHED, ONA, AIRING, AIRING_NO_MAL]);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), Some(606.0));
        assert_eq!(store.resume_position(ONA, "2").expect("a read"), Some(810.0));
        // One watched dubbed, the rest subbed.
        let dubbed =
            continuing.iter().map(|p| store.last_translation(p.anilist_id).expect("a read"));
        let sub = Some(Translation::Sub);
        assert_eq!(dubbed.collect::<Vec<_>>(), [sub, Some(Translation::Dub), sub, sub]);
    }

    #[test]
    fn sourceless_is_watching_with_no_source_at_all() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        seed_scene(&store, &FfiScene::Sourceless, 1_800_000_000);
        assert_eq!(store.continue_list(15).expect("the rail").len(), 4);
        let none = scene_sources(
            &FfiScene::Sourceless,
            "http://127.0.0.1:9/art/",
            &world().provided(),
        );
        assert!(none.is_empty());
    }

    #[test]
    fn unsure_asks_about_the_finished_title_and_finds_it_by_its_english_one() {
        use anistream_core::Provider;
        use anistream_meta::title::{MatchTarget, Rejection, rank};
        let target = MatchTarget {
            titles: vec![
                "Hoshikuzu no Tabibito".into(),
                "Stardust Wayfarer".into(),
                "Hoshitabi".into(),
            ],
            episode_count: Some(12),
            year: Some(2024),
            format: Some(MediaFormat::Tv),
        };
        let source = unsure_source("http://127.0.0.1:9/art/", &sources::episode_clip());
        let runtime =
            tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let search =
            |query: &str| runtime.block_on(source.search(query, Translation::Sub)).unwrap();

        let ladder = search("Hoshikuzu no Tabibito");
        let ranked = rank(&target, &ladder);
        let open: Vec<_> = ranked.iter().filter(|s| s.rejected.is_none()).collect();
        assert_eq!(open.len(), 2, "{ranked:?}");
        assert!(open.iter().all(|s| !s.is_confident()), "{ranked:?}");
        let mut out: Vec<_> = ranked.iter().filter_map(|s| s.rejected).collect();
        out.sort_by_key(|r| format!("{r:?}"));
        assert_eq!(out, [Rejection::Format, Rejection::Year]);

        let asked = search("  stardust WAYFARER ");
        let ranked = rank(&target, &asked);
        assert!(ranked[0].is_confident(), "{ranked:?}");
        assert_eq!(ranked[0].hit.title, "Stardust Wayfarer");
        assert!(search("Kumo no Ue no Toshokan").is_empty());

        let registry =
            scene_sources(&FfiScene::Unsure, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(registry.ids(), [UNSURE_SOURCE, sources::PROVIDER_ID]);
    }

    #[test]
    fn fresh_leaves_the_database_empty() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        seed_scene(&store, &FfiScene::Fresh, 1_800_000_000);
        assert!(store.continue_list(15).expect("the rail").is_empty());
    }

    #[test]
    fn playing_leaves_episode_seven_to_start_and_streams_the_media_for_every_episode() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        let scene = FfiScene::Playing { media: "/tmp/episode.mkv".into() };
        seed_scene(&store, &scene, 1_800_000_000);
        let continuing = store.continue_list(15).expect("the rail");
        assert_eq!(continuing.iter().map(|p| p.anilist_id).collect::<Vec<_>>(), [FINISHED]);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), None);
        let playing = scene_sources(&scene, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(playing.ids(), [sources::PROVIDER_ID]);
        // Watching on a file of its own: Watching's history, and the file for every episode.
        let file = FfiScene::WatchingMedia { media: "/tmp/episode.mp4".into() };
        let store = Store::open(dir.path().join("watching.db")).expect("a database");
        seed_scene(&store, &file, 1_800_000_000);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), Some(606.0));
        assert_eq!(
            scene_sources(&file, "http://127.0.0.1:9/art/", &world().provided()).ids(),
            [sources::PROVIDER_ID]
        );
        // Every scene has the fixture provider, with the long franchise beside the six.
        let watching =
            scene_sources(&FfiScene::Watching, "http://127.0.0.1:9/art/", &world().provided());
        assert_eq!(watching.ids(), [sources::PROVIDER_ID]);
    }

    #[test]
    fn the_fixture_downloads_are_one_finished_at_the_clip_one_running_and_one_waiting() {
        use anistream_store::DownloadState;
        let dir = tempfile::tempdir().expect("a temporary directory");
        let store = Store::open(dir.path().join("anistream.db")).expect("a database");
        seed_downloads(&store, "/sdcard/clip.mkv", 2_048).expect("seeded");
        let rows = store.downloads().expect("the queue");
        let states: Vec<_> = rows.iter().map(|row| (row.episode.as_str(), row.state)).collect();
        assert!(states.contains(&("6", DownloadState::Done)), "{states:?}");
        assert!(states.contains(&("7", DownloadState::Active)), "{states:?}");
        assert!(states.contains(&("3", DownloadState::Queued)), "{states:?}");
        let done = rows.iter().find(|row| row.episode == "6").expect("the finished one");
        assert_eq!(done.path.as_deref(), Some("/sdcard/clip.mkv"));
        assert_eq!((done.downloaded, done.total), (2_048, 2_048));
    }

    #[test]
    fn playing_offers_skips_in_a_config_of_its_own_and_keeps_one_already_there() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let at = |name: &str| dir.path().join(name).display().to_string();
        let dirs = FfiDirs { config: at("config"), data: at("data"), cache: at("cache") };
        offer_skips(&dirs).expect("written");
        let written = std::fs::read_to_string(dir.path().join("config/config.toml")).unwrap();
        assert!(written.contains("skip_opening = false"), "{written}");
        std::fs::write(dir.path().join("config/config.toml"), "# mine\n").unwrap();
        offer_skips(&dirs).expect("left alone");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config/config.toml")).unwrap(),
            "# mine\n"
        );
    }
}
