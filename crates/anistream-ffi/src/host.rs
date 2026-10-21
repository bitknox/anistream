//! The machine the engine runs on, as the native app offers it.

use std::sync::Arc;

use anistream_core::config::{Config, Paths};
use anistream_engine::host::{
    Backend, Host, SecretError, SecretStore, Storage, shared_secrets,
};

use crate::start::SecretsMode;

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

/// An [`FfiHost`], as the engine's [`Host`], with tokens wherever the app's [`SecretsMode`] puts
/// them.
pub(crate) struct Adapter {
    app: Arc<dyn FfiHost>,
    secrets: Arc<dyn SecretStore>,
}

impl Adapter {
    /// The app's host, keeping tokens where `mode` says for the config and the directories the
    /// engine starts on.
    pub(crate) fn new(
        app: Arc<dyn FfiHost>,
        mode: SecretsMode,
        config: &Config,
        paths: &Paths,
    ) -> Self {
        let env = std::env::var(Backend::ENV).ok();
        let secrets = mode.secrets(app.clone(), config, paths, env.as_deref());
        Self { app, secrets }
    }
}

impl SecretsMode {
    /// Where tokens are kept in this mode, `app` being the app's keychain and `env` what
    /// `ANISTREAM_TOKEN_STORAGE` says, as the environment's value is handed in: a test that set
    /// the process's would race every other test.
    pub(crate) fn secrets(
        self,
        app: Arc<dyn FfiHost>,
        config: &Config,
        paths: &Paths,
        env: Option<&str>,
    ) -> Arc<dyn SecretStore> {
        let keychain = Arc::new(Keychain(app));
        match self {
            Self::Host => keychain,
            Self::FollowConfig => shared_secrets(
                Backend::chosen(env, &config.trackers.token_storage),
                paths,
                keychain,
            ),
        }
    }
}

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
        self.app
            .open_url(url.to_owned())
            .then_some(())
            .ok_or_else(|| "the app did not open it".into())
    }

    fn notify(&self, title: &str, body: &str) {
        self.app.notify(title.to_owned(), body.to_owned());
    }

    fn secrets(&self) -> Option<Arc<dyn SecretStore>> {
        Some(self.secrets.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Mutex};

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

    /// The directories an engine starts on, under `root`: never the user's own.
    fn paths(root: &Path) -> Paths {
        Paths {
            config_file: root.join("config/config.toml"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
        }
    }

    /// A config whose `trackers.token_storage` is `storage`.
    fn storing(storage: &str) -> Config {
        let mut config = Config::default();
        config.trackers.token_storage = storage.into();
        config
    }

    /// The app's host on today's default, the app's keychain.
    fn adapter(app: Arc<dyn FfiHost>) -> Adapter {
        let root = tempfile::tempdir().unwrap();
        let secrets = SecretsMode::Host.secrets(
            app.clone(),
            &Config::default(),
            &paths(root.path()),
            None,
        );
        Adapter { app, secrets }
    }

    /// The token file the terminal app keeps `id` in under `root`'s data directory.
    fn token_file(root: &Path, id: &str) -> std::path::PathBuf {
        root.join("data/tokens").join(format!("{id}.token"))
    }

    #[test]
    fn the_engine_asks_the_apps_host() {
        let app = Arc::new(Recording { opens: true, ..Recording::default() });
        let host = adapter(app.clone());
        assert_eq!(host.open_url("https://trailer.example/1"), Ok(()));
        host.notify("Episode 6", "out now");
        assert_eq!(
            *app.asked.lock().unwrap(),
            ["https://trailer.example/1", "Episode 6: out now"]
        );
    }

    #[test]
    fn a_link_the_app_did_not_open_is_an_error_with_words() {
        let host = adapter(Arc::new(Recording::default()));
        assert_eq!(
            host.open_url("https://trailer.example/1"),
            Err("the app did not open it".into())
        );
    }

    #[test]
    fn tokens_are_kept_in_the_apps_keychain_and_said_to_be() {
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        let secrets = adapter(app.clone()).secrets().expect("the app's keychain");
        assert_eq!(secrets.get("kitsu").unwrap(), None);
        assert_eq!(secrets.set("kitsu", "a-kitsu-token").unwrap(), Storage::Keychain);
        assert_eq!(secrets.get("kitsu").unwrap().as_deref(), Some("a-kitsu-token"));
        assert_eq!(secrets.storage_for("kitsu"), Storage::Keychain);
        secrets.delete("kitsu").unwrap();
        assert!(app.kept.lock().unwrap().is_empty());
    }

    #[test]
    fn a_keychain_that_refuses_is_an_error_with_words() {
        let secrets = adapter(Arc::new(Recording::default())).secrets().unwrap();
        let refused = secrets.set("kitsu", "a-kitsu-token").unwrap_err();
        assert_eq!(refused.to_string(), "keychain: the app's keychain did not keep it");
        assert!(secrets.delete("kitsu").is_err());
    }

    #[test]
    fn the_host_mode_keeps_tokens_in_the_apps_keychain_whatever_the_config_says() {
        // An iOS app's: the config and the environment have no say.
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        let secrets = SecretsMode::Host.secrets(
            app.clone(),
            &storing("file"),
            &paths(root.path()),
            Some("file"),
        );
        assert_eq!(secrets.set("anilist", "an-anilist-token").unwrap(), Storage::Keychain);
        assert_eq!(
            app.kept.lock().unwrap().get("anilist").map(String::as_str),
            Some("an-anilist-token")
        );
        assert!(!root.path().join("data/tokens").exists(), "a token file was written");
    }

    #[test]
    fn following_a_file_config_keeps_tokens_in_the_terminals_own_files() {
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        let secrets = SecretsMode::FollowConfig.secrets(
            app.clone(),
            &storing("file"),
            &paths(root.path()),
            None,
        );
        assert_eq!(secrets.storage_for("anilist"), Storage::File);
        assert_eq!(secrets.set("anilist", "an-anilist-token").unwrap(), Storage::File);
        // The file the terminal reads, with nothing in the app's keychain.
        let file = token_file(root.path(), "anilist");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "an-anilist-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "the token file is not 0600");
        }
        assert!(app.kept.lock().unwrap().is_empty(), "the app's keychain was written");

        // And what the terminal wrote there is read.
        std::fs::write(token_file(root.path(), "mal"), "a-mal-token\n").unwrap();
        assert_eq!(secrets.get("mal").unwrap().as_deref(), Some("a-mal-token"));
        secrets.delete("anilist").unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn following_a_keychain_config_keeps_tokens_in_the_apps_keychain() {
        // The items the terminal keeps through `keyring`, reached through the app's own
        // keychain: one sign-in for both.
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        app.kept.lock().unwrap().insert("kitsu".into(), "the-terminals-token".into());
        let secrets = SecretsMode::FollowConfig.secrets(
            app.clone(),
            &storing("keychain"),
            &paths(root.path()),
            None,
        );
        assert_eq!(secrets.get("kitsu").unwrap().as_deref(), Some("the-terminals-token"));
        assert_eq!(secrets.storage_for("anilist"), Storage::Keychain);
        assert_eq!(secrets.set("anilist", "an-anilist-token").unwrap(), Storage::Keychain);
        assert_eq!(
            app.kept.lock().unwrap().get("anilist").map(String::as_str),
            Some("an-anilist-token")
        );
        assert!(!token_file(root.path(), "anilist").exists(), "a token file was written");
        secrets.delete("anilist").unwrap();
        assert!(!app.kept.lock().unwrap().contains_key("anilist"));
    }

    #[test]
    fn following_a_keychain_config_falls_back_to_the_file_as_the_terminal_does() {
        // Where the keychain refuses a token, the terminal keeps it in the file and says so; so
        // does the app.
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(Recording::default());
        let secrets = SecretsMode::FollowConfig.secrets(
            app.clone(),
            &storing("keychain"),
            &paths(root.path()),
            None,
        );
        assert_eq!(secrets.set("anilist", "an-anilist-token").unwrap(), Storage::File);
        assert_eq!(secrets.storage_for("anilist"), Storage::File);
        assert_eq!(secrets.get("anilist").unwrap().as_deref(), Some("an-anilist-token"));
        assert!(token_file(root.path(), "anilist").exists());
    }

    #[test]
    fn following_the_config_the_environment_wins_as_in_the_terminal() {
        let root = tempfile::tempdir().unwrap();
        let app = Arc::new(Recording { keeps: true, ..Recording::default() });
        let paths = paths(root.path());
        let to_file = SecretsMode::FollowConfig.secrets(
            app.clone(),
            &storing("keychain"),
            &paths,
            Some("file"),
        );
        assert_eq!(to_file.set("anilist", "into-the-file").unwrap(), Storage::File);
        assert!(app.kept.lock().unwrap().is_empty(), "the app's keychain was written");

        let to_keychain = SecretsMode::FollowConfig.secrets(
            app.clone(),
            &storing("file"),
            &paths,
            Some("keychain"),
        );
        assert_eq!(to_keychain.set("mal", "into-the-keychain").unwrap(), Storage::Keychain);
        assert!(app.kept.lock().unwrap().contains_key("mal"));
    }
}
