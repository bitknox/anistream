//! The remote: a phone, or any other anistream app, finds an anistream TV on the local network,
//! pairs with it, and drives it.
//!
//! The TV runs a [`server::RemoteServer`] beside its engine; a phone holds a
//! [`client::RemoteClient`] for one connection, or a [`client::Link`] that reconnects while the
//! remote screen is open. Media never goes through the phone: a phone says what to play, and the
//! TV's engine finds the streams and plays them.
//!
//! - [`discovery`]: what a TV advertises over DNS-SD and how a phone reads it, behind traits the
//!   platform implements, and the pairing link a QR code carries when multicast is blocked.
//! - [`tls`]: TLS 1.3 with each side's raw Ed25519 key ([`identity`]).
//! - [`pairing`]: a six-digit code through SPAKE2, bound to both keys and the connection.
//! - [`trust`]: the phones a TV paired with, and how one is revoked.
//! - [`wire`]: frames and messages; [`command`]: what a phone can ask, as the engine's own
//!   commands; [`state`]: what a phone is told, read off the engine's own events.
//!
//! The protocol is written up in `docs/remote-protocol.md`.

pub mod client;
pub mod command;
pub mod discovery;
#[cfg(feature = "testing")]
pub mod fake;
pub mod identity;
pub mod known;
pub mod pairing;
pub mod server;
pub mod state;
pub mod tls;
pub mod trust;
pub mod wire;

use wire::{PairFailure, Rejection};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network: {0}")]
    Io(#[from] std::io::Error),
    #[error("TLS: {0}")]
    Tls(#[from] rustls::Error),
    #[error("a message could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Frame(&'static str),
    #[error("{0}")]
    Crypto(&'static str),
    #[error("the other side broke the protocol: {0}")]
    Protocol(&'static str),
    #[error("the TV refused: {0:?}")]
    Rejected(Rejection),
    #[error("pairing failed: {0:?}")]
    PairFailed(PairFailure),
    #[error(transparent)]
    Pair(#[from] pairing::PairError),
    #[error("the TV removed this phone")]
    Revoked,
    #[error("the connection closed")]
    Closed,
    #[error("not connected to the TV")]
    NotConnected,
    #[error("the TV did not answer in time")]
    Timeout,
    /// The TV answered with a key other than the one this phone paired with.
    #[error("the TV's key changed since it paired")]
    KeyChanged,
}

/// Now, in Unix seconds: when a phone paired and was last seen.
pub(crate) fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
