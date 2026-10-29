//! Matroska files made up for tests: a video track of stand-in frames at 25 a second with a
//! keyframe every two seconds, one cluster each, and an audio track of stand-in frames, with cues
//! at the end and a SeekHead naming them, as mkvmerge lays a file out. Nothing in them decodes;
//! every frame's bytes name its track and time, so a test can read back what a segment holds.

/// What to make.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub seconds: u32,
    /// `V_MPEG4/ISO/AVC` or another codec id.
    pub video: &'static str,
    pub audio: Option<FixtureAudio>,
    /// Without cues, the remuxer can't plan segments.
    pub cues: bool,
    /// The picture's size the track header says.
    pub width: u64,
    pub height: u64,
}

#[derive(Debug, Clone)]
pub struct FixtureAudio {
    /// `A_AAC`, `A_AC3`, `A_OPUS`.
    pub codec: &'static str,
    pub channels: u8,
    /// One frame's length.
    pub frame_ms: u32,
    pub private: Vec<u8>,
    pub language: &'static str,
}

impl Default for Fixture {
    fn default() -> Self {
        Self {
            seconds: 20,
            video: "V_MPEG4/ISO/AVC",
            audio: Some(FixtureAudio::ac3()),
            cues: true,
            width: 640,
            height: 360,
        }
    }
}

impl FixtureAudio {
    /// AC-3 5.1: 1536 samples at 48 kHz a frame.
    pub fn ac3() -> Self {
        Self { codec: "A_AC3", channels: 6, frame_ms: 32, private: Vec::new(), language: "jpn" }
    }

    /// AAC-LC stereo at 48 kHz.
    pub fn aac() -> Self {
        Self {
            codec: "A_AAC",
            channels: 2,
            frame_ms: 21,
            private: vec![0x11, 0x90],
            language: "jpn",
        }
    }
}

/// An H.264 High profile `avcC`, level 4.0, with a stand-in SPS and PPS.
const AVCC: [u8; 19] = [
    1, 0x64, 0x00, 0x28, 0xFF, 0xE1, 0, 4, 0x67, 0x64, 0, 0x28, 1, 0, 4, 0x68, 0xEE, 0x3C, 0x80,
];

fn id_bytes(id: u32) -> Vec<u8> {
    let bytes = id.to_be_bytes();
    let skip = bytes.iter().take_while(|b| **b == 0).count();
    bytes[skip..].to_vec()
}

/// An element: its id, an eight-byte size, its body.
fn element(id: u32, body: &[u8]) -> Vec<u8> {
    let mut out = id_bytes(id);
    let size = body.len() as u64;
    out.push(0x01);
    out.extend(&size.to_be_bytes()[1..]);
    out.extend_from_slice(body);
    out
}

fn uint(id: u32, value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

fn float(id: u32, value: f64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

fn text(id: u32, value: &str) -> Vec<u8> {
    element(id, value.as_bytes())
}

/// One frame's stand-in bytes: its track's letter and its time in milliseconds, padded.
pub fn frame(track: u8, ms: u32, len: usize) -> Vec<u8> {
    let mut data = format!("{}@{ms};", if track == 1 { 'v' } else { 'a' }).into_bytes();
    data.resize(len, 0x55);
    data
}

fn simple_block(track: u8, relative: i16, key: bool, data: &[u8]) -> Vec<u8> {
    let mut body = vec![0x80 | track];
    body.extend(relative.to_be_bytes());
    body.push(if key { 0x80 } else { 0 });
    body.extend_from_slice(data);
    element(0xA3, &body)
}

/// The file `fixture` describes.
pub fn matroska(fixture: &Fixture) -> Vec<u8> {
    let ebml = element(0x1A45_DFA3, &[text(0x4282, "matroska"), uint(0x4287, 4)].concat());
    let total_ms = fixture.seconds * 1000;
    let info = element(
        0x1549_A966,
        &[uint(0x2A_D7B1, 1_000_000), float(0x4489, f64::from(total_ms))].concat(),
    );
    let mut tracks = element(
        0xAE,
        &[
            uint(0xD7, 1),
            uint(0x73C5, 1),
            uint(0x83, 1),
            text(0x86, fixture.video),
            element(0x63A2, &AVCC),
            uint(0x23_E383, 40_000_000),
            element(0xE0, &[uint(0xB0, fixture.width), uint(0xBA, fixture.height)].concat()),
        ]
        .concat(),
    );
    if let Some(audio) = &fixture.audio {
        let mut entry = vec![
            uint(0xD7, 2),
            uint(0x73C5, 2),
            uint(0x83, 2),
            text(0x86, audio.codec),
            text(0x22_B59C, audio.language),
            uint(0x23_E383, u64::from(audio.frame_ms) * 1_000_000),
            element(
                0xE1,
                &[float(0xB5, 48_000.0), uint(0x9F, u64::from(audio.channels))].concat(),
            ),
        ];
        if !audio.private.is_empty() {
            entry.push(element(0x63A2, &audio.private));
        }
        tracks.extend(element(0xAE, &entry.concat()));
    }
    let tracks = element(0x1654_AE6B, &tracks);

    // A cluster every two seconds, frames interleaved by time.
    let mut clusters = Vec::new();
    let mut starts = Vec::new();
    for cluster in 0..total_ms.div_ceil(2000) {
        let at = cluster * 2000;
        let end = (at + 2000).min(total_ms);
        let mut blocks: Vec<(u32, u8, Vec<u8>)> = Vec::new();
        for ms in (at..end).step_by(40) {
            blocks.push((
                ms,
                1,
                simple_block(
                    1,
                    i16::try_from(ms - at).unwrap_or(0),
                    ms == at,
                    &frame(1, ms, 200),
                ),
            ));
        }
        if let Some(audio) = &fixture.audio {
            let first = at.div_ceil(audio.frame_ms) * audio.frame_ms;
            for ms in (first..end).step_by(audio.frame_ms as usize) {
                blocks.push((
                    ms,
                    2,
                    simple_block(
                        2,
                        i16::try_from(ms - at).unwrap_or(0),
                        true,
                        &frame(2, ms, 60),
                    ),
                ));
            }
        }
        blocks.sort_by_key(|(ms, track, _)| (*ms, *track));
        let mut body = uint(0xE7, u64::from(at));
        for (_, _, block) in blocks {
            body.extend(block);
        }
        starts.push((at, clusters.len()));
        clusters.extend(element(0x1F43_B675, &body));
    }

    let seek_head = |cues_at: u64| {
        element(
            0x114D_9B74,
            &element(
                0x4DBB,
                &[element(0x53AB, &id_bytes(0x1C53_BB6B)), uint(0x53AC, cues_at)].concat(),
            ),
        )
    };
    let seek_len = if fixture.cues { seek_head(0).len() } else { 0 };
    let head_len = seek_len + info.len() + tracks.len();
    let cues_at = (head_len + clusters.len()) as u64;
    let mut body = if fixture.cues { seek_head(cues_at) } else { Vec::new() };
    body.extend(&info);
    body.extend(&tracks);
    body.extend(&clusters);
    if fixture.cues {
        let mut points = Vec::new();
        for (at, offset) in starts {
            points.extend(element(
                0xBB,
                &[
                    uint(0xB3, u64::from(at)),
                    element(
                        0xB7,
                        &[uint(0xF7, 1), uint(0xF1, (head_len + offset) as u64)].concat(),
                    ),
                ]
                .concat(),
            ));
        }
        body.extend(element(0x1C53_BB6B, &points));
    }
    [ebml, element(0x1853_8067, &body)].concat()
}
