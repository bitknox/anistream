//! Assembling the provider chain.
//!
//! Lifted out of the binary so it can be exercised. This is the code that decides whether the
//! torrent source exists at all — the VPN guard runs here, before any session is created — and
//! while it lived in `main.rs` the only way to find out what it had registered was to start the
//! whole interface and look. A probe can now ask it directly.

use std::{sync::Arc, time::Duration};

use anistream_core::config::{Config, Paths};
use anistream_net::HttpClient;
use anistream_providers::{
    ProviderRegistry, remote::RemoteHttpProvider, torrent::TorrentSession, vpn::VpnGuard,
};

/// Assemble the provider chain from config.
///
/// Only sources that are actually configured are registered. A source listed in `order` but
/// unconfigured is skipped rather than added as a permanently-failing entry, which would
/// clutter the Providers screen with something the user never asked for.
pub async fn build_registry(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> (ProviderRegistry, Option<(VpnGuard, Arc<TorrentSession>)>, Option<String>) {
    let mut providers: Vec<std::sync::Arc<dyn anistream_core::traits::Provider>> = Vec::new();
    let mut vpn_guard = None;
    let mut note: Option<String> = None;

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
            "torrent" if config.providers.torrent.enabled => {
                let started = std::time::Instant::now();
                let outcome = start_torrent_provider(config, http, paths).await;
                tracing::info!(elapsed = ?started.elapsed(), ok = outcome.is_ok(), "torrent source");
                match outcome {
                    Ok((provider, guard, session)) => {
                        vpn_guard = Some((guard, session));
                        providers.push(provider);
                    }
                    // A source that cannot start safely is left out rather than registered
                    // as permanently failing — and the reason is surfaced, not swallowed.
                    Err(reason) => {
                        tracing::warn!(%reason, "torrent source not started");
                        note = Some(reason);
                    }
                }
            }
            // Off by default: torrenting stays unreachable until a VPN mode is chosen.
            "torrent" => tracing::info!("torrent source disabled in config"),
            // Every `.wasm` in the plugin directory, registered at this point in the order. A
            // plugin is indistinguishable from a native source from here on: same ranking, same
            // health tracking, same failover.
            // Deliberately *not* loaded here — see `spawn_plugin_load`. Compiling a component
            // costs hundreds of milliseconds and nothing needs a plugin before the first frame.
            "plugins" => {}
            other => tracing::warn!(provider = other, "unknown provider in config order"),
        }
    }

    (ProviderRegistry::new(providers), vpn_guard, note)
}

/// Load plugins in the background and add them to a live registry.
///
/// The whole reason this is not part of [`build_registry`]: compiling a WebAssembly component is
/// expensive and deterministic. Measured at 874 ms for the JavaScript reference plugin — which was
/// most of the ~976 ms the app spent assembling sources before it could draw anything, for a
/// capability most launches never reach for. wasmtime's compilation cache makes the *second* run
/// cheap; loading off the critical path makes the first one cheap too.
///
/// Ordering is preserved by appending, so a plugin that finishes compiling mid-session slots in
/// behind the sources that were already there rather than jumping the queue.
pub fn spawn_plugin_load(
    registry: ProviderRegistry,
    config: Config,
    http: HttpClient,
    paths: Paths,
) -> tokio::task::JoinHandle<Option<String>> {
    tokio::spawn(async move {
        if !config.providers.order.iter().any(|id| id == "plugins")
            || config.providers.disabled.iter().any(|id| id == "plugins")
        {
            return None;
        }
        let started = std::time::Instant::now();
        match load_plugins(&config, &http, &paths).await {
            Ok(loaded) => {
                let count = loaded.len();
                registry.extend(loaded);
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

/// Load every plugin in the plugin directory as a provider.
async fn load_plugins(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> std::result::Result<Vec<Arc<dyn anistream_core::traits::Provider>>, String> {
    let limits = anistream_plugin::Limits {
        memory_bytes: config.providers.plugins.memory_mb.saturating_mul(1024 * 1024),
        deadline: Duration::from_secs(config.providers.plugins.deadline_secs.max(1)),
        ..Default::default()
    };
    let cache = paths.cache_dir.join("wasm");
    let host =
        anistream_plugin::PluginHost::with_cache(limits, Some(http.clone()), Some(&cache))
            .map_err(|e| e.to_string())?;

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

/// Bring up the VPN guard, verify egress, then start the torrent session.
///
/// Ordering is the whole point: the guard is verified *before* a session exists, so there is
/// no window in which torrent traffic could leave unprotected.
async fn start_torrent_provider(
    config: &Config,
    http: &HttpClient,
    paths: &Paths,
) -> std::result::Result<
    (Arc<dyn anistream_core::traits::Provider>, VpnGuard, Arc<TorrentSession>),
    String,
> {
    // No indexer, no source. anistream ships none, so this is the user's to supply and
    // there is nothing to search until they do.
    let settings = anistream_providers::torrent::provider::IndexerSettings::from_config(
        &config.providers.torrent,
    )
    .ok_or_else(|| {
        "no providers.torrent.rss_url configured (anistream ships no indexer)".to_owned()
    })?;

    let guard = VpnGuard::new(config.providers.torrent.vpn.clone())?;

    let state = guard.verify().await;
    if !state.is_protected() {
        return Err(format!("vpn guard: {}", state.reason().unwrap_or("egress not verified")));
    }
    tracing::info!(badge = %state.badge(), "vpn guard satisfied");

    // Warn loudly when there is no OS-level kill switch. The guard below is
    // application-level: it stops *anistream* leaking, but cannot stop a bug in anistream,
    // a change in librqbit, or any other process. Only a firewall rule can.
    let enforcement = anistream_providers::vpn::detect_kernel_enforcement().await;
    if enforcement.is_enforced() {
        tracing::info!("os-level kill switch enforced");
    } else {
        tracing::warn!(advice = enforcement.advice(), "no os-level kill switch");
        eprintln!("warning: no OS-level VPN kill switch detected.");
        eprintln!("         {}", enforcement.advice());
        eprintln!("         anistream's own guard is defence in depth, not a guarantee.");
    }

    let session = Arc::new(
        anistream_providers::torrent::TorrentSession::start(
            guard.clone(),
            paths.cache_dir.join("torrents"),
            random_token(),
        )
        .await
        .map_err(|e| e.to_string())?,
    );

    Ok((
        Arc::new(anistream_providers::torrent::TorrentProvider::new(
            http.clone(),
            guard.clone(),
            Arc::clone(&session),
            config.playback.quality,
            settings,
        )),
        guard,
        session,
    ))
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
