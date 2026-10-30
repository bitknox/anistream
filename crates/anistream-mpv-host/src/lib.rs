//! libmpv behind the engine's hosted player protocol, for the desktop apps: the Rust twin of the
//! Android app's JNI shim and player model, and of the Apple app's `MpvClient`.
//!
//! - [`ops`]: what each `PlayerOp` is on mpv, and the properties the engine reads. Pure.
//! - [`log`]: mpv's log lines with every address cut to its host. Pure.
//! - `player` (feature `libmpv`): one mpv core driven by the ops, reporting back.
//! - `render` (feature `libmpv`): the video drawn into the app's OpenGL framebuffer through
//!   libmpv's render API, for a `GtkGLArea` or any other GL widget.
//!
//! On Windows the picture takes the other way, [`VideoOutput::D3d11Composition`]: mpv presents
//! into a swapchain made for composition, and the app hands it to a XAML `SwapChainPanel`.
//!
//! The crate knows no toolkit: the app hands in a GL function loader, its native display and
//! a sink for what the player says, or binds the swapchain itself, and moves what the player
//! says to its own main thread.

#[cfg(any(feature = "libmpv", test))]
// Without libmpv only its tests read it.
#[cfg_attr(not(feature = "libmpv"), allow(dead_code))]
mod book;
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
    player::{
        HostEvent, Player, PlayerOptions, PlayerState, Sink, VideoOutput, composition_size,
    },
    render::{GetProcAddress, GlRenderer, NativeDisplay},
};
