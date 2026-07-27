//! HTTP layer.
//!
//! Two clients, chosen per host. Most of what anistream talks to — AniList, its image CDN,
//! `raw.githubusercontent.com`, the mapping datasets — is a plain documented API, and those
//! use a plain [`reqwest`] client.
//!
//! The second client goes through [`wreq`], which reproduces a real browser's TLS and HTTP/2
//! handshake. Some hosts serve different responses depending on the client that connects, and
//! matching a browser's handshake is what makes their responses parseable. It is slower to set
//! up, so it is opt-in per host rather than the default.

pub mod client;
pub mod fetch;
pub mod ratelimit;

pub use client::{HttpClient, NetError, Profile};
pub use fetch::{Conditional, ConditionalResponse};
pub use ratelimit::RateLimiter;

pub type Result<T> = std::result::Result<T, NetError>;
