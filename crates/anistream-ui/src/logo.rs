//! The pixel mark: the app icon drawn in half-block cells.
//!
//! Deliberately not the image pipeline. A logo that only appears on Kitty-class
//! terminals is a logo most terminals never see; two stacked pixels per character cell
//! render anywhere colour does, and a pixel grid is the one aesthetic a terminal is
//! natively good at. It appears on empty states only — a printer's mark on the blank
//! page, never competing with content.

use std::sync::OnceLock;

use ratatui::{buffer::Buffer, layout::Rect};

use crate::theme::Rgb;

/// The 16×16 icon, embedded so the mark cannot go missing at runtime.
const ICON: &[u8] = include_bytes!("../../../assets/icon/16x16.png");

/// Pixel grid, row-major; `None` is transparency.
type Grid = Vec<Vec<Option<Rgb>>>;

fn grid() -> &'static Grid {
    static GRID: OnceLock<Grid> = OnceLock::new();
    GRID.get_or_init(|| {
        // A decode failure yields an empty grid and the mark simply does not draw —
        // an empty state without a logo is not an error worth surfacing.
        let Ok(decoded) = image::load_from_memory(ICON) else { return Vec::new() };
        let rgba = decoded.to_rgba8();
        (0..rgba.height())
            .map(|y| {
                (0..rgba.width())
                    .map(|x| {
                        let p = rgba.get_pixel(x, y);
                        (p[3] >= 128).then(|| Rgb::new(p[0], p[1], p[2]))
                    })
                    .collect()
            })
            .collect()
    })
}

/// Character-cell footprint of the mark: `(width, height)`.
pub fn size() -> (u16, u16) {
    let grid = grid();
    let height = grid.len() as u16;
    let width = grid.first().map_or(0, |row| row.len() as u16);
    (width, height.div_ceil(2))
}

/// Draw the mark with its top-left at `(x, y)`, clipped to `area`.
///
/// Each cell carries two vertically stacked pixels: `▀` with the top pixel as
/// foreground and the bottom as background. Transparent halves fall back to a plain
/// half block over the terminal's own ground, so the mark keeps its silhouette on any
/// background.
pub fn render(buf: &mut Buffer, area: Rect, x: u16, y: u16) {
    for (cell_row, rows) in grid().chunks(2).enumerate() {
        let top_row = &rows[0];
        for (cell_col, top) in top_row.iter().enumerate() {
            let cx = x + cell_col as u16;
            let cy = y + cell_row as u16;
            if cx >= area.right() || cy >= area.bottom() {
                continue;
            }
            let bottom = rows.get(1).and_then(|r| r.get(cell_col).copied().flatten());
            let cell = &mut buf[(cx, cy)];
            match (top, bottom) {
                (Some(top), Some(bottom)) => {
                    cell.set_char('▀').set_fg(top.to_ratatui()).set_bg(bottom.to_ratatui());
                }
                (Some(top), None) => {
                    cell.set_char('▀').set_fg(top.to_ratatui());
                }
                (None, Some(bottom)) => {
                    cell.set_char('▄').set_fg(bottom.to_ratatui());
                }
                (None, None) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_icon_decodes_to_a_square_pixel_grid() {
        let grid = grid();
        assert!(!grid.is_empty(), "the embedded icon must decode");
        assert!(grid.iter().all(|row| row.len() == grid.len()), "the icon is square");
        let (w, h) = size();
        assert_eq!(w, grid.len() as u16);
        assert_eq!(h, (grid.len() as u16).div_ceil(2), "two pixels per cell");
    }

    #[test]
    fn the_mark_never_draws_outside_its_area() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 10, 4));
        // An area smaller than the mark: rendering must clip, not panic.
        render(&mut buf, Rect::new(0, 0, 10, 4), 0, 0);
    }
}
