//! Settings' preview column: what the pane is about, or the selected row's effect where it has
//! one. Upscaling shows the 480p source beside the mode on one frame; subtitles show a caption
//! sample. On the end of the pane at 1280 and wider, and on a row's own page below that.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_model::{downloads::DownloadState, settings::SettingKey};
use gtk::{gdk, glib};

use crate::{catalog, hub::Known};

/// What the column shows: a pane's note, or a row's effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Pane(&'static str),
    Upscaling,
    Subtitles,
}

/// The topic a row selects, for the rows that show something of their own.
pub fn topic_of(key: SettingKey) -> Option<Topic> {
    match key {
        SettingKey::Upscaling => Some(Topic::Upscaling),
        SettingKey::Subtitles | SettingKey::Translation => Some(Topic::Subtitles),
        _ => None,
    }
}

/// The frames: a 480p encode scaled to fit, and the same upscaled by each mode.
const OFF: &[u8] = include_bytes!("../../../../website/public/player/compare-off.webp");
const NATURAL: &[u8] = include_bytes!("../../../../website/public/player/compare-natural.webp");
const SHARP: &[u8] = include_bytes!("../../../../website/public/player/compare-sharp.webp");

/// A frame's pixels, RGBA, decoded once.
struct Frame {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

thread_local! {
    static FRAMES: RefCell<Vec<(&'static str, Rc<Frame>)>> = RefCell::default();
}

fn frame(name: &'static str) -> Option<Rc<Frame>> {
    if let Some(found) =
        FRAMES.with_borrow(|f| f.iter().find(|(n, _)| *n == name).map(|(_, f)| Rc::clone(f)))
    {
        return Some(found);
    }
    let bytes = match name {
        "natural" => NATURAL,
        "sharp" => SHARP,
        _ => OFF,
    };
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    let frame = Rc::new(Frame {
        width: width as usize,
        height: height as usize,
        rgba: image.into_raw(),
    });
    FRAMES.with_borrow_mut(|f| f.push((name, Rc::clone(&frame))));
    Some(frame)
}

/// The 480p source on the start half and `mode` on the end half, a silk rule between them.
fn split(left: &Frame, right: &Frame) -> Vec<u8> {
    let (width, height) = (left.width.min(right.width), left.height.min(right.height));
    let half = width / 2;
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let pixel = if x + 1 == half || x == half {
                &[0xE4, 0xE0, 0xD6, 0xFF][..]
            } else {
                let frame = if x < half { left } else { right };
                let at = (y * frame.width + x) * 4;
                &frame.rgba[at..at + 4]
            };
            rgba.extend_from_slice(pixel);
        }
    }
    rgba
}

/// A mode's setting value as the frame it shows: "natural", "sharp" or "off".
fn mode_frame(value: &str) -> &'static str {
    if value.starts_with("natural") {
        "natural"
    } else if value.starts_with("sharp") {
        "sharp"
    } else {
        "off"
    }
}

/// What the end chip and the note say of a mode.
pub fn mode_words(value: &str) -> (&'static str, &'static str) {
    match mode_frame(value) {
        "natural" => ("natural · ArtCNN", "Faithful line work and texture, nothing restyled."),
        "sharp" => {
            ("sharp · Anime4K", "Crisper lines and stronger edges, at some cost to texture.")
        }
        _ => ("off", "The 480p source, scaled to fit without processing."),
    }
}

fn chip(text: &str, align: gtk::Align) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("chip");
    label.set_halign(align);
    label.set_valign(gtk::Align::End);
    label.set_margin_start(8);
    label.set_margin_end(8);
    label.set_margin_bottom(8);
    label
}

fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("heading");
    label.set_xalign(0.0);
    label
}

fn note(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.set_max_width_chars(36);
    label
}

/// A frame in a rounded box at its own aspect, with `over` laid on it.
fn framed(texture: &gdk::Texture, over: &[gtk::Widget]) -> gtk::Widget {
    let picture = gtk::Picture::for_paintable(texture);
    picture.set_content_fit(gtk::ContentFit::Cover);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&picture));
    for widget in over {
        overlay.add_overlay(widget);
    }
    let frame = gtk::AspectFrame::new(0.5, 0.5, 3.0 / 2.0, false);
    frame.set_child(Some(&overlay));
    frame.add_css_class("preview-frame");
    frame.set_overflow(gtk::Overflow::Hidden);
    frame.upcast()
}

fn texture(width: usize, height: usize, rgba: Vec<u8>) -> gdk::Texture {
    gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &glib::Bytes::from_owned(rgba),
        width * 4,
    )
    .upcast()
}

/// Upscaling: the 480p source beside `value`'s mode, and what the mode does.
pub fn upscaling(value: &str) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&heading("Upscaling"));
    let (name, words) = mode_words(value);
    if let (Some(off), Some(mode)) = (frame("off"), frame(mode_frame(value))) {
        let rgba = split(&off, &mode);
        let picture = texture(off.width.min(mode.width), off.height.min(mode.height), rgba);
        column.append(&framed(
            &picture,
            &[
                chip("480p source", gtk::Align::Start).upcast(),
                chip(name, gtk::Align::End).upcast(),
            ],
        ));
    }
    column.append(&note(words));
    column
}

/// Subtitles: a plain line as the player sets it, over the 480p frame.
pub fn caption(language: &str) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.append(&heading("Subtitles"));
    if let Some(off) = frame("off") {
        let line = gtk::Label::new(Some("The lantern road runs on past the last town."));
        line.add_css_class("caption-sample");
        line.set_wrap(true);
        line.set_justify(gtk::Justification::Center);
        line.set_valign(gtk::Align::End);
        line.set_margin_bottom(14);
        line.set_margin_start(12);
        line.set_margin_end(12);
        column.append(&framed(
            &texture(off.width, off.height, off.rgba.clone()),
            &[line.upcast()],
        ));
    }
    let language = crate::say::language(language);
    column.append(&note(&format!(
        "Plain subtitles look like this, in {language} when an episode has them. Styled \
         subtitles keep their own look."
    )));
    column
}

/// A pane's note, from what is known now.
fn pane_note(pane: &str, known: &Known) -> Option<(&'static str, String)> {
    let on = |key| known.setting(key).is_some_and(|r| catalog::is_on(&r));
    Some(match pane {
        "downloads" => {
            let taken: u64 = known
                .downloads
                .iter()
                .filter(|r| r.state == DownloadState::Done)
                .map(|r| r.total.max(r.downloaded))
                .sum();
            ("Storage", format!("Downloads take {}.", glib::format_size(taken)))
        }
        "notifications" => (
            "New episodes",
            if on(SettingKey::EpisodeNotifications) {
                "One notification when an episode of a show in your Watching list comes out, at \
                 the time AniList gives. A show's own setting comes first."
            } else {
                "Off. Only shows turned on in their own settings notify."
            }
            .to_owned(),
        ),
        "sources" if on(SettingKey::Torrents) => (
            "Connection check",
            "anistream checks where torrent traffic leaves from when a torrent starts and every \
             minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is \
             what protects you."
                .to_owned(),
        ),
        "sources" => (
            "Sources",
            "Sources are tried in order, top to bottom. A show can keep its own source, chosen \
             from its title page."
                .to_owned(),
        ),
        "accounts" => (
            "Accounts",
            "anistream works without an account. Your watch history stays on this computer when \
             you sign out."
                .to_owned(),
        ),
        "integrations" => (
            "Discord presence",
            if on(SettingKey::Presence) {
                "Your Discord profile shows what you watch while an episode plays."
            } else {
                "Off. When on, your Discord profile shows what you watch while an episode plays."
            }
            .to_owned(),
        ),
        "advanced" => (
            "Advanced",
            "An option here can undo a Playback setting, such as upscaling or night mode. Most \
             people never need this pane."
                .to_owned(),
        ),
        _ => return None,
    })
}

/// The column's content for `topic`, or `None` when the pane has nothing to say.
pub fn content(topic: Topic, known: &Known) -> Option<gtk::Widget> {
    let value = |key| known.setting(key).map(|r| r.value).unwrap_or_default();
    Some(match topic {
        Topic::Upscaling | Topic::Pane("playback") => {
            upscaling(&value(SettingKey::Upscaling)).upcast()
        }
        Topic::Subtitles => caption(&value(SettingKey::Subtitles)).upcast(),
        Topic::Pane(pane) => {
            let (title, text) = pane_note(pane, known)?;
            let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
            column.append(&heading(title));
            column.append(&note(&text));
            column.upcast()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(width: usize, height: usize, value: u8) -> Frame {
        Frame { width, height, rgba: vec![value; width * height * 4] }
    }

    #[test]
    fn the_split_puts_the_source_first_and_a_rule_between() {
        let rgba = split(&plain(6, 1, 10), &plain(6, 1, 200));
        let at = |x: usize| rgba[x * 4];
        assert_eq!((at(0), at(1)), (10, 10));
        assert_eq!((at(2), at(3)), (0xE4, 0xE4));
        assert_eq!((at(4), at(5)), (200, 200));
    }

    #[test]
    fn modes_read_by_their_setting_value() {
        assert_eq!(mode_words("natural").0, "natural · ArtCNN");
        assert_eq!(mode_words("sharp").0, "sharp · Anime4K");
        assert_eq!(mode_words("off").0, "off");
        assert_eq!(topic_of(SettingKey::Upscaling), Some(Topic::Upscaling));
        assert_eq!(topic_of(SettingKey::Quality), None);
    }
}
