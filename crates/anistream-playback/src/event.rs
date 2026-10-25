//! What a player reports of playback, whichever player it is.

use crate::companion::ScriptEvent;

/// What the UI needs to know about playback.
///
/// A narrowed projection of the protocol events: the raw stream carries a lot that only
/// matters internally, and passing it all to the UI would invite decisions being made in the
/// wrong layer.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    /// Position and, once known, duration.
    Progress {
        position: f64,
        duration: Option<f64>,
    },
    Paused(bool),
    Speed(f64),
    Volume(f64),
    /// The file's chapter markers arrived: `(title, start_seconds)`.
    Chapters(Vec<(String, f64)>),
    /// The file finished loading. Until this, property reports describe no file at all.
    Loaded,
    /// The viewer did something inside the player — a key, a button, a menu pick.
    Script(ScriptEvent),
    /// The decoded picture's parameters, raw.
    VideoParams(serde_json::Value),
    /// The video track's description, raw.
    VideoTrack(serde_json::Value),
    /// The file's tracks, raw.
    TrackList(serde_json::Value),
    Aid(Option<i64>),
    Sid(Option<i64>),
    SubDelay(f64),
    /// Playback is held waiting for data, or has resumed.
    Buffering(bool),
    /// The stretches of the file the player holds and can seek within without waiting, as
    /// `(start, end)` in seconds of the file: mpv's `demuxer-cache-state`, its
    /// `seekable-ranges`.
    Cached(Vec<(f64, f64)>),
    /// Running total of frames the video output dropped.
    FrameDrops(u64),
    Seeking(bool),
    /// The display the window is on, once mpv knows it, and whether the renderer runs compute
    /// shaders there.
    Display {
        width: u32,
        height: u32,
        compute: bool,
    },
    /// The renderer disabled a shader hook it could not compile or run, as it names it.
    HookDisabled(String),
    /// The file was replaced by our own `loadfile`. The player is still there.
    Replaced,
    /// Playback of the file ended. `complete` is true only when mpv reached the end of it.
    ///
    /// The window outlives this: whether the player closes now is the playback loop's call.
    Ended {
        complete: bool,
    },
}
