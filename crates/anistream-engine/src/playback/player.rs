//! The mpv player as the app builds it, from the configuration as it stands now.
//!
//! In the engine rather than the binary so the preview example and the tests build exactly
//! the player a playback gets.

use anistream_core::config::{Config, Paths};
use anistream_model::{Msg, notice::Notice, playback::PlayerTheme};
use anistream_player::protocol::MpvVersion;

/// The player, built from the configuration *as it stands now*.
///
/// Built per playback rather than held from launch, and that is the whole point.
/// `upscaling`, `mpv_binary` and `mpv_args` are all editable on the Settings screen,
/// so a handle made once at startup freezes whatever they were then — turning Anime4K
/// off did nothing until the app was restarted, because `--glsl-shaders` stayed in the
/// argument list every episode was spawned with. Shader files are written on demand and
/// skipped when already current, so rebuilding costs a stat per shader.
pub fn player(config: &Config, paths: &Paths, theme: &PlayerTheme) -> anistream_player::Mpv {
    // Upscaling is no longer on the command line: the chain is chosen per file and set over
    // IPC (see `playback`). What is left here is the HDR curve the viewer picked, before the
    // user's own flags so a hand-written one still wins.
    let mut args = Vec::new();
    let curve = &config.playback.tone_mapping;
    if !curve.is_empty() && curve != crate::playback::picture::DEFAULT_TONE_MAPPING {
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
        .with_theme(theme.options())
        .with_extra_args(args)
}

/// What launch says of the player, or `None` when it plays as it should: that mpv is not there
/// to play anything, or that it is too old to wear anistream's interface.
///
/// Said once at launch rather than at the moment someone presses Enter on an episode. `binary`
/// is what the config calls mpv, the name the notice gives it.
pub async fn launch_notice(mpv: &anistream_player::Mpv, binary: &str) -> Option<Notice> {
    if !mpv.is_available().await {
        return Some(Notice::alert(Msg::NoticeMpvMissing { binary: binary.to_owned() }));
    }
    mpv.mpv_version().await.and_then(old_mpv_msg).map(Notice::info)
}

/// What an older mpv costs, or `None` when it costs nothing.
///
/// Ubuntu 24.04 ships 0.37 and Debian 12 ships 0.35, and the `.deb` installs whichever the system
/// has, so this is a common case rather than an edge one.
pub fn old_mpv_msg(version: MpvVersion) -> Option<Msg> {
    (!version.has_companion_support())
        .then(|| Msg::NoticeMpvOld { version: version.to_string() })
}

/// [`old_mpv_msg`] in English, for the command line's own report.
pub fn old_mpv_note(version: MpvVersion) -> Option<String> {
    old_mpv_msg(version).map(|msg| msg.english())
}

// Where the shaders were written, named from here too, beside the player they dress.
pub use super::shaders::shader_dir;
