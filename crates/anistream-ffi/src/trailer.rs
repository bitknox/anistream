//! Trailers for a front end that cannot call the model itself: the page its web view loads, the
//! scripts it sends that page, what the page says back, and whether a title's trailer may start
//! by itself. See `anistream_model::trailer`.

use anistream_model::{
    catalog::{Entry, Trailer},
    trailer::{self, AppIdentity, EmbedStyle, TrailerControl, TrailerMessage},
};

/// The page to load into the web view for `trailer`, under [`trailer_base_url`]. `None` for a
/// trailer the app's player does not play: Play trailer opens [`trailer_watch_url`] instead.
#[uniffi::export]
pub fn trailer_embed_page(
    trailer: Trailer,
    style: EmbedStyle,
    app: AppIdentity,
) -> Option<String> {
    trailer::embed_page(&trailer, style, &app)
}

/// The base URL the page is loaded under, which makes the app's identifier its Referer.
#[uniffi::export]
pub fn trailer_base_url(app: AppIdentity) -> String {
    app.base_url()
}

/// A message the page posted through the bridge, or `None` for one to ignore.
#[uniffi::export]
pub fn trailer_message(message: String) -> Option<TrailerMessage> {
    TrailerMessage::parse(&message)
}

/// The script that tells the page to play, pause, mute or unmute.
#[uniffi::export]
pub fn trailer_script(control: TrailerControl) -> String {
    control.script().to_owned()
}

/// The trailer's watch page on its host: where Play trailer goes when the embed refuses it.
#[uniffi::export]
pub fn trailer_watch_url(trailer: Trailer) -> String {
    trailer.watch_url()
}

/// Whether the app's own player plays it (YouTube only).
#[uniffi::export]
pub fn trailer_embeddable(trailer: Trailer) -> bool {
    trailer.embeddable()
}

/// Whether this title's trailer may start by itself, as far as the title goes; `Event::Autoplay`
/// says whether any may start now, and both must hold.
#[uniffi::export]
pub fn trailer_may_autoplay(entry: Entry, spoiler_protection: bool) -> bool {
    trailer::trailer_may_autoplay(&entry, spoiler_protection)
}
