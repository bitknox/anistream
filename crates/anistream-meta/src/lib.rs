//! The metadata spine.
//!
//! AniList is the canonical identity for every title, the mapping datasets translate that
//! identity into whatever ids external services insist on, and [`title`] matches by text
//! for the sources that have no shared id at all.
//!
//! This layer is deliberately the *stable* half of the application. Streaming sources decay
//! constantly; AniList, the mapping corpora and the curation source do not. Keeping identity
//! here means a dead source can be replaced without re-deriving what anything is.

pub mod anilist;
pub mod dataset;
pub mod filler;
pub mod title;

pub use anilist::{AniList, AniListError, BrowseFilter, Media, Season};
pub use dataset::{MAPPING_DATASETS, RefreshOutcome, refresh_all};
pub use title::{CONFIDENCE_FLOOR, MatchTarget, Scored, normalise, rank, similarity};
