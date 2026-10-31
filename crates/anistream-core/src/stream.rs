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
    /// A torrent a source named but nothing has started: `url` is a magnet link, as
    /// [`torrent_link`] writes it. Never reaches a player. The torrent bridge in
    /// `anistream-providers` starts it behind the protection check and hands on the
    /// [`Self::TorrentHttp`] stream it serves, or holds it back while torrent playback is off.
    Torrent,
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
        !matches!(self, Self::ExternalDeepLink | Self::Torrent)
    }
}

/// A magnet link for what a source gave as a torrent: a magnet link, or a bare info hash of 40
/// hex or 32 base32 characters. `file` is the file to play, counting from 0, carried as BEP 53's
/// `so=` so it survives wherever the link is kept, such as the download queue.
///
/// `None` for anything else, an HTTP URL to a `.torrent` file included: a source names a torrent,
/// and the session fetches nothing from a URL a source chose.
pub fn torrent_link(input: &str, file: Option<u32>) -> Option<String> {
    let input = input.trim();
    let magnet = if input.get(..8).is_some_and(|scheme| scheme.eq_ignore_ascii_case("magnet:?"))
    {
        let hash = magnet_hash(input)?;
        // Normalised so one torrent is one link, whichever case a source wrote the hash in.
        let rest: Vec<&str> = input[8..]
            .split('&')
            .filter(|part| !part.is_empty())
            .filter(|part| !part.to_ascii_lowercase().starts_with("xt="))
            .filter(|part| !(file.is_some() && part.to_ascii_lowercase().starts_with("so=")))
            .collect();
        let mut link = format!("magnet:?xt=urn:btih:{hash}");
        for part in rest {
            link.push('&');
            link.push_str(part);
        }
        link
    } else if is_info_hash(input) {
        let hash = if input.len() == 40 {
            input.to_ascii_lowercase()
        } else {
            input.to_ascii_uppercase()
        };
        format!("magnet:?xt=urn:btih:{hash}")
    } else {
        return None;
    };
    Some(match file {
        Some(index) => format!("{magnet}&so={index}"),
        None => magnet,
    })
}

/// The file a magnet link selects with BEP 53's `so=`, when it selects exactly one.
pub fn torrent_link_file(link: &str) -> Option<u32> {
    let query = link.trim().get(8..)?;
    query
        .split('&')
        .find_map(|part| part.strip_prefix("so=").or_else(|| part.strip_prefix("SO=")))
        .and_then(|index| index.parse().ok())
}

/// The info hash a magnet link carries, lowercased when it is hex.
fn magnet_hash(link: &str) -> Option<String> {
    link[8..].split('&').find_map(|part| {
        let value = part.get(..12).filter(|key| key.eq_ignore_ascii_case("xt=urn:btih:"))?;
        let hash = &part[value.len()..];
        is_info_hash(hash).then(|| {
            if hash.len() == 40 { hash.to_ascii_lowercase() } else { hash.to_ascii_uppercase() }
        })
    })
}

/// A v1 info hash: 40 hex characters, or 32 base32 ones.
fn is_info_hash(text: &str) -> bool {
    match text.len() {
        40 => text.bytes().all(|b| b.is_ascii_hexdigit()),
        32 => text.bytes().all(|b| b.is_ascii_alphabetic() || (b'2'..=b'7').contains(&b)),
        _ => false,
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
    /// Track format when the URL does not end in a telling extension: `vtt`, `srt`, `ass`.
    ///
    /// The player picks its parser from the file extension, and an API-shaped URL carries
    /// none — without this a track served from `/subtitles?id=…` is guessed at.
    #[serde(default)]
    pub format: Option<String>,
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
    /// One line saying why automatic resolution chose this stream — `curated:` or
    /// `top-ranked:` plus the release name. `None` when there was nothing to choose
    /// between. Surfaced as a toast so the pick is never a mystery.
    #[serde(default)]
    pub pick_note: Option<String>,
    /// Where a seekbar thumbnailer should read this stream instead of `url`, when that matters.
    ///
    /// A thumbnailer is a second player seeking all over the file. Against a web source that
    /// costs a few range requests; against a torrent, every read of `url` is a stream librqbit
    /// fetches ahead for, so the thumbnailer would pull the swarm away from the playback head.
    /// The torrent source sets this to a sibling loopback address that serves only the pieces
    /// already verified and never waits for the rest. `None` means `url` is fine to read.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
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
            pick_note: None,
            thumbnail_url: None,
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
    /// Saturating throughout: `quality` comes from a WASM plugin's return value or a remote
    /// API's JSON, neither of which is validated, so `u32::MAX` is reachable. The addition
    /// used to overflow — a panic in a debug build, and in release a wrap that ranked the
    /// nonsense value *ahead* of a genuine 2160p stream and played it.
    pub fn quality_rank(&self, desired: u32) -> u32 {
        match self.quality {
            Some(q) if q == desired => 0,
            Some(q) if q < desired => desired - q,
            // One below the "unknown" sentinel at most, so a real stream always outranks a
            // stream whose quality nobody stated.
            Some(q) => (q - desired).saturating_add(10_000).min(u32::MAX - 1),
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

    const HASH: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn a_bare_info_hash_becomes_a_magnet_link() {
        let upper = HASH.to_ascii_uppercase();
        assert_eq!(torrent_link(&upper, None), Some(format!("magnet:?xt=urn:btih:{HASH}")));
        let base32 = "abcdefghijklmnopqrstuvwxyz234567";
        assert_eq!(
            torrent_link(base32, None),
            Some(format!("magnet:?xt=urn:btih:{}", base32.to_ascii_uppercase()))
        );
    }

    #[test]
    fn a_magnet_link_keeps_its_own_parts_and_carries_the_file() {
        let upper = HASH.to_ascii_uppercase();
        let link =
            format!("magnet:?dn=Episode%2012&xt=urn:btih:{upper}&tr=udp%3A%2F%2Ft.example");
        let made = torrent_link(&link, Some(3)).unwrap();
        assert_eq!(
            made,
            format!("magnet:?xt=urn:btih:{HASH}&dn=Episode%2012&tr=udp%3A%2F%2Ft.example&so=3")
        );
        assert_eq!(torrent_link_file(&made), Some(3));
        // A file given replaces the link's own choice; none given keeps it.
        let chosen = format!("magnet:?xt=urn:btih:{HASH}&so=1");
        assert_eq!(torrent_link_file(&torrent_link(&chosen, Some(4)).unwrap()), Some(4));
        assert_eq!(torrent_link_file(&torrent_link(&chosen, None).unwrap()), Some(1));
    }

    #[test]
    fn anything_but_a_magnet_or_a_hash_is_refused() {
        for input in [
            "https://index.example/file.torrent",
            "magnet:?dn=no-hash",
            "magnet:?xt=urn:btih:short",
            "0123",
            "",
        ] {
            assert_eq!(torrent_link(input, None), None, "{input}");
        }
        assert_eq!(torrent_link_file(&format!("magnet:?xt=urn:btih:{HASH}")), None);
    }

    #[test]
    fn a_torrent_nothing_has_started_is_not_for_a_player() {
        assert!(!StreamKind::Torrent.is_playable_locally());
        assert!(StreamKind::TorrentHttp.is_playable_locally());
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
    fn an_absurd_quality_ranks_last_rather_than_overflowing() {
        // `quality` is whatever a WASM plugin returned or a remote API's JSON said, neither
        // of which is validated. The upscale arithmetic overflowed on it — a panic in debug,
        // and in release a wrap that ranked the nonsense value ahead of a real 2160p stream
        // and played it.
        let mut streams =
            [stream_at(Some(u32::MAX)), stream_at(Some(2160)), stream_at(Some(1080))];
        streams.sort_by_key(|s| s.quality_rank(1080));

        let order: Vec<Option<u32>> = streams.iter().map(|s| s.quality).collect();
        assert_eq!(order, [Some(1080), Some(2160), Some(u32::MAX)]);
        assert!(
            stream_at(Some(u32::MAX)).quality_rank(1080) < u32::MAX,
            "a stated quality, however silly, still beats one nobody stated"
        );
    }

    #[test]
    fn subtitle_selection_prefers_soft_track_in_requested_language() {
        let mut s = stream_at(Some(1080));
        s.subtitles = vec![
            Subtitle { language: "eng".into(), url: "hard".into(), hard: true, format: None },
            Subtitle {
                language: "spa".into(),
                url: "soft-es".into(),
                hard: false,
                format: None,
            },
            Subtitle {
                language: "eng".into(),
                url: "soft-en".into(),
                hard: false,
                format: None,
            },
        ];
        assert_eq!(s.preferred_subtitle("eng").unwrap().url, "soft-en");
        // No soft Japanese track exists, so fall back to any soft track rather than
        // committing the viewer to burned-in subtitles they cannot remove.
        assert!(!s.preferred_subtitle("jpn").unwrap().hard);
    }
}
