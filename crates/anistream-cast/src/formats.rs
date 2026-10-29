//! The ways a stream can be cast, best first: where the TV reads it from, in what container, and
//! what happens to its audio. What a TV is known to refuse is left out, and when nothing is left
//! the reason says which part can't play there.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{aac::AacEncoder, memory::CastCaps, probe::CorsProbe};

/// The video codec, as precise as a TV's answer differs by it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoFormat {
    H264,
    Hevc,
    HevcMain10,
    Vp9,
    Av1,
    /// Anything else, named: H.264 High 10 among them, which no Chromecast decodes.
    Other(String),
}

impl VideoFormat {
    /// The part's name in a TV's memory.
    pub fn part(&self) -> String {
        match self {
            Self::H264 => "h264".into(),
            Self::Hevc => "hevc".into(),
            Self::HevcMain10 => "hevc-main10".into(),
            Self::Vp9 => "vp9".into(),
            Self::Av1 => "av1".into(),
            Self::Other(name) => format!("video:{}", name.to_ascii_lowercase()),
        }
    }

    /// From an RFC 6381 codecs entry: `avc1.640028`, `hvc1.2.4.L120.B0`, `vp09.00.40.08`.
    pub fn from_rfc6381(codec: &str) -> Option<Self> {
        let codec = codec.trim();
        let (family, rest) = codec.split_once('.').unwrap_or((codec, ""));
        Some(match family {
            "avc1" | "avc3" => {
                let profile = rest.get(..2).and_then(|p| u8::from_str_radix(p, 16).ok());
                Self::from_avc_profile(profile.unwrap_or(100))
            }
            "hvc1" | "hev1" => {
                let profile =
                    rest.split('.').next().map(|p| p.trim_start_matches(['A', 'B', 'C']));
                Self::from_hevc_profile(profile.and_then(|p| p.parse().ok()).unwrap_or(1))
            }
            "vp09" | "vp9" => Self::Vp9,
            "av01" => Self::Av1,
            "mp4a" | "opus" | "Opus" | "fLaC" | "flac" | "ac-3" | "ec-3" | "dtsc" | "dtsh"
            | "dtsl" | "mlpa" => return None,
            other => Self::Other(other.to_owned()),
        })
    }

    /// H.264 by its `profile_idc`: High 10 and above are a different decoder.
    pub fn from_avc_profile(profile: u8) -> Self {
        match profile {
            110 | 122 | 244 => Self::Other("H.264 High 10".into()),
            _ => Self::H264,
        }
    }

    /// HEVC by its general profile: 2 is Main 10.
    pub fn from_hevc_profile(profile: u8) -> Self {
        if profile == 2 { Self::HevcMain10 } else { Self::Hevc }
    }

    /// Whether any Default Media Receiver could try it.
    fn castable(&self) -> bool {
        !matches!(self, Self::Other(_))
    }
}

impl fmt::Display for VideoFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::H264 => "H.264",
            Self::Hevc => "HEVC",
            Self::HevcMain10 => "HEVC Main 10",
            Self::Vp9 => "VP9",
            Self::Av1 => "AV1",
            Self::Other(name) => name,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    Aac,
    Opus,
    Flac,
    Ac3,
    Eac3,
    Dts,
    TrueHd,
    Other(String),
}

impl AudioCodec {
    pub fn part(&self) -> String {
        match self {
            Self::Aac => "aac".into(),
            Self::Opus => "opus".into(),
            Self::Flac => "flac".into(),
            Self::Ac3 => "ac3".into(),
            Self::Eac3 => "eac3".into(),
            Self::Dts => "dts".into(),
            Self::TrueHd => "truehd".into(),
            Self::Other(name) => format!("audio:{}", name.to_ascii_lowercase()),
        }
    }

    /// From a Matroska codec id: `A_AAC`, `A_OPUS`, `A_EAC3`.
    pub fn from_matroska(id: &str) -> Self {
        match id {
            "A_AAC" => Self::Aac,
            id if id.starts_with("A_AAC/") => Self::Aac,
            "A_OPUS" => Self::Opus,
            "A_FLAC" => Self::Flac,
            "A_AC3" => Self::Ac3,
            "A_EAC3" => Self::Eac3,
            id if id.starts_with("A_DTS") => Self::Dts,
            "A_TRUEHD" => Self::TrueHd,
            other => Self::Other(other.trim_start_matches("A_").to_owned()),
        }
    }

    /// From an RFC 6381 codecs entry or an MP4 sample entry's type.
    pub fn from_rfc6381(codec: &str) -> Option<Self> {
        let family = codec.trim().split('.').next().unwrap_or_default();
        Some(match family {
            "mp4a" => Self::Aac,
            "opus" | "Opus" => Self::Opus,
            "fLaC" | "flac" => Self::Flac,
            "ac-3" => Self::Ac3,
            "ec-3" => Self::Eac3,
            "dtsc" | "dtsh" | "dtsl" | "dtse" => Self::Dts,
            "mlpa" => Self::TrueHd,
            _ => return None,
        })
    }

    /// Whether the TV gets it as it is when the container carries it: AAC always, Opus and FLAC
    /// unless that TV refused them. The rest is turned to AAC.
    fn copied(&self) -> bool {
        matches!(self, Self::Aac | Self::Opus | Self::Flac)
    }
}

impl fmt::Display for AudioCodec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Aac => "AAC",
            Self::Opus => "Opus",
            Self::Flac => "FLAC",
            Self::Ac3 => "AC-3",
            Self::Eac3 => "E-AC-3",
            Self::Dts => "DTS",
            Self::TrueHd => "TrueHD",
            Self::Other(name) => name,
        })
    }
}

/// The container the TV is handed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Delivery {
    HlsFmp4,
    HlsTs,
    Mp4,
    Mkv,
    WebM,
}

impl Delivery {
    pub fn part(self) -> &'static str {
        match self {
            Self::HlsFmp4 => "hls-fmp4",
            Self::HlsTs => "hls-ts",
            Self::Mp4 => "mp4",
            Self::Mkv => "mkv",
            Self::WebM => "webm",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            Self::HlsFmp4 | Self::HlsTs => "application/x-mpegurl",
            Self::Mp4 => "video/mp4",
            Self::Mkv => "video/x-matroska",
            Self::WebM => "video/webm",
        }
    }

    pub fn is_hls(self) -> bool {
        matches!(self, Self::HlsFmp4 | Self::HlsTs)
    }
}

/// What happens to the audio.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioPlan {
    Copy(AudioCodec),
    /// Re-encoded on the sender by the remuxer.
    ToAac {
        from: AudioCodec,
    },
    /// The stream has no audio.
    Silent,
}

/// Where the TV reads the stream from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastRoute {
    /// The TV fetches the source's own address.
    Direct { url: String },
    /// This device serves it through the relay; remuxed from Matroska to HLS when `remux`.
    Relayed { remux: bool },
}

/// One way to play a stream on a TV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastPlan {
    pub route: CastRoute,
    pub video: VideoFormat,
    pub audio: AudioPlan,
    pub delivery: Delivery,
}

impl CastPlan {
    /// The format parts a TV's memory keeps an answer for, in the order a refusal is blamed:
    /// video, then audio, then the container. H.264 and AAC are left out: every Chromecast
    /// plays them.
    pub fn parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if self.video != VideoFormat::H264 {
            parts.push(self.video.part());
        }
        if let AudioPlan::Copy(codec) = &self.audio
            && *codec != AudioCodec::Aac
        {
            parts.push(codec.part());
        }
        parts.push(self.delivery.part().to_owned());
        parts
    }

    pub fn relayed(&self) -> bool {
        matches!(self.route, CastRoute::Relayed { .. })
    }
}

/// What the source is, as the cast sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Hls,
    /// A progressive file over HTTP: MP4, WebM or Matroska by what it holds.
    Progressive,
    /// The torrent client's loopback server.
    Torrent,
    /// A file on this device: a download.
    File,
    /// A link for another app, which nothing here plays.
    DeepLink,
}

/// The stream as its source gave it, before the mender or the torrent server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastSource {
    /// The source's own address, or the file's path.
    pub url: String,
    pub kind: SourceKind,
    /// The headers its host wants, a referer most often.
    pub headers: Vec<(String, String)>,
}

impl CastSource {
    /// Whether the TV can't reach it however it asks: this device's loopback or a file here.
    pub fn local_only(&self) -> bool {
        matches!(self.kind, SourceKind::Torrent | SourceKind::File)
            || is_loopback_url(&self.url)
    }
}

pub fn is_loopback_url(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, host)| host);
    let host = if let Some(bracketed) = host.strip_prefix('[') {
        bracketed.split(']').next().unwrap_or_default()
    } else {
        host.split(':').next().unwrap_or_default()
    };
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

/// One audio track of a stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AudioTrack {
    pub codec: Option<AudioCodec>,
    pub channels: u16,
    pub language: Option<String>,
}

/// What the engine read from the stream: the MKV's tracks, the HLS master's `CODECS`, an MP4's
/// sample entries. Unknown parts are taken to be H.264 and AAC, what nearly every web source
/// sends, and a TV's answer corrects the guess.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MediaInfo {
    pub video: Option<VideoFormat>,
    pub audio: Vec<AudioTrack>,
    /// Which of `audio` plays: the one the viewer's language picks.
    pub audio_choice: usize,
    pub container: Option<Delivery>,
}

impl MediaInfo {
    pub fn chosen_audio(&self) -> Option<&AudioTrack> {
        self.audio.get(self.audio_choice).or_else(|| self.audio.first())
    }

    /// Choose the first audio track in `language`, when one is.
    pub fn choose_language(&mut self, language: Option<&str>) {
        let Some(language) = language.map(two_letter) else { return };
        if let Some(index) = self.audio.iter().position(|track| {
            track.language.as_deref().is_some_and(|l| two_letter(l) == language)
        }) {
            self.audio_choice = index;
        }
    }
}

/// A language tag's primary language as two letters, for the codes a file's tracks use:
/// `jpn` and `ja-JP` are both `ja`.
fn two_letter(tag: &str) -> String {
    let primary = tag.split(['-', '_']).next().unwrap_or_default().to_ascii_lowercase();
    let mapped = match primary.as_str() {
        "jpn" => "ja",
        "eng" => "en",
        "ger" | "deu" => "de",
        "fre" | "fra" => "fr",
        "spa" => "es",
        "ita" => "it",
        "por" => "pt",
        "rus" => "ru",
        "chi" | "zho" => "zh",
        "kor" => "ko",
        "ara" => "ar",
        "dan" => "da",
        "dut" | "nld" => "nl",
        "swe" => "sv",
        "nor" | "nob" => "no",
        "pol" => "pl",
        "tur" => "tr",
        other => other,
    };
    mapped.to_owned()
}

/// Why a stream can't be cast at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Uncastable {
    /// The TV can't decode the picture: H.264 High 10, or a codec it refused before.
    #[error("the TV can't play {0} video")]
    VideoCodec(String),
    /// The TV can't play the sound, and this device can't turn it to AAC.
    #[error("the TV can't play {0} audio")]
    AudioCodec(String),
    /// Every container this stream could be handed in, the TV refused.
    #[error("the TV refused {0}")]
    Container(String),
    /// A link for another app.
    #[error("this source opens in another app")]
    DeepLink,
}

/// The plans for `source` on the TV `caps` describes, best first, without those it is known to
/// refuse; or why none can play there. `media` is what the engine read from the stream, `probe`
/// whether the TV may fetch the source by itself, and `encoder` this platform's AAC encoder.
pub fn cast_plans(
    source: &CastSource,
    media: &MediaInfo,
    caps: &CastCaps,
    probe: &CorsProbe,
    encoder: &dyn AacEncoder,
) -> Result<Vec<CastPlan>, Uncastable> {
    if source.kind == SourceKind::DeepLink {
        return Err(Uncastable::DeepLink);
    }
    let video = media.video.clone().unwrap_or(VideoFormat::H264);
    if !video.castable() || caps.refused(&video.part()) {
        return Err(Uncastable::VideoCodec(video.to_string()));
    }
    let container = media.container.unwrap_or(match source.kind {
        SourceKind::Hls => Delivery::HlsTs,
        _ => Delivery::Mp4,
    });
    // A Matroska file lists every track; anything else that names none is taken to carry AAC.
    let source_audio = match media.chosen_audio() {
        Some(track) => Some(track.codec.clone().unwrap_or(AudioCodec::Aac)),
        None if container == Delivery::Mkv => None,
        None => Some(AudioCodec::Aac),
    };

    // Audio as it is, when the container carries it and the TV plays it.
    let copy = match &source_audio {
        None => Some(AudioPlan::Silent),
        Some(codec) if codec.copied() && !caps.refused(&codec.part()) => {
            Some(AudioPlan::Copy(codec.clone()))
        }
        Some(_) => None,
    };
    // Audio the remuxer turns to AAC.
    let remuxed = copy.clone().or_else(|| {
        let codec = source_audio.clone()?;
        encoder.can_decode(&codec).then_some(AudioPlan::ToAac { from: codec })
    });
    let audio_name = || source_audio.as_ref().map(ToString::to_string).unwrap_or_default();

    let plan = |route, audio: AudioPlan, delivery| CastPlan {
        route,
        video: video.clone(),
        audio,
        delivery,
    };
    let direct_allowed = source.headers.is_empty() && !source.local_only();

    let mut plans = Vec::new();
    match container {
        Delivery::Mkv => {
            let Some(audio) = remuxed else {
                return Err(Uncastable::AudioCodec(audio_name()));
            };
            // Remuxed to HLS first: what every Chromecast plays. Then the file as it is, which
            // a newer TV opens itself, for files the remuxer can't read.
            if !caps.refused(Delivery::HlsFmp4.part()) {
                plans.push(plan(CastRoute::Relayed { remux: true }, audio, Delivery::HlsFmp4));
            }
            if let Some(copy) = copy
                && !caps.refused(Delivery::Mkv.part())
            {
                plans.push(plan(CastRoute::Relayed { remux: false }, copy, Delivery::Mkv));
            }
            if plans.is_empty() {
                return Err(Uncastable::Container("Matroska".into()));
            }
        }
        delivery => {
            // HLS, MP4 and WebM are handed over as they come, so their audio can't change.
            let Some(audio) = copy else {
                return Err(Uncastable::AudioCodec(audio_name()));
            };
            if caps.refused(delivery.part()) {
                return Err(Uncastable::Container(delivery.part().to_owned()));
            }
            // Direct when nothing stands between the TV and the source: no headers, an address
            // it can reach, and for HLS, CORS and segments that are what they say.
            let direct = direct_allowed && (!delivery.is_hls() || probe.passed());
            if direct {
                plans.push(plan(
                    CastRoute::Direct { url: source.url.clone() },
                    audio.clone(),
                    delivery,
                ));
            }
            plans.push(plan(CastRoute::Relayed { remux: false }, audio, delivery));
        }
    }
    Ok(plans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aac::{FakeEncoder, NoEncoder};

    fn source(kind: SourceKind, headers: bool) -> CastSource {
        CastSource {
            url: "https://cdn.example/a/index.m3u8".into(),
            kind,
            headers: if headers {
                vec![("Referer".into(), "https://player.example/".into())]
            } else {
                Vec::new()
            },
        }
    }

    fn media(video: VideoFormat, audio: AudioCodec, container: Delivery) -> MediaInfo {
        MediaInfo {
            video: Some(video),
            audio: vec![AudioTrack { codec: Some(audio), channels: 2, language: None }],
            audio_choice: 0,
            container: Some(container),
        }
    }

    fn passed() -> CorsProbe {
        CorsProbe { manifest: true, segment: true, plain_media: true }
    }

    #[test]
    fn hls_goes_direct_only_when_the_probe_passes_and_needs_no_headers() {
        let caps = CastCaps::default();
        let info = media(VideoFormat::H264, AudioCodec::Aac, Delivery::HlsTs);
        let plans =
            cast_plans(&source(SourceKind::Hls, false), &info, &caps, &passed(), &NoEncoder)
                .unwrap();
        assert!(matches!(plans[0].route, CastRoute::Direct { .. }));
        assert_eq!(plans[1].route, CastRoute::Relayed { remux: false });

        let failed = CorsProbe { segment: false, ..passed() };
        let plans =
            cast_plans(&source(SourceKind::Hls, false), &info, &caps, &failed, &NoEncoder)
                .unwrap();
        assert_eq!(plans.len(), 1);
        assert!(plans[0].relayed());

        let plans =
            cast_plans(&source(SourceKind::Hls, true), &info, &caps, &passed(), &NoEncoder)
                .unwrap();
        assert_eq!(plans.len(), 1, "a referer keeps it on the relay");
    }

    #[test]
    fn a_matroska_file_is_remuxed_first_and_passed_through_after() {
        let caps = CastCaps::default();
        let info = media(VideoFormat::Hevc, AudioCodec::Opus, Delivery::Mkv);
        let plans = cast_plans(
            &source(SourceKind::Torrent, false),
            &info,
            &caps,
            &passed(),
            &NoEncoder,
        )
        .unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].route, CastRoute::Relayed { remux: true });
        assert_eq!(plans[0].delivery, Delivery::HlsFmp4);
        assert_eq!(plans[1].delivery, Delivery::Mkv);
        assert_eq!(plans[1].audio, AudioPlan::Copy(AudioCodec::Opus));
        assert_eq!(plans[0].parts(), ["hevc", "opus", "hls-fmp4"]);
    }

    #[test]
    fn dolby_audio_is_always_turned_to_aac_and_needs_an_encoder() {
        let caps = CastCaps::default();
        let info = media(VideoFormat::H264, AudioCodec::Eac3, Delivery::Mkv);
        let plans = cast_plans(
            &source(SourceKind::File, false),
            &info,
            &caps,
            &passed(),
            &FakeEncoder::default(),
        )
        .unwrap();
        assert_eq!(plans.len(), 1, "no pass-through: the TV would get E-AC-3");
        assert_eq!(plans[0].audio, AudioPlan::ToAac { from: AudioCodec::Eac3 });
        assert_eq!(
            cast_plans(&source(SourceKind::File, false), &info, &caps, &passed(), &NoEncoder),
            Err(Uncastable::AudioCodec("E-AC-3".into()))
        );
        let hls = media(VideoFormat::H264, AudioCodec::Ac3, Delivery::HlsTs);
        assert_eq!(
            cast_plans(
                &source(SourceKind::Hls, false),
                &hls,
                &caps,
                &passed(),
                &FakeEncoder::default()
            ),
            Err(Uncastable::AudioCodec("AC-3".into())),
            "HLS is handed over as it is"
        );
    }

    #[test]
    fn what_a_tv_refused_is_left_out() {
        let mut caps = CastCaps::default();
        caps.remember("opus", false, 0);
        let info = media(VideoFormat::H264, AudioCodec::Opus, Delivery::Mkv);
        let plans = cast_plans(
            &source(SourceKind::Torrent, false),
            &info,
            &caps,
            &passed(),
            &FakeEncoder::default(),
        )
        .unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].audio, AudioPlan::ToAac { from: AudioCodec::Opus });

        caps.remember("hevc-main10", false, 0);
        let info = media(VideoFormat::HevcMain10, AudioCodec::Aac, Delivery::Mkv);
        assert_eq!(
            cast_plans(
                &source(SourceKind::Torrent, false),
                &info,
                &caps,
                &passed(),
                &NoEncoder
            ),
            Err(Uncastable::VideoCodec("HEVC Main 10".into()))
        );
    }

    #[test]
    fn high_10_and_deep_links_cast_nowhere() {
        let caps = CastCaps::default();
        let info = MediaInfo {
            video: VideoFormat::from_rfc6381("avc1.6E0028"),
            ..MediaInfo::default()
        };
        assert!(matches!(
            cast_plans(&source(SourceKind::Hls, false), &info, &caps, &passed(), &NoEncoder),
            Err(Uncastable::VideoCodec(_))
        ));
        assert_eq!(
            cast_plans(
                &source(SourceKind::DeepLink, false),
                &MediaInfo::default(),
                &caps,
                &passed(),
                &NoEncoder
            ),
            Err(Uncastable::DeepLink)
        );
    }

    #[test]
    fn codecs_strings_read_as_their_formats() {
        assert_eq!(VideoFormat::from_rfc6381("avc1.640028"), Some(VideoFormat::H264));
        assert_eq!(
            VideoFormat::from_rfc6381("hvc1.2.4.L120.B0"),
            Some(VideoFormat::HevcMain10)
        );
        assert_eq!(VideoFormat::from_rfc6381("hev1.1.6.L93.B0"), Some(VideoFormat::Hevc));
        assert_eq!(VideoFormat::from_rfc6381("mp4a.40.2"), None);
        assert_eq!(AudioCodec::from_rfc6381("ec-3"), Some(AudioCodec::Eac3));
        assert!(is_loopback_url("http://127.0.0.1:4000/x"));
        assert!(is_loopback_url("http://localhost/x"));
        assert!(!is_loopback_url("https://cdn.example/x"));
    }

    #[test]
    fn the_viewers_language_chooses_the_audio() {
        let mut info = MediaInfo {
            audio: vec![
                AudioTrack {
                    codec: Some(AudioCodec::Aac),
                    channels: 2,
                    language: Some("jpn".into()),
                },
                AudioTrack {
                    codec: Some(AudioCodec::Ac3),
                    channels: 6,
                    language: Some("eng".into()),
                },
            ],
            ..MediaInfo::default()
        };
        info.choose_language(Some("en"));
        assert_eq!(info.audio_choice, 1);
        info.choose_language(Some("ja-JP"));
        assert_eq!(info.audio_choice, 0);
        info.choose_language(Some("fr"));
        assert_eq!(info.audio_choice, 0, "no French: as it was");
    }
}
