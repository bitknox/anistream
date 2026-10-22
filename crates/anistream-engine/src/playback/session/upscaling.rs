//! The picture: the upscaling chain chosen for the file as it stands and the window it plays
//! in, what the window keeps of it from one episode to the next, and the source's size and
//! kind as mpv reports them.

use anistream_core::stream::Stream;
use anistream_playback::companion;

use super::Run;
use crate::playback::{
    link::{Overlay, PlayerLink},
    picture::{self, Governor, HdrKind},
    shaders,
};

impl Run<'_> {
    /// Choose the upscaling chain for the file as it now stands, and set it if it changed.
    ///
    /// `announce` is for changes the viewer did not ask for — the governor easing, the battery
    /// — which get a short note on the video; a pick from the menu speaks for itself.
    pub(super) async fn apply_picture(
        &self,
        picture: &mut PictureState,
        link: &dyn PlayerLink,
        source: Option<&shaders::Source>,
        streams: &[Stream],
        index: usize,
        announce: bool,
    ) {
        let (Some(source), Some(dir)) = (source, &self.settings.shader_dir) else { return };
        let budget =
            shaders::Budget { on_battery: picture.on_battery, eased: picture.governor.eased() };
        let chain = shaders::choose(self.look, source, picture.display, budget);
        let paths = shaders::paths(&chain, dir);
        if paths != picture.applied && link.shaders(&paths).await.is_ok() {
            tracing::info!(label = chain.label, ?budget, "upscaling chain");
            picture.applied = paths;
            if announce {
                let text = format!("Upscaling eased · {}", chain.label);
                let _ =
                    link.overlay(Overlay::Toast { text, level: companion::Level::Info }).await;
            }
        }
        let media = companion::Media {
            hdr: source.hdr.map(HdrKind::badge),
            upscaler: chain.upscaler.map(str::to_owned),
        };
        let _ = link.overlay(Overlay::Media(media)).await;
        let _ = link.overlay(Overlay::Menus(self.menus(streams, index))).await;
    }
}

/// What the window is doing to the picture.
///
/// Kept with the player rather than the episode, like the track policy: a GPU that could not
/// keep up on episode 3 will not on episode 4 either, so the governor's steps carry over, and
/// mpv keeps the chain from one file to the next, so what is applied is the window's.
pub struct PictureState {
    pub(super) governor: Governor,
    /// The chain mpv is running, as absolute paths; compared before every write.
    pub(super) applied: Vec<String>,
    pub(super) display: Option<shaders::Display>,
    /// The chain that was eased for a hook the renderer disabled: one step for it, however
    /// many of its hooks are said to have failed.
    pub(super) failed: Option<Vec<String>>,
    pub(super) on_battery: bool,
    pub(super) battery_checked: Option<tokio::time::Instant>,
    /// The governor's clock: monotonic, so a pause or a seek cannot move it.
    pub(super) clock: std::time::Instant,
}

impl Default for PictureState {
    fn default() -> Self {
        Self {
            governor: Governor::new(Governor::DEFAULT_CAP),
            applied: Vec::new(),
            display: None,
            failed: None,
            on_battery: false,
            battery_checked: None,
            clock: std::time::Instant::now(),
        }
    }
}

/// The picture's size and kind, from `video-params` — `dw`/`dh`, the size after aspect
/// correction, which is what is actually shown.
pub(super) fn read_source(
    params: &serde_json::Value,
    track: Option<&serde_json::Value>,
) -> Option<shaders::Source> {
    let dimension = |key: &str| params.get(key)?.as_u64().and_then(|v| u32::try_from(v).ok());
    Some(shaders::Source {
        width: dimension("dw")?,
        height: dimension("dh")?,
        hdr: picture::classify(params, track),
        // Android's zero-copy decoding hands the renderer one RGB texture; anything else, a
        // software frame or a copy back from the decoder, comes as planes.
        planar: params.get("pixelformat").and_then(serde_json::Value::as_str)
            != Some("mediacodec"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_a_zero_copy_frame_comes_without_planes() {
        let params = |format: Option<&str>| {
            let mut params = json!({ "w": 1280, "h": 720, "dw": 1280, "dh": 720 });
            if let Some(format) = format {
                params["pixelformat"] = json!(format);
            }
            params
        };
        let planar = |format| read_source(&params(format), None).map(|source| source.planar);
        assert_eq!(planar(Some("mediacodec")), Some(false));
        for format in [Some("yuv420p"), Some("nv12"), Some("yuv420p10"), None] {
            assert_eq!(planar(format), Some(true), "{format:?}");
        }
    }
}
