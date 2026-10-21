//! Real-time upscaling: two looks that adapt to the source, the display, the battery and
//! the GPU, instead of one fixed chain passed at spawn.
//!
//! - **Natural** is ArtCNN: small convolutional luma doublers trained to reconstruct, not
//!   to restyle. The model follows the source — AniSD for standard-definition masters,
//!   plain ArtCNN for HD — with KrigBilateral
//!   rebuilding chroma from luma and SSimDownscaler taking the doubled image back down when
//!   it overshoots the display.
//! - **Sharp** is Anime4K's own presets: restore, then upscale, with modes A/B/C picked per
//!   source as Anime4K's instructions describe. Crisper lines, stronger cleanup, more
//!   artefacts when the source does not need it.
//!
//! [`choose`] is pure: source, display, look and budget in, a [`Chain`] out. The host sets
//! it at runtime as mpv's `glsl-shaders` (one native array over IPC) whenever a file loads
//! or `video-params` change, and again when the [`super::picture::Governor`] eases, so a
//! chain can change mid-episode without a respawn.
//!
//! Every shader is vendored into the binary and written to a content-addressed cache
//! directory by [`materialize`], so the feature works offline and no setting ever points at
//! a file the user has to find. Provenance is in `assets/shaders/THIRD-PARTY.md`. Licences
//! differ and matter: Anime4K and ArtCNN are MIT; KrigBilateral and SSimDownscaler are
//! LGPL-3.0; **the AniSD model is CC BY-NC 4.0**, bundled because anistream is free and
//! non-commercial — anyone redistributing anistream commercially must drop that file.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anistream_core::config::Upscaling;
use serde_json::{Value, json};

use crate::playback::picture::HdrKind;

// --- The files ---------------------------------------------------------------------------

/// A vendored shader: its file name in the cache directory and its source.
struct Shader {
    name: &'static str,
    body: &'static str,
}

macro_rules! shader {
    ($name:literal) => {
        Shader {
            name: $name,
            // Found from the crate's root rather than from this file, so the file can move.
            body: include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/shaders/",
                $name
            )),
        }
    };
}

/// Every shader [`materialize`] writes. Order is irrelevant; chains name files.
const SHADERS: &[Shader] = &[
    // Anime4K v4.0.1 (MIT).
    shader!("Anime4K_Clamp_Highlights.glsl"),
    shader!("Anime4K_Restore_CNN_M.glsl"),
    shader!("Anime4K_Restore_CNN_VL.glsl"),
    shader!("Anime4K_Restore_CNN_Soft_M.glsl"),
    shader!("Anime4K_Restore_CNN_Soft_VL.glsl"),
    shader!("Anime4K_Upscale_CNN_x2_S.glsl"),
    shader!("Anime4K_Upscale_CNN_x2_M.glsl"),
    shader!("Anime4K_Upscale_CNN_x2_VL.glsl"),
    shader!("Anime4K_Upscale_Denoise_CNN_x2_M.glsl"),
    shader!("Anime4K_Upscale_Denoise_CNN_x2_VL.glsl"),
    shader!("Anime4K_AutoDownscalePre_x2.glsl"),
    shader!("Anime4K_AutoDownscalePre_x4.glsl"),
    // ArtCNN v1.6.2 (MIT).
    shader!("ArtCNN_C4F16.glsl"),
    // ArtCNN-architecture models trained by Sirosky (CC BY-NC 4.0, not MIT).
    shader!("AniSD_ArtCNN_C4F32_i4.glsl"),
    // igv's mpv shaders (LGPL-3.0).
    shader!("KrigBilateral.glsl"),
    shader!("SSimDownscaler.glsl"),
];

/// Licences and provenance, written beside the shaders so the directory mpv reads from
/// carries its own terms: the LGPL and CC BY licences both ask that recipients get them.
const NOTICES: &[Shader] = &[
    shader!("THIRD-PARTY.md"),
    shader!("LICENSE-Anime4K"),
    shader!("LICENSE-ArtCNN"),
    shader!("LICENSE-Upscale-Hub"),
    shader!("LICENSE-LGPL-3.0"),
    shader!("LICENSE-GPL-3.0"),
];

const ANISD: &str = "AniSD_ArtCNN_C4F32_i4.glsl";
const ARTCNN_LIGHT: &str = "ArtCNN_C4F16.glsl";
const KRIG: &str = "KrigBilateral.glsl";
const SSIM: &str = "SSimDownscaler.glsl";

// --- Looks and the menu ------------------------------------------------------------------

/// What the user picks: a look, not a shader list. The chooser does the rest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Look {
    /// No shaders at all; mpv's own scalers.
    Off,
    /// ArtCNN models: faithful lines, no restyling.
    #[default]
    Natural,
    /// Anime4K: crisper lines, stronger restoration.
    Sharp,
}

impl Look {
    /// The id used in config, the menu and the `choose` event.
    pub fn id(self) -> &'static str {
        match self {
            Look::Off => "off",
            Look::Natural => "natural",
            Look::Sharp => "sharp",
        }
    }

    pub fn from_id(id: &str) -> Option<Look> {
        match id {
            "off" => Some(Look::Off),
            "natural" => Some(Look::Natural),
            "sharp" => Some(Look::Sharp),
            _ => None,
        }
    }
}

/// The setting, as a look. The old Anime4K modes already read as `Sharp` in the config
/// (serde aliases): those users chose Anime4K's look, and the fast/quality split is now the
/// budget's job.
impl From<Upscaling> for Look {
    fn from(setting: Upscaling) -> Look {
        match setting {
            Upscaling::Off => Look::Off,
            Upscaling::Natural => Look::Natural,
            Upscaling::Sharp => Look::Sharp,
        }
    }
}

/// One row of the in-player upscaling menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuOption {
    pub id: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

pub const MENU: [MenuOption; 3] = [
    MenuOption {
        id: "natural",
        label: "Natural",
        hint: "ArtCNN · faithful lines, adapts to the source",
    },
    MenuOption { id: "sharp", label: "Sharp", hint: "Anime4K · crisper lines, heavier" },
    MenuOption { id: "off", label: "Off", hint: "The source as it is" },
];

/// The `menus.upscaling` value for the companion script.
pub fn menu(current: Look) -> Value {
    json!({
        "current": current.id(),
        "options": MENU
            .iter()
            .map(|o| json!({ "id": o.id, "label": o.label, "hint": o.hint }))
            .collect::<Vec<_>>(),
    })
}

// --- The chooser -------------------------------------------------------------------------

/// The video as mpv will scale it: `video-params/dw` × `dh` (aspect-corrected), plus what
/// [`super::picture::classify`] made of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub width: u32,
    pub height: u32,
    pub hdr: Option<HdrKind>,
    /// Whether the frames reach the renderer as planes, luma apart from chroma, which LUMA and
    /// CHROMA hooks read. Software decoding and copying hardware decoders give planes; Android's
    /// zero-copy MediaCodec (`video-params/pixelformat` `mediacodec`) gives one RGB texture,
    /// which only MAIN hooks can work on.
    pub planar: bool,
}

/// The monitor, in physical pixels: mpv's `display-width` × `display-height` (3024 × 1964
/// on a 14" MacBook Pro, with `display-hidpi-scale` 2). The window is deliberately not
/// used: resizing it would churn the chain, and fullscreen is the case worth getting right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Display {
    pub width: u32,
    pub height: u32,
    /// Whether the renderer runs compute shaders, which ArtCNN C4F16's passes are: libplacebo
    /// has none on OpenGL ES.
    pub compute: bool,
}

/// What the machine can spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Budget {
    /// [`super::power::on_battery`] said yes: start one step lighter.
    pub on_battery: bool,
    /// Steps the governor has taken this episode ([`super::picture::Governor::eased`]).
    pub eased: u8,
}

/// A chain to hand to mpv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chain {
    /// File names in [`materialize`]'s directory, in order. Empty means `glsl-shaders=[]`.
    pub files: Vec<&'static str>,
    /// What runs, for a toast or the log: "ArtCNN AniSD", "Anime4K A (Fast)".
    pub label: &'static str,
    /// The `media.upscaler` badge: `"ArtCNN"`, `"Anime4K"`, or none when nothing upscales.
    pub upscaler: Option<&'static str>,
}

impl Chain {
    fn new(
        files: Vec<&'static str>,
        label: &'static str,
        upscaler: Option<&'static str>,
    ) -> Chain {
        Chain { files, label, upscaler }
    }

    fn off() -> Chain {
        Chain::new(Vec::new(), "Off", None)
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

/// Source tiers, by the lines the source would fill on a 16:9 frame — `max(h, w·9/16)` — so
/// a 1920×800 scope crop is 1080-class and a 1440×1080 4:3 master is too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tier {
    /// 480/540/576: DVD-era masters and SD web encodes.
    Sd,
    /// 720-class.
    Hd,
    /// 1080 and up.
    Fhd,
}

fn tier(source: &Source) -> Tier {
    let lines = source.height.max(source.width.saturating_mul(9) / 16);
    match lines {
        0..600 => Tier::Sd,
        600..900 => Tier::Hd,
        _ => Tier::Fhd,
    }
}

/// Assumed when mpv cannot say. 1080p is the commonest panel and the conservative guess:
/// it never runs a network on a 1080p source that may already be native, while SD and
/// 720p sources need upscaling on any display that size or larger.
const FALLBACK_DISPLAY: Display = Display { width: 1920, height: 1080, compute: true };

/// Below this scale factor nothing CNN-sized runs. It is ArtCNN's own `//!WHEN` threshold,
/// so the chooser and the shaders agree on what "needs upscaling" means.
const CNN_MIN_SCALE: f64 = 1.3;

/// Steps below full before only chroma is left. Matches the governor's default cap.
const LEVELS: u8 = 2;

/// Pick the chain. The rules, in order:
///
/// 1. **Off** is empty, always.
/// 2. **Dolby Vision gets nothing.** libplacebo applies the RPU's reshaping after the
///    planes are read, so a LUMA or CHROMA hook would see the pre-reshape signal, not
///    YCbCr; no shader here was made for that.
/// 3. **Other HDR (PQ, HLG) gets chroma only.** Every network here was trained on SDR,
///    gamma-encoded BT.709 frames, and mpv runs hooks before tone mapping, on PQ/HLG code
///    values. That is off-distribution input, and PQ makes errors expensive: a small
///    overshoot near the top of the curve is hundreds of nits. Until a CNN is checked on
///    real HDR masters, it stays off. KrigBilateral is safe: it predicts chroma from luma
///    with a local linear model over code values, with no transfer function or matrix in
///    it — the same domain mpv's own chroma scaler works in. SSimDownscaler is left out:
///    its weighting assumes BT.709 luma and a gamma-2 curve.
/// 4. **No CNN when the source already meets the display** (scale ≤ 1.3, see
///    [`CNN_MIN_SCALE`]): doubling only to throw the result away is the most expensive way
///    to do nothing. Natural gets chroma only; SSimDownscaler joins when the source is
///    larger than the display. Sharp, when the scale is 1.0–1.3 and the tier has a
///    restore pass (modes A/B), runs Anime4K's Restore at native size — Anime4K documents
///    that mode A improves 1080p on a 1080p screen — and otherwise falls to chroma only.
/// 5. **Natural upscaling**: AniSD for SD, as its author's own auto-profile assigns it, and
///    ArtCNN C4F16 for everything HD. The author's profile puts 720p and 1080p on the Ani4K
///    v2 model instead, and it was measured: on an M4 Pro (16-core GPU, fullscreen
///    3024×1701) Ani4K v2 only just holds real time on a 720p source and drops frames on
///    1080p, as does plain ArtCNN C4F32, while C4F16 holds 2× on 720p and 1.5× on 1080p.
///    Side by side at 2× zoom the difference from Ani4K v2 is slight; a default that
///    stutters on most machines until the governor steps in is not, so it is not shipped.
///    KrigBilateral always; SSimDownscaler when the doubled image overshoots the display
///    (scale < 2).
/// 6. **Sharp upscaling** uses Anime4K's documented presets by tier, as its quick-usage
///    table assigns them: mode C for 480p, B for 720p, A for 1080p; HQ (VL networks) at
///    full budget and Fast (M) one step down. 1080p starts at Fast for the same reason as
///    Natural: mode A (HQ) drops frames at real time on the M4 Pro, Fast holds 2×. The
///    presets carry their own AutoDownscalePre passes, so they need no SSimDownscaler.
/// 7. **Budget.** Battery starts one step down; each governor step goes one more. Past the
///    lightest network is chroma only. Nothing below chroma: if KrigBilateral alone drops
///    frames, shaders are not the problem.
/// 8. **What the renderer can run.** Without compute shaders ArtCNN C4F16 is no rung: HD and
///    1080p go to chroma only rather than to Anime4K, which would be another look, and SD keeps
///    AniSD, a fragment shader. Without planar frames there is nothing for a LUMA or CHROMA
///    hook to read, so ArtCNN, AniSD and KrigBilateral are out: Natural has nothing left but
///    SSimDownscaler taking an oversized picture down, and Sharp, whose Anime4K hooks MAIN,
///    runs as ever.
pub fn choose(look: Look, source: &Source, display: Option<Display>, budget: Budget) -> Chain {
    if look == Look::Off {
        return Chain::off();
    }
    match source.hdr {
        Some(HdrKind::DolbyVision) => {
            return Chain::new(Vec::new(), "Dolby Vision: none", None);
        }
        Some(_) if !source.planar => return Chain::new(Vec::new(), "HDR: none", None),
        Some(_) => return Chain::new(vec![KRIG], "HDR: chroma only", None),
        None => {}
    }

    let display = display.filter(|d| d.width > 0 && d.height > 0).unwrap_or(FALLBACK_DISPLAY);
    let scale = if source.width == 0 || source.height == 0 {
        1.0
    } else {
        (f64::from(display.width) / f64::from(source.width))
            .min(f64::from(display.height) / f64::from(source.height))
    };
    let level = budget.eased.saturating_add(u8::from(budget.on_battery)).min(LEVELS);
    let tier = tier(source);

    let chroma = || match (source.planar, scale < 1.0) {
        (true, true) => Chain::new(vec![KRIG, SSIM], "Chroma only", None),
        (true, false) => Chain::new(vec![KRIG], "Chroma only", None),
        // No chroma plane for KrigBilateral to rebuild; SSimDownscaler works on the scaled
        // picture, which there still is.
        (false, true) => Chain::new(vec![SSIM], "Downscale only", None),
        (false, false) => Chain::new(Vec::new(), "Zero-copy frames: none", None),
    };

    if scale <= CNN_MIN_SCALE {
        return match look {
            Look::Sharp if scale >= 1.0 => sharp_native(tier, level).unwrap_or_else(chroma),
            _ => chroma(),
        };
    }

    match look {
        Look::Off => Chain::off(),
        // Every Natural network hooks LUMA.
        Look::Natural if !source.planar => chroma(),
        Look::Natural => {
            let rungs: &[(&str, &str)] = match (tier, display.compute) {
                (Tier::Sd, true) => &[(ANISD, "ArtCNN AniSD"), (ARTCNN_LIGHT, "ArtCNN C4F16")],
                (Tier::Sd, false) => &[(ANISD, "ArtCNN AniSD")],
                (Tier::Hd | Tier::Fhd, true) => &[(ARTCNN_LIGHT, "ArtCNN C4F16")],
                (Tier::Hd | Tier::Fhd, false) => &[],
            };
            let Some(&(model, label)) = rungs.get(usize::from(level)) else {
                return chroma();
            };
            let mut files = vec![model, KRIG];
            if scale < 2.0 {
                files.push(SSIM);
            }
            Chain::new(files, label, Some("ArtCNN"))
        }
        Look::Sharp => {
            // `true` is HQ (VL networks), `false` Fast (M).
            let (mode, rungs): (Mode, &[bool]) = match tier {
                Tier::Sd => (Mode::C, &[true, false]),
                Tier::Hd => (Mode::B, &[true, false]),
                Tier::Fhd => (Mode::A, &[false]),
            };
            match rungs.get(usize::from(level)) {
                Some(&hq) => Chain::new(mode.chain(hq), mode.label(hq), Some("Anime4K")),
                None => chroma(),
            }
        }
    }
}

/// Anime4K's three modes (`md/GLSL_Instructions_Advanced.md`, v4.x): A restores blur and
/// resampling damage (most 1080p), B is softer and targets ringing and aliasing (most
/// 720p, 1080p downscaled to 720p), C is denoise-and-upscale for clean or downscaled SD.
#[derive(Debug, Clone, Copy)]
enum Mode {
    A,
    B,
    C,
}

impl Mode {
    /// The template presets, file for file (`md/Template/GLSL_Mac_Linux_{High,Low}-end`).
    fn chain(self, hq: bool) -> Vec<&'static str> {
        let first: &[&str] = match (self, hq) {
            (Mode::A, true) => {
                &["Anime4K_Restore_CNN_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl"]
            }
            (Mode::A, false) => {
                &["Anime4K_Restore_CNN_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl"]
            }
            (Mode::B, true) => {
                &["Anime4K_Restore_CNN_Soft_VL.glsl", "Anime4K_Upscale_CNN_x2_VL.glsl"]
            }
            (Mode::B, false) => {
                &["Anime4K_Restore_CNN_Soft_M.glsl", "Anime4K_Upscale_CNN_x2_M.glsl"]
            }
            (Mode::C, true) => &["Anime4K_Upscale_Denoise_CNN_x2_VL.glsl"],
            (Mode::C, false) => &["Anime4K_Upscale_Denoise_CNN_x2_M.glsl"],
        };
        let last =
            if hq { "Anime4K_Upscale_CNN_x2_M.glsl" } else { "Anime4K_Upscale_CNN_x2_S.glsl" };
        let mut files = vec!["Anime4K_Clamp_Highlights.glsl"];
        files.extend_from_slice(first);
        files.extend_from_slice(&[
            "Anime4K_AutoDownscalePre_x2.glsl",
            "Anime4K_AutoDownscalePre_x4.glsl",
            last,
        ]);
        files
    }

    fn label(self, hq: bool) -> &'static str {
        match (self, hq) {
            (Mode::A, true) => "Anime4K A (HQ)",
            (Mode::A, false) => "Anime4K A (Fast)",
            (Mode::B, true) => "Anime4K B (HQ)",
            (Mode::B, false) => "Anime4K B (Fast)",
            (Mode::C, true) => "Anime4K C (HQ)",
            (Mode::C, false) => "Anime4K C (Fast)",
        }
    }
}

/// Sharp at native size: the mode's restore pass alone. Mode C has none (it starts with an
/// upscaler, which would skip itself), so SD falls through to chroma only.
fn sharp_native(tier: Tier, level: u8) -> Option<Chain> {
    let restore = match (tier, level) {
        (Tier::Fhd, 0) => ("Anime4K_Restore_CNN_VL.glsl", "Anime4K A restore (HQ)"),
        (Tier::Fhd, 1) => ("Anime4K_Restore_CNN_M.glsl", "Anime4K A restore (Fast)"),
        (Tier::Hd, 0) => ("Anime4K_Restore_CNN_Soft_VL.glsl", "Anime4K B restore (HQ)"),
        (Tier::Hd, 1) => ("Anime4K_Restore_CNN_Soft_M.glsl", "Anime4K B restore (Fast)"),
        _ => return None,
    };
    Some(Chain::new(
        vec!["Anime4K_Clamp_Highlights.glsl", restore.0],
        restore.1,
        Some("Anime4K"),
    ))
}

// --- Files on disk -----------------------------------------------------------------------

/// A digest of every vendored file, naming the cache directory.
///
/// Computed once per process. `DefaultHasher::new()` uses fixed keys, so the same binary
/// always lands in the same directory; a different binary with different shaders lands in
/// another, which is the point.
fn digest() -> &'static str {
    static DIGEST: OnceLock<String> = OnceLock::new();
    DIGEST.get_or_init(|| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for s in SHADERS.iter().chain(NOTICES) {
            s.name.hash(&mut h);
            s.body.hash(&mut h);
        }
        format!("{:016x}", h.finish())
    })
}

/// Marker written last: its presence means every file before it is complete.
const COMPLETE: &str = ".complete";

/// Where the upscaling shaders were written, or `None` when anistream should leave
/// `glsl-shaders` alone: the cache could not be written, or the user passes their own chain
/// in `playback.mpv_args`, which our runtime writes would otherwise replace.
pub fn shader_dir(
    config: &anistream_core::config::Config,
    paths: &anistream_core::config::Paths,
) -> Option<std::path::PathBuf> {
    let own_chain = config.playback.mpv_args.iter().any(|arg| arg.starts_with("--glsl-shader"));
    if own_chain {
        return None;
    }
    materialize(&paths.cache_dir)
}

/// Write every vendored shader to `<cache_dir>/shaders/<digest>/` and return that
/// directory, or `None` when the cache cannot be written — playback without upscaling
/// beats no playback.
///
/// Skips all writes when the directory is already complete, so calling this per playback
/// costs one `stat`. Files go through a temporary name and a rename, so a second anistream
/// writing the same directory never leaves mpv a half-written shader. Older directories
/// (and the flat files earlier versions wrote) are removed on the way; a file another
/// process still holds open on Windows just stays until next time.
pub fn materialize(cache_dir: &Path) -> Option<PathBuf> {
    let root = cache_dir.join("shaders");
    let dir = root.join(digest());
    if dir.join(COMPLETE).is_file() {
        return Some(dir);
    }
    if let Err(e) = write_all(&dir) {
        tracing::warn!(error = %e, dir = %dir.display(), "could not write shaders; upscaling disabled");
        return None;
    }
    if let Ok(entries) = std::fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path == dir {
                continue;
            }
            let _ = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
        }
    }
    Some(dir)
}

fn write_all(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let suffix = format!(".tmp-{}", std::process::id());
    for s in SHADERS.iter().chain(NOTICES) {
        let tmp = dir.join(format!("{}{suffix}", s.name));
        std::fs::write(&tmp, s.body)?;
        std::fs::rename(&tmp, dir.join(s.name))?;
    }
    std::fs::write(dir.join(COMPLETE), digest())
}

/// The vendored file a hook belongs to, by the description libplacebo names it by in its log:
/// a `//!DESC` line, such as `ArtCNN C4F32 (Conv2D-2-ReLU)`. `None` for a hook none of them
/// describes.
pub fn hook_file(hook: &str) -> Option<&'static str> {
    let hook = hook.trim();
    SHADERS
        .iter()
        .find(|shader| {
            shader
                .body
                .lines()
                .filter_map(|line| line.strip_prefix("//!DESC"))
                .map(str::trim)
                .any(|desc| !desc.is_empty() && hook.contains(desc))
        })
        .map(|shader| shader.name)
}

/// Absolute paths for `glsl-shaders`, in chain order. Set them as one native array over
/// IPC — no list separator, so no escaping of `:` or `;` in a path.
pub fn paths(chain: &Chain, dir: &Path) -> Vec<String> {
    chain.files.iter().map(|name| dir.join(name).display().to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Anime4K "mode A", medium networks — what `anime4k-fast` used to be.
    const LEGACY_FAST: &[&str] = &[
        "Anime4K_Clamp_Highlights.glsl",
        "Anime4K_Restore_CNN_M.glsl",
        "Anime4K_Upscale_CNN_x2_M.glsl",
        "Anime4K_AutoDownscalePre_x2.glsl",
        "Anime4K_AutoDownscalePre_x4.glsl",
        "Anime4K_Upscale_CNN_x2_S.glsl",
    ];

    /// Anime4K "mode A (HQ)", large networks — what `anime4k-quality` used to be.
    const LEGACY_QUALITY: &[&str] = &[
        "Anime4K_Clamp_Highlights.glsl",
        "Anime4K_Restore_CNN_VL.glsl",
        "Anime4K_Upscale_CNN_x2_VL.glsl",
        "Anime4K_AutoDownscalePre_x2.glsl",
        "Anime4K_AutoDownscalePre_x4.glsl",
        "Anime4K_Upscale_CNN_x2_M.glsl",
    ];

    const FHD: Source = Source { width: 1920, height: 1080, hdr: None, planar: true };
    const HD: Source = Source { width: 1280, height: 720, hdr: None, planar: true };
    const SD: Source = Source { width: 854, height: 480, hdr: None, planar: true };
    const MAC: Display = Display { width: 3024, height: 1964, compute: true };
    const UHD: Display = Display { width: 3840, height: 2160, compute: true };
    const QHD: Display = Display { width: 2560, height: 1440, compute: true };
    const FULL: Budget = Budget { on_battery: false, eased: 0 };

    fn vendored(name: &str) -> bool {
        SHADERS.iter().any(|s| s.name == name)
    }

    /// Every chain `choose` can produce, over a grid of inputs.
    fn every_chain() -> Vec<Chain> {
        let sources = [
            FHD,
            HD,
            SD,
            Source { width: 720, height: 576, hdr: None, planar: true },
            Source { width: 1920, height: 800, hdr: None, planar: true },
            Source { width: 3840, height: 2160, hdr: None, planar: true },
            Source { width: 0, height: 0, hdr: None, planar: true },
            Source { hdr: Some(HdrKind::Hdr10), ..FHD },
            Source { hdr: Some(HdrKind::DolbyVision), ..FHD },
        ];
        let displays = [None, Some(MAC), Some(UHD), Some(QHD), Some(FALLBACK_DISPLAY)];
        let mut out = Vec::new();
        for look in [Look::Off, Look::Natural, Look::Sharp] {
            for s in &sources {
                for d in displays {
                    for on_battery in [false, true] {
                        for eased in 0..=4 {
                            out.push(choose(look, s, d, Budget { on_battery, eased }));
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn every_chosen_file_is_vendored_and_used_once() {
        for chain in every_chain() {
            for name in &chain.files {
                assert!(vendored(name), "{name} is chained but not vendored");
            }
            let mut sorted = chain.files.clone();
            sorted.sort_unstable();
            sorted.dedup();
            // Anime4K: "Using the same file two or more times causes buggy behaviour".
            assert_eq!(sorted.len(), chain.files.len(), "{chain:?}");
        }
        for name in LEGACY_FAST.iter().chain(LEGACY_QUALITY) {
            assert!(vendored(name), "{name} is chained but not vendored");
        }
    }

    #[test]
    fn every_vendored_file_is_reachable() {
        let chains = every_chain();
        for s in SHADERS {
            assert!(
                chains.iter().any(|c| c.files.contains(&s.name)),
                "{} is vendored but no chain uses it",
                s.name
            );
        }
    }

    #[test]
    fn off_is_empty() {
        assert!(choose(Look::Off, &SD, Some(UHD), FULL).is_empty());
    }

    #[test]
    fn natural_follows_the_model_authors_by_tier() {
        let sd = choose(Look::Natural, &SD, Some(MAC), FULL);
        assert_eq!(sd.files, [ANISD, KRIG]);
        assert_eq!(sd.upscaler, Some("ArtCNN"));
        let hd = choose(Look::Natural, &HD, Some(MAC), FULL);
        assert_eq!(hd.files, [ARTCNN_LIGHT, KRIG]);
        let fhd = choose(Look::Natural, &FHD, Some(MAC), FULL);
        assert_eq!(fhd.files, [ARTCNN_LIGHT, KRIG, SSIM]);
    }

    #[test]
    fn a_576_line_pal_master_is_sd_and_a_scope_crop_is_1080_class() {
        let pal = Source { width: 720, height: 576, hdr: None, planar: true };
        assert_eq!(choose(Look::Natural, &pal, Some(UHD), FULL).files[0], ANISD);
        let scope = Source { width: 1920, height: 800, hdr: None, planar: true };
        assert_eq!(tier(&scope), Tier::Fhd);
    }

    #[test]
    fn ssim_joins_only_when_the_doubled_image_overshoots() {
        // 720p on a 4K panel: doubled is 1440, still under 2160.
        assert!(!choose(Look::Natural, &HD, Some(UHD), FULL).files.contains(&SSIM));
        // 720p on a 1440p panel: scale 2.0 exactly, doubled lands on the panel.
        assert!(!choose(Look::Natural, &HD, Some(QHD), FULL).files.contains(&SSIM));
        // 1080p on a 1440p panel: doubled is 2160, downscaled to 1440.
        assert!(choose(Look::Natural, &FHD, Some(QHD), FULL).files.contains(&SSIM));
    }

    #[test]
    fn no_cnn_when_the_source_meets_the_display() {
        let native = choose(Look::Natural, &FHD, Some(FALLBACK_DISPLAY), FULL);
        assert_eq!(native.files, [KRIG]);
        assert_eq!(native.upscaler, None);
        // A 4K source on a 1080p panel: downscaling, so SSimDownscaler too.
        let big = Source { width: 3840, height: 2160, hdr: None, planar: true };
        assert_eq!(
            choose(Look::Natural, &big, Some(FALLBACK_DISPLAY), FULL).files,
            [KRIG, SSIM]
        );
        assert_eq!(choose(Look::Sharp, &big, Some(FALLBACK_DISPLAY), FULL).files, [KRIG, SSIM]);
        // 900p-ish on 1080p is under the 1.3 threshold.
        let near = Source { width: 1600, height: 900, hdr: None, planar: true };
        assert_eq!(choose(Look::Natural, &near, Some(FALLBACK_DISPLAY), FULL).files, [KRIG]);
    }

    #[test]
    fn sharp_at_native_size_restores_without_upscaling() {
        let native = choose(Look::Sharp, &FHD, Some(FALLBACK_DISPLAY), FULL);
        assert_eq!(
            native.files,
            ["Anime4K_Clamp_Highlights.glsl", "Anime4K_Restore_CNN_VL.glsl"]
        );
        let eased = choose(
            Look::Sharp,
            &FHD,
            Some(FALLBACK_DISPLAY),
            Budget { on_battery: true, eased: 0 },
        );
        assert_eq!(eased.files[1], "Anime4K_Restore_CNN_M.glsl");
        let floor = choose(
            Look::Sharp,
            &FHD,
            Some(FALLBACK_DISPLAY),
            Budget { on_battery: true, eased: 1 },
        );
        assert_eq!(floor.files, [KRIG]);
    }

    #[test]
    fn an_unknown_display_is_treated_as_1080p() {
        assert_eq!(choose(Look::Natural, &FHD, None, FULL).files, [KRIG]);
        assert_eq!(choose(Look::Natural, &HD, None, FULL).files[0], ARTCNN_LIGHT);
        let zero = Some(Display { width: 0, height: 0, compute: true });
        assert_eq!(
            choose(Look::Natural, &HD, zero, FULL),
            choose(Look::Natural, &HD, None, FULL)
        );
    }

    #[test]
    fn sharp_uses_anime4ks_mode_per_tier() {
        // 1080p starts at Fast: exactly the old anime4k-fast chain.
        let a = choose(Look::Sharp, &FHD, Some(UHD), FULL);
        assert_eq!(a.label, "Anime4K A (Fast)");
        assert_eq!(a.files, LEGACY_FAST);
        let b = choose(Look::Sharp, &HD, Some(UHD), FULL);
        assert_eq!(b.files[1], "Anime4K_Restore_CNN_Soft_VL.glsl");
        let c = choose(Look::Sharp, &SD, Some(UHD), FULL);
        assert_eq!(c.files[1], "Anime4K_Upscale_Denoise_CNN_x2_VL.glsl");
        assert_eq!(c.upscaler, Some("Anime4K"));
        // One step down from there is chroma only.
        let eased =
            choose(Look::Sharp, &FHD, Some(UHD), Budget { on_battery: false, eased: 1 });
        assert_eq!(eased.files, [KRIG]);
    }

    #[test]
    fn battery_and_governor_steps_walk_down_to_chroma_and_stop() {
        let at = |on_battery, eased| {
            choose(Look::Natural, &SD, Some(MAC), Budget { on_battery, eased }).files
        };
        assert_eq!(at(false, 0), [ANISD, KRIG]);
        assert_eq!(at(true, 0), [ARTCNN_LIGHT, KRIG]);
        assert_eq!(at(false, 1), [ARTCNN_LIGHT, KRIG]);
        assert_eq!(at(true, 1), [KRIG]);
        assert_eq!(at(false, 2), [KRIG]);
        assert_eq!(at(false, 200), [KRIG]);
        // 1080p starts at the light network, so its first step is chroma.
        let fhd = choose(Look::Natural, &FHD, Some(MAC), Budget { on_battery: true, eased: 0 });
        assert_eq!(fhd.files, [KRIG]);
        // Sharp: HQ, then Fast, then chroma.
        let sharp =
            |eased| choose(Look::Sharp, &HD, Some(MAC), Budget { on_battery: false, eased });
        assert_eq!(sharp(0).label, "Anime4K B (HQ)");
        assert_eq!(sharp(1).label, "Anime4K B (Fast)");
        assert_eq!(sharp(2).files, [KRIG]);
    }

    #[test]
    fn without_compute_natural_steps_to_chroma_only_and_never_to_anime4k() {
        // A phone's GLES renderer, on its 2400 × 1080 panel.
        let gles = Display { width: 2400, height: 1080, compute: false };
        let at = |source: &Source, eased| {
            choose(Look::Natural, source, Some(gles), Budget { on_battery: false, eased })
        };
        // HD and 1080p had ArtCNN C4F16 alone: chroma only now, said so, and no badge. 1080p
        // on a 4K tablet, where it would have doubled.
        let tablet = Display { width: 3840, height: 2160, compute: false };
        for chain in [at(&HD, 0), choose(Look::Natural, &FHD, Some(tablet), FULL)] {
            assert_eq!((chain.files.as_slice(), chain.label), (&[KRIG][..], "Chroma only"));
            assert_eq!(chain.upscaler, None);
        }
        // SD keeps AniSD, a fragment shader, and its next step is chroma, not C4F16.
        assert_eq!(at(&SD, 0).files, [ANISD, KRIG]);
        assert_eq!(at(&SD, 1).files, [KRIG]);
        assert!(at(&SD, 1).files.iter().all(|f| !f.starts_with("Anime4K")));
        // Sharp's Anime4K is fragment shaders throughout, and is as it was.
        let with_compute = Display { compute: true, ..gles };
        assert_eq!(
            choose(Look::Sharp, &HD, Some(gles), FULL),
            choose(Look::Sharp, &HD, Some(with_compute), FULL)
        );
    }

    #[test]
    fn zero_copy_frames_leave_nothing_that_reads_planes() {
        let zero_copy = |source: Source| Source { planar: false, ..source };
        let hooks_planes =
            |chain: &Chain| chain.files.iter().any(|f| [ANISD, ARTCNN_LIGHT, KRIG].contains(f));
        // Natural has no network left, nor KrigBilateral.
        for source in [SD, HD, FHD] {
            for display in [MAC, UHD, FALLBACK_DISPLAY] {
                let chain = choose(Look::Natural, &zero_copy(source), Some(display), FULL);
                assert!(!hooks_planes(&chain), "{source:?} on {display:?}: {chain:?}");
                assert_eq!(chain.upscaler, None);
            }
        }
        assert_eq!(
            choose(Look::Natural, &zero_copy(HD), Some(MAC), FULL).label,
            "Zero-copy frames: none"
        );
        // A picture larger than the display is still taken down well.
        let big = Source { width: 3840, height: 2160, hdr: None, planar: false };
        let down = choose(Look::Natural, &big, Some(FALLBACK_DISPLAY), FULL);
        assert_eq!((down.files.as_slice(), down.label), (&[SSIM][..], "Downscale only"));
        // Sharp's Anime4K hooks the picture itself, and runs as on planes.
        assert_eq!(
            choose(Look::Sharp, &zero_copy(HD), Some(MAC), FULL),
            choose(Look::Sharp, &HD, Some(MAC), FULL)
        );
        // And where Sharp would fall to chroma only, it falls to nothing.
        let floor = Budget { on_battery: false, eased: 2 };
        assert!(!hooks_planes(&choose(Look::Sharp, &zero_copy(HD), Some(MAC), floor)));
        // HDR has no chroma pass to keep.
        let hdr = Source { hdr: Some(HdrKind::Hdr10), ..zero_copy(FHD) };
        assert!(choose(Look::Natural, &hdr, Some(UHD), FULL).is_empty());
    }

    #[test]
    fn a_disabled_hook_is_traced_to_its_file_by_its_description() {
        assert_eq!(hook_file("ArtCNN C4F32 (Conv2D-2-ReLU)"), Some(ANISD));
        assert_eq!(hook_file("ArtCNN C4F16 (Conv2D)"), Some(ARTCNN_LIGHT));
        // As a log line may quote it.
        assert_eq!(hook_file("'KrigBilateral Upscaling UV'"), Some(KRIG));
        assert_eq!(hook_file("some user shader"), None);
        assert_eq!(hook_file(""), None);
    }

    #[test]
    fn hdr_gets_chroma_only_and_dolby_vision_nothing() {
        for kind in [HdrKind::Hdr10, HdrKind::Hdr10Plus, HdrKind::Hlg] {
            let src = Source { hdr: Some(kind), ..SD };
            for look in [Look::Natural, Look::Sharp] {
                let chain = choose(look, &src, Some(UHD), FULL);
                assert_eq!(chain.files, [KRIG], "{kind:?} {look:?}");
                assert_eq!(chain.upscaler, None);
            }
        }
        let dv = Source { hdr: Some(HdrKind::DolbyVision), ..FHD };
        assert!(choose(Look::Natural, &dv, Some(UHD), FULL).is_empty());
        assert!(
            choose(Look::Off, &Source { hdr: Some(HdrKind::Hdr10), ..FHD }, None, FULL)
                .is_empty()
        );
    }

    #[test]
    fn looks_round_trip_and_old_settings_map() {
        for look in [Look::Off, Look::Natural, Look::Sharp] {
            assert_eq!(Look::from_id(look.id()), Some(look));
        }
        assert_eq!(Look::from_id("natural-auto"), None);
        assert_eq!(Look::from(Upscaling::Off), Look::Off);
        assert_eq!(Look::from(Upscaling::Natural), Look::Natural);
        assert_eq!(Look::from(Upscaling::Sharp), Look::Sharp);
        assert_eq!(Look::default(), Look::Natural);
    }

    #[test]
    fn the_menu_is_the_protocol_shape() {
        let m = menu(Look::Sharp);
        assert_eq!(m["current"], "sharp");
        let ids: Vec<_> =
            m["options"].as_array().unwrap().iter().map(|o| o["id"].clone()).collect();
        assert_eq!(ids, ["natural", "sharp", "off"]);
        for o in m["options"].as_array().unwrap() {
            assert!(o["label"].is_string() && o["hint"].is_string());
            assert!(Look::from_id(o["id"].as_str().unwrap()).is_some());
        }
    }

    #[test]
    fn non_commercial_models_keep_their_attribution_header() {
        let body = SHADERS.iter().find(|s| s.name == ANISD).unwrap().body;
        assert!(body.starts_with("// "));
        assert!(body.lines().next().unwrap().contains("CC BY-NC 4.0"));
    }

    #[test]
    fn materialize_writes_once_and_paths_point_at_real_files() {
        let tmp = tempfile::tempdir().unwrap();
        // A flat file from an older version, which should be cleaned away.
        std::fs::create_dir_all(tmp.path().join("shaders")).unwrap();
        std::fs::write(tmp.path().join("shaders/Anime4K_Restore_CNN_M.glsl"), "old").unwrap();

        let dir = materialize(tmp.path()).expect("cache is writable");
        assert!(dir.join(COMPLETE).is_file());
        assert!(!tmp.path().join("shaders/Anime4K_Restore_CNN_M.glsl").exists());
        for s in SHADERS.iter().chain(NOTICES) {
            assert_eq!(std::fs::read_to_string(dir.join(s.name)).unwrap(), s.body);
        }

        // Second call: same directory, nothing rewritten.
        let marker = dir.join(COMPLETE).metadata().unwrap().modified().unwrap();
        assert_eq!(materialize(tmp.path()), Some(dir.clone()));
        assert_eq!(dir.join(COMPLETE).metadata().unwrap().modified().unwrap(), marker);

        let chain = choose(Look::Natural, &SD, Some(UHD), FULL);
        let paths = paths(&chain, &dir);
        assert_eq!(paths.len(), chain.files.len());
        for p in &paths {
            assert!(Path::new(p).is_absolute() && Path::new(p).is_file(), "{p}");
        }
    }
}
