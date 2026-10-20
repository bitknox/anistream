//! One key binding: the `ctrl+k` notation `[keys]` is written in, how it reads in the help
//! overlay, and the keystroke a terminal reports, normalised so the tables can look it up.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A parsed key binding.
///
/// Ordering is implemented by hand because crossterm's `KeyCode` and `KeyModifiers` are not
/// `Ord`. It is worth the few lines: a `BTreeMap` gives the help overlay and the command
/// palette a stable, reproducible order, whereas a hash map would shuffle them between runs
/// and make the docs feel arbitrary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Binding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl Binding {
    /// Sortable projection: plain keys before modified ones, then by key.
    fn sort_key(&self) -> (u8, u32, char) {
        let (kind, value, ch) = match self.code {
            KeyCode::Char(c) => (0u8, u32::from(c), c),
            KeyCode::Enter => (1, 0, '\0'),
            KeyCode::Esc => (1, 1, '\0'),
            KeyCode::Tab => (1, 2, '\0'),
            KeyCode::Backspace => (1, 3, '\0'),
            KeyCode::Up => (2, 0, '\0'),
            KeyCode::Down => (2, 1, '\0'),
            KeyCode::Left => (2, 2, '\0'),
            KeyCode::Right => (2, 3, '\0'),
            KeyCode::PageUp => (2, 4, '\0'),
            KeyCode::PageDown => (2, 5, '\0'),
            KeyCode::Home => (2, 6, '\0'),
            KeyCode::End => (2, 7, '\0'),
            KeyCode::F(n) => (3, u32::from(n), '\0'),
            _ => (4, 0, '\0'),
        };
        // Modifier bits lead so unmodified keys sort first, which reads better in help.
        (self.modifiers.bits().count_ones() as u8, (kind as u32) << 24 | value, ch)
    }
}

impl PartialOrd for Binding {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Binding {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sort_key()
            .cmp(&other.sort_key())
            .then_with(|| self.modifiers.bits().cmp(&other.modifiers.bits()))
    }
}

impl Binding {
    pub const fn plain(code: KeyCode) -> Self {
        Self { code, modifiers: KeyModifiers::NONE }
    }

    pub const fn ctrl(code: KeyCode) -> Self {
        Self { code, modifiers: KeyModifiers::CONTROL }
    }

    /// Parse a config string like `"ctrl+k"`, `"esc"`, `"?"`.
    pub fn parse(spec: &str) -> Option<Self> {
        let spec = spec.trim();
        let (modifiers, key) = match spec.rsplit_once('+') {
            Some((mods, key)) => {
                let mut m = KeyModifiers::NONE;
                for part in mods.split('+') {
                    match part.trim().to_ascii_lowercase().as_str() {
                        "ctrl" | "control" => m |= KeyModifiers::CONTROL,
                        "alt" | "meta" => m |= KeyModifiers::ALT,
                        "shift" => m |= KeyModifiers::SHIFT,
                        _ => return None,
                    }
                }
                (m, key.trim())
            }
            None => (KeyModifiers::NONE, spec),
        };

        let code = match key.to_ascii_lowercase().as_str() {
            "esc" | "escape" => KeyCode::Esc,
            "enter" | "return" => KeyCode::Enter,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "pageup" => KeyCode::PageUp,
            "pagedown" => KeyCode::PageDown,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "backspace" => KeyCode::Backspace,
            _ => {
                let mut chars = key.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                // Preserve the original case: `G` and `g` are different bindings, and
                // lowercasing here would silently merge them.
                KeyCode::Char(key.chars().next().unwrap_or(ch))
            }
        };
        Some(Self { code, modifiers })
    }

    /// Render for display, matching the notation used in config.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if self.modifiers.contains(KeyModifiers::CONTROL) {
            out.push_str("ctrl+");
        }
        if self.modifiers.contains(KeyModifiers::ALT) {
            out.push_str("alt+");
        }
        match self.code {
            KeyCode::Char(' ') => out.push_str("space"),
            KeyCode::Char(c) => out.push(c),
            KeyCode::Esc => out.push_str("esc"),
            KeyCode::Enter => out.push('↵'),
            KeyCode::Tab => out.push_str("tab"),
            KeyCode::Up => out.push('↑'),
            KeyCode::Down => out.push('↓'),
            KeyCode::Left => out.push('←'),
            KeyCode::Right => out.push('→'),
            KeyCode::PageUp => out.push_str("pgup"),
            KeyCode::PageDown => out.push_str("pgdn"),
            other => out.push_str(&format!("{other:?}").to_lowercase()),
        }
        out
    }

    pub(super) fn from_event(event: KeyEvent) -> Self {
        // Crossterm reports SHIFT alongside uppercase characters on some platforms and not
        // others. Normalising it away keeps `G` matching whether or not the flag arrives.
        let modifiers = if matches!(event.code, KeyCode::Char(c) if c.is_uppercase()) {
            event.modifiers & !KeyModifiers::SHIFT
        } else {
            event.modifiers
        };
        Self { code: event.code, modifiers }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{Action, Context, Keymap};

    #[test]
    fn uppercase_resolves_whether_or_not_shift_is_reported() {
        // Crossterm is inconsistent about the SHIFT flag across platforms.
        let keymap = Keymap::new();
        let without = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE);
        let with = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(keymap.action_for(without, Context::Browsing), Some(Action::Bottom));
        assert_eq!(keymap.action_for(with, Context::Browsing), Some(Action::Bottom));
    }

    #[test]
    fn binding_specs_parse_and_render_round_trip() {
        for spec in ["ctrl+k", "esc", "tab", "space", "?", "G"] {
            let parsed = Binding::parse(spec).unwrap_or_else(|| panic!("failed on {spec:?}"));
            let rendered = parsed.render();
            assert_eq!(
                Binding::parse(&rendered),
                Some(parsed),
                "{spec:?} rendered as {rendered:?} and did not parse back"
            );
        }
    }

    #[test]
    fn nonsense_specs_are_rejected_rather_than_silently_accepted() {
        assert!(Binding::parse("hyper+x").is_none());
        assert!(Binding::parse("notakey").is_none());
    }
}
