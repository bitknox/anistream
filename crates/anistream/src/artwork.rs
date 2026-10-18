//! Fetching and decoding cover art.
//!
//! Runs entirely off the UI thread. Decoding a cover takes long enough that doing it inline
//! would visibly stutter scrolling, and a banner arrives at nearly two megapixels — so both
//! the download and the decode happen in a task, and only a finished image reaches the
//! event loop.
//!
//! Every failure here is silent and local. A dead CDN, a corrupt JPEG, an unwritable cache
//! directory: all of them leave the reserved plate in place. Cover art is decoration, and
//! the title underneath it is not.

use std::path::PathBuf;

use anistream_net::HttpClient;
use anistream_ui::{
    app::Update,
    image::{cache_is_fresh, downscale},
};
use tokio::sync::mpsc;

/// How long a cached cover stays valid. Art changes rarely; a month avoids re-fetching a
/// library's worth of images while still letting a replaced cover through eventually.
const CACHE_TTL_SECS: u64 = 30 * 24 * 3_600;

/// Longest edge kept in memory.
///
/// Even a full-width banner occupies only a few hundred terminal pixels, so holding the
/// original would cost far more than the rendered result can use — multiplied by every
/// cover on screen.
const MAX_EDGE: u32 = 900;

// Compile-time sanity on the two tuning constants above. A source banner arrives around
// 1900px wide, so holding the original would dwarf anything a terminal can display; and a
// cache that never expires would pin a replaced cover forever.
const _: () = {
    assert!(MAX_EDGE < 1900);
    assert!(CACHE_TTL_SECS > 0);
};

/// Fetch, cache and decode one image, then hand it to the UI.
pub fn spawn_fetch(
    url: String,
    cache_dir: PathBuf,
    http: HttpClient,
    tx: mpsc::UnboundedSender<Update>,
) {
    tokio::spawn(async move {
        let path = cache_dir.join(format!("{}.img", anistream_ui::image::stable_hash(&url)));

        let fetched = match read_cached(&path).await {
            Some(bytes) => Ok(bytes),
            None => match download(&http, &url).await {
                Ok(bytes) => {
                    write_cached(&path, &bytes).await;
                    Ok(bytes)
                }
                // An expired copy beats a placeholder while the network is down.
                Err(Failure::Offline) => read_stale(&path).await.ok_or(Failure::Offline),
                Err(failure) => Err(failure),
            },
        };
        let bytes = match fetched {
            Ok(bytes) => bytes,
            // Silent, as the module promises. This was a "cover unavailable" toast per cover,
            // which buried the notices that mattered — "showing what was last cached" among
            // them — under a screenful of the same line. The UI marks the URL failed, which
            // is what lets it be tried again.
            Err(failure) => {
                let _ =
                    tx.send(Update::ImageFailed { url, offline: failure == Failure::Offline });
                return;
            }
        };

        // Decoding is CPU-bound, so it goes to the blocking pool rather than occupying an
        // async worker that other requests need.
        let decoded = tokio::task::spawn_blocking(move || {
            image::load_from_memory(&bytes).ok().map(|img| downscale(img, MAX_EDGE))
        })
        .await
        .ok()
        .flatten();

        match decoded {
            Some(image) => {
                let _ = tx.send(Update::Image { url, image: Box::new(image) });
            }
            None => {
                tracing::debug!(%url, "could not decode artwork");
                let _ = tx.send(Update::ImageFailed { url, offline: false });
            }
        }
    });
}

/// Why a cover could not be downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failure {
    /// The server was not reached: no network, a timeout, a refused connection.
    Offline,
    /// The server answered, and not with the image.
    Refused,
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
    if !response.status().is_success() {
        tracing::debug!(%url, status = response.status().as_u16(), "artwork fetch failed");
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
        let a = anistream_ui::image::stable_hash("https://s4.anilist.co/cover/1.jpg");
        let b = anistream_ui::image::stable_hash("https://s4.anilist.co/cover/1.jpg");
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn a_missing_cache_file_reads_as_absent_rather_than_erroring() {
        assert!(read_cached(&PathBuf::from("/definitely/not/here.img")).await.is_none());
    }

    /// What one fetch sent back to the UI.
    async fn fetch_once(url: String, cache: &std::path::Path) -> Update {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        spawn_fetch(url, cache.to_path_buf(), http, tx);
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
            Update::ImageFailed { offline, .. } => assert!(offline),
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
            Update::ImageFailed { offline, .. } => assert!(!offline),
            other => panic!("expected a quiet failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_expired_copy_is_used_while_the_network_is_down() {
        // Better than a placeholder: the cache only ages while nothing can refresh it.
        let dir = tempfile::tempdir().unwrap();
        let url = "http://127.0.0.1:1/cached.jpg".to_owned();
        let path = dir.path().join(format!("{}.img", anistream_ui::image::stable_hash(&url)));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 6))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(&path, &png).unwrap();
        let old =
            std::time::SystemTime::now() - std::time::Duration::from_secs(CACHE_TTL_SECS + 60);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(old).unwrap();

        match fetch_once(url, dir.path()).await {
            Update::Image { .. } => {}
            other => panic!("expected the stale copy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_unwritable_cache_path_is_survivable() {
        // A read-only or missing cache directory must not take down artwork loading.
        write_cached(&PathBuf::from("/proc/nonexistent/x.img"), b"data").await;
    }
}
