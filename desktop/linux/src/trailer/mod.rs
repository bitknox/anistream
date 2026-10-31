//! Trailers: the hover preview and the Play trailer window. A title page's banner never plays one.
//!
//! The engine decides whether a trailer may start by itself ([`Event::Autoplay`] and
//! [`trailer_may_autoplay`]) and writes the page; this module owns the web view, which is
//! WebKitGTK 6 loaded only when a trailer is about to play (see [`webkit`]).
//!
//! The terms of YouTube's embedded player shape everything here: nothing is drawn over a playing
//! trailer, its area is at least 200 by 200 px, it starts only while more than half of it shows,
//! only one plays by itself at a time, and the pointer over it does nothing.

mod preview;
mod stage;
mod webkit;
mod window;

use anistream_model::{catalog::Entry, settings::SettingKey, trailer::trailer_may_autoplay};

pub use preview::{
    attach as hover_preview, close as close_preview, shot_popover as shot_preview,
    show as show_preview,
};
pub use stage::Stage;
pub use window::play as play_window;

use crate::{catalog, hub::Hub};

/// The playback setting's subtitle language, which a trailer's captions follow.
pub fn subtitle_language(hub: &Hub) -> String {
    hub.known.borrow().setting(SettingKey::Subtitles).map(|row| row.value).unwrap_or_default()
}

/// What YouTube's error code says, for the log.
pub fn meaning(code: u32) -> &'static str {
    match code {
        0 => "the page or the player API did not load",
        2 => "a bad video id",
        5 => "the HTML5 player failed",
        100 => "the video is gone or private",
        101 | 150 => {
            "embedding not allowed, or YouTube's sign-in check (a flagged network, such as a VPN exit)"
        }
        152 => "the embed's identity was refused",
        153 => "the embed sent no identity (Referer)",
        _ => "an unknown code",
    }
}

/// Whether this title's trailer may start by itself now: the engine allows trailers on this
/// device and network, and the title is one it plays and the spoiler rule does not hold back.
pub fn may_autoplay(hub: &Hub, entry: &Entry) -> bool {
    let known = hub.known.borrow();
    let allowed = known.autoplay.as_ref().is_some_and(|view| view.trailers);
    let spoilers =
        known.setting(SettingKey::SpoilerProtection).is_some_and(|row| catalog::is_on(&row));
    allowed && trailer_may_autoplay(entry, spoilers) && webkit::available()
}
