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

    /// The colour as the terminal is sent it, at the depth it was found to draw.
    pub fn to_ratatui(self) -> ratatui::style::Color {
        match depth() {
            Depth::TrueColor => ratatui::style::Color::Rgb(self.r, self.g, self.b),
            Depth::Indexed => ratatui::style::Color::Indexed(self.nearest_indexed()),
            Depth::Basic => self.nearest_basic(),
            Depth::NoColor => ratatui::style::Color::Reset,
        }
    }

    fn distance(self, other: Self) -> u32 {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2) as u32;
        d(self.r, other.r) + d(self.g, other.g) + d(self.b, other.b)
    }

    /// The nearest of the 256-colour palette's cube and grey ramp, leaving the first sixteen,
    /// which each terminal theme sets its own way.
    fn nearest_indexed(self) -> u8 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let level = |v: u8| {
            (0..6).min_by_key(|&i| (i32::from(LEVELS[i]) - i32::from(v)).abs()).unwrap_or(0)
        };
        let (r, g, b) = (level(self.r), level(self.g), level(self.b));
        let cube = Self::new(LEVELS[r], LEVELS[g], LEVELS[b]);
        let cube_index = 16 + 36 * r as u8 + 6 * g as u8 + b as u8;
        let mean = (u16::from(self.r) + u16::from(self.g) + u16::from(self.b)) / 3;
        let step = (mean.saturating_sub(3) / 10).min(23) as u8;
        let grey = 8 + 10 * step;
        if self.distance(Self::new(grey, grey, grey)) < self.distance(cube) {
            232 + step
        } else {
            cube_index
        }
    }

    /// The sixteen named colours by what they are rather than by distance, which sends every
    /// muted colour to grey: a grey by its lightness, anything else by its hue, bright when
    /// the colour is.
    fn nearest_basic(self) -> ratatui::style::Color {
        use ratatui::style::Color as C;
        let (r, g, b) = (i32::from(self.r), i32::from(self.g), i32::from(self.b));
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        if max - min < 40 {
            return match (r + g + b) / 3 {
                0..50 => C::Black,
                50..150 => C::DarkGray,
                150..215 => C::Gray,
                _ => C::White,
            };
        }
        let chroma = f64::from(max - min);
        let hue = if max == r {
            60.0 * (f64::from(g - b) / chroma).rem_euclid(6.0)
        } else if max == g {
            60.0 * (f64::from(b - r) / chroma + 2.0)
        } else {
            60.0 * (f64::from(r - g) / chroma + 4.0)
        };
        let bright = max > 200;
        let hues = [
            (C::Red, C::LightRed),
            (C::Yellow, C::LightYellow),
            (C::Green, C::LightGreen),
            (C::Cyan, C::LightCyan),
            (C::Blue, C::LightBlue),
            (C::Magenta, C::LightMagenta),
        ];
        let (dim, light) = hues[((hue + 30.0) / 60.0) as usize % 6];
        if bright { light } else { dim }
    }
}

/// How many colours the terminal draws, found once at start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// 24-bit colour: what the palette is written in.
    TrueColor,
    /// The 256-colour palette.
    Indexed,
    /// The sixteen named colours.
    Basic,
    /// `NO_COLOR`, or a terminal that draws none: emphasis by weight and reverse video.
    NoColor,
}

static DEPTH: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Set the depth every colour is drawn at, once, before the first frame.
pub fn set_depth(depth: Depth) {
    let n = match depth {
        Depth::TrueColor => 0,
        Depth::Indexed => 1,
        Depth::Basic => 2,
        Depth::NoColor => 3,
    };
    DEPTH.store(n, std::sync::atomic::Ordering::Relaxed);
}

/// The depth colours are drawn at: true colour unless [`set_depth`] said otherwise.
pub fn depth() -> Depth {
    match DEPTH.load(std::sync::atomic::Ordering::Relaxed) {
        1 => Depth::Indexed,
        2 => Depth::Basic,
        3 => Depth::NoColor,
        _ => Depth::TrueColor,
    }
}

/// The depth the environment says the terminal draws: `NO_COLOR` first, then `COLORTERM`,
/// then `TERM`. `var` reads one variable, so this is tested without touching the process's.
pub fn detect_depth(var: impl Fn(&str) -> Option<String>) -> Depth {
    if var("NO_COLOR").is_some_and(|v| !v.is_empty()) {
        return Depth::NoColor;
    }
    let colorterm = var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
    if matches!(colorterm.as_str(), "truecolor" | "24bit") || var("WT_SESSION").is_some() {
        return Depth::TrueColor;
    }
    let term = var("TERM").unwrap_or_default().to_ascii_lowercase();
    match term.as_str() {
        "dumb" => Depth::NoColor,
        // No TERM is a console that never sets one; the host draws what it is sent.
        "" => Depth::TrueColor,
        t if t.contains("direct") || t.contains("truecolor") => Depth::TrueColor,
        t if t.contains("256") => Depth::Indexed,
        _ => Depth::Basic,
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
    fn the_depth_follows_no_color_then_colorterm_then_term() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |key: &str| pairs.iter().find(|(k, _)| *k == key).map(|(_, v)| (*v).to_owned())
        };
        assert_eq!(
            detect_depth(env(&[("NO_COLOR", "1"), ("COLORTERM", "truecolor")])),
            Depth::NoColor
        );
        assert_eq!(
            detect_depth(env(&[("NO_COLOR", ""), ("COLORTERM", "truecolor")])),
            Depth::TrueColor
        );
        assert_eq!(detect_depth(env(&[("TERM", "xterm-256color")])), Depth::Indexed);
        assert_eq!(
            detect_depth(env(&[("TERM", "xterm-256color"), ("COLORTERM", "24bit")])),
            Depth::TrueColor
        );
        assert_eq!(detect_depth(env(&[("TERM", "xterm")])), Depth::Basic);
        assert_eq!(detect_depth(env(&[("TERM", "dumb")])), Depth::NoColor);
    }

    #[test]
    fn a_colour_falls_back_to_its_nearest_in_each_smaller_palette() {
        let amber = Rgb::from_hex(0xF2A64B);
        assert_eq!(amber.nearest_indexed(), 215, "the cube's orange");
        assert_eq!(Rgb::new(128, 128, 128).nearest_indexed(), 244, "a grey takes the ramp");
        use ratatui::style::Color as C;
        // The dark palette: amber, salmon, water, then the text, dim text and hairline greys.
        let basic = [0xF2A64B, 0xF59B85, 0x7FC7D9, 0xE4E0D6, 0xA8AEC9, 0x46464D]
            .map(|hex| Rgb::from_hex(hex).nearest_basic());
        assert_eq!(
            basic,
            [C::LightYellow, C::LightRed, C::LightCyan, C::White, C::Gray, C::DarkGray]
        );
    }

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
