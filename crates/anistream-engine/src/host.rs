//! What the engine asks of the machine it runs on, through the front end that runs it.
//!
//! Some of the engine's work ends outside it: a link opened in the browser, a notification on
//! the desktop. On a desktop that is a call to the system, and on a phone it is the app's to
//! make. So the engine asks a [`Host`], the one callback interface it has, and each front end
//! hands it the host it runs on. Everything else crosses the boundary as a command or an event.
//!
//! P6 adds the secrets backend here, beside these, for the tokens the keychain holds now.

/// The machine the engine runs on, as the front end offers it.
///
/// Called from the engine's tasks, on whichever thread runs them, so a host is shared and has
/// to be safe to call from any of them. Neither call may block for long: they are asked in the
/// middle of the work, which waits on them.
pub trait Host: Send + Sync {
    /// Open `url` in the browser, or say why it could not be, in words the engine puts in the
    /// notice it shows instead.
    fn open_url(&self, url: &str) -> Result<(), String>;

    /// Put a notification on the desktop: a copy of a notice for whoever has switched away.
    /// Best effort and silent: the notice in the app is the real channel.
    fn notify(&self, title: &str, body: &str);
}

/// The desktop's host: links opened by the system's own handler, and notifications through
/// `osascript` on macOS and `notify-send` on Linux.
///
/// What the engine did itself before front ends other than the terminal were in view. P6 puts
/// it behind a `desktop` feature, with the rest of what has no mobile story.
#[derive(Debug, Default, Clone, Copy)]
pub struct DesktopHost;

impl Host for DesktopHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        // Detached, so a browser that is slow to start, or that stays in the foreground,
        // never holds up the work that asked for it.
        open::that_detached(url).map_err(|e| e.to_string())
    }

    fn notify(&self, title: &str, body: &str) {
        #[cfg(target_os = "macos")]
        {
            let quoted = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                quoted(body),
                quoted(title)
            );
            let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send").args([title, body]).spawn();
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = (title, body);
        }
    }
}
