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
pub mod demo;
pub mod filler;
pub mod franchise;
pub mod source;
pub mod tenrai;
pub mod title;
pub mod titles;

pub use anilist::{
    AniList, AniListError, BrowseFilter, FacetTag, Facets, Media, Season, in_background,
};
pub use dataset::{MAPPING_DATASETS, RefreshOutcome, refresh_all};
pub use source::{Found, Meta};
pub use tenrai::Tenrai;
pub use title::{CONFIDENCE_FLOOR, MatchTarget, Scored, normalise, rank, similarity};
