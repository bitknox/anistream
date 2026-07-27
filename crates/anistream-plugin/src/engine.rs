//! The wasmtime host.
//!
//! Two configuration choices carry the sandbox, and both are easy to get subtly wrong:
//!
//! **Epoch interruption, not a future timeout.** A guest in `loop {}` never yields, so
//! `tokio::time::timeout` around a call would wait forever — a timeout fires only when the future
//! is polled, and a spinning guest never returns control. Epoch interruption makes wasmtime check
//! a counter at loop back-edges and function entries, so the guest can be stopped mid-loop. A
//! background ticker advances the epoch; each call sets its own deadline.
//!
//! **A `ResourceLimiter`, not a memory flag.** A maximum-memory setting bounds one memory; the
//! limiter is consulted for every growth of every memory and table, which is what actually bounds
//! a guest's footprint.
//!
//! **WASI is linked, but it grants nothing.** This was going to say "no WASI at all", which would
//! have been a nicer claim and a false one. A `wasm32-wasip2` component built in *any* language
//! imports a WASI floor for its standard library — measured on the reference plugin with
//! `wasm-tools component wit`:
//!
//! ```text
//! wasi:io/poll, wasi:io/streams, wasi:io/error
//! wasi:clocks/monotonic-clock
//! wasi:cli/std{in,out,err}, wasi:cli/terminal-*, wasi:cli/environment, wasi:cli/exit
//! ```
//!
//! What matters is what is *absent*: no `wasi:filesystem`, no `wasi:sockets`, no `wasi:random`,
//! no wall clock. Those are not merely unlinked — a component that imported them would fail to
//! instantiate, because they are not in the linker. So the context handed over is empty: no
//! preopened directories, no environment variables, no inherited stdio, no network. A guest can
//! measure elapsed time and write to a discarded stdout; that is the whole extent of it.
//!
//! Refusing to link WASI at all would mean plugin authors compiling to
//! `wasm32-unknown-unknown` and running `wasm-tools component new` by hand — friction paid by
//! every plugin in every language, to remove capabilities that grant nothing.

use std::{path::Path, sync::Arc, time::Duration};

use anistream_core::error::ProviderError;

use crate::{
    host::{self, Capabilities},
    sandbox::Limits,
};

wasmtime::component::bindgen!({
    path: "../../wit/anistream-provider.wit",
    world: "plugin",
    // Async on both sides. Imports must be async because `fetch` performs real I/O, and exports
    // must be async because a guest call can block on one — a synchronous export awaiting an
    // async import would deadlock the runtime.
    //
    // `trappable` lets a host function return `wasmtime::Result`, so a host-side failure surfaces
    // as an attributable trap rather than a panic that would take the process down.
    imports: { default: async | trappable },
    exports: { default: async },
});

// Types declared in the *exported* interface live under `exports::`; the imported `host`
// interface's types do not. Re-exported so callers need not know the generated shape.
use self::anistream::provider::host::{HostError as GuestHostError, HttpRequest, HttpResponse};
pub use self::exports::anistream::provider::provider::{
    Episode, Manifest, MediaStream, ProviderError as GuestError, SearchHit, Subtitle,
};

/// How often the epoch advances.
///
/// This is the deadline's granularity: a guest can overrun by up to one tick. 100ms keeps the
/// ticker's cost negligible while making the overshoot invisible next to a 20-second budget.
const EPOCH_TICK: Duration = Duration::from_millis(100);

/// A table entry is a function reference. Tens of thousands is far more than a parser needs;
/// millions would be a memory-exhaustion vector.
const MAX_TABLE_ENTRIES: usize = 100_000;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("{path}: {message}")]
    Load { path: String, message: String },
    #[error("plugin {plugin} exceeded its {limit:?} deadline")]
    Deadline { plugin: String, limit: Duration },
    #[error("plugin {plugin} trapped: {message}")]
    Trap { plugin: String, message: String },
}

impl PluginError {
    /// Map onto the provider vocabulary the registry's failover rules understand.
    ///
    /// A plugin that hangs or traps is *broken*, not merely empty, so it counts against health and
    /// triggers failover — the same treatment a native provider returning `Parse` gets.
    pub fn as_provider_error(&self) -> ProviderError {
        match self {
            Self::Deadline { .. } => ProviderError::Parse("plugin timed out".into()),
            Self::Trap { message, .. } => {
                ProviderError::Parse(format!("plugin trapped: {message}"))
            }
            Self::Load { message, .. } => ProviderError::Other(message.clone()),
        }
    }

    /// Whether this looks like the deadline firing rather than a genuine fault.
    fn from_call(plugin: &str, limit: Duration, error: &wasmtime::Error) -> Self {
        let message = format!("{error:?}");
        // Attributed deliberately: reporting a deadline as "unreachable" would send whoever reads
        // it into the parser instead of the loop.
        if message.contains("epoch deadline") || message.contains("interrupt") {
            Self::Deadline { plugin: plugin.to_owned(), limit }
        } else {
            Self::Trap { plugin: plugin.to_owned(), message: error.to_string() }
        }
    }
}

/// Per-call guest state: the capabilities it was granted, and its resource ceiling.
///
/// The ceiling lives here rather than in a box because wasmtime borrows the limiter from the
/// store's data — which is the right shape anyway: a guest's limits are part of its state.
pub struct PluginState {
    capabilities: Capabilities,
    ceiling: Ceiling,
    /// A WASI context that grants nothing — see the module docs. Built with no preopens, no
    /// environment, no stdio and no network, purely so a wasip2 component's standard library
    /// finds the imports it links against.
    wasi: wasmtime_wasi::WasiCtx,
    table: wasmtime::component::ResourceTable,
}

impl wasmtime_wasi::WasiView for PluginState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        wasmtime_wasi::WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

/// A context with every capability withheld.
///
/// `WasiCtxBuilder` defaults to granting nothing, so this is deliberately *not* calling any of
/// `inherit_stdio`, `inherit_env`, `preopened_dir` or `inherit_network`. Written out rather than
/// left implicit, because the absence of those calls is the security property.
fn empty_wasi() -> wasmtime_wasi::WasiCtx {
    wasmtime_wasi::WasiCtxBuilder::new().build()
}

/// Bounds every memory and table growth a guest attempts.
struct Ceiling {
    memory_bytes: usize,
}

impl wasmtime::ResourceLimiter for Ceiling {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= self.memory_bytes)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(desired <= MAX_TABLE_ENTRIES)
    }
}

// The host side of the WIT `host` interface. This is the entire surface a guest can reach.
impl anistream::provider::host::Host for PluginState {
    async fn fetch(
        &mut self,
        request: HttpRequest,
    ) -> wasmtime::Result<Result<HttpResponse, GuestHostError>> {
        let outbound = host::Request {
            method: request.method,
            url: request.url,
            headers: request.headers,
            body: request.body,
        };
        Ok(match self.capabilities.fetch(outbound).await {
            Ok(response) => Ok(HttpResponse {
                status: response.status,
                headers: response.headers,
                body: response.body,
            }),
            Err(host::HostError::Denied(m)) => Err(GuestHostError::Denied(m)),
            Err(host::HostError::Timeout) => Err(GuestHostError::Timeout),
            Err(host::HostError::Transport(m)) => Err(GuestHostError::Transport(m)),
        })
    }

    async fn log(&mut self, level: String, msg: String) -> wasmtime::Result<()> {
        // A guest's only output channel, and deliberately attributed: an unexplained log line
        // from a third-party parser would be worse than none.
        let plugin = self.capabilities.plugin_id();
        match level.to_ascii_lowercase().as_str() {
            "error" => tracing::error!(plugin, "{msg}"),
            "warn" => tracing::warn!(plugin, "{msg}"),
            "debug" | "trace" => tracing::debug!(plugin, "{msg}"),
            _ => tracing::info!(plugin, "{msg}"),
        }
        Ok(())
    }

    async fn aes_decrypt(
        &mut self,
        key: Vec<u8>,
        iv: Vec<u8>,
        data: Vec<u8>,
    ) -> wasmtime::Result<Result<Vec<u8>, String>> {
        Ok(host::aes_decrypt(&key, &iv, &data))
    }

    async fn regex_captures(
        &mut self,
        pattern: String,
        haystack: String,
    ) -> wasmtime::Result<Vec<Vec<String>>> {
        Ok(host::regex_captures(&pattern, &haystack))
    }
}

/// Loads plugins and owns the shared engine.
#[derive(Clone)]
pub struct PluginHost {
    engine: wasmtime::Engine,
    limits: Limits,
    http: Option<anistream_net::HttpClient>,
    /// Keeps the epoch ticker alive for as long as any plugin might run.
    _ticker: Arc<TickerGuard>,
}

/// Advances the engine's epoch on a timer, and stops when the host is dropped.
///
/// **A dedicated OS thread, not a tokio task.** This was a tokio task first, and the adversarial
/// test hung: a guest spinning in `loop {}` occupies the executor thread, so a task that advances
/// the epoch never gets polled — the deadline can never fire, and the very case epoch interruption
/// exists for is the one case it fails to handle. A plain thread with `thread::sleep` advances
/// regardless of what the async runtime is doing, or how saturated it is.
struct TickerGuard {
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for TickerGuard {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        // Not joined: the thread wakes at most one tick from now and exits on its own. Joining
        // would make dropping the host block for up to a tick for no benefit.
        drop(self.handle.take());
    }
}

impl PluginHost {
    /// Build a host with the given limits.
    ///
    /// `http` may be `None`, yielding a host whose plugins load and describe themselves but
    /// cannot fetch — which is exactly what `plugin inspect` and the conformance tests want.
    pub fn new(
        limits: Limits,
        http: Option<anistream_net::HttpClient>,
    ) -> Result<Self, PluginError> {
        Self::with_cache(limits, http, None)
    }

    /// A host that caches compiled components under `cache_dir`.
    ///
    /// Compilation is the expensive part and it is entirely deterministic, so paying it once per
    /// build of a plugin rather than once per launch is the obvious win: the JavaScript reference
    /// plugin measured **874 ms**, which was most of what the app spent before its first frame.
    ///
    /// wasmtime owns the cache, which is the point of doing it this way. Loading a precompiled
    /// artefact by hand needs `unsafe` — it maps native code — and this crate forbids `unsafe`
    /// because its whole job is running code we do not trust. Delegating keeps that invariant and
    /// still gets the version-and-settings validation right, which is the part a hand-rolled cache
    /// would most likely get wrong.
    pub fn with_cache(
        limits: Limits,
        http: Option<anistream_net::HttpClient>,
        cache_dir: Option<&Path>,
    ) -> Result<Self, PluginError> {
        let mut config = wasmtime::Config::new();
        config
            .wasm_component_model(true)
            // The only mechanism that can stop a guest which never yields. Everything else here
            // is a default; async support is unconditional in wasmtime 47 and no longer a flag.
            .epoch_interruption(true);

        if let Some(dir) = cache_dir {
            // Best-effort: a cache that will not initialise costs a slow start, never a failure,
            // so a bad path must not stop plugins from loading at all.
            let mut cache_config = wasmtime::CacheConfig::new();
            cache_config.with_directory(dir);
            match wasmtime::Cache::new(cache_config) {
                Ok(cache) => {
                    config.cache(Some(cache));
                    tracing::debug!(dir = %dir.display(), "plugin compilation cache enabled");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "plugin cache unavailable; compiling cold")
                }
            }
        }

        let engine = wasmtime::Engine::new(&config)
            .map_err(|e| PluginError::Load { path: "engine".into(), message: e.to_string() })?;

        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handle = {
            let engine = engine.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name("anistream-plugin-epoch".into())
                .spawn(move || {
                    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                        std::thread::sleep(EPOCH_TICK);
                        engine.increment_epoch();
                    }
                })
                .map_err(|e| PluginError::Load {
                    path: "epoch ticker".into(),
                    message: e.to_string(),
                })?
        };

        Ok(Self {
            engine,
            limits,
            http,
            _ticker: Arc::new(TickerGuard { stop, handle: Some(handle) }),
        })
    }

    /// Compile a `.wasm` component and read its manifest.
    ///
    /// The manifest is read immediately because it declares `allowed-hosts`, and nothing can be
    /// authorised until that is known.
    pub async fn load(&self, path: impl AsRef<Path>) -> Result<LoadedPlugin, PluginError> {
        let path = path.as_ref();
        let display = path.display().to_string();
        let fail = |message: String| PluginError::Load { path: display.clone(), message };

        let bytes = tokio::fs::read(path).await.map_err(|e| fail(e.to_string()))?;
        // Measured at 874 ms for the JavaScript reference plugin, which is 90% of the time the
        // app spends assembling its provider chain. Caching wasmtime's precompiled artefact would
        // remove it, but loading one requires `unsafe` — this crate forbids it, and running
        // untrusted plugins is not where that invariant gets traded for 800 ms. Logged instead, so
        // a slow start is attributable rather than mysterious.
        let started = std::time::Instant::now();
        let component = wasmtime::component::Component::new(&self.engine, &bytes)
            .map_err(|e| fail(e.to_string()))?;
        tracing::info!(elapsed = ?started.elapsed(), plugin = %path.display(), "component compiled");

        let mut linker = wasmtime::component::Linker::new(&self.engine);
        // The empty WASI floor first — a wasip2 component's standard library links against it in
        // every language, and without it nothing instantiates. It grants nothing; see the module
        // docs for what is deliberately not in this linker.
        wasmtime_wasi::p2::add_to_linker_async(&mut linker).map_err(|e| fail(e.to_string()))?;
        Plugin::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |state| state)
            .map_err(|e| fail(e.to_string()))?;

        let plugin = LoadedPlugin {
            engine: self.engine.clone(),
            linker,
            component,
            // Filled in below; a plugin cannot be trusted to be asked about itself twice and
            // answer the same way, so the answer is captured once.
            manifest: Manifest {
                id: String::new(),
                display_name: String::new(),
                version: String::new(),
                allowed_hosts: Vec::new(),
                translation_types: Vec::new(),
            },
            limits: self.limits,
            http: self.http.clone(),
        };

        // Describing itself is the one call made with *no* capabilities: a manifest must not be
        // able to depend on the network access it is about to request.
        let manifest = plugin
            .describe_with(Capabilities::new(display.clone(), Vec::new(), self.limits, None))
            .await
            .map_err(|e| fail(e.to_string()))?;

        if manifest.id.trim().is_empty() {
            return Err(fail("manifest has no id".into()));
        }

        tracing::info!(
            id = %manifest.id,
            version = %manifest.version,
            hosts = ?manifest.allowed_hosts,
            "plugin loaded"
        );
        Ok(LoadedPlugin { manifest, ..plugin })
    }

    /// Every `*.wasm` in a directory, in a stable order.
    ///
    /// A missing directory yields an empty list rather than an error: having no plugins is the
    /// normal case, not a failure.
    pub async fn load_dir(
        &self,
        dir: impl AsRef<Path>,
    ) -> Vec<Result<LoadedPlugin, PluginError>> {
        let Ok(mut entries) = tokio::fs::read_dir(dir.as_ref()).await else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("wasm")) {
                paths.push(path);
            }
        }
        // Sorted, so provider precedence never depends on filesystem order.
        paths.sort();

        let mut loaded = Vec::with_capacity(paths.len());
        for path in paths {
            loaded.push(self.load(&path).await);
        }
        loaded
    }
}

/// A compiled plugin, ready to call.
///
/// Compilation happens once at load; every call gets a fresh `Store`, which is what stops one
/// call leaking state into the next — a guest cannot stash a cookie from your search and use it
/// during someone else's.
pub struct LoadedPlugin {
    engine: wasmtime::Engine,
    linker: wasmtime::component::Linker<PluginState>,
    component: wasmtime::component::Component,
    manifest: Manifest,
    limits: Limits,
    http: Option<anistream_net::HttpClient>,
}

/// Identity and limits only.
///
/// Hand-written because the interesting fields — a compiled component, a linker — have no useful
/// representation, and a derived impl would print pages of wasmtime internals into a log line.
impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("id", &self.manifest.id)
            .field("version", &self.manifest.version)
            .field("allowed_hosts", &self.manifest.allowed_hosts)
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

impl LoadedPlugin {
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    pub fn id(&self) -> &str {
        &self.manifest.id
    }

    /// A store with this call's capabilities, ceilings and deadline.
    fn store(&self, capabilities: Capabilities) -> wasmtime::Store<PluginState> {
        let mut store = wasmtime::Store::new(
            &self.engine,
            PluginState {
                capabilities,
                ceiling: Ceiling { memory_bytes: self.limits.memory_bytes },
                wasi: empty_wasi(),
                table: wasmtime::component::ResourceTable::new(),
            },
        );
        store.limiter(|state| &mut state.ceiling);
        // Ticks, not milliseconds — the deadline is counted in epoch increments.
        let ticks = (self.limits.deadline.as_millis() / EPOCH_TICK.as_millis()).max(1) as u64;
        store.set_epoch_deadline(ticks);
        store
    }

    /// Capabilities for a real call: the declared hosts, and the shared client.
    fn capabilities(&self) -> Capabilities {
        Capabilities::new(
            self.manifest.id.clone(),
            self.manifest.allowed_hosts.clone(),
            self.limits,
            self.http.clone(),
        )
    }

    async fn instantiate(
        &self,
        store: &mut wasmtime::Store<PluginState>,
    ) -> Result<Plugin, PluginError> {
        Plugin::instantiate_async(store, &self.component, &self.linker)
            .await
            .map_err(|e| PluginError::from_call(&self.manifest.id, self.limits.deadline, &e))
    }

    async fn describe_with(&self, capabilities: Capabilities) -> Result<Manifest, PluginError> {
        let mut store = self.store(capabilities);
        let instance = self.instantiate(&mut store).await?;
        instance
            .anistream_provider_provider()
            .call_describe(&mut store)
            .await
            .map_err(|e| PluginError::from_call("plugin", self.limits.deadline, &e))
    }

    /// Re-read the manifest, for `plugin inspect`.
    pub async fn describe(&self) -> Result<Manifest, PluginError> {
        self.describe_with(self.capabilities()).await
    }

    pub async fn search(
        &self,
        query: &str,
        translation: &str,
    ) -> Result<Result<Vec<SearchHit>, GuestError>, PluginError> {
        let mut store = self.store(self.capabilities());
        let instance = self.instantiate(&mut store).await?;
        instance
            .anistream_provider_provider()
            .call_search(&mut store, query, translation)
            .await
            .map_err(|e| PluginError::from_call(&self.manifest.id, self.limits.deadline, &e))
    }

    pub async fn list_episodes(
        &self,
        id: &str,
        translation: &str,
    ) -> Result<Result<Vec<Episode>, GuestError>, PluginError> {
        let mut store = self.store(self.capabilities());
        let instance = self.instantiate(&mut store).await?;
        instance
            .anistream_provider_provider()
            .call_list_episodes(&mut store, id, translation)
            .await
            .map_err(|e| PluginError::from_call(&self.manifest.id, self.limits.deadline, &e))
    }

    pub async fn resolve(
        &self,
        id: &str,
        episode: &str,
        translation: &str,
    ) -> Result<Result<Vec<MediaStream>, GuestError>, PluginError> {
        let mut store = self.store(self.capabilities());
        let instance = self.instantiate(&mut store).await?;
        instance
            .anistream_provider_provider()
            .call_resolve(&mut store, id, episode, translation)
            .await
            .map_err(|e| PluginError::from_call(&self.manifest.id, self.limits.deadline, &e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deadline_maps_to_a_failover_worthy_error() {
        // A hung plugin is broken, not empty: it must count against health and trigger failover,
        // exactly like a native provider that failed to parse.
        let error = PluginError::Deadline { plugin: "p".into(), limit: Duration::from_secs(1) };
        let mapped = error.as_provider_error();
        assert!(matches!(mapped, ProviderError::Parse(_)));
        assert!(
            mapped.should_failover(),
            "a timed-out plugin must fail over to the next source"
        );
    }

    #[test]
    fn a_trap_maps_to_a_failover_worthy_error() {
        let error = PluginError::Trap { plugin: "p".into(), message: "unreachable".into() };
        assert!(error.as_provider_error().should_failover());
    }

    #[test]
    fn the_epoch_tick_gives_the_deadline_real_granularity() {
        // The deadline is expressed in ticks, so a tick coarser than the deadline would round to
        // one and cut every call short.
        let limits = Limits::default();
        let ticks = limits.deadline.as_millis() / EPOCH_TICK.as_millis();
        assert!(ticks >= 10, "only {ticks} ticks of granularity");
    }

    #[tokio::test]
    async fn a_missing_plugin_file_is_a_load_error_not_a_panic() {
        let host = PluginHost::new(Limits::default(), None).unwrap();
        let error = host.load("/nonexistent/nope.wasm").await.unwrap_err();
        assert!(matches!(error, PluginError::Load { .. }));
    }

    #[tokio::test]
    async fn a_file_that_is_not_a_component_is_rejected() {
        // A plugin directory is user-writable, so anything at all can appear in it.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("junk.wasm");
        std::fs::write(&path, b"this is not webassembly").unwrap();

        let host = PluginHost::new(Limits::default(), None).unwrap();
        assert!(matches!(host.load(&path).await, Err(PluginError::Load { .. })));
    }

    #[tokio::test]
    async fn a_missing_plugin_directory_is_empty_rather_than_an_error() {
        // Having no plugins is the normal case.
        let host = PluginHost::new(Limits::default(), None).unwrap();
        assert!(host.load_dir("/nonexistent/plugins").await.is_empty());
    }

    #[tokio::test]
    async fn non_wasm_files_in_the_plugin_directory_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("README.md"), b"notes").unwrap();
        std::fs::write(dir.path().join("config.toml"), b"x = 1").unwrap();

        let host = PluginHost::new(Limits::default(), None).unwrap();
        assert!(host.load_dir(dir.path()).await.is_empty());
    }
}
