//! The torrent source.
//!
//! anistream provides the *transport* — a librqbit session behind the VPN guard, streaming
//! over loopback HTTP — and no index of what to fetch. The indexer is a URL the user
//! supplies; see [`indexer`] and [`curation`]. Nothing is contacted until one is configured.
//!
//! Structurally different from a web source, and that is the point: it fails for unrelated
//! reasons, so the two are unlikely to be down together.
//!
//! Gated by [`crate::vpn::VpnGuard`]. The provider reports itself unavailable — and is
//! therefore never contacted — until the guard passes.

pub mod curation;
pub mod http;
pub mod indexer;
pub mod provider;
pub mod release;
pub mod session;

pub use http::{StreamServer, StreamSource};
pub use provider::TorrentProvider;
pub use release::Release;
pub use session::TorrentSession;
