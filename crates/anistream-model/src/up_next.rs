//! "Up next for you": the group of suggestions under Continue.
//!
//! The engine decides which titles and in which order; a front end draws what it is sent. The
//! group never sits above the queue and never takes the selection.

use crate::catalog::{Entry, FuzzyDate};

/// Why a title is among the suggestions. The heading over the group is the same for all of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum UpNextReason {
    /// A sequel of a title the viewer finished, which they have not started.
    NewSeason,
    /// On the viewer's Planning list. Only when signed in.
    Planning,
    /// One of AniList's recommendations from a title the viewer finished.
    Recommended,
}

/// What the main button of a suggestion does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum UpNextAction {
    /// Play episode 1: it is out.
    PlayFirstEpisode,
    /// Add the title to Planning: it has not started airing.
    AddToPlanning,
    /// Neither: it has not started airing and is on Planning already.
    Nothing,
}

/// One suggestion.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpNextItem {
    pub entry: Entry,
    pub reason: UpNextReason,
    /// For [`UpNextReason::Recommended`], the finished title it was recommended from.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub because: Option<String>,
    /// When it starts airing, for a title that has not: "Starts 9 January". `None` once it airs.
    /// The year alone when AniList knows no more.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starts: Option<FuzzyDate>,
    pub action: UpNextAction,
}
