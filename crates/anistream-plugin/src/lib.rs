//! WebAssembly component host for provider plugins.
//!
//! **Providers are pluggable across languages.** A `.wasm` component authored in Rust, Go
//! (TinyGo), TypeScript (jco) or Python (componentize-py) implements the same `provider` interface
//! and is indistinguishable from a native source at the registry boundary.
//!
//! The design decision that makes this work is in [`host`]: **the sandbox has no sockets, and the
//! host lends `fetch`.** Provider CDNs often sit behind Cloudflare, and passing its checks needs a
//! browser-shaped TLS/HTTP2 fingerprint — solved once, in `anistream-net`. If guests opened their
//! own connections each would have to solve it again, badly, and carry a TLS stack to do it. So a
//! plugin is a pure parser: it says what to request, the host requests it, the guest reads bytes.
//!
//! Everything follows from that. Plugins are kilobytes rather than megabytes. They inherit the
//! fingerprint and the rate limiter for free. And they cannot exfiltrate
//! anything, because [`sandbox::is_allowed`] is enforced host-side on every call.
//!
//! | Module | Job |
//! |---|---|
//! | [`sandbox`] | The allowlist and the resource ceilings. Pure policy, heavily tested. |
//! | [`host`] | The four capabilities a guest can reach, and their enforcement. |
//! | [`engine`] | wasmtime configuration: epoch interruption, memory limiter, empty WASI. |
//! | [`provider`] | A loaded plugin as an ordinary `Provider`. Deliberately boring. |
//!
//! `tests/conformance.rs` runs against two real compiled components — a reference plugin and a
//! deliberately hostile one that spins forever, allocates without bound, and tries to reach a host
//! it never declared.

pub mod engine;
pub mod host;
pub mod provider;
pub mod sandbox;

pub use engine::{LoadedPlugin, PluginError, PluginHost};

/// The HTTP client a host lends its plugins for `fetch`.
#[cfg(feature = "fetch")]
pub type HttpClient = anistream_net::HttpClient;

/// Without the `fetch` feature there is no client to lend, so there are none of these: a host
/// built that way loads plugins and asks them to describe themselves, and nothing more. That is
/// all a plugin store's server needs, and it leaves the browser-emulating network stack out of
/// its build.
#[cfg(not(feature = "fetch"))]
#[derive(Debug, Clone)]
pub enum HttpClient {}
pub use provider::{LoadedDir, WasmProvider, load_providers, load_providers_reporting};
pub use sandbox::Limits;
