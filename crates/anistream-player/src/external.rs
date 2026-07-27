//! Handing a stream to whatever is licensed to play it.
//!
//! This exists because of a hard limit, not a design preference: every Crunchyroll stream is
//! Widevine *and* PlayReady protected over DASH, so mpv cannot decrypt them and neither can
//! we. Extracting a CDM would be DRM circumvention, so the honest option is to open the
//! episode where it is licensed to play.
//!
//! The pluggable [`Player`] trait is what makes this cost nothing: the licensed path needed no
//! new machinery anywhere else, just another implementation.

use anistream_core::{
    Error,
    stream::{Stream, StreamKind},
    traits::{PlaybackRequest, Player},
};
use async_trait::async_trait;

/// Opens a stream in the system's default handler.
#[derive(Debug, Clone, Default)]
pub struct ExternalPlayer;

impl ExternalPlayer {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Player for ExternalPlayer {
    fn id(&self) -> &str {
        "external"
    }

    /// Only deep links. Anything we can actually decode should go to mpv, where progress can
    /// be tracked and skips applied — handing a playable stream to a browser would silently
    /// lose all of that.
    fn supports(&self, stream: &Stream) -> bool {
        stream.kind == StreamKind::ExternalDeepLink
    }

    async fn play(&self, stream: &Stream, _request: PlaybackRequest) -> Result<(), Error> {
        tracing::info!(url = %stream.url, "handing off to the system handler");
        open::that_detached(&stream.url)
            .map_err(|e| Error::Player(format!("could not open {}: {e}", stream.url)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(kind: StreamKind) -> Stream {
        Stream::new("https://www.crunchyroll.com/watch/G2XU04E88", kind)
    }

    #[test]
    fn only_deep_links_are_handled() {
        let player = ExternalPlayer::new();
        assert!(player.supports(&stream(StreamKind::ExternalDeepLink)));
    }

    #[test]
    fn a_playable_stream_is_refused_so_it_reaches_mpv() {
        // Opening an HLS URL in a browser would work but silently lose progress tracking,
        // resume and skips.
        let player = ExternalPlayer::new();
        assert!(!player.supports(&stream(StreamKind::Hls)));
        assert!(!player.supports(&stream(StreamKind::Mp4)));
        assert!(!player.supports(&stream(StreamKind::TorrentHttp)));
    }

    #[test]
    fn the_id_is_stable_because_config_references_it() {
        assert_eq!(ExternalPlayer::new().id(), "external");
    }
}
