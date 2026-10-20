//! Driving mpv over its JSON IPC socket.
//!
//! mpv is spawned with `--input-ipc-server`, then we connect and observe the properties that
//! matter: position, duration, pause, speed. Position updates are what drive resume, progress
//! and the Now Playing surface, so they flow back as [`PlaybackEvent`]s on a channel — the UI
//! never blocks on the socket.
//!
//! The socket does not exist the instant mpv starts, so connecting retries briefly. Treating
//! the first failure as fatal would make playback flaky for no reason.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_core::{config::PlayerUi, stream::Stream, traits::PlaybackRequest};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command as ProcessCommand},
    sync::{Mutex, mpsc},
};

use crate::{
    bundle::Bundle,
    companion::{self, ScriptEvent},
    ipc::{self, IpcStream},
    protocol::{
        Command, EndReason, Event, MpvVersion, base_args, file_options, observed, parse_line,
        session_args,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum PlayerError {
    #[error("mpv is not installed or not on PATH")]
    NotInstalled,

    #[error("could not start mpv: {0}")]
    Spawn(String),

    #[error("could not reach mpv's IPC socket: {0}")]
    Ipc(String),

    #[error("mpv exited before playback began")]
    ExitedEarly,
}

/// What the UI needs to know about playback.
///
/// A narrowed projection of the protocol events: the raw stream carries a lot that only
/// matters internally, and passing it all to the UI would invite decisions being made in the
/// wrong layer.
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackEvent {
    /// Position and, once known, duration.
    Progress {
        position: f64,
        duration: Option<f64>,
    },
    Paused(bool),
    Speed(f64),
    Volume(f64),
    /// The file's chapter markers arrived: `(title, start_seconds)`.
    Chapters(Vec<(String, f64)>),
    /// The file finished loading. Until this, property reports describe no file at all.
    Loaded,
    /// The viewer did something inside the player — a key, a button, a menu pick.
    Script(ScriptEvent),
    /// The decoded picture's parameters, raw.
    VideoParams(serde_json::Value),
    /// The video track's description, raw.
    VideoTrack(serde_json::Value),
    /// The file's tracks, raw.
    TrackList(serde_json::Value),
    Aid(Option<i64>),
    Sid(Option<i64>),
    SubDelay(f64),
    /// Playback is held waiting for data, or has resumed.
    Buffering(bool),
    /// Running total of frames the video output dropped.
    FrameDrops(u64),
    Seeking(bool),
    /// The display the window is on, once mpv knows it.
    Display {
        width: u32,
        height: u32,
    },
    /// The file was replaced by our own `loadfile`. The player is still there.
    Replaced,
    /// Playback of the file ended. `complete` is true only when mpv reached the end of it.
    ///
    /// The window outlives this: whether the player closes now is the playback loop's call.
    Ended {
        complete: bool,
    },
}

/// What mpv said on stderr.
///
/// Kept because mpv's own diagnosis is very often the whole answer — `Failed to open`, an unknown
/// `--vo`, a codec it cannot decode — and the alternative is guessing on the user's behalf.
#[derive(Clone, Default)]
pub struct Diagnostics {
    lines: Arc<Mutex<Vec<String>>>,
}

/// Stderr lines retained. mpv is not chatty at default verbosity; this is a bound, not a budget.
const DIAGNOSTIC_LINES: usize = 40;

impl Diagnostics {
    /// Read mpv's stderr in the background until the process exits.
    fn drain(&self, stderr: tokio::process::ChildStderr) {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let lines = Arc::clone(&self.lines);
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let line = line.trim().to_owned();
                if line.is_empty() {
                    continue;
                }
                tracing::warn!(mpv = %line, "mpv stderr");
                let mut lines = lines.lock().await;
                if lines.len() >= DIAGNOSTIC_LINES {
                    lines.remove(0);
                }
                lines.push(line);
            }
        });
    }

    /// The single most useful line to show a user, if there is one.
    ///
    /// mpv prefixes its real failures with a component tag (`[ffmpeg]`, `[stream]`) or the word
    /// "Failed"/"Error"; the rest is version banner and codec chatter. Preferring the last such
    /// line beats showing forty lines in a toast.
    pub async fn most_relevant(&self) -> Option<String> {
        let lines = self.lines.lock().await;
        lines
            .iter()
            .rev()
            .find(|line| {
                let lower = line.to_lowercase();
                lower.contains("failed")
                    || lower.contains("error")
                    || lower.contains("cannot")
                    || lower.contains("unrecognized")
                    || lower.contains("no such")
            })
            .or_else(|| lines.last())
            .cloned()
    }
}

/// A live mpv process.
pub struct MpvSession {
    child: Child,
    writer: Arc<Mutex<ipc::WriteHalf>>,
    request_id: AtomicU64,
    socket_path: PathBuf,
    diagnostics: Diagnostics,
    /// Cleared by the reader when the socket closes, which is how a parked session that the
    /// viewer closed while it waited is told apart from one that is still there to reuse.
    alive: Arc<AtomicBool>,
    /// Set by the reader when mpv ends a file because it is quitting: the viewer closed the
    /// window or pressed q, or something asked it to quit. See [`Self::has_quit`].
    quit: Arc<AtomicBool>,
    /// Whether this mpv takes `loadfile`'s index argument (0.38 and newer).
    loadfile_index: bool,
}

impl std::fmt::Debug for MpvSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpvSession")
            .field("socket", &self.socket_path)
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

/// An endpoint id no other session in this process, or any other, will produce.
///
/// The resume point used to supply the entropy, which meant there was none: it is `None` for
/// every unwatched episode, so playing one and then another derived the same path — and
/// starting the second unlinked the first's socket while it was still being used to control a
/// running mpv. A counter is the thing that is genuinely unique per session; the pid keeps it
/// distinct from another anistream sharing the directory.
fn next_endpoint_id() -> u64 {
    static SESSION: AtomicU64 = AtomicU64::new(0);
    (u64::from(std::process::id()) << 32)
        | (SESSION.fetch_add(1, Ordering::Relaxed) & 0xFFFF_FFFF)
}

/// Last resort: kill mpv and take its socket with it.
///
/// [`MpvSession::shutdown`] is the orderly path and asks mpv to quit first, but it is `async`
/// and only reached when playback ends the way it is supposed to. Every other way out — a
/// failure between spawning and connecting, an error partway through setting up property
/// observation, a task dropped mid-play — used to leave mpv running with nothing able to
/// address it and its socket file behind.
impl Drop for MpvSession {
    fn drop(&mut self) {
        // `start_kill` rather than `kill`: this is not an async context, and a signal that has
        // been delivered is enough — the process is reaped by the runtime.
        let _ = self.child.start_kill();
        ipc::remove_endpoint_blocking(&self.socket_path);
    }
}

impl MpvSession {
    /// What mpv reported on stderr.
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Whether mpv is still there to talk to.
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Whether mpv said it is quitting.
    ///
    /// `end-file` with reason `quit` arrives a moment *before* the socket closes, so a file's
    /// ending read against [`Self::is_alive`] alone still saw a live window, and took a window
    /// the viewer had just closed for a stream that failed: it fell over to the next stream in
    /// a new window. This is the window going away, said at the moment mpv says it.
    pub fn has_quit(&self) -> bool {
        self.quit.load(Ordering::Relaxed)
    }

    /// Send a command, without waiting for the reply.
    ///
    /// Fire-and-forget is right for playback control: a dropped seek is better than a stalled
    /// UI, and mpv reports the resulting state through the properties we already observe.
    pub async fn send(&self, command: Command) -> Result<(), PlayerError> {
        let id = self.request_id.fetch_add(1, Ordering::Relaxed);
        let line = command.to_line(id);
        let mut writer = self.writer.lock().await;
        writer.write_all(line.as_bytes()).await.map_err(|e| PlayerError::Ipc(e.to_string()))?;
        writer.flush().await.map_err(|e| PlayerError::Ipc(e.to_string()))
    }

    /// Open a stream in this window, in place of whatever was playing.
    ///
    /// Everything that belongs to the file — headers, resume point, title, track preference —
    /// travels with it, so the previous file's never leak into this one. External subtitles are
    /// attached separately once the file has loaded; see [`Self::add_subtitles`].
    pub async fn load(
        &self,
        stream: &Stream,
        request: &PlaybackRequest,
    ) -> Result<(), PlayerError> {
        // Before the load, so thumbfast sees the right address when it starts on this file.
        // Torrents give a second address that reads only what is already downloaded, so
        // thumbnails never pull bandwidth away from the playhead.
        let thumbnails = match &stream.thumbnail_url {
            Some(url) => companion::state("thumbnail-url", url),
            None => companion::clear("thumbnail-url"),
        };
        self.send(thumbnails).await?;
        let options = file_options(
            &stream.headers,
            request.start_at,
            &request.title,
            request.subtitle_language.as_deref(),
            request.dub,
        );
        tracing::info!(url = %stream.url, ?options, "loading into mpv");
        let index_slot = self.loadfile_index;
        self.send(Command::LoadFile { url: stream.url.clone(), options, index_slot }).await
    }

    /// Attach a stream's external subtitle tracks to the loaded file.
    pub async fn add_subtitles(
        &self,
        stream: &Stream,
        subtitle_language: Option<&str>,
    ) -> Result<(), PlayerError> {
        for subtitle in crate::protocol::subtitle_order(&stream.subtitles, subtitle_language) {
            let language = (!subtitle.language.is_empty()).then(|| subtitle.language.clone());
            self.send(Command::SubAdd {
                url: subtitle.url.clone(),
                title: language.clone(),
                language,
            })
            .await?;
        }
        Ok(())
    }

    pub async fn pause_toggle(&self) -> Result<(), PlayerError> {
        // `cycle` rather than reading then setting: no round trip, and no chance of acting
        // on a stale value — mpv's own keybindings can pause it behind our back.
        self.send(Command::Cycle("pause")).await
    }

    pub async fn seek(&self, seconds: f64) -> Result<(), PlayerError> {
        self.send(Command::Seek { seconds, absolute: false }).await
    }

    /// Toggle fullscreen. `cycle` for the same reason pause does: mpv's own `f` binding
    /// can change it behind our back, and cycling can never act on a stale read.
    pub async fn fullscreen_toggle(&self) -> Result<(), PlayerError> {
        self.send(Command::Cycle("fullscreen")).await
    }

    pub async fn seek_to(&self, seconds: f64) -> Result<(), PlayerError> {
        self.send(Command::Seek { seconds, absolute: true }).await
    }

    pub async fn set_speed(&self, speed: f64) -> Result<(), PlayerError> {
        self.send(Command::SetProperty("speed", speed.into())).await
    }

    /// Step the speed by a delta, letting mpv clamp it.
    pub async fn nudge_speed(&self, delta: f64) -> Result<(), PlayerError> {
        self.send(Command::Add("speed", delta)).await
    }

    /// Step the volume by a delta. mpv clamps to its own `volume-max`.
    pub async fn nudge_volume(&self, delta: f64) -> Result<(), PlayerError> {
        self.send(Command::Add("volume", delta)).await
    }

    /// Show a message on the video, in the companion's style.
    ///
    /// The viewer is looking at the video, not at the terminal.
    pub async fn notify(&self, text: impl AsRef<str>) -> Result<(), PlayerError> {
        self.send(companion::toast(text.as_ref(), companion::Level::Info)).await
    }

    /// Replace the upscaling chain, all at once.
    ///
    /// One write of the whole list, so a change mid-episode never renders a frame with half a
    /// chain; an empty list turns upscaling off.
    pub async fn set_shaders(&self, paths: &[String]) -> Result<(), PlayerError> {
        self.send(Command::SetProperty("glsl-shaders", serde_json::json!(paths))).await
    }

    /// Ask mpv to restate the chapter list as it stands right now.
    ///
    /// Registering one more observer is the mechanism: mpv greets every new observer
    /// with the current value, so the answer arrives as a normal chapters event. Asked
    /// after the file loads, because that is the moment the property stops describing
    /// nothing — the load-time value is what decides whether markers get written.
    pub async fn restate_chapters(&self) -> Result<(), PlayerError> {
        self.send(Command::ObserveProperty(observed::CHAPTERS_SETTLED, "chapter-list")).await
    }

    /// Replace the player's chapter list.
    ///
    /// mpv accepts a write to `chapter-list` on a loaded file, which is what puts the markers
    /// on the seek bar and under chapter navigation without an external file to write and clean
    /// up. Markers past the runtime are dropped by mpv rather than rejected, so an ending whose
    /// trailing boundary runs off the end of a slightly short encode still lands.
    pub async fn set_chapters(&self, chapters: &[(String, f64)]) -> Result<(), PlayerError> {
        let list: Vec<serde_json::Value> = chapters
            .iter()
            .map(|(title, time)| serde_json::json!({ "title": title, "time": time }))
            .collect();
        self.send(Command::SetProperty("chapter-list", list.into())).await
    }

    /// Stop the file but keep the window, so it can wait for the next one.
    pub async fn stop(&self) -> Result<(), PlayerError> {
        self.send(Command::Stop).await
    }

    /// Ask mpv to exit.
    pub async fn quit(&self) -> Result<(), PlayerError> {
        self.send(Command::Quit).await
    }

    /// Wait for mpv to exit.
    pub async fn wait(&mut self) -> Result<(), PlayerError> {
        self.child.wait().await.map(|_| ()).map_err(|e| PlayerError::Spawn(e.to_string()))
    }

    /// Kill mpv and remove the socket.
    pub async fn shutdown(mut self) {
        let _ = self.quit().await;
        // Give it a moment to exit cleanly before insisting.
        match tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await {
            Ok(_) => {}
            Err(_) => {
                let _ = self.child.kill().await;
            }
        }
        ipc::remove_endpoint(&self.socket_path).await;
    }
}

/// Spawns mpv and streams its events.
#[derive(Debug, Clone)]
pub struct Mpv {
    binary: String,
    socket_dir: PathBuf,
    extra_args: Vec<String>,
    ui: PlayerUi,
    /// Where the bundled scripts, fonts and uosc live once written out. `None` when they could
    /// not be written, which degrades to mpv's own interface rather than to no playback.
    bundle: Option<Bundle>,
    /// `anistream-<key>=<value>` options for the companion: the palette and font.
    theme: Vec<(String, String)>,
    /// The binary's version, read once on first use.
    version: Arc<tokio::sync::OnceCell<Option<MpvVersion>>>,
}

impl Mpv {
    pub fn new(socket_dir: impl Into<PathBuf>) -> Self {
        Self {
            binary: "mpv".into(),
            socket_dir: socket_dir.into(),
            extra_args: Vec::new(),
            ui: PlayerUi::Mpv,
            bundle: None,
            theme: Vec::new(),
            version: Arc::new(tokio::sync::OnceCell::new()),
        }
    }

    pub fn with_binary(mut self, binary: impl Into<String>) -> Self {
        self.binary = binary.into();
        self
    }

    /// Whose interface the player wears, and where the bundled one was written.
    pub fn with_ui(mut self, ui: PlayerUi, bundle: Option<Bundle>) -> Self {
        self.ui = ui;
        self.bundle = bundle;
        self
    }

    /// The companion's palette and font, as `(key, value)` pairs without the prefix.
    pub fn with_theme(mut self, theme: Vec<(String, String)>) -> Self {
        self.theme = theme;
        self
    }

    /// Where this player keeps its per-session scratch files.
    ///
    /// Exposed so a caller can put a file *next to* the IPC socket rather than inventing its
    /// own temp directory — subtitles fetched ahead of playback are the case that needs it.
    pub fn scratch_dir(&self) -> &std::path::Path {
        &self.socket_dir
    }

    /// Flags appended after everything anistream sets.
    ///
    /// Last wins in mpv, so these can override our own choices — which is what makes it a
    /// usable escape hatch rather than a suggestion box. It is also how the headless probe
    /// runs with no video output.
    pub fn with_extra_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    /// The extra arguments this handle will spawn mpv with.
    ///
    /// Exposed so a caller can prove what it assembled — a shader chain that should not
    /// be there is otherwise only visible as a pinned GPU.
    pub fn args_for_test(&self) -> &[String] {
        &self.extra_args
    }

    /// Whether mpv can be found.
    pub async fn is_available(&self) -> bool {
        ProcessCommand::new(&self.binary)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|s| s.success())
    }

    /// mpv's own version string, for diagnostics.
    ///
    /// Worth reporting rather than just "installed": a broken `mpv.conf` or a bad `vo`/`hwdec`
    /// choice makes mpv accept a file and then never render it, which from anistream's side is
    /// indistinguishable from a source with no data. Knowing which mpv is on the `PATH` is the
    /// first step in telling those apart, and `mpv --version` also prints the config it loaded.
    pub async fn version(&self) -> Option<String> {
        let output = ProcessCommand::new(&self.binary).arg("--version").output().await.ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().next().map(|line| line.trim().to_owned())
    }

    /// The parsed version of the configured binary, read once and kept. `None` when it could
    /// not be run or its banner not read.
    pub async fn mpv_version(&self) -> Option<MpvVersion> {
        *self
            .version
            .get_or_init(|| async { self.version().await.and_then(|b| MpvVersion::parse(&b)) })
            .await
    }

    /// The flags that dress the player: which interface, which scripts, which font.
    ///
    /// None at all for an mpv older than 0.36: it has neither the `user-data` property the
    /// companion is driven through nor `--osd-fonts-dir`, which it would refuse as an unknown
    /// option and not start. Plain mpv plays; it just does not wear anistream's interface.
    fn interface_args(&self, version: Option<MpvVersion>) -> Vec<String> {
        let Some(bundle) = &self.bundle else { return Vec::new() };
        if version.is_some_and(|v| !v.has_companion_support()) {
            return Vec::new();
        }
        let mut args = Vec::new();
        let opt = |key: &str, value: &str| format!("--script-opts-append={key}={value}");

        if self.ui == PlayerUi::Anistream {
            // The user's mpv.conf still applies; the scripts in their mpv folder do not, so a
            // second OSC or a track picker of their own cannot fight ours.
            args.push("--load-scripts=no".to_string());
            // uosc replaces both.
            args.push("--osc=no".to_string());
            args.push("--osd-bar=no".to_string());
            args.push(format!("--osd-fonts-dir={}", bundle.fonts().display()));
            args.push(format!("--script={}", bundle.uosc().display()));
            args.push(format!("--script={}", bundle.thumbfast().display()));
            for (key, value) in bundle.uosc_opts() {
                args.push(opt(&format!("uosc-{key}"), &value));
            }
            for (key, value) in bundle.thumbfast_opts() {
                args.push(opt(&format!("thumbfast-{key}"), &value));
            }
            // thumbfast starts its own hidden mpv, and only finds the running one by itself on
            // macOS; a configured binary path has to be handed to it.
            args.push(opt("thumbfast-mpv_path", &self.binary));
            args.push(opt("anistream-uosc", "yes"));
        }
        args.push(format!("--script={}", bundle.companion().display()));
        // So the companion can close an idle window whose host has gone.
        args.push(opt("anistream-host_pid", &std::process::id().to_string()));
        for (key, value) in &self.theme {
            args.push(opt(&format!("anistream-{key}"), value));
        }
        if self.ui == PlayerUi::Anistream
            && let Some((_, font)) = self.theme.iter().find(|(key, _)| key == "font")
        {
            args.push(format!("--osd-font={font}"));
        }
        args
    }

    /// Open the player window with nothing in it yet.
    ///
    /// The window comes up idle and files are loaded into it with [`MpvSession::load`], which
    /// is what lets the same window carry the next episode.
    pub async fn spawn(
        &self,
        request: &PlaybackRequest,
    ) -> Result<(MpvSession, mpsc::UnboundedReceiver<PlaybackEvent>), PlayerError> {
        // The endpoint only lives on disk on Unix (Windows uses the pipe namespace), but the
        // scratch files beside it need a real directory everywhere.
        tokio::fs::create_dir_all(&self.socket_dir)
            .await
            .map_err(|e| PlayerError::Spawn(e.to_string()))?;

        // A unique endpoint per session, so a lingering process from a previous run cannot be
        // mistaken for this one — and so two sessions in *this* run cannot be mistaken for
        // each other.
        let socket_path = ipc::mpv_endpoint(&self.socket_dir, next_endpoint_id());
        ipc::remove_endpoint(&socket_path).await;

        // An unreadable version is taken as current: that is what nearly everyone runs, and the
        // older paths only exist for distributions that pin an old mpv.
        let version = self.mpv_version().await;
        let mut args = base_args(&socket_path.to_string_lossy());
        // Before the user's extra args, so `--no-fullscreen` there still wins.
        args.extend(session_args(request.speed, request.volume, request.fullscreen));
        args.extend(self.interface_args(version));
        // Last: mpv takes the last value for a repeated flag, so these genuinely override.
        args.extend(self.extra_args.iter().cloned());

        tracing::info!(?args, "starting mpv");

        let child = ProcessCommand::new(&self.binary)
            .args(&args)
            // mpv's own output would corrupt the alternate screen the TUI is drawing on, so
            // stdout goes nowhere — but stderr is *captured* rather than discarded. Discarding it
            // was a real defect: when mpv could not open a URL it said so there and nowhere else,
            // so a failed playback reached the user as the eyecatch quietly wiping back to the
            // episode table with no message at all. The pipe is drained below.
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    PlayerError::NotInstalled
                } else {
                    PlayerError::Spawn(e.to_string())
                }
            })?;

        let mut child = child;
        let diagnostics = Diagnostics::default();
        if let Some(stderr) = child.stderr.take() {
            diagnostics.drain(stderr);
        }

        let socket = connect_with_retry(&socket_path).await?;
        let (read_half, write_half) = ipc::split(socket);
        let writer = Arc::new(Mutex::new(write_half));
        let alive = Arc::new(AtomicBool::new(true));
        let quit = Arc::new(AtomicBool::new(false));

        let (tx, rx) = mpsc::unbounded_channel();
        spawn_reader(read_half, tx, Arc::clone(&alive), Arc::clone(&quit));

        let session = MpvSession {
            child,
            writer: Arc::clone(&writer),
            request_id: AtomicU64::new(1),
            socket_path,
            diagnostics,
            alive,
            quit,
            loadfile_index: version.is_none_or(MpvVersion::has_loadfile_index),
        };

        // Observe the properties the UI needs. Without these, no position updates arrive and
        // resume would never learn anything.
        for (id, name) in [
            (observed::TIME_POS, "time-pos"),
            (observed::DURATION, "duration"),
            (observed::PAUSE, "pause"),
            (observed::SPEED, "speed"),
            (observed::VOLUME, "volume"),
            (observed::CHAPTERS, "chapter-list"),
            (observed::VIDEO_PARAMS, "video-params"),
            (observed::VIDEO_TRACK, "current-tracks/video"),
            (observed::TRACK_LIST, "track-list"),
            (observed::AID, "aid"),
            (observed::SID, "sid"),
            (observed::SUB_DELAY, "sub-delay"),
            (observed::PAUSED_FOR_CACHE, "paused-for-cache"),
            (observed::FRAME_DROPS, "frame-drop-count"),
            (observed::SEEKING, "seeking"),
            (observed::DISPLAY_WIDTH, "display-width"),
            (observed::DISPLAY_HEIGHT, "display-height"),
        ] {
            session.send(Command::ObserveProperty(id, name)).await?;
        }
        session.send(companion::state("protocol", &companion::PROTOCOL)).await?;
        session.send(companion::hello()).await?;

        Ok((session, rx))
    }

    /// Open a window and start playing one stream in it.
    pub async fn play(
        &self,
        stream: &Stream,
        request: &PlaybackRequest,
    ) -> Result<(MpvSession, mpsc::UnboundedReceiver<PlaybackEvent>), PlayerError> {
        let (session, rx) = self.spawn(request).await?;
        session.load(stream, request).await?;
        Ok((session, rx))
    }
}

/// Connect to mpv's socket, retrying while it starts up.
///
/// The socket appears a moment after the process does. Treating the first failure as fatal
/// would make playback flaky for no reason.
async fn connect_with_retry(path: &Path) -> Result<IpcStream, PlayerError> {
    const ATTEMPTS: u32 = 50;
    let mut last = String::new();

    for _ in 0..ATTEMPTS {
        match ipc::connect(path).await {
            Ok(stream) => return Ok(stream),
            Err(e) => {
                last = e.to_string();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    Err(PlayerError::Ipc(format!("socket never appeared: {last}")))
}

/// Read events off the socket and forward the ones the UI cares about.
///
/// Runs for the life of the process rather than of one file: the end of a file is reported
/// and the reader keeps going, because the same window may be about to play the next one.
fn spawn_reader(
    read_half: ipc::ReadHalf,
    tx: mpsc::UnboundedSender<PlaybackEvent>,
    alive: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(read_half).lines();
        // Duration arrives separately from position, so it is remembered and attached.
        let mut duration: Option<f64> = None;
        // Display size arrives as two properties; reported once both are known.
        let mut display: (Option<u32>, Option<u32>) = (None, None);

        loop {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                // Socket closed: mpv exited. Anything not already reported as an ending was
                // an abandonment, not a completion.
                Ok(None) | Err(_) => {
                    alive.store(false, Ordering::Relaxed);
                    let _ = tx.send(PlaybackEvent::Ended { complete: false });
                    break;
                }
            };

            let Some(event) = parse_line(&line) else {
                continue;
            };

            let forwarded = match event {
                Event::TimePos(position) => {
                    Some(PlaybackEvent::Progress { position, duration })
                }
                Event::Duration(seconds) => {
                    duration = Some(seconds);
                    None
                }
                Event::Paused(paused) => Some(PlaybackEvent::Paused(paused)),
                Event::Speed(speed) => Some(PlaybackEvent::Speed(speed)),
                Event::Volume(volume) => Some(PlaybackEvent::Volume(volume)),
                Event::Chapters(chapters) => Some(PlaybackEvent::Chapters(chapters)),
                Event::ClientMessage(args) => {
                    companion::parse_event(&args).map(PlaybackEvent::Script)
                }
                Event::EndFile(EndReason::Stop) => Some(PlaybackEvent::Replaced),
                Event::EndFile(reason) => {
                    // The next file's duration is its own.
                    duration = None;
                    // Before the ending is sent, so whoever reads it already knows the window
                    // is going with it.
                    if reason == EndReason::Quit {
                        quit.store(true, Ordering::Relaxed);
                    }
                    Some(PlaybackEvent::Ended { complete: reason.is_complete() })
                }
                Event::FileLoaded => Some(PlaybackEvent::Loaded),
                Event::VideoParams(value) => Some(PlaybackEvent::VideoParams(value)),
                Event::VideoTrack(value) => Some(PlaybackEvent::VideoTrack(value)),
                Event::TrackList(value) => Some(PlaybackEvent::TrackList(value)),
                Event::Aid(id) => Some(PlaybackEvent::Aid(id)),
                Event::Sid(id) => Some(PlaybackEvent::Sid(id)),
                Event::SubDelay(delay) => Some(PlaybackEvent::SubDelay(delay)),
                Event::Buffering(held) => Some(PlaybackEvent::Buffering(held)),
                Event::FrameDrops(count) => Some(PlaybackEvent::FrameDrops(count)),
                Event::Seeking(seeking) => Some(PlaybackEvent::Seeking(seeking)),
                Event::DisplayWidth(width) => {
                    display.0 = Some(width);
                    display_event(display)
                }
                Event::DisplayHeight(height) => {
                    display.1 = Some(height);
                    display_event(display)
                }
                Event::Seek | Event::Reply { .. } | Event::Ignored => None,
            };

            if let Some(event) = forwarded
                && tx.send(event).is_err()
            {
                // Nobody is listening any more.
                break;
            }
        }
    });
}

fn display_event((width, height): (Option<u32>, Option<u32>)) -> Option<PlaybackEvent> {
    Some(PlaybackEvent::Display { width: width?, height: height? })
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::stream::StreamKind;

    fn stream() -> Stream {
        Stream::new("http://127.0.0.1:1/s/x", StreamKind::TorrentHttp)
    }

    #[test]
    fn two_sessions_never_share_an_endpoint() {
        // The id used to be derived from the resume point, which is `None` for every unwatched
        // episode — so watching one and then the next produced the same socket path, and
        // starting the second unlinked the first's while it was still controlling a running
        // mpv. The old test only checked that two *different* numbers gave different paths,
        // which never touched the part that was wrong.
        let ids: Vec<u64> = (0..64).map(|_| next_endpoint_id()).collect();
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "two sessions derived the same endpoint");
    }

    #[tokio::test]
    async fn a_detected_mpv_also_reports_its_version() {
        // What is under test is the detection pair, not whether this machine has mpv — CI
        // runners do not, and asserting otherwise makes the suite fail on the environment
        // rather than on the code. The negative path is covered hermetically below.
        let mpv = Mpv::new(std::env::temp_dir());
        if !mpv.is_available().await {
            // CI's mpv job installs mpv and sets this, so a missing mpv fails there
            // instead of passing as a skip.
            assert!(
                std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
            );
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
        // `--doctor` reports both together, so a detected mpv that cannot say which build it is
        // would leave the diagnostic half-blank.
        assert!(mpv.version().await.is_some(), "a detected mpv should report a version");
    }

    #[test]
    fn an_mpv_too_old_for_the_interface_starts_without_it() {
        // 0.35 refuses `--osd-fonts-dir` as an unknown option and would not start at all; 0.37
        // has everything the interface uses. An unreadable banner is taken as current.
        let cache = tempfile::tempdir().unwrap();
        let bundle = Bundle::materialize(cache.path()).unwrap();
        let mpv = Mpv::new(std::env::temp_dir()).with_ui(PlayerUi::Anistream, Some(bundle));
        let fonts = |args: Vec<String>| args.iter().any(|a| a.starts_with("--osd-fonts-dir="));

        assert!(mpv.interface_args(MpvVersion::parse("mpv 0.35.1 Copyright")).is_empty());
        assert!(fonts(mpv.interface_args(MpvVersion::parse("mpv 0.37.0 Copyright"))));
        assert!(fonts(mpv.interface_args(None)));
    }

    #[tokio::test]
    async fn a_missing_binary_reports_not_installed_rather_than_a_raw_os_error() {
        let mpv = Mpv::new(std::env::temp_dir()).with_binary("definitely-not-a-real-player");
        assert!(!mpv.is_available().await);

        let error = mpv.play(&stream(), &PlaybackRequest::default()).await.unwrap_err();
        assert!(
            matches!(error, PlayerError::NotInstalled),
            "expected NotInstalled, got {error:?}"
        );
    }

    // Paused time: the retry loop's sleeps resolve instantly instead of costing five real
    // seconds, and the bound below is measured in the same virtual clock they advance.
    #[tokio::test(start_paused = true)]
    async fn connecting_to_a_socket_that_never_appears_fails_with_a_reason() {
        let path = std::env::temp_dir().join("anistream-nonexistent.sock");
        let _ = std::fs::remove_file(&path);

        // Shortened by pointing at a path nothing will ever create; the retry loop bounds it.
        let started = tokio::time::Instant::now();
        let error = connect_with_retry(&path).await.unwrap_err();
        assert!(matches!(error, PlayerError::Ipc(_)));
        assert!(
            started.elapsed() < Duration::from_secs(15),
            "retry loop must be bounded, took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn playback_events_distinguish_completion_from_abandonment() {
        // The distinction that decides whether progress is pushed to a tracker.
        assert_eq!(
            PlaybackEvent::Ended { complete: true },
            PlaybackEvent::Ended { complete: true }
        );
        assert_ne!(
            PlaybackEvent::Ended { complete: true },
            PlaybackEvent::Ended { complete: false }
        );
    }
}
