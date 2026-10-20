//! Handing focus back to the terminal after an external player closes.
//!
//! mpv takes the foreground when its window opens; when it exits, whether focus returns to
//! the terminal is up to the window manager, and on macOS it routinely lands on whichever
//! app was frontmost before the terminal instead. The viewer's next keypress belongs to the
//! episode list, so the app asks for its own window back rather than leaving that to luck.
//!
//! Everything here is best-effort and silent: focus is a courtesy, and a machine where it
//! cannot be granted — no bundle to name, no tool to ask, a compositor that refuses — is a
//! machine where playback still worked.

/// Bring the terminal application back to the foreground.
///
/// Returns immediately; the actual activation runs detached so a slow `open` can never
/// stall the event loop that called this.
pub fn refocus_terminal() {
    #[cfg(target_os = "macos")]
    {
        // The app bundle this process was launched inside — Terminal.app, iTerm2, an
        // editor's integrated terminal — names itself in the environment. `open -b`
        // activates a running bundle without the automation-permission prompt that an
        // AppleScript `activate` would trigger on first use.
        if let Some(bundle) =
            std::env::var("__CFBundleIdentifier").ok().filter(|b| !b.is_empty())
        {
            activate("open", &["-b", &bundle]);
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // X11 terminals export their window id, and xdotool is the common way to raise it.
        // Missing either means doing nothing — Wayland compositors do not let a background
        // process steal focus regardless, which is their call to make.
        if let Some(window) = std::env::var("WINDOWID").ok().filter(|w| !w.is_empty()) {
            activate("xdotool", &["windowactivate", &window]);
        }
    }
}

#[cfg(unix)]
fn activate(program: &str, args: &[&str]) {
    use std::process::Stdio;
    let mut command = tokio::process::Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    match command.spawn() {
        // Awaited off to the side purely to reap the child; the outcome changes nothing.
        Ok(mut child) => {
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
        Err(error) => tracing::debug!(%error, program, "could not ask for focus back"),
    }
}
