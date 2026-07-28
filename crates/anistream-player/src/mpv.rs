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
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_core::{stream::Stream, traits::PlaybackRequest};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command as ProcessCommand},
    sync::{Mutex, mpsc},
};

use crate::{
    ipc::{self, IpcStream},
    protocol::{Command, EndReason, Event, base_args, observed, parse_line, stream_args},
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
    /// Playback ended. `complete` is true only when mpv reached the end of the file.
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
}

impl std::fmt::Debug for MpvSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MpvSession")
            .field("socket", &self.socket_path)
            .field("pid", &self.child.id())
            .finish_non_exhaustive()
    }
}

impl MpvSession {
    /// What mpv reported on stderr.
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
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

    /// Show a message on mpv's own OSD.
    ///
    /// Used for the skip prompt: the viewer is looking at the video, not at the terminal.
    pub async fn notify(&self, text: impl Into<String>) -> Result<(), PlayerError> {
        self.send(Command::ShowText { text: text.into(), duration_ms: 3000 }).await
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
}

impl Mpv {
    pub fn new(socket_dir: impl Into<PathBuf>) -> Self {
        Self { binary: "mpv".into(), socket_dir: socket_dir.into(), extra_args: Vec::new() }
    }

    pub fn with_binary(mut self, binary: impl Into<String>) -> Self {
        self.binary = binary.into();
        self
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

    /// Start playing, returning the session and a stream of events.
    pub async fn play(
        &self,
        stream: &Stream,
        request: &PlaybackRequest,
    ) -> Result<(MpvSession, mpsc::UnboundedReceiver<PlaybackEvent>), PlayerError> {
        // Only Unix puts the endpoint on disk; on Windows it lives in the flat pipe namespace and
        // there is no directory to make.
        if cfg!(unix) {
            tokio::fs::create_dir_all(&self.socket_dir)
                .await
                .map_err(|e| PlayerError::Spawn(e.to_string()))?;
        }

        // A unique endpoint per session, so a lingering process from a previous run cannot be
        // mistaken for this one.
        let unique =
            std::process::id() as u64 ^ (request.start_at.unwrap_or_default() * 1000.0) as u64;
        let socket_path = ipc::mpv_endpoint(&self.socket_dir, unique);
        ipc::remove_endpoint(&socket_path).await;

        let mut args = base_args(&socket_path.to_string_lossy(), &request.title);
        args.extend(stream_args(
            &stream.headers,
            request.start_at,
            request.speed,
            request.volume,
            request.subtitle_language.as_deref(),
        ));
        // After ours, before the URL: mpv takes the last value for a repeated flag, so these
        // genuinely override.
        args.extend(self.extra_args.iter().cloned());
        args.push(stream.url.clone());

        tracing::info!(url = %stream.url, ?args, "starting mpv");

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

        let (tx, rx) = mpsc::unbounded_channel();
        spawn_reader(read_half, tx);

        let session = MpvSession {
            child,
            writer: Arc::clone(&writer),
            request_id: AtomicU64::new(1),
            socket_path,
            diagnostics,
        };

        // Observe the properties the UI needs. Without these, no position updates arrive and
        // resume would never learn anything.
        for (id, name) in [
            (observed::TIME_POS, "time-pos"),
            (observed::DURATION, "duration"),
            (observed::PAUSE, "pause"),
            (observed::SPEED, "speed"),
            (observed::VOLUME, "volume"),
        ] {
            session.send(Command::ObserveProperty(id, name)).await?;
        }

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
fn spawn_reader(
    read_half: ipc::ReadHalf,
    tx: mpsc::UnboundedSender<PlaybackEvent>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(read_half).lines();
        // Duration arrives separately from position, so it is remembered and attached.
        let mut duration: Option<f64> = None;

        loop {
            let line = match lines.next_line().await {
                Ok(Some(line)) => line,
                // Socket closed: mpv exited. Anything not already reported as an ending was
                // an abandonment, not a completion.
                Ok(None) | Err(_) => {
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
                Event::EndFile(reason) => {
                    let complete = reason.is_complete();
                    let _ = tx.send(PlaybackEvent::Ended { complete });
                    if matches!(reason, EndReason::Eof | EndReason::Quit | EndReason::Error) {
                        break;
                    }
                    None
                }
                Event::FileLoaded | Event::Seek | Event::Reply { .. } | Event::Ignored => None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::stream::StreamKind;

    fn stream() -> Stream {
        Stream::new("http://127.0.0.1:1/s/x", StreamKind::TorrentHttp)
    }

    #[tokio::test]
    async fn a_detected_mpv_also_reports_its_version() {
        // What is under test is the detection pair, not whether this machine has mpv — CI
        // runners do not, and asserting otherwise makes the suite fail on the environment
        // rather than on the code. The negative path is covered hermetically below.
        let mpv = Mpv::new(std::env::temp_dir());
        if !mpv.is_available().await {
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
        // `--doctor` reports both together, so a detected mpv that cannot say which build it is
        // would leave the diagnostic half-blank.
        assert!(mpv.version().await.is_some(), "a detected mpv should report a version");
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

    #[tokio::test]
    async fn connecting_to_a_socket_that_never_appears_fails_with_a_reason() {
        let path = std::env::temp_dir().join("anistream-nonexistent.sock");
        let _ = std::fs::remove_file(&path);

        // Shortened by pointing at a path nothing will ever create; the retry loop bounds it.
        let started = std::time::Instant::now();
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
