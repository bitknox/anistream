//! The download queue, and the watched episodes still seeding, as the Downloads screen lists
//! them.

use anistream_core::ids::AnilistId;

/// One row of the download queue, flattened for the screen.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DownloadRow {
    pub id: i64,
    /// The series this file belongs to — what lets a downloaded episode carry the same
    /// history, resume and sync a streamed one does.
    pub anilist_id: AnilistId,
    pub title: String,
    pub episode: String,
    pub state: DownloadState,
    pub fraction: f64,
    pub downloaded: u64,
    pub total: u64,
    /// Why it failed, when it did. Kept on the row because a toast lasts seconds and the reason
    /// still matters afterwards.
    pub error: Option<String>,
    /// Where the file is, once it exists — so a finished download can be played.
    pub path: Option<String>,
}

/// Where a download is in its life, as its row has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum DownloadState {
    /// Accepted, not started: the queue is bounded, so this lasts.
    Queued,
    Active,
    Paused,
    Done,
    Failed,
}

/// A watched episode still seeding, as the Downloads screen lists it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SeedRow {
    pub name: String,
    pub uploaded_bytes: u64,
    /// Uploaded over what this machine has of it.
    pub ratio: f64,
    /// The ratio it is seeding towards.
    pub goal: f64,
    /// Until the time limit retires it.
    pub left_secs: u64,
}
