//! What a native app's home-screen widgets show: a compact record of Continue and of today's
//! broadcasts, which the app writes to a file its widgets read, as
//! [`Command::WidgetSnapshot`](crate::command::Command::WidgetSnapshot) answers it.
//!
//! A widget runs apart from the app and the engine, and can read a file the app wrote under an
//! older build. So the record says which shape it has, [`WidgetData::version`], and carries only
//! what a widget draws: no synopsis, no episode names, no stills.

use anistream_core::ids::AnilistId;

/// The shape of [`WidgetData`] this build writes. Raised when a field changes meaning or goes,
/// not when one is added with a default, so a widget can refuse a file it would misread.
pub const WIDGET_DATA_VERSION: u32 = 1;

/// Everything the widgets show, as of [`Self::generated_at`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WidgetData {
    /// Which shape this is: [`WIDGET_DATA_VERSION`] when the engine made it.
    pub version: u32,
    /// When it was made, in seconds since the Unix epoch. A widget counts "aired" and "in 2 h"
    /// from its own clock, and can tell from this how old what it shows is.
    pub generated_at: i64,
    /// What to continue, most recently watched first: at most four, as Continue lists them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub continue_watching: Vec<WidgetContinue>,
    /// What airs today in the device's time zone, aired or not, in the order it airs.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_today: Vec<WidgetAiring>,
}

/// One title to continue.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WidgetContinue {
    pub id: AnilistId,
    /// In the form the title setting picks.
    pub title: String,
    /// The episode Continue starts: the one left part-way, or the next one.
    pub episode: u32,
    /// How many episodes have been watched.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: u32,
    /// How many episodes the title has, when that is known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: Option<u32>,
    /// How far into [`Self::episode`] it was left, from nought to one, when it was started and
    /// its length is known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fraction: Option<f64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}

/// One broadcast of today's.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WidgetAiring {
    pub id: AnilistId,
    /// In the form the title setting picks.
    pub title: String,
    pub episode: u32,
    /// When it airs, or aired, in seconds since the Unix epoch.
    pub airs_at: i64,
    /// Whether the show is one being followed: on the signed-in tracker's list as watching,
    /// rewatching or planning, or watched here in the last few weeks. The shows the new-episode
    /// check follows, whatever their own notification setting.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub on_list: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}
