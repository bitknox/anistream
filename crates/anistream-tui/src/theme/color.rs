//! Colour arithmetic: sRGB luminance and WCAG contrast.
//!
//! This exists so the palette's contrast claims are *checked* rather than asserted. The
//! values in [`super::palette`] were chosen against these functions, and a test drives
//! every role/variant pair through [`contrast_ratio`] — so the palette cannot regress
//! into something unreadable without a test failing.

/// An 8-bit-per-channel sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#RRGGBB` or `RRGGBB`.
    pub const fn from_hex(hex: u32) -> Self {
        Self {
            r: ((hex >> 16) & 0xFF) as u8,
            g: ((hex >> 8) & 0xFF) as u8,
            b: (hex & 0xFF) as u8,
        }
    }

    /// Relative luminance per WCAG 2.x, in `0.0..=1.0`.
    pub fn luminance(self) -> f64 {
        fn channel(v: u8) -> f64 {
            let v = f64::from(v) / 255.0;
            if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    pub const fn to_ratatui(self) -> ratatui::style::Color {
        ratatui::style::Color::Rgb(self.r, self.g, self.b)
    }
}

impl From<Rgb> for ratatui::style::Color {
    fn from(c: Rgb) -> Self {
        c.to_ratatui()
    }
}

/// WCAG contrast ratio between two colours, from `1.0` (identical) to `21.0`.
pub fn contrast_ratio(a: Rgb, b: Rgb) -> f64 {
    contrast_from_luminance(a.luminance(), b.luminance())
}

/// Contrast between two known luminances.
///
/// Taking luminance directly matters for the adaptive variants: the terminal's background
/// is *unknown*, so the palette has to be validated against a whole range of possible
/// grounds rather than one sample colour.
pub fn contrast_from_luminance(a: f64, b: f64) -> f64 {
    let (hi, lo) = if a > b { (a, b) } else { (b, a) };
    (hi + 0.05) / (lo + 0.05)
}

/// Linearly interpolate between two colours in sRGB space.
///
/// `t = 0.0` yields `from`, `t = 1.0` yields `to`. Used to derive hairline colours as a
/// fixed perceptual step away from whatever ground the terminal actually has, which is
/// the only way a subtle rule can stay visible across an unknown range of backgrounds.
pub fn mix(from: Rgb, to: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8| (f64::from(a) * (1.0 - t) + f64::from(b) * t).round() as u8;
    Rgb { r: lerp(from.r, to.r), g: lerp(from.g, to.g), b: lerp(from.b, to.b) }
}

/// WCAG AA floor for normal-size text.
///
/// Everything in a terminal is normal-size text — there is no "large text" exemption to
/// hide behind — so this is the bar for every role that renders glyphs.
pub const AA_NORMAL: f64 = 4.5;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luminance_endpoints_are_right() {
        assert!((Rgb::from_hex(0x000000).luminance() - 0.0).abs() < 1e-9);
        assert!((Rgb::from_hex(0xFFFFFF).luminance() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn contrast_is_symmetric_and_bounded() {
        let black = Rgb::from_hex(0x000000);
        let white = Rgb::from_hex(0xFFFFFF);
        let ratio = contrast_ratio(black, white);
        assert!((ratio - 21.0).abs() < 0.01, "black on white is 21:1, got {ratio}");
        assert!((contrast_ratio(white, black) - ratio).abs() < 1e-9);
        assert!((contrast_ratio(white, white) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn hex_parsing_splits_channels_correctly() {
        assert_eq!(Rgb::from_hex(0xF2A64B), Rgb::new(0xF2, 0xA6, 0x4B));
    }
}
