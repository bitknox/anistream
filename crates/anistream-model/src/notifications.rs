//! New episodes, as a native app notifies of them: what
//! [`Command::CheckNewEpisodes`](crate::command::Command::CheckNewEpisodes) finds.

use anistream_core::ids::AnilistId;

/// An episode of a show being followed that aired since the show was last checked, said once.
///
/// Spoiler-safe by what it leaves out: the episode's number and when it aired, never its name,
/// which can give away what happens in it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct NewEpisode {
    /// The show.
    pub id: AnilistId,
    /// The show's title, in the form the title setting picks, as every list names it.
    pub title: String,
    /// The episode's number, as AniList counts the show's episodes.
    pub episode: u32,
    /// When it aired, in seconds since the Unix epoch.
    pub aired_at: i64,
    /// The show's cover, for the notification's picture.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}
