//! The glyph vocabulary.
//!
//! Collected in one place because these characters *are* the design. With every panel
//! border deleted, structure has to come from hairlines, negative space and the obi bar,
//! so the exact glyphs matter more than they would in a bordered layout.
//!
//! All of these are Unicode box-drawing, block or geometric characters, present in any
//! reasonable monospace font. Nothing here needs a patched nerd font, which stays an
//! opt-in decoration rather than a dependency.

/// The obi (帯): a one-cell vertical bar marking focus. The signature element.
pub const OBI: char = '▌';

/// A thinner bar, for secondary emphasis such as episode counts in the rail.
pub const OBI_THIN: char = '▏';

/// Horizontal hairline. Separates sections in place of a border.
pub const RULE_H: char = '─';

/// Vertical hairline. The single divider between rail and stage.
pub const RULE_V: char = '│';

/// Progress meter cells: filled and empty.
pub const METER_FULL: char = '█';
pub const METER_EMPTY: char = '░';

/// Eighth-block ramp, for sub-cell precision in progress meters.
pub const RAMP: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];

/// Health and status indicators. Geometric shapes rather than emoji: emoji render at
/// inconsistent widths, break alignment in a character grid, and read as templated.
pub const STATE_READY: char = '●';
pub const STATE_DEGRADED: char = '▲';
pub const STATE_DOWN: char = '✕';
pub const STATE_UNKNOWN: char = '·';

/// Sync indicator for the status line.
pub const SYNC: char = '⇅';

/// Back-navigation marker used in pushed-view headers.
pub const BACK: char = '←';

/// Render a progress meter of `width` cells at `fraction` complete.
///
/// Uses the eighth-block ramp for the partial cell, so a meter moves smoothly instead of
/// jumping a whole character at a time — the detail that separates a progress bar that
/// feels alive from one that feels like a checkbox.
pub fn meter(fraction: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let fraction = fraction.clamp(0.0, 1.0);
    let total_eighths = (fraction * (width * 8) as f64).round() as usize;
    let full = total_eighths / 8;
    let remainder = total_eighths % 8;

    let mut out = String::with_capacity(width * 3);
    for _ in 0..full.min(width) {
        out.push(METER_FULL);
    }
    if full < width && remainder > 0 {
        out.push(RAMP[remainder - 1]);
    }
    let rendered = full.min(width) + usize::from(full < width && remainder > 0);
    for _ in rendered..width {
        out.push(METER_EMPTY);
    }
    out
}

/// An eyebrow: a small label above or beside content, set in caps.
///
/// This used to letterspace as well — `M A D H O U S E` — on the theory that tracking is a
/// typographic lever almost no TUI uses. It is unused for a reason. Read on a real terminal
/// it is tiring rather than refined, and it is worst on exactly the content that needs it
/// least: digits. `2 8 E P · 2 0 2 6 · 8 5` defeats the grouping that makes a number
/// scannable at a glance, and doubling every label's width crowds out the content beside it.
///
/// So the distinction between an eyebrow and body text is carried by case, by the dim role
/// and by the interpunct separators instead — all of which cost no width at all.
pub fn eyebrow(text: &str) -> String {
    text.to_uppercase()
}

/// Shade ramp, darkest to brightest. Unlike [`RAMP`] these vary in *density* rather than
/// width, so a sequence of them reads as one cell brightening rather than a bar growing.
pub const SHADES: [char; 4] = ['░', '▒', '▓', '█'];

/// Cells in the shared loading pulse.
pub const PULSE_CELLS: usize = 3;

/// The one loading indicator in the app: a three-cell wave that rotates in place.
///
/// Deliberately not a spinner. A spinning ASCII baton is the single most recognisable
/// templated-TUI tell, and it also implies *progress* that nothing here can measure — a
/// stream resolve either answers or it does not. A wave says "busy" without lying about how
/// far along it is, and it is built from the same block glyphs as the obi and the meters, so
/// it belongs to this design rather than being borrowed from every other one.
pub fn pulse(frame: u64) -> String {
    // Three shades, brightest leading, rotated by the frame. The period is PULSE_CELLS, so
    // at the UI's 100 ms cadence the wave travels a full cycle in 300 ms.
    let head = (frame as usize) % PULSE_CELLS;
    (0..PULSE_CELLS)
        .map(|cell| {
            // Distance behind the head, wrapping — the bright cell trails off into shade.
            let behind = (cell + PULSE_CELLS - head) % PULSE_CELLS;
            SHADES[SHADES.len() - 1 - behind]
        })
        .collect()
}

/// Join metadata fragments with the interpunct separator used across the UI.
pub fn dotted(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("  ·  ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_is_always_exactly_the_requested_width() {
        // Width in *cells*, not bytes — a ragged meter would break the table alignment
        // that the borderless layout depends on.
        for width in [1_usize, 3, 10, 20] {
            for pct in [0.0, 0.01, 0.33, 0.5, 0.99, 1.0] {
                let m = meter(pct, width);
                assert_eq!(m.chars().count(), width, "meter({pct}, {width}) rendered {m:?}");
            }
        }
        assert_eq!(meter(0.5, 0), "");
    }

    #[test]
    fn meter_endpoints_are_fully_empty_and_fully_full() {
        assert_eq!(meter(0.0, 4), "░░░░");
        assert_eq!(meter(1.0, 4), "████");
    }

    #[test]
    fn meter_uses_partial_cells_for_smooth_movement() {
        // 1/8 of a 1-cell meter should show the narrowest ramp glyph, not an empty cell.
        assert_eq!(meter(0.125, 1), "▏");
        assert!(meter(0.5, 2).starts_with('█'));
    }

    #[test]
    fn meter_clamps_out_of_range_input() {
        assert_eq!(meter(-1.0, 3), "░░░");
        assert_eq!(meter(5.0, 3), "███");
    }

    #[test]
    fn an_eyebrow_is_caps_and_nothing_else() {
        // Specifically *not* letterspaced. The width matters: a tracked label is twice as
        // wide, and in a fixed grid that width comes straight out of the content beside it.
        assert_eq!(eyebrow("Madhouse"), "MADHOUSE");
        assert_eq!(eyebrow("28 EP"), "28 EP");
        assert_eq!(eyebrow(""), "");
    }

    #[test]
    fn the_pulse_is_always_three_cells_and_actually_moves() {
        let frames: Vec<String> = (0..PULSE_CELLS as u64 + 1).map(pulse).collect();
        for frame in &frames {
            assert_eq!(frame.chars().count(), PULSE_CELLS, "{frame:?} is the wrong width");
        }
        assert_ne!(frames[0], frames[1], "a still frame is not an animation");
        assert_eq!(frames[0], frames[PULSE_CELLS], "the wave must cycle cleanly");
    }

    #[test]
    fn the_pulse_has_exactly_one_bright_head() {
        for frame in 0..8 {
            let rendered = pulse(frame);
            let heads = rendered.chars().filter(|c| *c == METER_FULL).count();
            assert_eq!(heads, 1, "{rendered:?} should have a single leading cell");
        }
    }

    #[test]
    fn dotted_skips_empty_fragments() {
        assert_eq!(dotted(&["TV", "", "28 EP"]), "TV  ·  28 EP");
        assert_eq!(dotted(&[]), "");
    }

    #[test]
    fn no_glyph_is_emoji_width() {
        // Emoji break a character grid. Every indicator must be a single-cell char.
        for g in [
            OBI,
            OBI_THIN,
            RULE_H,
            RULE_V,
            METER_FULL,
            METER_EMPTY,
            STATE_READY,
            STATE_DEGRADED,
            STATE_DOWN,
            STATE_UNKNOWN,
            SYNC,
            BACK,
        ] {
            assert!(!g.is_ascii(), "{g:?} should be a drawing char");
            assert!((g as u32) < 0x1F000, "{g:?} is in emoji planes and will break alignment");
        }
    }
}
