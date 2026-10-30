//! Keep next: the next episodes of each show being watched, downloaded ahead, and let go of a day
//! after they are watched.
//!
//! The shows are those being watched: on the list as watching or rewatching, and watched here in
//! the last 30 days unless the list has them dropped or paused, which is the rule the
//! new-episode check follows too. A viewer with no tracker account has the second half. For each
//! with a number to keep — its own, or else Settings' — the episodes after where the viewer is
//! are queued, as many as that number, and only those that are out. Each is queued as a download
//! asked for by hand would be: in the show's own download quality and translation, from the
//! source it is pinned to.
//!
//! **What it does is top up.** It queues what is missing and nothing else: an episode with a
//! row in the queue, however that row stands, or one the viewer let go of from it, is not asked
//! for again. A show that stops being watched keeps what it has; it is only no longer topped up.
//! Starting the downloads is the queue's own work, at its own concurrency.
//!
//! **What it lets go of.** An episode it queued goes a day after it counts as watched: its row,
//! and its file. Never one the viewer asked for, nor one queued ahead that the viewer has since
//! asked for themselves, and never one at or after where the viewer is, as in a rewatch, where
//! the episodes ahead were watched long ago and are about to be again.
//!
//! **When it runs.** On start; after an episode is marked watched, counts as watched in the
//! player, or plays to its end; after the new-episode check or the airing digest finds a new
//! episode; when the number to
//! keep changes; when the network, the app's phase or the connection check lets downloads run
//! again; and after a list sync moves a show's progress or where it is on the list. Asked for
//! again while a top-up runs, the latest ask runs once it is done.
//!
//! **What it skips.** Nothing, with no source set up, and no show whose episode table no source
//! could list, as its last load found: its episodes are AniList's alone, with nothing to download
//! them from.
//!
//! **What it waits for.** Nothing is topped up while downloads wait: off screen but for a
//! downloads window, on cellular unless downloads may run there, in Low Data Mode, or while the
//! connection check that guards torrents fails. And it stops before the
//! device would be full: what is free, less what the queue still has to fetch and an episode
//! more, has to leave [`KEEP_FREE`]. It says so once, until there is room again.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anistream_core::{config::Config, ids::AnilistId};
use anistream_meta::{Media, Meta};
use anistream_model::{Msg, event::Event, notice::Notice, settings::SettingKey};
use anistream_providers::ProviderRegistry;
use anistream_store::{Download, DownloadState, Store};

use crate::{
    downloads::{self, Queued, Refusal, Wanted},
    outbox::Outbox,
    services::Services,
    show_settings,
    state::EngineState,
};

const GB: u64 = 1_000_000_000;
const DAY: i64 = 24 * 3_600;

/// What keep next leaves free on the device, whatever it would keep: room for the system and
/// everything else on it.
pub const KEEP_FREE: u64 = 2 * GB;

/// What an episode is taken to need while no download has finished to measure one by: an
/// ordinary 1080p episode.
pub const EPISODE_GUESS: u64 = 3 * GB / 2;

/// How long an episode kept ahead stays once it counts as watched: room for a rewatch of the
/// ending, or a finish that was a slip of the seek bar.
pub const GRACE: i64 = DAY;

/// How recently a show has to have been watched here to be followed without being on the list:
/// the new-episode check's `WATCHED_WITHIN`.
pub const WATCHED_WITHIN: i64 = 30 * DAY;

/// What keep next says, once, when the device has no room for another episode.
pub const FULL: Msg = Msg::NoticeKeepNextFull;

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
    /// The shows whose episode table was last listed from AniList's data alone, because no
    /// source here could list them: no source is asked to keep them until one does. Only for
    /// as long as the engine runs, so a show is asked about again on the next launch.
    unlisted: Mutex<HashSet<AnilistId>>,
    /// The episodes of each show that AniList says are out and no source lists: nothing is
    /// there to download until one does.
    unsourced: Mutex<std::collections::HashMap<AnilistId, HashSet<String>>>,
}

/// The top-ups asked for: the latest, waiting for the one under way.
#[derive(Default)]
struct Queue {
    /// What the next top-up runs on: the config in effect when it was asked for, and whether
    /// the list was a signed-in tracker's then.
    next: Option<(Config, bool)>,
    running: bool,
}

/// Whose shows are followed, and as of when: the list counts only while the tracker it came from
/// is signed in, as the Library's.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Viewer {
    pub now: i64,
    pub listed: bool,
}

impl Viewer {
    /// The viewer as `services` have them now.
    fn now(services: &Services) -> Self {
        Self { now: services.clock.now(), listed: listed(services) }
    }
}

/// Whether the list the database keeps is a signed-in tracker's: the Library's, the first.
pub(crate) fn listed(services: &Services) -> bool {
    services.sync.trackers.first().is_some_and(|tracker| tracker.is_authenticated())
}

impl Ahead {
    /// Keep next for an engine that does it, when `on`, with `free` for what a test takes to be
    /// free on the device.
    pub fn new(on: bool, free: Option<u64>) -> Self {
        Self(Arc::new(Inner { on, free, ..Inner::default() }))
    }

    /// Take in what `event` says of which shows a source lists: a table listed from AniList's
    /// data alone puts its show among the unlisted, and one a source listed takes it out. An
    /// empty table says neither.
    pub fn observe(&self, event: &Event) {
        let Event::Episodes { id, rows, failed: None } = event else { return };
        let Some(first) = rows.first() else { return };
        self.0.unsourced.lock().unwrap_or_else(|e| e.into_inner()).insert(
            *id,
            rows.iter()
                .filter(|row| row.unsourced)
                .map(|row| row.number.trim().to_owned())
                .collect(),
        );
        let mut unlisted = self.0.unlisted.lock().unwrap_or_else(|e| e.into_inner());
        if first.metadata_only {
            unlisted.insert(*id);
        } else {
            unlisted.remove(id);
        }
    }

    /// Whether episode `episode` of show `id` is one its table last said no source lists.
    pub fn unsourced(&self, id: AnilistId, episode: &str) -> bool {
        let map = self.0.unsourced.lock().unwrap_or_else(|e| e.into_inner());
        map.get(&id).is_some_and(|set| set.contains(episode.trim()))
    }

    /// Whether show `id`'s table was last listed from AniList's data alone: see
    /// [`Self::observe`].
    pub fn unlisted(&self, id: AnilistId) -> bool {
        self.0.unlisted.lock().unwrap_or_else(|e| e.into_inner()).contains(&id)
    }

    /// Run a top-up on `config`, now or after the one under way, which then runs no more than
    /// once for every ask that came while it ran: on the config of the latest.
    fn ask(&self, config: Config, listed: bool, services: &Services, tx: &Outbox) {
        let mut queue = self.0.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.next = Some((config, listed));
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
            ahead: self.clone(),
        };
        let (clock, tx) = (services.clock.clone(), tx.clone());
        tokio::spawn(async move {
            loop {
                let (config, listed) = {
                    let mut queue = inner.queue.lock().unwrap_or_else(|e| e.into_inner());
                    match queue.next.take() {
                        Some(next) => next,
                        None => {
                            queue.running = false;
                            return;
                        }
                    }
                };
                let viewer = Viewer { now: clock.now(), listed };
                run(&config, viewer, &live, &live.store, &inner.said_full, &tx).await;
            }
        });
    }
}

/// Let go of the episodes kept ahead that were watched a day ago, then top up the shows being
/// watched: when keep next runs, downloads may run now, and a show has an episode to keep that
/// is not in the queue. What goes, and what is missing, is read here, from the database; finding
/// which of it is out, and queueing it, runs off the actor.
pub fn top_up(engine: &EngineState, services: &Services, tx: &Outbox) {
    if !engine.ahead.0.on {
        return;
    }
    let viewer = Viewer::now(services);
    // Whatever downloads may do: letting go asks nothing of the network.
    expire(&services.store, viewer, tx);
    if downloads_wait(engine) {
        return;
    }
    // With no source set up there is nothing to download from, and every show's episodes are
    // AniList's alone.
    if services.registry.is_empty() && !services.registry.is_arriving() {
        return;
    }
    let config = engine.config.effective();
    // Off in Settings and set by no show, as it is unless asked for: nothing to read.
    let kept_by_shows = engine.config.shows_setting(SettingKey::KeepNextEpisodes);
    if config.downloads.keep_next == 0 && kept_by_shows == 0 {
        return;
    }
    if missing(config, &services.store, viewer).is_empty() {
        return;
    }
    engine.ahead.ask(config.clone(), viewer.listed, services, tx);
}

/// Whether downloads wait now, as the torrent session is told: off screen but for a downloads
/// window, on cellular unless downloads may run there, in Low Data Mode, or while the
/// connection check that guards torrents fails.
pub(crate) fn downloads_wait(engine: &EngineState) -> bool {
    let config = engine.config.effective();
    let held = crate::phases::gate_for(engine.phase, engine.network, config).hold.downloads;
    let refused = engine.vpn_guard.as_ref().is_some_and(|(guard, _)| guard.permit().is_err());
    held || refused
}

/// Whether `event` moves where a viewer is in a show, or what is out of one: an episode that
/// counts as watched in the player, one played to its end, or the new-episode check finding
/// one.
pub fn after(event: &Event) -> bool {
    match event {
        Event::ProgressQueued | Event::PlaybackEnded { watched: true, .. } => true,
        Event::NewEpisodes(found) => !found.is_empty(),
        _ => false,
    }
}

/// Whether a change to setting `key` can change what keep next keeps, or whether it may run.
pub fn follows(key: SettingKey) -> bool {
    matches!(key, SettingKey::KeepNextEpisodes | SettingKey::DownloadsOnCellular)
}

/// A show being watched, and whether the list has it as rewatched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Show {
    pub id: AnilistId,
    pub rewatching: bool,
}

/// The shows being watched, as of `viewer`: the most recently watched here first.
pub(crate) fn followed(store: &Store, viewer: Viewer) -> Vec<Show> {
    let ids =
        store.watching(viewer.now - WATCHED_WITHIN, viewer.listed).unwrap_or_else(|error| {
            tracing::warn!(%error, "keep next could not read the shows being watched");
            Vec::new()
        });
    ids.into_iter().map(|id| Show { id, rewatching: rewatching(store, id, viewer) }).collect()
}

/// Whether the list has show `id` as rewatched.
fn rewatching(store: &Store, id: AnilistId, viewer: Viewer) -> bool {
    viewer.listed
        && matches!(store.library_status(id), Ok(Some(status)) if status == "REPEATING")
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
pub(crate) fn missing(config: &Config, store: &Store, viewer: Viewer) -> Vec<Missing> {
    followed(store, viewer)
        .into_iter()
        .filter_map(|show| {
            let config = show_settings::for_show(config, store, show.id);
            let keep = config.downloads.keep_next;
            if keep == 0 {
                return None;
            }
            let episodes: Vec<u32> = next_episodes(store, show, keep)
                .into_iter()
                .filter(|episode| {
                    let episode = episode.to_string();
                    // Any row is one: queued, running, paused, done, or failed and left for
                    // the viewer to retry. And one the viewer let go of stays gone.
                    !matches!(store.download_for(show.id, &episode), Ok(Some(_)))
                        && !store.download_dismissed(show.id, &episode).unwrap_or(true)
                })
                .collect();
            (!episodes.is_empty()).then_some(Missing { id: show.id, episodes, config })
        })
        .collect()
}

/// The `keep` episodes of `show` after where the viewer is: from [`position`], the ones not yet
/// watched, or in a rewatch every one, since all were watched before.
pub(crate) fn next_episodes(store: &Store, show: Show, keep: u32) -> Vec<u32> {
    let start = position(store, show);
    let completed = if show.rewatching {
        Vec::new()
    } else {
        store.completed_episodes(show.id).unwrap_or_default()
    };
    let watched = |n: u32| completed.iter().any(|e| e.trim().parse::<u32>() == Ok(n));
    // Bounded: a run of watched episodes ahead must still end.
    (start..start.saturating_add(1_000)).filter(|n| !watched(*n)).take(keep as usize).collect()
}

/// The episode the viewer is on in `show`: the one Continue offers, or in a rewatch the one
/// last played, until it is finished, and then the one after it.
pub(crate) fn position(store: &Store, show: Show) -> u32 {
    let Ok(Some(progress)) = store.progress(show.id) else { return 1 };
    if show.rewatching
        && let Ok(last) = progress.last_episode.trim().parse::<u32>()
    {
        let finished =
            matches!(store.last_watch(show.id, &progress.last_episode), Ok(Some((true, _))));
        return if finished { last.saturating_add(1) } else { last };
    }
    let completed = store.completed_episodes(show.id).unwrap_or_default();
    crate::data::continue_from(&progress, &completed, None)
}

/// The downloads keep next queued that go now, as of `viewer`: each of an episode that counts
/// as watched, last played to its end at least [`GRACE`] ago, and before where the viewer is.
/// One the viewer asked for is not keep next's to let go of.
pub(crate) fn expired(store: &Store, viewer: Viewer) -> Vec<Download> {
    let rows = store.downloads().unwrap_or_default();
    rows.into_iter()
        .filter(|row| row.ahead)
        .filter(|row| {
            let Ok(episode) = row.episode.trim().parse::<u32>() else { return false };
            let watched = matches!(
                store.last_watch(row.anilist_id, &row.episode),
                Ok(Some((true, at))) if viewer.now - at >= GRACE
            );
            let show = Show {
                id: row.anilist_id,
                rewatching: rewatching(store, row.anilist_id, viewer),
            };
            watched && episode < position(store, show)
        })
        .collect()
}

/// Let go of what [`expired`] says, its files with it, and say the queue when anything went.
/// Not kept as let go of by the viewer: a watched episode is not queued again anyway, and in a
/// rewatch it is wanted again.
pub(crate) fn expire(store: &Store, viewer: Viewer, tx: &Outbox) -> usize {
    let mut gone = 0;
    for row in expired(store, viewer) {
        // The row first. A running download's torrent, and its part of the file, the queue
        // drops once its row is gone; a finished one's file is deleted here.
        if !store.expire_download(row.id).unwrap_or(false) {
            continue;
        }
        gone += 1;
        if let (DownloadState::Done | DownloadState::Failed, Some(path)) =
            (row.state, &row.path)
            && let Err(error) = std::fs::remove_file(path)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(%error, %path, "keep next could not delete a watched episode's file");
        }
        tracing::info!(id = %row.anilist_id, episode = %row.episode, "kept ahead, watched, let go");
    }
    if gone > 0 {
        downloads::publish_now(store, tx);
    }
    gone
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
    /// Whether no source here lists show `id`, as its table last said: see [`Ahead::observe`].
    fn unlisted(&self, _id: AnilistId) -> bool {
        false
    }
    /// Whether episode `episode` of show `id` is out and no source lists it.
    fn unsourced(&self, _id: AnilistId, _episode: &str) -> bool {
        false
    }
}

/// A top-up's work, as the engine does it.
struct Live {
    store: Store,
    registry: ProviderRegistry,
    meta: Meta,
    cache_dir: PathBuf,
    /// A test's word for what is free, in place of the disk's.
    free: Option<u64>,
    /// Which shows no source lists.
    ahead: Ahead,
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

    fn unlisted(&self, id: AnilistId) -> bool {
        self.ahead.unlisted(id)
    }

    fn unsourced(&self, id: AnilistId, episode: &str) -> bool {
        self.ahead.unsourced(id, episode)
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

/// Not asked off Unix: unknown room is taken as room. Windows has no safe way to ask in this
/// build's dependencies, and the workspace forbids unsafe code.
#[cfg(not(unix))]
fn free_space(_: &Path) -> Option<u64> {
    None
}

/// Top up, on `config`, for `viewer`: for each show missing what it keeps, the episodes of it
/// that are out, queued in order while they fit. A show stops at the first episode refused, and
/// the whole top-up at the first that would not fit, which is said once until one fits again.
pub(crate) async fn run(
    config: &Config,
    viewer: Viewer,
    work: &impl Work,
    store: &Store,
    said_full: &AtomicBool,
    tx: &Outbox,
) {
    for show in missing(config, store, viewer) {
        // Its episodes are AniList's alone: no source here has it to download.
        if work.unlisted(show.id) {
            continue;
        }
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
            // Out, and nothing lists it yet: the next top-up asks again.
            if work.unsourced(show.id, &episode) {
                continue;
            }
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

    use anistream_model::{Cause, command::AppState, notice::NoticeKind};
    use anistream_store::WatchEvent;
    use anistream_testkit::fixtures::{self, AIRING, FINISHED, ONA, SEQUEL};
    use tokio::sync::mpsc;

    use super::*;

    /// Plenty of room: a hundred episodes' worth over what is kept free.
    const ROOMY: u64 = KEEP_FREE + 100 * EPISODE_GUESS;

    /// The moment the tests are at.
    const NOW: i64 = 2_000_000_000;
    /// A viewer signed in to the tracker the list is from, at [`NOW`].
    const SIGNED_IN: Viewer = Viewer { now: NOW, listed: true };
    /// One with no tracker account, at [`NOW`].
    const NO_ACCOUNT: Viewer = Viewer { now: NOW, listed: false };

    /// A store with `shows` on the list as `status`.
    fn listed(shows: &[(AnilistId, &str)]) -> Store {
        let store = Store::open_in_memory().unwrap();
        store.replace_library_statuses("anilist", shows, 10).unwrap();
        store
    }

    /// Episodes `from..=upto` of `id` watched to the end, at `at`.
    fn watched_from(store: &Store, id: AnilistId, from: u32, upto: u32, at: i64) {
        for n in from..=upto {
            let mut event = WatchEvent::new(id, n.to_string(), 1_400.0);
            event.duration_secs = Some(1_440.0);
            event.completed = true;
            event.at = at;
            store.record_event(&event).unwrap();
        }
    }

    /// Episodes `1..=upto` of `id` watched to the end, at `at`.
    fn watched(store: &Store, id: AnilistId, upto: u32, at: i64) {
        watched_from(store, id, 1, upto, at);
    }

    /// Episode `n` of `id` started, at `at`.
    fn started(store: &Store, id: AnilistId, n: u32, at: i64) {
        let mut event = WatchEvent::new(id, n.to_string(), 300.0);
        (event.duration_secs, event.at) = (Some(1_440.0), at);
        store.record_event(&event).unwrap();
    }

    fn keeping(n: u32) -> Config {
        let mut config = Config::default();
        config.downloads.keep_next = n;
        config
    }

    /// Each show and what it is missing.
    fn wants(config: &Config, store: &Store, viewer: Viewer) -> Vec<(AnilistId, Vec<u32>)> {
        missing(config, store, viewer).into_iter().map(|m| (m.id, m.episodes)).collect()
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

    /// Episodes `episodes` of `id` downloaded ahead, finished.
    fn kept(store: &Store, id: AnilistId, episodes: &[u32]) {
        for n in episodes {
            let row = store.enqueue_download_ahead(id, &n.to_string(), "Show", "m").unwrap();
            store.finish_download(row.id, None).unwrap();
        }
    }

    /// The episodes of `id` that [`expired`] lets go of.
    fn going(store: &Store, id: AnilistId, viewer: Viewer) -> Vec<String> {
        let mut going: Vec<_> = expired(store, viewer)
            .into_iter()
            .filter(|row| row.anilist_id == id)
            .map(|row| row.episode)
            .collect();
        going.sort();
        going
    }

    /// A top-up's work from memory: the fixtures' schedules, `free` bytes free, and each
    /// episode queued into `store` as asked.
    struct Fake {
        store: Store,
        free: Option<u64>,
        media: HashMap<AnilistId, Media>,
        asked: Mutex<Vec<(AnilistId, String, Wanted)>>,
        /// Which shows no source lists, as the engine's record has them.
        ahead: Ahead,
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
                ahead: Ahead::default(),
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
                .map_err(|e| Refusal::Title(Cause::verbatim(e.to_string())))
        }

        fn free(&self, _: &Config) -> Option<u64> {
            self.free
        }

        fn unlisted(&self, id: AnilistId) -> bool {
            self.ahead.unlisted(id)
        }

        fn unsourced(&self, id: AnilistId, episode: &str) -> bool {
            self.ahead.unsourced(id, episode)
        }
    }

    /// Run one top-up on `config` with `work`, signed in, and return what it said.
    async fn top_up_with(config: &Config, work: &Fake, said_full: &AtomicBool) -> Vec<Event> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        run(config, SIGNED_IN, work, &work.store, said_full, &Outbox::from(tx)).await;
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn toasts(said: &[Event]) -> Vec<Msg> {
        said.iter()
            .filter_map(|event| match event {
                Event::Toast(notice) => Some(notice.message()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_number_kept_is_settings_or_the_shows_own() {
        let store = listed(&[(FINISHED, "CURRENT"), (SEQUEL, "REPEATING"), (ONA, "CURRENT")]);
        watched(&store, FINISHED, 4, NOW - DAY);
        // A show that keeps more for itself, and one that keeps none.
        store.set_show_setting(SEQUEL, "keep_next", Some("3"), 1).unwrap();
        store.set_show_setting(ONA, "keep_next", Some("0"), 1).unwrap();

        assert_eq!(
            wants(&keeping(2), &store, SIGNED_IN),
            [(FINISHED, vec![5, 6]), (SEQUEL, vec![1, 2, 3])],
            "the last watched first, each with its own number",
        );
        // Off in Settings, a show that keeps its own still keeps them.
        assert_eq!(wants(&keeping(0), &store, SIGNED_IN), [(SEQUEL, vec![1, 2, 3])]);
        let off = listed(&[(FINISHED, "CURRENT")]);
        assert!(wants(&keeping(0), &off, SIGNED_IN).is_empty());
    }

    #[test]
    fn a_viewer_with_no_tracker_account_keeps_the_shows_watched_here_lately() {
        // On the list, but nobody is signed in to it any more: it says nothing.
        let store = listed(&[(SEQUEL, "CURRENT"), (ONA, "PAUSED")]);
        watched(&store, FINISHED, 4, NOW - 29 * DAY);
        watched(&store, ONA, 2, NOW - DAY);
        watched(&store, AIRING, 2, NOW - WATCHED_WITHIN - DAY);
        assert_eq!(
            wants(&keeping(1), &store, NO_ACCOUNT),
            [(ONA, vec![3]), (FINISHED, vec![5])],
            "watched here in the last 30 days, the list's pause or not",
        );
        // Signed in, the list counts: its show, and not the one it has paused.
        assert_eq!(
            wants(&keeping(1), &store, SIGNED_IN),
            [(FINISHED, vec![5]), (SEQUEL, vec![1])],
        );
    }

    #[test]
    fn the_episodes_kept_are_the_ones_after_where_the_viewer_is() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, 300);
        // Episode 6 watched out of order, and episode 5 left part way.
        watched_from(&store, FINISHED, 6, 6, 310);
        started(&store, FINISHED, 5, 320);
        let show = |id| Show { id, rewatching: false };
        assert_eq!(next_episodes(&store, show(FINISHED), 3), [5, 7, 8], "6 is watched already");

        // Nothing watched here: from the first.
        assert_eq!(next_episodes(&store, show(SEQUEL), 2), [1, 2]);
    }

    #[test]
    fn a_rewatch_keeps_the_episodes_after_the_one_rewatched() {
        let store = listed(&[(FINISHED, "REPEATING")]);
        watched(&store, FINISHED, 12, NOW - 100 * DAY);
        watched(&store, FINISHED, 3, NOW - DAY);
        assert_eq!(wants(&keeping(2), &store, SIGNED_IN), [(FINISHED, vec![4, 5])]);
        // Partway into the fourth again.
        started(&store, FINISHED, 4, NOW - 60);
        assert_eq!(wants(&keeping(2), &store, SIGNED_IN), [(FINISHED, vec![4, 5])]);
        // Not a rewatch the list knows of: Continue's next, past the last episode, which the
        // schedule then rules out.
        let current = listed(&[(FINISHED, "CURRENT")]);
        watched(&current, FINISHED, 12, NOW - 100 * DAY);
        watched(&current, FINISHED, 3, NOW - DAY);
        assert_eq!(wants(&keeping(2), &current, SIGNED_IN), [(FINISHED, vec![13, 14])]);
    }

    #[test]
    fn an_episode_with_a_row_in_the_queue_is_not_asked_for_again() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 2, NOW - DAY);
        let done = store.enqueue_download(FINISHED, "3", "Show", "m").unwrap();
        store.finish_download(done.id, None).unwrap();
        let failed = store.enqueue_download_ahead(FINISHED, "5", "Show", "m").unwrap();
        store.fail_download(failed.id, "no peers").unwrap();
        let paused = store.enqueue_download(FINISHED, "6", "Show", "m").unwrap();
        store.set_download_state(paused.id, DownloadState::Paused).unwrap();
        store.enqueue_download_ahead(FINISHED, "7", "Show", "m").unwrap();
        assert_eq!(wants(&keeping(6), &store, SIGNED_IN), [(FINISHED, vec![4, 8])]);

        // Cancelled by the viewer: not queued again.
        let cancelled = store.download_for(FINISHED, "7").unwrap().unwrap();
        store.remove_download(cancelled.id).unwrap();
        assert_eq!(wants(&keeping(6), &store, SIGNED_IN), [(FINISHED, vec![4, 8])]);
    }

    #[tokio::test]
    async fn a_top_up_queues_what_is_missing_once_in_the_shows_own_quality() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, NOW - DAY);
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
        watched(&store, AIRING, 3, NOW - DAY);
        watched(&store, FINISHED, 10, NOW - 2 * DAY);
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

    /// Show `id`'s table, `metadata_only` or listed by a source.
    fn table(id: AnilistId, metadata_only: bool) -> Event {
        let row = |n: u32| anistream_model::episodes::EpisodeRow {
            number: n.to_string(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: None,
            delayed_from: None,
            metadata_only,
            source: None,
            unsourced: false,
            past_progress: false,
        };
        Event::Episodes { id, rows: (1..=12).map(row).collect(), failed: None }
    }

    #[tokio::test]
    async fn an_episode_no_source_lists_is_not_kept() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, NOW - DAY);
        let work = Fake::on(&store);
        // The table lists 1 to 12, and says 6 and 7 are out with nobody to download them from.
        let Event::Episodes { id, mut rows, .. } = table(FINISHED, false) else {
            unreachable!()
        };
        rows[5].unsourced = true;
        rows[6].unsourced = true;
        work.ahead.observe(&Event::Episodes { id, rows, failed: None });
        top_up_with(&keeping(3), &work, &AtomicBool::new(false)).await;
        assert_eq!(
            work.asked(),
            [(FINISHED, "5".into())],
            "6 and 7 wait for a source; the count is not made up with later ones"
        );
    }

    #[tokio::test]
    async fn a_show_no_source_could_list_is_not_kept_until_one_does() {
        let store = listed(&[(FINISHED, "CURRENT"), (AIRING, "CURRENT")]);
        watched(&store, FINISHED, 4, NOW - DAY);
        watched(&store, AIRING, 2, NOW - 2 * DAY);
        let work = Fake::on(&store);
        // Its table came from AniList alone; an empty one says nothing either way.
        work.ahead.observe(&table(FINISHED, true));
        work.ahead.observe(&Event::Episodes { id: FINISHED, rows: Vec::new(), failed: None });
        assert!(work.ahead.unlisted(FINISHED));
        top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        assert_eq!(
            work.asked(),
            [(AIRING, "3".into()), (AIRING, "4".into())],
            "only the other"
        );

        // Listed by a source since: kept as any other show.
        work.ahead.observe(&table(FINISHED, false));
        assert!(!work.ahead.unlisted(FINISHED));
        top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        assert_eq!(work.asked()[2..], [(FINISHED, "5".into()), (FINISHED, "6".into())]);
    }

    #[tokio::test]
    async fn a_show_that_stops_being_watched_keeps_what_it_has_and_is_topped_up_no_more() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        watched(&store, FINISHED, 4, NOW - WATCHED_WITHIN - 2 * DAY);
        let work = Fake::on(&store);
        top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        assert_eq!(queued(&store, FINISHED).len(), 2);

        // Not watched here lately, and off the list's watching.
        for status in ["COMPLETED", "PLANNING"] {
            store.replace_library_statuses("anilist", &[(FINISHED, status)], 20).unwrap();
            assert!(wants(&keeping(2), &store, SIGNED_IN).is_empty(), "{status}");
            top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        }
        // Set aside on the list, however lately it was watched here.
        for status in ["PAUSED", "DROPPED"] {
            store.replace_library_statuses("anilist", &[(FINISHED, status)], 20).unwrap();
            watched_from(&store, FINISHED, 5, 5, NOW - 60);
            assert!(wants(&keeping(2), &store, SIGNED_IN).is_empty(), "{status}");
            top_up_with(&keeping(2), &work, &AtomicBool::new(false)).await;
        }
        assert_eq!(work.asked().len(), 2, "asked for nothing more");
        assert_eq!(queued(&store, FINISHED), [("5".into(), true), ("6".into(), true)]);
    }

    #[test]
    fn an_episode_kept_ahead_goes_a_day_after_it_is_watched() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        kept(&store, FINISHED, &[3, 4, 5]);
        // Three a second over a day ago, four a second short of one, five not yet.
        watched(&store, FINISHED, 3, NOW - GRACE - 1);
        watched_from(&store, FINISHED, 4, 4, NOW - GRACE + 1);
        assert_eq!(going(&store, FINISHED, SIGNED_IN), ["3"]);
        assert_eq!(going(&store, FINISHED, Viewer { now: NOW + 2, ..SIGNED_IN }), ["3", "4"]);

        // Its file goes with it, and the queue is said; nothing is kept as let go of.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("03.mkv");
        std::fs::write(&file, b"an episode").unwrap();
        let three = store.download_for(FINISHED, "3").unwrap().unwrap();
        store.finish_download(three.id, Some(file.to_str().unwrap())).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert_eq!(expire(&store, SIGNED_IN, &Outbox::from(tx)), 1);
        assert!(!file.exists());
        assert!(matches!(rx.try_recv(), Ok(Event::Downloads(_))));
        assert_eq!(queued(&store, FINISHED), [("4".into(), true), ("5".into(), true)]);
        assert!(!store.download_dismissed(FINISHED, "3").unwrap());
        // Nothing more to go: nothing said.
        let (tx, mut rx) = mpsc::unbounded_channel();
        assert_eq!(expire(&store, SIGNED_IN, &Outbox::from(tx)), 0);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn a_download_the_viewer_asked_for_is_never_let_go_of() {
        let store = listed(&[(FINISHED, "CURRENT")]);
        let mine = store.enqueue_download(FINISHED, "2", "Show", "m").unwrap();
        store.finish_download(mine.id, None).unwrap();
        // Kept ahead, then asked for by hand.
        kept(&store, FINISHED, &[3]);
        store.enqueue_download(FINISHED, "3", "Show", "m").unwrap();
        watched(&store, FINISHED, 4, NOW - 10 * DAY);
        assert!(going(&store, FINISHED, SIGNED_IN).is_empty());
    }

    #[test]
    fn a_rewatch_keeps_the_episode_the_viewer_is_on_and_the_ones_after() {
        let store = listed(&[(FINISHED, "REPEATING")]);
        // Watched through long ago, with four to six downloaded ahead back then.
        watched(&store, FINISHED, 12, NOW - 100 * DAY);
        kept(&store, FINISHED, &[2, 4, 5, 6]);
        // Rewatched to three, two of them days ago, and partway into four now.
        watched(&store, FINISHED, 3, NOW - 3 * DAY);
        watched_from(&store, FINISHED, 3, 3, NOW - 2 * 3_600);
        started(&store, FINISHED, 4, NOW - 60);
        assert_eq!(going(&store, FINISHED, SIGNED_IN), ["2"]);

        // Four finished just now: five, not started, is where the viewer is.
        watched_from(&store, FINISHED, 4, 4, NOW - 30);
        assert_eq!(going(&store, FINISHED, SIGNED_IN), ["2"]);
        // A day on, four goes too.
        assert_eq!(
            going(&store, FINISHED, Viewer { now: NOW + GRACE, ..SIGNED_IN }),
            ["2", "4"]
        );
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
        use crate::phases::Network;
        let dir = tempfile::tempdir().unwrap();
        let paths = anistream_core::config::Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let (arrivals, _) = mpsc::unbounded_channel();
        let mut engine = EngineState::new(Config::default(), None, &paths, arrivals);
        assert!(!downloads_wait(&engine), "on screen, on Wi-Fi");
        let cellular = Network { cellular: true, ..Network::default() };
        engine.network = cellular;
        assert!(downloads_wait(&engine), "Wi-Fi only");
        engine.config.effective_mut().downloads.on_cellular = true;
        assert!(!downloads_wait(&engine), "downloads on cellular");
        engine.network = Network { constrained: true, ..Network::default() };
        assert!(downloads_wait(&engine), "Low Data Mode");
        engine.network = Network::default();
        engine.phase = AppState::Background { playing: true };
        assert!(downloads_wait(&engine));
        engine.phase = AppState::Background { playing: false };
        assert!(downloads_wait(&engine));
        engine.phase = AppState::BackgroundDownloads;
        assert!(!downloads_wait(&engine), "a downloads window");
    }

    #[test]
    fn what_moves_a_viewer_on_and_the_settings_keep_next_follows() {
        use anistream_model::playback::Ending;
        assert!(after(&Event::ProgressQueued));
        assert!(after(&Event::playback_ended(true, Ending::Complete { next: None })));
        assert!(!after(&Event::playback_ended(false, Ending::Stopped)));
        assert!(!after(&Event::Downloads(Vec::new())));
        assert!(!after(&Event::NewEpisodes(Vec::new())), "nothing new");
        let new = anistream_model::notifications::NewEpisode {
            id: AIRING,
            title: "Show".into(),
            episode: 6,
            aired_at: NOW,
            cover_url: None,
            next_for_you: None,
            translation: None,
            banner_url: None,
        };
        assert!(after(&Event::NewEpisodes(vec![new])));
        assert!(follows(SettingKey::KeepNextEpisodes));
        assert!(follows(SettingKey::DownloadsOnCellular));
        assert!(!follows(SettingKey::DownloadQuality));
    }
}
