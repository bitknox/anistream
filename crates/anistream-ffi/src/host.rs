//! The machine the engine runs on, as the native app offers it.

use std::sync::Arc;

use anistream_engine::host::{Host, SecretError, SecretStore, Storage};

/// What the engine asks of the app it runs in: the one callback interface, implemented in Swift
/// or Kotlin.
///
/// Called on the engine's own threads, never the app's main thread, so an implementation moves to
/// its own thread where it needs one. No call may block for long: the work that asks waits for
/// the answer.
#[uniffi::export(foreign)]
pub trait FfiHost: Send + Sync {
    /// Open `url` in the browser, or in the app that handles it. False when it could not be
    /// opened, which the engine then says in a notice.
    fn open_url(&self, url: String) -> bool;

    /// Post a notification: a copy of a notice, for a user who has switched away. Best effort,
    /// since the notice in the app is the real channel.
    fn notify(&self, title: String, body: String);

    /// The secret kept under `id` in the app's keychain, or `None` when none is, or it could not
    /// be read. The engine keeps the trackers' and the plugin stores' tokens this way, each under
    /// an id of its own, as opaque strings: the iOS keychain, or Android's keystore-backed
    /// storage, is the place for them. These are the only store of tokens the engine has here.
    fn secret_get(&self, id: String) -> Option<String>;

    /// Keep `value` under `id`, in place of whatever was kept there. False when the keychain
    /// refused it, which the engine says in a notice: signed in, but not kept for the next launch.
    fn secret_set(&self, id: String, value: String) -> bool;

    /// Forget what is kept under `id`. True when nothing is kept there now, whether or not
    /// anything was: signing out twice is no failure.
    fn secret_delete(&self, id: String) -> bool;
}

/// An [`FfiHost`], as the engine's [`Host`].
pub(crate) struct Adapter(pub(crate) Arc<dyn FfiHost>);

/// The app's keychain, through its host, as the engine keeps tokens in it.
struct Keychain(Arc<dyn FfiHost>);

impl SecretStore for Keychain {
    fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
        Ok(self.0.secret_get(id.to_owned()))
    }

    fn set(&self, id: &str, value: &str) -> Result<Storage, SecretError> {
        if self.0.secret_set(id.to_owned(), value.to_owned()) {
            Ok(Storage::Keychain)
        } else {
            Err(SecretError::Keychain("the app's keychain did not keep it".into()))
        }
    }

    fn delete(&self, id: &str) -> Result<(), SecretError> {
        if self.0.secret_delete(id.to_owned()) {
            Ok(())
        } else {
            Err(SecretError::Keychain("the app's keychain did not let it go".into()))
        }
    }

    fn storage_for(&self, _: &str) -> Storage {
        Storage::Keychain
    }
}

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

    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        Some(Arc::new(Keychain(self.0.clone())))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Opens what it is asked to when `opens` says so, keeps secrets when `keeps` does, and
    /// keeps what it was asked.
    #[derive(Default)]
    struct Recording {
        opens: bool,
        keeps: bool,
        asked: Mutex<Vec<String>>,
        kept: Mutex<std::collections::HashMap<String, String>>,
    }

    impl FfiHost for Recording {
        fn open_url(&self, url: String) -> bool {
            self.asked.lock().unwrap().push(url);
            self.opens
        }

        fn notify(&self, title: String, body: String) {
            self.asked.lock().unwrap().push(format!("{title}: {body}"));
        }

        fn secret_get(&self, id: String) -> Option<String> {
            self.kept.lock().unwrap().get(&id).cloned()
        }

        fn secret_set(&self, id: String, value: String) -> bool {
            if self.keeps {
                self.kept.lock().unwrap().insert(id, value);
            }
            self.keeps
        }

        fn secret_delete(&self, id: String) -> bool {
            self.kept.lock().unwrap().remove(&id);
            self.keeps
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

    #[test]
    fn tokens_are_kept_in_the_apps_keychain_and_said_to_be() {
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        let secrets = Adapter(app.clone()).secrets().expect("the app's keychain");
        assert_eq!(secrets.get("kitsu").unwrap(), None);
        assert_eq!(secrets.set("kitsu", "a-kitsu-token").unwrap(), Storage::Keychain);
        assert_eq!(secrets.get("kitsu").unwrap().as_deref(), Some("a-kitsu-token"));
        assert_eq!(secrets.storage_for("kitsu"), Storage::Keychain);
        secrets.delete("kitsu").unwrap();
        assert!(app.kept.lock().unwrap().is_empty());
    }

    #[test]
    fn a_keychain_that_refuses_is_an_error_with_words() {
        let secrets = Adapter(Arc::new(Recording::default())).secrets().unwrap();
        let refused = secrets.set("kitsu", "a-kitsu-token").unwrap_err();
        assert_eq!(refused.to_string(), "keychain: the app's keychain did not keep it");
        assert!(secrets.delete("kitsu").is_err());
    }
}
