//! The terminal the interface runs in, as the binary drives it: the terminal's state and the
//! signals that end it, and focus handed back to it after an external player closes.
//!
//! Terminal code is this front end's alone, so it lives in the binary, apart from the work that
//! other front ends are to share.

pub mod focus;
pub mod terminal;
