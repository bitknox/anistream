//! Theme: the "Obi & Silk" palette, plus the glyph vocabulary the layout is built from.

pub mod color;
pub mod detect;
pub mod glyph;
pub mod palette;

pub use color::{AA_NORMAL, Depth, Rgb, contrast_ratio, detect_depth, set_depth};
pub use detect::{resolve, resolve_with};
pub use palette::{Palette, Role, Variant, player_theme};
