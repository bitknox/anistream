//! Playable stream descriptors.

use serde::{Deserialize, Serialize};

/// How a stream should be handed to a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StreamKind {
    /// HLS manifest. The common case for web sources.
    Hls,
    /// Progressive MP4.
    Mp4,
    /// A localhost URL served by the embedded torrent session, which streams while
    /// downloading and supports range requests. Plays like any other HTTP source, so it
    /// needs no special handling in the player layer.
    TorrentHttp,
    /// Not playable by us at all — a link to open in a licensed player.
    ///
    /// Crunchyroll streams are Widevine + PlayReady protected, so mpv cannot play them.
    /// Rather than pretend otherwise, those episodes resolve to this and open in
    /// Crunchyroll. The UI labels it distinctly so it never looks like a silent failure.
    ExternalDeepLink,
}

impl StreamKind {
    /// Whether our own player can render this, as opposed to handing it off.
    pub const fn is_playable_locally(self) -> bool {
        !matches!(self, Self::ExternalDeepLink)
    }
}

/// A subtitle track offered alongside a stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subtitle {
    /// BCP-47-ish language tag or whatever label the provider gave us.
    pub language: String,
    pub url: String,
    /// Burned into the video rather than a separate track. Can't be turned off, and
    /// can't be restyled, so it is worth preferring a soft track when both exist.
    #[serde(default)]
    pub hard: bool,
}

/// A resolved, playable (or hand-offable) stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stream {
    pub url: String,
    pub kind: StreamKind,
    /// Vertical resolution, when known: `1080`, `720`, …
    pub quality: Option<u32>,
    /// Headers the player must send. Many provider CDNs are referer-locked, so dropping
    /// these turns a working stream into a 403.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub subtitles: Vec<Subtitle>,
    /// Which provider produced this, for UI attribution and health accounting.
    #[serde(default)]
    pub provider_id: String,
    /// What to hand a downloader to fetch this offline, when that is possible at all.
    ///
    /// Separate from `url` because they are genuinely different things: `url` is what a player
    /// consumes *now* — for a torrent, a loopback address that only exists while the session does —
    /// whereas this is the durable reference a download queue can persist and resume from. A source
    /// that can only be streamed leaves it `None`, which is how the queue knows to say
    /// "this cannot be downloaded" rather than failing obscurely.
    #[serde(default)]
    pub download_source: Option<String>,
}

impl Stream {
    pub fn new(url: impl Into<String>, kind: StreamKind) -> Self {
        Self {
            url: url.into(),
            kind,
            quality: None,
            headers: Vec::new(),
            subtitles: Vec::new(),
            provider_id: String::new(),
            download_source: None,
        }
    }

    /// Attach a durable reference a download queue can resume from.
    pub fn with_download_source(mut self, source: impl Into<String>) -> Self {
        self.download_source = Some(source.into());
        self
    }

    pub fn with_quality(mut self, quality: u32) -> Self {
        self.quality = Some(quality);
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Rank against a desired vertical resolution.
    ///
    /// Prefers an exact match, then the closest lower quality, and only then a higher
    /// one — upscaling wastes bandwidth for no visible gain, while going one step down
    /// is usually imperceptible. Unknown quality sorts last.
    pub fn quality_rank(&self, desired: u32) -> u32 {
        match self.quality {
            Some(q) if q == desired => 0,
            Some(q) if q < desired => desired - q,
            Some(q) => (q - desired) + 10_000,
            None => u32::MAX,
        }
    }

    /// Best soft subtitle track for a preferred language, falling back to any soft
    /// track, then to whatever exists.
    pub fn preferred_subtitle(&self, language: &str) -> Option<&Subtitle> {
        let matches_lang = |s: &&Subtitle| s.language.eq_ignore_ascii_case(language);
        self.subtitles
            .iter()
            .find(|s| matches_lang(s) && !s.hard)
            .or_else(|| self.subtitles.iter().find(|s| !s.hard))
            .or_else(|| self.subtitles.iter().find(matches_lang))
            .or_else(|| self.subtitles.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_at(quality: Option<u32>) -> Stream {
        let mut s = Stream::new("https://example.test/v.m3u8", StreamKind::Hls);
        s.quality = quality;
        s
    }

    #[test]
    fn external_deep_links_are_not_locally_playable() {
        assert!(!StreamKind::ExternalDeepLink.is_playable_locally());
        assert!(StreamKind::Hls.is_playable_locally());
        assert!(StreamKind::TorrentHttp.is_playable_locally());
    }

    #[test]
    fn quality_ranking_prefers_exact_then_lower_then_higher() {
        let mut streams = [
            stream_at(Some(2160)),
            stream_at(None),
            stream_at(Some(720)),
            stream_at(Some(1080)),
            stream_at(Some(480)),
        ];
        streams.sort_by_key(|s| s.quality_rank(1080));
        let order: Vec<Option<u32>> = streams.iter().map(|s| s.quality).collect();
        assert_eq!(
            order,
            [Some(1080), Some(720), Some(480), Some(2160), None],
            "exact match first, then step down, and only then upscale"
        );
    }

    #[test]
    fn subtitle_selection_prefers_soft_track_in_requested_language() {
        let mut s = stream_at(Some(1080));
        s.subtitles = vec![
            Subtitle { language: "eng".into(), url: "hard".into(), hard: true },
            Subtitle { language: "spa".into(), url: "soft-es".into(), hard: false },
            Subtitle { language: "eng".into(), url: "soft-en".into(), hard: false },
        ];
        assert_eq!(s.preferred_subtitle("eng").unwrap().url, "soft-en");
        // No soft Japanese track exists, so fall back to any soft track rather than
        // committing the viewer to burned-in subtitles they cannot remove.
        assert!(!s.preferred_subtitle("jpn").unwrap().hard);
    }
}
