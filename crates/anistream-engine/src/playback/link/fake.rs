//! A scripted player for the engine's own tests: windows that write down every call the session
//! makes, and report only what the test scripted, in any build, the desktop feature or not.
//!
//! Each file a window is told to load reports the next of the scripts the opener was given, the
//! moment it is loaded, as mpv answers `loadfile`. Every event is waiting on the window's channel
//! by the time the session reads it, so the session reads them in order, one at a time.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anistream_core::traits::PlaybackRequest;
use async_trait::async_trait;
use tokio::sync::mpsc;

use super::{LinkError, Opened, Overlay, PlaybackEvent, PlayerLink, PlayerOpener};

/// One call a session made on its player, as [`RecordingLink`] writes it down.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    /// A window opened, for a request.
    Open {
        fullscreen: bool,
    },
    Load {
        url: String,
        options: Vec<(String, String)>,
    },
    AddSubtitle {
        url: String,
        title: Option<String>,
        language: Option<String>,
    },
    Set {
        property: &'static str,
        value: serde_json::Value,
    },
    Cycle(&'static str),
    Add {
        property: &'static str,
        delta: f64,
    },
    Seek {
        seconds: f64,
        absolute: bool,
    },
    Shaders(Vec<String>),
    AudioFilter {
        add: bool,
        filter: String,
    },
    RestateChapters,
    Overlay(Overlay),
    Stop,
    Quit,
    Shutdown,
}

/// Every call made on the windows one opener opened, in order.
pub(crate) type Calls = Arc<Mutex<Vec<Call>>>;

/// The files still to be loaded, each what it reports once it is.
type Files = Arc<Mutex<VecDeque<Vec<PlaybackEvent>>>>;

/// A window that writes down every call made on it, and is alive until it is shut down.
pub(crate) struct RecordingLink {
    calls: Calls,
    files: Files,
    /// The window's channel, which stays open as a live window's does: a script that does not
    /// end its file leaves the playback waiting.
    events: mpsc::UnboundedSender<PlaybackEvent>,
    alive: AtomicBool,
    quit: AtomicBool,
}

impl RecordingLink {
    fn note(&self, call: Call) -> Result<(), LinkError> {
        self.calls.lock().unwrap().push(call);
        if self.alive.load(Ordering::Relaxed) {
            Ok(())
        } else {
            Err(LinkError("the window has closed".into()))
        }
    }
}

#[async_trait]
impl PlayerLink for RecordingLink {
    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        self.note(Call::Load { url: url.to_owned(), options })?;
        for event in self.files.lock().unwrap().pop_front().unwrap_or_default() {
            let _ = self.events.send(event);
        }
        Ok(())
    }

    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError> {
        self.note(Call::AddSubtitle { url: url.to_owned(), title, language })
    }

    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError> {
        self.note(Call::Set { property, value })
    }

    async fn cycle(&self, property: &'static str) -> Result<(), LinkError> {
        self.note(Call::Cycle(property))
    }

    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError> {
        self.note(Call::Add { property, delta })
    }

    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError> {
        self.note(Call::Seek { seconds, absolute })
    }

    async fn shaders(&self, paths: &[String]) -> Result<(), LinkError> {
        self.note(Call::Shaders(paths.to_vec()))
    }

    async fn audio_filter(&self, add: bool, filter: &str) -> Result<(), LinkError> {
        self.note(Call::AudioFilter { add, filter: filter.to_owned() })
    }

    async fn restate_chapters(&self) -> Result<(), LinkError> {
        self.note(Call::RestateChapters)
    }

    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        self.note(Call::Overlay(overlay))
    }

    async fn stop(&self) -> Result<(), LinkError> {
        self.note(Call::Stop)
    }

    async fn quit(&self) -> Result<(), LinkError> {
        let noted = self.note(Call::Quit);
        self.quit.store(true, Ordering::Relaxed);
        noted
    }

    fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    fn has_quit(&self) -> bool {
        self.quit.load(Ordering::Relaxed)
    }

    async fn diagnostics(&self) -> Option<String> {
        None
    }

    async fn shutdown(self: Box<Self>) {
        let _ = self.note(Call::Shutdown);
        self.alive.store(false, Ordering::Relaxed);
    }
}

/// Opens [`RecordingLink`]s, whose files report the scripts it was given, in turn.
pub(crate) struct ScriptedOpener {
    files: Files,
    calls: Calls,
    /// The channel of the window opened last, for a test that reports more on it itself.
    window: Mutex<Option<mpsc::UnboundedSender<PlaybackEvent>>>,
    scratch: PathBuf,
    /// Whether a window opens at all.
    opens: bool,
}

impl ScriptedOpener {
    /// Windows whose files report `files`, one each, in turn, and keep their scratch files in
    /// `scratch`.
    pub(crate) fn new(scratch: &Path, files: Vec<Vec<PlaybackEvent>>) -> Self {
        Self {
            files: Arc::new(Mutex::new(files.into())),
            calls: Calls::default(),
            window: Mutex::default(),
            scratch: scratch.to_owned(),
            opens: true,
        }
    }

    /// An opener whose windows never open, as an mpv that is not installed.
    pub(crate) fn refusing(scratch: &Path) -> Self {
        Self { opens: false, ..Self::new(scratch, Vec::new()) }
    }

    /// Every call made on the windows this opened, in order.
    pub(crate) fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// Where the window opened last reports, for a test that has it report more once something
    /// it waits for has happened.
    pub(crate) fn window(&self) -> Option<mpsc::UnboundedSender<PlaybackEvent>> {
        self.window.lock().unwrap().clone()
    }
}

#[async_trait]
impl PlayerOpener for ScriptedOpener {
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError> {
        self.calls.lock().unwrap().push(Call::Open { fullscreen: request.fullscreen });
        if !self.opens {
            return Err(LinkError("not installed".into()));
        }
        let (events, rx) = mpsc::unbounded_channel();
        *self.window.lock().unwrap() = Some(events.clone());
        let link = RecordingLink {
            calls: Arc::clone(&self.calls),
            files: Arc::clone(&self.files),
            events,
            alive: AtomicBool::new(true),
            quit: AtomicBool::new(false),
        };
        Ok((Box::new(link), rx))
    }

    fn scratch_dir(&self) -> &Path {
        &self.scratch
    }
}
