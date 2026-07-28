//! The anistream terminal interface.
//!
//! Visual direction is "Obi & Silk": a borderless print-catalogue composition where
//! structure comes from hairlines and negative space, focus is marked by a single amber
//! obi bar, and the key visual is the hero. See [`theme`] for the palette and glyph
//! vocabulary that the rest of the UI is assembled from.

pub mod app;
pub mod eyecatch;
pub mod image;
pub mod keymap;
pub mod layout;
pub mod logo;
pub mod nav;
pub mod screens;
pub mod theme;
pub mod widgets;

pub use app::{
    MatchCandidate,
    App, ConflictRow, Content, Entry, LibrarySegment, NowPlaying, PlayerCommand, SyncState,
    Task, Toast, Update,
};
pub use eyecatch::Eyecatch;
pub use keymap::{Action, Binding, Keymap, Scope};
pub use nav::{Focus, Nav, Overlay, RailWidth, Section, StageView};
pub use theme::{Palette, Role, Variant};
