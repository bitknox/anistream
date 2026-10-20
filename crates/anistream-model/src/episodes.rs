//! A title's episodes, as its episode table and the player's Episodes menu list them.

/// One row of the timing-sheet episode table.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeRow {
    pub number: String,
    pub title: Option<String>,
    /// Still frame for this episode, when one was published.
    ///
    /// Only ever drawn for the selected row: the table is dense, and one image that follows the
    /// cursor costs a single fetch per move rather than one per visible row.
    pub thumbnail: Option<String>,
    /// One-paragraph synopsis, when the source has one.
    ///
    /// Shown for the selected row only, under its still, for the same reason the still is:
    /// the table is a timing sheet, and a paragraph per row would drown it.
    pub description: Option<String>,
    pub duration_secs: Option<u64>,
    /// How far through this episode the local history says we are.
    pub watched: f64,
    pub completed: bool,
    /// Filler, mixed or canon — from AnimeFillerList, when the show is covered.
    ///
    /// `None` means unknown, which is the common case: most shows have no filler and are not in the
    /// index at all. Rendered only when present, so an uncovered show shows no column rather than a
    /// column of blanks.
    pub kind: Option<EpisodeKind>,
    /// Whether offering to skip this whole episode is reasonable.
    ///
    /// Only pure filler. `mixed` episodes contain filler *and* canon, so skipping one loses story —
    /// the distinction is the whole reason this is a separate field rather than `kind == "filler"`
    /// at every call site.
    pub skippable: bool,
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
