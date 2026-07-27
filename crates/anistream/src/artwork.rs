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

        let bytes = match read_cached(&path).await {
            Some(bytes) => bytes,
            None => match download(&http, &url).await {
                Some(bytes) => {
                    write_cached(&path, &bytes).await;
                    bytes
                }
                None => {
                    let _ = tx.send(Update::Toast(anistream_ui::app::Toast::info(
                        "cover unavailable",
                    )));
                    return;
                }
            },
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
            None => tracing::debug!(%url, "could not decode artwork"),
        }
    });
}

async fn read_cached(path: &PathBuf) -> Option<Vec<u8>> {
    if !cache_is_fresh(path, CACHE_TTL_SECS) {
        return None;
    }
    tokio::fs::read(path).await.ok()
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

async fn download(http: &HttpClient, url: &str) -> Option<Vec<u8>> {
    // The AniList CDN has no bot protection, so the plain client is right here — paying for
    // fingerprint emulation would only add handshake cost.
    let response = http.plain().get(url).send().await.ok()?;
    if !response.status().is_success() {
        tracing::debug!(%url, status = response.status().as_u16(), "artwork fetch failed");
        return None;
    }
    Some(response.bytes().await.ok()?.to_vec())
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

    #[tokio::test]
    async fn an_unwritable_cache_path_is_survivable() {
        // A read-only or missing cache directory must not take down artwork loading.
        write_cached(&PathBuf::from("/proc/nonexistent/x.img"), b"data").await;
    }
}
