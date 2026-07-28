//! Anime4K upscaling as a toggle rather than a scavenger hunt.
//!
//! The shader files are vendored into the binary (MIT, from bloc97/Anime4K — see
//! `assets/shaders/LICENSE-Anime4K`) and written to the cache at launch, so the feature
//! works offline and no config value ever points at a file the user has to go find.
//! The chains are Anime4K's documented "mode A" presets: medium networks for the fast
//! mode, large ones for quality.

use std::path::{Path, PathBuf};

use anistream_core::config::Upscaling;

const SHADERS: &[(&str, &str)] = &[
    (
        "Anime4K_Clamp_Highlights.glsl",
        include_str!("../../../assets/shaders/Anime4K_Clamp_Highlights.glsl"),
    ),
    (
        "Anime4K_Restore_CNN_M.glsl",
        include_str!("../../../assets/shaders/Anime4K_Restore_CNN_M.glsl"),
    ),
    (
        "Anime4K_Restore_CNN_VL.glsl",
        include_str!("../../../assets/shaders/Anime4K_Restore_CNN_VL.glsl"),
    ),
    (
        "Anime4K_Upscale_CNN_x2_M.glsl",
        include_str!("../../../assets/shaders/Anime4K_Upscale_CNN_x2_M.glsl"),
    ),
    (
        "Anime4K_Upscale_CNN_x2_VL.glsl",
        include_str!("../../../assets/shaders/Anime4K_Upscale_CNN_x2_VL.glsl"),
    ),
    (
        "Anime4K_Upscale_CNN_x2_S.glsl",
        include_str!("../../../assets/shaders/Anime4K_Upscale_CNN_x2_S.glsl"),
    ),
    (
        "Anime4K_AutoDownscalePre_x2.glsl",
        include_str!("../../../assets/shaders/Anime4K_AutoDownscalePre_x2.glsl"),
    ),
    (
        "Anime4K_AutoDownscalePre_x4.glsl",
        include_str!("../../../assets/shaders/Anime4K_AutoDownscalePre_x4.glsl"),
    ),
];

/// Anime4K "mode A", medium networks.
const FAST_CHAIN: &[&str] = &[
    "Anime4K_Clamp_Highlights.glsl",
    "Anime4K_Restore_CNN_M.glsl",
    "Anime4K_Upscale_CNN_x2_M.glsl",
    "Anime4K_AutoDownscalePre_x2.glsl",
    "Anime4K_AutoDownscalePre_x4.glsl",
    "Anime4K_Upscale_CNN_x2_S.glsl",
];

/// Anime4K "mode A (HQ)", large networks.
const QUALITY_CHAIN: &[&str] = &[
    "Anime4K_Clamp_Highlights.glsl",
    "Anime4K_Restore_CNN_VL.glsl",
    "Anime4K_Upscale_CNN_x2_VL.glsl",
    "Anime4K_AutoDownscalePre_x2.glsl",
    "Anime4K_AutoDownscalePre_x4.glsl",
    "Anime4K_Upscale_CNN_x2_M.glsl",
];

/// The mpv argument for the configured mode, materialising the shader files first.
///
/// Returns nothing when off or when the cache cannot be written — playback without
/// upscaling beats no playback.
pub fn mpv_args(mode: Upscaling, cache_dir: &Path) -> Vec<String> {
    let chain = match mode {
        Upscaling::Off => return Vec::new(),
        Upscaling::Anime4kFast => FAST_CHAIN,
        Upscaling::Anime4kQuality => QUALITY_CHAIN,
    };

    let dir = cache_dir.join("shaders");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "could not write shaders; upscaling disabled");
        return Vec::new();
    }
    for (name, body) in SHADERS {
        let path = dir.join(name);
        // Sizes differ between releases, so a stale cache from an old binary gets
        // refreshed; matching sizes skip the write.
        let current = path.metadata().ok().map(|m| m.len() as usize);
        if current != Some(body.len())
            && let Err(e) = std::fs::write(&path, body)
        {
            tracing::warn!(error = %e, shader = name, "could not write shader");
            return Vec::new();
        }
    }

    // mpv takes the chain as a platform path list.
    let separator = if cfg!(windows) { ';' } else { ':' };
    let list = chain
        .iter()
        .map(|name| dir.join(name))
        .collect::<Vec<PathBuf>>()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<String>>()
        .join(&separator.to_string());
    vec![format!("--glsl-shaders={list}")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chain_entry_is_a_vendored_shader() {
        for name in FAST_CHAIN.iter().chain(QUALITY_CHAIN) {
            assert!(
                SHADERS.iter().any(|(n, _)| n == name),
                "{name} is chained but not vendored"
            );
        }
    }

    #[test]
    fn off_produces_no_arguments_and_writes_nothing() {
        let dir = std::env::temp_dir().join("anistream-shader-test-off");
        assert!(mpv_args(Upscaling::Off, &dir).is_empty());
        assert!(!dir.join("shaders").exists());
    }

    #[test]
    fn a_mode_produces_one_chain_argument_with_real_files() {
        let dir = std::env::temp_dir().join("anistream-shader-test-fast");
        let args = mpv_args(Upscaling::Anime4kFast, &dir);
        assert_eq!(args.len(), 1);
        assert!(args[0].starts_with("--glsl-shaders="));
        assert!(dir.join("shaders").join("Anime4K_Clamp_Highlights.glsl").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
