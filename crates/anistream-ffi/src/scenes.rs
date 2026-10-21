//! The engine started on a scene: the test kit's fixtures with a history already in the
//! database, and the fake AniList answering at the real time, for the Apple app's Fixtures build
//! to show its screens filled.

use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anistream_store::Store;
use anistream_testkit::{
    FakeAniList,
    fixtures::{AIRING, AIRING_NO_MAL, FINISHED, ONA},
    seed,
};

use crate::{
    engine::{FfiEngine, Opened, runtime},
    host::{Adapter, FfiHost},
    start::{FfiDirs, FfiError, FfiStart, SecretsMode},
    testing,
};

/// What a scene puts in the database before the engine starts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
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
        let opened = Opened::open(&dirs)?;
        let now = now();
        seed_scene(&opened.store, scene, now);
        let host = Adapter::new(host, SecretsMode::Host, &opened.config, &opened.paths);
        let runtime = runtime()?;
        let started = runtime.block_on(async {
            let server = FakeAniList::start().await;
            server.set_now(now);
            let options = testing::options(opened, &server, &start)?;
            Ok::<_, FfiError>((Self::launch(options, host).await?, server))
        });
        match started {
            Ok((started, server)) => Self::hold(runtime, Ok(started), Some(Box::new(server))),
            Err(e) => Self::hold(runtime, Err(e), None),
        }
    }
}

/// Seconds since the epoch, now.
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// The history `scene` describes, the newest watch an hour before `now`.
fn seed_scene(store: &Store, scene: FfiScene, now: i64) {
    const HOUR: i64 = 3_600;
    const DAY: i64 = 24 * HOUR;
    match scene {
        FfiScene::Fresh => {}
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
        seed_scene(&store, FfiScene::Watching, 1_800_000_000);

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
        seed_scene(&store, FfiScene::Fresh, 1_800_000_000);
        assert!(store.continue_list(15).expect("the rail").is_empty());
    }
}
