//! The mpv player as the app builds it, from the configuration as it stands now.
//!
//! In the library rather than the binary so the preview example and the tests build exactly
//! the player a playback gets.

use anistream_core::config::{Config, Paths};

/// The player, built from the configuration *as it stands now*.
///
/// Built per playback rather than held from launch, and that is the whole point.
/// `upscaling`, `mpv_binary` and `mpv_args` are all editable on the Settings screen,
/// so a handle made once at startup freezes whatever they were then — turning Anime4K
/// off did nothing until the app was restarted, because `--glsl-shaders` stayed in the
/// argument list every episode was spawned with. Shader files are written on demand and
/// skipped when already current, so rebuilding costs a stat per shader.
pub fn player(config: &Config, paths: &Paths) -> anistream_player::Mpv {
    // Upscaling is no longer on the command line: the chain is chosen per file and set over
    // IPC (see `playback`). What is left here is the HDR curve the viewer picked, before the
    // user's own flags so a hand-written one still wins.
    let mut args = Vec::new();
    let curve = &config.playback.tone_mapping;
    if !curve.is_empty() && curve != crate::picture::DEFAULT_TONE_MAPPING {
        args.push(format!("--tone-mapping={curve}"));
    }
    args.extend(config.playback.mpv_args.clone());
    // Content-addressed and skipped when current, so this is a stat per launch of a player.
    let bundle = anistream_player::Bundle::materialize(&paths.cache_dir)
        .inspect_err(|e| {
            tracing::warn!(error = %e, "could not write the player bundle; using mpv's own look")
        })
        .ok();
    anistream_player::Mpv::new(paths.runtime_dir())
        .with_binary(config.playback.mpv_binary.clone())
        .with_ui(config.playback.player_ui, bundle)
        .with_theme(player_theme())
        .with_extra_args(args)
}

/// Where the upscaling shaders were written, or `None` when anistream should leave
/// `glsl-shaders` alone: the cache could not be written, or the user passes their own chain
/// in `playback.mpv_args`, which our runtime writes would otherwise replace.
pub fn shader_dir(config: &Config, paths: &Paths) -> Option<std::path::PathBuf> {
    let own_chain = config.playback.mpv_args.iter().any(|arg| arg.starts_with("--glsl-shader"));
    if own_chain {
        return None;
    }
    crate::shaders::materialize(&paths.cache_dir)
}

/// The companion's palette and font: the immersive palette, because a video window is dark
/// whatever the terminal is.
pub fn player_theme() -> Vec<(String, String)> {
    use anistream_ui::theme::{Palette, Role};
    let palette = Palette::immersive();
    let hex =
        |rgb: anistream_ui::theme::Rgb| format!("#{:02X}{:02X}{:02X}", rgb.r, rgb.g, rgb.b);
    let mut theme: Vec<(String, String)> = [
        ("text", Role::Text),
        ("dim", Role::TextDim),
        ("obi", Role::Obi),
        ("state", Role::State),
        ("alert", Role::Alert),
    ]
    .into_iter()
    .map(|(key, role)| (key.to_owned(), hex(palette.get(role))))
    .collect();
    theme.push(("ground".into(), hex(palette.ground_ref())));
    theme.push(("font".into(), "Zen Kaku Gothic New".into()));
    theme
}
