//! The hosted player for an app that hosts libmpv through `anistream-mpv-host` rather than its
//! own binding: the Windows app, whose video is a XAML `SwapChainPanel` fed by mpv's d3d11
//! composition mode. Behind the `mpv-host` feature, which links libmpv.
//!
//! The player takes the engine's `PlayerOp`s as `Event::PlayerOps` brings them, and sends its
//! `PlayerReport`s straight to the engine, from mpv's own threads, so they keep mpv's order and
//! never wait on the app's UI thread. What the app's controls need comes to its
//! [`FfiPlayerSink`], on those threads too.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use anistream_model::{
    command::Command,
    hosted::{MpvOption, PlayerOp, TrackKind},
    playback::PlaybackId,
};
use anistream_mpv_host::{HostEvent, Player, PlayerOptions, PlayerState, VideoOutput};

use crate::{FfiEngine, start::FfiError};

/// Where the picture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiVideo {
    /// A swapchain for composition, `width` by `height` pixels: [`FfiPlayer::swapchain`] is its
    /// address, for `ISwapChainPanelNative::SetSwapChain`.
    Composition { width: u32, height: u32 },
    /// Nowhere: `vo=null`, for a test.
    Null,
}

/// How the player's core starts.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiPlayerOptions {
    pub video: FfiVideo,
    /// `hwdec`: `auto-safe` on a machine with a GPU to decode on, `no` without.
    #[uniffi(default = "auto-safe")]
    pub hwdec: String,
    /// Where compiled shaders are cached.
    #[uniffi(default = None)]
    pub cache_dir: Option<String>,
    /// `ao`: `null` in a test, the system's otherwise.
    #[uniffi(default = None)]
    pub audio_output: Option<String>,
    /// mpv's lines from this level up go to the app's log.
    #[uniffi(default = "warn")]
    pub log_level: String,
    /// Anything else, set last, so it wins.
    #[uniffi(default = [])]
    pub extra: Vec<MpvOption>,
}

/// What the controls draw from: the playhead and the state around it.
#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct FfiPlayerState {
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub paused: bool,
    /// Held waiting for data.
    pub buffering: bool,
    pub seeking: bool,
    /// The file on screen has shown its first frame.
    pub has_picture: bool,
    pub frame_drops: i64,
    /// The file's audio and subtitle tracks, for the Tracks menu; a pick goes to the engine as
    /// `PlayerCommand::ChooseTrack`, which keeps it for the series.
    pub tracks: Vec<FfiTrack>,
}

/// One of the file's audio or subtitle tracks.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiTrack {
    pub kind: TrackKind,
    pub id: i64,
    /// The language code the file gives; `track_name` turns it and the title into words.
    pub language: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
    /// Added from a file beside the stream.
    pub external: bool,
}

impl From<PlayerState> for FfiPlayerState {
    fn from(s: PlayerState) -> Self {
        Self {
            position: s.position,
            duration: s.duration,
            paused: s.paused,
            buffering: s.buffering,
            seeking: s.seeking,
            has_picture: s.has_picture,
            frame_drops: s.frame_drops,
            tracks: s
                .tracks
                .into_iter()
                .map(|t| FfiTrack {
                    kind: t.kind,
                    id: t.id,
                    language: t.language,
                    title: t.title,
                    selected: t.selected,
                    external: t.external,
                })
                .collect(),
        }
    }
}

/// What the player tells the app's interface.
#[derive(Debug, Clone, PartialEq, uniffi::Enum)]
pub enum FfiPlayerEvent {
    /// Show the player.
    Opened { fullscreen: bool },
    /// Hide it: the playback is over, or the viewer closed it.
    Closed,
    /// Full screen on, off, or (`None`) the other way.
    Fullscreen { on: Option<bool> },
    /// The controls' state changed.
    State { state: FfiPlayerState },
}

/// Where the player's events go: implemented by the app. Called on mpv's threads, never the
/// app's UI thread, so an implementation moves what it is told to its own.
#[uniffi::export(with_foreign)]
pub trait FfiPlayerSink: Send + Sync {
    fn event(&self, event: FfiPlayerEvent);
}

/// One mpv core driven by the engine's ops, for as long as the app holds it.
#[derive(uniffi::Object)]
pub struct FfiPlayer {
    player: Mutex<Option<Player>>,
}

#[uniffi::export]
impl FfiPlayer {
    /// Start a core with `options`, reporting to `engine` and telling `sink` what the controls
    /// need. The engine is held weakly: the player says nothing more once it has stopped.
    #[uniffi::constructor]
    pub fn new(
        options: FfiPlayerOptions,
        engine: Arc<FfiEngine>,
        sink: Arc<dyn FfiPlayerSink>,
    ) -> Result<Arc<Self>, FfiError> {
        let engine: Weak<FfiEngine> = Arc::downgrade(&engine);
        let host_sink = Arc::new(move |event: HostEvent| match event {
            HostEvent::Report { playback, report } => {
                if let Some(engine) = engine.upgrade() {
                    engine.send(Command::PlayerReport { playback, report });
                }
            }
            HostEvent::Opened { fullscreen } => {
                sink.event(FfiPlayerEvent::Opened { fullscreen })
            }
            HostEvent::Closed => sink.event(FfiPlayerEvent::Closed),
            HostEvent::Fullscreen(on) => sink.event(FfiPlayerEvent::Fullscreen { on }),
            HostEvent::State(state) => {
                sink.event(FfiPlayerEvent::State { state: state.into() })
            }
        });
        let player = Player::new(&host_options(options), host_sink)
            .map_err(|e| FfiError::Player { reason: e.to_string() })?;
        Ok(Arc::new(Self { player: Mutex::new(Some(player)) }))
    }

    /// Carry out the engine's ops for `playback`, in order, after the ones before them. Returns
    /// at once: mpv runs them on a thread of its own.
    pub fn apply(&self, playback: PlaybackId, ops: Vec<PlayerOp>) {
        if let Some(player) = self.player().as_ref() {
            for op in &ops {
                player.apply(playback, op);
            }
        }
    }

    /// The viewer closed the player: Esc, the back button, the window closing.
    pub fn viewer_closed(&self) {
        if let Some(player) = self.player().as_ref() {
            player.viewer_closed();
        }
    }

    /// The display the video plays on, in pixels, for the engine's upscaling; with its refresh
    /// rate, which composition mode cannot read, mpv paces frames to it.
    pub fn display(&self, width: u32, height: u32, scale: f64, refresh_hz: Option<f64>) {
        if let Some(player) = self.player().as_ref() {
            // libplacebo has compute shaders on Direct3D 11.
            player.display(width, height, scale, refresh_hz, true);
        }
    }

    /// What the machine runs on.
    pub fn power(&self, on_battery: bool, low_power: bool) {
        if let Some(player) = self.player().as_ref() {
            player.power(on_battery, low_power);
        }
    }

    /// The panel's new size in pixels.
    pub fn composition_size(&self, width: u32, height: u32) {
        if let Some(player) = self.player().as_ref() {
            player.composition_size(width, height);
        }
    }

    /// The composition swapchain's address, or 0 while there is none.
    pub fn swapchain(&self) -> u64 {
        self.player().as_ref().and_then(Player::swapchain).unwrap_or(0)
    }

    /// The display's HDR: its peak in nits while it shows HDR, `None` for SDR.
    pub fn hdr_output(&self, peak: Option<f64>) {
        if let Some(player) = self.player().as_ref() {
            player.hdr_output(peak);
        }
    }

    /// The controls' state as it stands.
    pub fn state(&self) -> FfiPlayerState {
        self.player().as_ref().map(|p| p.state().into()).unwrap_or_default()
    }

    /// Stop the core and its threads, waiting for them. Calls after the first do nothing, and
    /// every other call then does nothing either.
    pub fn shutdown(&self) {
        let player = self.player().take();
        drop(player);
    }
}

impl FfiPlayer {
    fn player(&self) -> MutexGuard<'_, Option<Player>> {
        self.player.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The host's options for the app's.
fn host_options(options: FfiPlayerOptions) -> PlayerOptions {
    let FfiPlayerOptions { video, hwdec, cache_dir, audio_output, log_level, extra } = options;
    let mut extra: Vec<(String, String)> =
        extra.into_iter().map(|o| (o.name, o.value)).collect();
    let video = match video {
        FfiVideo::Composition { width, height } => {
            VideoOutput::D3d11Composition { width, height }
        }
        FfiVideo::Null => {
            extra.insert(0, ("vo".into(), "null".into()));
            VideoOutput::default()
        }
    };
    PlayerOptions { hwdec, config_dir: None, cache_dir, audio_output, log_level, video, extra }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_composition_player_starts_on_the_panels_size_and_a_null_one_draws_nothing() {
        let options = |video| FfiPlayerOptions {
            video,
            hwdec: "no".into(),
            cache_dir: None,
            audio_output: Some("null".into()),
            log_level: "warn".into(),
            extra: vec![MpvOption { name: "volume".into(), value: "50".into() }],
        };
        let composed = host_options(options(FfiVideo::Composition { width: 800, height: 450 }));
        assert_eq!(composed.video, VideoOutput::D3d11Composition { width: 800, height: 450 });
        assert_eq!(composed.extra, [("volume".to_owned(), "50".to_owned())]);
        let null = host_options(options(FfiVideo::Null)).resolved();
        let vo = null.iter().rev().find(|(k, _)| k == "vo").map(|(_, v)| v.as_str());
        assert_eq!(vo, Some("null"));
    }
}
