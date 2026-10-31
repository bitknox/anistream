//! Your media: folders on this computer, the series found in them, and what each is matched to.
//!
//! The engine fills these in; the apps draw them. File names are matched to AniList folder first,
//! and a match below the confidence that is safe to act on waits in Needs a look until the viewer
//! confirms it. Nothing is written to a tracker from a match that has not been confirmed.

use anistream_core::ids::AnilistId;

/// A folder the viewer added.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FolderRow {
    pub path: String,
    /// Video files found in the last scan.
    pub files: u32,
    /// Whether the folder could be read at the last scan. A drive that is not plugged in is not.
    pub available: bool,
}

/// Where a group of files stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum GroupState {
    /// Matched with enough confidence, or confirmed by the viewer.
    Matched,
    /// A likely match that waits for the viewer to confirm it.
    NeedsLook,
    /// Nothing matched. The viewer can choose a match or keep the files unmatched.
    NotMatched,
    /// Kept unmatched: a title with its own metadata, never written to a tracker.
    Kept,
}

/// Why nothing was matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NotMatchedReason {
    /// AniList has nothing like the name.
    NoResult,
    /// AniList could not be asked just now. The group is tried again on the next scan.
    Offline,
}

/// The files of one series, as found.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LocalGroup {
    /// Stable across scans: the folder and the series' name in it.
    pub id: String,
    /// The name read from the folder or file names.
    pub name: String,
    pub files: u32,
    pub state: GroupState,
    /// The entry it is matched to, or the own-metadata id when kept.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub anilist: Option<AnilistId>,
    /// The matched entry's title, for the row.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub matched_title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub reason: Option<NotMatchedReason>,
}

/// Everything Your media shows.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LocalView {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub folders: Vec<FolderRow>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub groups: Vec<LocalGroup>,
    /// A scan is running.
    pub scanning: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LocalEvent {
    /// Your media, after any change and as a scan finds more.
    View(LocalView),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LocalCommand {
    /// Send the view.
    Load,
    /// Add a folder and scan it.
    AddFolder { path: String },
    /// Stop reading a folder. The files are not touched.
    RemoveFolder { path: String },
    /// Scan every folder again.
    Scan,
    /// Confirm a match, or fix one, for a group. Chosen with the match picker.
    Choose { group: String, anilist: AnilistId },
    /// Keep a group's files as a title of their own.
    Keep { group: String },
}
