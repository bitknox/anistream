//! Playback, as the work reports it: which playback it is, what a skip would seek past, how a
//! playback came to an end, and what a newer one is about to play. Also what the player's Up Next
//! card and Episodes menu show, the theme a front end dresses the player in, and a stream as a
//! front end's own player is handed it.

use anistream_core::{
    ids::AnilistId,
    stream::{Stream, StreamKind, Subtitle},
};

/// One playback, numbered by the engine as it starts it, once each per run: two playbacks of
/// one episode are two numbers. What a playback says is named with it, so whose it is never has
/// to be guessed from what was started last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaybackId(u64);

// The number itself on the far side of an FFI, under the type's name, as a job's is.
#[cfg(feature = "ffi")]
uniffi::custom_newtype!(PlaybackId, u64);

impl PlaybackId {
    /// The playback numbered `n`. The engine numbers them as it starts them.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// The playback's number, for a front end that keys playbacks by a plain integer.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// What a skip would seek past.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SkipKind {
    Opening,
    Ending,
}

/// How a playback came to an end.
///
/// More than `watched` can say: an episode closed past the commit threshold counts as watched,
/// and rolling on from it would open a new window on someone who just closed one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Ending {
    /// mpv played the file to its end. `next` is the episode the player kept its window open
    /// for — set only when the episode rolls on, which the playback decides: auto-next on, Up
    /// Next not cancelled, and an episode list that has a next episode. Auto-next follows this
    /// and nothing else, so a window is never kept for an episode that is not coming, and no
    /// episode is searched for that the list says does not exist.
    Complete { next: Option<String> },
    /// The viewer stopped it from the terminal.
    Stopped,
    /// The player window went away: the viewer closed it, or mpv exited on its own.
    Closed,
    /// It could not play: no stream was found, mpv would not start, or the stream failed.
    Failed,
    /// Something else took the stream: a watch party, or the browser for a licensed stream.
    Elsewhere,
}

/// What a newer playback is about to play, as the window it takes over announces it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Successor {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    pub translation: anistream_core::media::Translation,
}

/// The episode that follows, as the Up Next card names it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NextEpisode {
    pub number: String,
    pub title: Option<String>,
    /// The episode's still, by URL, for the Up Next card.
    pub thumbnail: Option<String>,
}

impl NextEpisode {
    pub fn label(&self) -> String {
        format!("Episode {}", self.number)
    }
}

/// One row of the player's Episodes menu.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeEntry {
    pub number: String,
    pub title: Option<String>,
    pub watched: bool,
    pub filler: bool,
}

/// The colours and font the player's own interface is drawn in.
///
/// Handed in by the front end, since the colours are its palette's and a palette is its own
/// business. Each colour is `#RRGGBB`, as the companion script reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PlayerTheme {
    pub text: String,
    pub dim: String,
    pub obi: String,
    pub state: String,
    pub alert: String,
    /// What the player's surfaces are painted on.
    pub ground: String,
    pub font: String,
}

impl PlayerTheme {
    /// The companion's options, as `(key, value)` pairs without their prefix.
    pub fn options(&self) -> Vec<(String, String)> {
        [
            ("text", &self.text),
            ("dim", &self.dim),
            ("obi", &self.obi),
            ("state", &self.state),
            ("alert", &self.alert),
            ("ground", &self.ground),
            ("font", &self.font),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.clone()))
        .collect()
    }
}

/// A stream, as a front end's own player is handed it to play: the core's [`Stream`] with what
/// only the engine uses left out, and in shapes that cross an FFI.
///
/// Built [`From`] a stream. Its download source stays with the download queue, and the note on
/// why it was picked is said as a notice of its own.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PlayableStream {
    pub url: String,
    pub kind: PlayableKind,
    /// Vertical resolution, when known: `1080`, `720`, …
    pub quality: Option<u32>,
    /// The headers the player must send with every request for the stream. Many CDNs are
    /// referer-locked, so a player that drops these gets a 403.
    pub headers: Vec<HttpHeader>,
    /// The subtitle tracks beside the stream, in the order the source listed them.
    pub subtitles: Vec<SubtitleTrack>,
    /// The source that found it.
    pub provider_id: String,
    /// How the player's source menu describes it on its own: its quality and kind, as
    /// `1080p · HLS`. Where two of a source's streams would read the same, the menu tells them
    /// apart by their servers.
    pub label: String,
    /// Where a seekbar thumbnailer reads the stream instead of `url`, when that matters: a
    /// torrent's, so the thumbnailer does not pull the swarm away from the playback head.
    pub thumbnail_url: Option<String>,
}

impl From<&Stream> for PlayableStream {
    fn from(stream: &Stream) -> Self {
        // Every field named, so one added to the core's stream is a choice made here.
        let Stream {
            url,
            kind,
            quality,
            headers,
            subtitles,
            provider_id,
            download_source: _,
            pick_note: _,
            thumbnail_url,
        } = stream;
        Self {
            url: url.clone(),
            kind: (*kind).into(),
            quality: *quality,
            headers: headers
                .iter()
                .map(|(name, value)| HttpHeader { name: name.clone(), value: value.clone() })
                .collect(),
            subtitles: subtitles.iter().map(SubtitleTrack::from).collect(),
            provider_id: provider_id.clone(),
            label: stream_label(stream),
            thumbnail_url: thumbnail_url.clone(),
        }
    }
}

/// How a stream is played: the core's [`StreamKind`], as it crosses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayableKind {
    /// An HLS manifest.
    Hls,
    /// A progressive MP4.
    Mp4,
    /// A loopback URL the engine's torrent session serves while it downloads.
    TorrentHttp,
    /// Not for the player at all: a link to open in a licensed app, through the host.
    ExternalDeepLink,
}

impl From<StreamKind> for PlayableKind {
    fn from(kind: StreamKind) -> Self {
        match kind {
            StreamKind::Hls => Self::Hls,
            StreamKind::Mp4 => Self::Mp4,
            StreamKind::TorrentHttp => Self::TorrentHttp,
            StreamKind::ExternalDeepLink => Self::ExternalDeepLink,
        }
    }
}

/// One HTTP header a player sends with a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

/// A subtitle track offered beside a stream: the core's [`Subtitle`], as it crosses.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SubtitleTrack {
    /// The language as the source gave it: a tag such as `en`, or a name.
    pub language: String,
    pub url: String,
    /// Burned into the video, so it cannot be turned off and is no file to load.
    pub hard: bool,
    /// The track's format when its URL does not say: `vtt`, `srt`, `ass`.
    pub format: Option<String>,
}

impl From<&Subtitle> for SubtitleTrack {
    fn from(subtitle: &Subtitle) -> Self {
        let Subtitle { language, url, hard, format } = subtitle;
        Self {
            language: language.clone(),
            url: url.clone(),
            hard: *hard,
            format: format.clone(),
        }
    }
}

/// How the player's source menu describes a stream on its own: its quality, when known, and
/// its kind, as `1080p · HLS`.
pub fn stream_label(stream: &Stream) -> String {
    let kind = match stream.kind {
        StreamKind::Hls => "HLS",
        StreamKind::TorrentHttp => "torrent",
        StreamKind::ExternalDeepLink => "external",
        _ => "direct",
    };
    match stream.quality {
        Some(quality) => format!("{quality}p · {kind}"),
        None => kind.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_playback_is_its_number() {
        assert_eq!(PlaybackId::new(3).get(), 3);
        assert!(PlaybackId::new(2) < PlaybackId::new(3));
    }

    #[test]
    fn a_stream_is_handed_over_with_everything_a_player_needs() {
        let stream = Stream {
            provider_id: "yamabiko".into(),
            subtitles: vec![
                Subtitle {
                    language: "en".into(),
                    url: "https://subs.example/ep6.ass".into(),
                    hard: false,
                    format: Some("ass".into()),
                },
                Subtitle {
                    language: "en".into(),
                    url: "https://cdn.example/burned".into(),
                    hard: true,
                    format: None,
                },
            ],
            pick_note: Some("top-ranked: a release".into()),
            thumbnail_url: Some("http://127.0.0.1:45001/thumb".into()),
            ..Stream::new("https://cdn.example/ep6/master.m3u8", StreamKind::Hls)
                .with_quality(1080)
                .with_header("Referer", "https://player.example/")
                .with_download_source("magnet:?xt=urn:btih:0000")
        };
        let playable = PlayableStream::from(&stream);
        assert_eq!(
            playable,
            PlayableStream {
                url: "https://cdn.example/ep6/master.m3u8".into(),
                kind: PlayableKind::Hls,
                quality: Some(1080),
                headers: vec![HttpHeader {
                    name: "Referer".into(),
                    value: "https://player.example/".into(),
                }],
                subtitles: vec![
                    SubtitleTrack {
                        language: "en".into(),
                        url: "https://subs.example/ep6.ass".into(),
                        hard: false,
                        format: Some("ass".into()),
                    },
                    SubtitleTrack {
                        language: "en".into(),
                        url: "https://cdn.example/burned".into(),
                        hard: true,
                        format: None,
                    },
                ],
                provider_id: "yamabiko".into(),
                label: "1080p · HLS".into(),
                thumbnail_url: Some("http://127.0.0.1:45001/thumb".into()),
            }
        );
    }

    #[test]
    fn a_stream_is_labelled_by_its_quality_and_kind() {
        let label = |kind, quality: Option<u32>| {
            let stream = Stream::new("https://cdn.example/a", kind);
            stream_label(&Stream { quality, ..stream })
        };
        assert_eq!(label(StreamKind::Hls, Some(720)), "720p · HLS");
        assert_eq!(label(StreamKind::Mp4, None), "direct");
        assert_eq!(label(StreamKind::TorrentHttp, Some(1080)), "1080p · torrent");
        assert_eq!(label(StreamKind::ExternalDeepLink, None), "external");
        for kind in [
            StreamKind::Hls,
            StreamKind::Mp4,
            StreamKind::TorrentHttp,
            StreamKind::ExternalDeepLink,
        ] {
            let playable = PlayableStream::from(&Stream::new("https://cdn.example/a", kind));
            assert_eq!(playable.kind, kind.into());
        }
    }
}
