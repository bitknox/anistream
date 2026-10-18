//! Giving the terminal back, however the interface ends.
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

#[cfg(test)]
mod tests {
    use super::*;

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
