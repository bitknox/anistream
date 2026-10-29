//! What a stream holds, read before it is cast: its codecs and container, which choose the plans,
//! and for HLS whether the TV may fetch it by itself.
//!
//! - HLS: the master playlist's `CODECS`, the media playlist's `EXT-X-MAP` (fragmented MP4) or
//!   its absence (MPEG-TS), and the init segment's sample entries when the master names none.
//! - A progressive file: its first bytes say Matroska, WebM or MP4; an MP4's sample entries are
//!   read from its `moov`, at the head or the tail.
//! - Matroska: the tracks, as the remuxer reads them.
//!
//! The probe asks as the receiver will, with `Origin: https://www.gstatic.com`, and passes when
//! the playlist and its first segment both answer with `Access-Control-Allow-Origin` and the
//! segment's bytes are media rather than media dressed as a picture, which only the mender's
//! repair would play.

use std::{
    io::{Read, Seek},
    sync::Arc,
};

use crate::{
    fetch::{RangeFile, Upstream, UpstreamResponse},
    formats::{AudioCodec, AudioTrack, Delivery, MediaInfo, VideoFormat},
    mkv,
    relay::resolve,
};

/// The origin the Default Media Receiver's fetches carry.
pub const RECEIVER_ORIGIN: &str = "https://www.gstatic.com";

/// What the probe found when it asked as the receiver will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CorsProbe {
    /// The playlist answered with CORS.
    pub manifest: bool,
    /// The first segment did too.
    pub segment: bool,
    /// The segment's bytes are plain media: MPEG-TS or fragmented MP4, not a disguise.
    pub plain_media: bool,
}

impl CorsProbe {
    pub fn passed(&self) -> bool {
        self.manifest && self.segment && self.plain_media
    }
}

/// The largest playlist read.
const PLAYLIST_LIMIT: usize = 4 << 20;

fn cors_allowed(response: &UpstreamResponse) -> bool {
    response
        .header("access-control-allow-origin")
        .is_some_and(|v| v.trim() == "*" || v.trim() == RECEIVER_ORIGIN)
}

async fn get(
    upstream: &dyn Upstream,
    url: &str,
    headers: &[(String, String)],
    range: Option<&str>,
) -> std::io::Result<UpstreamResponse> {
    let mut asked = headers.to_vec();
    asked.push(("Origin".into(), RECEIVER_ORIGIN.into()));
    if let Some(range) = range {
        asked.push(("Range".into(), range.into()));
    }
    let response = upstream.get(url, &asked).await?;
    if !(200..300).contains(&response.status) {
        return Err(std::io::Error::other(format!("answered {}", response.status)));
    }
    Ok(response)
}

/// An HLS stream's media and what the TV would find fetching it itself.
pub async fn hls(
    upstream: &dyn Upstream,
    url: &str,
    headers: &[(String, String)],
) -> (MediaInfo, CorsProbe) {
    let mut info = MediaInfo::default();
    let mut probe = CorsProbe::default();
    let Ok(response) = get(upstream, url, headers, None).await else { return (info, probe) };
    probe.manifest = cors_allowed(&response);
    let Ok(body) = response.bytes(PLAYLIST_LIMIT).await else { return (info, probe) };
    let mut text = String::from_utf8_lossy(&body).into_owned();
    let mut playlist_url = url.to_owned();

    if let Some(variant) = best_variant(&text) {
        if let Some(codecs) = variant.codecs {
            read_codecs(&codecs, &mut info);
        }
        info.height = variant.height;
        let variant = variant.uri;
        playlist_url = resolve(url, &variant);
        let Ok(response) = get(upstream, &playlist_url, headers, None).await else {
            return (info, probe);
        };
        probe.manifest &= cors_allowed(&response);
        let Ok(body) = response.bytes(PLAYLIST_LIMIT).await else { return (info, probe) };
        text = String::from_utf8_lossy(&body).into_owned();
    }

    let map = text.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("#EXT-X-MAP:").and_then(|attrs| attribute(attrs, "URI"))
    });
    info.container = Some(if map.is_some() { Delivery::HlsFmp4 } else { Delivery::HlsTs });
    if let Some(map) = &map
        && info.video.is_none()
        && let Ok(response) = get(upstream, &resolve(&playlist_url, map), headers, None).await
        && let Ok(init) = response.prefix(1 << 20).await
    {
        read_sample_entries(&init, &mut info);
    }

    let first = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'));
    if let Some(first) = first
        && let Ok(response) =
            get(upstream, &resolve(&playlist_url, first), headers, Some("bytes=0-4095")).await
    {
        probe.segment = cors_allowed(&response);
        if let Ok(head) = response.prefix(4096).await {
            probe.plain_media = plain_media(&head, map.is_some());
        }
    }
    (info, probe)
}

/// A master playlist's variant: its address, its `CODECS` and the height its `RESOLUTION` says.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Variant {
    uri: String,
    codecs: Option<String>,
    height: Option<u32>,
}

/// The variant with the most bandwidth, when `text` is a master playlist.
fn best_variant(text: &str) -> Option<Variant> {
    let mut best: Option<(u64, Variant)> = None;
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        let Some(attrs) = line.strip_prefix("#EXT-X-STREAM-INF:") else { continue };
        let bandwidth = attribute(attrs, "BANDWIDTH").and_then(|b| b.parse().ok()).unwrap_or(0);
        let codecs = attribute(attrs, "CODECS");
        let height = attribute(attrs, "RESOLUTION")
            .and_then(|r| r.split_once(['x', 'X']).and_then(|(_, h)| h.parse().ok()));
        let Some(uri) = lines.by_ref().find(|l| !l.is_empty() && !l.starts_with('#')) else {
            break;
        };
        if best.as_ref().is_none_or(|(b, _)| bandwidth > *b) {
            best = Some((bandwidth, Variant { uri: uri.to_owned(), codecs, height }));
        }
    }
    best.map(|(_, variant)| variant)
}

/// An attribute of an HLS tag, quoted or not.
fn attribute(attrs: &str, name: &str) -> Option<String> {
    let mut rest = attrs;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = if let Some(quoted) = after.strip_prefix('"') {
            let end = quoted.find('"')?;
            (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
        } else {
            let end = after.find(',').unwrap_or(after.len());
            (&after[..end], after.get(end + 1..).unwrap_or_default())
        };
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(value.to_owned());
        }
        rest = next;
    }
    None
}

fn read_codecs(codecs: &str, info: &mut MediaInfo) {
    for codec in codecs.split(',') {
        if let Some(video) = VideoFormat::from_rfc6381(codec) {
            info.video.get_or_insert(video);
        } else if let Some(audio) = AudioCodec::from_rfc6381(codec) {
            info.audio.push(AudioTrack { codec: Some(audio), channels: 2, language: None });
        }
    }
}

/// Whether a segment's first bytes are media: MPEG-TS (perhaps after an ID3 tag), or a
/// fragmented MP4 box for an `fmp4` playlist.
fn plain_media(head: &[u8], fmp4: bool) -> bool {
    let mut at = 0;
    if head.starts_with(b"ID3") && head.len() >= 10 {
        let size = head[6..10].iter().fold(0usize, |acc, b| (acc << 7) | usize::from(b & 0x7f));
        at = 10 + size;
    }
    let ts = head.get(at) == Some(&0x47) && head.get(at + 188).is_none_or(|b| *b == 0x47);
    let mp4 = fmp4
        && head.get(4..8).is_some_and(|kind| {
            matches!(kind, b"ftyp" | b"styp" | b"moof" | b"sidx" | b"emsg" | b"prft")
        });
    ts || mp4
}

/// The container a progressive file's first bytes say.
pub fn sniff(head: &[u8]) -> Option<Delivery> {
    if head.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        let webm = head.windows(4).take(64).any(|w| w == b"webm");
        return Some(if webm { Delivery::WebM } else { Delivery::Mkv });
    }
    if head.get(4..8).is_some_and(|kind| kind == b"ftyp") {
        return Some(Delivery::Mp4);
    }
    None
}

/// Every sample entry under an `stsd` in `data`: the codec of each MP4 track.
pub fn read_sample_entries(data: &[u8], info: &mut MediaInfo) {
    let mut from = 0;
    while let Some(found) = find(&data[from..], b"stsd") {
        let at = from + found;
        from = at + 4;
        // After the type: version and flags, the entry count, then the first entry's size and
        // type.
        let Some(entry) = data.get(at + 12..) else { break };
        let (Some(size), Some(kind)) = (entry.get(..4), entry.get(4..8)) else { continue };
        let size = u32::from_be_bytes([size[0], size[1], size[2], size[3]]) as usize;
        let body = entry.get(8..size.min(entry.len())).unwrap_or_default();
        let kind = String::from_utf8_lossy(kind).into_owned();
        // A visual sample entry's height follows its reserved and predefined fields.
        let height = body
            .get(26..28)
            .map(|h| u32::from(u16::from_be_bytes([h[0], h[1]])))
            .filter(|h| *h > 0);
        if matches!(kind.as_str(), "avc1" | "avc3" | "hvc1" | "hev1" | "vp09" | "av01")
            && info.video.is_none()
        {
            info.height = height;
        }
        match kind.as_str() {
            "avc1" | "avc3" => {
                let profile = find(body, b"avcC").and_then(|i| body.get(i + 5)).copied();
                info.video.get_or_insert(VideoFormat::from_avc_profile(profile.unwrap_or(100)));
            }
            "hvc1" | "hev1" => {
                let profile =
                    find(body, b"hvcC").and_then(|i| body.get(i + 5)).map(|p| p & 0x1f);
                info.video.get_or_insert(VideoFormat::from_hevc_profile(profile.unwrap_or(1)));
            }
            "vp09" => {
                info.video.get_or_insert(VideoFormat::Vp9);
            }
            "av01" => {
                info.video.get_or_insert(VideoFormat::Av1);
            }
            "encv" | "enca" => {}
            other => {
                if let Some(codec) = AudioCodec::from_rfc6381(other) {
                    let channels =
                        body.get(16..18).map_or(2, |c| u16::from_be_bytes([c[0], c[1]]));
                    info.audio.push(AudioTrack {
                        codec: Some(codec),
                        channels,
                        language: None,
                    });
                }
            }
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// A progressive file over HTTP: its container from its first bytes, and an MP4's codecs from
/// its `moov`, read from the head or else the tail.
pub async fn progressive(
    upstream: &dyn Upstream,
    url: &str,
    headers: &[(String, String)],
) -> MediaInfo {
    let mut info = MediaInfo::default();
    let Ok(response) = get(upstream, url, headers, Some("bytes=0-524287")).await else {
        return info;
    };
    let size = response.total_size();
    let Ok(head) = response.prefix(512 * 1024).await else { return info };
    info.container = sniff(&head);
    if info.container == Some(Delivery::Mp4) {
        read_sample_entries(&head, &mut info);
        if info.video.is_none()
            && let Some(size) = size.filter(|s| *s > 512 * 1024)
        {
            let from = size.saturating_sub(4 << 20);
            if let Ok(response) =
                get(upstream, url, headers, Some(&format!("bytes={from}-"))).await
                && let Ok(tail) = response.prefix(4 << 20).await
            {
                read_sample_entries(&tail, &mut info);
            }
        }
    }
    info
}

/// A Matroska file's tracks, from any `Read + Seek`: a file here, or a torrent's through its
/// loopback server. Blocking.
pub fn matroska<R: Read + Seek>(reader: &mut R) -> std::io::Result<MediaInfo> {
    let header = mkv::header(reader)?;
    let mut info = MediaInfo { container: Some(Delivery::Mkv), ..MediaInfo::default() };
    for track in &header.tracks {
        match track.kind {
            1 if info.video.is_none() => {
                info.height = u32::try_from(track.height).ok().filter(|h| *h > 0);
                info.video = Some(match track.codec.as_str() {
                    "V_MPEG4/ISO/AVC" => VideoFormat::from_avc_profile(
                        track.private.get(1).copied().unwrap_or(100),
                    ),
                    "V_MPEGH/ISO/HEVC" => VideoFormat::from_hevc_profile(
                        track.private.get(1).map_or(1, |p| p & 0x1f),
                    ),
                    "V_VP9" => VideoFormat::Vp9,
                    "V_AV1" => VideoFormat::Av1,
                    other => VideoFormat::Other(other.trim_start_matches("V_").to_owned()),
                });
            }
            2 => info.audio.push(AudioTrack {
                codec: Some(AudioCodec::from_matroska(&track.codec)),
                channels: u16::try_from(track.channels).unwrap_or(2),
                language: Some(track.language.clone()).filter(|l| !l.is_empty()),
            }),
            _ => {}
        }
    }
    Ok(info)
}

/// A Matroska file through `upstream`, read on a blocking thread.
pub async fn matroska_remote(
    upstream: Arc<dyn Upstream>,
    url: &str,
    headers: Vec<(String, String)>,
) -> std::io::Result<MediaInfo> {
    let url = url.to_owned();
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || {
        let mut file = RangeFile::remote(upstream, &url, headers, runtime)?;
        matroska(&mut file)
    })
    .await
    .map_err(std::io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_master_names_the_codecs_of_its_best_variant() {
        let master = "#EXTM3U\n\
            #EXT-X-STREAM-INF:BANDWIDTH=800000,CODECS=\"avc1.4d401f,mp4a.40.2\"\nlow.m3u8\n\
            #EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,CODECS=\"hvc1.2.4.L120.B0,ec-3\"\nhigh/index.m3u8\n";
        let variant = best_variant(master).unwrap();
        assert_eq!(variant.uri, "high/index.m3u8");
        assert_eq!(variant.height, Some(1080));
        let mut info = MediaInfo::default();
        read_codecs(&variant.codecs.unwrap(), &mut info);
        assert_eq!(info.video, Some(VideoFormat::HevcMain10));
        assert_eq!(info.audio[0].codec, Some(AudioCodec::Eac3));
        assert_eq!(best_variant("#EXTM3U\n#EXTINF:4,\nseg0.ts\n"), None);
    }

    #[test]
    fn segments_dressed_as_pictures_are_not_plain_media() {
        let mut ts = vec![0u8; 400];
        ts[0] = 0x47;
        ts[188] = 0x47;
        assert!(plain_media(&ts, false));
        let mut disguised = b"\x89PNG\r\n\x1a\n".to_vec();
        disguised.extend(&ts);
        assert!(!plain_media(&disguised, false));
        let mut id3 = b"ID3\x04\x00\x00\x00\x00\x00\x02xx".to_vec();
        id3.extend(&ts);
        assert!(plain_media(&id3, false));
        assert!(plain_media(b"\0\0\0\x18moof....", true));
        assert!(!plain_media(b"\0\0\0\x18moof....", false));
    }

    #[test]
    fn sample_entries_name_an_mp4s_codecs() {
        // stsd, version 0, one entry: an hvc1 with an hvcC of general profile 2.
        let mut hvcc = b"\0\0\0\x0bhvcC\x01\x02\x00".to_vec();
        hvcc.resize(11, 0);
        let mut entry = vec![0u8; 78];
        entry[24..26].copy_from_slice(&3840u16.to_be_bytes());
        entry[26..28].copy_from_slice(&2160u16.to_be_bytes());
        entry.extend(&hvcc);
        let mut stsd = b"\0\0\0\0stsd\0\0\0\0\0\0\0\x01".to_vec();
        stsd.extend(((8 + entry.len()) as u32).to_be_bytes());
        stsd.extend(b"hvc1");
        stsd.extend(&entry);
        let mut audio = b"\0\0\0\0stsd\0\0\0\0\0\0\0\x01\0\0\0\x24ac-3".to_vec();
        audio.extend([0u8; 16]);
        audio.extend(6u16.to_be_bytes());
        audio.extend([0u8; 10]);
        stsd.extend(audio);
        let mut info = MediaInfo::default();
        read_sample_entries(&stsd, &mut info);
        assert_eq!(info.video, Some(VideoFormat::HevcMain10));
        assert_eq!(info.height, Some(2160));
        assert_eq!(info.audio[0].codec, Some(AudioCodec::Ac3));
        assert_eq!(info.audio[0].channels, 6);
    }

    #[test]
    fn attributes_read_quoted_and_bare() {
        let attrs = "BANDWIDTH=1,CODECS=\"a,b\",URI=\"init.mp4\"";
        assert_eq!(attribute(attrs, "CODECS").as_deref(), Some("a,b"));
        assert_eq!(attribute(attrs, "URI").as_deref(), Some("init.mp4"));
        assert_eq!(attribute(attrs, "BANDWIDTH").as_deref(), Some("1"));
        assert_eq!(
            sniff(&[0x1A, 0x45, 0xDF, 0xA3, 0x42, 0x82, 0x84, b'w', b'e', b'b', b'm']),
            Some(Delivery::WebM)
        );
    }
}
