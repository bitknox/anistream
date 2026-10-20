//! The machine the engine runs on, as the native app offers it.

use std::sync::Arc;

use anistream_engine::host::Host;

/// What the engine asks of the app it runs in: the one callback interface, implemented in Swift
/// or Kotlin.
///
/// Called on the engine's own threads, never the app's main thread, so an implementation moves to
/// its own thread where it needs one. Neither call may block for long: the work that asks waits
/// for the answer.
#[uniffi::export(foreign)]
pub trait FfiHost: Send + Sync {
    /// Open `url` in the browser, or in the app that handles it. False when it could not be
    /// opened, which the engine then says in a notice.
    fn open_url(&self, url: String) -> bool;

    /// Post a notification: a copy of a notice, for a user who has switched away. Best effort,
    /// since the notice in the app is the real channel.
    fn notify(&self, title: String, body: String);
}

/// An [`FfiHost`], as the engine's [`Host`].
pub(crate) struct Adapter(pub(crate) Arc<dyn FfiHost>);

impl Host for Adapter {
    fn open_url(&self, url: &str) -> Result<(), String> {
        // The engine puts the reason after its own words: "could not open: …".
        self.0
            .open_url(url.to_owned())
            .then_some(())
            .ok_or_else(|| "the app did not open it".into())
    }

    fn notify(&self, title: &str, body: &str) {
        self.0.notify(title.to_owned(), body.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Opens what it is asked to when `opens` says so, and keeps what it was asked.
    #[derive(Default)]
    struct Recording {
        opens: bool,
        asked: Mutex<Vec<String>>,
    }

    impl FfiHost for Recording {
        fn open_url(&self, url: String) -> bool {
            self.asked.lock().unwrap().push(url);
            self.opens
        }

        fn notify(&self, title: String, body: String) {
            self.asked.lock().unwrap().push(format!("{title}: {body}"));
        }
    }

    #[test]
    fn the_engine_asks_the_apps_host() {
        let app = Arc::new(Recording { opens: true, ..Recording::default() });
        let host = Adapter(app.clone());
        assert_eq!(host.open_url("https://trailer.example/1"), Ok(()));
        host.notify("Episode 6", "out now");
        assert_eq!(
            *app.asked.lock().unwrap(),
            ["https://trailer.example/1", "Episode 6: out now"]
        );
    }

    #[test]
    fn a_link_the_app_did_not_open_is_an_error_with_words() {
        let host = Adapter(Arc::new(Recording::default()));
        assert_eq!(
            host.open_url("https://trailer.example/1"),
            Err("the app did not open it".into())
        );
    }
}
