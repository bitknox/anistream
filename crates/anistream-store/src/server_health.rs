//! How each server a provider streams from has fared lately.
//!
//! A provider often offers an episode from several servers, and playback falls over from one to
//! the next in order. With no memory of how they did, a server that answered `502` on every play
//! this evening was still the one tried first on the next, and every episode began by waiting for
//! it to fail. This keeps a short record per server so the engine can try it later: not to take
//! it out of rotation, only to know which to try first.
//!
//! **What a server is.** The provider, and the origin its stream is served from:
//! `scheme://host[:port]`. Never a path, a query or a token, which is what a signed stream
//! address carries, so nothing kept here can be replayed or names what was watched. An address
//! that is not on the web, or is on this machine — a torrent's loopback server, a local file —
//! names no server and has no record.
//!
//! **What is kept.** How many plays the server served and how many failed, the run of failures
//! it is on, when it last served and last failed and how, and a smoothed time to the first frame.
//!
//! **What is forgotten.** A success ends a run of failures. A week with no failure forgets the
//! failures as well, so a bad evening a month ago counts for nothing. A server not tried for 30
//! days is deleted outright, so the table stays the size of the set of servers in use.

use std::{fmt, time::Duration};

use rusqlite::OptionalExtension;

use crate::{Result, Store};

/// How long a failure counts for: a server whose last failure is older than this reads as having
/// none.
pub const FAILURE_MEMORY_SECS: i64 = 7 * 24 * 60 * 60;

/// How long a server's record is kept after it was last tried.
pub const SERVER_RETENTION_SECS: i64 = 30 * 24 * 60 * 60;

/// How much of each new time to first frame the smoothed one takes: enough to follow a server
/// that has become slow within a few plays, not so much that one slow start defines it.
const FIRST_FRAME_WEIGHT: f64 = 0.3;

/// One server a provider streams from: the provider, and the origin of the stream's address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServerKey {
    provider_id: String,
    origin: String,
}

impl ServerKey {
    /// The server `url` is served from, as `provider_id` offered it.
    ///
    /// `None` where there is no server to keep a record of: an address that is not `http` or
    /// `https` — a local file, a magnet, a test pattern — or one on this machine, which is the
    /// torrent session's, or the mender's, never a host's.
    pub fn of(provider_id: &str, url: &str) -> Option<Self> {
        let (scheme, rest) = url.split_once("://")?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return None;
        }
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        // Credentials in the authority are not the server's, and must never be kept.
        let authority = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
        let authority = authority.to_ascii_lowercase();
        let host = match authority.strip_prefix('[') {
            Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
            None => authority.rsplit_once(':').map_or(authority.as_str(), |(host, _)| host),
        };
        let local = host == "localhost"
            || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if host.is_empty() || local {
            return None;
        }
        Some(Self {
            provider_id: provider_id.to_owned(),
            origin: format!("{scheme}://{authority}"),
        })
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// `https://cdn-a.example`.
    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// The origin without its scheme, for a log line: `cdn-a.example`.
    pub fn host(&self) -> &str {
        self.origin.split_once("://").map_or(self.origin.as_str(), |(_, host)| host)
    }
}

/// How an attempt on a server failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerFailure {
    /// The host answered a request for the stream with this HTTP error status.
    Http(u16),
    /// The file ended in an error before a frame was shown.
    Load,
    /// No frame came within the time a first frame is waited for.
    NoFrames,
    /// It stalled mid-episode, and the viewer took another stream.
    Stalled,
}

impl ServerFailure {
    /// As the table keeps it: `http-502`, `load`, `no-frames`, `stalled`.
    pub fn code(self) -> String {
        match self {
            Self::Http(status) => format!("http-{status}"),
            Self::Load => "load".into(),
            Self::NoFrames => "no-frames".into(),
            Self::Stalled => "stalled".into(),
        }
    }

    /// Read back what [`Self::code`] wrote; `None` for anything else.
    pub fn from_code(code: &str) -> Option<Self> {
        Some(match code {
            "load" => Self::Load,
            "no-frames" => Self::NoFrames,
            "stalled" => Self::Stalled,
            other => Self::Http(other.strip_prefix("http-")?.parse().ok()?),
        })
    }
}

impl fmt::Display for ServerFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http(status) => write!(f, "HTTP {status}"),
            Self::Load => f.write_str("load error"),
            Self::NoFrames => f.write_str("no frames"),
            Self::Stalled => f.write_str("stalled"),
        }
    }
}

/// One server's record.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerHealth {
    pub key: ServerKey,
    /// Attempts that showed a frame.
    pub successes: u32,
    /// Failures within the last [`FAILURE_MEMORY_SECS`].
    pub failures: u32,
    /// The run of failures the server is on: ended by a success, or by a week without failing.
    pub consecutive_failures: u32,
    pub last_success_at: Option<i64>,
    pub last_failure_at: Option<i64>,
    /// How it failed last, even once the failure itself no longer counts.
    pub last_failure: Option<ServerFailure>,
    /// Time to the first frame, smoothed over the plays it served.
    pub first_frame: Option<Duration>,
    /// When it was last tried, which is what keeps it from being pruned.
    pub seen_at: i64,
}

impl ServerHealth {
    fn new(key: ServerKey, at: i64) -> Self {
        Self {
            key,
            successes: 0,
            failures: 0,
            consecutive_failures: 0,
            last_success_at: None,
            last_failure_at: None,
            last_failure: None,
            first_frame: None,
            seen_at: at,
        }
    }

    /// The record as of `now`: failures older than [`FAILURE_MEMORY_SECS`] no longer count.
    ///
    /// The last failure is the newest, so when it is past the memory every one before it is too.
    pub fn as_of(mut self, now: i64) -> Self {
        if self.last_failure_at.is_some_and(|at| now - at > FAILURE_MEMORY_SECS) {
            self.failures = 0;
            self.consecutive_failures = 0;
        }
        self
    }

    /// The record after an attempt at `at` showed its first frame `first_frame` after loading.
    pub fn served(self, at: i64, first_frame: Duration) -> Self {
        let mut next = self.as_of(at);
        next.successes = next.successes.saturating_add(1);
        next.consecutive_failures = 0;
        next.last_success_at = Some(at);
        next.first_frame = Some(match next.first_frame {
            Some(smoothed) => {
                smoothed.mul_f64(1.0 - FIRST_FRAME_WEIGHT)
                    + first_frame.mul_f64(FIRST_FRAME_WEIGHT)
            }
            None => first_frame,
        });
        next.seen_at = next.seen_at.max(at);
        next
    }

    /// The record after an attempt at `at` failed as `failure`.
    pub fn failed(self, at: i64, failure: ServerFailure) -> Self {
        let mut next = self.as_of(at);
        next.failures = next.failures.saturating_add(1);
        next.consecutive_failures = next.consecutive_failures.saturating_add(1);
        next.last_failure_at = Some(at);
        next.last_failure = Some(failure);
        next.seen_at = next.seen_at.max(at);
        next
    }
}

impl Store {
    /// What is known of the server `key` names, as of `now`; `None` for a server never tried, or
    /// not tried within [`SERVER_RETENTION_SECS`].
    pub fn server_health(&self, key: &ServerKey, now: i64) -> Result<Option<ServerHealth>> {
        let found = self.with_conn(|c| read(c, key))?;
        Ok(found.filter(|h| now - h.seen_at <= SERVER_RETENTION_SECS).map(|h| h.as_of(now)))
    }

    /// Write down that an attempt on `key`'s server at `at` showed its first frame `first_frame`
    /// after loading, and return the record as it now stands.
    pub fn record_server_served(
        &self,
        key: &ServerKey,
        first_frame: Duration,
        at: i64,
    ) -> Result<ServerHealth> {
        self.update_server(key, at, |health| health.served(at, first_frame))
    }

    /// Write down that an attempt on `key`'s server at `at` failed as `failure`, and return the
    /// record as it now stands.
    pub fn record_server_failure(
        &self,
        key: &ServerKey,
        failure: ServerFailure,
        at: i64,
    ) -> Result<ServerHealth> {
        self.update_server(key, at, |health| health.failed(at, failure))
    }

    /// Delete every server not tried within [`SERVER_RETENTION_SECS`] of `now`, returning how
    /// many went. Every record does this as it writes, so the table never outgrows a month.
    pub fn prune_servers(&self, now: i64) -> Result<usize> {
        self.with_conn(|c| prune(c, now))
    }

    fn update_server(
        &self,
        key: &ServerKey,
        at: i64,
        change: impl FnOnce(ServerHealth) -> ServerHealth,
    ) -> Result<ServerHealth> {
        self.with_tx(|tx| {
            // A record past its retention is a stranger's: start it again rather than carry
            // its counts into a new month.
            let known = read(tx, key)?.filter(|h| at - h.seen_at <= SERVER_RETENTION_SECS);
            let health = change(known.unwrap_or_else(|| ServerHealth::new(key.clone(), at)));
            write(tx, &health)?;
            prune(tx, at)?;
            Ok(health)
        })
    }
}

fn read(c: &rusqlite::Connection, key: &ServerKey) -> Result<Option<ServerHealth>> {
    Ok(c.query_row(
        "SELECT successes, failures, consecutive_failures, last_success_at, last_failure_at,
                last_failure, first_frame_ms, seen_at
         FROM server_health WHERE provider_id = ?1 AND origin = ?2",
        rusqlite::params![key.provider_id, key.origin],
        |r| {
            let count = |i: usize| r.get::<_, i64>(i).map(|n| u32::try_from(n).unwrap_or(0));
            Ok(ServerHealth {
                key: key.clone(),
                successes: count(0)?,
                failures: count(1)?,
                consecutive_failures: count(2)?,
                last_success_at: r.get(3)?,
                last_failure_at: r.get(4)?,
                last_failure: r
                    .get::<_, Option<String>>(5)?
                    .as_deref()
                    .and_then(ServerFailure::from_code),
                first_frame: r
                    .get::<_, Option<f64>>(6)?
                    .filter(|ms| ms.is_finite() && *ms >= 0.0)
                    .map(|ms| Duration::from_secs_f64(ms / 1000.0)),
                seen_at: r.get(7)?,
            })
        },
    )
    .optional()?)
}

fn write(c: &rusqlite::Connection, health: &ServerHealth) -> Result<()> {
    c.execute(
        "INSERT INTO server_health (provider_id, origin, successes, failures,
             consecutive_failures, last_success_at, last_failure_at, last_failure,
             first_frame_ms, seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(provider_id, origin) DO UPDATE SET
             successes            = excluded.successes,
             failures             = excluded.failures,
             consecutive_failures = excluded.consecutive_failures,
             last_success_at      = excluded.last_success_at,
             last_failure_at      = excluded.last_failure_at,
             last_failure         = excluded.last_failure,
             first_frame_ms       = excluded.first_frame_ms,
             seen_at              = excluded.seen_at",
        rusqlite::params![
            health.key.provider_id,
            health.key.origin,
            health.successes,
            health.failures,
            health.consecutive_failures,
            health.last_success_at,
            health.last_failure_at,
            health.last_failure.map(ServerFailure::code),
            health.first_frame.map(|d| d.as_secs_f64() * 1000.0),
            health.seen_at,
        ],
    )?;
    Ok(())
}

fn prune(c: &rusqlite::Connection, now: i64) -> Result<usize> {
    Ok(c.execute(
        "DELETE FROM server_health WHERE seen_at < ?1",
        [now.saturating_sub(SERVER_RETENTION_SECS)],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 60 * 60;
    const DAY: i64 = 24 * HOUR;
    /// Some evening, as a clock reads it.
    const NOW: i64 = 1_790_000_000;

    fn store() -> Store {
        Store::open_in_memory().expect("store")
    }

    fn key(origin: &str) -> ServerKey {
        ServerKey::of("plugin", origin).expect("a server")
    }

    #[test]
    fn a_server_is_its_origin_and_never_the_address() {
        let key = ServerKey::of(
            "plugin",
            "HTTPS://viewer:secret@CDN-A.example:8443/hls/ep-1/master.m3u8?token=signed#t=1",
        )
        .unwrap();
        assert_eq!(key.origin(), "https://cdn-a.example:8443");
        assert_eq!(key.host(), "cdn-a.example:8443");
        assert_eq!(key.provider_id(), "plugin");
        assert_eq!(
            ServerKey::of("plugin", "https://cdn-a.example").unwrap().origin(),
            "https://cdn-a.example"
        );
        assert_eq!(
            ServerKey::of("plugin", "http://[2001:db8::1]:80/x").unwrap().origin(),
            "http://[2001:db8::1]:80"
        );
    }

    #[test]
    fn what_is_not_a_server_on_the_web_has_no_record() {
        for url in [
            "http://127.0.0.1:4040/stream/1",
            "http://localhost:4040/stream/1",
            "http://[::1]:4040/stream/1",
            "file:///Users/someone/episode.mkv",
            "av://lavfi:testsrc=duration=4",
            "magnet:?xt=urn:btih:abc",
            "/an/absolute/path.mkv",
            "https:///no-host",
        ] {
            assert_eq!(ServerKey::of("plugin", url), None, "{url}");
        }
    }

    #[test]
    fn a_failure_code_reads_back_as_it_was_written() {
        for failure in [
            ServerFailure::Http(502),
            ServerFailure::Http(404),
            ServerFailure::Load,
            ServerFailure::NoFrames,
            ServerFailure::Stalled,
        ] {
            assert_eq!(ServerFailure::from_code(&failure.code()), Some(failure));
        }
        assert_eq!(ServerFailure::from_code("http-x"), None);
        assert_eq!(ServerFailure::from_code("something new"), None);
        assert_eq!(ServerFailure::Http(502).to_string(), "HTTP 502");
    }

    #[test]
    fn a_server_never_tried_has_no_record() {
        assert_eq!(store().server_health(&key("https://cdn-a.example"), NOW).unwrap(), None);
    }

    #[test]
    fn failures_run_on_until_a_success_ends_them() {
        let store = store();
        let a = key("https://cdn-a.example");
        store.record_server_failure(&a, ServerFailure::Http(502), NOW).unwrap();
        let after = store.record_server_failure(&a, ServerFailure::Load, NOW + 60).unwrap();
        assert_eq!((after.failures, after.consecutive_failures), (2, 2));
        assert_eq!(after.last_failure_at, Some(NOW + 60));
        assert_eq!(after.last_failure, Some(ServerFailure::Load));
        assert_eq!(store.server_health(&a, NOW + 60).unwrap(), Some(after));

        let served =
            store.record_server_served(&a, Duration::from_millis(800), NOW + 120).unwrap();
        assert_eq!(served.consecutive_failures, 0, "one success ends the run");
        assert_eq!(served.failures, 2, "the failures still count, for a week");
        assert_eq!((served.successes, served.last_success_at), (1, Some(NOW + 120)));
        assert_eq!(served.last_failure, Some(ServerFailure::Load));
    }

    #[test]
    fn each_server_keeps_its_own_record_under_its_own_provider() {
        let store = store();
        let a = key("https://cdn-a.example");
        store.record_server_failure(&a, ServerFailure::Load, NOW).unwrap();
        assert_eq!(store.server_health(&key("https://cdn-b.example"), NOW).unwrap(), None);
        let other = ServerKey::of("another", "https://cdn-a.example/x").unwrap();
        assert_eq!(store.server_health(&other, NOW).unwrap(), None);
        // Only the origin is ever written, whatever the address it came from.
        let origins: Vec<String> = store
            .with_conn(|c| {
                let mut rows = c.prepare("SELECT origin FROM server_health")?;
                let found =
                    rows.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
                Ok(found)
            })
            .unwrap();
        assert_eq!(origins, ["https://cdn-a.example"]);
    }

    #[test]
    fn the_time_to_a_first_frame_is_smoothed_over_the_plays() {
        let store = store();
        let a = key("https://cdn-a.example");
        let first = store.record_server_served(&a, Duration::from_millis(1_000), NOW).unwrap();
        assert_eq!(first.first_frame, Some(Duration::from_millis(1_000)));
        let second = store.record_server_served(&a, Duration::from_millis(2_000), NOW).unwrap();
        let smoothed = second.first_frame.unwrap().as_secs_f64();
        assert!((smoothed - 1.3).abs() < 1e-6, "{smoothed}");
        // And read back as it was written, to the millisecond.
        let read = store.server_health(&a, NOW).unwrap().unwrap();
        let back = read.first_frame.unwrap().as_secs_f64();
        assert!((back - 1.3).abs() < 1e-6, "{back}");
    }

    #[test]
    fn failures_a_week_old_no_longer_count() {
        let store = store();
        let a = key("https://cdn-a.example");
        store.record_server_failure(&a, ServerFailure::Http(503), NOW).unwrap();
        store.record_server_failure(&a, ServerFailure::Http(503), NOW + HOUR).unwrap();

        let six_days = store.server_health(&a, NOW + 6 * DAY).unwrap().unwrap();
        assert_eq!((six_days.failures, six_days.consecutive_failures), (2, 2));
        let eight_days = store.server_health(&a, NOW + 8 * DAY).unwrap().unwrap();
        assert_eq!((eight_days.failures, eight_days.consecutive_failures), (0, 0));
        assert_eq!(eight_days.last_failure, Some(ServerFailure::Http(503)), "how, still");

        // The next failure starts a new run rather than adding to the old one.
        let again =
            store.record_server_failure(&a, ServerFailure::Load, NOW + 8 * DAY).unwrap();
        assert_eq!((again.failures, again.consecutive_failures), (1, 1));
    }

    #[test]
    fn a_server_not_tried_in_a_month_is_deleted() {
        let store = store();
        let old = key("https://cdn-old.example");
        let recent = key("https://cdn-recent.example");
        store.record_server_failure(&old, ServerFailure::Load, NOW).unwrap();
        store.record_server_served(&recent, Duration::from_secs(1), NOW + 20 * DAY).unwrap();
        assert!(store.server_health(&old, NOW + 29 * DAY).unwrap().is_some());

        // Gone from what is read past its retention, and from the table at the next write.
        assert_eq!(store.server_health(&old, NOW + 31 * DAY).unwrap(), None);
        store.record_server_served(&recent, Duration::from_secs(1), NOW + 31 * DAY).unwrap();
        let rows: i64 = store
            .with_conn(|c| {
                Ok(c.query_row("SELECT COUNT(*) FROM server_health", [], |r| r.get(0))?)
            })
            .unwrap();
        assert_eq!(rows, 1);
        assert!(store.server_health(&recent, NOW + 31 * DAY).unwrap().is_some());
        assert_eq!(store.prune_servers(NOW + 62 * DAY).unwrap(), 1);
    }

    #[test]
    fn a_server_back_after_its_record_was_due_to_go_starts_clean() {
        let store = store();
        let a = key("https://cdn-a.example");
        store.record_server_failure(&a, ServerFailure::Load, NOW).unwrap();
        store.record_server_failure(&a, ServerFailure::Load, NOW).unwrap();
        let back =
            store.record_server_served(&a, Duration::from_secs(1), NOW + 40 * DAY).unwrap();
        assert_eq!((back.successes, back.failures, back.last_failure), (1, 0, None));
    }
}
