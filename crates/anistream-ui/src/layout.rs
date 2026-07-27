//! Frame geometry.
//!
//! Pure arithmetic, separated from drawing so the layout rules can be tested without a
//! terminal. The composition is an asymmetric catalogue spread: a narrow rail, one vertical
//! hairline, and a wide stage that gets everything else.
//!
//! Degradation is a first-class concern rather than an afterthought. A terminal can be any
//! size, and the rule is that narrow windows lose *chrome* before they lose *content* — the
//! rail collapses, then disappears, so the stage keeps enough room to stay readable.

use ratatui::layout::Rect;

use crate::nav::RailWidth;

/// Below this total width the rail is forced to collapse regardless of the view's wishes.
pub const NARROW_WIDTH: u16 = 90;

/// Below this the rail is dropped entirely.
pub const VERY_NARROW_WIDTH: u16 = 60;

/// The stage is never squeezed below this; the rail yields first.
pub const MIN_STAGE_WIDTH: u16 = 40;

/// Resolved regions for one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// Title and global status, one line at the top.
    pub header: Rect,
    /// Hairline under the header.
    pub header_rule: Rect,
    /// The rail, or zero-width when hidden.
    pub rail: Rect,
    /// The vertical hairline between rail and stage, or zero-width when there is no rail.
    pub divider: Rect,
    /// Everything else.
    pub stage: Rect,
    /// Hairline above the status line.
    pub status_rule: Rect,
    /// State on the left, a few contextual hints on the right.
    pub status: Rect,
}

impl Frame {
    pub fn has_rail(&self) -> bool {
        self.rail.width > 0
    }
}

/// Compute the frame for an area and a requested rail width.
///
/// The requested width is a *preference*: a narrow terminal overrides it, because the stage
/// staying readable matters more than the rail staying visible.
pub fn compute(area: Rect, requested: RailWidth) -> Frame {
    // Reserve the fixed chrome rows first: header, its rule, the status rule, and status.
    // On a very short terminal these are given up from the bottom, so the stage always
    // exists even if the chrome does not.
    let has_header = area.height >= 3;
    let has_status = area.height >= 5;

    let header_h = u16::from(has_header);
    let header_rule_h = u16::from(has_header);
    let status_h = u16::from(has_status);
    let status_rule_h = u16::from(has_status);

    let header = Rect { x: area.x, y: area.y, width: area.width, height: header_h };
    let header_rule = Rect { y: area.y + header_h, height: header_rule_h, ..header };

    let body_y = area.y + header_h + header_rule_h;
    let body_h =
        area.height.saturating_sub(header_h + header_rule_h + status_rule_h + status_h);

    let rail_width = effective_rail_width(area.width, requested);
    // The divider only exists when there is something to divide.
    let divider_width = u16::from(rail_width > 0);

    let rail = Rect { x: area.x, y: body_y, width: rail_width, height: body_h };
    let divider =
        Rect { x: area.x + rail_width, y: body_y, width: divider_width, height: body_h };
    let stage = Rect {
        x: area.x + rail_width + divider_width,
        y: body_y,
        width: area.width.saturating_sub(rail_width + divider_width),
        height: body_h,
    };

    let status_rule_y = body_y + body_h;
    let status_rule =
        Rect { x: area.x, y: status_rule_y, width: area.width, height: status_rule_h };
    let status = Rect {
        x: area.x,
        y: status_rule_y + status_rule_h,
        width: area.width,
        height: status_h,
    };

    Frame { header, header_rule, rail, divider, stage, status_rule, status }
}

/// Resolve the rail width against the available space.
pub fn effective_rail_width(total_width: u16, requested: RailWidth) -> u16 {
    if requested == RailWidth::Hidden || total_width < VERY_NARROW_WIDTH {
        return 0;
    }

    let wanted = if total_width < NARROW_WIDTH {
        // Not enough room for a comfortable spread; keep the rail as a strip.
        RailWidth::Collapsed.cells()
    } else {
        requested.cells()
    };

    // The stage never goes below its floor — the rail gives way first.
    let max_affordable = total_width.saturating_sub(MIN_STAGE_WIDTH + 1);
    wanted.min(max_affordable)
}

/// Split a stage row into a left column and a right column at a golden-ish ratio.
///
/// Used where a screen wants its own internal spread, such as the Title screen's metadata
/// beside its synopsis.
pub fn split_stage(area: Rect, left_fraction: f32) -> (Rect, Rect) {
    let left_width = ((area.width as f32) * left_fraction).round() as u16;
    let left_width = left_width.min(area.width);
    (
        Rect { width: left_width, ..area },
        Rect { x: area.x + left_width, width: area.width.saturating_sub(left_width), ..area },
    )
}

/// Shrink a rect by a uniform margin, saturating rather than panicking.
pub fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    Rect {
        x: area.x + horizontal.min(area.width / 2),
        y: area.y + vertical.min(area.height / 2),
        width: area.width.saturating_sub(horizontal * 2),
        height: area.height.saturating_sub(vertical * 2),
    }
}

/// A horizontal band for modal overlays.
///
/// Overlays are bands rather than centred boxes with borders: the design has no borders
/// anywhere, so a modal is a ground shift plus an obi bar and two hairlines.
///
/// The band spans the **full width** deliberately. A partial-width panel leaves live
/// content visible either side of it, which reads as a rendering fault rather than a
/// deliberate modal — and edge-to-edge is truer to the obi it is named after.
pub fn overlay_band(area: Rect, height: u16) -> Rect {
    let height = height.min(area.height);
    Rect {
        x: area.x,
        // A third of the way down rather than centred: it reads better, and leaves the
        // content below still legible as context.
        y: area.y + (area.height.saturating_sub(height)) / 3,
        width: area.width,
        height,
    }
}

/// How many columns of `min_column_width` fit in a band.
///
/// Used by the help overlay: a single column of forty-odd bindings does not fit any
/// reasonable terminal, and clipping the list mid-scope would hide whole categories.
pub fn columns_for(width: u16, min_column_width: u16) -> usize {
    if min_column_width == 0 {
        return 1;
    }
    ((width / min_column_width) as usize).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect { x: 0, y: 0, width, height }
    }

    #[test]
    fn regions_tile_the_area_without_overlap_or_gaps() {
        let a = area(120, 40);
        let f = compute(a, RailWidth::Expanded);

        // Rows account for the full height.
        let rows = f.header.height
            + f.header_rule.height
            + f.stage.height
            + f.status_rule.height
            + f.status.height;
        assert_eq!(rows, a.height);

        // Columns across the body account for the full width.
        assert_eq!(f.rail.width + f.divider.width + f.stage.width, a.width);
        assert_eq!(f.rail.x, 0);
        assert_eq!(f.divider.x, f.rail.width);
        assert_eq!(f.stage.x, f.rail.width + f.divider.width);
    }

    #[test]
    fn the_divider_is_exactly_one_cell_when_a_rail_exists() {
        // It is a hairline, not a border. Two cells would read as a panel edge.
        let f = compute(area(120, 40), RailWidth::Expanded);
        assert_eq!(f.divider.width, 1);
        assert!(f.has_rail());
    }

    #[test]
    fn hiding_the_rail_also_removes_the_divider() {
        // A divider with nothing on its left would be a stray vertical line.
        let f = compute(area(120, 40), RailWidth::Hidden);
        assert_eq!(f.rail.width, 0);
        assert_eq!(f.divider.width, 0);
        assert_eq!(f.stage.width, 120);
        assert!(!f.has_rail());
    }

    #[test]
    fn the_expanded_rail_is_the_designed_width_when_there_is_room() {
        let f = compute(area(120, 40), RailWidth::Expanded);
        assert_eq!(f.rail.width, RailWidth::Expanded.cells());
    }

    #[test]
    fn a_narrow_terminal_collapses_the_rail_before_squeezing_the_stage() {
        // Chrome yields to content — that is the rule.
        let f = compute(area(80, 30), RailWidth::Expanded);
        assert_eq!(f.rail.width, RailWidth::Collapsed.cells());
        assert!(f.stage.width >= MIN_STAGE_WIDTH);
    }

    #[test]
    fn a_very_narrow_terminal_drops_the_rail_entirely() {
        let f = compute(area(50, 20), RailWidth::Expanded);
        assert_eq!(f.rail.width, 0);
        assert_eq!(f.stage.width, 50);
    }

    #[test]
    fn the_stage_never_falls_below_its_floor() {
        // The failure this prevents: a rail eating the screen until content is unreadable.
        for width in VERY_NARROW_WIDTH..200 {
            let f = compute(area(width, 30), RailWidth::Expanded);
            assert!(
                f.stage.width >= MIN_STAGE_WIDTH,
                "at width {width} the stage was only {}",
                f.stage.width
            );
        }
    }

    #[test]
    fn an_80x24_terminal_still_has_a_usable_stage() {
        // The classic default size has to work.
        let f = compute(area(80, 24), RailWidth::Expanded);
        assert!(f.stage.width >= 70);
        assert!(f.stage.height >= 18);
        assert_eq!(f.status.height, 1);
    }

    #[test]
    fn a_very_short_terminal_gives_up_chrome_rather_than_the_stage() {
        let f = compute(area(80, 4), RailWidth::Expanded);
        assert_eq!(f.status.height, 0, "status is dropped first");
        assert!(f.stage.height >= 1, "the stage must always exist");

        let tiny = compute(area(80, 2), RailWidth::Expanded);
        assert_eq!(tiny.header.height, 0);
        assert!(tiny.stage.height >= 1);
    }

    #[test]
    fn degenerate_sizes_do_not_panic_or_overflow() {
        for (w, h) in [(0, 0), (1, 1), (1, 100), (100, 1), (u16::MAX, 3)] {
            let f = compute(area(w, h), RailWidth::Expanded);
            assert!(f.stage.width <= w);
            // Regions must tile the width exactly, with no overflow at the extremes.
            assert_eq!(
                f.rail.width as u32 + f.divider.width as u32 + f.stage.width as u32,
                u32::from(w),
                "regions did not tile {w}x{h}"
            );
        }
    }

    #[test]
    fn splitting_the_stage_preserves_total_width() {
        let a = Rect { x: 10, y: 2, width: 81, height: 20 };
        let (left, right) = split_stage(a, 0.31);
        assert_eq!(left.width + right.width, a.width);
        assert_eq!(right.x, left.x + left.width);
        assert_eq!(left.x, a.x);
    }

    #[test]
    fn insetting_saturates_instead_of_underflowing() {
        let tiny = Rect { x: 0, y: 0, width: 2, height: 1 };
        let r = inset(tiny, 4, 4);
        assert_eq!(r.width, 0);
        assert_eq!(r.height, 0);
    }

    #[test]
    fn an_overlay_band_stays_inside_its_area_and_sits_above_centre() {
        let a = area(120, 40);
        let band = overlay_band(a, 12);
        // Full width by design: a partial-width panel leaves live content visible either
        // side of it, which reads as a rendering fault rather than a modal.
        assert_eq!(band.width, a.width);
        assert_eq!(band.x, a.x);
        assert!(band.y + band.height <= a.y + a.height);
        // Positioned a third of the way down rather than dead centre.
        assert!(band.y < a.height / 2);
    }

    #[test]
    fn column_count_scales_with_width_and_never_reaches_zero() {
        // A long keymap needs columns or it clips mid-scope, hiding whole categories.
        assert_eq!(columns_for(120, 46), 2);
        assert_eq!(columns_for(200, 46), 4);
        assert_eq!(columns_for(40, 46), 1, "must never divide by zero columns");
        assert_eq!(columns_for(0, 46), 1);
        assert_eq!(columns_for(100, 0), 1);
    }

    #[test]
    fn an_overlay_band_fits_a_tiny_terminal() {
        let a = area(24, 6);
        let band = overlay_band(a, 20);
        assert!(band.height <= a.height, "must not exceed a short terminal");
        assert!(band.x + band.width <= a.width);
    }
}
