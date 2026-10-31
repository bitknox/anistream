//! The overlays that are not text fields: help, the logs, the command palette, and the list
//! overlays' movement and Enter.

use anistream_model::{catalog::LibrarySegment, command::Command};

use super::{App, Field, cycle_search_filter, search_filter_labels};
use crate::{
    keymap::{Action, Context},
    nav::{Overlay, StageView},
};

/// Palette rows shown at once. Also the number the arrows can reach, so the two cannot drift.
pub const PALETTE_ROWS: usize = 12;

/// What a frame found out that only the renderer can know: how the overlay on screen was laid
/// out at the size it was drawn at.
///
/// The renderer takes the App as it is and changes none of it; what it learns comes back as
/// this, and whatever drew the frame hands it to [`App::took_frame`] before the next key is read
/// or the next frame drawn. A field is `None` when the frame drew nothing that decides it, and
/// the App keeps what an earlier frame said.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FrameReport {
    /// The first row a list overlay's window showed, when an overlay other than Help was drawn.
    pub overlay_offset: Option<usize>,
    /// How many pages the help table ran to, when it was drawn.
    pub help_pages: Option<usize>,
}

impl App {
    /// What an open, non-text overlay does with each action. Everything not listed is
    /// swallowed.
    ///
    /// Every overlay: Back closes it, Quit quits, and Help toggles the key reference on top —
    /// stacked, so Esc returns to the question it was opened from. Beyond that there are two
    /// kinds. **Help and Logs** are references with nothing to lose by leaving, so the keys
    /// that go somewhere — the palette, search, the section numbers, Logs — close them and go,
    /// save the section numbers over a playing Now Playing; their movement keys scroll. **Every other overlay asks a question**, and only its own
    /// keys work until it is answered or dismissed: a `5` in the rating picker must not jump to
    /// Downloads.
    pub(super) fn handle_in_overlay(
        &mut self,
        overlay: &Overlay,
        action: Action,
        visible_rows: usize,
    ) -> Option<Command> {
        match action {
            Action::Quit => {
                self.should_quit = true;
                return None;
            }
            Action::Back => {
                self.nav.close_overlay();
                return None;
            }
            Action::Help if *overlay == Overlay::Help => {
                self.nav.close_overlay();
                return None;
            }
            Action::Help => {
                self.open_help();
                return None;
            }
            _ => {}
        }

        let reference = matches!(overlay, Overlay::Help | Overlay::Logs);
        if reference {
            match action {
                // The same key again closes it, as `?` does for Help.
                Action::ShowLogs if *overlay == Overlay::Logs => {
                    self.nav.close_overlay();
                    return None;
                }
                // Over Now Playing the digits belong to the player — `9` and `0` are its
                // volume — so behind Help or Logs a section number does nothing, rather than
                // closing the reference and leaving the player for another screen.
                Action::JumpSection(_)
                    if self.playing.is_some()
                        && matches!(self.nav.current(), StageView::NowPlaying) =>
                {
                    return None;
                }
                Action::CommandPalette
                | Action::FocusSearch
                | Action::FilterSearch
                | Action::JumpSection(_)
                | Action::ShowLogs
                | Action::ShowAccounts
                | Action::ShowConflicts => {
                    // Replaced rather than stacked: the palette runs what it picks against the
                    // screen, and anything left open would swallow that.
                    self.nav.close_all_overlays();
                    return self.perform(action, visible_rows);
                }
                _ => {}
            }
        }

        if *overlay == Overlay::Help {
            let last = self.help_pages.saturating_sub(1);
            self.help_page = match action {
                Action::Down | Action::PageDown => (self.help_page + 1).min(last),
                Action::Up | Action::PageUp => self.help_page.saturating_sub(1),
                Action::Top => 0,
                Action::Bottom => last,
                _ => self.help_page,
            };
            return None;
        }

        // The store confirmations and the invite link are questions with one answer key: a
        // stray `j` must not move the cursor off the member a "remove" is about to be applied to.
        match (overlay, action) {
            (Overlay::StoreConfirm, Action::Open) => return self.confirm_store_action(),
            (Overlay::InviteLink, Action::Open) => {
                self.nav.close_overlay();
                self.invite_link = None;
                return None;
            }
            (Overlay::OpenLink, Action::Open) => {
                self.nav.close_overlay();
                return None;
            }
            (Overlay::StoreConfirm | Overlay::InviteLink | Overlay::OpenLink, _) => {
                return None;
            }
            _ => {}
        }

        // A list overlay owns movement and Enter. Without this, arrowing through the Accounts
        // list would scroll the library behind it instead.
        let len = self.overlay_len();
        let page = visible_rows.saturating_sub(4).max(1);
        match action {
            Action::Down => {
                self.cursors.overlay.selected =
                    (self.cursors.overlay.selected + 1).min(len.saturating_sub(1));
            }
            Action::Up => {
                self.cursors.overlay.selected = self.cursors.overlay.selected.saturating_sub(1)
            }
            Action::PageDown => {
                self.cursors.overlay.selected =
                    (self.cursors.overlay.selected + page).min(len.saturating_sub(1));
            }
            Action::PageUp => {
                self.cursors.overlay.selected =
                    self.cursors.overlay.selected.saturating_sub(page);
            }
            Action::Top => self.cursors.overlay.selected = 0,
            Action::Bottom => self.cursors.overlay.selected = len.saturating_sub(1),
            Action::Open if len > 0 => return self.confirm_overlay(visible_rows),
            // The filters overlay edits in place: the vertical axis picks a row and the
            // horizontal one walks its values, the same grammar as the Settings screen.
            Action::Left if *overlay == Overlay::SearchFilters => {
                cycle_search_filter(
                    &mut self.search_filter,
                    self.cursors.overlay.selected,
                    false,
                    &self.clock,
                );
            }
            Action::Right if *overlay == Overlay::SearchFilters => {
                cycle_search_filter(
                    &mut self.search_filter,
                    self.cursors.overlay.selected,
                    true,
                    &self.clock,
                );
            }
            _ => {}
        }
        None
    }

    /// Open the key reference at its first page.
    pub(super) fn open_help(&mut self) {
        self.help_page = 0;
        self.nav.open_overlay(Overlay::Help);
    }

    /// The Logs overlay, at its newest line.
    pub(super) fn show_logs(&mut self) {
        // Rendered newest-first, so nought is the most recent line.
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Logs);
    }

    /// The command palette, with nothing typed into it and its first match highlighted.
    pub(super) fn open_palette(&mut self) {
        self.field_mut(Field::Palette).clear();
        // The overlays share one cursor, and each list overlay sets it as it opens. Left as it
        // was, the palette opened on whichever row Logs or the palette itself was left on, and
        // Enter ran that action rather than the top match. At nought the renderer puts the
        // window at the top as well.
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::CommandPalette);
    }

    /// Confirm the selection in whichever overlay is open.
    ///
    /// Overlays own their own selection index so closing one cannot disturb the list behind it.
    pub(super) fn confirm_overlay(&mut self, visible_rows: usize) -> Option<Command> {
        match self.nav.overlay() {
            Some(Overlay::CommandPalette) => self.run_palette_pick(visible_rows),
            Some(Overlay::Logs) => None,
            Some(Overlay::ListStatus) => self.confirm_list_status(),
            Some(Overlay::SearchFilters) => self.confirm_search_filters(),
            Some(Overlay::Rate) => self.confirm_rating(),
            Some(Overlay::Conflicts) => self.confirm_conflict(),
            Some(Overlay::SourceProvider) => self.confirm_source_provider(),
            Some(Overlay::SourceRelease) => self.confirm_source_release(),
            Some(Overlay::WatchOrder) => self.confirm_watch_order(),
            Some(Overlay::Recommendations) => self.confirm_recommendation(),
            Some(Overlay::Sources) => self.confirm_source(),
            Some(Overlay::Disambiguate) => self.confirm_match(),
            Some(Overlay::Accounts) => self.confirm_account(),
            Some(Overlay::PluginAction) => self.confirm_plugin_choice(),
            Some(Overlay::PublishPick) => self.confirm_publish(),
            _ => None,
        }
    }

    /// Rows the currently-open overlay is listing, so movement can be bounded.
    fn overlay_len(&self) -> usize {
        match self.nav.overlay() {
            Some(Overlay::ListStatus) => LibrarySegment::ALL.len(),
            Some(Overlay::Rate) => 10,
            Some(Overlay::SearchFilters) => search_filter_labels().len(),
            Some(Overlay::Conflicts) => self.conflicts.len() * 2,
            Some(Overlay::Accounts) => self.sync.len(),
            // The palette was missing from this list, which meant the arrows scrolled the list
            // behind it and Enter did nothing — so the app's stated discoverability mechanism
            // could filter actions but never run one.
            Some(Overlay::CommandPalette) => self.palette_matches().len(),
            Some(Overlay::Logs) => self.logs.len(),
            Some(Overlay::Disambiguate) => self.match_candidates.len(),
            Some(Overlay::Sources) => self.sources.len(),
            Some(Overlay::SourceProvider) => self.provider_choices().len(),
            Some(Overlay::SourceRelease) => self.release_choices().len(),
            Some(Overlay::WatchOrder) => {
                self.target().map_or(0, |e| super::watch_order(e).len())
            }
            Some(Overlay::Recommendations) => self.target().map_or(0, |e| e.recommended.len()),
            Some(Overlay::PluginAction) => self.plugin_choices().len(),
            Some(Overlay::PublishPick) => self.local_plugins.len(),
            _ => 0,
        }
    }

    /// Actions matching the current palette query, in the order they are rendered.
    ///
    /// One source of truth shared by movement, Enter and the renderer. Deriving the selectable
    /// set separately from the drawn set is how a palette ends up running the wrong entry.
    pub fn palette_matches(&self) -> Vec<(Action, String)> {
        // The keys the screen will read once the palette is closed, not the palette's own.
        let context = if self.playing.is_some() { Context::Playing } else { Context::Browsing };
        self.keymap
            .palette_entries(self.text(Field::Palette), context)
            .into_iter()
            .take(PALETTE_ROWS)
            .collect()
    }

    /// The palette's highlighted action, run against the screen.
    fn run_palette_pick(&mut self, visible_rows: usize) -> Option<Command> {
        let (action, _) = self.palette_matches().get(self.cursors.overlay.selected)?.clone();
        // Close first, then run: several palette actions open an overlay of their own,
        // and leaving the palette on the stack would bury them.
        self.nav.close_overlay();
        self.field_mut(Field::Palette).clear();
        self.cursors.overlay.selected = 0;
        self.perform(action, visible_rows)
    }

    /// Down in the palette, stopping at its last match.
    pub(super) fn palette_down(&mut self) {
        let last = self.palette_matches().len().saturating_sub(1);
        self.cursors.overlay.selected = (self.cursors.overlay.selected + 1).min(last);
    }

    /// The page of the key reference showing.
    pub fn help_page(&self) -> usize {
        self.help_page
    }

    /// How many pages the help table ran to when it was last drawn.
    pub fn help_pages(&self) -> usize {
        self.help_pages
    }

    /// The first row the list overlay showed when one was last drawn.
    pub fn overlay_offset(&self) -> usize {
        self.cursors.overlay.offset
    }

    /// Keep what the frame just drawn found out: see [`FrameReport`].
    pub fn took_frame(&mut self, report: FrameReport) {
        if let Some(offset) = report.overlay_offset {
            self.cursors.overlay.offset = offset;
        }
        if let Some(pages) = report.help_pages {
            self.help_pages = pages;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::fixtures::*;

    #[test]
    fn an_open_list_overlay_owns_movement() {
        // Without this, arrowing through Accounts would scroll the library behind it.
        let mut a = app();
        a.apply(a.content_here(entries(10)));
        a.apply(Event::Conflicts(vec![conflict("a"), conflict("b"), conflict("c")]));
        a.handle(Action::ShowConflicts, 20);

        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.overlay_cursor(), 2);
        assert_eq!(a.cursor(), 0, "the list behind the overlay moved");

        // And it cannot run off the end: three differences, two rows each.
        for _ in 0..5 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(a.overlay_cursor(), 5);
    }

    #[test]
    fn a_digit_behind_help_on_now_playing_stays_with_the_player() {
        // `9` is volume on Now Playing; behind Help it closed Help and went to Settings.
        let mut a = home_list(3);
        let _ = a.begin_playback(AnilistId::new(1), "1".into());
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        for overlay in [Action::Help, Action::ShowLogs] {
            a.handle(overlay, 5);
            let nine = KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE);
            assert_eq!(a.press(nine, 5), None);
            assert!(a.nav.has_overlay(), "{overlay:?} must stay open");
            assert_eq!(a.nav.current(), &StageView::NowPlaying);
            a.handle(Action::Back, 5);
        }
    }

    #[test]
    fn the_palette_can_be_moved_through_and_run() {
        // It could filter but never pick: the arrows moved the list behind it and Enter closed
        // it without running anything, so the app's discoverability mechanism was a dead end.
        let mut a = app();
        assert!(a.handle(Action::CommandPalette, 10).is_none());
        assert!(a.is_typing(), "the palette takes text");

        let first = a.palette_matches().first().cloned().expect("some actions match");
        a.handle(Action::Down, 10);
        assert_eq!(a.overlay_cursor(), 1, "down moves within the palette");
        a.handle(Action::Up, 10);
        assert_eq!(a.overlay_cursor(), 0);

        // Typing resets the cursor, since filtering reorders the matches under it.
        a.handle(Action::Down, 10);
        a.type_char('s');
        assert_eq!(a.overlay_cursor(), 0, "a filtered list must not keep a stale index");

        a.backspace();
        a.handle(Action::Open, 10);
        assert_ne!(
            a.nav.overlay(),
            Some(&Overlay::CommandPalette),
            "Enter must dismiss the palette"
        );
        assert!(
            a.text(Field::Palette).is_empty(),
            "the query must not persist into the next open"
        );
        // Whatever ran, it was the row under the cursor rather than nothing at all.
        assert_ne!(first.0, Action::CommandPalette, "sanity: the fixture is a real action");
    }

    #[test]
    fn the_palette_actually_performs_the_selected_action() {
        let mut a = app();
        a.handle(Action::CommandPalette, 10);
        a.set_text(Field::Palette, "keys".into());
        let matched = a.palette_matches();
        assert!(
            matched.iter().any(|(action, _)| *action == Action::Help),
            "expected the help action to match 'keys', got {matched:?}"
        );
        a.set_overlay_cursor(
            matched.iter().position(|(action, _)| *action == Action::Help).unwrap(),
        );
        a.handle(Action::Open, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Help), "the chosen action must run");
    }
}
