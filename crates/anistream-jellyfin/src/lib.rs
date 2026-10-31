//! A client for a Jellyfin server the person runs.
//!
//! It signs in with a password or Quick Connect, lists libraries, series and episodes with the
//! ids other services know them by, builds the addresses that play, and reports what is watched
//! back. Only the access token is kept, by the caller, in the secure store. This crate does no
//! matching to AniList and keeps nothing: the engine does both.

pub mod address;
pub mod client;
pub mod discovery;
pub mod error;
pub mod model;
pub mod source;

pub use address::{Address, is_private_host};
pub use client::{ClientInfo, Jellyfin, MINIMUM_VERSION, version_supported};
pub use error::{JellyfinError, Result};
pub use source::{JellyfinSource, SeriesMatch, source_id};
