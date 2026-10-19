//! The catalogue: titles as the lists show them, the lists themselves, and which view asked for
//! each.

use anistream_core::ids::AnilistId;

/// A title as the list views need it.
///
/// A flattened projection rather than the full `Media`, so rendering does not depend on the
/// metadata crate and can be exercised without one.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Entry {
    pub id: AnilistId,
    pub title: String,
    pub secondary: Option<String>,
    pub format: Option<String>,
    pub episodes: Option<u32>,
    pub year: Option<u16>,
    pub score: Option<u32>,
    pub studio: Option<String>,
    pub synopsis: String,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    pub genres: Vec<String>,
    /// Services carrying this title, from AniList's external links.
    pub available_on: Vec<String>,
    /// Local progress: episodes completed, and the next episode to watch.
    pub progress: Option<(u32, u32)>,
    /// Seconds until *this row's* broadcast — which for a calendar entry is the episode the row
    /// is about, and runs negative once that episode has aired. Everywhere else it is the same
    /// thing as [`Self::next_airing_in`].
    pub airing_in: Option<i64>,
    /// The episode a calendar row is about — the one [`Self::airing_in`] dates. `None`
    /// everywhere else.
    ///
    /// A field of its own rather than `ep 12` appended to the title. Appended, it was the first
    /// thing truncation took, so a column of one show's episodes read as identical rows, and it
    /// followed the title into the Title and Episodes headings.
    pub airing_episode: Option<u32>,
    /// The episode the title broadcasts next, from its own schedule.
    pub next_episode: Option<u32>,
    /// Seconds until [`Self::next_episode`] airs.
    ///
    /// Separate from [`Self::airing_in`] because a calendar row overwrites that with its own
    /// slot, and the two then describe different episodes — which is how a row about an episode
    /// that had just aired came to read "EP 6 in now". This one is the title's schedule and
    /// nothing else's, so the pair is always about the same episode.
    pub next_airing_in: Option<i64>,
    /// The most recent broadcast: episode number and how many seconds ago.
    ///
    /// Answers the question a list of airing shows is actually asked — *is there something new
    /// to watch* — which neither the total episode count nor the next countdown can.
    pub last_aired: Option<(u32, i64)>,
    /// An episode left part-watched, and where in it you stopped.
    ///
    /// Deliberately independent of the commit threshold. That threshold governs when an episode is
    /// *counted* — it exists so opening one to check the subtitles does not push progress to a
    /// tracker. Picking up where you left off is a different question with a different answer:
    /// quitting halfway through an episode you fully intend to finish should be the single most
    /// visible thing on the home screen, and under the old behaviour it was invisible.
    pub resume: Option<ResumePoint>,
    /// Titles adjacent in the main watch order — prequels, the parent story, sequels.
    /// Present on the detail fetch only; list fetches leave it empty.
    pub related: Vec<RelatedTitle>,
    /// What people who liked this rated highest. Rides the detail fetch, like `related`.
    pub recommended: Vec<RelatedTitle>,
    /// A watchable trailer URL, when AniList has one. Rides the detail fetch.
    pub trailer_url: Option<String>,
    /// Where the title sits on the tracker's list, when that is known: a Library row knows its
    /// segment, and a status set in this session is remembered on the entry. `None` means not
    /// known, not "on no list" — nothing outside the Library carries the tracker's answer.
    pub list_status: Option<LibrarySegment>,
}

/// One step of a title's watch order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RelatedTitle {
    pub id: AnilistId,
    pub title: String,
    /// `prequel`, `parent` or `sequel`, already lowercased for display.
    pub relation: String,
    pub format: Option<String>,
}

/// Where an unfinished episode was left.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ResumePoint {
    /// Seconds into the episode.
    pub position: f64,
    /// Fraction of the episode watched, when the runtime is known.
    pub fraction: Option<f64>,
}

impl ResumePoint {
    /// `12:30` — the position as a viewer thinks of it.
    pub fn clock(&self) -> String {
        let total = self.position.max(0.0) as u64;
        let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}

impl Entry {
    pub fn new(id: AnilistId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            secondary: None,
            format: None,
            episodes: None,
            year: None,
            score: None,
            studio: None,
            synopsis: String::new(),
            cover_url: None,
            banner_url: None,
            genres: Vec::new(),
            available_on: Vec::new(),
            progress: None,
            airing_in: None,
            airing_episode: None,
            next_episode: None,
            next_airing_in: None,
            last_aired: None,
            resume: None,
            related: Vec::new(),
            recommended: Vec::new(),
            trailer_url: None,
            list_status: None,
        }
    }

    /// What progress is counted against, and whether that is the season's length. AniList
    /// leaves the length empty while a show airs without an announced one, and counting against
    /// nothing read as "2 / 0"; the newest episode out is the honest stand-in until it is known.
    pub fn progress_of(&self) -> Option<(u32, bool)> {
        match self.episodes.filter(|n| *n > 0) {
            Some(total) => Some((total, true)),
            None => self.latest_aired().map(|(episode, _)| (episode, false)),
        }
    }

    /// Fraction watched, for the progress meter.
    pub fn watched_fraction(&self) -> f64 {
        match (self.progress, self.progress_of()) {
            (Some((done, _)), Some((of, _))) => {
                (f64::from(done) / f64::from(of)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// The count beside the meter: `2 / 12`, `2 / 3 aired` while the length is unannounced, or
    /// `2 watched` when there is nothing to count against.
    pub fn progress_label(&self) -> Option<String> {
        let (done, _) = self.progress?;
        Some(match self.progress_of() {
            Some((total, true)) => format!("{done} / {total}"),
            Some((aired, false)) => format!("{done} / {aired} aired"),
            None => format!("{done} watched"),
        })
    }

    /// The newest episode known to be out, with how long ago when that is known too.
    ///
    /// Reconciles two sources that disagree for a stretch right after a broadcast: the
    /// last-aired schedule row lags AniList's own `nextAiringEpisode`, which can already
    /// hold episode N+1 while the schedule query still answers N-1. Holding N+1 as *next*
    /// proves N is out, so the larger claim wins — this is what stops a freshly-launched
    /// episode from being named only by its successor. The "ago" is carried only when it
    /// belongs to the episode being named, absent rather than guessed otherwise.
    pub fn latest_aired(&self) -> Option<(u32, Option<i64>)> {
        let derived = self.next_episode.and_then(|n| n.checked_sub(1)).filter(|n| *n >= 1);
        match (self.last_aired, derived) {
            (Some((episode, _)), Some(proven)) if proven > episode => Some((proven, None)),
            (Some((episode, ago)), _) => Some((episode, Some(ago))),
            (None, proven) => proven.map(|episode| (episode, None)),
        }
    }
}

/// One segment of the Library screen.
///
/// These are the tracker's own statuses rather than something local, because the Library screen
/// *is* a view of the tracker's list — the local equivalent is the `CONTINUE` rail on Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LibrarySegment {
    #[default]
    Watching,
    Planning,
    Completed,
    Paused,
    Dropped,
}

impl LibrarySegment {
    pub const ALL: [Self; 5] =
        [Self::Watching, Self::Planning, Self::Completed, Self::Paused, Self::Dropped];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::Planning => "planning",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Dropped => "dropped",
        }
    }

    /// The tracker-side status string this segment corresponds to.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Watching => "CURRENT",
            Self::Planning => "PLANNING",
            Self::Completed => "COMPLETED",
            Self::Paused => "PAUSED",
            Self::Dropped => "DROPPED",
        }
    }

    /// The neighbouring segment, stopping at both ends.
    ///
    /// Clamps rather than wraps, the same rule as the rail, every list and every value ladder:
    /// the ends are informative, and the left end is where Left carries on to the rail.
    pub fn step(self, delta: isize) -> Self {
        let all = Self::ALL;
        let current = all.iter().position(|s| *s == self).unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, all.len() as isize - 1) as usize;
        all[next]
    }
}

/// What the stage is currently showing.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Content {
    #[default]
    Empty,
    Loading,
    Entries(Vec<Entry>),
    /// A per-provider or network failure, shown in place of content.
    ///
    /// Never an empty list with no explanation: when something breaks the user must be
    /// able to see *what*.
    Failed(String),
}

impl Content {
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Entries(e) => e,
            _ => &[],
        }
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Which list an [`Event::Content`](crate::event::Event::Content) answers.
///
/// Requests are not cancelled when the user moves on, and the AniList rate limiter spaces them
/// unevenly, so the answer for a section just left can land after the switch. Untagged, it was
/// shown under the new section's heading until that section's own answer replaced it. The reducer
/// takes a list only while the view it was fetched for is still the one on screen.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContentFor {
    Continue,
    Seasonal,
    Calendar,
    /// The query and filters as submitted, so a slow answer to an earlier search cannot replace
    /// the results of a later one.
    Search {
        query: String,
        filter: SearchFilters,
    },
    Library(LibrarySegment),
}

/// Server-side search filters, set from the FILTERS overlay on the Search screen.
///
/// Wire values rather than display values — what AniList's enums expect, lowercased only
/// when a row is drawn. `None` reads as "any" and is omitted from the query entirely,
/// because AniList treats an explicit null differently from a missing argument.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SearchFilters {
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub season: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    /// `None` is relevance — AniList's `SEARCH_MATCH` — which is only meaningful with a
    /// search term, so the executor substitutes popularity for a filter-only browse.
    pub sort: Option<String>,
}

impl SearchFilters {
    pub fn is_empty(&self) -> bool {
        self.genre.is_none()
            && self.year.is_none()
            && self.season.is_none()
            && self.format.is_none()
            && self.status.is_none()
            && self.sort.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_segments_step_both_ways_and_stop_at_the_ends() {
        // The same rule as the rail and every list: the ends clamp rather than wrap.
        assert_eq!(LibrarySegment::Watching.step(1), LibrarySegment::Planning);
        assert_eq!(LibrarySegment::Planning.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Watching.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Dropped.step(1), LibrarySegment::Dropped);
    }

    #[test]
    fn the_segment_wire_names_are_the_trackers_own() {
        // These strings are matched against what AniList returns, so a rename here would
        // silently empty a segment.
        assert_eq!(LibrarySegment::Watching.wire(), "CURRENT");
        assert_eq!(LibrarySegment::Completed.wire(), "COMPLETED");
    }

    #[test]
    fn watched_fraction_is_bounded_and_safe_without_data() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        assert_eq!(e.watched_fraction(), 0.0);

        e.episodes = Some(28);
        e.progress = Some((14, 15));
        assert!((e.watched_fraction() - 0.5).abs() < 1e-9);

        // More progress than episodes (a rewatch, or bad remote data) must not exceed 1.
        e.progress = Some((99, 100));
        assert_eq!(e.watched_fraction(), 1.0);

        e.episodes = Some(0);
        assert_eq!(e.watched_fraction(), 0.0, "must not divide by zero");
    }

    #[test]
    fn progress_counts_against_what_is_known() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        e.progress = Some((2, 3));
        assert_eq!(
            e.progress_label().as_deref(),
            Some("2 watched"),
            "nothing to count against"
        );
        assert_eq!(e.watched_fraction(), 0.0);

        // Airing, with no announced length: episode 3 next means 2 are out.
        e.next_episode = Some(3);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 2 aired"));
        assert_eq!(e.watched_fraction(), 1.0, "caught up is a full meter, not an empty one");

        e.episodes = Some(12);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 12"));
        assert!((e.watched_fraction() - 2.0 / 12.0).abs() < 1e-9);
    }

    #[test]
    fn the_next_episode_number_proves_its_predecessor_aired() {
        // Right after a broadcast AniList's `nextAiringEpisode` can already say 9 while
        // the last-aired schedule row still answers 7 — during that window the app named
        // the fresh episode only by its successor. Episode 8 is out, provably.
        let mut e = Entry::new(AnilistId::new(1), "Frieren");
        e.last_aired = Some((7, 7 * 24 * 3600));
        e.next_episode = Some(9);
        assert_eq!(e.latest_aired(), Some((8, None)), "the stale schedule row won");

        // When the sources agree, the schedule row's own timestamp is kept.
        e.last_aired = Some((8, 3600));
        assert_eq!(e.latest_aired(), Some((8, Some(3600))));

        // A premiere still ahead proves nothing aired.
        let mut fresh = Entry::new(AnilistId::new(2), "Not Yet Out");
        fresh.next_episode = Some(1);
        assert_eq!(fresh.latest_aired(), None);
    }
}
