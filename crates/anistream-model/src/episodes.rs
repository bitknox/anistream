//! A title's episodes, as its episode table and the player's Episodes menu list them.

/// One row of the timing-sheet episode table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeRow {
    pub number: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    /// Still frame for this episode, when one was published.
    ///
    /// Only ever drawn for the selected row: the table is dense, and one image that follows the
    /// cursor costs a single fetch per move rather than one per visible row.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub thumbnail: Option<String>,
    /// One-paragraph synopsis, when the source has one.
    ///
    /// Shown for the selected row only, under its still, for the same reason the still is:
    /// the table is a timing sheet, and a paragraph per row would drown it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub description: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub duration_secs: Option<u64>,
    /// How far through this episode the local history says we are.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: f64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub completed: bool,
    /// Filler, where the source that listed the episode says so.
    ///
    /// `None` means unknown, which is the common case: most shows have no filler, and most sources
    /// say nothing either way. Rendered only when present, so such a show shows no column rather
    /// than a column of blanks.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub kind: Option<EpisodeKind>,
    /// Whether offering to skip this whole episode is reasonable.
    ///
    /// Only pure filler. `mixed` episodes contain filler *and* canon, so skipping one loses story —
    /// the distinction is the whole reason this is a separate field rather than `kind == "filler"`
    /// at every call site.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub skippable: bool,
    /// When the episode airs or aired, in Unix seconds, from AniList's schedule. Later than now
    /// means it has not aired yet. `None` when the schedule does not say, which is not the same
    /// as unaired: a long runner's oldest episodes, and titles AniList keeps no schedule for.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airs_at: Option<i64>,
    /// When the episode was to air before AniList's schedule moved it later, in Unix seconds:
    /// "Delayed (was 23:00)". Known only when this device saw the schedule before the move;
    /// `None` when it has not moved since it was first seen here.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub delayed_from: Option<i64>,
    /// Listed from AniList's own data, because no source is set up or none could find the
    /// show: it can be marked watched, but there is nothing to play or download it from, so
    /// a front end offers to add a source instead. A table is all of one kind or the other.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub metadata_only: bool,
    /// The name of the source that serves this episode, when it is not the one the table is
    /// from: the title's first source in the order had no such episode, and this one filled the
    /// gap. `None` for an episode from the table's own source, which is nearly all of them.
    /// A front end shows it quietly, where it already says an episode's source.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub source: Option<String>,
    /// AniList says the episode is out and no source set up lists it. It stays in the table so
    /// the season is whole and can be marked watched, and a play says no source has it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub unsourced: bool,
}

/// How far a title's dub goes, as the source a dubbed play would use lists it: what
/// [`Event::TitleDub`] says, and what [`present::dub_line`] words for the title page.
///
/// No dub at all, and a source that could not say, are both no extent: `None` where it is
/// carried, and nothing on the page.
///
/// [`Event::TitleDub`]: crate::event::Event::TitleDub
/// [`present::dub_line`]: crate::present::dub_line
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DubExtent {
    /// The last whole episode the source lists in the dub, in the numbering of the title's
    /// episode table: 6 for "Dub to episode 6".
    pub through: u32,
    /// The dub has every episode aired so far: all of a finished show, or an airing one's dub
    /// caught up with its broadcast. `false` when it stops short, and when how far the show
    /// has aired cannot be lined up with the source's numbers.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub complete: bool,
}

/// One episode's broadcast, from AniList's schedule: [`EpisodeRow::airs_at`] by number, and
/// what a play can be checked against before looking for an episode that is not out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeAiring {
    pub episode: u32,
    /// Unix seconds.
    pub airs_at: i64,
}

impl EpisodeAiring {
    /// Whether this broadcast is still to come at `now`.
    pub fn is_upcoming(&self, now: i64) -> bool {
        self.airs_at > now
    }
}

/// What an episode is to the story, by the filler index or the source's own claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EpisodeKind {
    /// Adapted from the source material.
    Canon,
    /// Canon with filler padding: labelled, never skipped, since skipping loses story.
    Mixed,
    /// Anime-original padding, the one kind worth offering to skip.
    Filler,
    /// Anime-original, but canon within the anime's own continuity.
    AnimeCanon,
}

impl EpisodeRow {
    /// Runtime as `mm:ss`, which is how a timing sheet reads.
    pub fn runtime(&self) -> String {
        match self.duration_secs {
            Some(secs) => format!("{}:{:02}", secs / 60, secs % 60),
            None => "--:--".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broadcast_is_upcoming_until_its_moment() {
        let airing = EpisodeAiring { episode: 6, airs_at: 1_000 };
        assert!(airing.is_upcoming(999));
        assert!(!airing.is_upcoming(1_000), "airing now is out");
        assert!(!airing.is_upcoming(5_000));
    }
}
