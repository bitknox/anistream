//! "Up next for you": the short group of suggestions under a short Continue queue.
//!
//! The engine decides which titles, in which order, and how many; a front end draws what it is
//! sent. The group never sits above the queue and never takes the selection.

use crate::catalog::{Entry, FuzzyDate};

/// The most suggestions there are room for beside `queue_rows` rows of Continue: eight less two
/// for each row, at most six, and none at four rows or more.
pub fn room_for(queue_rows: usize) -> usize {
    8usize.saturating_sub(2 * queue_rows).min(MOST)
}

/// The most suggestions ever sent, with a queue of one row.
pub const MOST: usize = 6;

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
    /// Airing today, when nothing personal is left to suggest.
    AiringToday,
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
    /// For [`UpNextReason::AiringToday`], when today's episode airs, in Unix seconds. A front end
    /// shows it in the viewer's zone.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airs_at: Option<i64>,
    /// The group is the fallback for a viewer with nothing personal to be suggested: every item
    /// of it is airing today, and the heading over it reads "Airing today", not "Up next for
    /// you".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fallback: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn room_shrinks_by_two_per_queue_row_and_is_gone_at_four() {
        let room: Vec<_> = (0..=6).map(room_for).collect();
        assert_eq!(room, [6, 6, 4, 2, 0, 0, 0]);
    }
}
