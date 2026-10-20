//! The lines the keyboard types into: Search's field and every prompt's, each a [`TextField`]
//! named by its [`Field`], and what decides whether a key is typed or acted on.

use super::App;
use crate::nav::{Focus, Overlay, Section, StageView};

/// A line the keyboard types into: a prompt's, or Search's own field.
///
/// Named by what it holds rather than where it is kept, so what reads one, a test setting the
/// stage among them, does not change when the buffers do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Search's query.
    Search,
    /// What narrows the command palette.
    Palette,
    /// What a manual match searches for.
    ManualQuery,
    /// The episodes a download range asks for: `4`, `1-12`, `7-`.
    Range,
    /// The value a text setting is being given.
    Setting,
    /// Who signs in to Kitsu.
    KitsuUser,
    /// Their password, drawn masked.
    KitsuPassword,
    /// An invite link being pasted.
    JoinLink,
    /// Who an invite is for.
    InviteName,
}

impl Field {
    pub const ALL: [Self; 9] = [
        Self::Search,
        Self::Palette,
        Self::ManualQuery,
        Self::Range,
        Self::Setting,
        Self::KitsuUser,
        Self::KitsuPassword,
        Self::JoinLink,
        Self::InviteName,
    ];

    /// Whether `ch` goes into this field as it stands: a field that takes only some of what
    /// can be typed says so by not taking the rest.
    fn takes(self, field: &TextField, ch: char) -> bool {
        match self {
            // A range is digits and one dash; letting anything else in would make the parse
            // failure the user's puzzle instead of the input's boundary.
            Self::Range => ch.is_ascii_digit() || ch == '-',
            // The server takes up to 64; stopping here says so before it is refused.
            Self::InviteName => field.len() < 64,
            _ => true,
        }
    }

    /// Whether the field is drawn one dot to a character rather than as typed. The screen can
    /// be shared, streamed or screenshotted, and a password never shows in it.
    pub const fn masked(self) -> bool {
        matches!(self, Self::KitsuPassword)
    }
}

/// A line of text typed at the keyboard.
///
/// Typed at its end and taken back from its end, which is all any field here does: there is no
/// caret to move, so the end is where the next character goes and where the screen draws one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextField {
    text: String,
}

impl TextField {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// How many characters it holds: what a masked field draws, a dot for each.
    pub fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// A character typed, onto the end.
    pub fn insert(&mut self, ch: char) {
        self.text.push(ch);
    }

    /// Backspace: the last character off, if there is one.
    pub fn backspace(&mut self) {
        self.text.pop();
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    /// Hold `text` instead, as if it had been typed: a prompt opened on a value to edit.
    pub fn set(&mut self, text: String) {
        self.text = text;
    }

    /// What it holds, leaving it empty: for a value that must not stay behind once it is used.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.text)
    }
}

impl App {
    /// Whether keystrokes should be treated as text rather than bindings.
    pub fn is_typing(&self) -> bool {
        self.nav.overlay().is_some_and(Overlay::takes_text_input)
            || (self.nav.section() == Section::Search
                && self.nav.focus() == Focus::Stage
                && !self.nav.has_overlay()
                && !self.search_in_results
                // Only the search section's *own* view is a text field. Without this, opening a
                // result and then typing edited the query behind the title you were looking at,
                // and `Esc` left the field instead of going back — two different surprises from
                // one missing condition.
                && matches!(self.nav.current(), StageView::Section(_)))
    }

    /// The field the keyboard types into now: the prompt on top, or else Search's own field.
    ///
    /// The prompt first and the section after, whatever has focus, as typing has always gone:
    /// the preview and the tests type into Search's field without focusing it, and the keys
    /// only type at all while [`Self::is_typing`] says so.
    pub fn focused_field(&self) -> Option<Field> {
        Some(match self.nav.overlay() {
            Some(Overlay::CommandPalette) => Field::Palette,
            Some(Overlay::ManualQuery) => Field::ManualQuery,
            Some(Overlay::DownloadRange) => Field::Range,
            Some(Overlay::EditSetting) => Field::Setting,
            Some(Overlay::KitsuUsername) => Field::KitsuUser,
            Some(Overlay::KitsuPassword) => Field::KitsuPassword,
            Some(Overlay::JoinStore) => Field::JoinLink,
            Some(Overlay::InviteName) => Field::InviteName,
            _ if self.nav.section() == Section::Search => Field::Search,
            _ => return None,
        })
    }

    /// Feed a character to whichever text field is active.
    pub fn type_char(&mut self, ch: char) {
        let Some(field) = self.focused_field() else { return };
        if field.takes(self.field(field), ch) {
            self.field_mut(field).insert(ch);
        }
        if field == Field::Palette {
            // Filtering reorders the matches, so the old index would point at an unrelated
            // action — and the top hit is what a fuzzy filter is for.
            self.cursors.overlay.selected = 0;
        }
    }

    pub fn backspace(&mut self) {
        let Some(field) = self.focused_field() else { return };
        self.field_mut(field).backspace();
        if field == Field::Palette {
            self.cursors.overlay.selected = 0;
        }
    }

    /// Esc in a field: the prompt dismissed, or the field left where there is none, and nothing
    /// typed into a sign-in or an invite kept.
    pub(super) fn dismiss_field(&mut self) {
        let credentials_open =
            matches!(self.nav.overlay(), Some(Overlay::KitsuUsername | Overlay::KitsuPassword));
        if !self.nav.close_overlay() {
            // Nothing to dismiss — leave the field rather than trapping the user
            // in it.
            self.nav.focus_rail();
        }
        if credentials_open {
            // A dismissed sign-in leaves nothing behind to resurface later.
            self.field_mut(Field::KitsuUser).clear();
            self.field_mut(Field::KitsuPassword).clear();
        }
        // An invite link is a one-time credential; a dismissed prompt forgets it.
        self.field_mut(Field::JoinLink).clear();
        self.field_mut(Field::InviteName).clear();
    }

    /// What `field` holds.
    pub fn text(&self, field: Field) -> &str {
        self.field(field).as_str()
    }

    /// `field` holding `text`, as if it had been typed.
    pub fn set_text(&mut self, field: Field, text: String) {
        self.field_mut(field).set(text);
    }

    /// The line `field` is.
    pub fn field(&self, field: Field) -> &TextField {
        &self.texts[field as usize]
    }

    pub(super) fn field_mut(&mut self, field: Field) -> &mut TextField {
        &mut self.texts[field as usize]
    }

    /// Feed pasted text to whichever field is active. Control characters are dropped, so the
    /// newline a copy brings along does not arrive as Enter halfway through a link.
    pub fn paste(&mut self, text: &str) {
        if !self.is_typing() {
            return;
        }
        for ch in text.chars().filter(|c| !c.is_control()) {
            self.type_char(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn a_paste_outside_a_text_field_does_nothing() {
        let mut a = at_stores(Vec::new());
        a.paste("dddd");
        assert!(a.text(Field::JoinLink).is_empty());
        assert!(a.toasts.is_empty(), "no stray bindings fired");
    }

    #[test]
    fn typing_suppresses_single_key_bindings() {
        // Otherwise "d" in a search box would trigger a download.
        let mut a = app();
        a.go_to_section(Section::Search);
        assert!(a.is_typing());

        a.handle(Action::Download, 10);
        assert!(a.toasts.is_empty(), "a binding fired while typing");

        a.type_char('f');
        a.type_char('r');
        assert_eq!(a.text(Field::Search), "fr");
    }

    #[test]
    fn the_command_palette_captures_typing_over_the_search_box() {
        let mut a = app();
        a.go_to_section(Section::Search);
        a.handle(Action::CommandPalette, 10);
        a.type_char('s');
        assert_eq!(a.text(Field::Palette), "s");
        assert!(a.text(Field::Search).is_empty(), "text went to the wrong field");
    }

    #[test]
    fn escape_while_typing_closes_the_overlay_rather_than_navigating() {
        let mut a = app();
        a.handle(Action::CommandPalette, 10);
        a.handle(Action::Back, 10);
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn backspace_edits_the_active_field_only() {
        let mut a = app();
        a.go_to_section(Section::Search);
        a.type_char('a');
        a.type_char('b');
        a.backspace();
        assert_eq!(a.text(Field::Search), "a");
        a.backspace();
        a.backspace();
        assert_eq!(a.text(Field::Search), "", "must not underflow");
    }

    /// Each field takes what it always took and only that: a range its digits and dashes, an
    /// invite name up to the server's 64 characters, counted as characters, and the rest
    /// whatever is typed. Backspace takes the last one back, in the field on top.
    #[test]
    fn each_field_takes_what_it_always_took() {
        let mut a = app();
        a.nav.open_overlay(Overlay::DownloadRange);
        "1a-b2 ".chars().for_each(|c| a.type_char(c));
        assert_eq!(a.text(Field::Range), "1-2");
        a.nav.close_overlay();

        a.nav.open_overlay(Overlay::InviteName);
        "é".repeat(70).chars().for_each(|c| a.type_char(c));
        assert_eq!(a.text(Field::InviteName), "é".repeat(64));
        a.nav.close_overlay();

        a.nav.open_overlay(Overlay::ManualQuery);
        "a b-?".chars().for_each(|c| a.type_char(c));
        assert_eq!(a.text(Field::ManualQuery), "a b-?");
        for _ in 0..7 {
            a.backspace();
        }
        assert_eq!(a.text(Field::ManualQuery), "");
        assert_eq!(a.text(Field::Range), "1-2", "the fields not on top are left alone");
        assert_eq!(a.text(Field::InviteName).chars().count(), 64);
    }
}
