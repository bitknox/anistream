//! What a native app's home-screen widgets show: a compact record of Continue and of the week's
//! broadcasts, which the app writes to a file its widgets read, as
//! [`Command::WidgetSnapshot`](crate::command::Command::WidgetSnapshot) answers it.
//!
//! A widget runs apart from the app and the engine, and can read a file the app wrote under an
//! older build. So the record says which shape it has, [`WidgetData::version`], and carries only
//! what a widget draws: no synopsis and no stills, and an episode's name only for the episode
//! Continue starts, as Continue shows it. Art comes as addresses, for the app to fetch with
//! `Command::FetchArtwork` and keep where its widgets read it.

use anistream_core::{ids::AnilistId, media::Translation};

/// The shape of [`WidgetData`] this build writes. Raised when a field changes meaning or goes,
/// or when fields are added that a widget has to know of to read the record right, so a widget
/// can tell a file from another build.
///
/// 2 added who is signed in, the next broadcast due, the six days after today, and for each row
/// what its layouts need: Continue's resume point and time left, and each broadcast's watched
/// mark and art.
pub const WIDGET_DATA_VERSION: u32 = 2;

/// Everything the widgets show, as of [`Self::generated_at`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WidgetData {
    /// Which shape this is: [`WIDGET_DATA_VERSION`] when the engine made it.
    pub version: u32,
    /// When it was made, in seconds since the Unix epoch. A widget counts "aired" and "in 2 h"
    /// from its own clock, and can tell from this how old what it shows is.
    pub generated_at: i64,
    /// What to continue, most recently watched first: at most five, as Continue lists them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub continue_watching: Vec<WidgetContinue>,
    /// What airs today in the device's time zone, aired or not, in the order it airs.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_today: Vec<WidgetAiring>,
    /// Whether a tracker is signed in, whose list [`WidgetAiring::on_list`] goes by. Signed out
    /// with nothing to continue, Continue asks the viewer to sign in or play something.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub signed_in: bool,
    /// Which tracker that is, `anilist` say, while one is signed in.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tracker: Option<String>,
    /// The next broadcast still to come of a show being followed, for "You're caught up" and
    /// "Nothing airing today": the soonest in the week ahead, or past it the soonest the
    /// metadata cache knows of.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_airing: Option<WidgetNextAiring>,
    /// What airs on the six days after today, in the order it airs, so a widget keeps a day's
    /// schedule for a week without the app.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_days: Vec<WidgetAiring>,
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
    /// The poster.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// The episode's name, when the metadata cache has the title's episode list.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode_title: Option<String>,
    /// Whether the episode was left part-way, to resume, rather than one to start.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: bool,
    /// Where it was left, in seconds, when it was.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub position_secs: Option<u32>,
    /// How long the episode is, in seconds: as its player found it, or AniList's length for
    /// the title's episodes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub duration_secs: Option<u32>,
    /// Whole minutes still to watch, rounded up: "14 min left". The whole length for an episode
    /// still to start.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub minutes_left: Option<u32>,
    /// When the title's next episode airs, in seconds since the Unix epoch, while one is still to
    /// come: "You're caught up · Next: Thursday 18:30".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_airing: Option<i64>,
    /// Which episode that is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_airing_episode: Option<u32>,
    /// The wide art: AniList's banner, when the title has one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
    /// The cover's colour, `#5d9be4`, for a plate drawn before the art arrives.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_color: Option<String>,
    /// The episode's mark, as Continue's rows carry it: "Dub" for a show watched dubbed, and
    /// `None`, to mark nothing, for a sub or when it is not known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub mark: Option<String>,
}

/// One broadcast.
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
    /// rewatching or planning, or watched here in the last few weeks. Planned shows count here,
    /// though the new-episode check leaves them out unless they are switched on for themselves.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub on_list: bool,
    /// The poster.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// Whether this episode has been watched here already: "Out" is only for one that has not.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: bool,
    /// Sub or dub, when the broadcast is one of them alone. Always `None` for now: AniList's
    /// schedule has one time for each episode, the original broadcast's, and no source of dub
    /// times is asked yet.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub translation: Option<Translation>,
    /// The wide art: AniList's banner, when the title has one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
    /// The cover's colour, for a plate drawn before the art arrives.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_color: Option<String>,
}

/// The next broadcast due of a show being followed.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct WidgetNextAiring {
    pub id: AnilistId,
    /// In the form the title setting picks.
    pub title: String,
    pub episode: u32,
    /// In seconds since the Unix epoch.
    pub airs_at: i64,
}
