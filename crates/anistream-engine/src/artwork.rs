//! Fetching cover art.
//!
//! Runs entirely off the UI thread, and stops at the bytes. What a cover becomes on screen is
//! the front end's to decide — a terminal decodes and shrinks it, another front end may hand
//! the bytes to its own image view — so the fetch sends them as they came, and decoding is
//! the front end's work, done off its own loop.
//!
//! A grid asks for a screenful of covers at once, so only [`DOWNLOADS_AT_ONCE`] download at a
//! time, the ones asked for last first: those are the covers on screen now. A cover on disk
//! already never waits for them. A server that is busy for a moment (429, or a 5xx) is asked
//! again a little later, a couple of times, before the cover is reported failed.
//!
//! Every failure here is silent and local. A dead CDN, an unwritable cache directory: both
//! leave the reserved plate in place, or in the second case the cover itself, since a cache
//! that cannot be written is no reason not to show what was just downloaded. Cover art is
//! decoration, and the title underneath it is not.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use anistream_model::event::Event;
use anistream_net::HttpClient;
use tokio::sync::oneshot;

use crate::outbox::Outbox;

/// How long a cached cover stays valid. Art changes rarely; a month avoids re-fetching a
/// library's worth of images while still letting a replaced cover through eventually.
pub const CACHE_TTL_SECS: u64 = 30 * 24 * 3_600;

// A cache that never expires would pin a replaced cover forever.
const _: () = assert!(CACHE_TTL_SECS > 0);

/// How many covers download at once. A grid of a hundred covers asked for together would
/// otherwise open a hundred connections to one CDN, which answers the last of them slowly or
/// not at all.
pub const DOWNLOADS_AT_ONCE: usize = 6;

/// How long a cover the server was too busy for waits before each of its tries again.
const RETRY_DELAYS: [Duration; 2] = [Duration::from_millis(500), Duration::from_millis(1_500)];

/// The longest a server's own `Retry-After` is waited, when it asks for longer than the delay.
const LONGEST_RETRY_AFTER: Duration = Duration::from_secs(5);

/// The covers downloading, and those waiting to: one per engine, shared by every fetch.
#[derive(Clone)]
pub struct Downloads {
    slots: Arc<Slots>,
    retry_delays: Arc<[Duration]>,
}

impl Default for Downloads {
    fn default() -> Self {
        Self::new(DOWNLOADS_AT_ONCE, &RETRY_DELAYS)
    }
}

impl Downloads {
    /// `at_once` downloads at a time, and a busy server asked again after each of
    /// `retry_delays`.
    pub fn new(at_once: usize, retry_delays: &[Duration]) -> Self {
        Self { slots: Slots::new(at_once.max(1)), retry_delays: retry_delays.into() }
    }

    /// `url`'s bytes from the server, in a slot of its own, asked again while the server is
    /// busy and there are tries left.
    async fn download(&self, http: &HttpClient, url: &str) -> Result<Vec<u8>, Failure> {
        let mut delays = self.retry_delays.iter();
        loop {
            let tried = {
                let _slot = self.slots.take().await;
                download(http, url).await
            };
            let Err(Failure::Busy { retry_after }) = tried else { return tried };
            let Some(&delay) = delays.next() else { return tried };
            // The slot is given back while it waits, for a cover that may get through.
            let wait =
                retry_after.map_or(delay, |after| after.clamp(delay, LONGEST_RETRY_AFTER));
            tokio::time::sleep(wait).await;
        }
    }
}

/// Fetch one image, from the cache when it is fresh there, and hand its bytes to the UI.
pub fn spawn_fetch(
    url: String,
    cache_dir: PathBuf,
    http: HttpClient,
    downloads: Downloads,
    tx: Outbox,
) {
    tokio::spawn(async move {
        let path = cache_dir.join(format!("{}.img", anistream_core::hash::stable_hash(&url)));

        let fetched = match read_cached(&path).await {
            Some(bytes) => Ok(bytes),
            None => match downloads.download(&http, &url).await {
                Ok(bytes) => {
                    write_cached(&path, &bytes).await;
                    Ok(bytes)
                }
                // An expired copy beats a placeholder while the network is down.
                Err(Failure::Offline) => read_stale(&path).await.ok_or(Failure::Offline),
                Err(failure) => Err(failure),
            },
        };
        let _ = tx.send(match fetched {
            Ok(bytes) => Event::ArtworkReady { url, bytes },
            // Silent, as the module promises. This was a "cover unavailable" toast per cover,
            // which buried the notices that mattered — "showing what was last cached" among
            // them — under a screenful of the same line. The UI marks the URL failed, which
            // is what lets it be tried again.
            Err(failure) => Event::ImageFailed { url, offline: failure == Failure::Offline },
        });
    });
}

/// A fixed number of slots, handed to the newest waiter first.
struct Slots {
    state: Mutex<SlotState>,
}

struct SlotState {
    free: usize,
    /// Those waiting, oldest first.
    waiting: Vec<oneshot::Sender<()>>,
}

impl Slots {
    fn new(count: usize) -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(SlotState { free: count, waiting: Vec::new() }) })
    }

    fn lock(&self) -> MutexGuard<'_, SlotState> {
        // Nothing panics while holding the lock; a poisoned one is still consistent.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A slot, once one is free and nobody who asked later is waiting for it.
    async fn take(self: &Arc<Self>) -> Slot {
        let waiting = {
            let mut state = self.lock();
            if state.free > 0 {
                state.free -= 1;
                None
            } else {
                let (handed, wait) = oneshot::channel();
                state.waiting.push(handed);
                Some(wait)
            }
        };
        if let Some(wait) = waiting {
            let mut waiter = Waiter { wait: Some(wait), slots: Arc::clone(self) };
            if let Some(wait) = waiter.wait.as_mut() {
                // Only a slot handed over ends this: the sender is kept until it is sent.
                let _ = wait.await;
            }
            waiter.wait = None;
        }
        Slot(Arc::clone(self))
    }

    /// A slot given back: to the newest waiter still waiting, else free.
    fn give_back(&self) {
        let mut state = self.lock();
        while let Some(next) = state.waiting.pop() {
            if next.send(()).is_ok() {
                return;
            }
        }
        state.free += 1;
    }

    #[cfg(test)]
    fn waiting(&self) -> usize {
        self.lock().waiting.len()
    }
}

/// A slot taken, given back when dropped.
struct Slot(Arc<Slots>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.give_back();
    }
}

/// A wait for a slot, which gives back a slot handed to it after it stopped waiting.
struct Waiter {
    wait: Option<oneshot::Receiver<()>>,
    slots: Arc<Slots>,
}

impl Drop for Waiter {
    fn drop(&mut self) {
        if let Some(mut wait) = self.wait.take() {
            wait.close();
            if wait.try_recv().is_ok() {
                self.slots.give_back();
            }
        }
    }
}

/// Why a cover could not be downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// The server was not reached: no network, a timeout, a refused connection.
    Offline,
    /// The server answered, and not with the image.
    Refused,
    /// The server is too busy to answer now (429, or a 5xx), and may later: after
    /// `retry_after`, when it said.
    Busy { retry_after: Option<Duration> },
}

/// Whether a cached file is still usable.
pub fn cache_is_fresh(path: &Path, max_age_secs: u64) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if meta.len() == 0 {
        return false;
    }
    meta.modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age.as_secs() < max_age_secs)
}

async fn read_cached(path: &PathBuf) -> Option<Vec<u8>> {
    if !cache_is_fresh(path, CACHE_TTL_SECS) {
        return None;
    }
    tokio::fs::read(path).await.ok()
}

/// A cached copy however old, for when the network cannot provide a fresher one.
async fn read_stale(path: &PathBuf) -> Option<Vec<u8>> {
    tokio::fs::read(path).await.ok().filter(|bytes| !bytes.is_empty())
}

async fn write_cached(path: &PathBuf, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    // A failed cache write is not worth reporting: the image already downloaded fine and
    // will simply be fetched again next time.
    if let Err(e) = tokio::fs::write(path, bytes).await {
        tracing::debug!(error = %e, "could not cache artwork");
    }
}

async fn download(http: &HttpClient, url: &str) -> Result<Vec<u8>, Failure> {
    // The AniList CDN has no bot protection, so the plain client is right here — paying for
    // fingerprint emulation would only add handshake cost.
    let response = http.plain().get(url).send().await.map_err(|e| {
        tracing::debug!(%url, error = %e, "artwork fetch failed");
        Failure::Offline
    })?;
    let status = response.status();
    if !status.is_success() {
        tracing::debug!(%url, status = status.as_u16(), "artwork fetch failed");
        if status.as_u16() == 429 || status.is_server_error() {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse().ok())
                .map(Duration::from_secs);
            return Err(Failure::Busy { retry_after });
        }
        return Err(Failure::Refused);
    }
    // A body cut off mid-transfer is the network going, not the server refusing.
    Ok(response.bytes().await.map_err(|_| Failure::Offline)?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_paths_are_stable_across_runs() {
        // If these drifted, the disk cache would never hit.
        let a = anistream_core::hash::stable_hash("https://s4.anilist.co/cover/1.jpg");
        let b = anistream_core::hash::stable_hash("https://s4.anilist.co/cover/1.jpg");
        assert_eq!(a, b);
    }

    #[test]
    fn a_missing_or_empty_cache_file_is_not_fresh() {
        assert!(!cache_is_fresh(Path::new("/definitely/not/here.img"), 3600));

        let dir = std::env::temp_dir().join("anistream-cache-test");
        std::fs::create_dir_all(&dir).ok();
        let empty = dir.join("empty.img");
        std::fs::write(&empty, b"").ok();
        assert!(!cache_is_fresh(&empty, 3600), "a zero-byte file is a failed download");
        std::fs::remove_file(&empty).ok();
    }

    #[tokio::test]
    async fn a_missing_cache_file_reads_as_absent_rather_than_erroring() {
        assert!(read_cached(&PathBuf::from("/definitely/not/here.img")).await.is_none());
    }

    /// `at_once` downloads at a time, retried at once, so a test does not wait out the delays.
    fn quick(at_once: usize) -> Downloads {
        Downloads::new(at_once, &[Duration::from_millis(10), Duration::from_millis(10)])
    }

    /// What one fetch sent back to the UI.
    async fn fetch_once(url: String, cache: &std::path::Path) -> Event {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        spawn_fetch(url, cache.to_path_buf(), http, quick(DOWNLOADS_AT_ONCE), tx.into());
        tokio::time::timeout(std::time::Duration::from_secs(20), rx.recv())
            .await
            .expect("the fetch never reported back")
            .expect("the fetch dropped its sender without a word")
    }

    #[tokio::test]
    async fn an_unreachable_cover_is_reported_quietly_as_offline() {
        // One "cover unavailable" toast per cover used to fill the stack and the Logs. Port 1
        // on loopback refuses the connection, which is the network failing, not the server.
        let dir = tempfile::tempdir().unwrap();
        match fetch_once("http://127.0.0.1:1/cover.jpg".into(), dir.path()).await {
            Event::ImageFailed { offline, .. } => assert!(offline),
            other => panic!("expected a quiet failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_cover_the_server_refuses_is_not_mistaken_for_the_network_being_down() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://{}/gone.jpg", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                )
                .await;
        });
        let dir = tempfile::tempdir().unwrap();
        match fetch_once(url, dir.path()).await {
            Event::ImageFailed { offline, .. } => assert!(!offline),
            other => panic!("expected a quiet failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_expired_copy_is_used_while_the_network_is_down() {
        // Better than a placeholder: the cache only ages while nothing can refresh it.
        let dir = tempfile::tempdir().unwrap();
        let url = "http://127.0.0.1:1/cached.jpg".to_owned();
        let path = dir.path().join(format!("{}.img", anistream_core::hash::stable_hash(&url)));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 6))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&path, &png).unwrap();
        let old =
            std::time::SystemTime::now() - std::time::Duration::from_secs(CACHE_TTL_SECS + 60);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();

        match fetch_once(url, dir.path()).await {
            Event::ArtworkReady { bytes, .. } => assert_eq!(bytes, png),
            other => panic!("expected the stale copy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_cover_that_cannot_be_cached_is_shown_all_the_same() {
        // The bytes were downloaded fine; a cache directory that cannot be made — here, one
        // under a plain file — only means fetching them again next time.
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let url = format!("http://{}/cover.jpg", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 2048];
            let _ = socket.read(&mut request).await;
            let _ = socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\ncover",
                )
                .await;
        });
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-directory");
        std::fs::write(&file, b"").unwrap();

        match fetch_once(url, &file.join("covers")).await {
            Event::ArtworkReady { bytes, .. } => assert_eq!(bytes, b"cover"),
            other => panic!("expected the cover, got {other:?}"),
        }
    }

    /// A server on loopback that answers each connection with the next of `replies`, the last
    /// one over and over, holding each answer back by `hold`. Counts the connections, and the
    /// most it had open at once.
    async fn scripted(
        replies: Vec<&'static [u8]>,
        hold: Duration,
    ) -> (String, std::sync::Arc<Counts>) {
        use std::sync::atomic::Ordering::SeqCst;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let counts = std::sync::Arc::new(Counts::default());
        let served = std::sync::Arc::clone(&counts);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else { return };
                let n = served.total.fetch_add(1, SeqCst);
                let reply = replies[n.min(replies.len() - 1)];
                let served = std::sync::Arc::clone(&served);
                tokio::spawn(async move {
                    let open = served.open.fetch_add(1, SeqCst) + 1;
                    served.most.fetch_max(open, SeqCst);
                    let mut request = [0_u8; 2048];
                    let _ = socket.read(&mut request).await;
                    tokio::time::sleep(hold).await;
                    served.open.fetch_sub(1, SeqCst);
                    let _ = socket.write_all(reply).await;
                });
            }
        });
        (base, counts)
    }

    #[derive(Default)]
    struct Counts {
        total: std::sync::atomic::AtomicUsize,
        open: std::sync::atomic::AtomicUsize,
        most: std::sync::atomic::AtomicUsize,
    }

    const OK: &[u8] = b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\ncover";
    const UNAVAILABLE: &[u8] =
        b"HTTP/1.1 503 Service Unavailable\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

    const TOO_MANY: &[u8] =
        b"HTTP/1.1 429 Too Many Requests\r\nretry-after: 0\r\ncontent-length: 0\r\n\
        connection: close\r\n\r\n";
    const NOT_FOUND: &[u8] =
        b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n";

    /// The next event, within a while.
    async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>) -> Event {
        tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .expect("the fetch never reported back")
            .expect("the fetch dropped its sender without a word")
    }

    #[tokio::test]
    async fn a_screenful_of_covers_downloads_a_few_at_a_time() {
        use std::sync::atomic::Ordering::SeqCst;
        // A grid's worth of covers asked for at once once opened two dozen connections.
        let (base, counts) = scripted(vec![OK], Duration::from_millis(100)).await;
        let dir = tempfile::tempdir().unwrap();
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let tx: Outbox = tx.into();
        let downloads = Downloads::default();
        for n in 0..24 {
            let url = format!("{base}/cover/{n}.jpg");
            spawn_fetch(
                url,
                dir.path().to_path_buf(),
                http.clone(),
                downloads.clone(),
                tx.clone(),
            );
        }
        for _ in 0..24 {
            let event = next(&mut rx).await;
            assert!(matches!(event, Event::ArtworkReady { .. }), "got {event:?}");
        }
        assert_eq!(counts.total.load(SeqCst), 24);
        assert!(
            counts.most.load(SeqCst) <= DOWNLOADS_AT_ONCE,
            "{} at once",
            counts.most.load(SeqCst)
        );
    }

    #[tokio::test]
    async fn a_cover_on_disk_does_not_wait_for_the_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let url = "http://127.0.0.1:1/cached.jpg".to_owned();
        let path = dir.path().join(format!("{}.img", anistream_core::hash::stable_hash(&url)));
        std::fs::write(&path, b"cover").unwrap();
        let downloads = quick(1);
        // Every slot taken, by a download that never ends.
        let _held = downloads.slots.take().await;
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        spawn_fetch(url, dir.path().to_path_buf(), http, downloads.clone(), tx.into());
        match next(&mut rx).await {
            Event::ArtworkReady { bytes, .. } => assert_eq!(bytes, b"cover"),
            other => panic!("expected the cached cover, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_covers_asked_for_last_download_first() {
        // Those are the ones on screen now; the first were scrolled past.
        let slots = Slots::new(1);
        let held = slots.take().await;
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        for n in 0..3 {
            let tx = tx.clone();
            tokio::spawn({
                let slots = Arc::clone(&slots);
                async move {
                    let _slot = slots.take().await;
                    tx.send(n).unwrap();
                }
            });
            while slots.waiting() < n + 1 {
                tokio::task::yield_now().await;
            }
        }
        drop(held);
        let mut order = Vec::new();
        for _ in 0..3 {
            order.push(
                tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.unwrap().unwrap(),
            );
        }
        assert_eq!(order, [2, 1, 0]);
    }

    #[tokio::test]
    async fn a_wait_given_up_hands_its_slot_on() {
        let slots = Slots::new(1);
        let held = slots.take().await;
        let first = tokio::spawn({
            let slots = Arc::clone(&slots);
            async move { drop(slots.take().await) }
        });
        while slots.waiting() < 1 {
            tokio::task::yield_now().await;
        }
        let given_up = tokio::spawn({
            let slots = Arc::clone(&slots);
            async move { drop(slots.take().await) }
        });
        while slots.waiting() < 2 {
            tokio::task::yield_now().await;
        }
        given_up.abort();
        let _ = given_up.await;
        drop(held);
        tokio::time::timeout(Duration::from_secs(5), first).await.unwrap().unwrap();
        // And every slot is free again.
        drop(tokio::time::timeout(Duration::from_secs(5), slots.take()).await.unwrap());
    }

    #[tokio::test]
    async fn a_server_busy_for_a_moment_is_asked_again_before_the_cover_fails() {
        use std::sync::atomic::Ordering::SeqCst;
        let (base, counts) = scripted(vec![UNAVAILABLE, TOO_MANY, OK], Duration::ZERO).await;
        let dir = tempfile::tempdir().unwrap();
        match fetch_once(format!("{base}/cover.jpg"), dir.path()).await {
            Event::ArtworkReady { bytes, .. } => assert_eq!(bytes, b"cover"),
            other => panic!("expected the cover, got {other:?}"),
        }
        assert_eq!(counts.total.load(SeqCst), 3);
    }

    #[tokio::test]
    async fn a_server_busy_for_longer_fails_the_cover_quietly() {
        use std::sync::atomic::Ordering::SeqCst;
        let (base, counts) = scripted(vec![UNAVAILABLE], Duration::ZERO).await;
        let dir = tempfile::tempdir().unwrap();
        match fetch_once(format!("{base}/cover.jpg"), dir.path()).await {
            Event::ImageFailed { offline, .. } => assert!(!offline),
            other => panic!("expected a quiet failure, got {other:?}"),
        }
        assert_eq!(counts.total.load(SeqCst), 1 + RETRY_DELAYS.len());
    }

    #[tokio::test]
    async fn a_cover_that_is_not_there_is_not_asked_again() {
        use std::sync::atomic::Ordering::SeqCst;
        let (base, counts) = scripted(vec![NOT_FOUND], Duration::ZERO).await;
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            fetch_once(format!("{base}/gone.jpg"), dir.path()).await,
            Event::ImageFailed { offline: false, .. }
        ));
        assert_eq!(counts.total.load(SeqCst), 1);
    }

    #[tokio::test]
    async fn an_unwritable_cache_path_is_survivable() {
        // A read-only or missing cache directory must not take down artwork loading.
        write_cached(&PathBuf::from("/proc/nonexistent/x.img"), b"data").await;
    }
}
