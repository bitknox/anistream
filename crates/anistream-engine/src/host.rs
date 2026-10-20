//! What the engine asks of the machine it runs on, through the front end that runs it.
//!
//! Some of the engine's work ends outside it: a link opened in the browser, a notification on
//! the desktop, a token kept in the keychain. On a desktop that is a call to the system, and on
//! a phone it is the app's to make. So the engine asks a [`Host`], the one callback interface it
//! has, and each front end hands it the host it runs on. Everything else crosses the boundary as
//! a command or an event.
//!
//! **Where tokens are kept**, the trackers' and the plugin stores', is decided here and nowhere
//! else, in this order:
//!
//! 1. A host's own store, from [`Host::secrets`], is the only store. A phone's keychain is one:
//!    no setting and no environment variable is read over it, since a phone has no environment
//!    its user sets, and its keychain prompts nobody.
//! 2. The [`DesktopHost`]'s store is the desktop's, as it always was: see [`desktop_secrets`].
//!    `ANISTREAM_TOKEN_STORAGE` still wins there over `trackers.token_storage`.
//! 3. A host that offers no store gets `0600` files under the data directory, the engine's
//!    fallback, which the Accounts screen says, since a file is less than a keychain.

use std::sync::Arc;

use anistream_core::config::{Config, Paths};
use anistream_track::secret::{Backend, LocalSecrets};

pub use anistream_track::secret::{SecretError, SecretStore, Storage};

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

    /// Where the trackers' and the plugin stores' tokens are kept: a phone's keychain, say, as
    /// the app reaches it. Asked once, as the engine starts, and the only store there is: see
    /// the [module docs](self) for what decides where tokens go.
    ///
    /// `None`, the default, for a host with no store to offer: the engine keeps them in `0600`
    /// files under its data directory instead.
    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        None
    }
}

/// Where tokens go for a host with no store to offer: `0600` files under the data directory
/// `paths` names, never a keychain.
pub(crate) fn file_secrets(paths: &Paths) -> Arc<dyn SecretStore> {
    Arc::new(LocalSecrets::new(crate::tracking::token_dir(paths), Backend::File))
}

/// Where the desktop keeps tokens, under the data directory `paths` names: the one place that is
/// decided, so that the app, `--login`, `--sync` and the probes agree. Signing in through one and
/// being signed out through another is what disagreeing would mean.
///
/// 1. `ANISTREAM_TOKEN_STORAGE`, when it is set. `file` keeps the keychain out of one run without
///    the config being edited: macOS keys keychain access on the binary, and every `cargo build`
///    is a new one, which prompts. Anything else means the keychain.
/// 2. Otherwise `trackers.token_storage`, `"keychain"` or `"file"`.
/// 3. The keychain, by default, with a `0600` file as its fallback where no keychain answers,
///    as on a Linux box with no D-Bus session. That fallback is said, not hidden.
pub fn desktop_secrets(config: &Config, paths: &Paths) -> Arc<dyn SecretStore> {
    let backend = Backend::from_env_or(&config.trackers.token_storage);
    Arc::new(LocalSecrets::new(crate::tracking::token_dir(paths), backend))
}

/// The desktop's host: links opened by the system's own handler, notifications through
/// `osascript` on macOS and `notify-send` on Linux, and tokens in the keychain or a `0600` file
/// as [`desktop_secrets`] decides.
///
/// What the engine did itself before front ends other than the terminal were in view. P6 puts
/// it behind a `desktop` feature, with the rest of what has no mobile story.
#[derive(Clone)]
pub struct DesktopHost {
    secrets: Arc<dyn SecretStore>,
}

impl DesktopHost {
    /// The desktop the app runs on, for the config it was launched with and the directories
    /// `paths` names: tokens where [`desktop_secrets`] says.
    pub fn new(config: &Config, paths: &Paths) -> Self {
        Self::with_secrets(desktop_secrets(config, paths))
    }

    /// The desktop, with tokens kept in `secrets` instead: a test's files in a temporary
    /// directory, so that nothing it runs reaches the keychain of whoever runs it.
    pub fn with_secrets(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets }
    }
}

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

    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        Some(self.secrets.clone())
    }
}

/// A host for the engine's own tests: it opens nothing and notifies nobody, and keeps tokens in
/// `0600` files under a directory of the test's, never in a keychain.
#[cfg(test)]
pub(crate) struct QuietHost(Arc<dyn SecretStore>);

#[cfg(test)]
impl QuietHost {
    /// Tokens kept under `dir`.
    pub(crate) fn in_dir(dir: &std::path::Path) -> Self {
        Self(Arc::new(LocalSecrets::new(dir, Backend::File)))
    }
}

#[cfg(test)]
impl Host for QuietHost {
    fn open_url(&self, _: &str) -> Result<(), String> {
        Err("a test opens nothing".into())
    }

    fn notify(&self, _: &str, _: &str) {}

    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        Some(self.0.clone())
    }
}
