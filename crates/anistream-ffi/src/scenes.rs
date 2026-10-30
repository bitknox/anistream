//! The engine started on a scene: the test kit's fixtures with a history already in the
//! database, and the fake AniList answering at the real time, for the Apple app's Fixtures build
//! to show its screens filled.

use std::sync::Arc;

use anistream_core::media::Translation;
use anistream_core::stream::{Stream, StreamKind};
use anistream_engine::clock::Clock;
use anistream_model::Msg;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_testkit::{
    FakeAniList,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
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
        Self::start_scene_from(dirs, host, scene, &start, Clock::default())
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
        Self::start_scene_from(dirs, host, scene, &FfiStart::default(), clock)
    }

    fn start_scene_from(
        dirs: FfiDirs,
        host: Arc<dyn FfiHost>,
        scene: FfiScene,
        start: &FfiStart,
        clock: Clock,
    ) -> Result<Arc<Self>, FfiError> {
        match scene {
            FfiScene::Watching | FfiScene::WatchingMedia { .. } | FfiScene::Sourceless => {
                sign_in_config(&dirs)?;
            }
            FfiScene::Playing { .. } => offer_skips(&dirs)?,
            FfiScene::Fresh => {}
        }
        let opened = Opened::open(&dirs)?;
        let now = clock.now();
        seed_scene(&opened.store, &scene, now);
        if matches!(
            scene,
            FfiScene::Watching | FfiScene::WatchingMedia { .. } | FfiScene::Sourceless
        ) {
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
            // Eight more titles in the browse, for a grid of fourteen, only when asked for.
            if std::env::var("ANISTREAM_FIXTURES_CATALOGUE").is_ok_and(|v| v == "1") {
                server.add_catalogue(fixtures::catalogue_titles());
            }
            let sources = Some(scene_sources(&scene, &server.art_base()));
            let testing = testing::Testing { server, clock };
            let options = testing::options(opened, &testing, start, sources)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, testing))
        });
        match started {
            Ok((started, testing)) => {
                Self::hold(runtime, Ok(started), Some(Box::new(testing)), home)
            }
            Err(e) => Self::hold(runtime, Err(e), None, home),
        }
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
/// franchise's titles beside the six, which the scene's fake AniList has too. Every episode
/// streams the test kit's episode-long clip, or `media` in a scene that plays. None at all for
/// [`FfiScene::Sourceless`].
fn scene_sources(scene: &FfiScene, art: &str) -> ProviderRegistry {
    if *scene == FfiScene::Sourceless {
        return ProviderRegistry::new(Vec::new());
    }
    let franchise = || sources::with_long_franchise(sources::provider_with_art(art));
    let (FfiScene::Playing { media } | FfiScene::WatchingMedia { media }) = scene else {
        let clip = sources::episode_clip();
        let mut provider = franchise();
        for id in fixtures::ALL {
            provider = provider.with_streams_for(sources::key(id), vec![clip.clone()]);
        }
        for media in fixtures::long_franchise_media() {
            provider = provider.with_streams_for(sources::key(media.id), vec![clip.clone()]);
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

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: &FfiScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        FfiScene::Fresh => {}
        FfiScene::Playing { .. } => seed::watched(store, FINISHED, 6, now - HOUR),
        FfiScene::Watching | FfiScene::WatchingMedia { .. } | FfiScene::Sourceless => {
            // Oldest first, since each watch moves its title to the top.
            seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
            seed::watched(store, AIRING, 3, now - 2 * DAY);
            seed::watched(store, ONA, 1, now - DAY - HOUR);
            // Switched to the dub after episode 1, so its row is marked "Dub".
            seed::stopped_in(store, ONA, "2", 13.0 * 60.0 + 30.0, Translation::Dub, now - DAY);
            seed::watched(store, FINISHED, 6, now - 2 * HOUR);
            seed::stopped_at(store, FINISHED, "7", 10.0 * 60.0 + 6.0, now - HOUR);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let none = scene_sources(&FfiScene::Sourceless, "http://127.0.0.1:9/art/");
        assert!(none.is_empty());
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
        let playing = scene_sources(&scene, "http://127.0.0.1:9/art/");
        assert_eq!(playing.ids(), [sources::PROVIDER_ID]);
        // Watching on a file of its own: Watching's history, and the file for every episode.
        let file = FfiScene::WatchingMedia { media: "/tmp/episode.mp4".into() };
        let store = Store::open(dir.path().join("watching.db")).expect("a database");
        seed_scene(&store, &file, 1_800_000_000);
        assert_eq!(store.resume_position(FINISHED, "7").expect("a read"), Some(606.0));
        assert_eq!(
            scene_sources(&file, "http://127.0.0.1:9/art/").ids(),
            [sources::PROVIDER_ID]
        );
        // Every scene has the fixture provider, with the long franchise beside the six.
        let watching = scene_sources(&FfiScene::Watching, "http://127.0.0.1:9/art/");
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
