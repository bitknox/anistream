//! Matroska to HLS with fragmented MP4 segments, on request, with the video and audio copied as
//! they are: what a cast receiver needs from a torrent's MKV, which it cannot open itself.
//!
//! - The segments are cut at the video keyframes the file's `Cues` name, grouped to about
//!   [`TARGET`] seconds each, so the playlist is known before any segment is made: the `Cues` are
//!   read once, and a torrent has to fetch the file's tail for them first.
//! - A segment is made when it is asked for: seek to its keyframe, read its frames, write one
//!   `moof` + `mdat`. Nothing is kept.
//! - Matroska stores presentation times in decode order. A segment's decode times are its
//!   presentation times sorted, and the difference rides in a version-1 `trun` as a signed
//!   composition offset, so B-frames and open GOPs come out right without parsing the bitstream.
//! - The decoder configuration is the track's `CodecPrivate`: for H.264 and HEVC it already is
//!   the `avcC` and `hvcC` body, for AAC the AudioSpecificConfig wrapped in an `esds`, for Opus
//!   the `OpusHead` rewritten as `dOps`, and for FLAC the STREAMINFO block as `dfLa`.
//!
//! Subtitles are not here: ASS goes to WebVTT through [`crate::vtt`] as a side-loaded track.

use std::{fs::File, io::BufReader, path::Path, sync::Mutex};

use crate::mkv::{self, Block};

/// The length a segment is grouped to, in seconds.
pub const TARGET: f64 = 6.0;

#[derive(Debug, thiserror::Error)]
pub enum RemuxError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a Matroska file this reads: {0}")]
    Demux(String),
    #[error("{0}")]
    Unsupported(String),
}

type Result<T> = std::result::Result<T, RemuxError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Codec {
    H264,
    Hevc,
    Aac,
    Opus,
    Flac,
}

#[derive(Debug, Clone)]
struct Track {
    /// The Matroska track number.
    number: u64,
    /// The MP4 track id: 1 for video, 2 for audio.
    id: u32,
    codec: Codec,
    private: Vec<u8>,
    timescale: u32,
    width: u16,
    height: u16,
    channels: u16,
    sample_rate: u32,
    language: String,
    /// Ticks to take off every block's time: the codec delay, for AAC and FLAC. Opus keeps its
    /// own, which the decoder skips by the `dOps` pre-skip.
    delay: i64,
}

/// One HLS segment: from `start` to `end`, in Matroska ticks, its first frame in the cluster at
/// `cluster`.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub start: u64,
    pub end: u64,
    pub cluster: u64,
}

pub struct Remuxer {
    file: Mutex<BufReader<File>>,
    video: Track,
    audio: Option<Track>,
    /// Nanoseconds per Matroska tick.
    scale: u64,
    segments: Vec<Segment>,
    /// Ticks added to every timestamp, so the earliest (an audio encoder's priming frame, at a
    /// few milliseconds below zero) lands at zero and both tracks keep their distance.
    shift: i64,
}

impl Remuxer {
    /// Open `path`, choose its first video and first audio track, and plan the segments.
    pub fn open(path: &Path) -> Result<Self> {
        let mut reader = BufReader::with_capacity(1 << 20, File::open(path)?);
        let header = mkv::header(&mut reader).map_err(|e| RemuxError::Demux(e.to_string()))?;
        let scale = header.scale;
        let ticks_per_second = 1_000_000_000 / scale;

        let mut video = None;
        let mut audio = None;
        for entry in &header.tracks {
            let language = entry.language.clone();
            match entry.kind {
                1 if video.is_none() => {
                    let codec = match entry.codec.as_str() {
                        "V_MPEG4/ISO/AVC" => Codec::H264,
                        "V_MPEGH/ISO/HEVC" => Codec::Hevc,
                        other => {
                            return Err(RemuxError::Unsupported(format!(
                                "video {other}: only H.264 and HEVC are copied"
                            )));
                        }
                    };
                    video = Some(Track {
                        number: entry.number,
                        id: 1,
                        codec,
                        private: entry.private.clone(),
                        timescale: ticks_per_second as u32,
                        width: entry.width as u16,
                        height: entry.height as u16,
                        channels: 0,
                        sample_rate: 0,
                        language,
                        delay: 0,
                    });
                }
                2 if audio.is_none() => {
                    let codec = match entry.codec.as_str() {
                        "A_AAC" => Codec::Aac,
                        id if id.starts_with("A_AAC/") => Codec::Aac,
                        "A_OPUS" => Codec::Opus,
                        "A_FLAC" => Codec::Flac,
                        // AC-3, E-AC-3, DTS, TrueHD and the rest would be re-encoded to AAC,
                        // which this prototype leaves out.
                        _ => continue,
                    };
                    let sample_rate = entry.sample_rate as u32;
                    audio = Some(Track {
                        number: entry.number,
                        id: 2,
                        codec,
                        private: if codec == Codec::Aac && entry.private.is_empty() {
                            asc_for(sample_rate, entry.channels as u8)
                        } else {
                            entry.private.clone()
                        },
                        timescale: if codec == Codec::Opus { 48_000 } else { sample_rate },
                        width: 0,
                        height: 0,
                        channels: entry.channels as u16,
                        sample_rate,
                        language,
                        delay: if codec == Codec::Opus {
                            0
                        } else {
                            (entry.codec_delay / scale) as i64
                        },
                    });
                }
                _ => {}
            }
        }
        let video = video.ok_or_else(|| RemuxError::Unsupported("no video track".into()))?;

        // The video's cues, grouped to about TARGET seconds a segment.
        let mut keys: Vec<(u64, u64)> = header
            .cues
            .iter()
            .filter(|cue| cue.track == video.number)
            .map(|cue| (cue.time, cue.cluster))
            .collect();
        keys.sort_unstable();
        keys.dedup_by_key(|key| key.0);
        if keys.is_empty() {
            return Err(RemuxError::Unsupported(
                "no cues for the video: the segments would need a scan of the whole file"
                    .into(),
            ));
        }
        if keys[0].0 != 0 {
            keys.insert(0, (0, header.first_cluster));
        }
        let target = (TARGET * ticks_per_second as f64) as u64;
        let mut segments = Vec::new();
        let (mut start, mut cluster) = keys[0];
        for &(key, at) in &keys[1..] {
            if key - start >= target {
                segments.push(Segment { start, end: key, cluster });
                (start, cluster) = (key, at);
            }
        }
        if header.duration > start {
            segments.push(Segment { start, end: header.duration, cluster });
        }
        let mut earliest = 0i64;
        let mut seen = 0;
        let audio_delay = audio.as_ref().map(|a| (a.number, a.delay));
        mkv::blocks(&mut reader, header.first_cluster, |block| {
            let delay = audio_delay.filter(|(n, _)| *n == block.track).map_or(0, |(_, d)| d);
            earliest = earliest.min(block.timestamp - delay);
            seen += 1;
            seen < 64
        })?;
        Ok(Self { file: Mutex::new(reader), video, audio, scale, segments, shift: -earliest })
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    fn seconds(&self, ticks: u64) -> f64 {
        ticks as f64 * self.scale as f64 / 1e9
    }

    /// What the tracks are, for the report.
    pub fn describe(&self) -> serde_json::Value {
        serde_json::json!({
            "video": format!("{:?} {}x{}", self.video.codec, self.video.width, self.video.height),
            "audio": self.audio.as_ref()
                .map(|a| format!("{:?} {} Hz {} ch", a.codec, a.sample_rate, a.channels)),
            "codecs": self.codecs(),
        })
    }

    /// The RFC 6381 codecs string.
    pub fn codecs(&self) -> String {
        let p = &self.video.private;
        let video = match self.video.codec {
            Codec::H264 if p.len() > 3 => format!("avc1.{:02x}{:02x}{:02x}", p[1], p[2], p[3]),
            Codec::Hevc if p.len() > 12 => {
                let space = ["", "A", "B", "C"][usize::from(p[1] >> 6)];
                let tier = if p[1] & 0x20 != 0 { 'H' } else { 'L' };
                let compat = u32::from_be_bytes([p[2], p[3], p[4], p[5]]).reverse_bits();
                let mut constraints: Vec<u8> = p[6..12].to_vec();
                while constraints.last() == Some(&0) {
                    constraints.pop();
                }
                let mut out = format!("hvc1.{space}{}.{compat:X}.{tier}{}", p[1] & 0x1f, p[12]);
                for byte in constraints {
                    out.push_str(&format!(".{byte:X}"));
                }
                out
            }
            _ => "avc1.640028".into(),
        };
        match &self.audio {
            Some(a) => {
                let audio = match a.codec {
                    Codec::Aac => {
                        format!("mp4a.40.{}", a.private.first().map_or(2, |b| b >> 3))
                    }
                    Codec::Opus => "opus".into(),
                    _ => "fLaC".into(),
                };
                format!("{video},{audio}")
            }
            None => video,
        }
    }

    /// The media playlist, its segments named `{prefix}{n}.m4s`.
    pub fn playlist(&self, prefix: &str, init: &str) -> String {
        let longest =
            self.segments.iter().map(|s| self.seconds(s.end - s.start)).fold(0.0, f64::max);
        let mut out = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{init}\"\n",
            longest.ceil() as u64
        );
        for (index, segment) in self.segments.iter().enumerate() {
            out.push_str(&format!(
                "#EXTINF:{:.3},\n{prefix}{index}.m4s\n",
                self.seconds(segment.end - segment.start)
            ));
        }
        out.push_str("#EXT-X-ENDLIST\n");
        out
    }

    /// The initialisation segment: `ftyp` and `moov`.
    pub fn init(&self) -> Result<Vec<u8>> {
        let trex =
            |id: u32| full(b"trex", 0, 0, &[&id.to_be_bytes(), &1u32.to_be_bytes(), &[0; 12]]);
        let mut traks = trak(&self.video)?;
        let mut trexes = trex(1);
        if let Some(audio) = &self.audio {
            traks.extend(trak(audio)?);
            trexes.extend(trex(2));
        }
        let mvhd = full(
            b"mvhd",
            0,
            0,
            &[
                &[0; 8],
                &1000u32.to_be_bytes(),
                &0u32.to_be_bytes(),
                &0x0001_0000u32.to_be_bytes(),
                &0x0100u16.to_be_bytes(),
                &[0; 10],
                &MATRIX,
                &[0; 24],
                &3u32.to_be_bytes(),
            ],
        );
        let moov = bx(b"moov", &[&mvhd, &traks, &bx(b"mvex", &[&trexes])]);
        let ftyp = bx(b"ftyp", &[b"iso6", &0u32.to_be_bytes(), b"iso6", b"mp41", b"dash"]);
        Ok([ftyp, moov].concat())
    }

    /// Segment `index`: one `moof` and its `mdat`, both tracks.
    pub fn segment(&self, index: usize) -> Result<Vec<u8>> {
        let segment = *self
            .segments
            .get(index)
            .ok_or_else(|| RemuxError::Unsupported(format!("no segment {index}")))?;
        let mut video: Vec<Block> = Vec::new();
        let mut audio: Vec<Block> = Vec::new();
        let audio_number = self.audio.as_ref().map(|a| a.number);
        let audio_delay = self.audio.as_ref().map_or(0, |a| a.delay);
        let (start, end) = (segment.start as i64, segment.end as i64);
        let mut video_done = false;
        let mut file = self.file.lock().unwrap_or_else(|e| e.into_inner());
        mkv::blocks(&mut *file, segment.cluster, |block| {
            if block.track == self.video.number {
                if block.key && block.timestamp >= end {
                    video_done = true;
                } else if !video_done
                    && (!video.is_empty() || (block.key && block.timestamp >= start))
                {
                    video.push(block);
                }
            } else if Some(block.track) == audio_number {
                let mut block = block;
                block.timestamp -= audio_delay;
                // The first segment takes the priming frames before zero too.
                if (start == 0 || block.timestamp >= start) && block.timestamp < end {
                    audio.push(block);
                } else if block.timestamp >= end && video_done {
                    return false;
                }
            }
            !(video_done && audio_number.is_none())
        })?;
        drop(file);
        if video.is_empty() {
            return Err(RemuxError::Demux(format!("segment {index} has no video")));
        }

        // Decode times are the presentation times sorted; durations run to the next one.
        let shift = self.shift;
        let mut sorted: Vec<u64> =
            video.iter().map(|f| (f.timestamp + shift).max(0) as u64).collect();
        sorted.sort_unstable();
        let video_samples: Vec<Sample> = video
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let dts = sorted[i];
                let next = sorted.get(i + 1).copied().unwrap_or(segment.end.max(dts + 1));
                Sample {
                    duration: (next - dts) as u32,
                    size: f.data.len() as u32,
                    key: f.key,
                    offset: f.timestamp + shift - dts as i64,
                }
            })
            .collect();

        let mut trafs = vec![(self.video.id, sorted[0], video_samples)];
        let mut payloads: Vec<&[u8]> = video.iter().map(|f| f.data.as_slice()).collect();
        if let Some(track) = &self.audio
            && !audio.is_empty()
        {
            let rate = u128::from(track.timescale);
            let scale = u128::from(self.scale);
            let to_rate =
                |ticks: u64| (u128::from(ticks) * scale * rate / 1_000_000_000) as u64;
            let samples = audio
                .iter()
                .enumerate()
                .map(|(i, f)| Sample {
                    duration: audio_duration(track, &f.data).unwrap_or_else(|| {
                        let next = audio.get(i + 1).map_or(f.timestamp + 21, |n| n.timestamp);
                        to_rate((next - f.timestamp).max(0) as u64) as u32
                    }),
                    size: f.data.len() as u32,
                    key: true,
                    offset: 0,
                })
                .collect();
            payloads.extend(audio.iter().map(|f| f.data.as_slice()));
            trafs.push((
                track.id,
                to_rate((audio[0].timestamp + shift).max(0) as u64),
                samples,
            ));
        }
        Ok(fragment(index as u32 + 1, &trafs, &payloads))
    }
}

struct Sample {
    duration: u32,
    size: u32,
    key: bool,
    offset: i64,
}

/// One `moof` with a `traf` per track, then the `mdat` holding every track's samples in order.
fn fragment(sequence: u32, trafs: &[(u32, u64, Vec<Sample>)], payloads: &[&[u8]]) -> Vec<u8> {
    // Built once with zero offsets to learn the moof's size, then again with the real ones.
    let build = |offsets: &[i32]| {
        let mut body = full(b"mfhd", 0, 0, &[&sequence.to_be_bytes()]);
        for ((id, base, samples), offset) in trafs.iter().zip(offsets) {
            let tfhd = full(b"tfhd", 0, 0x02_0000, &[&id.to_be_bytes()]);
            let tfdt = full(b"tfdt", 1, 0, &[&base.to_be_bytes()]);
            let mut entries = Vec::with_capacity(samples.len() * 16);
            for s in samples {
                entries.extend(s.duration.to_be_bytes());
                entries.extend(s.size.to_be_bytes());
                let flags: u32 = if s.key { 0x0200_0000 } else { 0x0101_0000 };
                entries.extend(flags.to_be_bytes());
                entries.extend((s.offset as i32).to_be_bytes());
            }
            let trun = full(
                b"trun",
                1,
                0x0001 | 0x0100 | 0x0200 | 0x0400 | 0x0800,
                &[&(samples.len() as u32).to_be_bytes(), &offset.to_be_bytes(), &entries],
            );
            body.extend(bx(b"traf", &[&tfhd, &tfdt, &trun]));
        }
        bx(b"moof", &[&body])
    };
    let moof_len = build(&vec![0; trafs.len()]).len();
    let mut offsets = Vec::new();
    let mut at = moof_len + 8;
    let mut payload = payloads.iter();
    for (_, _, samples) in trafs {
        offsets.push(at as i32);
        for _ in samples {
            at += payload.next().map_or(0, |p| p.len());
        }
    }
    let moof = build(&offsets);
    let mdat_len: usize = payloads.iter().map(|p| p.len()).sum::<usize>() + 8;
    let mut out = Vec::with_capacity(moof.len() + mdat_len);
    out.extend(moof);
    out.extend((mdat_len as u32).to_be_bytes());
    out.extend(b"mdat");
    for p in payloads {
        out.extend_from_slice(p);
    }
    out
}

const MATRIX: [u8; 36] = [
    0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0x40, 0, 0, 0,
];

fn bx(kind: &[u8; 4], parts: &[&[u8]]) -> Vec<u8> {
    let len: usize = parts.iter().map(|p| p.len()).sum::<usize>() + 8;
    let mut out = Vec::with_capacity(len);
    out.extend((len as u32).to_be_bytes());
    out.extend(kind);
    for p in parts {
        out.extend_from_slice(p);
    }
    out
}

fn full(kind: &[u8; 4], version: u8, flags: u32, parts: &[&[u8]]) -> Vec<u8> {
    let mut head = vec![version];
    head.extend(&flags.to_be_bytes()[1..]);
    let mut all: Vec<&[u8]> = vec![&head];
    all.extend_from_slice(parts);
    bx(kind, &all)
}

fn language_code(language: &str) -> [u8; 2] {
    let code: Vec<u8> = language.bytes().take(3).collect();
    if code.len() != 3 || !code.iter().all(u8::is_ascii_lowercase) {
        return language_code("und");
    }
    let packed = code.iter().fold(0u16, |acc, c| (acc << 5) | u16::from(c - 0x60));
    packed.to_be_bytes()
}

fn trak(track: &Track) -> Result<Vec<u8>> {
    let video = matches!(track.codec, Codec::H264 | Codec::Hevc);
    let tkhd = full(
        b"tkhd",
        0,
        3,
        &[
            &[0; 8],
            &track.id.to_be_bytes(),
            &[0; 4],
            &0u32.to_be_bytes(),
            &[0; 8],
            &0u16.to_be_bytes(),
            &0u16.to_be_bytes(),
            &(if video { 0u16 } else { 0x0100 }).to_be_bytes(),
            &[0; 2],
            &MATRIX,
            &(u32::from(track.width) << 16).to_be_bytes(),
            &(u32::from(track.height) << 16).to_be_bytes(),
        ],
    );
    let mdhd = full(
        b"mdhd",
        0,
        0,
        &[
            &[0; 8],
            &track.timescale.to_be_bytes(),
            &0u32.to_be_bytes(),
            &language_code(&track.language),
            &[0; 2],
        ],
    );
    let hdlr = full(
        b"hdlr",
        0,
        0,
        &[
            &[0; 4],
            if video { b"vide" } else { b"soun" },
            &[0; 12],
            if video { b"VideoHandler\0" } else { b"SoundHandler\0" },
        ],
    );
    let header =
        if video { full(b"vmhd", 0, 1, &[&[0; 8]]) } else { full(b"smhd", 0, 0, &[&[0; 4]]) };
    let dref = full(b"dref", 0, 0, &[&1u32.to_be_bytes(), &full(b"url ", 0, 1, &[])]);
    let dinf = bx(b"dinf", &[&dref]);
    let stsd = full(b"stsd", 0, 0, &[&1u32.to_be_bytes(), &sample_entry(track)?]);
    let empty = |kind: &[u8; 4]| full(kind, 0, 0, &[&0u32.to_be_bytes()]);
    let stsz = full(b"stsz", 0, 0, &[&0u32.to_be_bytes(), &0u32.to_be_bytes()]);
    let stbl = bx(b"stbl", &[&stsd, &empty(b"stts"), &empty(b"stsc"), &stsz, &empty(b"stco")]);
    let minf = bx(b"minf", &[&header, &dinf, &stbl]);
    let mdia = bx(b"mdia", &[&mdhd, &hdlr, &minf]);
    Ok(bx(b"trak", &[&tkhd, &mdia]))
}

fn sample_entry(track: &Track) -> Result<Vec<u8>> {
    let visual = |config: &[u8]| {
        [
            &[0u8; 6][..],
            &1u16.to_be_bytes(),
            &[0; 16],
            &track.width.to_be_bytes(),
            &track.height.to_be_bytes(),
            &0x0048_0000u32.to_be_bytes(),
            &0x0048_0000u32.to_be_bytes(),
            &[0; 4],
            &1u16.to_be_bytes(),
            &[0; 32],
            &0x0018u16.to_be_bytes(),
            &(-1i16).to_be_bytes(),
            config,
        ]
        .concat()
    };
    // FLAC says its own sample size, and Chromium checks the entry against it.
    let bits = match track.codec {
        Codec::Flac => track
            .private
            .get(8 + 12..8 + 14)
            .map_or(16, |b| u16::from(((b[0] & 1) << 4) | (b[1] >> 4)) + 1),
        _ => 16,
    };
    let audio = |rate: u32, config: &[u8]| {
        [
            &[0u8; 6][..],
            &1u16.to_be_bytes(),
            &[0; 8],
            &track.channels.to_be_bytes(),
            &bits.to_be_bytes(),
            &[0; 4],
            &(if rate > 0xffff { 0 } else { rate << 16 }).to_be_bytes(),
            config,
        ]
        .concat()
    };
    Ok(match track.codec {
        Codec::H264 => bx(b"avc1", &[&visual(&bx(b"avcC", &[&track.private]))]),
        Codec::Hevc => bx(b"hvc1", &[&visual(&bx(b"hvcC", &[&track.private]))]),
        Codec::Aac => bx(b"mp4a", &[&audio(track.sample_rate, &esds(&track.private))]),
        Codec::Opus => bx(b"Opus", &[&audio(48_000, &dops(&track.private)?)]),
        Codec::Flac => bx(b"fLaC", &[&audio(track.sample_rate, &dfla(&track.private)?)]),
    })
}

/// An MPEG-4 descriptor, its length in the four-byte form every reader takes.
fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    let len = body.len() as u32;
    let mut out = vec![
        tag,
        0x80 | ((len >> 21) as u8 & 0x7f),
        0x80 | ((len >> 14) as u8 & 0x7f),
        0x80 | ((len >> 7) as u8 & 0x7f),
        len as u8 & 0x7f,
    ];
    out.extend_from_slice(body);
    out
}

fn esds(asc: &[u8]) -> Vec<u8> {
    let mut config = vec![0x40, 0x15, 0, 0, 0];
    config.extend(0u32.to_be_bytes());
    config.extend(0u32.to_be_bytes());
    config.extend(descriptor(5, asc));
    let mut es = vec![0, 2, 0];
    es.extend(descriptor(4, &config));
    es.extend(descriptor(6, &[2]));
    full(b"esds", 0, 0, &[&descriptor(3, &es)])
}

fn asc_for(rate: u32, channels: u8) -> Vec<u8> {
    const RATES: [u32; 13] = [
        96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
    ];
    let index = RATES.iter().position(|&r| r == rate).unwrap_or(4) as u16;
    let packed: u16 = (2 << 11) | (index << 7) | (u16::from(channels) << 3);
    packed.to_be_bytes().to_vec()
}

fn dops(head: &[u8]) -> Result<Vec<u8>> {
    if head.len() < 19 || &head[..8] != b"OpusHead" {
        return Err(RemuxError::Unsupported("an Opus track without its OpusHead".into()));
    }
    let mut body = vec![0, head[9]];
    body.extend(u16::from_le_bytes([head[10], head[11]]).to_be_bytes());
    body.extend(u32::from_le_bytes([head[12], head[13], head[14], head[15]]).to_be_bytes());
    body.extend(i16::from_le_bytes([head[16], head[17]]).to_be_bytes());
    body.push(head[18]);
    if head[18] != 0 {
        body.extend_from_slice(&head[19..]);
    }
    Ok(bx(b"dOps", &[&body]))
}

fn dfla(private: &[u8]) -> Result<Vec<u8>> {
    let blocks = private
        .strip_prefix(b"fLaC")
        .ok_or_else(|| RemuxError::Unsupported("a FLAC track without its header".into()))?;
    // STREAMINFO alone, marked as the last block.
    let len = blocks.get(1..4).map(|l| u32::from_be_bytes([0, l[0], l[1], l[2]]) as usize);
    let info = len
        .and_then(|len| blocks.get(..4 + len))
        .ok_or_else(|| RemuxError::Unsupported("a FLAC header without STREAMINFO".into()))?;
    let mut info = info.to_vec();
    info[0] = 0x80;
    Ok(full(b"dfLa", 0, 0, &[&info]))
}

/// A frame's duration in the track's timescale, from the codec's own framing.
fn audio_duration(track: &Track, data: &[u8]) -> Option<u32> {
    match track.codec {
        Codec::Aac => Some(1024),
        Codec::Opus => {
            let toc = *data.first()?;
            let config = toc >> 3;
            let tenths_ms: u32 = match config {
                0..=11 => [100, 200, 400, 600][usize::from(config % 4)],
                12..=15 => [100, 200][usize::from(config % 2)],
                _ => [25, 50, 100, 200][usize::from(config % 4)],
            };
            let frames = match toc & 3 {
                0 => 1,
                1 | 2 => 2,
                _ => u32::from(*data.get(1)? & 0x3f),
            };
            Some(tenths_ms * 48 / 10 * frames)
        }
        Codec::Flac => {
            let code = data.get(2)? >> 4;
            match code {
                1 => Some(192),
                2..=5 => Some(576 << (code - 2)),
                8..=15 => Some(256 << (code - 8)),
                6 | 7 => {
                    // After the UTF-8 coded frame number.
                    let first = *data.get(4)?;
                    let at = 5 + first.leading_ones().saturating_sub(1) as usize;
                    if code == 6 {
                        Some(u32::from(*data.get(at)?) + 1)
                    } else {
                        let n = u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]);
                        Some(u32::from(n) + 1)
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_pack_into_fifteen_bits() {
        assert_eq!(language_code("und"), [0x55, 0xc4]);
        assert_eq!(language_code("jpn"), [0x2a, 0x0e]);
    }

    #[test]
    fn opus_frames_are_measured_from_their_toc() {
        let track = Track {
            number: 2,
            id: 2,
            codec: Codec::Opus,
            private: Vec::new(),
            timescale: 48_000,
            width: 0,
            height: 0,
            channels: 2,
            sample_rate: 48_000,
            language: "und".into(),
            delay: 0,
        };
        // CELT, 20 ms, one frame: config 31.
        assert_eq!(audio_duration(&track, &[31 << 3]), Some(960));
        // SILK, 60 ms, code 3 with two frames.
        assert_eq!(audio_duration(&track, &[(3 << 3) | 3, 2]), Some(5760));
    }

    #[test]
    fn an_aac_config_is_built_for_files_without_one() {
        assert_eq!(asc_for(48000, 2), [0x11, 0x90]);
    }
}
