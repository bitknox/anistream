//! Assembling the provider chain.
//!
//! Lifted out of the binary so it can be exercised. This is the code that decides whether
//! torrent playback comes up at all — the VPN guard runs here, before any session is created — and
//! while it lived in `main.rs` the only way to find out what it had registered was to start the
//! whole interface and look. A probe can now ask it directly.

pub mod commands;
pub mod options;
pub mod torrent;
pub mod vpn;

use anistream_model::Msg;
use std::{sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths};
use anistream_net::HttpClient;
use anistream_providers::{
    ProviderRegistry,
    remote::RemoteHttpProvider,
    torrent::TorrentSession,
    vpn::{GuardState, VpnGuard},
};

/// The largest torrent file read: one far past this is not a torrent file.
const TORRENT_FILE_LIMIT: u64 = 16 * 1024 * 1024;

/// What the person gave to open, as the magnet link torrent playback starts: a magnet link or an
/// info hash as it is, or else the path of a torrent file, read for what it says.
pub fn torrent_to_open(input: &str) -> std::result::Result<String, Msg> {
    use anistream_providers::torrent::open;
    if let Some(link) = open::magnet_for(input) {
        return Ok(link);
    }
    let path = std::path::Path::new(input.trim());
    let small =
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() <= TORRENT_FILE_LIMIT);
    let bytes = small.then(|| std::fs::read(path).ok()).flatten();
    bytes.as_deref().and_then(open::magnet_from_file).ok_or(Msg::TorrentLinkUnusable)
}

/// Assemble the provider chain from config, waiting for torrent playback.
///
/// For the command-line paths, which need every source before they can do anything. The
/// interface uses [`build_registry_deferred`] and [`bring_up_torrent`] instead, because torrent
/// playback waits on the VPN guard, and a guard whose tunnel is down takes as long as its
/// timeout to say so. Once up, the session is lent to every source in the chain that names
/// torrents, plugins that join later included.
///
/// Only sources that are actually configured are registered. A source listed in `order` but
/// unconfigured is skipped rather than added as a permanently-failing entry, which would
/// clutter the Providers screen with something the user never asked for.
///
/// The last element is the guard's verdict when it is what held torrent playback back, for
/// a preview to show as the interface would.
pub async fn build_registry(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
    store: &anistream_store::Store,
) -> (
    ProviderRegistry,
    Option<(VpnGuard, Arc<TorrentSession>)>,
    Option<String>,
    Option<GuardState>,
) {
    let registry = build_registry_deferred(config, http);
    if !torrent_wanted(config) {
        return (registry, None, None, None);
    }
    let started = std::time::Instant::now();
    let outcome = start_torrent_playback(config, paths, store).await;
    tracing::info!(elapsed = ?started.elapsed(), ok = outcome.is_ok(), "torrent playback");
    match outcome {
        Ok(up) => {
            // No interface to tell: these paths print to the terminal they were run from.
            if let Some(warning) = &up.warning {
                eprintln!("warning: {}", warning.english());
            }
            lend_torrents(&registry, Some(&up.session), config);
            (registry, Some((up.guard, up.session)), None, None)
        }
        // Playback that cannot start safely stays off, and the reason is surfaced, not
        // swallowed.
        Err((reason, held)) => {
            tracing::warn!(reason = %reason.english(), "torrent playback not started");
            (registry, None, Some(reason.english()), held)
        }
    }
}

/// The provider chain as config describes it. Torrent playback comes up beside it once the VPN
/// guard has passed — see [`bring_up_torrent`].
pub fn build_registry_deferred(config: &Config, http: &HttpClient) -> ProviderRegistry {
    let mut providers: Vec<std::sync::Arc<dyn anistream_core::traits::Provider>> = Vec::new();
    for id in &config.providers.order {
        if config.providers.disabled.contains(id) {
            continue;
        }
        match id.as_str() {
            "remote" => {
                if let Some(url) = &config.providers.remote_url {
                    providers.push(std::sync::Arc::new(RemoteHttpProvider::new(
                        url.clone(),
                        http.clone(),
                    )));
                }
            }
            // Built-in torrent search, which 3.0.0 removed. An entry the upgrade did not reach
            // names nothing, and no plugin may take the id.
            "torrent" => {}
            // Every `.wasm` in the plugin directory, registered at this point in the order. A
            // plugin is indistinguishable from a native source from here on: same ranking, same
            // health tracking, same failover.
            // Deliberately *not* loaded here — see `spawn_plugin_load`. Compiling a component
            // costs hundreds of milliseconds and nothing needs a plugin before the first frame.
            "plugins" => {}
            // Anything else may be a plugin's id, which ranks that plugin at this point in the
            // order — see `rank`. Plugins compile later, so a typo cannot be told from one yet.
            other => tracing::debug!(provider = other, "order entry left for a plugin"),
        }
    }
    let registry = ProviderRegistry::new(providers);
    // A plugin switched off by name joins the chain all the same, and is not tried.
    registry.set_disabled(config.providers.disabled.iter().cloned());
    registry
}

/// Whether the configuration turns torrent playback on.
pub fn torrent_wanted(config: &Config) -> bool {
    config.providers.torrent.enabled
}

/// Lend `session` to every source in `registry` that names torrents, or take it back.
pub fn lend_torrents(
    registry: &ProviderRegistry,
    session: Option<&Arc<TorrentSession>>,
    config: &Config,
) {
    registry.torrents().set(session.cloned(), config.providers.torrent.trackers.clone());
}

/// Put the whole chain in the order `providers.order` gives it, for a source that has just
/// joined it or an order that has just changed. See [`rank`].
pub fn arrange(registry: &ProviderRegistry, config: &Config) {
    let order = &config.providers.order;
    registry.rank_by(|p| rank(p, order));
    // Whatever the user just did to the sources, a source that went down earlier gets to
    // answer again, rather than staying out until the app is restarted.
    registry.retry_down();
}

/// Where a source stands in `order`: at its own entry when the order names it, at `"plugins"`
/// for a plugin it does not name, and after everything for a source it leaves out.
///
/// Then by id, so the plugins left at `"plugins"` run alphabetically. They used to run in the
/// order their files were read, which no one chose and a reinstall could change. Naming a plugin
/// only moves it: whether plugins run at all is still `"plugins"`'s to say, see
/// [`plugins_enabled`].
pub fn rank(
    provider: &dyn anistream_core::traits::Provider,
    order: &[String],
) -> (usize, String) {
    let manifest = provider.manifest();
    let at = |name: &str| order.iter().position(|id| id == name);
    let place = at(&manifest.id)
        .or_else(|| match manifest.kind {
            anistream_core::traits::ProviderKind::Plugin => at("plugins"),
            _ => None,
        })
        .unwrap_or(order.len());
    (place, manifest.id.clone())
}

/// Load plugins in the background and add them to a live registry.
///
/// The whole reason this is not part of [`build_registry`]: compiling a WebAssembly component is
/// expensive and deterministic. Measured at 874 ms for the JavaScript reference plugin — which was
/// most of the ~976 ms the app spent assembling sources before it could draw anything, for a
/// capability most launches never reach for. wasmtime's compilation cache makes the *second* run
/// cheap; loading off the critical path makes the first one cheap too.
///
/// The chain is put in order once they have joined it, so where a plugin is tried is what
/// `providers.order` says, whenever it finished compiling. See [`rank`].
///
/// The chain says they are on their way until they have joined it, or failed to, so a play
/// asked for meanwhile can wait for them rather than look up the episode without them: see
/// [`ProviderRegistry::expect_arrivals`].
pub fn spawn_plugin_load(
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
) -> tokio::task::JoinHandle<Option<String>> {
    // Said before the task is spawned, so a play sent the moment the engine is up sees it.
    let arriving = plugins_enabled(&config).then(|| Arrivals::expected(&registry));
    tokio::spawn(async move {
        let _arriving = arriving?;
        let started = std::time::Instant::now();
        match load_plugins(&config, &http, &paths).await {
            Ok(loaded) => {
                let count = loaded.len();
                registry.extend(loaded);
                arrange(&registry, &config);
                tracing::info!(count, elapsed = ?started.elapsed(), "plugins registered");
                None
            }
            Err(reason) => {
                tracing::warn!(%reason, "plugin host not started");
                Some(reason)
            }
        }
    })
}

/// Sources on their way into a chain, said done when this is dropped: once they have joined it
/// and it is in order, or the load failed, or its task was cut short.
struct Arrivals(ProviderRegistry);

impl Arrivals {
    fn expected(registry: &ProviderRegistry) -> Self {
        registry.expect_arrivals();
        Self(registry.clone())
    }
}

impl Drop for Arrivals {
    fn drop(&mut self) {
        self.0.arrivals_done();
    }
}

/// How long a lookup waits for sources still on their way before it goes ahead without them.
///
/// Compiling a plugin takes under a second once its build is cached and a few the first time;
/// past this, something is wrong with the load, and an answer from the rest of the chain is
/// worth more than waiting on.
pub const ARRIVALS_WAIT: Duration = Duration::from_secs(30);

/// Wait for sources on their way into `registry`, for at most [`ARRIVALS_WAIT`], saying so with
/// `note` while it waits.
pub async fn arrived(registry: &ProviderRegistry, note: impl FnOnce()) {
    if !registry.is_arriving() {
        return;
    }
    note();
    if !registry.arrivals(ARRIVALS_WAIT).await {
        tracing::warn!(wait = ?ARRIVALS_WAIT, "sources still loading; going ahead without them");
    }
}

/// The limits `[providers.plugins]` sets.
pub fn plugin_limits(config: &Config) -> anistream_plugin::Limits {
    anistream_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.max(1).saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    }
}

/// A plugin host as the app runs plugins: lent the HTTP client, given the user's settings, and
/// caching compiled components.
fn plugin_host(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> std::result::Result<anistream_plugin::PluginHost, String> {
    let cache = paths.cache_dir.join("wasm");
    Ok(anistream_plugin::PluginHost::with_cache(
        plugin_limits(config),
        Some(http.clone()),
        Some(&cache),
    )
    .map_err(|e| e.to_string())?
    .with_plugin_settings(config.providers.plugins.settings.clone()))
}

/// Whether plugins are part of the chain at all.
pub fn plugins_enabled(config: &Config) -> bool {
    config.providers.order.iter().any(|id| id == "plugins")
        && !config.providers.disabled.iter().any(|id| id == "plugins")
}

/// Load one plugin file as a provider, for a plugin installed while the app runs.
pub async fn load_plugin(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
    path: &std::path::Path,
) -> std::result::Result<Arc<dyn anistream_core::traits::Provider>, String> {
    let host = plugin_host(config, http, paths)?;
    let plugin = host.load(path).await.map_err(|e| e.to_string())?;
    Ok(Arc::new(anistream_plugin::WasmProvider::new(plugin)))
}

/// Load every plugin in the plugin directory as a provider.
async fn load_plugins(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> std::result::Result<Vec<Arc<dyn anistream_core::traits::Provider>>, String> {
    let host = plugin_host(config, http, paths)?;

    let dir = paths.plugin_dir();
    let providers = anistream_plugin::load_providers(&host, &dir).await;
    tracing::info!(
        dir = %dir.display(),
        count = providers.len(),
        "plugins registered"
    );

    Ok(providers
        .into_iter()
        .map(|p| Arc::new(p) as Arc<dyn anistream_core::traits::Provider>)
        .collect())
}

/// Torrent playback, up: the guard it runs behind, and its session.
pub struct TorrentUp {
    pub guard: VpnGuard,
    pub session: Arc<TorrentSession>,
    /// Said once playback is up, when there is no OS-level kill switch behind the guard.
    pub warning: Option<Msg>,
}

/// The guard torrent playback runs behind, when the configuration can build one: a mistake a
/// retry cannot fix otherwise.
fn prepare_torrent(
    config: &Config,
    fixture: Option<&anistream_providers::vpn::Egress>,
) -> std::result::Result<VpnGuard, Msg> {
    guard_for(config, fixture).map_err(|reason| Msg::TorrentNotChecked { reason })
}

/// The guard `config` describes: one that looks the exit up, or in a fixture build one that
/// answers with `fixture` and makes no request.
pub fn guard_for(
    config: &Config,
    fixture: Option<&anistream_providers::vpn::Egress>,
) -> std::result::Result<VpnGuard, String> {
    let vpn = config.providers.torrent.vpn.clone();
    match fixture {
        Some(egress) => VpnGuard::fixed(vpn, egress.clone()),
        None => VpnGuard::new(vpn),
    }
}

/// Bring up the VPN guard, verify egress, then start the torrent session.
///
/// Ordering is the whole point: the guard is verified *before* a session exists, so there is
/// no window in which torrent traffic could leave unprotected.
// The error is a whole message with its values, which is large; it is built once, on failure.
#[allow(clippy::result_large_err)]
async fn start_torrent_playback(
    config: &Config,
    paths: &Paths,
    store: &anistream_store::Store,
) -> std::result::Result<TorrentUp, (Msg, Option<GuardState>)> {
    let guard = prepare_torrent(config, None).map_err(|reason| (reason, None))?;

    let state = guard.verify().await;
    if !state.is_protected() {
        let reason = match state.reason() {
            Some(reason) => Msg::TorrentVpnWaiting { reason: reason.to_owned() },
            None => Msg::TorrentVpnUnverified,
        };
        return Err((reason, Some(state)));
    }
    tracing::info!(badge = %state.badge(), "vpn guard satisfied");

    sweep_before_session(config, paths, store).await;
    start_torrent_session(paths, guard).await.map_err(|reason| (reason, None))
}

/// Bring torrent playback up for the interface, however long the VPN guard takes to pass.
///
/// The interface draws while this runs: a tunnel that is down made the guard's check wait out
/// its whole timeout before the first frame, with the terminal not yet set up and every key
/// echoing into the shell. And it keeps checking rather than giving up, so a VPN connected a
/// minute after launch brings torrents up without a restart — a failed check used to leave
/// torrents out for the rest of the session. Each verdict reaches the badge as it arrives.
///
/// Errs only for what no amount of waiting fixes: a configuration that cannot work, or a
/// session that would not start.
pub async fn bring_up_torrent(
    config: Config,
    paths: Paths,
    store: anistream_store::Store,
    tx: crate::outbox::Outbox,
    check: std::sync::Arc<tokio::sync::Notify>,
    fixture: Option<anistream_providers::vpn::Egress>,
) -> std::result::Result<TorrentUp, Msg> {
    use anistream_model::event::Event;
    use anistream_providers::vpn::{GuardLog, GuardNote, GuardState};

    let guard = prepare_torrent(&config, fixture.as_ref())?;
    // Before the wait rather than after it: a cache left full by an earlier run should not stay
    // full for as long as the tunnel is down.
    sweep_before_session(&config, &paths, &store).await;

    let mut log = GuardLog::default();
    // Quick at first, for a tunnel that is a moment from connecting, then no more often than
    // the guard's own interval — and that interval at once for a real leak, where every check
    // reaches the lookup service.
    let mut wait = Duration::from_secs(5);
    loop {
        let state = guard.verify().await;
        let leaking = !state.is_protected();
        if tx.send(Event::Vpn(Some(guard_view(&state)))).is_err() {
            return Err(Msg::TorrentInterfaceClosed);
        }
        match log.observe(&state, std::time::Instant::now()) {
            GuardNote::Failing(reason) => {
                tracing::warn!(%reason, "vpn guard failing; torrents wait for it");
                let _ = tx.send(Event::ProviderNote(Msg::TorrentVpnWaiting {
                    reason: reason.clone(),
                }));
            }
            GuardNote::StillFailing { reason, minutes, checks } => {
                tracing::warn!(%reason, minutes, checks, "vpn guard still failing");
            }
            GuardNote::Recovered { minutes } => {
                tracing::info!(minutes, badge = %state.badge(), "vpn guard satisfied");
            }
            GuardNote::Passed => tracing::info!(badge = %state.badge(), "vpn guard satisfied"),
            GuardNote::Quiet => {}
        }
        if !leaking {
            break;
        }
        let ceiling = guard.verify_interval();
        if matches!(state, GuardState::Leaking { .. }) {
            wait = ceiling;
        }
        // Or sooner, when Check Now asks.
        tokio::select! {
            () = tokio::time::sleep(wait.min(ceiling)) => {}
            () = check.notified() => {}
        }
        wait = (wait * 2).min(ceiling);
    }

    start_torrent_session(&paths, guard).await
}

/// The guard's verdict as the interface is told it: what the guard found, without its reasons,
/// which are for the log and the Providers note.
pub fn guard_view(state: &GuardState) -> anistream_model::sources::GuardView {
    use anistream_model::sources::GuardView;
    match state {
        GuardState::Unverified => GuardView::Checking,
        GuardState::Protected { egress } => GuardView::Protected { exit: egress.describe() },
        GuardState::Leaking { .. } => GuardView::Leaking,
        GuardState::Unreachable { .. } => GuardView::Unreachable,
        GuardState::Unprotected => GuardView::Unprotected,
    }
}

/// Bring the stream cache within its limit before a session exists, so nothing a session is
/// about to write can be mistaken for a leftover. Nothing is live yet; downloads are kept by the
/// sweep itself.
async fn sweep_before_session(config: &Config, paths: &Paths, store: &anistream_store::Store) {
    let dir = paths.cache_dir.join("torrents");
    let limit = config.providers.torrent.cache_limit_bytes();
    crate::background::prune::sweep_torrent_cache(dir, limit, store, Default::default()).await;
}

/// Start the torrent session behind a guard that has just passed.
async fn start_torrent_session(
    paths: &Paths,
    guard: VpnGuard,
) -> std::result::Result<TorrentUp, Msg> {
    // Said when there is no OS-level kill switch. The guard is application-level: it stops
    // *anistream* leaking, but cannot stop a bug in anistream, a change in librqbit, or any
    // other process. Only a firewall rule can.
    let enforcement = anistream_providers::vpn::detect_kernel_enforcement().await;
    let warning = if enforcement.is_enforced() {
        tracing::info!("os-level kill switch enforced");
        None
    } else {
        tracing::warn!(advice = enforcement.advice(), "no os-level kill switch");
        Some(Msg::TorrentNoKillSwitch)
    };

    let session = Arc::new(
        anistream_providers::torrent::TorrentSession::start(
            guard.clone(),
            paths.cache_dir.join("torrents"),
            paths.cache_dir.join("dht.json"),
            random_token(),
        )
        .await
        .map_err(|e| Msg::TorrentSessionFailed { reason: e.to_string() })?,
    );

    Ok(TorrentUp { guard, session, warning })
}

/// An unguessable path segment for the loopback stream server.
///
/// Not a security boundary — loopback binding is — but it stops an unrelated local process
/// from stumbling onto the stream via a predictable path.
fn random_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(anistream_store::now() as u64);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use anistream_core::traits::ProviderKind;
    use anistream_providers::mock::MockProvider;

    use super::*;

    fn order(entries: &[&str]) -> Config {
        let mut config = Config::default();
        config.providers.order = entries.iter().map(|e| (*e).to_owned()).collect();
        config
    }

    fn plugin(id: &str) -> Arc<dyn anistream_core::traits::Provider> {
        MockProvider::new(id).kind(ProviderKind::Plugin).arc()
    }

    fn source(id: &str, kind: ProviderKind) -> Arc<dyn anistream_core::traits::Provider> {
        MockProvider::new(id).kind(kind).arc()
    }

    #[test]
    fn a_named_plugin_ranks_at_its_own_entry_and_the_rest_at_plugins_by_id() {
        let config = order(&["beta", "remote-a", "plugins", "remote"]);
        let order = &config.providers.order;
        let ranked = |p: Arc<dyn anistream_core::traits::Provider>| rank(p.as_ref(), order);
        assert_eq!(ranked(plugin("beta")), (0, "beta".into()));
        assert_eq!(ranked(source("remote-a", ProviderKind::Remote)), (1, "remote-a".into()));
        assert_eq!(ranked(plugin("gamma")), (2, "gamma".into()));
        assert_eq!(ranked(plugin("alpha")), (2, "alpha".into()));
        assert_eq!(ranked(source("remote", ProviderKind::Remote)), (3, "remote".into()));
        // Left out of the order: last, as before.
        assert_eq!(ranked(source("other", ProviderKind::Native)), (4, "other".into()));
    }

    #[test]
    fn only_a_plugin_falls_back_to_the_plugins_entry() {
        // A built-in source the order leaves out is not a plugin, whatever the order says
        // about plugins.
        let config = order(&["plugins"]);
        let order = &config.providers.order;
        assert_eq!(rank(source("remote", ProviderKind::Remote).as_ref(), order).0, 1);
        assert_eq!(rank(plugin("alpha").as_ref(), order).0, 0);
    }

    #[test]
    fn a_source_that_went_down_is_tried_again_once_the_order_changes() {
        let registry = ProviderRegistry::new(vec![plugin("alpha"), plugin("beta")]);
        let failure = anistream_core::error::ProviderError::Transport("timed out".into());
        for _ in 0..3 {
            registry.health().record_failure("alpha", &failure, 0);
        }
        assert_eq!(registry.usable_ids(), ["beta"], "alpha is down");
        arrange(&registry, &order(&["beta", "alpha"]));
        assert_eq!(
            registry.usable_ids(),
            ["beta", "alpha"],
            "the user's change is a new chance"
        );
    }

    #[test]
    fn arranging_puts_the_chain_in_the_order_the_config_gives() {
        // As plugins arrive: in the order their files were read, after the built-in sources.
        let registry = ProviderRegistry::new(vec![
            source("native", ProviderKind::Native),
            plugin("gamma"),
            plugin("beta"),
            plugin("alpha"),
        ]);
        arrange(&registry, &order(&["beta", "native", "plugins"]));
        assert_eq!(registry.ids(), ["beta", "native", "alpha", "gamma"]);

        // A late source still takes its place in it.
        let config = order(&["beta", "remote", "native", "plugins"]);
        arrange(&registry, &config);
        let order = &config.providers.order;
        registry.insert_ranked(source("remote", ProviderKind::Remote), |p| rank(p, order));
        assert_eq!(registry.ids(), ["beta", "remote", "native", "alpha", "gamma"]);
    }

    #[test]
    fn a_torrent_to_open_is_a_link_a_hash_or_a_torrent_file() {
        let hash = "fd45c2c75aedb6a1916c201a03f9640e9ee006f6";
        assert_eq!(torrent_to_open(hash), Ok(format!("magnet:?xt=urn:btih:{hash}")));

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ep01.torrent");
        std::fs::write(
            &file,
            b"d4:infod6:lengthi10e4:name8:ep01.mkv12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaaee",
        )
        .unwrap();
        let opened = torrent_to_open(file.to_str().unwrap()).unwrap();
        assert_eq!(opened, format!("magnet:?xt=urn:btih:{hash}&dn=ep01.mkv"));

        let page = dir.path().join("page.html");
        std::fs::write(&page, "<html></html>").unwrap();
        for unusable in [page.to_str().unwrap(), "no such file", "https://index.example/1"] {
            assert_eq!(torrent_to_open(unusable), Err(Msg::TorrentLinkUnusable), "{unusable}");
        }
    }

    /// A preview shows the badge and toast the interface would, so the guard's verdict has to
    /// come back with the reason it held torrent playback back.
    #[tokio::test]
    async fn a_guard_that_holds_torrents_back_comes_back_with_its_verdict() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let mut config = Config::default();
        let torrent = &mut config.providers.torrent;
        torrent.enabled = true;
        // A proxy nothing listens on: the egress check fails at once and offline.
        torrent.vpn.socks_url = Some("socks5://127.0.0.1:1".into());
        let http = HttpClient::new(&config.network).unwrap();
        let store = anistream_store::Store::open_in_memory().unwrap();

        let (registry, guard, note, held) =
            build_registry(&config, &http, &paths, &store).await;
        assert!(guard.is_none() && registry.torrents().session().is_none());
        assert!(
            note.is_some_and(|n| n.starts_with("Torrents start once the VPN guard passes."))
        );
        let held = held.expect("the verdict that held it back");
        assert!(!held.is_protected(), "{held:?}");
    }
}
