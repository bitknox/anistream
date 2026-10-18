//! Plugin stores, portable.
//!
//! Everything about a store that does not depend on where it runs: the protocol, the rules
//! ([`store`]), the routes ([`http`]), the admin page, and the formats a client checks (signed
//! indexes, invite links, plugin manifests). It compiles natively and to WebAssembly, so the
//! reference server, a Worker and a serverless function all run this code and differ only in
//! how they store things and receive requests:
//!
//! - [`db::Database`] — SQLite, Postgres, D1
//! - [`blobs::Blobs`] — a directory, S3-compatible storage, R2, a platform's blob store
//! - [`inspect::Inspector`] — a WebAssembly runtime to ask plugins what they are, where there is one
//!
//! `docs/plugin-stores.md` describes the protocol.

pub mod admin;
pub mod blobs;
pub mod crypto;
pub mod db;
pub mod http;
pub mod inspect;
pub mod invite;
pub mod manifest;
pub mod protocol;
pub mod store;

/// `Send + Sync` natively, nothing in WebAssembly. A Worker's storage handles are JavaScript
/// objects, which are neither, and run on one thread anyway.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSendSync: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> MaybeSendSync for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSendSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSendSync for T {}
