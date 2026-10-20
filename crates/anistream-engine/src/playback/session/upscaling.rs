//! The picture: the upscaling chain chosen for the file as it stands and the window it plays
//! in, what the window keeps of it from one episode to the next, and the source's size and
//! kind as mpv reports them.

use anistream_core::stream::Stream;
use anistream_player::{MpvSession, companion};

use super::Run;
use crate::playback::{
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
        session: &MpvSession,
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
        if paths != picture.applied && session.set_shaders(&paths).await.is_ok() {
            tracing::info!(label = chain.label, ?budget, "upscaling chain");
            picture.applied = paths;
            if announce {
                let _ = session.notify(format!("Upscaling eased · {}", chain.label)).await;
            }
        }
        let media = companion::Media {
            hdr: source.hdr.map(HdrKind::badge),
            upscaler: chain.upscaler.map(str::to_owned),
        };
        let _ = session.send(companion::state("media", &media)).await;
        let _ = session.send(companion::state("menus", &self.menus(streams, index))).await;
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
    })
}
