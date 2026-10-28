//! The test kit: what a test needs to stand anistream up with no network and no real HOME.
//!
//! - [`FakeAniList`]: AniList's GraphQL endpoint on loopback, answering the queries the app
//!   really sends from fixture JSON, recording every request, and failing on demand.
//! - [`fixtures`]: the invented titles those answers are made of, as values too, so a test that
//!   never touches the server can build the same screens.
//! - [`MetaKit`]: a real [`Meta`] wired to the fake and to an in-memory [`Store`].
//! - [`seed`]: history, progress, mappings and downloads written into a store.
//! - [`sources`]: a provider registry that knows the fixture titles' episodes and streams.
//! - [`assert_golden`]: compare text against a committed file, or rewrite it on request.
//! - [`art`]: the fixture titles' cover, banner and still art, which the fake serves on request.
//!
//! Development-only by construction: `publish = false`, used only as a dev-dependency, and
//! built below the TUI and the app crate so that both can test against it.

pub mod anilist;
pub mod art;
pub mod fixtures;
pub mod golden;
pub mod seed;
pub mod sources;

use anistream_core::config::NetworkConfig;
use anistream_meta::Meta;
use anistream_net::HttpClient;
use anistream_store::Store;

pub use anilist::{COMPLEXITY_LIMIT, Failure, FakeAniList, Operation, Request, complexity};
pub use golden::{BLESS_VAR, assert_golden, check_golden};

/// The AniList budget a test client gets, per minute.
///
/// High enough that the limiter never makes a test wait, and the limiter still runs: a test
/// goes through the same `acquire` the app does, which is the point of testing against it.
/// A [`Failure::RateLimited`] answer still holds later requests for as long as it says.
pub const TEST_RATE_LIMIT: u32 = 6_000;

/// An HTTP client for talking to the fake.
///
/// The app's own client, with a shorter timeout: nothing on loopback takes seconds, so a test
/// that is waiting that long has found a bug and should say so sooner than the default would.
///
/// Like every reqwest client it honours `HTTP_PROXY` and `ALL_PROXY`, and makes no exception
/// for loopback. Where one is set, `NO_PROXY` has to name `127.0.0.1` for the fake to be
/// reachable — as it already must for the app's other loopback tests.
pub fn http() -> HttpClient {
    HttpClient::new(&NetworkConfig { timeout_secs: 5, ..NetworkConfig::default() })
        .expect("the default network config builds a client")
}

/// A fresh, migrated in-memory store.
pub fn store() -> Store {
    Store::open_in_memory().expect("an in-memory store opens")
}

/// A fake AniList, a store, and a [`Meta`] over both: the metadata half of the app, whole.
pub struct MetaKit {
    pub server: FakeAniList,
    pub store: Store,
    pub meta: Meta,
}

impl MetaKit {
    /// With the fallback off, as `meta.fallback = false` has it: AniList, then the cache.
    pub async fn start() -> Self {
        let server = FakeAniList::start().await;
        let store = store();
        let meta = server.meta(&store);
        Self { server, store, meta }
    }

    /// With the fallback on, pointed at an empty Tenrai beside the fake (see
    /// [`FakeAniList::tenrai_endpoint`]), as the default config has it.
    pub async fn start_with_tenrai() -> Self {
        let server = FakeAniList::start().await;
        let store = store();
        let meta = server.meta_with_tenrai(&store);
        Self { server, store, meta }
    }
}
