//! Watch parties: a client for the Syncplay protocol that keeps one player with a room of others.
//!
//! The protocol is JSON lines over TCP, optionally upgraded to TLS on the same port. This crate
//! speaks the part of it a viewer needs (joining a room, its members and their ready state, the
//! room's position and pause, chat coming in) and turns it into what a front end's player is to
//! do. It knows no player and no engine: [`client::Party`] takes the player's reports and gives
//! back [`sync::PlayerAction`]s and [`notice::Notice`]s, so every front end's own player can be
//! driven by it.
//!
//! - [`wire`]: the messages, built and read.
//! - [`ping`]: the round trip, and how far a position from the room is moved on for it.
//! - [`sync`]: the rules that keep a player with the room, pure and clocked by the caller.
//! - [`notice`]: what the room says, in words.
//! - [`client`]: the connection, reconnecting, and the members.
//! - `fake` (feature `testing`): a server on the loopback interface for tests and Fixtures builds.
//!
//! See `docs/party-protocol.md` for the protocol as this client speaks it.

pub mod client;
#[cfg(any(test, feature = "testing"))]
pub mod fake;
pub mod notice;
pub mod ping;
pub mod sync;
pub mod wire;

pub use client::{
    BadServer, DEFAULT_PORT, Member, Party, PartyEvent, PartyOptions, Retry, Server, Tls,
};
pub use notice::{Notice, NoticeKind};
pub use sync::{PlayerAction, Sample};
pub use wire::FileInfo;

#[cfg(test)]
mod tests;
