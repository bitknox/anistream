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

/// What will notify of new episodes in the next seven days, and what is left out and why, as
/// [`Command::UpcomingNotices`](crate::command::Command::UpcomingNotices) answers it: for a
/// native app to schedule its notifications at air time, and to list them in its Notifications
/// pane.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpcomingNotices {
    /// Whether [`SettingKey::EpisodeNotifications`](crate::settings::SettingKey) is on, on this
    /// device: whether the shows being watched notify.
    pub enabled: bool,
    /// When this was made, in seconds since the Unix epoch.
    pub generated_at: i64,
    /// Every broadcast from now to the end of the sixth day on, in the device's time zone, of a
    /// show being watched or with a switch of its own, in the order they air. An episode already
    /// watched here is left out.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airings: Vec<UpcomingNotice>,
    /// Every show being watched, airing this week or not: on the signed-in tracker's list as
    /// watching or rewatching, or watched here in the last 30 days and not set aside on the
    /// list. A show's own settings say from this whether following Settings notifies of it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watching: Vec<AnilistId>,
    /// How many shows have a switch of their own, on or off: "Overridden for 2 shows".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub overrides: u32,
}

/// One broadcast of the week, of a show that notifies or is left out by its own switch.
///
/// Spoiler-safe as [`NewEpisode`] is: the episode's number and when it airs, never its name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct UpcomingNotice {
    pub id: AnilistId,
    /// In the form the title setting picks.
    pub title: String,
    pub episode: u32,
    /// When it airs by AniList's schedule, in seconds since the Unix epoch. AniList gives the
    /// broadcast's time, which is the subtitled release's.
    pub airs_at: i64,
    /// Why it notifies, or why not.
    pub reason: NoticeReason,
    /// The episode Play starts for the show: the first not watched here. Lower than
    /// [`Self::episode`] when the viewer is behind.
    pub next_to_watch: u32,
    /// The show's poster.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// The show's wide banner, when AniList has one: the notification's picture before the
    /// poster.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
}

/// Why a broadcast of the week notifies, or is left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoticeReason {
    /// The show is being watched and has no switch of its own: it notifies while
    /// [`UpcomingNotices::enabled`] is on.
    Watching,
    /// Switched on in its own settings, which takes in a show whatever the list says of it, a
    /// show only planned among them.
    SwitchedOn,
    /// Switched off in its own settings: left out, being watched or not.
    SwitchedOff,
}
