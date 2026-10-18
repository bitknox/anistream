//! Plugin stores: sharing provider plugins with people you invite.
//!
//! A store is a small server someone runs. Its owner sends invite links; a link is spent once for
//! a per-device token; members fetch a signed plugin list and install from it.
//! `docs/plugin-stores.md` describes the protocol. The rules themselves live in
//! `anistream-store-core`, which also runs in Workers and serverless functions; this crate is the
//! native side:
//!
//! - [`client`] talks to a store and verifies everything that comes back,
//! - [`local`] remembers what this machine has joined and installed,
//! - [`install`] is the gate between a download and the plugin directory,
//! - [`server`] runs a store on SQLite and a directory, or on Postgres.

pub mod client;
pub mod install;
pub mod local;
pub mod server;

pub use anistream_store_core::{crypto, inspect, invite, manifest, protocol};

pub use client::{ClientError, StoreClient};
pub use inspect::{Inspector, PluginInfo};
pub use invite::{Invite, InviteError};
pub use local::{Joined, LocalState, PluginStatus};
