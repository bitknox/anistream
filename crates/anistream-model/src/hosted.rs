//! The player a front end hosts: what the engine tells it to do, what its own interface shows,
//! and what it reports back.
//!
//! The terminal plays in mpv's child process. A native app plays in a player of its own, libmpv
//! embedded, and the engine drives it the same way, through one protocol in mpv's own vocabulary:
//!
//! - [`PlayerOp`]s, on `Event::PlayerOps`, are what the player is to do, each one call of the
//!   engine's, in order: load a file with its options, set a property, seek. The app hands them
//!   to libmpv as they are.
//! - [`PlayerUi`], on `Event::PlayerUi`, is what the player's own interface shows: the starting
//!   screen, the skip button, Up Next, the menus, the stall prompt. The app draws it in place of
//!   the companion script mpv wears in the terminal.
//! - [`PlayerReport`]s, on `Command::PlayerReport`, are what the player says: each property it
//!   observes as mpv reports it, the file loading and ending, the display it plays on, and the
//!   power it runs on. The engine reads them exactly as it reads mpv's.
//!
//! Each names the playback it belongs to, a [`PlaybackId`]: one window can carry a binge, so the
//! playback driving it changes as one episode rolls on to the next. What the viewer does in the
//! player's own interface, taking a skip or picking from a menu, goes back as player commands,
//! as the terminal's keys do.
//!
//! [`PlaybackId`]: crate::playback::PlaybackId
//!
//! # Closing
//!
//! A player closes one of two ways, and the playback ends differently for each:
//!
//! - The viewer closes it, with the window's close button or a swipe down. The player reports
//!   `Ended { reason: Quit }` for the file, then `Closed`. Nothing told it to: the playback ends
//!   as `Ending::Closed`, what mpv's window going away is.
//! - The engine stops it: the front end asked for `PlayerCommand::Stop`. The player is sent
//!   `PlayerOp::Quit`, reports `Ended { reason: Quit }` and then `Closed`, and the playback ends
//!   as `Ending::Stopped`.
//!
//! Either way `Closed` is the player's last word, and nothing more is said to it: a
//! `PlayerOp::Close` that crosses it is let go by the player.

use anistream_core::{ids::AnilistId, media::Translation};

use crate::{
    notice::NoticeKind,
    playback::{NextEpisode, SkipKind},
};

/// One thing a hosted player is to do, as mpv would be told it. In order: a later one may
/// depend on an earlier, as a seek does on the file loaded before it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerOp {
    /// Show the player, with nothing in it yet: what follows loads into it. `speed` and `volume`
    /// are where the viewer left them, when the settings keep them.
    Open { fullscreen: bool, speed: Option<f64>, volume: Option<f64> },
    /// Open `url` in place of whatever is playing, with `options` that belong to this file
    /// alone, as `loadfile`'s per-file options: the headers, the resume point as `start`, the
    /// title and the track languages. A web stream may come as a loopback address the engine
    /// serves, which mends what its host sends.
    Load { url: String, options: Vec<MpvOption> },
    /// Add an external subtitle track to the loaded file without selecting it, as `sub-add` with
    /// the `auto` flag. `url` may be a local path the engine wrote the track to.
    AddSubtitle { url: String, title: Option<String>, language: Option<String> },
    /// Set a property.
    Set { property: String, value: MpvValue },
    /// Flip a flag, as `cycle`: `pause`.
    Cycle { property: String },
    /// Take the window into full screen, or out of it: the host's window, which mpv inside it
    /// knows nothing of. Never sent as a `fullscreen` property.
    ToggleFullscreen,
    /// Step a number by `delta`, as `add`: `speed`, `volume`. The player clamps it.
    Add { property: String, delta: f64 },
    /// Seek to `seconds`, or by them when not `absolute`; exactly, as `absolute+exact` and
    /// `relative+exact`.
    Seek { seconds: f64, absolute: bool },
    /// Replace the upscaling chain with these shader files, all at once, as `glsl-shaders`.
    /// Empty turns upscaling off.
    Shaders { paths: Vec<String> },
    /// Add or remove one labelled audio filter, as `af add` and `af remove`.
    AudioFilter { add: bool, filter: String },
    /// Report `chapter-list` again, as it stands now: the file has loaded, and whether it brought
    /// chapters of its own decides whether the engine marks some.
    RestateChapters,
    /// Stop the file and keep the player, for the next one.
    Stop,
    /// The viewer stopped playback: close the player, and report how the file ended.
    Quit,
    /// Close the player: the playback is over and nothing more will be said to it.
    Close,
}

/// One per-file option of `loadfile`, as mpv takes them: strings, lists already joined.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MpvOption {
    pub name: String,
    pub value: String,
}

/// A property's value in mpv's own formats, for `mpv_set_property`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MpvValue {
    Flag(bool),
    Int(i64),
    Double(f64),
    String(String),
    /// A node, a list or a map, as JSON text: `chapter-list`'s entries, the shader paths.
    Node(String),
}

/// What a hosted player's own interface is to show: a state it keeps until told otherwise, or a
/// moment to show.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerUi {
    /// The starting screen: an episode is being found or opened, this one or the one that
    /// follows it in the same player. It stays until the player has a picture. `id` is the
    /// title's, and its art is by URL where the engine knows it, so the screen needs nothing
    /// remembered from `Event::PlaybackStaged`.
    Starting {
        id: AnilistId,
        title: String,
        episode: String,
        episode_title: Option<String>,
        cover_url: Option<String>,
        banner_url: Option<String>,
    },
    /// The episode on screen.
    Episode {
        number: String,
        label: String,
        title: Option<String>,
        show: String,
        translation: Translation,
    },
    /// What the player's menus list, whole.
    Menus {
        episodes: Vec<MenuEpisode>,
        sources: Vec<MenuSource>,
        translation: Translation,
        upscaling: MenuChoices,
        /// `None` while the picture is not HDR: the choice means nothing then.
        tone_mapping: Option<MenuChoices>,
        night_mode: bool,
    },
    /// The playhead entered a segment that can be skipped: offer to skip to `to`.
    Skip { kind: SkipKind, to: f64 },
    /// The segment on offer has passed.
    SkipCleared,
    /// A segment was skipped by itself: offer to undo it, back to `undo_to`.
    AutoSkipped { kind: SkipKind, undo_to: f64 },
    /// The Up Next card for `next`, whose `thumbnail` is the still to show. Its countdown is the
    /// app's: it starts at `ending_at`, where the ending begins, or near the end when that is not
    /// known, runs `countdown` seconds, and when it reaches zero the app sends `PlayNext`; `0`
    /// is no countdown, and the card waits. `auto` is whether the episode rolls on by itself at
    /// the end when nobody does. `still_watching_after` is how many episodes in a row with no
    /// input from the viewer the app plays before it pauses to ask "Still watching?".
    UpNext {
        next: NextEpisode,
        auto: bool,
        ending_at: Option<f64>,
        countdown: u32,
        still_watching_after: u32,
    },
    /// Nothing plays next.
    UpNextCleared,
    /// The stream has been held waiting for data for `seconds`, mid-episode: offer another
    /// stream, or to keep waiting.
    Stall { seconds: u64 },
    /// The stall is over.
    StallCleared,
    /// The file opened at `position`, where the viewer left it.
    Resumed { position: f64 },
    /// What the picture is and what is done to it, for its badges: the upscaler running, and the
    /// HDR format.
    Picture { upscaler: Option<String>, hdr: Option<String> },
    /// Where the seek bar's previews are read from instead of the stream, when that differs: a
    /// torrent's, which reads only what has downloaded.
    Thumbnails { url: Option<String> },
    /// A message on the video.
    Alert { text: String, kind: NoticeKind },
}

/// One row of the player's Episodes menu.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MenuEpisode {
    pub number: String,
    pub label: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub filler: bool,
    /// The episode playing.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub current: bool,
}

/// One row of the player's Sources menu: a stream found for the episode, `id` being what
/// `PlayerChoice::Source` picks it by.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MenuSource {
    pub id: String,
    pub label: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub detail: Option<String>,
    /// The stream playing.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub current: bool,
    /// A short note on how the stream's server has fared lately, when it has been failing:
    /// `failed the last 3 times · 12 min ago`. `None` for a server with nothing against it. A
    /// server that keeps failing is listed after the others of its quality; this says why.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub health: Option<String>,
}

/// A menu of named choices with one current, the upscaling looks and the HDR curves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MenuChoices {
    pub current: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub options: Vec<MenuChoice>,
}

/// One choice in a [`MenuChoices`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MenuChoice {
    pub id: String,
    pub label: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hint: Option<String>,
}

/// What a hosted player says of the playback it plays.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerReport {
    /// The file opened: mpv's `file-loaded`.
    FileLoaded,
    /// An observed property changed: its name and its value as JSON, in mpv's own form, the
    /// way `mpv_observe_property` with `MPV_FORMAT_NODE` gives it. The engine reads `time-pos`,
    /// `duration`, `pause`, `speed`, `volume`, `chapter-list`, `video-params`,
    /// `current-tracks/video`, `track-list`, `aid`, `sid`, `sub-delay`, `paused-for-cache`,
    /// `demuxer-cache-state`, `frame-drop-count` and `seeking`.
    Property { name: String, json: String },
    /// The file ended: mpv's `end-file`, with its reason.
    Ended { reason: PlayerEnd },
    /// The player is gone: the viewer closed it, or it failed. Nothing more is said to it.
    Closed,
    /// The display the player is on, in pixels, with its scale, refresh rate and the EDR
    /// headroom an HDR picture has above SDR white; and whether the player's renderer runs
    /// compute shaders. libplacebo has none on OpenGL ES, which is Android's GL path, so ArtCNN
    /// cannot run there; Metal and Vulkan have them.
    Display {
        width: u32,
        height: u32,
        scale: f64,
        refresh_hz: Option<f64>,
        edr_headroom: Option<f64>,
        compute: bool,
    },
    /// A shader hook the renderer gave up on: libplacebo disables a hook that does not compile
    /// or run ("Failed executing hook …, disabling" in mpv's log), and the picture plays on
    /// without it. `hook` is the hook as the log names it. The chain steps down, rather than
    /// being taken for running.
    HookDisabled { hook: String },
    /// What the device runs on: the upscaling chain starts one step lighter on battery, or in
    /// low power mode.
    Power { on_battery: bool, low_power: bool },
}

/// Why a file ended, as mpv's `end-file` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerEnd {
    /// Played through to the end.
    Eof,
    /// The viewer closed it, or the player was told to quit.
    Quit,
    /// Replaced by the next `Load`.
    Stop,
    /// Moved on to another file in a playlist.
    Redirect,
    /// It could not play.
    Error,
    Unknown,
}

/// Which kind of track a viewer picks: the audio, or the subtitles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrackKind {
    Audio,
    Subtitle,
}

/// A pick in a hosted player's menus.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerChoice {
    /// An episode, by number, from the Episodes menu.
    Episode(String),
    /// A stream, by its [`MenuSource`] id.
    Source(String),
    /// The other translation.
    Translation(Translation),
    /// An HDR curve, by id.
    ToneMapping(String),
}
