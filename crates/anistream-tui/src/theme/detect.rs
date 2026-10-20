//! Terminal background detection over OSC 11.
//!
//! Adaptive mode is the default, which means the palette has to know whether it is
//! sitting on a dark or a light ground. Guessing from `$TERM` or an environment variable
//! is unreliable, so the terminal is asked directly: OSC 11 is a real query that most
//! modern terminals answer with their actual background RGB.
//!
//! Every failure path here degrades rather than erroring. A terminal that ignores the
//! query, a pipe instead of a tty, a CI runner — all of these produce a sensible dark
//! default rather than a crash, because the theme is not worth failing startup over.

use anistream_core::config::ThemeMode;
use terminal_colorsaurus::QueryOptions;

use super::{
    color::Rgb,
    palette::{Palette, Variant, select},
};

/// Ask the terminal for its background colour.
///
/// `None` means "no answer" — not an error. Terminals that don't implement OSC 11 simply
/// stay quiet, and non-tty output has nothing to ask.
pub fn detect_background() -> Option<Rgb> {
    match terminal_colorsaurus::background_color(QueryOptions::default()) {
        Ok(color) => {
            let (r, g, b) = color.scale_to_8bit();
            let rgb = Rgb::new(r, g, b);
            tracing::debug!(
                r,
                g,
                b,
                luminance = rgb.luminance(),
                "detected terminal background"
            );
            Some(rgb)
        }
        Err(e) => {
            tracing::debug!(error = %e, "terminal did not report a background colour");
            None
        }
    }
}

/// Resolve the palette to use, querying the terminal when the mode calls for it.
///
/// Immersive mode skips detection entirely — we paint the ground, so what the terminal
/// would have used is irrelevant.
pub fn resolve(mode: ThemeMode) -> Palette {
    let detected = match mode {
        ThemeMode::Immersive => None,
        ThemeMode::Adaptive => detect_background(),
    };
    let palette = resolve_with(mode, detected);
    tracing::info!(?mode, variant = ?palette.variant, "resolved theme");
    palette
}

/// Resolve from an explicitly supplied background, bypassing the terminal query.
///
/// Used by tests and by the `--background` escape hatch for terminals that answer the
/// OSC 11 query incorrectly rather than not at all.
///
/// When a background is known it is threaded into [`Palette::for_ground`] rather than
/// discarded — that is what lets the hairline colour be derived from the *real* ground
/// instead of a stand-in, which is the difference between a visible rule and an
/// invisible one on themes like Nord.
pub fn resolve_with(mode: ThemeMode, background: Option<Rgb>) -> Palette {
    let variant = select(mode, background);
    match background {
        // Immersive paints its own ground, so a detected one must not override it.
        Some(bg) if variant != Variant::Immersive => Palette::for_ground(variant, bg),
        _ => Palette::of(variant),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection_never_panics_without_a_tty() {
        // Under `cargo test` stdin is not a terminal, so this exercises the quiet-failure
        // path that CI and piped output will also take.
        let _ = detect_background();
    }

    #[test]
    fn resolution_is_deterministic_when_background_is_supplied() {
        assert_eq!(
            resolve_with(ThemeMode::Adaptive, Some(Rgb::from_hex(0x1E1E2E))).variant,
            Variant::Dark
        );
        assert_eq!(
            resolve_with(ThemeMode::Adaptive, Some(Rgb::from_hex(0xFFFFFF))).variant,
            Variant::Light
        );
        assert_eq!(
            resolve_with(ThemeMode::Immersive, Some(Rgb::from_hex(0xFFFFFF))).variant,
            Variant::Immersive
        );
    }

    #[test]
    fn detected_background_reaches_the_palette() {
        // Regression guard: an earlier version detected the background and then threw it
        // away, so the hairline was derived from a stand-in ground instead of the real one.
        let nord = Rgb::from_hex(0x2E3440);
        let palette = resolve_with(ThemeMode::Adaptive, Some(nord));
        assert_eq!(palette.ground_ref(), nord);
    }

    #[test]
    fn immersive_ignores_a_detected_background() {
        // Immersive paints its own dusk-indigo ground; a detected white background must
        // not leak in and silently turn it into a light theme.
        let palette = resolve_with(ThemeMode::Immersive, Some(Rgb::from_hex(0xFFFFFF)));
        assert_eq!(palette.ground(), Some(super::super::palette::IMMERSIVE_GROUND));
    }
}
