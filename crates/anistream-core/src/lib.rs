//! Domain types, traits and configuration shared by every anistream crate.
//!
//! The architecture layers outward from here in increasing order of volatility:
//! metadata (AniList) is stable, ID mapping is stable glue, video providers are
//! hostile and replaceable, and the player is stable again. This crate owns the
//! vocabulary all of those layers speak, and depends on none of them.

pub mod config;
pub mod error;
pub mod ids;
pub mod media;
pub mod settings;
pub mod stream;
pub mod traits;

pub use error::{Error, ProviderError, Result};
pub use ids::{AnilistId, KitsuId, MalId, ProviderKey, TvdbId};
pub use media::{Episode, EpisodeNumber, MediaFormat, MediaStatus, SearchHit, Translation};
pub use stream::{Stream, StreamKind, Subtitle};
pub use traits::{Player, Provider, ProviderKind, ProviderManifest, Tracker};
