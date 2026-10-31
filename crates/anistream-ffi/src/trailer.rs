//! Trailers for a front end that cannot call the model itself: the page its web view loads, the
//! scripts it sends that page, what the page says back, and whether a title's trailer may start
//! by itself. See `anistream_model::trailer`.

use anistream_model::{
    catalog::{Entry, Trailer},
    trailer::{self, AppIdentity, EmbedStyle, TrailerControl, TrailerMessage},
};

/// The page with captions in `subtitle_language` (the playback setting), and with no video for
/// `None`: a player loaded ahead of the first trailer, which [`trailer_load_script`] then plays.
#[uniffi::export]
pub fn trailer_page(
    trailer: Option<Trailer>,
    style: EmbedStyle,
    app: AppIdentity,
    subtitle_language: String,
) -> Option<String> {
    trailer::page(trailer.as_ref(), style, &app, &subtitle_language)
}

/// The two-letter code YouTube names captions by, for the subtitle language setting; `None` for
/// none, `off` or a language it does not know.
#[uniffi::export]
pub fn trailer_caption_language(subtitle_language: String) -> Option<String> {
    trailer::caption_language(&subtitle_language).map(str::to_owned)
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

/// The script that switches a page that is already up to `trailer`, so a front end keeps one
/// web view and its player. `None` for a trailer the app's player does not play.
#[uniffi::export]
pub fn trailer_load_script(trailer: Trailer) -> Option<String> {
    trailer::load_script(&trailer)
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

/// The script that reads YouTube's player state back from the page, for the log when the front
/// end gives up on a trailer: a number, or null while there is no player.
#[uniffi::export]
pub fn trailer_state_script() -> String {
    trailer::STATE_SCRIPT.to_owned()
}

/// The preview's style that hides YouTube's own controls in its frame, for a user script in every
/// frame of the preview's web view. Never for Play trailer.
#[uniffi::export]
pub fn trailer_chrome_script() -> String {
    trailer::CHROME_SCRIPT.to_owned()
}

/// What the chrome script says once its style is in place.
#[uniffi::export]
pub fn trailer_chrome_message() -> String {
    trailer::CHROME_MESSAGE.to_owned()
}

/// What the warm page's YouTube frame has fetched so far, asked once: the frame answers `bytes:`
/// and the count.
#[uniffi::export]
pub fn trailer_bytes_script() -> String {
    trailer::BYTES_SCRIPT.to_owned()
}
