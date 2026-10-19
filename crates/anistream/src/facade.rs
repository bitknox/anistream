//! The engine, under the name the binary, its examples and its tests knew it by before it moved
//! into its own crate.
//!
//! Temporary: `tools/refactor/crate_paths.sh` points every caller at `anistream_engine`, and this
//! goes, with the library target, once none is left.

pub use anistream_engine::*;
