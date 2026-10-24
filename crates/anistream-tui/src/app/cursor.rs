//! Where each view's highlight is, and how the movement keys move it.
//!
//! One answer for every movement key on every screen: [`App::rows_here`] says which rows the
//! screen on top has, and [`App::move_cursor`] walks them, stopping at either end.

use anistream_model::{command::Command, settings::SettingKey};

use super::App;
use crate::{
    keymap::Action,
    nav::{Focus, Section, StageView},
};

/// The rows a screen's cursor walks. See [`App::rows_here`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    /// The section's list: `content`, and the section's cursor in `cursors`.
    List,
    Downloads,
    Accounts,
    Settings,
    /// The Providers table, in the order failover walks it.
    Providers,
    Episodes,
    Stores,
    StoreLines,
    Members,
}

/// Where a view's highlight is, and the first row its window shows.
///
/// The window is kept only where it should not follow the highlight on every move: the sections'
/// lists and the overlays'. A table that draws its window from the highlight leaves it at nought.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cursor {
    pub selected: usize,
    pub offset: usize,
}

/// Every view's cursor: each section's own view, the views pushed over them, and the overlays.
///
/// One for each, where the sections used to share one. A section is always entered at its top
/// — see [`App::go_to_section`] — so whichever section is on screen has its cursor where the
/// shared one was, and the rest wait to be entered afresh. The overlays share one between them,
/// apart from the screens': each list overlay, the palette among them, sets it as it opens.
#[derive(Debug, Clone, Default)]
pub struct Cursors {
    pub home: Cursor,
    pub calendar: Cursor,
    pub seasonal: Cursor,
    pub search: Cursor,
    pub library: Cursor,
    pub downloads: Cursor,
    pub providers: Cursor,
    pub accounts: Cursor,
    pub settings: Cursor,
    /// The episode table, kept apart from the list it was opened from so stepping back out of
    /// Episodes does not disturb where you were in the list.
    pub episodes: Cursor,
    /// The three store screens, so stepping back out of a store finds the store list where it
    /// was. One past the last store is the join row.
    pub stores: Cursor,
    pub store_lines: Cursor,
    pub members: Cursor,
    /// The list overlay on top: Accounts, Conflicts, the list status, the palette and the rest.
    ///
    /// Its window is the renderer's to place, the only place that knows how many rows fit: see
    /// [`FrameReport`]. Kept so the window moves only when the highlight would leave it, rather
    /// than recomputed from the highlight each frame and pinning it to the bottom edge on the
    /// way back up.
    ///
    /// [`FrameReport`]: super::FrameReport
    pub overlay: Cursor,
}

impl Cursors {
    /// The cursor of `section`'s own view: its list, or its table.
    pub fn section(&self, section: Section) -> &Cursor {
        match section {
            Section::Home => &self.home,
            Section::Calendar => &self.calendar,
            Section::Seasonal => &self.seasonal,
            Section::Search => &self.search,
            Section::Library => &self.library,
            Section::Downloads => &self.downloads,
            Section::Providers => &self.providers,
            Section::Accounts => &self.accounts,
            Section::Settings => &self.settings,
        }
    }

    pub fn section_mut(&mut self, section: Section) -> &mut Cursor {
        match section {
            Section::Home => &mut self.home,
            Section::Calendar => &mut self.calendar,
            Section::Seasonal => &mut self.seasonal,
            Section::Search => &mut self.search,
            Section::Library => &mut self.library,
            Section::Downloads => &mut self.downloads,
            Section::Providers => &mut self.providers,
            Section::Accounts => &mut self.accounts,
            Section::Settings => &mut self.settings,
        }
    }
}

impl App {
    /// How many rows of content fit, used for paging.
    pub fn page_size(&self, visible_rows: usize) -> usize {
        visible_rows.max(1)
    }

    fn move_selection(&mut self, delta: isize, visible_rows: usize) {
        let len = self.content.len();
        if len == 0 {
            return;
        }
        let cursor = self.cursors.section_mut(self.nav.section());
        let next = (cursor.selected as isize + delta).clamp(0, len as isize - 1) as usize;
        cursor.selected = next;

        // Keep the selection inside the viewport.
        if cursor.selected < cursor.offset {
            cursor.offset = cursor.selected;
        } else if visible_rows > 0 && cursor.selected >= cursor.offset + visible_rows {
            cursor.selected = cursor.selected.min(len - 1);
            cursor.offset = cursor.selected + 1 - visible_rows;
        }

        self.backfill_calendar_if_near_top();
        self.more_search_if_near_end();
    }

    /// Fit the list's window to `visible_rows` again, after the terminal changed size.
    ///
    /// The offset was only ever corrected by moving, so shrinking the terminal left the
    /// highlighted row — the one every key acts on — below the bottom edge until the next
    /// keypress. Fitted for the section's list even under a pushed view, so Back lands on a
    /// window that shows its selection.
    pub fn fit_to(&mut self, visible_rows: usize) {
        if self.content_view().is_none() {
            return;
        }
        let len = self.content.len();
        let cursor = self.cursors.section_mut(self.nav.section());
        if len == 0 {
            cursor.offset = 0;
            return;
        }
        let rows = visible_rows.max(1);
        cursor.selected = cursor.selected.min(len - 1);
        if cursor.selected < cursor.offset {
            cursor.offset = cursor.selected;
        } else if cursor.selected >= cursor.offset + rows {
            cursor.offset = cursor.selected + 1 - rows;
        }
        // A taller window shows more of the list rather than empty rows under its end.
        cursor.offset = cursor.offset.min(len.saturating_sub(rows));
    }

    /// Which rows the cursor walks on the screen on top, if it has any.
    ///
    /// One answer for every movement key. Each screen used to claim ↑ and ↓ for itself, and
    /// every key it forgot — `g`, `G`, ctrl+u, ctrl+d, or all of them on a title page — fell
    /// through to the section's list hidden underneath, so `G` on Settings landed on whichever
    /// row matched the length of a list nobody could see.
    fn rows_here(&self) -> Option<Rows> {
        match self.nav.current() {
            StageView::Episodes(_) => Some(Rows::Episodes),
            StageView::Stores => Some(Rows::Stores),
            StageView::Store(_) => Some(Rows::StoreLines),
            StageView::StoreMembers(_) => Some(Rows::Members),
            StageView::Section(Section::Downloads) => Some(Rows::Downloads),
            StageView::Section(Section::Accounts) => Some(Rows::Accounts),
            StageView::Section(Section::Settings) => Some(Rows::Settings),
            // The cursor is the row `K` and `J` move up and down the chain.
            StageView::Section(Section::Providers) => Some(Rows::Providers),
            StageView::Section(_) => Some(Rows::List),
            // A title page is one title, not a list, and Now Playing's keys belong to the
            // player. Moving on either moved the list behind it, so Back landed rows away from
            // where it was left.
            StageView::Title(_) | StageView::NowPlaying => None,
        }
    }

    /// Up, Down, the pages, Top and Bottom, for whatever has the keyboard.
    ///
    /// Every cursor clamps at its ends — the rail, the lists, the tables — so holding a key
    /// stops somewhere informative rather than cycling.
    pub(super) fn move_cursor(
        &mut self,
        action: Action,
        visible_rows: usize,
    ) -> Option<Command> {
        let page = visible_rows.max(1) as isize;
        let delta = match action {
            Action::Down => 1,
            Action::Up => -1,
            Action::PageDown => page,
            Action::PageUp => -page,
            Action::Top | Action::Bottom => 0,
            _ => return None,
        };

        // The rail owns the vertical axis when it has focus, all of it: the arrows used to walk
        // it while `g` and `G` scrolled the list behind it. The rail is shorter than any page,
        // so a page or an end goes to the first or last section.
        if self.nav.focus() == Focus::Rail {
            let all = Section::ALL.len() as isize;
            return self.step_section(match action {
                Action::Down => 1,
                Action::Up => -1,
                Action::PageUp | Action::Top => -all,
                _ => all,
            });
        }

        let rows = self.rows_here()?;
        if rows == Rows::List {
            // Above the first result is the query it answered.
            if self.search_in_results
                && action == Action::Up
                && self.section_cursor().selected == 0
            {
                self.search_in_results = false;
                return None;
            }
            let last = self.content.len().saturating_sub(1);
            match action {
                Action::Top => {
                    *self.cursors.section_mut(self.nav.section()) = Cursor::default()
                }
                Action::Bottom => {
                    let offset = last.saturating_sub(visible_rows.saturating_sub(1));
                    *self.cursors.section_mut(self.nav.section()) =
                        Cursor { selected: last, offset };
                }
                _ => self.move_selection(delta, visible_rows),
            }
            return None;
        }

        let len = match rows {
            Rows::Downloads => self.downloads.len(),
            Rows::Accounts => self.sync.len(),
            Rows::Settings => SettingKey::ROWS.len(),
            Rows::Providers => self.providers.len(),
            Rows::Episodes => self.episodes.len(),
            // The last row is the one that joins another store.
            Rows::Stores => self.stores.len() + 1,
            Rows::StoreLines => self
                .store_view()
                .and_then(|id| self.store(id))
                .map_or(0, |s| self.store_lines(s).len()),
            Rows::Members => self.member_lines().len(),
            Rows::List => 0,
        };
        if len == 0 {
            return None;
        }
        let cursor = match rows {
            Rows::Downloads => &mut self.cursors.downloads.selected,
            Rows::Accounts => &mut self.cursors.accounts.selected,
            Rows::Settings => &mut self.cursors.settings.selected,
            Rows::Providers => &mut self.cursors.providers.selected,
            Rows::Episodes => &mut self.cursors.episodes.selected,
            Rows::Stores => &mut self.cursors.stores.selected,
            Rows::StoreLines => &mut self.cursors.store_lines.selected,
            Rows::Members => &mut self.cursors.members.selected,
            Rows::List => return None,
        };
        let last = len as isize - 1;
        *cursor = match action {
            Action::Top => 0,
            Action::Bottom => last,
            _ => (*cursor as isize).saturating_add(delta).clamp(0, last),
        } as usize;
        None
    }
}

/// Where each cursor is and what each field holds, asked by what they are rather than by where
/// they are kept: the tests set the stage and check it through these, so the state behind them
/// can move without the tests moving with it.
impl App {
    /// The highlighted row of the section's own view: its list, or its table on Downloads,
    /// Providers, Accounts and Settings. A view pushed over the section leaves it where it was.
    pub fn cursor(&self) -> usize {
        self.section_cursor().selected
    }

    pub fn set_cursor(&mut self, row: usize) {
        self.cursors.section_mut(self.nav.section()).selected = row;
    }

    /// The first row the section's list shows.
    pub fn offset(&self) -> usize {
        self.section_cursor().offset
    }

    pub fn set_offset(&mut self, row: usize) {
        self.cursors.section_mut(self.nav.section()).offset = row;
    }

    /// The cursor of the section's own view, whatever is pushed over it.
    pub fn section_cursor(&self) -> &Cursor {
        self.cursors.section(self.nav.section())
    }

    /// The highlighted row of the episode table.
    pub fn episode_cursor(&self) -> usize {
        self.cursors.episodes.selected
    }

    pub fn set_episode_cursor(&mut self, row: usize) {
        self.cursors.episodes.selected = row;
    }

    /// The highlighted row of the list overlay on top: one cursor for every overlay, which
    /// each one that lists something sets as it opens.
    pub fn overlay_cursor(&self) -> usize {
        self.cursors.overlay.selected
    }

    pub fn set_overlay_cursor(&mut self, row: usize) {
        self.cursors.overlay.selected = row;
    }

    /// The highlighted row of the store list; one past the last store is the join row.
    pub fn store_cursor(&self) -> usize {
        self.cursors.stores.selected
    }

    /// The highlighted line of a store's page.
    pub fn store_line_cursor(&self) -> usize {
        self.cursors.store_lines.selected
    }

    /// The highlighted row of a store's members, its unused invites after them.
    pub fn member_cursor(&self) -> usize {
        self.cursors.members.selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn selection_cannot_move_outside_the_content() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        // The rail owns the vertical axis while it has focus, and it holds focus at startup.
        a.nav.focus_stage();

        for _ in 0..10 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.cursor(), 2, "clamped at the last item");

        for _ in 0..10 {
            a.handle(Action::Up, 10);
        }
        assert_eq!(a.cursor(), 0, "clamped at the first");
    }

    #[test]
    fn moving_in_empty_content_is_a_no_op() {
        let mut a = app();
        a.apply(a.content_here(ContentList::Entries(vec![])));
        a.handle(Action::Down, 10);
        a.handle(Action::Bottom, 10);
        assert_eq!(a.cursor(), 0);
    }

    #[test]
    fn shrinking_content_keeps_the_selection_in_range() {
        // The crash this prevents: selecting item 40, a refresh returning 3, and the next
        // render indexing past the end.
        let mut a = app();
        a.apply(a.content_here(entries(50)));
        a.nav.focus_stage();
        a.handle(Action::Bottom, 10);
        assert_eq!(a.cursor(), 49);

        a.apply(a.content_here(entries(3)));
        assert!(a.cursor() < 3, "selection {} out of range", a.cursor());
        assert!(a.selected_entry().is_some());
    }

    #[test]
    fn emptying_content_resets_selection_and_scroll() {
        let mut a = app();
        a.apply(a.content_here(entries(50)));
        a.handle(Action::Bottom, 10);
        a.apply(a.content_here(ContentList::Entries(vec![])));
        assert_eq!(a.cursor(), 0);
        assert_eq!(a.offset(), 0);
        assert!(a.selected_entry().is_none());
    }

    #[test]
    fn scrolling_keeps_the_selection_inside_the_viewport() {
        let mut a = app();
        a.apply(a.content_here(entries(100)));
        a.nav.focus_stage();
        let rows = 10;
        for _ in 0..25 {
            a.handle(Action::Down, rows);
        }
        assert_eq!(a.cursor(), 25);
        assert!(a.cursor() >= a.offset(), "selection scrolled off the top");
        assert!(
            a.cursor() < a.offset() + rows,
            "selection {} outside viewport starting {}",
            a.cursor(),
            a.offset()
        );
    }

    #[test]
    fn every_movement_key_moves_the_episode_table() {
        // ↑ and ↓ did; `g`, `G` and the pages moved the list behind the table instead.
        let mut a = app_at_episodes();
        a.handle(Action::Bottom, 5);
        assert_eq!(a.episode_cursor(), 11);
        a.handle(Action::Top, 5);
        assert_eq!(a.episode_cursor(), 0);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.episode_cursor(), 5);
        a.handle(Action::PageUp, 5);
        assert_eq!(a.episode_cursor(), 0);
        assert_eq!((a.cursor(), a.offset()), (0, 0), "the list under the table stays put");
    }

    #[test]
    fn settings_and_accounts_move_their_own_rows_and_act_on_no_title() {
        // With Continue loaded before them, `G` landed on the row matching that list's length
        // and the title keys opened overlays for one of its shows.
        let mut a = home_list(5);
        a.handle(Action::JumpSection(9), 5);
        a.handle(Action::Bottom, 5);
        assert_eq!(a.cursor(), SettingKey::ROWS.len() - 1);
        a.handle(Action::Top, 5);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.cursor(), 5);
        let title_keys = [
            Action::SetListStatus,
            Action::RateTitle,
            Action::ShowEpisodes,
            Action::WatchOrder,
            Action::Download,
            Action::ShowSources,
            Action::OpenInBrowser,
        ];
        for action in title_keys {
            assert_eq!(a.handle(action, 5), None, "{action:?}");
            assert_eq!(a.nav.overlay(), None, "{action:?}");
            assert_eq!(a.nav.current(), &StageView::Section(Section::Settings), "{action:?}");
        }

        a.handle(Action::JumpSection(8), 5);
        a.apply(Event::Sync(Box::new(SyncState::new("anilist"))));
        a.apply(Event::Sync(Box::new(SyncState::new("mal"))));
        a.handle(Action::Bottom, 5);
        assert_eq!(a.cursor(), 1);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.cursor(), 1, "past the last tracker");
        for action in title_keys.into_iter().chain([Action::PlayNext]) {
            assert_eq!(a.handle(action, 5), None, "{action:?}");
            assert_eq!(a.nav.overlay(), None, "{action:?}");
            assert_eq!(a.nav.current(), &StageView::Section(Section::Accounts), "{action:?}");
        }
    }

    #[test]
    fn a_shorter_window_brings_the_selection_back_into_view() {
        let mut a = home_list(50);
        for _ in 0..30 {
            a.handle(Action::Down, 40);
        }
        assert_eq!((a.cursor(), a.offset()), (30, 0));
        a.fit_to(10);
        assert!(
            a.cursor() >= a.offset() && a.cursor() < a.offset() + 10,
            "offset {}",
            a.offset()
        );

        // And a taller one shows more of the list rather than empty rows under its end.
        a.handle(Action::Bottom, 10);
        assert_eq!(a.offset(), 40);
        a.fit_to(30);
        assert_eq!(a.offset(), 20);
    }
}
