//! The platform's AAC encoder, and the decoder in front of it, which the remuxer calls for audio
//! a plan turns to AAC: AC-3, E-AC-3, DTS and TrueHD always, Opus and FLAC on a TV that refused
//! them. Each front end hands the engine its own at start (AudioToolbox on Apple platforms,
//! libavcodec and MediaCodec on Android, Media Foundation on Windows, libavcodec and FFmpeg's
//! native `aac` on Linux and the terminal); a build that hands none casts that audio nowhere.
//!
//! Not `libfdk-aac`: its licence isn't free software.

use crate::formats::AudioCodec;

/// AAC-LC at 48 kHz, as every plan that re-encodes writes it.
pub const AAC_RATE: u32 = 48_000;

/// Samples in one AAC-LC access unit.
pub const AAC_FRAME: u32 = 1024;

/// What the encoder writes for one source track: stereo at 192 kbit/s, or 5.1 at 384 kbit/s
/// where the source has six channels or more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AacOutput {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
}

impl AacOutput {
    pub fn for_channels(source_channels: u16) -> Self {
        if source_channels >= 6 {
            Self { sample_rate: AAC_RATE, channels: 6, bitrate: 384_000 }
        } else {
            Self { sample_rate: AAC_RATE, channels: 2, bitrate: 192_000 }
        }
    }

    /// The AudioSpecificConfig the `esds` of the init segment carries: AAC-LC, 48 kHz, this many
    /// channels (configuration 6 is 5.1).
    pub fn audio_specific_config(&self) -> Vec<u8> {
        const RATES: [u32; 13] = [
            96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000,
            7350,
        ];
        let index = RATES.iter().position(|&r| r == self.sample_rate).unwrap_or(3);
        let index = u16::try_from(index).unwrap_or(3);
        let config = if self.channels >= 6 { 6 } else { self.channels.min(2) };
        let packed: u16 = (2 << 11) | (index << 7) | (config << 3);
        packed.to_be_bytes().to_vec()
    }
}

/// One source audio track, as the encoder is opened for it.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioTrackInfo {
    pub codec: AudioCodec,
    /// The track's `CodecPrivate`, as Matroska stores it: what a decoder is configured from.
    pub codec_private: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
    /// What to write.
    pub output: AacOutput,
}

/// One AAC access unit, with its presentation time in seconds of the source.
#[derive(Debug, Clone, PartialEq)]
pub struct AacUnit {
    pub data: Vec<u8>,
    pub pts: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EncodeError(pub String);

/// The platform's decoder and AAC encoder.
pub trait AacEncoder: Send + Sync {
    /// Whether this platform decodes `codec` for the encoder.
    fn can_decode(&self, codec: &AudioCodec) -> bool;
    fn open(&self, track: &AudioTrackInfo) -> Result<Box<dyn AacStream>, EncodeError>;
}

/// One running encode.
pub trait AacStream: Send {
    /// Source frames in, AAC access units out, with their presentation times.
    fn push(&mut self, frame: &[u8], pts: f64) -> Result<Vec<AacUnit>, EncodeError>;
    fn flush(&mut self) -> Result<Vec<AacUnit>, EncodeError>;
}

/// The encoder of a build that has none: nothing is decoded, so audio that needs AAC is
/// uncastable.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEncoder;

impl AacEncoder for NoEncoder {
    fn can_decode(&self, _codec: &AudioCodec) -> bool {
        false
    }

    fn open(&self, _track: &AudioTrackInfo) -> Result<Box<dyn AacStream>, EncodeError> {
        Err(EncodeError("this build has no AAC encoder".into()))
    }
}

/// An encoder for tests: it "decodes" anything and writes a stand-in access unit for every 1024
/// samples at 48 kHz of source time, after two units of priming before the first frame, as a
/// real encoder's delay puts them. Each unit's bytes name its time in milliseconds, so a test can
/// read back what landed in a segment.
#[cfg(any(test, feature = "fake"))]
#[derive(Debug, Default, Clone)]
pub struct FakeEncoder {
    /// Every encoder opened, counted, for a test that checks a seek started a new one.
    pub opened: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

#[cfg(any(test, feature = "fake"))]
impl AacEncoder for FakeEncoder {
    fn can_decode(&self, codec: &AudioCodec) -> bool {
        !matches!(codec, AudioCodec::Other(_))
    }

    fn open(&self, _track: &AudioTrackInfo) -> Result<Box<dyn AacStream>, EncodeError> {
        self.opened.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Box::new(FakeStream { next: None }))
    }
}

#[cfg(any(test, feature = "fake"))]
struct FakeStream {
    /// The time of the next unit to write.
    next: Option<f64>,
}

#[cfg(any(test, feature = "fake"))]
impl FakeStream {
    fn unit(pts: f64) -> AacUnit {
        // Rounded, not cut: 21.333 ms steps land a hair under the millisecond.
        let ms = (pts * 1000.0).round();
        AacUnit { data: format!("aac@{ms}").into_bytes(), pts }
    }
}

#[cfg(any(test, feature = "fake"))]
impl AacStream for FakeStream {
    fn push(&mut self, _frame: &[u8], pts: f64) -> Result<Vec<AacUnit>, EncodeError> {
        let step = f64::from(AAC_FRAME) / f64::from(AAC_RATE);
        // Priming: two units ahead of the first frame.
        let mut at = *self.next.get_or_insert(pts - 2.0 * step);
        let mut out = Vec::new();
        // Units up to the frame's time; the frame's own samples come out with the next push,
        // as an encoder's delay holds them.
        while at < pts - 1e-9 {
            out.push(Self::unit(at));
            at += step;
        }
        self.next = Some(at);
        Ok(out)
    }

    fn flush(&mut self) -> Result<Vec<AacUnit>, EncodeError> {
        let step = f64::from(AAC_FRAME) / f64::from(AAC_RATE);
        Ok(self
            .next
            .take()
            .map(|at| vec![Self::unit(at), Self::unit(at + step)])
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_config_describes_aac_lc_at_48_khz() {
        assert_eq!(AacOutput::for_channels(2).audio_specific_config(), [0x11, 0x90]);
        let surround = AacOutput::for_channels(6);
        assert_eq!((surround.channels, surround.bitrate), (6, 384_000));
        assert_eq!(surround.audio_specific_config(), [0x11, 0xb0]);
        assert_eq!(AacOutput::for_channels(1).channels, 2, "mono is written as stereo");
    }
}
