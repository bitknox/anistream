//! What a remote on a phone asks of a TV's front end: the part of a remote that is not the
//! engine's, since only the front end knows what is on its screen. The protocol is
//! `anistream-remote`'s, written up in `docs/remote-protocol.md`.

use anistream_core::ids::AnilistId;

/// What the TV front end is asked to do by a remote, as [`crate::event::Event::Remote`] brings
/// it. A front end that is not a TV ignores it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FrontIntent {
    /// Show the title's page.
    Open { id: AnilistId },
    /// Set the focused field's text, and submit it when asked.
    Text { text: String, submit: bool },
    /// Move the focus, as the TV's own remote would.
    Navigate { key: RemoteKey },
    /// Step the TV's own volume, the system's, by `step` points of 100; negative turns it down.
    /// Not the player's volume: on many TVs the system's reaches the set itself over HDMI-CEC.
    Volume { step: f64 },
    /// Show a pairing code, for the phone named, until [`FrontIntent::HidePairingCode`].
    ShowPairingCode { code: String, phone: String },
    /// The code on screen is used up: a phone paired with it, it ran out, or too many wrong
    /// tries were made.
    HidePairingCode,
    /// The phone named has paired, for the notice that says so.
    Paired { phone: String },
}

/// The keys of a TV's own remote, as a phone presses them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RemoteKey {
    Up,
    Down,
    Left,
    Right,
    Select,
    Back,
    Home,
}
