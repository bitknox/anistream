//! Casting spike: what it takes for anistream to play on a Chromecast.
//!
//! - [`discovery`]: one mDNS question for `_googlecast._tcp`.
//! - [`proto`] and [`channel`]: the Cast v2 channel, TLS on port 8009 with framed protobuf.
//! - [`client`]: the Default Media Receiver, `LOAD`, the controls and `MEDIA_STATUS`.
//! - [`serve`]: the LAN proxy the device fetches from, with a per-cast token.
//! - [`vtt`]: ASS to WebVTT, dialogue kept and typesetting dropped.
//! - [`remux`]: Matroska to HLS with fragmented MP4 segments, the video copied as it is.
//! - [`link`]: a cast device as the session's player, in the session's own words.
//! - `fake` (tests, and the `fake` feature): a Cast device on loopback.
//!
//! Nothing links this crate yet; see the README beside it for what was measured.

pub mod channel;
pub mod client;
pub mod discovery;
#[cfg(any(test, feature = "fake"))]
pub mod fake;
pub mod link;
pub mod mkv;
pub mod proto;
pub mod remux;
pub mod serve;
pub mod vtt;

pub use channel::{CastError, Channel, Incoming};
pub use client::{App, DEFAULT_MEDIA_RECEIVER, Device, Load, Media, MediaStatus, TextTrack};
pub use discovery::{CastDevice, discover, local_address_towards};
pub use serve::{LanServer, Options};
