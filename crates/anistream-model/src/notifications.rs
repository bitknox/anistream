//! New episodes, as a native app notifies of them: what
//! [`Command::CheckNewEpisodes`](crate::command::Command::CheckNewEpisodes) finds, and the ones
//! coming up that [`Command::ComingEpisodes`](crate::command::Command::ComingEpisodes) lists.

use anistream_core::{ids::AnilistId, media::Translation};

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
    /// The episode the viewer is up to, when that is before this one: how far behind they are,
    /// and the one Play starts, as Continue offers it. `None` when they are caught up, or have
    /// watched none of the show here.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_for_you: Option<u32>,
    /// The translation the viewer watches the show in: its own setting, or Settings'. AniList
    /// dates the original broadcast only, so a dub can come out after [`Self::aired_at`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub translation: Option<Translation>,
    /// The show's banner, for the notification's expanded picture, where AniList has one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
}

/// An episode of a show the new-episode check follows, airing in the next week: what the
/// Notifications settings list under "Coming up", whether notifications are on or not.
///
/// Spoiler-safe as [`NewEpisode`] is: the show, the episode's number and when it airs.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ComingEpisode {
    /// The show.
    pub id: AnilistId,
    /// The show's title, in the form the title setting picks.
    pub title: String,
    /// The episode's number.
    pub episode: u32,
    /// When it airs, by AniList's schedule, in seconds since the Unix epoch.
    pub airs_at: i64,
    /// The show's cover.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// Whether the show is followed because it turned notifications on for itself, rather than
    /// by being watched: "On in this show's settings".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub switched_on: bool,
    /// The translation the viewer watches the show in: its own setting, or Settings'.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub translation: Option<Translation>,
}
