//! The engine, in-process: the same `Engine` the terminal and the phones' FFI wrap, started on a
//! tokio runtime of its own, hosting its player in this app.

use std::{path::Path, sync::Arc};

use anistream_core::config::{Config, NetworkConfig, Paths};
use anistream_engine::{
    engine::{Engine, EventStream},
    host::Host,
    options::{Background, EngineOptions},
};
use anistream_model::playback::PlayerTheme;
use anistream_net::HttpClient;
use anistream_store::Store;

/// What the engine asks of the desktop. A spike's: links and notifications are logged, and
/// tokens go to the engine's `0600` files under the data directory.
struct SpikeHost;

impl Host for SpikeHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        tracing::info!(%url, "the engine asked to open a link");
        Err("links are not opened in the spike".into())
    }

    fn notify(&self, title: &str, body: &str) {
        tracing::info!(%title, %body, "notification");
    }
}

/// Start the engine on `root`'s directories, with nothing in the background and no network: the
/// spike plays a file from disk.
pub async fn start(root: &Path) -> Result<(Engine, EventStream), String> {
    let paths = Paths {
        config_file: root.join("config").join("config.toml"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
    };
    for dir in [root.join("config"), paths.data_dir.clone(), paths.cache_dir.clone()] {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let config = Config::read(&paths).map_err(|e| format!("config: {e:?}"))?;
    let store = Store::open(paths.database()).map_err(|e| format!("database: {e}"))?;
    let http = HttpClient::offline(&NetworkConfig::default()).map_err(|e| e.to_string())?;
    let mut options = EngineOptions::new(paths, config.clone(), http, store, theme());
    options.saved = Some(config);
    options.hosted_player = true;
    options.background = Background::none();
    let host: Arc<dyn Host> = Arc::new(SpikeHost);
    Engine::start(options, host).await.map_err(|e| e.to_string())
}

/// The terminal's immersive palette, as the FFI's start uses when an app hands in none.
fn theme() -> PlayerTheme {
    PlayerTheme {
        text: "#E4E0D6".into(),
        dim: "#9298B5".into(),
        obi: "#F2A64B".into(),
        state: "#7FC7D9".into(),
        alert: "#E06B54".into(),
        ground: "#161A2E".into(),
        font: "Cantarell".into(),
    }
}
