//! Just enough Matroska for the remuxer: the segment's info and tracks, its cues, and the blocks
//! of the clusters from any cue on.
//!
//! The reader asks for what it needs by byte offset, so a torrent's file can be read as it
//! arrives: the head for the tracks, the tail for the cues (where mkvmerge and FFmpeg both put
//! them), then each cluster as playback reaches it.

use std::io::{Read, Seek, SeekFrom};

const EBML: u64 = 0x1A45_DFA3;
const SEGMENT: u64 = 0x1853_8067;
const SEEK_HEAD: u64 = 0x114D_9B74;
const SEEK: u64 = 0x4DBB;
const SEEK_ID: u64 = 0x53AB;
const SEEK_POSITION: u64 = 0x53AC;
const INFO: u64 = 0x1549_A966;
const TIMESTAMP_SCALE: u64 = 0x2AD7B1;
const DURATION: u64 = 0x4489;
const TRACKS: u64 = 0x1654_AE6B;
const TRACK_ENTRY: u64 = 0xAE;
const CUES: u64 = 0x1C53_BB6B;
const CLUSTER: u64 = 0x1F43_B675;

#[derive(Debug, Clone, Default)]
pub struct TrackInfo {
    pub number: u64,
    /// 1 video, 2 audio, 17 subtitle.
    pub kind: u64,
    pub codec: String,
    pub private: Vec<u8>,
    pub language: String,
    pub name: Option<String>,
    pub width: u64,
    pub height: u64,
    pub sample_rate: f64,
    pub channels: u64,
    /// Nanoseconds a frame lasts, when the track says.
    pub default_duration: Option<u64>,
    /// Nanoseconds of the codec's own delay, which every block's time includes: AAC's priming
    /// frame, Opus's pre-skip.
    pub codec_delay: u64,
}

/// A cue: a time in ticks, its track, and the absolute offset of the cluster holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cue {
    pub time: u64,
    pub track: u64,
    pub cluster: u64,
}

#[derive(Debug, Clone)]
pub struct Header {
    /// Nanoseconds a tick lasts.
    pub scale: u64,
    /// In ticks.
    pub duration: u64,
    pub tracks: Vec<TrackInfo>,
    pub cues: Vec<Cue>,
    /// Where the first cluster starts.
    pub first_cluster: u64,
}

/// One frame of a block, laced frames split apart.
#[derive(Debug, Clone)]
pub struct Block {
    pub track: u64,
    /// In ticks: the cluster's timestamp plus the block's.
    pub timestamp: i64,
    pub key: bool,
    pub data: Vec<u8>,
}

fn invalid(what: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_owned())
}

/// An EBML number and its length; for a size, whether it is "unknown" (all ones).
fn vint(reader: &mut impl Read, keep_marker: bool) -> std::io::Result<(u64, usize, bool)> {
    let mut first = [0u8];
    reader.read_exact(&mut first)?;
    let len = first[0].leading_zeros() as usize + 1;
    if len > 8 {
        return Err(invalid("a bad EBML number"));
    }
    // The marker bit cleared: for an eight-byte number the first byte holds no value bits.
    let mask = (0xffu16 >> len) as u8;
    let mut value = u64::from(if keep_marker { first[0] } else { first[0] & mask });
    for _ in 1..len {
        let mut byte = [0u8];
        reader.read_exact(&mut byte)?;
        value = (value << 8) | u64::from(byte[0]);
    }
    let unknown = !keep_marker && value == (1u64 << (7 * len)) - 1;
    Ok((value, len, unknown))
}

/// An element's id, body size and header length, and whether the size is unknown.
fn element(reader: &mut impl Read) -> std::io::Result<(u64, u64, usize, bool)> {
    let (id, a, _) = vint(reader, true)?;
    let (size, b, unknown) = vint(reader, false)?;
    Ok((id, size, a + b, unknown))
}

fn unsigned(data: &[u8]) -> u64 {
    data.iter().fold(0, |acc, b| (acc << 8) | u64::from(*b))
}

fn float(data: &[u8]) -> f64 {
    match data.len() {
        4 => f64::from(f32::from_be_bytes(data.try_into().unwrap_or_default())),
        8 => f64::from_be_bytes(data.try_into().unwrap_or_default()),
        _ => 0.0,
    }
}

fn text(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim_end_matches('\0').to_owned()
}

/// The children of a master element's body, as (id, body).
fn children(body: &[u8]) -> std::io::Result<Vec<(u64, &[u8])>> {
    let mut out = Vec::new();
    let mut cursor = std::io::Cursor::new(body);
    while (cursor.position() as usize) < body.len() {
        let (id, len, _, _) = element(&mut cursor)?;
        let start = cursor.position() as usize;
        let end = start.saturating_add(len as usize).min(body.len());
        out.push((id, &body[start..end]));
        cursor.set_position(end as u64);
    }
    Ok(out)
}

fn body(reader: &mut impl Read, size: u64) -> std::io::Result<Vec<u8>> {
    if size > 64 * 1024 * 1024 {
        return Err(invalid("an element too large to hold"));
    }
    let mut out = vec![0u8; size as usize];
    reader.read_exact(&mut out)?;
    Ok(out)
}

fn track(entry: &[u8]) -> std::io::Result<TrackInfo> {
    let mut track = TrackInfo { language: "eng".into(), ..TrackInfo::default() };
    for (id, value) in children(entry)? {
        match id {
            0xD7 => track.number = unsigned(value),
            0x83 => track.kind = unsigned(value),
            0x86 => track.codec = text(value),
            0x63A2 => track.private = value.to_vec(),
            0x22B59C => track.language = text(value),
            0x536E => track.name = Some(text(value)),
            0x23E383 => track.default_duration = Some(unsigned(value)),
            0x56AA => track.codec_delay = unsigned(value),
            0xE0 => {
                for (id, value) in children(value)? {
                    match id {
                        0xB0 => track.width = unsigned(value),
                        0xBA => track.height = unsigned(value),
                        _ => {}
                    }
                }
            }
            0xE1 => {
                track.channels = 1;
                for (id, value) in children(value)? {
                    match id {
                        0xB5 => track.sample_rate = float(value),
                        0x9F => track.channels = unsigned(value),
                        _ => {}
                    }
                }
            }
            0x6D80 => return Err(invalid("a track with content encodings, which is not read")),
            _ => {}
        }
    }
    Ok(track)
}

fn cues(body: &[u8], segment: u64) -> std::io::Result<Vec<Cue>> {
    let mut out = Vec::new();
    for (id, point) in children(body)? {
        if id != 0xBB {
            continue;
        }
        let mut time = 0;
        let mut positions = Vec::new();
        for (id, value) in children(point)? {
            match id {
                0xB3 => time = unsigned(value),
                0xB7 => {
                    let (mut track, mut cluster) = (0, 0);
                    for (id, field) in children(value)? {
                        match id {
                            0xF7 => track = unsigned(field),
                            0xF1 => cluster = unsigned(field),
                            _ => {}
                        }
                    }
                    positions.push((track, cluster));
                }
                _ => {}
            }
        }
        out.extend(positions.into_iter().map(|(track, cluster)| Cue {
            time,
            track,
            cluster: segment + cluster,
        }));
    }
    Ok(out)
}

/// Read the header, tracks and cues.
pub fn header<R: Read + Seek>(reader: &mut R) -> std::io::Result<Header> {
    let (id, size, _, _) = element(reader)?;
    if id != EBML {
        return Err(invalid("not EBML"));
    }
    reader.seek(SeekFrom::Current(size as i64))?;
    let (id, _, _, _) = element(reader)?;
    if id != SEGMENT {
        return Err(invalid("no Segment"));
    }
    let segment = reader.stream_position()?;
    let mut header = Header {
        scale: 1_000_000,
        duration: 0,
        tracks: Vec::new(),
        cues: Vec::new(),
        first_cluster: 0,
    };
    let mut cues_at = None;
    let mut at = segment;
    loop {
        reader.seek(SeekFrom::Start(at))?;
        let Ok((id, size, head, unknown)) = element(reader) else { break };
        match id {
            SEEK_HEAD => {
                for (id, seek) in children(&body(reader, size)?)? {
                    if id != SEEK {
                        continue;
                    }
                    let fields = children(seek)?;
                    let field =
                        |want| fields.iter().find(|(id, _)| *id == want).map(|f| unsigned(f.1));
                    if field(SEEK_ID) == Some(CUES) {
                        cues_at = field(SEEK_POSITION).map(|p| segment + p);
                    }
                }
            }
            INFO => {
                let mut duration = 0.0;
                for (id, value) in children(&body(reader, size)?)? {
                    match id {
                        TIMESTAMP_SCALE => header.scale = unsigned(value).max(1),
                        DURATION => duration = float(value),
                        _ => {}
                    }
                }
                header.duration = duration as u64;
            }
            TRACKS => {
                for (id, entry) in children(&body(reader, size)?)? {
                    if id == TRACK_ENTRY {
                        header.tracks.push(track(entry)?);
                    }
                }
            }
            CUES => header.cues = cues(&body(reader, size)?, segment)?,
            CLUSTER if header.first_cluster == 0 => {
                header.first_cluster = at;
                // The cues, from the SeekHead, are past the clusters: read them there.
                if let Some(cues_at) = cues_at {
                    reader.seek(SeekFrom::Start(cues_at))?;
                    let (id, size, _, _) = element(reader)?;
                    if id == CUES {
                        header.cues = cues(&body(reader, size)?, segment)?;
                    }
                }
                if !header.cues.is_empty() || unknown {
                    break;
                }
            }
            _ => {}
        }
        if unknown {
            break;
        }
        at += head as u64 + size;
    }
    if header.tracks.is_empty() {
        return Err(invalid("no tracks"));
    }
    Ok(header)
}

/// Every frame of the clusters from `cluster` on, in file order, until `take` says stop.
pub fn blocks<R: Read + Seek>(
    reader: &mut R,
    cluster: u64,
    mut take: impl FnMut(Block) -> bool,
) -> std::io::Result<()> {
    let mut at = cluster;
    loop {
        reader.seek(SeekFrom::Start(at))?;
        let Ok((id, size, head, unknown)) = element(reader) else { return Ok(()) };
        if id != CLUSTER {
            // The cues, tags or attachments after the last cluster: the end.
            return Ok(());
        }
        if unknown {
            return Err(invalid("a cluster of unknown size, as a live stream writes"));
        }
        let cluster = body(reader, size)?;
        let mut timestamp = 0i64;
        for (id, value) in children(&cluster)? {
            match id {
                0xE7 => timestamp = unsigned(value) as i64,
                0xA3 => {
                    for block in block(value, timestamp, None)? {
                        if !take(block) {
                            return Ok(());
                        }
                    }
                }
                0xA0 => {
                    let group = children(value)?;
                    let referenced = group.iter().any(|(id, _)| *id == 0xFB);
                    if let Some((_, data)) = group.iter().find(|(id, _)| *id == 0xA1) {
                        for block in block(data, timestamp, Some(!referenced))? {
                            if !take(block) {
                                return Ok(());
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        at += head as u64 + size;
    }
}

/// A SimpleBlock or Block's frames. `key` is the BlockGroup's word; a SimpleBlock says its own.
fn block(data: &[u8], cluster: i64, key: Option<bool>) -> std::io::Result<Vec<Block>> {
    let mut cursor = std::io::Cursor::new(data);
    let (track, _, _) = vint(&mut cursor, false)?;
    let at = cursor.position() as usize;
    let header = data.get(at..at + 3).ok_or_else(|| invalid("a short block"))?;
    let relative = i64::from(i16::from_be_bytes([header[0], header[1]]));
    let flags = header[2];
    let key = key.unwrap_or(flags & 0x80 != 0);
    let mut rest = &data[at + 3..];
    let lacing = (flags >> 1) & 3;
    let frames: Vec<&[u8]> = if lacing == 0 {
        vec![rest]
    } else {
        let (&count, tail) = rest.split_first().ok_or_else(|| invalid("a short lace"))?;
        rest = tail;
        let count = usize::from(count) + 1;
        let mut sizes = Vec::with_capacity(count);
        match lacing {
            1 => {
                for _ in 0..count - 1 {
                    let mut size = 0usize;
                    loop {
                        let (&byte, tail) =
                            rest.split_first().ok_or_else(|| invalid("a short lace"))?;
                        rest = tail;
                        size += usize::from(byte);
                        if byte != 255 {
                            break;
                        }
                    }
                    sizes.push(size);
                }
            }
            3 => {
                let mut cursor = std::io::Cursor::new(rest);
                let (first, _, _) = vint(&mut cursor, false)?;
                let mut size = first as i64;
                sizes.push(size as usize);
                for _ in 1..count - 1 {
                    let (raw, len, _) = vint(&mut cursor, false)?;
                    let bias = (1i64 << (7 * len - 1)) - 1;
                    size += raw as i64 - bias;
                    sizes.push(size.max(0) as usize);
                }
                rest = &rest[cursor.position() as usize..];
            }
            _ => {
                let each = rest.len() / count;
                sizes.extend(std::iter::repeat_n(each, count - 1));
            }
        }
        let mut frames = Vec::with_capacity(count);
        for size in sizes {
            let (frame, tail) = rest.split_at(size.min(rest.len()));
            frames.push(frame);
            rest = tail;
        }
        frames.push(rest);
        frames
    };
    Ok(frames
        .into_iter()
        .map(|frame| Block { track, timestamp: cluster + relative, key, data: frame.to_vec() })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xiph_laced_frames_split_apart() {
        // Track 1, timestamp 0, keyframe, Xiph lacing: three frames of 2, 300 and 1 bytes.
        let mut data = vec![0x81, 0, 0, 0x80 | 0x02, 2, 2, 255, 45];
        data.extend([1u8; 2]);
        data.extend([2u8; 300]);
        data.extend([3u8; 1]);
        let frames = block(&data, 10, None).unwrap();
        assert_eq!(frames.iter().map(|f| f.data.len()).collect::<Vec<_>>(), [2, 300, 1]);
        assert!(frames.iter().all(|f| f.key && f.timestamp == 10 && f.track == 1));
    }

    #[test]
    fn a_block_group_with_a_reference_is_not_a_keyframe() {
        let data = [0x82, 0, 5, 0, 9, 9];
        let frames = block(&data, 100, Some(false)).unwrap();
        assert_eq!((frames[0].timestamp, frames[0].key, frames[0].track), (105, false, 2));
    }
}
