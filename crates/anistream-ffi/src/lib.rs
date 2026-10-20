//! The engine as a native front end holds it, through the bindings UniFFI generates for Swift and
//! Kotlin: started, sent commands, read for what happened, and shut down.
//!
//! [`FfiEngine`] is the [`Engine`](anistream_engine::engine::Engine) with what a foreign caller
//! cannot supply itself. It runs on a multi-thread tokio runtime of its own, since a native app
//! has none. It opens the config and the database from the directories the app passes, and it
//! takes the app's [`FfiHost`] as the engine's host. The engine's shape carries over unchanged:
//! [`FfiEngine::send`] only puts a command on a channel, so the app's main thread can call it, and
//! events are pulled with [`FfiEngine::next_events`], an async call the bindings turn into a Swift
//! `async` function or a Kotlin `suspend` one.
//!
//! The surface is narrow for now. The model's own `Command` and `Event`, and the rows they carry,
//! cannot cross yet: they hold tuples, borrowed strings, `Result` fields and the core's newtypes.
//! So this crate has small types of its own in [`protocol`] and maps them to and from the
//! model's: two commands, the events a search and the Continue row answer with, and every other
//! event by its name alone. P6 of the engine extraction reshapes the model's types so they can
//! cross, and widens these to the whole protocol. Each command or event that starts crossing is a
//! new variant, and the variants here keep their meaning.

uniffi::setup_scaffolding!();

mod engine;
mod host;
pub mod protocol;
#[cfg(feature = "testing")]
mod testing;

pub use engine::FfiEngine;
pub use host::FfiHost;
pub use protocol::{FfiCommand, FfiContent, FfiDirs, FfiError, FfiEvent, FfiTitle, FfiView};
