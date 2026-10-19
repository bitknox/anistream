//! Covers decoded for the terminal.
//!
//! The artwork fetch stops at the bytes, since what a cover becomes on screen is the front
//! end's to decide. Here it becomes an image a terminal can hold. Decoding a cover takes long
//! enough that doing it on the event loop would visibly stutter scrolling, and a banner arrives
//! at nearly two megapixels — so the loop hands the bytes to the blocking pool, and only the
//! finished image comes back, on a channel of its own, to be given to the app.
//!
//! A cover that does not decode is a failed cover like any other: silent, its plate kept, and
//! tried again after the backoff.

use anistream_ui::{App, image::downscale};
use tokio::sync::mpsc;

/// Longest edge kept in memory.
///
/// Even a full-width banner occupies only a few hundred terminal pixels, so holding the
/// original would cost far more than the rendered result can use — multiplied by every
/// cover on screen.
const MAX_EDGE: u32 = 900;

// A source banner arrives around 1900px wide, so holding the original would dwarf anything a
// terminal can display.
const _: () = assert!(MAX_EDGE < 1900);

/// One cover's bytes, as an image or as the news that they were not one.
pub struct Decoded {
    url: String,
    image: Option<image::DynamicImage>,
}

/// Decode on the blocking pool, and send what came of it to `tx` for [`hand_over`].
pub fn spawn(url: String, bytes: Vec<u8>, tx: &mpsc::UnboundedSender<Decoded>) {
    let tx = tx.clone();
    // CPU-bound, so on the blocking pool rather than occupying an async worker that other
    // requests need.
    tokio::task::spawn_blocking(move || {
        let _ = tx.send(now(url, &bytes));
    });
}

/// Decode on this thread, for what settles the app rather than runs it: `--preview` and the
/// contract tests' harness, which have no frame to keep moving.
pub fn now(url: String, bytes: &[u8]) -> Decoded {
    let image = image::load_from_memory(bytes).ok().map(|img| downscale(img, MAX_EDGE));
    if image.is_none() {
        tracing::debug!(%url, "could not decode artwork");
    }
    Decoded { url, image }
}

/// Give the app what came of one cover.
pub fn hand_over(app: &mut App, decoded: Decoded) {
    app.artwork_decoded(&decoded.url, decoded.image);
}

#[cfg(test)]
mod tests {
    use anistream_core::config::Config;
    use anistream_model::event::Event;
    use anistream_ui::{
        Keymap,
        image::{ImageEngine, RequestState},
        theme,
    };

    use super::*;

    /// An app that draws covers, in halfblocks so no terminal has to be asked.
    fn app() -> App {
        let palette = theme::resolve_with(Default::default(), None);
        App::with_images(Config::default(), palette, Keymap::new(), ImageEngine::halfblocks())
    }

    fn png() -> Vec<u8> {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 6))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png
    }

    #[tokio::test]
    async fn a_cover_s_bytes_reach_the_app_as_an_image() {
        let url = "https://covers.example/1.png".to_owned();
        let mut app = app();
        assert!(app.images.should_fetch(&url), "claimed, as the loop claims what it fetches");
        app.apply(Event::ArtworkReady { url: url.clone(), bytes: png() });
        assert!(!app.images.has(&url), "the reducer does not decode on the loop");

        let (tx, mut rx) = mpsc::unbounded_channel();
        spawn(url.clone(), png(), &tx);
        hand_over(&mut app, rx.recv().await.expect("the decode reported back"));
        assert!(app.images.has(&url));
        assert_eq!(app.images.requests().state(&url), Some(RequestState::Ready));
    }

    #[test]
    fn bytes_that_are_not_an_image_fail_as_a_refused_cover_does() {
        let url = "https://covers.example/2.png".to_owned();
        let mut app = app();
        assert!(app.images.should_fetch(&url));
        hand_over(&mut app, now(url.clone(), b"<html>not a cover</html>"));
        assert!(!app.images.has(&url));
        assert_eq!(app.images.requests().state(&url), Some(RequestState::Failed));
        // Not mistaken for the network being down: a returning network does not retry it.
        app.images.requests().network_returned();
        assert!(!app.images.should_fetch(&url));
    }
}
