//! The terminal the interface draws on: written to only when the picture changes, and given back
//! however the interface ends.
//!
//! The interface puts the terminal in raw mode on the alternate screen with the cursor hidden,
//! and only its own exit path undoes that. A signal's default action ends the process without
//! running any of it, and so does a panic on the interface's thread: the shell came back raw,
//! typing invisible, the screen still showing the last frame, and mpv — whose session is only
//! closed when the playback task is dropped — orphaned and still playing. This module is the
//! rest of the ways out.

use std::io::{self, Write};

use crossterm::{
    cursor::Show,
    event::DisableBracketedPaste,
    execute,
    terminal::{LeaveAlternateScreen, disable_raw_mode},
};

/// Put the terminal back the way the shell expects it.
///
/// Every step is attempted whatever happened to the one before, because this runs exactly when
/// things have gone wrong: after a hangup the terminal is gone and every write fails, and a
/// cursor left hidden because leaving the alternate screen failed first would be the same bug
/// in a smaller way. The first error is returned once all of them have been tried.
pub fn restore(out: &mut impl Write) -> io::Result<()> {
    let raw = disable_raw_mode();
    let paste = execute!(out, DisableBracketedPaste);
    let screen = execute!(out, LeaveAlternateScreen);
    let cursor = execute!(out, Show);
    raw.and(paste).and(screen).and(cursor)
}

/// Restore the terminal before a panic on the interface's thread is reported, and keep every
/// other panic off the screen.
///
/// The interface runs on the main thread; spawned work runs on the runtime's workers, where
/// tokio contains a panic to its task and the interface carries on. Printing one of those would
/// paint over a live screen, so it goes to the log. A panic on the main thread ends the
/// interface: the terminal is restored first so the report is readable, and unwinding drops the
/// runtime, whose tasks — the playback session among them — close the player as they go.
pub fn install_panic_hook() {
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(%info, "panicked");
        anistream_engine::crash::save_panic(info);
        if std::thread::current().name() == Some("main") {
            let _ = restore(&mut io::stdout());
            report(info);
        }
    }));
}

/// The signals that ask the interface to end: a polite kill, a closed terminal, and an
/// interrupt sent from outside — inside the interface, raw mode makes Ctrl-C a key.
pub struct Signals {
    #[cfg(unix)]
    terminate: tokio::signal::unix::Signal,
    #[cfg(unix)]
    hangup: tokio::signal::unix::Signal,
    #[cfg(unix)]
    interrupt: tokio::signal::unix::Signal,
}

impl Signals {
    /// Start listening. From here on these signals no longer end the process on their own:
    /// whoever holds this has to.
    pub fn new() -> io::Result<Self> {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{SignalKind, signal};
            Ok(Self {
                terminate: signal(SignalKind::terminate())?,
                hangup: signal(SignalKind::hangup())?,
                interrupt: signal(SignalKind::interrupt())?,
            })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    /// The next signal, by name.
    pub async fn recv(&mut self) -> &'static str {
        #[cfg(unix)]
        {
            tokio::select! {
                _ = self.terminate.recv() => "SIGTERM",
                _ = self.hangup.recv() => "SIGHUP",
                _ = self.interrupt.recv() => "SIGINT",
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            "Ctrl-C"
        }
    }
}

/// End the process at the next signal, whatever the orderly shutdown is doing.
///
/// Called once that shutdown has begun. The handlers stay installed for the life of the
/// process, so without this a shutdown that hung — a player that would not close — could not be
/// interrupted any more, not even by the signal that started it.
pub fn exit_on_second_signal() {
    tokio::spawn(async {
        let Ok(mut signals) = Signals::new() else { return };
        let name = signals.recv().await;
        tracing::warn!(signal = name, "second signal; exiting at once");
        let _ = restore(&mut io::stdout());
        std::process::exit(130);
    });
}

/// A backend that writes nothing when nothing on screen changed.
///
/// ratatui only sends the cells that differ from the last frame, but every frame still ends with
/// a reset of colours and attributes and a hide-cursor sequence. At the loop's ten frames a
/// second that was a quarter of a kilobyte every second of every idle hour — twenty megabytes a
/// day down an SSH session, for a screen that had not moved. An empty frame now sends nothing,
/// and the cursor is only touched when its state actually changes.
pub struct Quiet<B> {
    inner: B,
    /// What the cursor was last told: hidden or shown, and where. `None` when unknown.
    hidden: Option<bool>,
    at: Option<ratatui::layout::Position>,
    /// Whether anything reached the terminal since [`Self::take_wrote`] last asked.
    wrote: bool,
}

impl<B> Quiet<B> {
    pub fn new(inner: B) -> Self {
        Self { inner, hidden: None, at: None, wrote: false }
    }

    /// Whether the frames since the last call changed anything on screen.
    pub fn take_wrote(&mut self) -> bool {
        std::mem::take(&mut self.wrote)
    }

    /// Drawing moves the terminal's cursor, so its position is no longer known.
    fn touched(&mut self) {
        self.wrote = true;
        self.at = None;
    }
}

impl<B: ratatui::backend::Backend> ratatui::backend::Backend for Quiet<B> {
    type Error = B::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a ratatui::buffer::Cell)>,
    {
        let mut content = content.peekable();
        if content.peek().is_none() {
            return Ok(());
        }
        self.touched();
        self.inner.draw(content)
    }

    fn append_lines(&mut self, n: u16) -> Result<(), Self::Error> {
        self.touched();
        self.inner.append_lines(n)
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        if self.hidden == Some(true) {
            return Ok(());
        }
        self.wrote = true;
        self.hidden = Some(true);
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        if self.hidden == Some(false) {
            return Ok(());
        }
        self.wrote = true;
        self.hidden = Some(false);
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<ratatui::layout::Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> Result<(), Self::Error> {
        let position = position.into();
        if self.at == Some(position) {
            return Ok(());
        }
        self.wrote = true;
        self.at = Some(position);
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.touched();
        self.inner.clear()
    }

    fn clear_region(
        &mut self,
        clear_type: ratatui::backend::ClearType,
    ) -> Result<(), Self::Error> {
        self.touched();
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> Result<ratatui::layout::Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<ratatui::backend::WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()
    }
}

/// Written to around the frames — a clipboard request, the restore at exit — so the cursor's
/// position is no longer what was last set.
impl<B: Write> Write for Quiet<B> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.touched();
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// When a pass of the event loop that nothing but the idle tick woke should paint.
///
/// Anything else that wakes the loop — a key, an update, a resize — may have changed the screen
/// and is always painted. A bare tick only is while something moves on its own: the activity
/// row's clocks and pulse while work runs, or a frame that changed when it was last painted,
/// which covers anything else animating. And once in a while regardless, for the coarse clocks
/// ("today", "expires tomorrow") that change with nothing to say so.
#[derive(Debug, Default)]
pub struct Repaint {
    /// The last paint changed something.
    moving: bool,
    last: Option<std::time::Instant>,
}

/// The longest a still screen goes without being painted.
const HEARTBEAT: std::time::Duration = std::time::Duration::from_secs(30);

impl Repaint {
    /// `live` when something on screen is known to be moving.
    pub fn due(&self, live: bool, now: std::time::Instant) -> bool {
        live || self.moving
            || self.last.is_none_or(|at| now.saturating_duration_since(at) >= HEARTBEAT)
    }

    /// A paint happened, and whether it changed anything.
    pub fn painted(&mut self, changed: bool, now: std::time::Instant) {
        self.moving = changed;
        self.last = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes a terminal was sent, readable while the terminal holds the writer.
    #[derive(Clone, Default)]
    struct Sent(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

    impl Write for Sent {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    type Counted = ratatui::Terminal<Quiet<ratatui::backend::CrosstermBackend<Sent>>>;

    /// A terminal over a byte buffer, for counting what each frame sends.
    fn quiet_terminal() -> (Counted, Sent) {
        let sent = Sent::default();
        let backend = Quiet::new(ratatui::backend::CrosstermBackend::new(sent.clone()));
        let area = ratatui::layout::Rect::new(0, 0, 40, 6);
        let terminal = ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions { viewport: ratatui::Viewport::Fixed(area) },
        )
        .unwrap();
        (terminal, sent)
    }

    fn sent(terminal: &mut Counted, bytes: &Sent) -> usize {
        terminal.backend_mut().take_wrote();
        std::mem::take(&mut *bytes.0.borrow_mut()).len()
    }

    #[test]
    fn an_unchanged_frame_sends_nothing_at_all() {
        let (mut terminal, bytes) = quiet_terminal();
        let frame = |text: &'static str| {
            move |f: &mut ratatui::Frame| {
                f.render_widget(ratatui::text::Text::raw(text), f.area())
            }
        };
        terminal.draw(frame("waiting")).unwrap();
        assert!(sent(&mut terminal, &bytes) > 0, "the first frame is drawn");

        for _ in 0..10 {
            terminal.draw(frame("waiting")).unwrap();
            assert!(!terminal.backend_mut().take_wrote(), "an idle frame counted as a change");
            assert_eq!(sent(&mut terminal, &bytes), 0, "an idle frame wrote to the terminal");
        }

        terminal.draw(frame("arrived")).unwrap();
        assert!(terminal.backend_mut().take_wrote());
        assert!(sent(&mut terminal, &bytes) > 0, "a change still reaches the screen");
    }

    #[test]
    fn a_still_screen_is_painted_only_while_something_moves() {
        let start = std::time::Instant::now();
        let later = |secs| start + std::time::Duration::from_secs(secs);
        let mut repaint = Repaint::default();
        assert!(repaint.due(false, start), "nothing painted yet");

        repaint.painted(true, start);
        assert!(repaint.due(false, later(1)), "the last frame moved, so look again");
        repaint.painted(false, later(1));
        assert!(!repaint.due(false, later(2)), "still: no paint on the idle tick");
        assert!(repaint.due(true, later(2)), "work on screen keeps its clock running");
        assert!(repaint.due(false, later(31)), "and now and then regardless");
    }

    #[test]
    fn restoring_tries_every_step_and_writes_what_the_shell_needs() {
        // Raw mode cannot be turned off here (no terminal under the test runner), which is the
        // point: that step failing must not stop the screen and the cursor coming back.
        let mut out = Vec::new();
        let _ = restore(&mut out);
        let written = String::from_utf8_lossy(&out);
        assert!(written.contains("\x1b[?2004l"), "bracketed paste off: {written:?}");
        assert!(written.contains("\x1b[?1049l"), "alternate screen left: {written:?}");
        assert!(written.contains("\x1b[?25h"), "cursor shown: {written:?}");
    }

    #[tokio::test]
    async fn a_terminate_signal_is_heard_rather_than_ending_the_process() {
        #[cfg(unix)]
        {
            let mut signals = Signals::new().unwrap();
            // Sent to this very process: without the handler the test runner would die here.
            let pid = std::process::id().to_string();
            std::process::Command::new("kill").args(["-TERM", &pid]).status().unwrap();
            let name =
                tokio::time::timeout(std::time::Duration::from_secs(5), signals.recv()).await;
            assert_eq!(name, Ok("SIGTERM"));
        }
    }
}
