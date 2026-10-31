//! How subtitles look: the viewer's choices as one record, and the mpv options that draw them.
//!
//! The choices live in the config's `[subtitles]` table, per device. Plain subtitles (SRT, WebVTT)
//! take every option; styled ones (ASS) take them according to the Styled subtitles choice, which
//! is mpv's `sub-ass-override`. The system's own caption style, where the platform has one, is
//! handed in by the app and takes the place of the font, colour, edge and background.
//!
//! The default style gives the options the apps gave before this existed, apart from the size,
//! which now goes through `sub-scale` so it reaches styled subtitles in every choice.

use anistream_core::config::{
    StyledSubtitles, SubtitleBackground, SubtitleColour, SubtitleEdge, SubtitleFont,
    SubtitleThickness, SubtitlesConfig,
};

/// The viewer's subtitle choices.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SubtitleStyle {
    /// Percent of the player's size: 50 to 200 in steps of 10.
    pub size: u32,
    pub font: SubtitleFont,
    pub colour: SubtitleColour,
    pub edge: SubtitleEdge,
    pub thickness: SubtitleThickness,
    pub background: SubtitleBackground,
    /// How opaque the background plate is, percent: 50, 75 or 100.
    pub opacity: u32,
    /// How far subtitles are raised, percent of the picture: 0 to 20.
    pub raise: u32,
    pub styled: StyledSubtitles,
    /// Take the system's caption style where the platform has one.
    pub follow_system: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        SubtitlesConfig::default().into()
    }
}

impl From<SubtitlesConfig> for SubtitleStyle {
    fn from(c: SubtitlesConfig) -> Self {
        Self {
            size: c.size,
            font: c.font,
            colour: c.colour,
            edge: c.edge,
            thickness: c.thickness,
            background: c.background,
            opacity: c.opacity,
            raise: c.raise,
            styled: c.styled,
            follow_system: c.follow_system,
        }
    }
}

impl From<&SubtitleStyle> for SubtitlesConfig {
    fn from(s: &SubtitleStyle) -> Self {
        Self {
            size: s.size,
            font: s.font,
            colour: s.colour,
            edge: s.edge,
            thickness: s.thickness,
            background: s.background,
            opacity: s.opacity,
            raise: s.raise,
            styled: s.styled,
            follow_system: s.follow_system,
        }
    }
}

/// How a system draws the edge of caption letters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CaptionEdge {
    /// No edge.
    Plain,
    Raised,
    Depressed,
    /// An outline all round.
    Uniform,
    DropShadow,
}

/// The system's caption style, read by the app from MediaAccessibility, `CaptioningManager` or
/// `ClosedCaptionProperties`. A field the system does not set is `None`, and the viewer's own
/// choice stands for it. Colours are `0xRRGGBBAA`.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SystemCaptionStyle {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub font_family: Option<String>,
    /// The text's size against the normal one: 1.0 is normal.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub relative_size: Option<f64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub text_rgba: Option<u32>,
    /// Behind each line's letters.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub background_rgba: Option<u32>,
    /// One box behind all the lines.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub window_rgba: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub edge: Option<CaptionEdge>,
}

/// One mpv option, as the player sets it: the hosted player's own record, so the apps pass the
/// options straight to it.
pub use crate::hosted::MpvOption;

fn option(name: &str, value: impl Into<String>) -> MpvOption {
    MpvOption { name: name.into(), value: value.into() }
}

/// An `#AARRGGBB` colour, as mpv takes it.
fn argb(rgb: u32, alpha: u8) -> String {
    format!("#{alpha:02X}{rgb:06X}")
}

/// `0xRRGGBBAA` as mpv's `#AARRGGBB`.
fn from_rgba(rgba: u32) -> String {
    argb(rgba >> 8, (rgba & 0xFF) as u8)
}

fn alpha_of(rgba: u32) -> u8 {
    (rgba & 0xFF) as u8
}

const BLACK: u32 = 0x00_0000;

/// The look once the system's style is applied: what the options are written from.
struct Look {
    font: Option<String>,
    scale: f64,
    text: String,
    outline: f64,
    outline_colour: String,
    shadow: f64,
    shadow_colour: String,
    border_style: &'static str,
}

/// The options that draw subtitles in `style`, with the system's caption style over it when the
/// style follows it, and the picture raised by `style.raise` and `controls_lift` together, both
/// percent of the picture's height.
///
/// `sub-font` is left out for the Default font: the app's own default stays, as it set it.
pub fn mpv_subtitle_options(
    style: &SubtitleStyle,
    system: Option<&SystemCaptionStyle>,
    controls_lift: f64,
) -> Vec<MpvOption> {
    let system = system.filter(|_| style.follow_system);
    let size =
        f64::from(style.size) / 100.0 * system.and_then(|s| s.relative_size).unwrap_or(1.0);
    let plate = match style.opacity {
        50 => 0x80,
        75 => 0xBF,
        _ => 0xFF,
    };
    let (weight_outline, weight_shadow) = match style.thickness {
        SubtitleThickness::Thin => (1.0, 1.0),
        SubtitleThickness::Medium => (1.65, 2.0),
        SubtitleThickness::Thick => (2.5, 3.0),
    };

    let mut look = Look {
        font: match style.font {
            SubtitleFont::Default => None,
            SubtitleFont::ZenKakuGothicNew => Some("Zen Kaku Gothic New".into()),
            SubtitleFont::Serif => Some("serif".into()),
            SubtitleFont::Monospace => Some("monospace".into()),
        },
        scale: size.clamp(0.25, 3.0),
        text: argb(
            match style.colour {
                SubtitleColour::White => 0xFFFFFF,
                SubtitleColour::Yellow => 0xFFFF00,
                SubtitleColour::Cyan => 0x00FFFF,
                SubtitleColour::Green => 0x00FF00,
            },
            0xFF,
        ),
        outline: 0.0,
        outline_colour: argb(BLACK, 0xFF),
        shadow: 0.0,
        shadow_colour: argb(BLACK, 0xAF),
        border_style: "outline-and-shadow",
    };
    match style.edge {
        SubtitleEdge::Outline => look.outline = weight_outline,
        SubtitleEdge::DropShadow => look.shadow = weight_shadow,
        SubtitleEdge::NoEdge => {}
    }
    match style.background {
        SubtitleBackground::NoBackground => {}
        // A box tight around each line, drawn by libass as the outline, so the edge gives way.
        SubtitleBackground::EachLine => {
            look.border_style = "opaque-box";
            look.outline = 3.0;
            look.outline_colour = argb(BLACK, plate);
            look.shadow = 0.0;
        }
        // One box behind all the lines, the shadow's offset its margin.
        SubtitleBackground::AllLines => {
            look.border_style = "background-box";
            look.shadow = 6.0;
            look.shadow_colour = argb(BLACK, plate);
        }
    }

    if let Some(system) = system {
        if let Some(family) = &system.font_family {
            look.font = Some(family.clone());
        }
        if let Some(text) = system.text_rgba {
            look.text = from_rgba(text);
        }
        if let Some(edge) = system.edge {
            // The system's edge replaces the viewer's.
            look.outline = 0.0;
            look.shadow = 0.0;
            look.shadow_colour = argb(BLACK, 0xAF);
            match edge {
                CaptionEdge::Plain => {}
                CaptionEdge::Uniform => look.outline = 1.65,
                CaptionEdge::DropShadow => look.shadow = 2.0,
                // Lit from above: a dark edge below and to the right.
                CaptionEdge::Raised => look.shadow = 1.0,
                // Pressed in: a light edge below and to the right.
                CaptionEdge::Depressed => {
                    look.shadow = 1.0;
                    look.shadow_colour = argb(0xFFFFFF, 0x99);
                }
            }
        }
        let box_behind = |rgba: Option<u32>| rgba.filter(|c| alpha_of(*c) > 0);
        if let Some(background) = box_behind(system.background_rgba) {
            look.border_style = "opaque-box";
            look.outline = 3.0;
            look.outline_colour = from_rgba(background);
            look.shadow = 0.0;
        } else if let Some(window) = box_behind(system.window_rgba) {
            look.border_style = "background-box";
            look.shadow = 6.0;
            look.shadow_colour = from_rgba(window);
        } else if system.background_rgba.is_some() || system.window_rgba.is_some() {
            // The system says there is no plate: the viewer's own plate gives way to it.
            look.border_style = "outline-and-shadow";
        }
    }

    let lift = (f64::from(style.raise) + controls_lift.max(0.0)).clamp(0.0, 100.0);
    let mut options = Vec::with_capacity(11);
    if let Some(font) = look.font {
        options.push(option("sub-font", font));
    }
    options.extend([
        // mpv's 38 is the size at a 720-line window; the scale does the choosing.
        option("sub-font-size", "38"),
        option("sub-scale", format!("{:.2}", look.scale)),
        option("sub-color", look.text),
        option("sub-border-style", look.border_style),
        option("sub-outline-size", format!("{:.2}", look.outline)),
        option("sub-outline-color", look.outline_colour),
        option("sub-shadow-offset", format!("{:.2}", look.shadow)),
        option("sub-back-color", look.shadow_colour),
        option("sub-pos", format!("{:.0}", 100.0 - lift)),
        option(
            "sub-ass-override",
            match style.styled {
                StyledSubtitles::Keep => "scale",
                StyledSubtitles::Restyle => "force",
                StyledSubtitles::Strip => "strip",
            },
        ),
    ]);
    options
}

/// The options that differ from mpv's own look under `style`: all the terminal sets, so the
/// viewer's `mpv.conf` keeps the rest. The system's style is not read there, and a viewer who
/// changed nothing gets none.
pub fn terminal_subtitle_options(style: &SubtitleStyle) -> Vec<MpvOption> {
    let defaults = mpv_subtitle_options(&SubtitleStyle::default(), None, 0.0);
    mpv_subtitle_options(style, None, 0.0)
        .into_iter()
        .filter(|changed| !defaults.contains(changed))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get<'a>(options: &'a [MpvOption], name: &str) -> Option<&'a str> {
        options.iter().find(|o| o.name == name).map(|o| o.value.as_str())
    }

    /// Yellow on a blue box, half again as large, in serif: the Android tests' style, handed in
    /// as the system's.
    fn yellow_on_blue() -> SystemCaptionStyle {
        SystemCaptionStyle {
            font_family: Some("serif".into()),
            relative_size: Some(1.5),
            text_rgba: Some(0xFFFF_00FF),
            background_rgba: Some(0x0000_FFBF),
            window_rgba: None,
            edge: Some(CaptionEdge::Uniform),
        }
    }

    fn with_system(system: &SystemCaptionStyle) -> Vec<MpvOption> {
        mpv_subtitle_options(&SubtitleStyle::default(), Some(system), 0.0)
    }

    #[test]
    fn the_default_style_is_what_the_apps_gave_before_but_for_the_size() {
        let options = mpv_subtitle_options(&SubtitleStyle::default(), None, 0.0);
        assert_eq!(get(&options, "sub-font"), None, "the app's own font stays");
        assert_eq!(get(&options, "sub-font-size"), Some("38"));
        assert_eq!(get(&options, "sub-scale"), Some("1.00"));
        assert_eq!(get(&options, "sub-color"), Some("#FFFFFFFF"));
        assert_eq!(get(&options, "sub-border-style"), Some("outline-and-shadow"));
        assert_eq!(get(&options, "sub-outline-size"), Some("1.65"));
        assert_eq!(get(&options, "sub-outline-color"), Some("#FF000000"));
        assert_eq!(get(&options, "sub-shadow-offset"), Some("0.00"));
        assert_eq!(get(&options, "sub-back-color"), Some("#AF000000"));
        assert_eq!(get(&options, "sub-pos"), Some("100"));
        assert_eq!(get(&options, "sub-ass-override"), Some("scale"));
    }

    #[test]
    fn a_background_behind_the_letters_is_a_tight_box_in_its_colour() {
        let options = with_system(&yellow_on_blue());
        assert_eq!(get(&options, "sub-font"), Some("serif"));
        assert_eq!(get(&options, "sub-scale"), Some("1.50"));
        assert_eq!(get(&options, "sub-color"), Some("#FFFFFF00"));
        // libass draws the tight box with the outline, so the box wins over the edge.
        assert_eq!(get(&options, "sub-border-style"), Some("opaque-box"));
        assert_eq!(get(&options, "sub-outline-color"), Some("#BF0000FF"));
        assert_eq!(get(&options, "sub-shadow-offset"), Some("0.00"));
    }

    #[test]
    fn a_window_is_one_box_behind_all_the_lines() {
        let system = SystemCaptionStyle {
            background_rgba: Some(0),
            window_rgba: Some(0x0000_0080),
            edge: Some(CaptionEdge::Plain),
            ..yellow_on_blue()
        };
        let options = with_system(&system);
        assert_eq!(get(&options, "sub-border-style"), Some("background-box"));
        assert_eq!(get(&options, "sub-back-color"), Some("#80000000"));
        assert_eq!(get(&options, "sub-shadow-offset"), Some("6.00"));
        assert_eq!(get(&options, "sub-outline-size"), Some("0.00"));
    }

    #[test]
    fn edges_without_a_box_are_the_outline_or_the_shadow() {
        let plain = SystemCaptionStyle { background_rgba: Some(0), ..yellow_on_blue() };
        let options = with_system(&plain);
        assert_eq!(get(&options, "sub-border-style"), Some("outline-and-shadow"));
        assert_eq!(get(&options, "sub-outline-size"), Some("1.65"));
        let shadow = with_system(&SystemCaptionStyle {
            edge: Some(CaptionEdge::DropShadow),
            ..plain.clone()
        });
        assert_eq!(get(&shadow, "sub-outline-size"), Some("0.00"));
        assert_eq!(get(&shadow, "sub-shadow-offset"), Some("2.00"));
        let pressed =
            with_system(&SystemCaptionStyle { edge: Some(CaptionEdge::Depressed), ..plain });
        assert_eq!(get(&pressed, "sub-back-color"), Some("#99FFFFFF"));
    }

    #[test]
    fn sizes_stay_in_range_whatever_the_system_asks() {
        let huge = SystemCaptionStyle { relative_size: Some(9.0), ..yellow_on_blue() };
        assert_eq!(get(&with_system(&huge), "sub-scale"), Some("3.00"));
        let tiny = SystemCaptionStyle { relative_size: Some(0.1), ..yellow_on_blue() };
        assert_eq!(get(&with_system(&tiny), "sub-scale"), Some("0.25"));
    }

    #[test]
    fn the_system_style_is_ignored_unless_the_style_follows_it() {
        let style = SubtitleStyle { follow_system: false, ..SubtitleStyle::default() };
        let options = mpv_subtitle_options(&style, Some(&yellow_on_blue()), 0.0);
        assert_eq!(options, mpv_subtitle_options(&style, None, 0.0));
    }

    #[test]
    fn the_size_multiplies_the_systems() {
        let style = SubtitleStyle { size: 150, ..SubtitleStyle::default() };
        let system =
            SystemCaptionStyle { relative_size: Some(1.2), ..SystemCaptionStyle::default() };
        let options = mpv_subtitle_options(&style, Some(&system), 0.0);
        assert_eq!(get(&options, "sub-scale"), Some("1.80"));
    }

    #[test]
    fn a_system_that_sets_nothing_leaves_the_viewers_own_choices() {
        let style = SubtitleStyle { colour: SubtitleColour::Cyan, ..SubtitleStyle::default() };
        let options = mpv_subtitle_options(&style, Some(&SystemCaptionStyle::default()), 0.0);
        assert_eq!(get(&options, "sub-color"), Some("#FF00FFFF"));
    }

    #[test]
    fn the_viewers_choices_reach_every_option() {
        let style = SubtitleStyle {
            size: 150,
            font: SubtitleFont::Serif,
            colour: SubtitleColour::Yellow,
            edge: SubtitleEdge::DropShadow,
            thickness: SubtitleThickness::Thick,
            background: SubtitleBackground::EachLine,
            opacity: 50,
            raise: 6,
            styled: StyledSubtitles::Restyle,
            follow_system: true,
        };
        let options = mpv_subtitle_options(&style, None, 0.0);
        assert_eq!(get(&options, "sub-scale"), Some("1.50"));
        assert_eq!(get(&options, "sub-font"), Some("serif"));
        assert_eq!(get(&options, "sub-color"), Some("#FFFFFF00"));
        assert_eq!(get(&options, "sub-border-style"), Some("opaque-box"));
        assert_eq!(get(&options, "sub-outline-color"), Some("#80000000"));
        assert_eq!(get(&options, "sub-pos"), Some("94"));
        assert_eq!(get(&options, "sub-ass-override"), Some("force"));
        let shadow = SubtitleStyle { background: SubtitleBackground::NoBackground, ..style };
        let options = mpv_subtitle_options(&shadow, None, 0.0);
        assert_eq!(get(&options, "sub-shadow-offset"), Some("3.00"));
        assert_eq!(get(&options, "sub-outline-size"), Some("0.00"));
    }

    #[test]
    fn all_lines_take_the_plates_opacity_in_the_back_colour() {
        let style = SubtitleStyle {
            background: SubtitleBackground::AllLines,
            opacity: 100,
            ..SubtitleStyle::default()
        };
        let options = mpv_subtitle_options(&style, None, 0.0);
        assert_eq!(get(&options, "sub-border-style"), Some("background-box"));
        assert_eq!(get(&options, "sub-back-color"), Some("#FF000000"));
    }

    #[test]
    fn raise_composes_with_the_lift_above_the_controls() {
        let style = SubtitleStyle { raise: 6, ..SubtitleStyle::default() };
        assert_eq!(get(&mpv_subtitle_options(&style, None, 10.0), "sub-pos"), Some("84"));
        assert_eq!(get(&mpv_subtitle_options(&style, None, 500.0), "sub-pos"), Some("0"));
    }

    #[test]
    fn styled_subtitles_choose_mpvs_override() {
        for (styled, value) in [
            (StyledSubtitles::Keep, "scale"),
            (StyledSubtitles::Restyle, "force"),
            (StyledSubtitles::Strip, "strip"),
        ] {
            let style = SubtitleStyle { styled, ..SubtitleStyle::default() };
            assert_eq!(
                get(&mpv_subtitle_options(&style, None, 0.0), "sub-ass-override"),
                Some(value)
            );
        }
    }

    #[test]
    fn the_terminal_sets_only_what_changed() {
        assert!(terminal_subtitle_options(&SubtitleStyle::default()).is_empty());
        let style = SubtitleStyle {
            size: 120,
            styled: StyledSubtitles::Strip,
            ..SubtitleStyle::default()
        };
        let names: Vec<_> =
            terminal_subtitle_options(&style).into_iter().map(|o| o.name).collect();
        assert_eq!(names, ["sub-scale", "sub-ass-override"]);
    }

    #[test]
    fn every_option_a_style_sets_is_set_by_the_default_too_but_the_font() {
        let all = mpv_subtitle_options(
            &SubtitleStyle { font: SubtitleFont::Monospace, ..SubtitleStyle::default() },
            None,
            0.0,
        );
        let default = mpv_subtitle_options(&SubtitleStyle::default(), None, 0.0);
        let names = |o: &[MpvOption]| o.iter().map(|o| o.name.clone()).collect::<Vec<_>>();
        let mut expected = names(&all);
        expected.retain(|n| n != "sub-font");
        assert_eq!(names(&default), expected, "turning a choice off sets every option back");
    }

    #[test]
    fn a_config_round_trips_through_the_record() {
        let config = SubtitlesConfig { size: 150, raise: 6, ..SubtitlesConfig::default() };
        let style = SubtitleStyle::from(config.clone());
        assert_eq!(SubtitlesConfig::from(&style), config);
    }
}
