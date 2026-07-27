//! The "Obi & Silk" palette.
//!
//! Colours are bound to *semantic roles*, never referenced directly by name. That
//! indirection is what makes an adaptive ground possible: the same widget code renders
//! against a dark terminal, a light one, or the painted dusk-indigo ground, because it
//! only ever asks for [`Role::Obi`] rather than "amber".
//!
//! The signature is the obi (帯) — the paper band wrapped around Japanese media
//! packaging — rendered as a one-cell vertical bar marking focus. It is the only place
//! saturated colour appears in chrome, which is what lets every panel border be deleted.
//!
//! Ground is dusk indigo 藍 and the accent is amber 琥珀, from the dusk-sky palette that
//! is the most recognisable visual signature of the medium.

use anistream_core::config::ThemeMode;

use super::color::{Rgb, contrast_from_luminance, mix};

/// A semantic slot in the palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Primary text. Warm off-white on dark grounds — ink on silk rather than the glare
    /// of pure white.
    Text,
    /// Secondary text: metadata, counts, inactive rail entries.
    TextDim,
    /// Hairline rules. Non-text, so exempt from the contrast floor, but still has to be
    /// visible or the layout loses its structure.
    Rule,
    /// The focus accent. The obi bar, progress fill, and nothing else.
    Obi,
    /// State and progress indication.
    State,
    /// Something is broken — a dead provider, a VPN leak, a failed sync.
    ///
    /// Provider death is this app's defining failure mode, so a dedicated colour for it
    /// is functional rather than decorative.
    Alert,
}

impl Role {
    pub const ALL: [Self; 6] =
        [Self::Text, Self::TextDim, Self::Rule, Self::Obi, Self::State, Self::Alert];

    /// Whether this role renders glyphs and therefore must meet the contrast floor.
    pub const fn is_text(self) -> bool {
        !matches!(self, Self::Rule)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::TextDim => "text-dim",
            Self::Rule => "rule",
            Self::Obi => "obi",
            Self::State => "state",
            Self::Alert => "alert",
        }
    }
}

/// Which set of colours is in force.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Variant {
    /// Adaptive, against a dark terminal background we do not control.
    Dark,
    /// Adaptive, against a light terminal background we do not control.
    Light,
    /// We paint the ground, so the exact contrast is known.
    Immersive,
}

/// Luminance range a *dark* terminal background may plausibly occupy.
///
/// The upper bound is the important one: it has to cover real themes rather than only
/// pure black. Nord's `#2E3440` sits at ~0.034 and Gruvbox dark at ~0.021, so a ceiling
/// of 0.05 keeps the palette honest on the backgrounds people actually use.
pub const DARK_GROUND_RANGE: (f64, f64) = (0.0, 0.05);

/// Luminance range a *light* terminal background may plausibly occupy.
///
/// Floor of 0.7 covers white, Solarized Light (~0.88) and paper-toned greys, without
/// pretending the palette also works on a mid-grey ground where nothing would.
pub const LIGHT_GROUND_RANGE: (f64, f64) = (0.7, 1.0);

/// The painted ground for immersive mode: dusk indigo, 藍.
pub const IMMERSIVE_GROUND: Rgb = Rgb::from_hex(0x161A2E);

/// Ground assumed for the dark variant when the terminal refuses to answer OSC 11.
const ASSUMED_DARK_GROUND: Rgb = Rgb::from_hex(0x1A1B26);

/// Ground assumed for the light variant when detection fails.
const ASSUMED_LIGHT_GROUND: Rgb = Rgb::from_hex(0xFFFFFF);

/// How far a hairline sits from the ground, as a fraction of the way toward `text`.
///
/// Tuned to land around 1.6–1.9:1 against the ground: unmistakably present, but quiet
/// enough to read as structure rather than as content. Rules do most of the work that
/// borders would in a conventional TUI, so getting this wrong is very visible.
const RULE_MIX: f64 = 0.22;

/// Ink for text sitting on a filled band. Not pure black or white — the same restraint as the
/// text roles, which never use `#000` or `#FFF` either.
const INK_ON_LIGHT: Rgb = Rgb::from_hex(0x161A2E);
const INK_ON_DARK: Rgb = Rgb::from_hex(0xE4E0D6);

/// A resolved set of role colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub variant: Variant,
    text: Rgb,
    text_dim: Rgb,
    rule: Rgb,
    obi: Rgb,
    state: Rgb,
    alert: Rgb,
    /// Only set in immersive mode. `None` means inherit the terminal's background, which
    /// is what lets anistream sit alongside the user's other tools.
    ground: Option<Rgb>,
    /// The ground we believe we are drawing on — detected, assumed, or painted. Used to
    /// derive the hairline colour and to report honest contrast figures.
    ground_ref: Rgb,
}

impl Palette {
    pub fn dark() -> Self {
        Self::for_ground(Variant::Dark, ASSUMED_DARK_GROUND)
    }

    pub fn light() -> Self {
        Self::for_ground(Variant::Light, ASSUMED_LIGHT_GROUND)
    }

    /// Immersive: we own the ground, so these can be richer than the adaptive dark set,
    /// which has to survive an unknown range of backgrounds.
    pub fn immersive() -> Self {
        Self::for_ground(Variant::Immersive, IMMERSIVE_GROUND)
    }

    pub fn of(variant: Variant) -> Self {
        match variant {
            Variant::Dark => Self::dark(),
            Variant::Light => Self::light(),
            Variant::Immersive => Self::immersive(),
        }
    }

    /// Build a palette for a variant against a *known* ground.
    ///
    /// The hairline colour is derived here rather than hardcoded. A fixed mid-tone rule
    /// cannot work across an unknown background range — its worst case is a ground near
    /// its own luminance, so on some themes it would vanish entirely. Deriving it as a
    /// fixed step away from the real ground keeps it consistently visible whether the
    /// terminal is pure black or Nord.
    pub fn for_ground(variant: Variant, ground: Rgb) -> Self {
        let (text, text_dim, obi, state, alert) = match variant {
            Variant::Dark => (
                Rgb::from_hex(0xE4E0D6), // 絹 silk
                Rgb::from_hex(0xA8AEC9), // 霞 haze
                Rgb::from_hex(0xF2A64B), // 琥珀 amber
                Rgb::from_hex(0x7FC7D9), // 水 water
                Rgb::from_hex(0xF59B85),
            ),
            Variant::Light => (
                Rgb::from_hex(0x1E2033),
                Rgb::from_hex(0x4F546E),
                Rgb::from_hex(0x7A4309),
                Rgb::from_hex(0x1F5D6E),
                Rgb::from_hex(0x94301F),
            ),
            Variant::Immersive => (
                Rgb::from_hex(0xE4E0D6),
                Rgb::from_hex(0x9298B5),
                Rgb::from_hex(0xF2A64B),
                Rgb::from_hex(0x7FC7D9),
                Rgb::from_hex(0xE06B54), // 朱 vermilion
            ),
        };
        Self {
            variant,
            text,
            text_dim,
            rule: mix(ground, text, RULE_MIX),
            obi,
            state,
            alert,
            ground_ref: ground,
            ground: matches!(variant, Variant::Immersive).then_some(ground),
        }
    }

    pub const fn get(&self, role: Role) -> Rgb {
        match role {
            Role::Text => self.text,
            Role::TextDim => self.text_dim,
            Role::Rule => self.rule,
            Role::Obi => self.obi,
            Role::State => self.state,
            Role::Alert => self.alert,
        }
    }

    pub const fn ground(&self) -> Option<Rgb> {
        self.ground
    }

    pub const fn color(&self, role: Role) -> ratatui::style::Color {
        self.get(role).to_ratatui()
    }

    pub fn style(&self, role: Role) -> ratatui::style::Style {
        ratatui::style::Style::default().fg(self.color(role))
    }

    /// A foreground guaranteed readable on top of `role` used as a fill.
    ///
    /// Only the eyecatch needs this, and only because it is the one place in the design where
    /// a saturated colour becomes a *background*. Picking by measured luminance rather than
    /// per-variant constants means the light variant's dark amber and the dark variant's bright
    /// amber both come out right, with no third value to keep in sync.
    pub fn on_fill(&self, role: Role) -> ratatui::style::Style {
        let fill = self.get(role);
        let ink = if fill.luminance() > 0.35 { INK_ON_LIGHT } else { INK_ON_DARK };
        ratatui::style::Style::default().fg(ink.to_ratatui()).bg(fill.to_ratatui())
    }

    /// A style that fills with `role`, for painting a band.
    pub fn fill(&self, role: Role) -> ratatui::style::Style {
        ratatui::style::Style::default().bg(self.color(role))
    }

    /// The worst-case ground luminance this variant must remain readable against.
    fn worst_ground_luminance(&self) -> f64 {
        match self.variant {
            // Lightest a dark ground may get.
            Variant::Dark => DARK_GROUND_RANGE.1,
            // Darkest a light ground may get.
            Variant::Light => LIGHT_GROUND_RANGE.0,
            Variant::Immersive => IMMERSIVE_GROUND.luminance(),
        }
    }

    /// The ground this palette was built against.
    pub const fn ground_ref(&self) -> Rgb {
        self.ground_ref
    }

    /// Contrast for a role against the worst plausible ground for this variant.
    ///
    /// Text roles are measured against the range endpoint, because a fixed light colour
    /// is hardest to read on the lightest ground it might meet. The hairline is instead
    /// measured against the *actual* ground, since it is derived from it — using the
    /// range endpoint there would report a meaningless figure for a colour that moves
    /// with the background.
    pub fn worst_case_contrast(&self, role: Role) -> f64 {
        let ground_luminance = if role.is_text() {
            self.worst_ground_luminance()
        } else {
            self.ground_ref.luminance()
        };
        contrast_from_luminance(self.get(role).luminance(), ground_luminance)
    }
}

/// Choose a variant from the configured mode and the detected background.
pub fn select(mode: ThemeMode, detected_background: Option<Rgb>) -> Variant {
    match mode {
        ThemeMode::Immersive => Variant::Immersive,
        ThemeMode::Adaptive => match detected_background {
            Some(bg) if bg.luminance() > 0.5 => Variant::Light,
            // Default to dark when the terminal does not answer the OSC 11 query. Dark
            // is both the commoner setup and the safer guess: light text on an unknown
            // ground is more often readable than dark text on one.
            _ => Variant::Dark,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{super::color::AA_NORMAL, *};

    /// The load-bearing test for the whole palette: every text role, in every variant,
    /// against the worst ground that variant claims to support.
    #[test]
    fn every_text_role_meets_aa_in_every_variant() {
        let mut failures = Vec::new();
        for variant in [Variant::Dark, Variant::Light, Variant::Immersive] {
            let palette = Palette::of(variant);
            for role in Role::ALL.into_iter().filter(|r| r.is_text()) {
                let ratio = palette.worst_case_contrast(role);
                if ratio < AA_NORMAL {
                    failures.push(format!(
                        "{variant:?}/{} = {ratio:.2}:1 (need {AA_NORMAL})",
                        role.name()
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "contrast failures:\n  {}", failures.join("\n  "));
    }

    #[test]
    fn hairlines_stay_in_the_visible_band_on_every_real_terminal_theme() {
        // Rules are exempt from AA because they render no glyphs, but an invisible rule
        // would silently destroy the layout's structure — with no borders anywhere, the
        // hairlines *are* the structure. They also must not shout, or they start
        // competing with content.
        //
        // Because the colour is derived from the ground, this checks a spread of real
        // backgrounds rather than one synthetic worst case.
        let cases: &[(&str, Variant, u32)] = &[
            ("black", Variant::Dark, 0x000000),
            ("tokyo-night", Variant::Dark, 0x1A1B26),
            ("catppuccin-mocha", Variant::Dark, 0x1E1E2E),
            ("gruvbox-dark", Variant::Dark, 0x282828),
            ("nord", Variant::Dark, 0x2E3440),
            ("white", Variant::Light, 0xFFFFFF),
            ("solarized-light", Variant::Light, 0xFDF6E3),
            ("paper", Variant::Light, 0xF7F6F2),
            ("immersive-indigo", Variant::Immersive, 0x161A2E),
        ];
        for &(name, variant, ground) in cases {
            let p = Palette::for_ground(variant, Rgb::from_hex(ground));
            let ratio = p.worst_case_contrast(Role::Rule);
            assert!(
                (1.25..=3.5).contains(&ratio),
                "{name} ({variant:?}) hairline at {ratio:.2}:1 is outside the visible band"
            );
        }
    }

    #[test]
    fn hairline_tracks_the_ground_rather_than_staying_fixed() {
        // The bug this design replaced: a single hardcoded mid-tone rule vanished
        // against grounds near its own luminance. Deriving it means the colour differs
        // per ground while the *contrast* stays roughly constant.
        let on_black = Palette::for_ground(Variant::Dark, Rgb::from_hex(0x000000));
        let on_nord = Palette::for_ground(Variant::Dark, Rgb::from_hex(0x2E3440));
        assert_ne!(
            on_black.get(Role::Rule),
            on_nord.get(Role::Rule),
            "hairline should move with the ground"
        );
        let delta = (on_black.worst_case_contrast(Role::Rule)
            - on_nord.worst_case_contrast(Role::Rule))
        .abs();
        assert!(delta < 0.5, "hairline contrast drifted by {delta:.2} across grounds");
    }

    #[test]
    fn text_hierarchy_is_preserved_in_every_variant() {
        // Dim text must actually read as dimmer than primary text, or the visual
        // hierarchy the design depends on collapses.
        for variant in [Variant::Dark, Variant::Light, Variant::Immersive] {
            let p = Palette::of(variant);
            let primary = p.worst_case_contrast(Role::Text);
            let dim = p.worst_case_contrast(Role::TextDim);
            assert!(
                primary > dim,
                "{variant:?}: primary {primary:.2} should out-contrast dim {dim:.2}"
            );
        }
    }

    #[test]
    fn only_immersive_paints_a_ground() {
        assert!(Palette::dark().ground().is_none());
        assert!(Palette::light().ground().is_none());
        assert_eq!(Palette::immersive().ground(), Some(IMMERSIVE_GROUND));
    }

    #[test]
    fn variant_selection_follows_mode_then_detection() {
        // Immersive ignores detection entirely.
        assert_eq!(
            select(ThemeMode::Immersive, Some(Rgb::from_hex(0xFFFFFF))),
            Variant::Immersive
        );
        assert_eq!(select(ThemeMode::Adaptive, Some(Rgb::from_hex(0xFDF6E3))), Variant::Light);
        assert_eq!(select(ThemeMode::Adaptive, Some(Rgb::from_hex(0x2E3440))), Variant::Dark);
        // No answer from the terminal: fall back to dark rather than guessing light.
        assert_eq!(select(ThemeMode::Adaptive, None), Variant::Dark);
    }

    #[test]
    fn real_world_dark_themes_fall_inside_the_supported_range() {
        // If these drift outside the range, the AA test above is checking the wrong bound.
        for (name, hex) in [
            ("nord", 0x2E3440),
            ("gruvbox-dark", 0x282828),
            ("catppuccin-mocha", 0x1E1E2E),
            ("tokyo-night", 0x1A1B26),
            ("black", 0x000000),
        ] {
            let l = Rgb::from_hex(hex).luminance();
            assert!(
                l <= DARK_GROUND_RANGE.1,
                "{name} ({hex:#08X}) has luminance {l:.4}, above the {:.2} ceiling",
                DARK_GROUND_RANGE.1
            );
        }
    }

    #[test]
    fn real_world_light_themes_fall_inside_the_supported_range() {
        for (name, hex) in
            [("solarized-light", 0xFDF6E3), ("white", 0xFFFFFF), ("paper", 0xF7F6F2)]
        {
            let l = Rgb::from_hex(hex).luminance();
            assert!(
                l >= LIGHT_GROUND_RANGE.0,
                "{name} ({hex:#08X}) has luminance {l:.4}, below the {:.2} floor",
                LIGHT_GROUND_RANGE.0
            );
        }
    }
}
