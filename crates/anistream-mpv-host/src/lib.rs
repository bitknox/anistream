//! libmpv behind the engine's hosted player protocol, for the desktop apps: the Rust twin of the
//! Android app's JNI shim and player model, and of the Apple app's `MpvClient`.
//!
//! - [`ops`]: what each `PlayerOp` is on mpv, and the properties the engine reads. Pure.
//! - [`log`]: mpv's log lines with every address cut to its host. Pure.
//! - `player` (feature `libmpv`): one mpv core driven by the ops, reporting back.
//! - `render` (feature `libmpv`): the video drawn into the app's OpenGL framebuffer through
//!   libmpv's render API, for a `GtkGLArea` or any other GL widget.
//!
//! The crate knows no toolkit: the app hands in a GL function loader, its native display and
//! a sink for what the player says, and moves that to its own main thread.

pub mod log;
mod node;
pub mod ops;
mod sys;

#[cfg(feature = "libmpv")]
pub mod mpv;
#[cfg(feature = "libmpv")]
pub mod player;
#[cfg(feature = "libmpv")]
pub mod render;

#[cfg(feature = "libmpv")]
pub use crate::{
    mpv::{Mpv, MpvError},
    player::{HostEvent, Player, PlayerOptions, PlayerState, Sink},
    render::{GetProcAddress, GlRenderer, NativeDisplay},
};
