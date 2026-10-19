//! What anistream's engine and its front ends say to each other.
//!
//! Every request a front end can make and everything it is told back, as plain values: the
//! commands it sends, the events it receives, the rows those carry, and the words the app uses for
//! them. The terminal is one front end; a native app on the same engine is meant to be another,
//! and each renders the same values its own way. So nothing here knows how it is shown.
//!
//! What belongs here:
//! - data that crosses between the engine and a front end;
//! - the pure rules over that data, such as where a title's progress is counted against;
//! - the words for it that every front end would otherwise write again: a count and its noun, a
//!   size at human scale, a countdown.
//!
//! What never does:
//! - I/O or a runtime: no tokio, no files, no network, and no clock read here. A moment arrives
//!   as a value, a span or a Unix time, and whoever draws it counts it down.
//! - Anything about a terminal: no ratatui or crossterm, no glyphs, no lifetimes counted in
//!   frames, no keys. A front end's view state stays in that front end.
//! - The engine's own state, its services, or how it decides.
//!
//! The `ffi` feature derives UniFFI records and enums on the types, for a native front end's
//! bindings. Not every type can cross yet, so CI builds it to report what cannot, without failing.

pub mod catalog;
pub mod command;
pub mod downloads;
pub mod episodes;
pub mod event;
pub mod jobs;
pub mod notice;
pub mod playback;
pub mod sources;
pub mod stores;
pub mod sync;
pub mod wording;

// What the types' UniFFI derives register with, under the `ffi` feature only. It compiles under the
// workspace's `unsafe_code = "forbid"`: the lint is not reported inside another crate's macros,
// and every unsafe block and exported symbol here comes out of UniFFI's.
#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();
