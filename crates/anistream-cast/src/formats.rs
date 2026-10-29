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

    /// The part for this format at `height`, read the way `delivery` is: what a TV's memory
    /// keeps, such as `hevc-main10/1080/progressive` or `h264/1080/hls`.
    pub fn part_in(&self, height: Option<u32>, delivery: Delivery) -> String {
        format!("{}/{}/{}", self.part(), height_class(height), delivery.family())
    }

    /// Whether every Chromecast plays it at `height` in `delivery`, so a TV is never asked:
    /// H.264 up to 720 lines anywhere, and up to 1080 lines as a file. The Nest Hub refuses
    /// 1080-line H.264 under HLS.
    pub fn plays_everywhere(&self, height: Option<u32>, delivery: Delivery) -> bool {
        *self == Self::H264
            && match height_class(height) {
                720 => true,
                1080 => !delivery.is_hls(),
                _ => false,
            }
    }
}

/// How tall the picture is, as a TV's answer differs by it: up to 720 lines, up to 1080, or
/// more. A height nobody read is taken as 1080, what most sources send.
pub fn height_class(height: Option<u32>) -> u32 {
    match height.unwrap_or(1080) {
        0..=720 => 720,
        721..=1080 => 1080,
        _ => 2160,
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

    /// The part for this codec read the way `delivery` is: `flac/hls`, `flac/progressive`.
    pub fn part_in(&self, delivery: Delivery) -> String {
        format!("{}/{}", self.part(), delivery.family())
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

    /// How the TV's media element reads it: `hls` through Media Source Extensions, where the
    /// Nest Hub stops at 720 lines, or `progressive` as a file.
    pub fn family(self) -> &'static str {
        if self.is_hls() { "hls" } else { "progressive" }
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
    /// The picture's height in lines, when the stream said.
    #[serde(default)]
    pub height: Option<u32>,
    pub audio: AudioPlan,
    pub delivery: Delivery,
}

impl CastPlan {
    /// The format parts a TV's memory keeps an answer for, in the order a refusal is blamed:
    /// the video at its height in this delivery's family, then the audio in that family, then
    /// the container. What every Chromecast plays is left out: AAC, and H.264 where
    /// [`VideoFormat::plays_everywhere`].
    pub fn parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if !self.video.plays_everywhere(self.height, self.delivery) {
            parts.push(self.video_part());
        }
        parts.extend(self.audio_part());
        parts.push(self.delivery.part().to_owned());
        parts
    }

    /// The copied audio's part in a TV's memory, when the audio is copied and isn't AAC.
    pub fn audio_part(&self) -> Option<String> {
        match &self.audio {
            AudioPlan::Copy(codec) if *codec != AudioCodec::Aac => {
                Some(codec.part_in(self.delivery))
            }
            _ => None,
        }
    }

    /// The video's part in a TV's memory.
    pub fn video_part(&self) -> String {
        self.video.part_in(self.height, self.delivery)
    }

    /// Whether the TV should be asked with a capability clip before this plan: a file whose
    /// picture the TV may not decode, where a TV with no decoder for it can play the sound alone
    /// and say nothing wrong. HLS refuses cleanly and needs no clip.
    pub fn wants_clip(&self) -> bool {
        !self.delivery.is_hls() && !self.video.plays_everywhere(self.height, self.delivery)
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
    /// The picture's height in lines.
    pub height: Option<u32>,
    pub audio: Vec<AudioTrack>,
    /// Which of `audio` plays: the one the viewer's language picks.
    pub audio_choice: usize,
    pub container: Option<Delivery>,
}

impl MediaInfo {
    pub fn chosen_audio(&self) -> Option<&AudioTrack> {
        self.audio.get(self.audio_choice).or_else(|| self.audio.first())
    }

    /// Whether the audio chosen is the file's first track, what a player with no choice of its
    /// own plays.
    pub fn chosen_is_first(&self) -> bool {
        self.audio_choice == 0 || self.audio_choice >= self.audio.len()
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
    if !video.castable() {
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
    let audio_name = || source_audio.as_ref().map(ToString::to_string).unwrap_or_default();

    // Audio as it is in `delivery`, when the container carries it and the TV plays it there.
    let copy_in = |delivery: Delivery| match &source_audio {
        None => Some(AudioPlan::Silent),
        Some(codec) if codec.copied() && !caps.refused(&codec.part_in(delivery)) => {
            Some(AudioPlan::Copy(codec.clone()))
        }
        Some(_) => None,
    };
    let plan = |route, audio: AudioPlan, delivery| CastPlan {
        route,
        video: video.clone(),
        height: media.height,
        audio,
        delivery,
    };
    let direct_allowed = source.headers.is_empty() && !source.local_only();

    let mut plans = Vec::new();
    match container {
        Delivery::Mkv => {
            // Remuxed to HLS first: what every Chromecast plays. The audio goes as it is where
            // the TV plays it, and then as AAC, for a TV that refuses or stalls on a codec it was
            // never asked about (the Nest Hub and FLAC).
            let hls = Delivery::HlsFmp4;
            let copied = copy_in(hls);
            let untried = |audio: &AudioPlan| match audio {
                AudioPlan::Copy(codec) => !caps.played(&codec.part_in(hls)),
                _ => false,
            };
            let to_aac = source_audio
                .as_ref()
                .filter(|codec| **codec != AudioCodec::Aac && encoder.can_decode(codec))
                .filter(|_| copied.as_ref().is_none_or(untried))
                .map(|codec| AudioPlan::ToAac { from: codec.clone() });
            for audio in copied.into_iter().chain(to_aac) {
                plans.push(plan(CastRoute::Relayed { remux: true }, audio, hls));
            }
            // Then the file as it is: for a file the remuxer can't read, and for a TV that
            // refuses the remux's HLS where it plays the file, as the Nest Hub does at 1080
            // lines. Its audio can't change, and the TV's media element plays the file's
            // first track, so only when that is the one chosen.
            if media.chosen_is_first()
                && let Some(audio) = copy_in(Delivery::Mkv)
            {
                plans.push(plan(CastRoute::Relayed { remux: false }, audio, Delivery::Mkv));
            }
            if plans.is_empty() {
                return Err(Uncastable::AudioCodec(audio_name()));
            }
        }
        delivery => {
            // HLS, MP4 and WebM are handed over as they come, so their audio can't change.
            let Some(audio) = copy_in(delivery) else {
                return Err(Uncastable::AudioCodec(audio_name()));
            };
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

    // What this TV is known to refuse is left out, and when nothing is left the reason is
    // the part it refused: the picture before the sound before the container.
    let refused: Vec<String> = plans
        .iter()
        .filter_map(|plan| plan.parts().into_iter().find(|part| caps.refused(part)))
        .collect();
    plans.retain(|plan| plan.parts().iter().all(|part| !caps.refused(part)));
    if plans.is_empty() {
        let video_prefix = format!("{}/", video.part());
        let audio_prefix = source_audio.as_ref().map(|codec| format!("{}/", codec.part()));
        return Err(if refused.iter().any(|part| part.starts_with(&video_prefix)) {
            Uncastable::VideoCodec(video.to_string())
        } else if audio_prefix
            .is_some_and(|prefix| refused.iter().any(|p| p.starts_with(&prefix)))
        {
            Uncastable::AudioCodec(audio_name())
        } else if container == Delivery::Mkv {
            Uncastable::Container("Matroska".into())
        } else {
            Uncastable::Container(container.part().to_owned())
        });
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
            height: None,
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
        assert_eq!(plans[0].parts(), ["hevc/1080/hls", "opus/hls", "hls-fmp4"]);
        assert!(!plans[0].wants_clip(), "HLS refuses cleanly");
        assert!(plans[1].wants_clip(), "a file with sound may play it alone");
    }

    #[test]
    fn a_codec_never_tried_is_followed_by_aac_in_the_same_load() {
        let mut caps = CastCaps::default();
        let info = media(VideoFormat::H264, AudioCodec::Flac, Delivery::Mkv);
        let plans = |caps: &CastCaps| {
            cast_plans(
                &source(SourceKind::File, false),
                &info,
                caps,
                &passed(),
                &FakeEncoder::default(),
            )
            .unwrap()
        };
        let first = plans(&caps);
        let audio: Vec<_> = first.iter().map(|p| (p.delivery, p.audio.clone())).collect();
        assert_eq!(
            audio,
            [
                (Delivery::HlsFmp4, AudioPlan::Copy(AudioCodec::Flac)),
                (Delivery::HlsFmp4, AudioPlan::ToAac { from: AudioCodec::Flac }),
                (Delivery::Mkv, AudioPlan::Copy(AudioCodec::Flac)),
            ]
        );
        caps.remember("flac/hls", true, 0);
        assert_eq!(plans(&caps).len(), 2, "FLAC known to play needs no fallback");
    }

    #[test]
    fn the_picture_is_kept_by_its_height_and_how_the_tv_reads_it() {
        let plan = |video: VideoFormat, height, delivery| CastPlan {
            route: CastRoute::Relayed { remux: false },
            video,
            height: Some(height),
            audio: AudioPlan::Copy(AudioCodec::Aac),
            delivery,
        };
        assert_eq!(plan(VideoFormat::H264, 720, Delivery::HlsTs).parts(), ["hls-ts"]);
        assert_eq!(
            plan(VideoFormat::H264, 1080, Delivery::HlsFmp4).parts(),
            ["h264/1080/hls", "hls-fmp4"]
        );
        assert_eq!(plan(VideoFormat::H264, 1080, Delivery::Mp4).parts(), ["mp4"]);
        assert!(!plan(VideoFormat::H264, 1080, Delivery::Mp4).wants_clip());
        assert!(plan(VideoFormat::H264, 2160, Delivery::Mp4).wants_clip());
        assert_eq!(
            plan(VideoFormat::HevcMain10, 2160, Delivery::Mkv).parts(),
            ["hevc-main10/2160/progressive", "mkv"]
        );

        // A TV that stops HLS at 720 lines still gets a 1080-line Matroska file as it is.
        let mut caps = CastCaps::default();
        caps.remember("h264/1080/hls", false, 0);
        let mut info = media(VideoFormat::H264, AudioCodec::Aac, Delivery::Mkv);
        info.height = Some(1080);
        let plans =
            cast_plans(&source(SourceKind::File, false), &info, &caps, &passed(), &NoEncoder)
                .unwrap();
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].delivery, Delivery::Mkv);
        info.height = Some(720);
        let plans =
            cast_plans(&source(SourceKind::File, false), &info, &caps, &passed(), &NoEncoder)
                .unwrap();
        assert_eq!(plans[0].delivery, Delivery::HlsFmp4, "720 lines are remuxed as before");
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
        caps.remember("opus/hls", false, 0);
        let info = media(VideoFormat::H264, AudioCodec::Opus, Delivery::Mkv);
        let plans = cast_plans(
            &source(SourceKind::Torrent, false),
            &info,
            &caps,
            &passed(),
            &FakeEncoder::default(),
        )
        .unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].audio, AudioPlan::ToAac { from: AudioCodec::Opus });
        assert_eq!(plans[1].audio, AudioPlan::Copy(AudioCodec::Opus), "files read Opus apart");

        caps.remember("hevc-main10/1080/hls", false, 0);
        caps.remember("hevc-main10/1080/progressive", false, 0);
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

    #[test]
    fn a_file_passed_through_only_when_the_chosen_audio_is_the_one_it_plays() {
        let track = |codec, language: &str| AudioTrack {
            codec: Some(codec),
            channels: 2,
            language: Some(language.into()),
        };
        let mut info = MediaInfo {
            audio: vec![track(AudioCodec::Eac3, "eng"), track(AudioCodec::Aac, "jpn")],
            ..media(VideoFormat::H264, AudioCodec::Aac, Delivery::Mkv)
        };
        let passed_through = |info: &MediaInfo| {
            cast_plans(
                &source(SourceKind::Torrent, false),
                info,
                &CastCaps::default(),
                &CorsProbe::default(),
                &NoEncoder,
            )
            .unwrap()
            .iter()
            .any(|plan| plan.route == CastRoute::Relayed { remux: false })
        };
        // A sub watcher's Japanese is the second track: the TV would play the English one.
        info.choose_language(Some("ja"));
        assert!(!passed_through(&info));
        info.audio.swap(0, 1);
        info.choose_language(Some("ja"));
        assert_eq!(info.audio_choice, 0);
        assert!(passed_through(&info));
    }
}
