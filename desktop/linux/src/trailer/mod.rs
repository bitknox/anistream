//! Trailers: the banner that plays one, the hover preview and the Play trailer window.
//!
//! The engine decides whether a trailer may start by itself ([`Event::Autoplay`] and
//! [`trailer_may_autoplay`]) and writes the page; this module owns the web view, which is
//! WebKitGTK 6 loaded only when a trailer is about to play (see [`webkit`]).
//!
//! The terms of YouTube's embedded player shape everything here: nothing is drawn over a playing
//! trailer, its area is at least 200 by 200 px, it starts only while more than half of it shows,
//! only one plays by itself at a time, and the pointer over it does nothing.

mod banner;
mod preview;
mod stage;
mod webkit;
mod window;

use anistream_model::{catalog::Entry, settings::SettingKey, trailer::trailer_may_autoplay};

pub use banner::PageTrailer;
pub use preview::{
    attach as hover_preview, close as close_preview, shot_popover as shot_preview,
    show as show_preview,
};
pub use stage::{Slot, Stage};
pub use window::play as play_window;

use crate::{catalog, hub::Hub};

/// Whether this title's trailer may start by itself now: the engine allows trailers on this
/// device and network, and the title is one it plays and the spoiler rule does not hold back.
pub fn may_autoplay(hub: &Hub, entry: &Entry) -> bool {
    let known = hub.known.borrow();
    let allowed = known.autoplay.as_ref().is_some_and(|view| view.trailers);
    let spoilers =
        known.setting(SettingKey::SpoilerProtection).is_some_and(|row| catalog::is_on(&row));
    allowed && trailer_may_autoplay(entry, spoilers) && webkit::available()
}
