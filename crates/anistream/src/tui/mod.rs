//! The terminal the interface runs in, as the binary drives it: the event loop, the terminal's
//! state and the signals that end it, its clipboard, focus handed back to it after an external
//! player closes, and the front end's half of the seam left to the engine side, which reads the
//! interface's state.
//!
//! Terminal code is this front end's alone, so it lives in the binary, apart from the work that
//! other front ends are to share.

pub mod clipboard;
pub mod focus;
pub mod run;
pub mod seams;
pub mod terminal;
