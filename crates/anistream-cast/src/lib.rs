//! Casting to TVs with Chromecast built in: the Cast v2 channel, the codec plans and what each TV
//! is known to play, the LAN relay the TV fetches from, Matroska to HLS with AAC audio, and ASS
//! to WebVTT.
//!
//! - [`proto`], [`channel`] and [`client`]: the Cast v2 channel (TLS on port 8009, framed
//!   protobuf), the Default Media Receiver, `LOAD`, the controls and `MEDIA_STATUS`.
//! - [`device`]: a receiver as its DNS-SD record describes it, and where to serve it from.
//! - [`formats`] and [`memory`]: the plans a stream can be cast by, best first, and what each TV
//!   played or refused, remembered in `cast-tvs.json`.
//! - [`probe`]: what a stream holds (its codecs and container) and whether the TV may fetch it
//!   by itself.
//! - [`fetch`] and [`relay`]: the streamed upstream, and the relay on the interface that faces
//!   the TV, for that TV alone.
//! - [`mkv`], [`remux`] and [`aac`]: Matroska to HLS with fragmented MP4 segments, the video
//!   copied and the audio copied or re-encoded to AAC by the platform's encoder.
//! - [`vtt`]: ASS and SRT to WebVTT, dialogue kept and typesetting dropped.
//! - [`player`]: a TV as the player a playback runs in, in the session's own words.
//! - `fake` (tests, and the `fake` feature): a Cast receiver on loopback.

pub mod aac;
pub mod channel;
pub mod client;
pub mod device;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
pub mod fetch;
pub mod formats;
pub mod memory;
pub mod mkv;
pub mod player;
pub mod probe;
pub mod proto;
pub mod relay;
pub mod remux;
pub mod vtt;

pub use aac::{
    AacEncoder, AacOutput, AacStream, AacUnit, AudioTrackInfo, EncodeError, NoEncoder,
};
pub use channel::{CastError, Channel, Incoming};
pub use client::{
    App, DEFAULT_MEDIA_RECEIVER, Device, Load, Media, MediaStatus, Metadata, ReceiverStatus,
    TextStyle, TextTrack, VolumeControl,
};
pub use device::{CastDevice, local_address_towards, same_network};
pub use fetch::{PlainHttp, Upstream, UpstreamResponse};
pub use formats::{
    AudioCodec, AudioPlan, CastPlan, CastRoute, CastSource, Delivery, MediaInfo, SourceKind,
    Uncastable, VideoFormat, cast_plans,
};
pub use memory::{CastCaps, CastMemory, FormatAnswer};
pub use player::{CastFailure, CastLoad, CastPlayer, CastSetup, CastSubtitle, Timings};
pub use probe::CorsProbe;
pub use relay::Relay;
