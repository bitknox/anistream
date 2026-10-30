//! Matroska to HLS with fragmented MP4 segments, on request: the video copied as it is, the audio
//! copied or re-encoded to AAC. What a TV needs from a torrent's MKV, which it can't open itself.
//!
//! - The file is read as `Read + Seek`: a file here, or a torrent's through its loopback server,
//!   the tail first for the cues, then each cluster as playback reaches it.
//! - The segments are cut at the video keyframes the file's `Cues` name, grouped to about
//!   [`TARGET`] seconds each, so the playlist is known before any segment is made.
//! - A segment is made when it is asked for: seek to its keyframe, read its frames, write one
//!   `moof` + `mdat`. Nothing is kept but the running AAC encoder.
//! - Matroska stores presentation times in decode order. A segment's decode times are its
//!   presentation times sorted, and the difference rides in a version-1 `trun` as a signed
//!   composition offset, so B-frames and open GOPs come out right without parsing the bitstream.
//! - The decoder configuration is the track's `CodecPrivate`: for H.264 and HEVC it already is
//!   the `avcC` and `hvcC` body, for AAC the AudioSpecificConfig wrapped in an `esds`, for Opus
//!   the `OpusHead` rewritten as `dOps`, and for FLAC the STREAMINFO block as `dfLa`.
//! - Audio the plan turns to AAC goes through the platform's [`AacEncoder`]: a segment's audio is
//!   the source's frames in its time range plus one frame of lead-in before it, whose output (the
//!   encoder's priming) is trimmed, and its `tfdt` comes from the source's times, so segments made
//!   out of order line up. Segments asked for in order share one running encoder; any other
//!   starts a new one at that segment. The `esds` describes the AAC, not the source.
//!
//! A file this can't read (no cues, header stripping, a codec it doesn't copy) is an error, and
//! the cast hands the TV the Matroska file as it is instead.
//!
//! Subtitles are not here: ASS goes to WebVTT through [`crate::vtt`] as a side-loaded track.

use std::{
    io::{Read, Seek},
    sync::{Arc, Mutex},
};

use crate::{
    aac::{AAC_FRAME, AAC_RATE, AacEncoder, AacOutput, AacStream, AacUnit, AudioTrackInfo},
    formats::AudioCodec,
    mkv::{self, Block},
};

/// The length a segment is grouped to, in seconds.
pub const TARGET: f64 = 6.0;

/// A source the remuxer reads.
pub trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}

#[derive(Debug, thiserror::Error)]
pub enum RemuxError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a Matroska file this reads: {0}")]
    Demux(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("the AAC encoder failed: {0}")]
    Encode(String),
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

/// What happens to the audio track.
#[derive(Clone)]
pub enum AudioMode {
    /// Copied: AAC, Opus or FLAC.
    Copy,
    /// Re-encoded to AAC by the platform's encoder.
    ToAac(Arc<dyn AacEncoder>),
}

/// How to remux one file.
#[derive(Clone)]
pub struct RemuxOptions {
    /// Which audio track, counted among the audio tracks.
    pub audio_track: usize,
    pub audio: AudioMode,
}

/// The encoder running across segments asked for in order.
struct Running {
    /// The segment it continues into.
    next: usize,
    stream: Box<dyn AacStream>,
    /// The source ticks fed so far: frames from here on are new to it.
    fed_until: i64,
}

/// The re-encode of one source track.
struct Encode {
    encoder: Arc<dyn AacEncoder>,
    track: AudioTrackInfo,
    /// One source frame, in ticks: the lead-in fed before a segment's start.
    lead: i64,
    running: Mutex<Option<Running>>,
}

pub struct Remuxer {
    file: Mutex<Box<dyn Source>>,
    video: Track,
    audio: Option<Track>,
    encode: Option<Encode>,
    /// Nanoseconds per Matroska tick.
    scale: u64,
    segments: Vec<Segment>,
    /// Every video cue, by time: where a segment's lead-in starts reading.
    cues: Vec<(u64, u64)>,
    /// Ticks added to every timestamp, so the earliest (an audio encoder's priming frame, at a
    /// few milliseconds below zero) lands at zero and both tracks keep their distance.
    shift: i64,
}

impl Remuxer {
    /// Open `source`, choose its first video track and the audio track `options` names, and plan
    /// the segments.
    pub fn open(mut source: Box<dyn Source>, options: RemuxOptions) -> Result<Self> {
        let header = mkv::header(&mut source).map_err(|e| RemuxError::Demux(e.to_string()))?;
        let scale = header.scale;
        let ticks_per_second = 1_000_000_000 / scale;
        let timescale = u32::try_from(ticks_per_second)
            .map_err(|_| RemuxError::Unsupported("an odd timestamp scale".into()))?;

        let entry = header
            .tracks
            .iter()
            .find(|t| t.kind == 1)
            .ok_or_else(|| RemuxError::Unsupported("no video track".into()))?;
        let video = {
            let codec = match entry.codec.as_str() {
                "V_MPEG4/ISO/AVC" => Codec::H264,
                "V_MPEGH/ISO/HEVC" => Codec::Hevc,
                other => {
                    return Err(RemuxError::Unsupported(format!(
                        "video {other}: only H.264 and HEVC are copied"
                    )));
                }
            };
            Track {
                number: entry.number,
                id: 1,
                codec,
                private: entry.private.clone(),
                timescale,
                width: u16::try_from(entry.width).unwrap_or(u16::MAX),
                height: u16::try_from(entry.height).unwrap_or(u16::MAX),
                channels: 0,
                sample_rate: 0,
                language: entry.language.clone(),
                delay: 0,
            }
        };

        let chosen = header
            .tracks
            .iter()
            .filter(|t| t.kind == 2)
            .nth(options.audio_track)
            .or_else(|| header.tracks.iter().find(|t| t.kind == 2));
        let mut audio = None;
        let mut encode = None;
        if let Some(entry) = chosen {
            let sample_rate = entry.sample_rate as u32;
            let channels = u16::try_from(entry.channels).unwrap_or(2);
            let language = entry.language.clone();
            let delay = i64::try_from(entry.codec_delay / scale).unwrap_or(0);
            match options.audio {
                AudioMode::Copy => {
                    let codec = match AudioCodec::from_matroska(&entry.codec) {
                        AudioCodec::Aac => Codec::Aac,
                        AudioCodec::Opus => Codec::Opus,
                        AudioCodec::Flac => Codec::Flac,
                        other => {
                            return Err(RemuxError::Unsupported(format!(
                                "{other} audio is not copied"
                            )));
                        }
                    };
                    audio = Some(Track {
                        number: entry.number,
                        id: 2,
                        codec,
                        private: if codec == Codec::Aac && entry.private.is_empty() {
                            asc_for(sample_rate, u8::try_from(channels).unwrap_or(2))
                        } else {
                            entry.private.clone()
                        },
                        timescale: if codec == Codec::Opus { 48_000 } else { sample_rate },
                        width: 0,
                        height: 0,
                        channels,
                        sample_rate,
                        language,
                        delay: if codec == Codec::Opus { 0 } else { delay },
                    });
                }
                AudioMode::ToAac(encoder) => {
                    let output = AacOutput::for_channels(channels);
                    let codec = AudioCodec::from_matroska(&entry.codec);
                    if !encoder.can_decode(&codec) {
                        return Err(RemuxError::Unsupported(format!(
                            "{codec} audio can't be decoded here"
                        )));
                    }
                    audio = Some(Track {
                        number: entry.number,
                        id: 2,
                        codec: Codec::Aac,
                        private: output.audio_specific_config(),
                        timescale: AAC_RATE,
                        width: 0,
                        height: 0,
                        channels: output.channels,
                        sample_rate: output.sample_rate,
                        language,
                        delay,
                    });
                    // One source frame: its default duration, or the longest these codecs
                    // use (1536 samples at 48 kHz is 32 ms).
                    let lead_ns = entry.default_duration.unwrap_or(64_000_000);
                    encode = Some(Encode {
                        encoder,
                        track: AudioTrackInfo {
                            codec,
                            codec_private: entry.private.clone(),
                            sample_rate,
                            channels,
                            output,
                        },
                        lead: i64::try_from(lead_ns.div_ceil(scale)).unwrap_or(64),
                        running: Mutex::new(None),
                    });
                }
            }
        }

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
        if header.duration == 0 {
            return Err(RemuxError::Unsupported("no duration".into()));
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
        mkv::blocks(&mut source, header.first_cluster, |block| {
            let delay = audio_delay.filter(|(n, _)| *n == block.track).map_or(0, |(_, d)| d);
            earliest = earliest.min(block.timestamp - delay);
            seen += 1;
            seen < 64
        })?;
        Ok(Self {
            file: Mutex::new(source),
            video,
            audio,
            encode,
            scale,
            segments,
            cues: keys,
            shift: -earliest,
        })
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    fn seconds(&self, ticks: i64) -> f64 {
        ticks as f64 * self.scale as f64 / 1e9
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
        let length = |s: &Segment| self.seconds(i64::try_from(s.end - s.start).unwrap_or(0));
        let longest = self.segments.iter().map(length).fold(0.0, f64::max);
        let mut out = format!(
            "#EXTM3U\n#EXT-X-VERSION:7\n#EXT-X-TARGETDURATION:{}\n#EXT-X-PLAYLIST-TYPE:VOD\n\
             #EXT-X-INDEPENDENT-SEGMENTS\n#EXT-X-MAP:URI=\"{init}\"\n",
            longest.ceil() as u64
        );
        for (index, segment) in self.segments.iter().enumerate() {
            out.push_str(&format!("#EXTINF:{:.3},\n{prefix}{index}.m4s\n", length(segment)));
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
        let (start, end) = (
            i64::try_from(segment.start).unwrap_or(i64::MAX),
            i64::try_from(segment.end).unwrap_or(i64::MAX),
        );

        // A re-encode continues the running encoder when this is the segment after its last, and
        // otherwise starts one a frame of lead-in early.
        let mut running =
            self.encode.as_ref().map(|e| e.running.lock().unwrap_or_else(|p| p.into_inner()));
        let continued =
            running.as_deref().and_then(Option::as_ref).is_some_and(|r| r.next == index);
        let encode_from = self.encode.as_ref().map(|encode| {
            if continued {
                running.as_deref().and_then(Option::as_ref).map_or(start, |r| r.fed_until)
            } else {
                start - encode.lead
            }
        });
        // Where to start reading: the segment's keyframe, or for a fresh encoder the cue at or
        // before its lead-in.
        let cluster = match encode_from {
            Some(from) if !continued && from < start => self
                .cues
                .iter()
                .rev()
                .find(|(time, _)| i64::try_from(*time).unwrap_or(i64::MAX) <= from)
                .map_or(segment.cluster, |(_, cluster)| *cluster),
            _ => segment.cluster,
        };

        let mut video: Vec<Block> = Vec::new();
        let mut audio: Vec<Block> = Vec::new();
        let audio_number = self.audio.as_ref().map(|a| a.number);
        let audio_delay = self.audio.as_ref().map_or(0, |a| a.delay);
        let audio_from = encode_from.unwrap_or(start);
        let mut video_done = false;
        let mut file = self.file.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        mkv::blocks(&mut *file, cluster, |block| {
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
                if (start == 0 || block.timestamp >= audio_from) && block.timestamp < end {
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
            video.iter().map(|f| u64::try_from(f.timestamp + shift).unwrap_or(0)).collect();
        sorted.sort_unstable();
        let segment_end = u64::try_from(end + shift).unwrap_or(0);
        let video_samples: Vec<Sample> = video
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let dts = sorted[i];
                let next = sorted.get(i + 1).copied().unwrap_or(segment_end.max(dts + 1));
                Sample {
                    duration: u32::try_from(next - dts).unwrap_or(u32::MAX),
                    size: u32::try_from(f.data.len()).unwrap_or(u32::MAX),
                    key: f.key,
                    offset: f.timestamp + shift - i64::try_from(dts).unwrap_or(0),
                }
            })
            .collect();

        let mut trafs = vec![(self.video.id, sorted[0], video_samples)];
        let mut payloads: Vec<Vec<u8>> = video.into_iter().map(|f| f.data).collect();
        let last = index + 1 == self.segments.len();
        match (&self.audio, &self.encode) {
            (Some(track), Some(encode)) => {
                let units =
                    self.encode_audio(encode, &mut running, index, start, &audio, last)?;
                if !units.is_empty() {
                    let base = (units[0].pts.max(0.0) * f64::from(AAC_RATE)).round() as u64;
                    let samples = units
                        .iter()
                        .map(|u| Sample {
                            duration: AAC_FRAME,
                            size: u32::try_from(u.data.len()).unwrap_or(u32::MAX),
                            key: true,
                            offset: 0,
                        })
                        .collect();
                    trafs.push((track.id, base, samples));
                    payloads.extend(units.into_iter().map(|u| u.data));
                }
            }
            (Some(track), None) if !audio.is_empty() => {
                let rate = u128::from(track.timescale);
                let scale = u128::from(self.scale);
                let to_rate = |ticks: i64| {
                    let ticks = u128::try_from(ticks.max(0)).unwrap_or(0);
                    u64::try_from(ticks * scale * rate / 1_000_000_000).unwrap_or(u64::MAX)
                };
                let samples = audio
                    .iter()
                    .enumerate()
                    .map(|(i, f)| Sample {
                        duration: audio_duration(track, &f.data).unwrap_or_else(|| {
                            let next =
                                audio.get(i + 1).map_or(f.timestamp + 21, |n| n.timestamp);
                            u32::try_from(to_rate(next - f.timestamp)).unwrap_or(0)
                        }),
                        size: u32::try_from(f.data.len()).unwrap_or(u32::MAX),
                        key: true,
                        offset: 0,
                    })
                    .collect();
                let base = to_rate(audio[0].timestamp + shift);
                payloads.extend(audio.into_iter().map(|f| f.data));
                trafs.push((track.id, base, samples));
            }
            _ => {}
        }
        let payloads: Vec<&[u8]> = payloads.iter().map(Vec::as_slice).collect();
        Ok(fragment(u32::try_from(index + 1).unwrap_or(u32::MAX), &trafs, &payloads))
    }

    /// The AAC units for segment `index` from its source `frames`: the running encoder's when it
    /// continues, a new one's otherwise with the output before the segment's start trimmed.
    fn encode_audio(
        &self,
        encode: &Encode,
        running: &mut Option<std::sync::MutexGuard<'_, Option<Running>>>,
        index: usize,
        start: i64,
        frames: &[Block],
        last: bool,
    ) -> Result<Vec<AacUnit>> {
        let slot =
            running.as_deref_mut().ok_or_else(|| RemuxError::Encode("no encoder".into()))?;
        let continued = slot.as_ref().is_some_and(|r| r.next == index);
        let mut current = match slot.take().filter(|_| continued) {
            Some(running) => running,
            None => Running {
                next: index,
                stream: encode
                    .encoder
                    .open(&encode.track)
                    .map_err(|e| RemuxError::Encode(e.to_string()))?,
                fed_until: start - encode.lead,
            },
        };
        let encode_error = |e: crate::aac::EncodeError| RemuxError::Encode(e.to_string());
        let mut units = Vec::new();
        let mut fed_until = current.fed_until;
        for frame in frames.iter().filter(|f| f.timestamp >= current.fed_until) {
            let pts = self.seconds(frame.timestamp + self.shift);
            units.extend(current.stream.push(&frame.data, pts).map_err(encode_error)?);
            fed_until = frame.timestamp + 1;
        }
        if last {
            units.extend(current.stream.flush().map_err(encode_error)?);
        }
        if !continued {
            // The lead-in's output, the encoder's priming among it, belongs to no segment.
            let from = self.seconds(start + self.shift) - 1e-6;
            units.retain(|u| u.pts >= from);
        }
        current.fed_until = fed_until.max(current.fed_until);
        current.next = index + 1;
        if !last {
            *slot = Some(current);
        }
        Ok(units)
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

    use crate::{
        aac::FakeEncoder,
        fake::matroska::{Fixture, FixtureAudio, matroska},
    };

    fn open(fixture: &Fixture, audio: AudioMode) -> Result<Remuxer> {
        let source: Box<dyn Source> = Box::new(std::io::Cursor::new(matroska(fixture)));
        Remuxer::open(source, RemuxOptions { audio_track: 0, audio })
    }

    fn find_all(data: &[u8], kind: &[u8]) -> Vec<usize> {
        data.windows(4).enumerate().filter(|(_, w)| *w == kind).map(|(i, _)| i).collect()
    }

    /// Each track's `tfdt` and sample count in a fragment, in track order.
    fn timing(fragment: &[u8]) -> Vec<(u64, u32)> {
        let tfdts = find_all(fragment, b"tfdt");
        let truns = find_all(fragment, b"trun");
        tfdts
            .iter()
            .zip(&truns)
            .map(|(&t, &r)| {
                let base = u64::from_be_bytes(fragment[t + 8..t + 16].try_into().unwrap());
                let count = u32::from_be_bytes(fragment[r + 8..r + 12].try_into().unwrap());
                (base, count)
            })
            .collect()
    }

    fn holds(data: &[u8], needle: &str) -> bool {
        data.windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    #[test]
    fn a_matroska_file_becomes_hls_with_its_aac_copied() {
        let fixture = Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() };
        let remuxer = open(&fixture, AudioMode::Copy).unwrap();
        assert_eq!(remuxer.segments().len(), 4, "keyframes every 2 s, grouped to 6 s");
        let playlist = remuxer.playlist("seg", "init.mp4");
        assert_eq!(playlist.matches("#EXTINF:").count(), 4);
        assert!(playlist.contains("#EXTINF:2.000,\nseg3.m4s"), "{playlist}");
        assert_eq!(remuxer.codecs(), "avc1.640028,mp4a.40.2");
        let init = remuxer.init().unwrap();
        assert!(holds(&init, "avc1") && holds(&init, "mp4a"));
        let segment = remuxer.segment(1).unwrap();
        assert!(holds(&segment, "v@6000;") && !holds(&segment, "v@5960;"));
        assert!(holds(&segment, "a@6006;"));
        // Video in milliseconds, audio at 48 kHz.
        assert_eq!(timing(&segment)[0], (6000, 150));
    }

    #[test]
    fn dolby_audio_is_turned_to_aac_and_lines_up_out_of_order() {
        let encoder = FakeEncoder::default();
        let opened = Arc::clone(&encoder.opened);
        let remuxer = open(&Fixture::default(), AudioMode::ToAac(Arc::new(encoder))).unwrap();
        assert_eq!(remuxer.codecs(), "avc1.640028,mp4a.40.2");
        let init = remuxer.init().unwrap();
        // The esds describes 5.1 AAC at 48 kHz, not the AC-3.
        assert!(init.windows(2).any(|w| w == [0x11, 0xb0]));

        let first = remuxer.segment(0).unwrap();
        let second = remuxer.segment(1).unwrap();
        assert_eq!(
            opened.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "in order: one encoder"
        );
        let (a0, n0) = timing(&first)[1];
        let (a1, _) = timing(&second)[1];
        assert_eq!(a0, 0, "the priming is trimmed");
        assert_eq!(
            a1,
            a0 + u64::from(n0) * 1024,
            "the running encoder carries on without a gap"
        );

        // A seek: a fresh encoder, its lead-in trimmed, its times from the source's.
        let last = remuxer.segment(3).unwrap();
        assert_eq!(opened.load(std::sync::atomic::Ordering::Relaxed), 2);
        let (a3, n3) = timing(&last)[1];
        assert!((a3 as i64 - 18 * 48_000).abs() < 1024, "tfdt {a3}");
        assert!(n3 > 0);
        assert!(!holds(&last, &format!("aac@{}", 17_979)), "no lead-in output");
    }

    #[test]
    fn what_the_remuxer_cant_read_is_an_error() {
        assert!(matches!(
            open(&Fixture::default(), AudioMode::Copy),
            Err(RemuxError::Unsupported(_))
        ));
        let no_cues =
            Fixture { cues: false, audio: Some(FixtureAudio::aac()), ..Fixture::default() };
        assert!(open(&no_cues, AudioMode::Copy).is_err());
        let vp9 = Fixture { video: "V_VP9", ..Fixture::default() };
        assert!(open(&vp9, AudioMode::Copy).is_err());
        assert!(matches!(
            open(&Fixture::default(), AudioMode::ToAac(Arc::new(crate::aac::NoEncoder))),
            Err(RemuxError::Unsupported(_))
        ));
    }
}
