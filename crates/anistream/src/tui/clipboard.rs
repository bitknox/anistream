//! The terminal's clipboard, as far as a terminal will share it.

use std::io;

/// Ask the terminal to put `text` on the clipboard (OSC 52). Returns whether the request was
/// written; whether the terminal honours it is up to its settings, which is why the link is also
/// shown in full.
pub fn copy_to_clipboard(out: &mut impl io::Write, text: &str) -> bool {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(text);
    write!(out, "\x1b]52;c;{encoded}\x07").and_then(|()| out.flush()).is_ok()
}
