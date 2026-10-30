//! Which of an episode's servers to try first, by how each has fared lately, and what each
//! attempt on one says of it.
//!
//! The record is the store's, in [`anistream_store::server_health`]. This is the rule that reads
//! it, and the part of a playback that writes to it.
//!
//! # The rule
//!
//! A server is **demoted** while it is on a run of [`DEMOTE_AFTER`] or more failures, the last
//! of them within [`DEMOTE_WINDOW_SECS`]. Once the streams are ranked by quality, a demoted
//! server's streams move behind the healthy streams *of the same quality*, the least recently
//! failed first; everything else keeps its order. Nothing is removed: a demoted server is still
//! tried, after the others, and one success ends its run and restores it.
//!
//! Quality outranks health. The ranking is the viewer's setting, while health is a guess from the
//! last few plays, and the two cost different things when wrong: trying a server that has been
//! failing costs a failover, but preferring a healthy 720p to a flaky 1080p would cost the whole
//! episode at the worse picture, with nothing on screen to say why. Within one quality the
//! servers are alike, and the one that has been working is the obvious first try.
//!
//! A release the viewer picked by hand is never reordered: it plays as it was picked.

use std::{collections::HashMap, time::Instant};

use anistream_core::stream::Stream;
use anistream_store::{
    Store,
    server_health::{SERVER_RETENTION_SECS, ServerFailure, ServerHealth, ServerKey},
};

/// The run of failures that puts a server behind its healthy peers. More than one, because a
/// single failure is as often the network as the server.
pub const DEMOTE_AFTER: u32 = 2;

/// How recent the last of those failures must be. A server that has gone a day without failing
/// is given its place back, and tried first again.
pub const DEMOTE_WINDOW_SECS: i64 = 24 * 60 * 60;

/// The server `stream` is served from, when it has one to keep a record of.
pub fn server(stream: &Stream) -> Option<ServerKey> {
    ServerKey::of(&stream.provider_id, &stream.url)
}

/// Whether `health` puts its server behind its healthy peers, as of `now`.
pub fn demoted(health: &ServerHealth, now: i64) -> bool {
    health.consecutive_failures >= DEMOTE_AFTER
        && health.last_failure_at.is_some_and(|at| now - at <= DEMOTE_WINDOW_SECS)
}

/// What [`reorder`] did.
#[derive(Debug, Default)]
pub struct Reordered {
    /// Whether any stream moved.
    pub moved: bool,
    /// The demoted servers, once each, in the order they are now tried.
    pub demoted: Vec<ServerHealth>,
}

/// Move each demoted server's streams in `streams`, already ranked by quality toward `desired`,
/// behind the healthy streams of the same quality, the least recently failed first. `health`
/// says what is known of a stream's server.
pub fn reorder(
    streams: &mut Vec<Stream>,
    desired: u32,
    health: impl Fn(&Stream) -> Option<ServerHealth>,
    now: i64,
) -> Reordered {
    let records: Vec<Option<ServerHealth>> =
        streams.iter().map(|s| health(s).filter(|h| demoted(h, now))).collect();
    let mut order: Vec<usize> = (0..streams.len()).collect();
    // Stable, so the healthy keep the order they were ranked in, and so do demoted servers that
    // last failed at the same moment.
    order.sort_by_key(|&i| {
        let failed = records[i].as_ref().and_then(|h| h.last_failure_at);
        (streams[i].quality_rank(desired), records[i].is_some(), failed.unwrap_or(i64::MIN))
    });

    let mut demoted: Vec<ServerHealth> = Vec::new();
    for health in order.iter().filter_map(|&i| records[i].as_ref()) {
        if !demoted.iter().any(|d| d.key == health.key) {
            demoted.push(health.clone());
        }
    }
    let moved = order.iter().enumerate().any(|(at, &i)| at != i);
    if moved {
        let mut taken: Vec<Option<Stream>> =
            std::mem::take(streams).into_iter().map(Some).collect();
        *streams = order.iter().filter_map(|&i| taken[i].take()).collect();
    }
    Reordered { moved, demoted }
}

/// Reorder `streams` by what `store` knows of their servers, as [`reorder`] does: unless the
/// viewer `picked` them, which plays them as picked.
pub fn order(store: &Store, streams: &mut Vec<Stream>, desired: u32, picked: bool, now: i64) {
    if picked || streams.len() < 2 {
        return;
    }
    let mut known: HashMap<ServerKey, Option<ServerHealth>> = HashMap::new();
    for key in streams.iter().filter_map(server) {
        known.entry(key).or_insert_with_key(|key| {
            store
                .server_health(key, now)
                .inspect_err(
                    |e| tracing::debug!(error = %e, "could not read a server's health"),
                )
                .ok()
                .flatten()
        });
    }
    let lookup =
        |stream: &Stream| server(stream).and_then(|key| known.get(&key).cloned().flatten());
    let reordered = reorder(streams, desired, lookup, now);
    if reordered.demoted.is_empty() {
        return;
    }
    let provider = streams.first().map(|s| s.provider_id.clone()).unwrap_or_default();
    let demoted = describe(&reordered.demoted, now);
    if reordered.moved {
        tracing::info!(%provider, %demoted, "servers reordered by health");
    } else {
        tracing::debug!(%provider, %demoted, "the servers failing lately are already tried last");
    }
}

/// `[cdn-a.example (3 failures in a row, last 12 min ago)]`, for the log.
fn describe(demoted: &[ServerHealth], now: i64) -> String {
    let each: Vec<String> = demoted
        .iter()
        .map(|h| {
            let last = h
                .last_failure_at
                .map_or_else(String::new, |at| format!(", last {}", ago(now - at)));
            format!("{} ({} failures in a row{last})", h.key.host(), h.consecutive_failures)
        })
        .collect();
    format!("[{}]", each.join(", "))
}

/// A short note on a server that has been failing, for a row in the Sources menu: `failed the
/// last 3 times · 12 min ago`. `None` for a server with nothing against it, which is most.
pub fn note(health: &ServerHealth, now: i64) -> Option<String> {
    let health = health.clone().as_of(now);
    let at = health.last_failure_at?;
    let times = match health.consecutive_failures {
        0 => return None,
        1 => "failed last time".to_owned(),
        n => format!("failed the last {n} times"),
    };
    Some(format!("{times} · {}", ago(now - at)))
}

/// How long ago `secs` seconds was, in words.
fn ago(secs: i64) -> String {
    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    match secs.max(0) {
        s if s < MINUTE => "just now".into(),
        s if s < HOUR => format!("{} min ago", s / MINUTE),
        s if s < DAY => format!("{} h ago", s / HOUR),
        s if s < 2 * DAY => "yesterday".into(),
        s => format!("{} days ago", s / DAY),
    }
}

/// What an attempt that ended before showing a frame says of its server.
///
/// An error status from the host is the server's failure whoever ended the attempt; so is no
/// frame by the time the player was warned about it (`late`). Otherwise a file the player ended
/// itself is a load error, and one the viewer ended, by stopping or closing the window, says
/// nothing: they did not give the server its chance.
pub fn verdict(viewer_ended: bool, late: bool, refused: Option<u16>) -> Option<ServerFailure> {
    match refused {
        Some(status) => Some(ServerFailure::Http(status)),
        None if late => Some(ServerFailure::NoFrames),
        None if viewer_ended => None,
        None => Some(ServerFailure::Load),
    }
}

/// How long a playback's ending waits for its servers' records to reach the store.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(2);

/// What one playback knows of its streams' servers, kept current as it tries them, and the
/// attempt under way.
#[derive(Debug, Default)]
pub(crate) struct Servers {
    known: HashMap<ServerKey, ServerHealth>,
    /// When the attempt under way was loaded, until it shows a frame.
    began: Option<Instant>,
    /// Whether the attempt under way went past the first-frame timeout without one.
    late: bool,
    /// The store's writes still under way, the last made: each waits for the one before.
    writing: Option<tokio::task::JoinHandle<()>>,
}

impl Servers {
    /// What `store` knows of the servers `streams` are served from.
    pub(crate) async fn load(store: &Store, streams: &[Stream]) -> Self {
        let mut keys: Vec<ServerKey> = streams.iter().filter_map(server).collect();
        keys.sort();
        keys.dedup();
        let store = store.clone();
        let now = anistream_store::now();
        let known = tokio::task::spawn_blocking(move || {
            keys.into_iter()
                .filter_map(|key| {
                    let health = store.server_health(&key, now).ok().flatten()?;
                    Some((key, health))
                })
                .collect()
        })
        .await
        .unwrap_or_default();
        Self { known, began: None, late: false, writing: None }
    }

    /// A short note on how `stream`'s server has fared, when it has been failing.
    pub(crate) fn note(&self, stream: &Stream) -> Option<String> {
        note(self.known.get(&server(stream)?)?, anistream_store::now())
    }

    /// An attempt starts: its file is loading.
    pub(crate) fn begin(&mut self) {
        self.began = Some(Instant::now());
        self.late = false;
    }

    /// The attempt under way has gone past the first-frame timeout without a frame.
    pub(crate) fn late(&mut self) {
        self.late = true;
    }

    /// The attempt under way, on `stream`, showed its first frame.
    pub(crate) fn served(&mut self, store: &Store, stream: &Stream) {
        let Some(began) = self.began.take() else { return };
        let first_frame = began.elapsed();
        let Some(key) = server(stream) else { return };
        let at = anistream_store::now();
        let health = self.held(key, at).served(at, first_frame);
        tracing::debug!(
            provider = health.key.provider_id(),
            server = health.key.host(),
            first_frame_ms = first_frame.as_millis() as u64,
            successes = health.successes,
            "a server showed a frame"
        );
        self.keep(store, health, move |s, key| s.record_server_served(key, first_frame, at));
    }

    /// An attempt on `stream` failed as `failure`.
    pub(crate) fn failed(&mut self, store: &Store, stream: &Stream, failure: ServerFailure) {
        self.began = None;
        let Some(key) = server(stream) else { return };
        let at = anistream_store::now();
        let health = self.held(key, at).failed(at, failure);
        tracing::debug!(
            provider = health.key.provider_id(),
            server = health.key.host(),
            %failure,
            in_a_row = health.consecutive_failures,
            "a server failed"
        );
        self.keep(store, health, move |s, key| s.record_server_failure(key, failure, at));
    }

    /// The attempt on `stream` ended before it showed a frame: see [`verdict`]. `refused` is the
    /// error status the host answered the stream's requests with, when the mender saw one.
    pub(crate) fn unplayed(
        &mut self,
        store: &Store,
        stream: &Stream,
        viewer_ended: bool,
        refused: Option<u16>,
    ) {
        match verdict(viewer_ended, self.late, refused) {
            Some(failure) => self.failed(store, stream, failure),
            None => self.began = None,
        }
    }

    /// The records written to the store, for the next playback to read, waiting at most
    /// [`SETTLE`]: an ending is not held up by a slow disk either. A write still under way then
    /// lands on its own.
    pub(crate) async fn settle(&mut self) {
        if let Some(writing) = self.writing.take() {
            let _ = tokio::time::timeout(SETTLE, writing).await;
        }
    }

    /// What is known of `key`'s server as of `at`, or a first record for it.
    fn held(&self, key: ServerKey, at: i64) -> ServerHealth {
        let known = self.known.get(&key).filter(|h| at - h.seen_at <= SERVER_RETENTION_SECS);
        known.cloned().unwrap_or_else(|| ServerHealth::new(key, at))
    }

    /// Keep `health` as its server's record, and write it to the store with `write`.
    ///
    /// The playback goes on without waiting for the disk: a viewer's switch or the next event
    /// never waits on a write. The writes still land in the order they were made, each after
    /// the one before.
    fn keep(
        &mut self,
        store: &Store,
        health: ServerHealth,
        write: impl FnOnce(&Store, &ServerKey) -> anistream_store::Result<ServerHealth>
        + Send
        + 'static,
    ) {
        let (store, key) = (store.clone(), health.key.clone());
        self.known.insert(key.clone(), health);
        let before = self.writing.take();
        self.writing = Some(tokio::spawn(async move {
            if let Some(before) = before {
                let _ = before.await;
            }
            let written =
                tokio::task::spawn_blocking(move || write(&store, &key).map_err(|e| (e, key)))
                    .await;
            if let Ok(Err((e, key))) = written {
                tracing::warn!(error = %e, server = key.host(), "could not record a server's health");
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_core::stream::StreamKind;

    use super::*;

    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const NOW: i64 = 1_790_000_000;

    fn at(origin: &str, quality: Option<u32>) -> Stream {
        let mut stream = Stream {
            provider_id: "plugin".into(),
            ..Stream::new(format!("{origin}/ep-1/master.m3u8?token=t"), StreamKind::Hls)
        };
        stream.quality = quality;
        stream
    }

    /// A record of `failures` failures in a row, the last `ago` seconds before [`NOW`].
    fn failing(origin: &str, failures: u32, ago: i64) -> ServerHealth {
        let key = ServerKey::of("plugin", origin).unwrap();
        let mut health = ServerHealth {
            key,
            successes: 0,
            failures: 0,
            consecutive_failures: 0,
            last_success_at: None,
            last_failure_at: None,
            last_failure: None,
            first_frame: None,
            seen_at: NOW - ago,
        };
        for _ in 0..failures {
            health = health.failed(NOW - ago, ServerFailure::Http(502));
        }
        health
    }

    fn hosts(streams: &[Stream]) -> Vec<String> {
        streams
            .iter()
            .map(|s| server(s).map(|k| k.host().to_owned()).unwrap_or_default())
            .collect()
    }

    /// Reorder against these records, by host.
    fn reordered(streams: &mut Vec<Stream>, records: &[ServerHealth]) -> Reordered {
        let records = records.to_vec();
        reorder(
            streams,
            1080,
            move |s| {
                let key = server(s)?;
                records.iter().find(|h| h.key == key).cloned()
            },
            NOW,
        )
    }

    #[test]
    fn two_failures_in_a_row_today_demote_a_server_and_one_does_not() {
        assert!(!demoted(&failing("https://cdn-a.example", 1, MINUTE), NOW));
        assert!(demoted(&failing("https://cdn-a.example", 2, MINUTE), NOW));
        assert!(demoted(&failing("https://cdn-a.example", 2, 23 * HOUR), NOW));
        assert!(!demoted(&failing("https://cdn-a.example", 5, 25 * HOUR), NOW), "a day on");
        // And one success restores it.
        let served = failing("https://cdn-a.example", 4, HOUR)
            .served(NOW - MINUTE, Duration::from_secs(1));
        assert!(!demoted(&served, NOW));
    }

    #[test]
    fn a_server_that_keeps_failing_goes_behind_its_healthy_peers() {
        let mut streams = vec![
            at("https://cdn-a.example", None),
            at("https://cdn-b.example", None),
            at("https://cdn-c.example", None),
        ];
        let result =
            reordered(&mut streams, &[failing("https://cdn-a.example", 3, 12 * MINUTE)]);
        assert_eq!(hosts(&streams), ["cdn-b.example", "cdn-c.example", "cdn-a.example"]);
        assert!(result.moved);
        assert_eq!(result.demoted.len(), 1);
        assert_eq!(
            describe(&result.demoted, NOW),
            "[cdn-a.example (3 failures in a row, last 12 min ago)]"
        );
    }

    #[test]
    fn the_least_recently_failed_of_the_demoted_goes_first() {
        let mut streams = vec![
            at("https://cdn-a.example", None),
            at("https://cdn-b.example", None),
            at("https://cdn-c.example", None),
            at("https://cdn-d.example", None),
        ];
        reordered(
            &mut streams,
            &[
                failing("https://cdn-a.example", 2, MINUTE),
                failing("https://cdn-b.example", 4, 3 * HOUR),
            ],
        );
        assert_eq!(
            hosts(&streams),
            ["cdn-c.example", "cdn-d.example", "cdn-b.example", "cdn-a.example"]
        );
    }

    #[test]
    fn quality_outranks_health() {
        // A healthy 720p is not preferred to a 1080p that keeps failing: the demoted 1080p is
        // tried after the other 1080p, and still before anything at 720p.
        let mut streams = vec![
            at("https://cdn-a.example", Some(1080)),
            at("https://cdn-b.example", Some(1080)),
            at("https://cdn-c.example", Some(720)),
            at("https://cdn-d.example", Some(720)),
        ];
        reordered(
            &mut streams,
            &[
                failing("https://cdn-a.example", 3, MINUTE),
                failing("https://cdn-c.example", 3, MINUTE),
            ],
        );
        assert_eq!(
            hosts(&streams),
            ["cdn-b.example", "cdn-a.example", "cdn-d.example", "cdn-c.example"]
        );

        // The only 1080p keeps its place, however it has been doing.
        let mut lone = vec![
            at("https://cdn-a.example", Some(1080)),
            at("https://cdn-b.example", Some(720)),
        ];
        let result = reordered(&mut lone, &[failing("https://cdn-a.example", 5, MINUTE)]);
        assert_eq!(hosts(&lone), ["cdn-a.example", "cdn-b.example"]);
        assert!(!result.moved);
    }

    #[test]
    fn nothing_is_removed_and_the_healthy_keep_their_order() {
        let mut streams = vec![
            at("https://cdn-a.example", None),
            at("https://cdn-b.example", None),
            at("https://cdn-a.example", None),
            // A torrent's loopback stream names no server, and keeps its place among the healthy.
            Stream {
                provider_id: "plugin".into(),
                ..Stream::new("http://127.0.0.1:4040/1", StreamKind::TorrentHttp)
            },
            at("https://cdn-c.example", None),
        ];
        let urls: Vec<String> = streams.iter().map(|s| s.url.clone()).collect();
        let result = reordered(&mut streams, &[failing("https://cdn-a.example", 2, MINUTE)]);
        assert_eq!(
            hosts(&streams),
            ["cdn-b.example", "", "cdn-c.example", "cdn-a.example", "cdn-a.example"]
        );
        assert_eq!(result.demoted.len(), 1, "a server once, however many streams it has");
        let mut after: Vec<String> = streams.iter().map(|s| s.url.clone()).collect();
        let mut before = urls;
        before.sort();
        after.sort();
        assert_eq!(after, before, "every stream is still there");
    }

    #[test]
    fn with_nothing_against_any_server_nothing_moves() {
        let mut streams =
            vec![at("https://cdn-a.example", None), at("https://cdn-b.example", None)];
        let result = reordered(&mut streams, &[failing("https://cdn-a.example", 1, MINUTE)]);
        assert!(!result.moved && result.demoted.is_empty());
        assert_eq!(hosts(&streams), ["cdn-a.example", "cdn-b.example"]);
    }

    #[test]
    fn a_pick_is_played_as_it_was_picked() {
        let store = Store::open_in_memory().unwrap();
        let a = ServerKey::of("plugin", "https://cdn-a.example").unwrap();
        for _ in 0..3 {
            store.record_server_failure(&a, ServerFailure::Http(502), NOW).unwrap();
        }
        let streams =
            vec![at("https://cdn-a.example", None), at("https://cdn-b.example", None)];

        let mut picked = streams.clone();
        order(&store, &mut picked, 1080, true, NOW);
        assert_eq!(picked, streams, "a release picked by hand is never reordered");

        let mut automatic = streams.clone();
        order(&store, &mut automatic, 1080, false, NOW);
        assert_eq!(hosts(&automatic), ["cdn-b.example", "cdn-a.example"]);
    }

    #[test]
    fn a_failing_server_is_noted_in_words() {
        assert_eq!(
            note(&failing("https://cdn-a.example", 3, 12 * MINUTE), NOW).as_deref(),
            Some("failed the last 3 times · 12 min ago")
        );
        assert_eq!(
            note(&failing("https://cdn-a.example", 1, 5), NOW).as_deref(),
            Some("failed last time · just now")
        );
        let served =
            failing("https://cdn-a.example", 3, HOUR).served(NOW, Duration::from_secs(1));
        assert_eq!(note(&served, NOW), None, "nothing against it now");
        assert_eq!(note(&failing("https://cdn-a.example", 2, 8 * 24 * HOUR), NOW), None);
        assert_eq!(ago(3 * HOUR), "3 h ago");
        assert_eq!(ago(30 * HOUR), "yesterday");
        assert_eq!(ago(80 * HOUR), "3 days ago");
    }

    #[test]
    fn an_attempt_that_never_played_is_judged_by_how_it_ended() {
        assert_eq!(verdict(false, false, Some(502)), Some(ServerFailure::Http(502)));
        assert_eq!(
            verdict(true, false, Some(403)),
            Some(ServerFailure::Http(403)),
            "whoever ended it"
        );
        assert_eq!(verdict(false, true, None), Some(ServerFailure::NoFrames));
        assert_eq!(verdict(true, true, None), Some(ServerFailure::NoFrames));
        assert_eq!(verdict(false, false, None), Some(ServerFailure::Load));
        assert_eq!(
            verdict(true, false, None),
            None,
            "the viewer stopped it before it had a chance"
        );
    }

    #[tokio::test]
    async fn a_record_is_kept_at_once_and_reaches_the_store_in_the_order_made() {
        let store = Store::open_in_memory().unwrap();
        let stream = at("https://cdn-a.example", None);
        let mut servers = Servers::load(&store, std::slice::from_ref(&stream)).await;
        servers.begin();
        servers.served(&store, &stream);
        servers.failed(&store, &stream, ServerFailure::Stalled);
        // Known to this playback before the store has it: the menus read it from here.
        let key = server(&stream).unwrap();
        let kept = servers.known.get(&key).cloned().unwrap();
        assert_eq!((kept.successes, kept.failures, kept.consecutive_failures), (1, 1, 1));

        servers.settle().await;
        let stored = store.server_health(&key, anistream_store::now()).unwrap().unwrap();
        assert_eq!((stored.successes, stored.failures, stored.consecutive_failures), (1, 1, 1));
        assert_eq!(stored.last_failure, Some(ServerFailure::Stalled));
    }
}
