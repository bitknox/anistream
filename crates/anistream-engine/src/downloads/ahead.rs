//! Keep next: the next episodes of each show being watched, downloaded ahead.
//!
//! For every show on the viewer's list as watching or rewatching, with a number to keep — its
//! own, or else Settings' — the episodes after where the viewer is are queued, as many as that
//! number, and only those that are out. Each is queued as a download asked for by hand would
//! be: in the show's own download quality and translation, from the source it is pinned to.
//!
//! **What it does is top up.** It queues what is missing and nothing else: an episode with a
//! row in the queue, however that row stands, is not asked for again, and nothing is ever
//! deleted. A show that leaves the list's watching keeps what it has; it is only no longer
//! topped up. Starting the downloads is the queue's own work, at its own concurrency.
//!
//! **When it tops up.** On start; after an episode is marked watched, counts as watched in the
//! player, or plays to its end; after the airing digest finds a new episode; when the number to
//! keep changes, or the network or the app's phase lets downloads run again; and after a list
//! sync moves a show's progress or where it is on the list. Asked for again while a top-up runs,
//! the latest ask runs once it is done.
//!
//! **What it waits for.** Nothing is topped up while downloads wait: off screen, on a metered
//! network, or while the connection check that guards torrents fails. And it stops before the
//! device would be full: what is free, less what the queue still has to fetch and an episode
//! more, has to leave [`KEEP_FREE`]. It says so once, until there is room again.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anistream_core::{config::Config, ids::AnilistId};
use anistream_meta::{Media, Meta};
use anistream_model::{event::Event, notice::Notice, settings::SettingKey};
use anistream_providers::{ProviderRegistry, torrent::session::Hold};
use anistream_store::{Download, DownloadState, Store};

use crate::{
    downloads::{self, Queued, Refusal, Wanted},
    outbox::Outbox,
    services::Services,
    show_settings,
    state::EngineState,
};

const GB: u64 = 1_000_000_000;

/// What keep next leaves free on the device, whatever it would keep: room for the system and
/// everything else on it.
pub const KEEP_FREE: u64 = 2 * GB;

/// What an episode is taken to need while no download has finished to measure one by: an
/// ordinary 1080p episode.
pub const EPISODE_GUESS: u64 = 3 * GB / 2;

/// What keep next says, once, when the device has no room for another episode.
pub const FULL: &str = "not enough free space to keep the next episodes downloaded";

/// Keep next's state, which the engine side keeps: whether it runs at all, the top-up asked for
/// while one runs, and whether a full device has been said.
#[derive(Clone, Default)]
pub struct Ahead(Arc<Inner>);

#[derive(Default)]
struct Inner {
    /// Off for an engine that does only what it is sent: a test's, a still frame's, a phone's
    /// background task's. See [`crate::options::Background::keep_next`].
    on: bool,
    /// What a test takes to be free on the device, in place of asking the disk.
    free: Option<u64>,
    queue: Mutex<Queue>,
    /// A full device has been said, and is not said again until there is room.
    said_full: AtomicBool,
}

/// The top-ups asked for: the latest, waiting for the one under way.
#[derive(Default)]
struct Queue {
    /// The config the next top-up runs on, the one in effect when it was asked for.
    next: Option<Config>,
    running: bool,
}

impl Ahead {
    /// Keep next for an engine that does it, when `on`, with `free` for what a test takes to be
    /// free on the device.
    pub fn new(on: bool, free: Option<u64>) -> Self {
        Self(Arc::new(Inner { on, free, ..Inner::default() }))
    }

    /// Run a top-up on `config`, now or after the one under way, which then runs no more than
    /// once for every ask that came while it ran: on the config of the latest.
    fn ask(&self, config: Config, services: &Services, tx: &Outbox) {
        let mut queue = self.0.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.next = Some(config);
        if queue.running {
            return;
        }
        queue.running = true;
        drop(queue);
        let inner = Arc::clone(&self.0);
        let live = Live {
            store: services.store.clone(),
            registry: services.registry.clone(),
            meta: services.meta.clone(),
            cache_dir: services.paths.cache_dir.clone(),
            free: inner.free,
        };
        let tx = tx.clone();
        tokio::spawn(async move {
            loop {
                let config = {
                    let mut queue = inner.queue.lock().unwrap_or_else(|e| e.into_inner());
                    match queue.next.take() {
                        Some(config) => config,
                        None => {
                            queue.running = false;
                            return;
                        }
                    }
                };
                run(&config, &live, &live.store, &inner.said_full, &tx).await;
            }
        });
    }
}

/// Top up the shows being watched: when keep next runs, downloads may run now, and a show has
/// an episode to keep that is not in the queue. What is missing is read here, from the
/// database; finding which of it is out, and queueing it, runs off the actor.
pub fn top_up(engine: &EngineState, services: &Services, tx: &Outbox) {
    if !engine.ahead.0.on || downloads_wait(engine) {
        return;
    }
    let config = engine.config.effective();
    // Off in Settings and set by no show, as it is unless asked for: nothing to read.
    let kept_by_shows = engine.config.shows_setting(SettingKey::KeepNextEpisodes);
    if config.downloads.keep_next == 0 && kept_by_shows == 0 {
        return;
    }
    if missing(config, &services.store).is_empty() {
        return;
    }
    engine.ahead.ask(config.clone(), services, tx);
}

/// Whether downloads wait now, as the torrent session is told: off screen, on a metered
/// network, or while the connection check that guards torrents fails.
pub(crate) fn downloads_wait(engine: &EngineState) -> bool {
    // With the merge train's gate, this is the gate's `hold.downloads`.
    let held = crate::phases::hold_now(engine) != Hold::Nothing;
    let refused = engine.vpn_guard.as_ref().is_some_and(|(guard, _)| guard.permit().is_err());
    held || refused
}

/// Whether `event` moves where a viewer is in a show: an episode that counts as watched in the
/// player, or one played to its end.
///
/// With the merge train's new-episode check, `Event::NewEpisodes` with any episode in it is one
/// too.
pub fn after(event: &Event) -> bool {
    matches!(event, Event::ProgressQueued | Event::PlaybackEnded { watched: true, .. })
}

/// Whether a change to setting `key` can change what keep next keeps, or whether it may run.
pub fn follows(key: SettingKey) -> bool {
    matches!(key, SettingKey::KeepNextEpisodes | SettingKey::DownloadsOnCellular)
}

/// What one show wants kept and the queue does not have: the episodes, in order, before the
/// show's schedule has said which of them are out; and the config a download of it runs under.
#[derive(Debug, Clone)]
pub(crate) struct Missing {
    pub id: AnilistId,
    pub episodes: Vec<u32>,
    pub config: Config,
}

/// What each show being watched wants kept and the queue does not have, on `config`, the one
/// in effect: the most recently watched show first.
pub(crate) fn missing(config: &Config, store: &Store) -> Vec<Missing> {
    let watching = store.watching().unwrap_or_else(|error| {
        tracing::warn!(%error, "keep next could not read the shows being watched");
        Vec::new()
    });
    watching
        .into_iter()
        .filter_map(|id| {
            let config = show_settings::for_show(config, store, id);
            let keep = config.downloads.keep_next;
            if keep == 0 {
                return None;
            }
            let episodes: Vec<u32> = next_episodes(store, id, keep)
                .into_iter()
                .filter(|episode| {
                    let episode = episode.to_string();
                    // Any row is one: queued, running, paused, done, or failed and left for
                    // the viewer to retry. And one the viewer let go of stays gone.
                    !matches!(store.download_for(id, &episode), Ok(Some(_)))
                        && !store.download_dismissed(id, &episode).unwrap_or(true)
                })
                .collect();
            (!episodes.is_empty()).then_some(Missing { id, episodes, config })
        })
        .collect()
}

/// The `keep` episodes of show `id` after where the viewer is: from the episode Continue
/// offers, the ones not yet watched.
pub(crate) fn next_episodes(store: &Store, id: AnilistId, keep: u32) -> Vec<u32> {
    let completed = store.completed_episodes(id).unwrap_or_default();
    let start = match store.progress(id) {
        Ok(Some(progress)) => crate::data::continue_from(&progress, &completed, None),
        _ => 1,
    };
    let watched = |n: u32| completed.iter().any(|e| e.trim().parse::<u32>() == Ok(n));
    // Bounded: a run of watched episodes ahead must still end.
    (start..start.saturating_add(1_000)).filter(|n| !watched(*n)).take(keep as usize).collect()
}

/// Of `episodes`, in order, those that are out: up to the first that is past the show's last
/// episode, or has not aired by its schedule.
pub(crate) fn out(media: &Media, episodes: &[u32]) -> Vec<u32> {
    episodes
        .iter()
        .copied()
        .take_while(|n| {
            media.episodes.is_none_or(|total| *n <= total)
                && crate::playback::resolve::not_aired(media, &n.to_string()).is_none()
        })
        .collect()
}

/// Whether one more episode fits on a device with `free` bytes free, beside `queue`: what is
/// free has to cover what the queue still has to fetch, an episode more, and [`KEEP_FREE`].
/// Unknown room is taken as room.
pub(crate) fn fits(free: Option<u64>, queue: &[Download]) -> bool {
    let Some(free) = free else { return true };
    let episode = episode_size(queue);
    let owed: u64 = queue
        .iter()
        .filter(|row| {
            matches!(
                row.state,
                DownloadState::Queued | DownloadState::Active | DownloadState::Paused
            )
        })
        .map(|row| match row.total {
            0 => episode,
            total => total.saturating_sub(row.downloaded),
        })
        .sum();
    free >= owed.saturating_add(episode).saturating_add(KEEP_FREE)
}

/// What an episode is taken to need: the mean of the downloads finished, or [`EPISODE_GUESS`]
/// before there are any.
fn episode_size(queue: &[Download]) -> u64 {
    let sizes: Vec<u64> = queue
        .iter()
        .filter(|row| row.state == DownloadState::Done && row.total > 0)
        .map(|row| row.total)
        .collect();
    match sizes.len() as u64 {
        0 => EPISODE_GUESS,
        count => sizes.iter().sum::<u64>() / count,
    }
}

/// What a top-up asks of the world: a show's schedule, an episode queued, the room on the
/// device. The engine's own, [`Live`], asks AniList, the sources and the disk; a test's answers
/// from memory.
pub(crate) trait Work {
    /// Show `id`, for its length and schedule, or `None` when it cannot be had now.
    async fn media(&self, id: AnilistId) -> Option<Media>;
    /// Queue `episode` of show `id` ahead, as `wanted` asks for it.
    async fn enqueue(
        &self,
        id: AnilistId,
        episode: &str,
        wanted: Wanted,
    ) -> Result<Queued, Refusal>;
    /// What is free where a download under `config` lands, when that can be said.
    fn free(&self, config: &Config) -> Option<u64>;
}

/// A top-up's work, as the engine does it.
struct Live {
    store: Store,
    registry: ProviderRegistry,
    meta: Meta,
    cache_dir: PathBuf,
    /// A test's word for what is free, in place of the disk's.
    free: Option<u64>,
}

impl Work for Live {
    async fn media(&self, id: AnilistId) -> Option<Media> {
        match self.meta.media(id).await {
            Ok(media) => Some(media),
            Err(error) => {
                tracing::info!(%id, %error, "keep next could not read a show's schedule");
                None
            }
        }
    }

    async fn enqueue(
        &self,
        id: AnilistId,
        episode: &str,
        wanted: Wanted,
    ) -> Result<Queued, Refusal> {
        downloads::enqueue(&self.store, &self.registry, &self.meta, id, episode, wanted, true)
            .await
            .map(|(_, queued)| queued)
    }

    fn free(&self, config: &Config) -> Option<u64> {
        if self.free.is_some() {
            return self.free;
        }
        // Where the bytes arrive, and where the file goes once finished, which can be another
        // disk: the less room of the two.
        let fetching = self.cache_dir.join("torrents");
        let finished = destination(config, &self.cache_dir);
        [free_space(&fetching), free_space(&finished)].into_iter().flatten().min()
    }
}

/// Where a finished download stays: the downloads folder, unless it seeds on from the cache.
fn destination(config: &Config, cache_dir: &Path) -> PathBuf {
    match (&config.downloads.directory, config.downloads.keep_seeding) {
        (Some(dir), false) => PathBuf::from(dir),
        _ => cache_dir.join("torrents"),
    }
}

/// What is free on the disk `path` is on, or would be once made.
#[cfg(unix)]
fn free_space(path: &Path) -> Option<u64> {
    let existing = path.ancestors().find(|ancestor| ancestor.exists())?;
    let stat = rustix::fs::statvfs(existing).ok()?;
    Some(stat.f_bavail.saturating_mul(stat.f_frsize))
}

/// Not asked off Unix: unknown room is taken as room.
#[cfg(not(unix))]
fn free_space(_: &Path) -> Option<u64> {
    None
}

/// Top up, on `config`: for each show missing what it keeps, the episodes of it that are out,
/// queued in order while they fit. A show stops at the first episode refused, and the whole
/// top-up at the first that would not fit, which is said once until one fits again.
pub(crate) async fn run(
    config: &Config,
    work: &impl Work,
    store: &Store,
    said_full: &AtomicBool,
    tx: &Outbox,
) {
    for show in missing(config, store) {
        let Some(media) = work.media(show.id).await else { continue };
        let wanted = Wanted::of(&show.config);
        for episode in out(&media, &show.episodes) {
            let queue = store.downloads().unwrap_or_default();
            if !fits(work.free(&show.config), &queue) {
                if !said_full.swap(true, Ordering::Relaxed) {
                    let _ = tx.send(Event::Toast(Notice::info(FULL)));
                }
                return;
            }
            said_full.store(false, Ordering::Relaxed);
            let episode = episode.to_string();
            match work.enqueue(show.id, &episode, wanted).await {
                Ok(Queued::New) => downloads::publish_now(store, tx),
                Ok(Queued::Existing) => {}
                // Said nowhere but the log: nobody asked for this one, and the next top-up
                // asks again.
                Err(refusal) => {
                    tracing::info!(id = %show.id, %episode, %refusal, "keep next not queued");
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use anistream_model::{command::AppState, notice::NoticeKind};
    use anistream_store::WatchEvent;
    use anistream_testkit::fixtures::{self, AIRING, FINISHED, ONA, SEQUEL};
    use tokio::sync::mpsc;

    use super::*;

    /// Plenty of room: a hundred episodes' worth over what is kept free.
    const ROOMY: u64 = KEEP_FREE + 100 * EPISODE_GUESS;

    /// A store with `shows` on the list as `status`.
    fn listed(shows: &[(AnilistId, &str)]) -> Store {
        let store = Store::open_in_memory().unwrap();
        store.replace_library_statuses("anilist", shows, 10).unwrap();
        store
    }

    /// Episodes `1..=upto` of `id` watched to the end, at `at`.
    fn watched(store: &Store, id: AnilistId, upto: u32, at: i64) {
        for n in 1..=upto {
            let mut event = WatchEvent::new(id, n.to_string(), 1_400.0);
            event.duration_secs = Some(1_440.0);
            event.completed = true;
            event.at = at;
            store.record_event(&event).unwrap();
        }
    }

    fn keeping(n: u32) -> Config {
        let mut config = Config::default();
        config.downloads.keep_next = n;
        config
    }

    /// Each show and what it is missing.
    fn wants(config: &Config, store: &Store) -> Vec<(AnilistId, Vec<u32>)> {
        missing(config, store).into_iter().map(|m| (m.id, m.episodes)).collect()
    }

    /// The episodes of `id` in the queue, each with whether it was queued ahead.
    fn queued(store: &Store, id: AnilistId) -> Vec<(String, bool)> {
        let mut rows: Vec<_> = store
            .downloads()
            .unwrap()
            .into_iter()
            .filter(|row| row.anilist_id == id)
            .map(|row| (row.episode, row.ahead))
            .collect();
        rows.sort_by_key(|(episode, _)| episode.parse::<u32>().unwrap_or(0));
        rows
    }

    /// A top-up's work from memory: the fixtures' schedules, `free` bytes free, and each
    /// episode queued into `store` as asked.
    struct Fake {
        store: Store,
        free: Option<u64>,
        media: HashMap<AnilistId, Media>,
        asked: Mutex<Vec<(AnilistId, String, Wanted)>>,
    }

    impl Fake {
        fn on(store: &Store) -> Self {
            Self {
                store: store.clone(),
                free: Some(ROOMY),
                media: [FINISHED, AIRING, ONA, SEQUEL]
                    .into_iter()
                    .map(|id| (id, fixtures::media(id)))
                    .collect(),
                asked: Mutex::default(),
            }
        }

        fn asked(&self) -> Vec<(AnilistId, String)> {
            self.asked.lock().unwrap().iter().map(|(id, e, _)| (*id, e.clone())).collect()
        }
    }

    impl Work for Fake {
        async fn media(&self, id: AnilistId) -> Option<Media> {
            self.media.get(&id).cloned()
        }

        async fn enqueue(
            &self,
            id: AnilistId,
            episode: &str,
            wanted: Wanted,
        ) -> Result<Queued, Refusal> {
            self.asked.lock().unwrap().push((id, episode.to_owned(), wanted));
            self.store
                .enqueue_download_ahead(id, episode, "Show", "magnet:?xt=urn:btih:fake")
                .map(|_| Queued::New)
                .map_err(|e| Refusal::Title(e.to_string()))
        }

        fn free(&self, _: &Config) -> Option<u64> {
            self.free
        }
    }

    /// Run one top-up on `config` with `work`, and return what it said.
    async fn top_up_with(config: &Config, work: &Fake, said_full: &AtomicBool) -> Vec<Event> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        run(config, work, &work.store, said_full, &Outbox::from(tx)).await;
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn toasts(said: &[Event]) -> Vec<String> {
        said.iter()
            .filter_map(|event| match event {
                Event::Toast(notice) => Some(notice.text.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_number_kept_is_settings_or_the_shows_own() {
        let store = listed(&[(FINISHED, "CURRENT"), (SEQUEL, "REPEATING"), (ONA, "CURRENT")]);
        watched(&store, FINISHED, 4, 300);
        // A show that keeps more for itself, and one that keeps none.
        store.set_show_setting(SEQUEL, "keep_next", Some("3"), 1).unwrap();
        store.set_show_setting(ONA, "keep_next", Some("0"), 1).unwrap();

        assert_eq!(
            wants(&keeping(2), &store),
            [(FINISHED, vec![5, 6]), (SEQUEL, vec![1, 2, 3])],
            "the last watched first, each with its own number",
        );
        // Off in Settings, a show that keeps its own still keeps them.
        assert_eq!(wants(&keeping(0), &store), [(SEQUEL, vec![1, 2, 3])]);
        assert!(wants(&keeping(0), &listed(&[(FINISHED, "CURRENT")])).is_empty());
    }

    #[test]
    fn the_episodes_kept_are_the_ones_after_where_the_viewer_is() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, 300);
        // Episode 6 watched out of order, and episode 5 left part way.
        let mut six = WatchEvent::new(FINISHED, "6", 1_400.0);
        (six.duration_secs, six.completed, six.at) = (Some(1_440.0), true, 310);
        store.record_event(&six).unwrap();
        let mut five = WatchEvent::new(FINISHED, "5", 600.0);
        (five.duration_secs, five.at) = (Some(1_440.0), 320);
        store.record_event(&five).unwrap();
        assert_eq!(next_episodes(&store, FINISHED, 3), [5, 7, 8], "6 is watched already");

        // Nothing watched here: from the first.
        assert_eq!(next_episodes(&store, SEQUEL, 2), [1, 2]);
    }

    #[test]
    fn an_episode_with_a_row_in_the_queue_is_not_asked_for_again() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 2, 300);
        let done = store.enqueue_download(FINISHED, "3", "Show", "m").unwrap();
        store.finish_download(done.id, None).unwrap();
        let failed = store.enqueue_download_ahead(FINISHED, "5", "Show", "m").unwrap();
        store.fail_download(failed.id, "no peers").unwrap();
        let paused = store.enqueue_download(FINISHED, "6", "Show", "m").unwrap();
        store.set_download_state(paused.id, DownloadState::Paused).unwrap();
        store.enqueue_download_ahead(FINISHED, "7", "Show", "m").unwrap();
        assert_eq!(wants(&keeping(6), &store), [(FINISHED, vec![4, 8])]);

        // Cancelled by the viewer: not queued again.
        let cancelled = store.download_for(FINISHED, "7").unwrap().unwrap();
        store.remove_download(cancelled.id).unwrap();
        assert_eq!(wants(&keeping(6), &store), [(FINISHED, vec![4, 8])]);
    }

    #[tokio::test]
    async fn a_top_up_queues_what_is_missing_once_in_the_shows_own_quality() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, 300);
        store.set_show_setting(FINISHED, "download_quality", Some("higher"), 1).unwrap();
        store.enqueue_download(FINISHED, "5", "Show", "m").unwrap();
        let work = Fake::on(&store);
        let full = AtomicBool::new(false);

        let said = top_up_with(&keeping(3), &work, &full).await;
        assert_eq!(work.asked(), [(FINISHED, "6".into()), (FINISHED, "7".into())]);
        let wanted = work.asked.lock().unwrap()[0].2;
        assert_eq!(wanted.resolution, u32::MAX, "the show's own download quality");
        assert_eq!(
            queued(&store, FINISHED),
            [("5".into(), false), ("6".into(), true), ("7".into(), true)],
            "the viewer's own stays theirs",
        );
        assert!(said.iter().all(|e| matches!(e, Event::Downloads(_))), "quietly: {said:?}");
        assert_eq!(said.len(), 2, "the queue said as each one lands");

        // Asked again with nothing new: nothing asked of the sources, nothing said.
        let again = top_up_with(&keeping(3), &work, &full).await;
        assert_eq!(work.asked().len(), 2);
        assert!(again.is_empty());
    }

    #[tokio::test]
    async fn an_episode_not_out_yet_is_not_asked_for() {
        // Five of its twenty-four out, the sixth two days off.
        let store = listed(&[(AIRING, "CURRENT"), (FINISHED, "CURRENT")]);
        watched(&store, AIRING, 3, 400);
        watched(&store, FINISHED, 10, 300);
        let work = Fake::on(&store);
        top_up_with(&keeping(4), &work, &AtomicBool::new(false)).await;
        assert_eq!(
            work.asked(),
            [
                (AIRING, "4".into()),
                (AIRING, "5".into()),
                // Twelve episodes: past the last is not out either.
                (FINISHED, "11".into()),
                (FINISHED, "12".into()),
            ]
        );
        assert_eq!(out(&fixtures::media(AIRING), &[5, 6, 7]), [5]);
    }

    #[tokio::test]
    async fn a_show_that_leaves_watching_keeps_what_it_has_and_is_topped_up_no_more() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, 300);
        let work = Fake::on(&store);
        top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        assert_eq!(queued(&store, FINISHED).len(), 2);

        for status in ["COMPLETED", "PAUSED", "DROPPED", "PLANNING"] {
            store.replace_library_statuses("anilist", &[(FINISHED, status)], 20).unwrap();
            watched(&store, FINISHED, 5, 400);
            assert!(wants(&keeping(2), &store).is_empty(), "{status}");
            top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        }
        // Off the list altogether.
        store.replace_library_statuses("anilist", &[], 30).unwrap();
        top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        assert_eq!(work.asked().len(), 2, "asked for nothing more");
        assert_eq!(queued(&store, FINISHED), [("5".into(), true), ("6".into(), true)]);
    }

    #[tokio::test]
    async fn a_full_device_stops_the_top_up_and_is_said_once() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        let mut work = Fake::on(&store);
        // Room for half an episode over what is kept free.
        work.free = Some(KEEP_FREE + EPISODE_GUESS / 2);
        let full = AtomicBool::new(false);

        let said = top_up_with(&keeping(3), &work, &full).await;
        assert!(work.asked().is_empty());
        assert_eq!(toasts(&said), [FULL]);
        let Event::Toast(notice) = &said[0] else { unreachable!() };
        assert_eq!(notice.kind, NoticeKind::Info);
        // Asked again, still full: said already.
        assert!(top_up_with(&keeping(3), &work, &full).await.is_empty());

        // Room for one: it is queued, and the next would not fit, which is said again.
        work.free = Some(KEEP_FREE + EPISODE_GUESS + EPISODE_GUESS / 2);
        let said = top_up_with(&keeping(3), &work, &full).await;
        assert_eq!(work.asked(), [(FINISHED, "1".into())]);
        assert_eq!(toasts(&said), [FULL]);
    }

    #[test]
    fn what_fits_counts_what_the_queue_still_has_to_fetch() {
        let store = Store::open_in_memory().unwrap();
        let done = store.enqueue_download(FINISHED, "1", "Show", "m").unwrap();
        store.update_download_progress(done.id, 0, 3 * GB, None).unwrap();
        store.finish_download(done.id, None).unwrap();
        let half = store.enqueue_download(FINISHED, "2", "Show", "m").unwrap();
        store.update_download_progress(half.id, GB, 2 * GB, None).unwrap();
        let queue = store.downloads().unwrap();
        // Episodes taken at the finished one's 3 GB: 1 GB still to fetch, 3 for the next.
        assert!(fits(Some(KEEP_FREE + 4 * GB), &queue));
        assert!(!fits(Some(KEEP_FREE + 4 * GB - 1), &queue));
        assert!(fits(None, &queue), "unknown room is room");
        assert!(fits(Some(KEEP_FREE + EPISODE_GUESS), &[]));
    }

    #[test]
    fn downloads_wait_off_screen_and_on_a_metered_network() {
        let dir = tempfile::tempdir().unwrap();
        let paths = anistream_core::config::Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let (arrivals, _) = mpsc::unbounded_channel();
        let mut engine = EngineState::new(Config::default(), None, &paths, arrivals);
        assert!(!downloads_wait(&engine), "on screen, on Wi-Fi");
        engine.metered = true;
        assert!(downloads_wait(&engine), "on cellular or Low Data Mode");
        engine.metered = false;
        engine.phase = AppState::Background { playing: true };
        assert!(downloads_wait(&engine));
        engine.phase = AppState::Background { playing: false };
        assert!(downloads_wait(&engine));
    }

    #[test]
    fn what_moves_a_viewer_on_and_the_settings_keep_next_follows() {
        use anistream_model::playback::Ending;
        assert!(after(&Event::ProgressQueued));
        assert!(after(&Event::playback_ended(true, Ending::Complete { next: None })));
        assert!(!after(&Event::playback_ended(false, Ending::Stopped)));
        assert!(!after(&Event::Downloads(Vec::new())));
        assert!(follows(SettingKey::KeepNextEpisodes));
        assert!(follows(SettingKey::DownloadsOnCellular));
        assert!(!follows(SettingKey::DownloadQuality));
    }
}
