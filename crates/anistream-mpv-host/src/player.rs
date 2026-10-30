//! The hosted player: one mpv core driven by the engine's `PlayerOp`s, reporting back what the
//! engine reads, and telling the app's interface what it needs to draw its controls.
//!
//! - Ops run on a worker thread of their own, in order, so nothing on the app's main thread waits
//!   on mpv: a reconfigure or a shader compile can take a while.
//! - mpv's events are read on a thread of their own and become [`HostEvent`]s, handed to the app's
//!   sink on that thread. The app moves them to its main thread as its toolkit does.
//! - A `Load` belongs to its playback from mpv's `start-file` on, and what comes before it to the
//!   playback before, as on the phones.
//! - `Quit`, and the viewer closing the player, report `Ended { Quit }` and then `Closed`; `Close`
//!   says nothing. Either way the file stops, and every op after it for those playbacks is
//!   refused, with `Closed` said once, until the engine opens a player for a newer one.
//!
//! The core lives as long as the [`Player`], unlike the phones' one per opening: the window's
//! video area holds a render context on it, and a render context has to be freed on the GL
//! thread before its core goes.

use std::{
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};

use anistream_model::{
    hosted::{PlayerEnd, PlayerOp, PlayerReport},
    playback::PlaybackId,
};
use serde_json::Value;

pub use crate::book::PlayerState;
use crate::{
    book::Book,
    log,
    mpv::{Mpv, MpvError, MpvEvent},
    ops::{self, MpvCall, REPORTED_PROPERTIES, Step},
};

/// How the core starts: the decoding, mpv's directories, and anything else, set before
/// `mpv_initialize`.
#[derive(Debug, Clone)]
pub struct PlayerOptions {
    /// `hwdec`: `auto-safe` on a desktop, `no` where there is no GPU to decode on.
    pub hwdec: String,
    /// mpv's `config-dir`, where its `fonts.conf` and scripts would be; `None` reads no config.
    pub config_dir: Option<String>,
    /// Where compiled shaders are cached.
    pub cache_dir: Option<String>,
    /// `ao`: `null` in a test.
    pub audio_output: Option<String>,
    /// mpv's log level for the app's log: `warn` normally, `v` to see the renderer pick.
    pub log_level: String,
    /// Where the picture goes: the app's own framebuffer, or a swapchain for composition.
    pub video: VideoOutput,
    /// Anything else, set last, so it wins.
    pub extra: Vec<(String, String)>,
}

/// Where the player's picture goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoOutput {
    /// libmpv's render API (`vo=libmpv`): the app draws each frame into its own framebuffer, as
    /// a `GtkGLArea` does.
    #[default]
    RenderApi,
    /// mpv's own renderer on Direct3D 11, presenting into a swapchain made for composition, with
    /// no window: the app reads its address from [`Player::swapchain`] and hands it to a XAML
    /// `SwapChainPanel`. `width` and `height` are the panel's size in pixels, which mpv cannot
    /// measure; [`Player::composition_size`] says it again when the panel resizes.
    D3d11Composition { width: u32, height: u32 },
}

impl VideoOutput {
    /// The options that send the picture there.
    fn options(self) -> Vec<(&'static str, String)> {
        match self {
            Self::RenderApi => vec![("vo", "libmpv".into())],
            Self::D3d11Composition { width, height } => vec![
                ("vo", "gpu-next".into()),
                ("gpu-api", "d3d11".into()),
                ("gpu-context", "d3d11".into()),
                ("d3d11-output-mode", "composition".into()),
                ("d3d11-composition-size", composition_size(width, height)),
                // SDR until the app says the display is HDR: in composition mode mpv cannot
                // see the display (mpv #16795), and the hint alone would send PQ to any.
                ("target-colorspace-hint", "no".into()),
            ],
        }
    }
}

/// `d3d11-composition-size` as mpv reads it: `1280x720`, at least a pixel each way.
pub fn composition_size(width: u32, height: u32) -> String {
    format!("{}x{}", width.max(1), height.max(1))
}

impl Default for PlayerOptions {
    fn default() -> Self {
        Self {
            hwdec: "auto-safe".into(),
            config_dir: None,
            cache_dir: None,
            audio_output: None,
            log_level: "warn".into(),
            video: VideoOutput::default(),
            extra: Vec::new(),
        }
    }
}

impl PlayerOptions {
    /// Every option, in order: the ones every desktop host shares, after the phones' where
    /// they apply, then the decoding, the directories and the extras.
    pub fn resolved(&self) -> Vec<(String, String)> {
        // Where the picture goes, first: mpv makes no window either way.
        let mut options: Vec<(String, String)> =
            self.video.options().into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        options.extend(
            [
                ("idle", "yes"),
                ("keep-open", "no"),
                ("input-default-bindings", "no"),
                ("input-vo-keyboard", "no"),
                ("osc", "no"),
                ("osd-level", "0"),
                ("embeddedfonts", "yes"),
                ("ytdl", "no"),
                ("save-position-on-quit", "no"),
                ("audio-client-name", "anistream"),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned())),
        );
        options.push(("hwdec".into(), self.hwdec.clone()));
        match &self.config_dir {
            Some(dir) => {
                options.push(("config".into(), "yes".into()));
                options.push(("config-dir".into(), dir.clone()));
            }
            None => options.push(("config".into(), "no".into())),
        }
        if let Some(dir) = &self.cache_dir {
            options.push(("gpu-shader-cache-dir".into(), dir.clone()));
            options.push(("icc-cache-dir".into(), dir.clone()));
        }
        if let Some(ao) = &self.audio_output {
            options.push(("ao".into(), ao.clone()));
        }
        options.extend(self.extra.iter().cloned());
        options
    }
}

/// What the player tells the app, on its event thread.
#[derive(Debug, Clone, PartialEq)]
pub enum HostEvent {
    /// For the engine: `Command::PlayerReport { playback, report }`.
    Report { playback: PlaybackId, report: PlayerReport },
    /// Show the player.
    Opened { fullscreen: bool },
    /// Hide it: the playback is over, or the viewer closed it.
    Closed,
    /// Full screen on, off, or (`None`) the other way.
    Fullscreen(Option<bool>),
    /// The controls' state changed.
    State(PlayerState),
}

/// What the app hands events to. Called on the player's own threads.
pub type Sink = Arc<dyn Fn(HostEvent) + Send + Sync>;

enum Work {
    Call(MpvCall),
    /// A file for `playback`: mpv's answer names the playlist entry its `start-file` will.
    Load {
        playback: PlaybackId,
        call: MpvCall,
    },
    /// Stop the file, after everything before it.
    Stop,
}

struct Shared {
    mpv: Mpv,
    sink: Sink,
    book: Mutex<Book>,
    stopping: AtomicBool,
}

impl Shared {
    fn book(&self) -> MutexGuard<'_, Book> {
        self.book.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn say(&self, event: HostEvent) {
        (self.sink)(event);
    }

    /// Report to the playback mpv's file belongs to.
    fn report(&self, report: PlayerReport) {
        let target = {
            let mut book = self.book();
            let Some(target) = book.target() else { return };
            if !book.accepts(target) {
                return;
            }
            if report == PlayerReport::Closed {
                book.closed_said.insert(target);
            }
            target
        };
        self.say(HostEvent::Report { playback: target, report });
    }

    /// Tell the engine this player is closed for `playback`, once.
    fn say_closed(&self, playback: PlaybackId) {
        if self.book().closed_said.insert(playback) {
            self.say(HostEvent::Report { playback, report: PlayerReport::Closed });
        }
    }
}

/// The hosted player.
pub struct Player {
    shared: Arc<Shared>,
    work: Option<mpsc::Sender<Work>>,
    threads: Vec<JoinHandle<()>>,
}

impl Player {
    /// Start the core with `options`, observing what the engine reads, with `sink` told
    /// everything the player says.
    pub fn new(options: &PlayerOptions, sink: Sink) -> Result<Self, MpvError> {
        let resolved = options.resolved();
        let mpv = Mpv::new(
            resolved.iter().map(|(k, v)| (k.as_str(), v.as_str())),
            &options.log_level,
        )?;
        for (id, property) in REPORTED_PROPERTIES.iter().enumerate() {
            mpv.observe(property, id as u64 + 1)?;
        }
        let shared = Arc::new(Shared {
            mpv,
            sink,
            book: Mutex::new(Book::default()),
            stopping: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::channel::<Work>();
        let worker = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("mpv-ops".into())
                .spawn(move || {
                    for work in rx {
                        run(&shared, work);
                    }
                })
                .expect("a thread for mpv's ops")
        };
        let events = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("mpv-events".into())
                .spawn(move || read_events(&shared))
                .expect("a thread for mpv's events")
        };
        Ok(Self { shared, work: Some(tx), threads: vec![worker, events] })
    }

    /// The core, for a renderer to attach to.
    pub fn mpv(&self) -> &Mpv {
        &self.shared.mpv
    }

    /// The controls' state as it stands.
    pub fn state(&self) -> PlayerState {
        self.shared.book().state.clone()
    }

    /// Carry out one of the engine's ops for `playback`, after the ones before it.
    pub fn apply(&self, playback: PlaybackId, op: &PlayerOp) {
        let shared = &self.shared;
        {
            let book = shared.book();
            // An Open from a newer playback is what reopens a player the viewer closed, as a
            // stage would.
            let reopening = matches!(op, PlayerOp::Open { .. })
                && book.closed_through.is_none_or(|through| playback > through);
            if !book.accepts(playback) && !reopening {
                drop(book);
                tracing::debug!(
                    playback = playback.get(),
                    ?op,
                    "refused: that playback is over"
                );
                shared.say_closed(playback);
                return;
            }
        }
        match Step::of(op) {
            Step::Open { fullscreen, speed, volume } => {
                {
                    let mut book = shared.book();
                    book.opened = Some(playback);
                    book.closed_by_viewer = false;
                }
                if let Some(speed) = speed {
                    self.enqueue(Work::Call(MpvCall::Set {
                        property: "speed".into(),
                        value: speed.into(),
                    }));
                }
                if let Some(volume) = volume {
                    self.enqueue(Work::Call(MpvCall::Set {
                        property: "volume".into(),
                        value: volume.into(),
                    }));
                }
                shared.say(HostEvent::Opened { fullscreen });
            }
            Step::Call(call @ MpvCall::Load { .. }) => {
                shared.book().loading(playback);
                self.enqueue(Work::Load { playback, call });
            }
            Step::Call(call) => self.enqueue(Work::Call(call)),
            Step::Fullscreen(on) => shared.say(HostEvent::Fullscreen(on)),
            Step::Quit => self.close(true),
            Step::Close => self.close(false),
        }
    }

    /// The viewer closed the player: the file ended by quitting, and the player is closed.
    /// Nothing more is taken until a newer playback opens it.
    pub fn viewer_closed(&self) {
        self.close(true);
        self.shared.book().closed_by_viewer = true;
    }

    /// The display the video plays on, for the engine's upscaling. With its refresh rate
    /// known, mpv paces frames to it.
    pub fn display(
        &self,
        width: u32,
        height: u32,
        scale: f64,
        refresh_hz: Option<f64>,
        compute: bool,
    ) {
        if let Some(hz) = refresh_hz {
            self.enqueue(Work::Call(MpvCall::Set {
                property: "display-fps-override".into(),
                value: hz.into(),
            }));
        }
        self.shared.report(PlayerReport::Display {
            width,
            height,
            scale,
            refresh_hz,
            edr_headroom: None,
            compute,
        });
    }

    /// What the machine runs on.
    pub fn power(&self, on_battery: bool, low_power: bool) {
        self.shared.report(PlayerReport::Power { on_battery, low_power });
    }

    /// The composition panel's new size in pixels: mpv resizes the buffers of the swapchain it
    /// has, which keeps its address. Only for [`VideoOutput::D3d11Composition`].
    pub fn composition_size(&self, width: u32, height: u32) {
        self.enqueue(Work::Call(MpvCall::Set {
            property: "d3d11-composition-size".into(),
            value: composition_size(width, height).into(),
        }));
    }

    /// The swapchain mpv presents into in composition mode, as an `IDXGISwapChain` address:
    /// `None` until the first video reconfig has made one. Anything that rebuilds the video
    /// output, such as a change of a `d3d11-output-*` option, makes a new one, and mpv says
    /// nothing when it does, so the app asks again from time to time and binds what is new.
    pub fn swapchain(&self) -> Option<u64> {
        self.shared.mpv.get("display-swapchain")?.as_i64().filter(|&a| a != 0).map(|a| a as u64)
    }

    /// The display's HDR, for composition mode, where mpv cannot see the display: with
    /// `Some(peak)`, the display shows HDR and libplacebo may switch the same swapchain to
    /// HDR10, tone-mapped to `peak` nits; with `None`, it tone-maps to SDR itself, which looks
    /// far better than Windows clipping PQ on an SDR display. Changes in place while playing.
    pub fn hdr_output(&self, peak: Option<f64>) {
        for (property, value) in hdr_properties(peak) {
            self.enqueue(Work::Call(MpvCall::Set { property: property.into(), value }));
        }
    }

    fn close(&self, reporting: bool) {
        let shared = &self.shared;
        if reporting {
            shared.report(PlayerReport::Ended { reason: PlayerEnd::Quit });
            shared.report(PlayerReport::Closed);
        }
        shared.book().close();
        self.enqueue(Work::Stop);
        // mpv keeps `pause` from file to file: the next one would start on a still frame, with
        // the controls, told nothing changed, saying it plays.
        self.enqueue(Work::Call(MpvCall::Set {
            property: "pause".into(),
            value: false.into(),
        }));
        shared.say(HostEvent::Closed);
    }

    fn enqueue(&self, work: Work) {
        if let Some(tx) = &self.work {
            let _ = tx.send(work);
        }
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        self.shared.stopping.store(true, Ordering::SeqCst);
        self.work = None;
        self.shared.mpv.wakeup();
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// What [`Player::hdr_output`] sets: the colour-space hint, and the peak it tone-maps to.
fn hdr_properties(peak: Option<f64>) -> [(&'static str, Value); 2] {
    match peak {
        Some(nits) => [("target-colorspace-hint", true.into()), ("target-peak", nits.into())],
        None => [("target-colorspace-hint", false.into()), ("target-peak", "auto".into())],
    }
}

/// One op's call on mpv, on the worker thread.
fn run(shared: &Shared, work: Work) {
    let mpv = &shared.mpv;
    let result = match work {
        Work::Stop => mpv.command(&["stop"]),
        Work::Load { playback, call } => {
            let answer = mpv.command_node_answered(&call.command_node().expect("a command"));
            let entry =
                answer.as_ref().ok().and_then(|answer| answer["playlist_entry_id"].as_i64());
            shared.book().loaded_as(playback, entry);
            answer.map(drop)
        }
        Work::Call(call) => match &call {
            MpvCall::Command(args) => {
                mpv.command(&args.iter().map(String::as_str).collect::<Vec<_>>())
            }
            MpvCall::Set { property, value } => mpv.set(property, value),
            MpvCall::Shaders(paths) => mpv.set("glsl-shaders", &Value::from(paths.clone())),
            MpvCall::Restate(property) => {
                let value = mpv.get(property).unwrap_or(Value::Null);
                shared.report(PlayerReport::Property {
                    name: property.clone(),
                    json: value.to_string(),
                });
                Ok(())
            }
            // A property mpv can't read now, `avsync` with no audio, is left out: the row it
            // feeds is left out with it.
            MpvCall::Sample(properties) => {
                for property in properties {
                    if let Some(value) = mpv.get(property) {
                        shared.report(PlayerReport::Property {
                            name: property.clone(),
                            json: value.to_string(),
                        });
                    }
                }
                Ok(())
            }
            MpvCall::Load { .. } | MpvCall::AddSubtitle { .. } | MpvCall::Seek { .. } => {
                mpv.command_node(&call.command_node().expect("a command"))
            }
        },
    };
    if let Err(e) = result {
        tracing::warn!(error = %e, "mpv refused a call");
    }
}

/// mpv's events, until the player is dropped or the core shuts down.
fn read_events(shared: &Shared) {
    loop {
        if shared.stopping.load(Ordering::SeqCst) {
            return;
        }
        let Some(event) = shared.mpv.wait_event(-1.0) else { continue };
        match event {
            MpvEvent::Shutdown => {
                shared.report(PlayerReport::Closed);
                shared.say(HostEvent::Closed);
                return;
            }
            MpvEvent::Log { prefix, level, text } => {
                if let Some(hook) = log::disabled_hook(&text) {
                    shared.report(PlayerReport::HookDisabled { hook });
                }
                let line = log::redacting(text.trim_end());
                match level.as_str() {
                    "fatal" | "error" => tracing::error!(target: "mpv", "[{prefix}] {line}"),
                    "warn" => tracing::warn!(target: "mpv", "[{prefix}] {line}"),
                    "info" => tracing::info!(target: "mpv", "[{prefix}] {line}"),
                    _ => tracing::debug!(target: "mpv", "[{prefix}] {line}"),
                }
            }
            MpvEvent::StartFile { entry } => shared.book().started(entry),
            MpvEvent::FileLoaded => shared.report(PlayerReport::FileLoaded),
            MpvEvent::EndFile { reason } => {
                shared.report(PlayerReport::Ended { reason: ops::player_end(reason) });
            }
            MpvEvent::PlaybackRestart => {
                let changed = shared.book().restarted();
                if let Some(state) = changed {
                    shared.say(HostEvent::State(state));
                }
            }
            MpvEvent::Property { name, value } => {
                let state = update(shared, &name, &value);
                if REPORTED_PROPERTIES.contains(&name.as_str()) {
                    shared.report(PlayerReport::Property { name, json: value.to_string() });
                }
                if let Some(state) = state {
                    shared.say(HostEvent::State(state));
                }
            }
            MpvEvent::Idle | MpvEvent::VideoReconfig | MpvEvent::Seek | MpvEvent::Other(_) => {}
        }
    }
}

/// The controls' state after `name` changed to `value`, when that changes it.
fn update(shared: &Shared, name: &str, value: &Value) -> Option<PlayerState> {
    let mut book = shared.book();
    let state = &mut book.state;
    let before = state.clone();
    match name {
        "time-pos" => state.position = value.as_f64(),
        "duration" => state.duration = value.as_f64(),
        "pause" => state.paused = value.as_bool().unwrap_or(false),
        "paused-for-cache" => state.buffering = value.as_bool().unwrap_or(false),
        "seeking" => state.seeking = value.as_bool().unwrap_or(false),
        "frame-drop-count" => state.frame_drops = value.as_i64().unwrap_or(0),
        "track-list" => state.tracks = ops::tracks(value),
        _ => return None,
    }
    (*state != before).then(|| state.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn option<'a>(options: &'a [(String, String)], name: &str) -> Option<&'a str> {
        options.iter().rev().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    #[test]
    fn the_render_api_draws_into_the_apps_framebuffer() {
        let options = PlayerOptions::default().resolved();
        assert_eq!(options[0], ("vo".into(), "libmpv".into()));
        assert_eq!(option(&options, "d3d11-output-mode"), None);
    }

    #[test]
    fn composition_presents_into_a_swapchain_of_the_panels_size_in_sdr() {
        let options = PlayerOptions {
            video: VideoOutput::D3d11Composition { width: 1280, height: 0 },
            ..PlayerOptions::default()
        }
        .resolved();
        assert_eq!(option(&options, "vo"), Some("gpu-next"));
        assert_eq!(option(&options, "gpu-api"), Some("d3d11"));
        assert_eq!(option(&options, "d3d11-output-mode"), Some("composition"));
        assert_eq!(option(&options, "d3d11-composition-size"), Some("1280x1"));
        assert_eq!(option(&options, "target-colorspace-hint"), Some("no"));
        // The rest is every host's.
        assert_eq!(option(&options, "idle"), Some("yes"));
    }

    #[test]
    fn hdr_is_the_hint_with_the_displays_peak_and_sdr_is_neither() {
        assert_eq!(
            hdr_properties(Some(600.0)),
            [("target-colorspace-hint", true.into()), ("target-peak", 600.0.into())]
        );
        assert_eq!(
            hdr_properties(None),
            [("target-colorspace-hint", false.into()), ("target-peak", "auto".into())]
        );
    }
}
