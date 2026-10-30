//! The engine, in-process: the same `Engine` the terminal and the phones' FFI wrap, started on a
//! tokio runtime of its own, hosting its player in this app. Nothing here runs on GTK's main
//! loop: the engine's work is on the runtime, and what it says reaches the loop over channels.

#[cfg(any(feature = "fixtures", test))]
use std::path::Path;
use std::sync::Arc;

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    engine::{Engine, EventStream},
    host::Host,
    options::{AppClients, EngineOptions},
};
use anistream_model::playback::PlayerTheme;
use anistream_net::HttpClient;
use anistream_store::Store;

/// What the engine asks of the desktop, for the main loop to do: it is asked on the engine's
/// threads, and links and notifications are GTK's to show.
#[derive(Debug, Clone, PartialEq)]
pub enum HostRequest {
    OpenUrl(String),
    Notify { title: String, body: String },
}

/// The engine's host: requests go to the main loop. Tokens stay in the engine's `0600` files
/// under the data directory until the Secret portal is wired in.
pub struct DesktopHost {
    requests: async_channel::Sender<HostRequest>,
}

impl DesktopHost {
    pub fn new(requests: async_channel::Sender<HostRequest>) -> Self {
        Self { requests }
    }
}

impl Host for DesktopHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.requests
            .send_blocking(HostRequest::OpenUrl(url.to_owned()))
            .map_err(|_| "the window is gone".to_owned())
    }

    fn notify(&self, title: &str, body: &str) {
        let _ = self
            .requests
            .send_blocking(HostRequest::Notify { title: title.into(), body: body.into() });
    }
}

/// The engine started, and whatever has to live as long as it does.
pub struct Started {
    pub engine: Engine,
    pub events: EventStream,
    /// A fixture scene's fake AniList, which answers on the engine's runtime.
    pub keep: Option<Box<dyn std::any::Any + Send>>,
    /// Run when the engine says it rebuilt the sources: a fixture scene puts its own chain
    /// back, since the rebuild goes by the config, which describes none of the scene's sources.
    pub rebuilt: Option<Box<dyn Fn() + Send>>,
}

/// Start the engine on the user's own directories, the terminal app's, so both share a history,
/// a config and a library.
pub async fn start(paths: Paths, host: Arc<dyn Host>) -> Result<Started, String> {
    let (config, store, set_aside) = open(&paths)?;
    let http = HttpClient::new(&config.network).map_err(|e| e.to_string())?;
    let mut options = EngineOptions::new(paths, config.clone(), http, store, theme());
    options.saved = Some(config);
    options.set_aside = set_aside;
    options.hosted_player = true;
    options.app_clients = app_clients();
    options.device_label = anistream_i18n::t::shell_device_label();
    let (engine, events) = Engine::start(options, host).await.map_err(|e| e.to_string())?;
    Ok(Started { engine, events, keep: None, rebuilt: None })
}

/// anistream's own tracker clients, the ones the Mac and the phones ship: AniList's implicit
/// grant and MyAnimeList's PKCE redirect to `anistream://auth/<tracker>`, which the desktop file
/// hands back to this app, and Simkl and Trakt sign in with a code typed on their site. None of
/// them is a secret.
pub fn app_clients() -> AppClients {
    AppClients {
        anilist: Some("54537".into()),
        mal: Some("0ccc4725381aca9513f26d32107a94cd".into()),
        simkl: Some("016a3721d77c54d04f5aa6f5a213fe76f047d83d3ae35b2c892074859b88be0b".into()),
        trakt: Some("p2yWSSGe7aVAlnHEMiQQh0RhiL6ivZfJNaubOJi4DZE".into()),
        ..AppClients::default()
    }
}

/// The directories under `root`, for a run that is not the user's own: a scene, or a test.
#[cfg(any(feature = "fixtures", test))]
pub fn paths_under(root: &Path) -> Paths {
    Paths {
        config_file: root.join("config").join("config.toml"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
    }
}

/// Create the directories, read the config and open the database.
pub fn open(paths: &Paths) -> Result<(Config, Store, Option<std::path::PathBuf>), String> {
    let config_dir = paths.config_file.parent().unwrap_or(&paths.data_dir).to_path_buf();
    for dir in [config_dir, paths.data_dir.clone(), paths.cache_dir.clone()] {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let config = Config::read(paths).map_err(|e| format!("config: {e:?}"))?; // i18n-skip
    let (store, set_aside) =
        Store::open_or_set_aside(paths.database()).map_err(|e| format!("database: {e}"))?; // i18n-skip
    Ok((config, store, set_aside))
}

/// How the engine dresses the player's own interface: the Obi & Silk palette.
pub fn theme() -> PlayerTheme {
    PlayerTheme {
        text: "#E4E0D6".into(),
        dim: "#9298B5".into(),
        obi: "#F2A64B".into(),
        state: "#7FC7D9".into(),
        alert: "#E06B54".into(),
        ground: "#161A2E".into(),
        font: "Zen Kaku Gothic New".into(), // i18n-skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_under_a_root_keeps_everything_under_it() {
        let root = tempfile::tempdir().expect("a scratch directory");
        let paths = paths_under(root.path());
        let (_config, _store, _) = open(&paths).expect("it opens");
        for path in [&paths.config_file, &paths.data_dir, &paths.cache_dir] {
            assert!(path.starts_with(root.path()), "{} is outside the root", path.display());
        }
        assert!(paths.database().exists(), "the database was made under the root");
    }
}
