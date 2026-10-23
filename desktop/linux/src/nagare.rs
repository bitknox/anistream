//! Nagare 流, the mascot: the hand-authored pixel sprite, 20 by 32, the same one the website and
//! the other apps draw. Shown in empty states and on first launch; never in navigation, the
//! player, or beside a show's art. Static, on the plain ground, each sprite pixel a whole number
//! of device pixels, so the figure stays crisp.

use adw::prelude::*;
use gtk::{gdk, glib};

pub const WIDTH: usize = 20;
pub const HEIGHT: usize = 32;

/// A row per line, a character per pixel, `.` for none.
const ROWS: [&str; HEIGHT] = [
    "........hH..........",
    ".........Hh.........",
    "........hH..........",
    "......hHHHHh........",
    ".....hHHHHHHh.......",
    "....hHHHHHHHHh......",
    "....HHHHHHHHHHh.....",
    "...hHFFFFhHHHHh.....",
    "...HFFFFFFhHHHh.....",
    "...HFEFFFFhHHHh.....",
    "...hFFFFFfhHHh......",
    ".ww.fFFFfhHHh.......",
    ".wWWAAAAAAHHh.......",
    ".wWaAAAAAAa..aAA....",
    "..w.aAAAa....aAAa...",
    "..SRCCCCCCc.........",
    "..SRCCCCCCCc........",
    "..SRCCCCCCCc........",
    "..SfFCCCCCCCc.......",
    "..SRCCCCCCCCc.......",
    "..SRCCCCCCCCc.......",
    "..SRCCCCCCCCcc......",
    "..SRCCCCCCCCCc......",
    "..SRCCCCCCCCCc......",
    "..SRCCCCCCCCCcc.....",
    "..SRCCCCCCCCCCc.....",
    "..SRCCCCCCCCCCc.....",
    "..SRCCCCCCCCCCcc....",
    "..SRCCCCCCCCCCCc....",
    "..S.BB......BB......",
    "....BB......BB......",
    "...BBB.....BBB......",
];

pub const DESCRIPTION: &str = "Nagare, the anistream mascot: a figure with a silk bob, an amber \
                               scarf and an indigo cloak, holding a staff with a water charm";

fn color(c: char) -> Option<[u8; 3]> {
    Some(match c {
        'H' => [0xE8, 0xE2, 0xD4],
        'h' => [0xC6, 0xBE, 0xA9],
        'F' => [0xEF, 0xCF, 0xAC],
        'f' => [0xD0, 0xA8, 0x83],
        'E' | 'B' => [0x23, 0x28, 0x42],
        'C' => [0x46, 0x4E, 0x7C],
        'c' => [0x2E, 0x34, 0x5C],
        'R' => [0x68, 0x72, 0x9E],
        'A' => [0xF2, 0xA6, 0x4B],
        'a' => [0xC5, 0x7C, 0x2C],
        'S' => [0x8A, 0x68, 0x44],
        'W' => [0x7F, 0xC7, 0xD9],
        'w' => [0xDF, 0xF5, 0xFB],
        _ => return None,
    })
}

/// The sprite as RGBA, `scale` pixels to a sprite pixel.
pub fn rgba(scale: usize) -> Vec<u8> {
    let (w, h) = (WIDTH * scale, HEIGHT * scale);
    let mut out = vec![0u8; w * h * 4];
    for (y, row) in ROWS.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            let Some([r, g, b]) = color(c) else { continue };
            for dy in 0..scale {
                for dx in 0..scale {
                    let at = ((y * scale + dy) * w + x * scale + dx) * 4;
                    out[at..at + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        }
    }
    out
}

/// Nagare as a texture `cell` pixels to the sprite pixel: drawn at half its size on a display at
/// scale 1, where every output pixel still covers whole sprite pixels, and at full size at 2.
pub fn texture(cell: usize) -> gdk::Texture {
    let scale = cell * 2;
    let bytes = glib::Bytes::from_owned(rgba(scale));
    gdk::MemoryTexture::new(
        (WIDTH * scale) as i32,
        (HEIGHT * scale) as i32,
        gdk::MemoryFormat::R8g8b8a8,
        &bytes,
        WIDTH * scale * 4,
    )
    .upcast()
}

/// An empty state: Nagare, a line and, when given, one action.
pub fn status_page(title: &str, line: &str, action: Option<&gtk::Button>) -> adw::StatusPage {
    let page = adw::StatusPage::builder()
        .title(title)
        .description(line)
        .paintable(&texture(4))
        .build();
    if let Some(action) = action {
        action.set_halign(gtk::Align::Center);
        page.set_child(Some(action));
    }
    page
}

/// Nagare as a picture of his own, `cell` logical pixels to the sprite pixel.
pub fn picture(cell: usize) -> gtk::Picture {
    let picture = gtk::Picture::for_paintable(&texture(cell));
    picture.set_can_shrink(false);
    picture.set_size_request((WIDTH * cell) as i32, (HEIGHT * cell) as i32);
    picture.update_property(&[gtk::accessible::Property::Label(DESCRIPTION)]);
    picture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_the_sprites_width_and_every_pixel_has_a_colour_or_none() {
        for row in ROWS {
            assert_eq!(row.chars().count(), WIDTH, "{row}");
            assert!(row.chars().all(|c| c == '.' || color(c).is_some()), "{row}");
        }
    }

    #[test]
    fn a_scaled_sprite_repeats_each_pixel_whole() {
        let px = rgba(2);
        assert_eq!(px.len(), WIDTH * 2 * HEIGHT * 2 * 4);
        // The top row's 'h' at x 8 covers 16..18 of the first two rows.
        let at = |x: usize, y: usize| &px[(y * WIDTH * 2 + x) * 4..(y * WIDTH * 2 + x) * 4 + 4];
        assert_eq!(at(16, 0), at(17, 1));
        assert_eq!(at(16, 0), &[0xC6, 0xBE, 0xA9, 255]);
        assert_eq!(at(0, 0)[3], 0, "a pixel with no colour is clear");
    }
}
