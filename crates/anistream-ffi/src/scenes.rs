//! The engine started on a scene: the test kit's fixtures with a history already in the
//! database, and the fake AniList answering at the real time, for the Apple app's Fixtures build
//! to show its screens filled.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anistream_core::stream::{Stream, StreamKind};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;
use anistream_testkit::{
    FakeAniList,
    fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, ONA},
    seed, sources,
};

use crate::{
    engine::{FfiEngine, Opened, runtime},
    host::{Adapter, FfiHost},
    start::{FfiDirs, FfiError, FfiStart, SecretsMode},
    testing,
};

/// What a scene puts in the database before the engine starts on it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiScene {
    /// Nothing: a first launch, as [`FfiEngine::start_testing`] starts.
    Fresh,
    /// Four titles under way, newest first, as Continue shows them:
    ///
    /// - Hoshikuzu no Tabibito, episodes 1 to 6 watched and episode 7 left at 10:06 of 24:00;
    /// - Hagane no Niwashi, episode 1 watched and episode 2 left at 13:30 of 18:00;
    /// - Kumo no Ue no Toshokan, episodes 1 to 3 watched, so episode 4 is next;
    /// - Natsukaze Relay, episodes 1 to 7 watched, and episode 8 out since 19 hours ago.
    Watching,
    /// A scene that plays: Hoshikuzu no Tabibito's episodes 1 to 6 watched, so Continue starts
    /// episode 7, and every episode of every title streams `media`, a local file or URL the
    /// app's own player can open (the test kit's own clip is a lavfi source, which the app's
    /// libmpv has no device for). Skipping the opening is off, so the skip is offered rather
    /// than taken, unless the config says otherwise.
    Playing { media: String },
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
        if matches!(scene, FfiScene::Playing { .. }) {
            offer_skips(&dirs)?;
        }
        let opened = Opened::open(&dirs)?;
        let now = now();
        seed_scene(&opened.store, &scene, now);
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let server = FakeAniList::start().await;
            server.set_now(now);
            let options = testing::options(opened, &server, &start, scene_sources(&scene))?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, server))
        });
        match started {
            Ok((started, server)) => Self::hold(runtime, Ok(started), Some(Box::new(server))),
            Err(e) => Self::hold(runtime, Err(e), None),
        }
    }
}

/// The scene's sources: the test kit's fixture provider, streaming `media` for a scene that
/// plays.
fn scene_sources(scene: &FfiScene) -> ProviderRegistry {
    let FfiScene::Playing { media } = scene else { return sources::registry() };
    let stream = Stream {
        provider_id: sources::PROVIDER_ID.into(),
        ..Stream::new(media, StreamKind::Mp4)
    };
    let mut provider = sources::provider();
    for id in fixtures::ALL {
        provider = provider.with_streams_for(sources::key(id), vec![stream.clone()]);
    }
    ProviderRegistry::new(vec![provider.arc()])
}

/// A config that offers skips rather than taking them, written where there is none yet.
fn offer_skips(dirs: &FfiDirs) -> Result<(), FfiError> {
    let file = std::path::Path::new(&dirs.config).join("config.toml");
    if file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&dirs.config)
        .and_then(|()| std::fs::write(&file, "[playback]\nskip_opening = false\n"))
        .map_err(|e| FfiError::Dirs {
            reason: format!("{} cannot be written: {e}", file.display()),
        })
}

/// Seconds since the epoch, now.
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: &FfiScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        FfiScene::Fresh => {}
        FfiScene::Playing { .. } => seed::watched(store, FINISHED, 6, now - HOUR),
        FfiScene::Watching => {
            // Oldest first, since each watch moves its title to the top.
            seed::watched(store, AIRING_NO_MAL, 7, now - 3 * DAY);
            seed::watched(store, AIRING, 3, now - 2 * DAY);
            seed::watched(store, ONA, 1, now - DAY - HOUR);
            seed::stopped_at(store, ONA, "2", 13.0 * 60.0 + 30.0, now - DAY);
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
        assert_eq!(scene_sources(&scene).ids(), [sources::PROVIDER_ID]);
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
