//! From a key to what it does: text into a field first, then an open overlay, then the
//! screen, and the way back out.

use anistream_i18n::t;
use anistream_model::{command::Command, downloads::DownloadState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, info};
use crate::{
    keymap::{Action, Context},
    nav::{Focus, Overlay, Section, StageView},
};

/// The website's privacy page, which the palette's "Open the privacy page" opens.
pub const PRIVACY_URL: &str = "https://anistream.tv/docs/privacy/"; // i18n-skip

impl App {
    /// Handle a keystroke: text into a focused field first, then the binding it resolves to.
    ///
    /// The whole route from key to action in one place, so what a key does in any state can be
    /// tested as a key rather than as the action someone assumed it would become.
    pub fn press(&mut self, key: KeyEvent, visible_rows: usize) -> Option<Command> {
        // Text first: while a field has focus, plain characters are input, not bindings.
        if self.is_typing() {
            match key.code {
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    self.type_char(c);
                    return None;
                }
                KeyCode::Backspace => {
                    self.backspace();
                    return None;
                }
                _ => {}
            }
        }
        let action = self.keymap.action_for(key, self.key_context())?;
        self.handle(action, visible_rows)
    }

    /// Which binding table a keystroke resolves against right now.
    ///
    /// Modal whenever an overlay or a text field has the keyboard, so their fixed keys hold
    /// whatever the rebinds are and the playback table cannot act behind them.
    pub fn key_context(&self) -> Context {
        if self.nav.has_overlay() || self.is_typing() {
            Context::Modal
        } else if self.playing.is_some() {
            Context::Playing
        } else {
            Context::Browsing
        }
    }

    /// Handle an action. Returns work for the caller to perform asynchronously.
    pub fn handle(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
        // While typing, most single-key bindings must not fire, or "d" would trigger a
        // download instead of entering a letter.
        if self.is_typing() {
            return self.handle_while_typing(action, visible_rows);
        }
        // An open overlay owns the keyboard: no key reaches the screen under it. Behind Help,
        // the arrows used to change — and save — whatever setting was highlighted.
        if let Some(overlay) = self.nav.overlay().cloned() {
            return self.handle_in_overlay(&overlay, action, visible_rows);
        }
        self.perform(action, visible_rows)
    }

    /// Run an action against the screen, with no overlay or text field in the way.
    ///
    /// Separate from [`Self::handle`] so the command palette can run what it picked: opened
    /// from Search's field, the field still has focus once the palette closes, and going back
    /// through the typing gate swallowed nearly every command.
    pub(super) fn perform(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
        // The download queue reads Space as pause and `x` as cancel, the keys it borrows from the
        // player. Resolved before the playback table: idle, those keys arrive as the browsing
        // actions — play next, remove from Continue — so a download could never be paused or
        // cancelled at all, and while something plays they would pause the player rather than
        // the row on screen.
        if self.in_downloads_stage() {
            match action {
                Action::PlayNext | Action::PlayPause => {
                    let id = self.downloads.get(self.cursors.downloads.selected)?.id;
                    return Some(Command::DownloadPause { id });
                }
                Action::HideFromContinue | Action::StopPlayback => {
                    let row = self.downloads.get(self.cursors.downloads.selected)?;
                    // A finished download has nothing left to cancel. Cancelling it forgot the
                    // row and left the file on disk with no way back to it from here.
                    if row.state == DownloadState::Done {
                        let delete = self.key_for(Action::DeleteDownload);
                        self.push_toast(info(t::toast_download_done(&delete)));
                        return None;
                    }
                    return Some(Command::DownloadCancel { id: row.id });
                }
                _ => {}
            }
        }

        // Playback bindings only mean anything while something is playing, and they collide
        // with browsing keys (`n`, `x`, digits), so they are dispatched first and only then.
        if self.playing.is_some()
            && let Some(task) = self.handle_playback(action)
        {
            return task;
        }

        // Title commands have nothing to act on without a title, on screen or highlighted, and
        // each says so the same way rather than some doing nothing at all.
        if action.acts_on_a_title()
            && !matches!(self.nav.current(), StageView::NowPlaying)
            && self.target().is_none()
        {
            self.push_toast(info(t::toast_nothing_selected()));
            return None;
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::Help => self.open_help(),
            // Goes to the screen now rather than opening the modal. The overlay stays in the
            // codebase for the one case it is still right for — being asked to sign in from
            // somewhere else — but "show me my accounts" belongs on a page.
            Action::ShowAccounts => return self.go_to_section(Section::Accounts),
            Action::ShowLogs => self.show_logs(),
            Action::ShowConflicts => self.show_conflicts(),
            Action::OpenPrivacy => {
                return Some(Command::OpenExternal { url: PRIVACY_URL.into() });
            }
            Action::ReportBug => return self.report_bug(),
            Action::CommandPalette => self.open_palette(),
            // Out of the search results is back into the query they answered, one step short
            // of the rail — the same order the two were entered in.
            Action::Back if self.search_in_results && self.on_search_root() => {
                self.search_in_results = false;
            }
            Action::Back => self.go_back(),
            Action::ToggleRail => self.nav.toggle_rail(),
            Action::JumpSection(n) => return self.jump_section(n),
            // Already on Search, `/` means "let me type" rather than "start again": it returns
            // to the field with the query and results as they were, without asking again.
            Action::FocusSearch if self.on_search_root() => self.focus_search_field(),
            Action::FocusSearch => return self.go_to_section(Section::Search),
            // Every vertical key, for whatever has the keyboard: the rail when it has focus,
            // otherwise the rows of the screen on top. See `move_cursor`.
            Action::Up
            | Action::Down
            | Action::PageUp
            | Action::PageDown
            | Action::Top
            | Action::Bottom => return self.move_cursor(action, visible_rows),
            // Left goes left until nothing is further left, then onto the rail. Screens with a
            // horizontal axis step it first — Settings a value, Library its segments — and a
            // pushed view's left is the view it was opened from, so a stack of titles is never
            // stranded behind a focused rail that switches section on the next ↓.
            Action::Right if self.in_settings_stage() => return self.step_setting(1),
            Action::Left if self.in_settings_stage() => return self.step_setting(-1),
            Action::Right if self.in_library_stage() => return self.step_segment(1),
            Action::Left if self.in_library_stage() => return self.step_segment(-1),
            Action::Right => self.nav.focus_stage(),
            // Now Playing hides the rail and leaves through its own keys, `q` and `x`.
            Action::Left
                if self.nav.depth() > 1
                    && !matches!(self.nav.current(), StageView::NowPlaying) =>
            {
                self.go_back();
            }
            Action::Left => self.left_to_rail(),
            // In the episode table Enter plays; everywhere else it opens the title.
            Action::Open if self.in_episodes() => return self.play_selected_episode(),
            // A title page is already open, so Enter goes one deeper — the episode table — the
            // way it opens a title from a list and plays an episode from the table. It used to
            // open whichever row of the list underneath was highlighted.
            Action::Open if matches!(self.nav.current(), StageView::Title(_)) => {
                return self.perform(Action::ShowEpisodes, visible_rows);
            }
            // On Settings, Enter and Space do the obvious thing: cycle a closed set, or
            // open a small typed editor for the values that are text — a screen that
            // points at config.toml for a language code has given up two keys early.
            Action::Open | Action::PlayNext if self.in_settings_stage() => {
                return self.open_setting();
            }
            Action::Open if self.in_accounts_stage() => return self.toggle_account(),
            // Plugin stores live behind Providers: Enter there opens them, from the rail too,
            // because the provider table has nothing else to open.
            Action::Open if self.on_providers() => return self.open_stores(),
            Action::Open if self.nav.focus() == Focus::Stage && self.in_stores() => {
                return self.open_store_row();
            }
            Action::Open if self.nav.focus() == Focus::Stage && self.store_view().is_some() => {
                return self.enter_store_line();
            }
            Action::Open
                if self.nav.focus() == Focus::Stage && self.members_view().is_some() =>
            {
                return self.enter_member_row();
            }
            // Enter plays a finished download from disk. Nothing else on this screen is an "open".
            Action::Open if self.in_downloads_stage() => return self.open_download(),
            Action::DeleteDownload if self.in_downloads_stage() => {
                return self.delete_download();
            }
            Action::ClearCompleted if self.in_downloads_stage() => {
                return Some(Command::DownloadClearCompleted);
            }
            // From the rail too, like Enter here: the table is the only thing on the screen.
            Action::RaiseSource if self.on_providers() => return self.move_source(-1),
            Action::LowerSource if self.on_providers() => return self.move_source(1),
            // From the rail, Enter means "into this section" rather than "open the title the
            // list happens to be sitting on" — which is what it used to do, opening something
            // the user never selected.
            Action::Open if self.nav.focus() == Focus::Rail => self.nav.focus_stage(),
            // On Continue, Enter does what the section is named for: a part-watched
            // episode resumes immediately, no detail screen in between. The title's other
            // views stay one key away — `e` episodes, `s` sources, `w` watch order — and
            // rows with nothing to resume still open normally.
            Action::Open if self.resumes_on_enter() => return self.resume_selected(),
            Action::Open => return self.open_selected(),
            // Space skips the detail screen entirely: from a list, straight into the next
            // episode you have not watched.
            Action::PlayNext => return self.play_next_unwatched(),
            Action::ShowEpisodes => return self.show_episodes(),
            Action::ToggleWatched if self.in_episodes() => return self.toggle_watched(),
            Action::MarkAllPrevious if self.in_episodes() => return self.mark_all_previous(),
            Action::DownloadRange if self.in_episodes() => self.open_download_range(),
            Action::DownloadRange => {
                self.push_toast(info(t::toast_ranges_elsewhere()));
            }
            Action::Filter if self.in_episodes() => self.cycle_episode_filter(),
            // By the view, not the section: a title opened from the results is a title page,
            // and `f` there means what it means on every other title page.
            Action::Filter if self.on_search_root() => self.open_search_filters(),
            Action::FilterSearch => return self.filter_search(),
            Action::Filter => {
                self.push_toast(info(t::toast_filters_elsewhere()));
            }
            Action::ToggleWatched | Action::MarkAllPrevious => {
                self.push_toast(info(t::toast_watched_elsewhere()));
            }
            Action::OpenInBrowser => return self.open_in_browser(),
            Action::PickProvider => return self.pick_provider(),
            Action::PlayTrailer => return self.play_trailer(),
            Action::Recommendations => return self.show_recommendations(),
            Action::WatchOrder => return self.show_watch_order(),
            Action::FixMapping => return self.fix_mapping(),
            Action::ShowSources => return self.show_sources(),
            Action::PlayParty => return self.play_party(),
            Action::Download => return self.download_episode(),
            Action::ToggleSynopsis => self.synopsis_expanded = !self.synopsis_expanded,
            Action::SetListStatus => self.pick_list_status(),
            Action::RateTitle => self.pick_rating(),
            Action::HideFromContinue => return self.hide_from_continue(),
            Action::ForceResync => return self.force_resync(),
            // On the store screens, refreshing means asking the stores again, not re-checking
            // the provider table they were opened from.
            Action::Refresh if self.in_stores() || self.store_view().is_some() => {
                return Some(Command::LoadStores);
            }
            Action::Refresh if self.members_view().is_some() => return self.refresh_members(),
            // A title page and an episode table refresh themselves, not the section's list
            // under them — which is what `r` used to fetch, saying "loading the calendar" over
            // a title page that never changed.
            Action::Refresh if matches!(self.nav.current(), StageView::Title(_)) => {
                return self.refresh_title();
            }
            Action::Refresh if self.in_episodes() => return self.refresh_episodes(),
            Action::Refresh => return self.refresh_section(),
            Action::ToggleTranslation => return self.toggle_translation(),
            _ => self.works_elsewhere(action),
        }
        None
    }

    /// Back: close an overlay, pop a pushed view, or at a section's root hand focus to the rail.
    fn go_back(&mut self) {
        match self.nav.back() {
            crate::nav::BackOutcome::AtRoot => {
                // At the very root, back is a no-op rather than a quit. Said as a toast:
                // written to the status line it stayed there, over the state it shows, for
                // the rest of the session.
                let quit = self.key_for(Action::Quit);
                self.push_toast(info(t::toast_at_top(&quit)));
            }
            crate::nav::BackOutcome::Popped => {
                if self.nav.focus() == Focus::Rail {
                    // Coming back from the rail lands in the search field, not mid-results.
                    self.search_in_results = false;
                }
                self.restore_title();
            }
            crate::nav::BackOutcome::ClosedOverlay => {}
        }
    }

    /// A section's number, counted from one: that section, at the top of its own view.
    fn jump_section(&mut self, n: u8) -> Option<Command> {
        if let Some(section) = Section::from_index(usize::from(n).saturating_sub(1)) {
            return self.go_to_section(section);
        }
        None
    }

    /// Left with nothing further left on the screen: onto the rail, and out of the search
    /// results, so coming back lands in the field.
    fn left_to_rail(&mut self) {
        self.search_in_results = false;
        self.nav.focus_rail();
    }

    /// An action that means nothing here, saying where it does.
    fn works_elsewhere(&mut self, action: Action) {
        // An action that means nothing here — a playback key with nothing playing, a
        // Downloads key elsewhere, usually run from the palette — says where it does
        // something rather than doing nothing, which would read as the key being broken.
        self.push_toast(info(t::toast_works_elsewhere(
            action.label(),
            action.where_it_works(),
        )));
    }

    fn handle_while_typing(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
        // Actions bound only to modified or special keys are unambiguous with text, so
        // suppressing them would make Escape and the palette mysteriously stop working
        // mid-search.
        if !action.works_while_typing() {
            return None;
        }
        // A prompt is a question, like the list overlays: its own keys and the way out, nothing
        // else. Search's field is a screen, and keeps the palette, filters and its result list.
        let in_prompt = self.nav.has_overlay();
        if in_prompt && matches!(action, Action::CommandPalette | Action::FilterSearch) {
            return None;
        }
        match action {
            Action::Back => self.dismiss_field(),
            Action::CommandPalette => self.open_palette(),
            Action::Help => self.open_help(),
            Action::Quit => self.should_quit = true,
            // The two sign-in steps. Enter with nothing typed stays put — an empty
            // submit is a slip of the finger, not an answer.
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuUsername) => {
                return self.submit_kitsu_user();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuPassword) => {
                return self.submit_kitsu_password();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::JoinStore) => {
                return self.submit_join_link();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::InviteName) => {
                return self.submit_invite_name();
            }
            // A join's confirmation, with this device's name in it to change.
            Action::Open if self.nav.overlay() == Some(&Overlay::StoreConfirm) => {
                return self.confirm_store_action();
            }
            // Mid-query is exactly when filters are wanted, so ctrl+f cannot wait for the
            // box to lose focus.
            Action::FilterSearch => self.open_search_filters(),
            // The palette takes text, so it reaches this path rather than the overlay block in
            // `handle`. It used to close on Enter without running anything, and move the list
            // *behind* it on the arrows — so the one mechanism meant to make every action
            // discoverable could show you an action and then refuse to perform it.
            Action::Open if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                return self.confirm_overlay(visible_rows);
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::EditSetting) => {
                return self.submit_setting();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::DownloadRange) => {
                return self.submit_range();
            }
            // Enter runs the manual search; the results come back as ordinary match
            // choices, so the whole Disambiguate tail is reused unchanged.
            Action::Open if self.nav.overlay() == Some(&Overlay::ManualQuery) => {
                return self.submit_manual_query();
            }
            Action::Down if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                self.palette_down()
            }
            Action::Up if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                self.cursors.overlay.selected = self.cursors.overlay.selected.saturating_sub(1);
            }
            Action::Open if self.nav.section() == Section::Search => {
                return self.submit_search();
            }
            // Down leaves the query for its results, where the keyboard is a list's again: `j`,
            // `k`, the pages and every title key. Moving the selection from inside the field
            // instead ran it past the end of the list, unbounded and unscrolled, with `j` and
            // `k` typed rather than moving it.
            Action::Down | Action::PageDown
                if self.on_search_root() && !in_prompt && !self.content.is_empty() =>
            {
                self.search_in_results = true;
            }
            // Anything else, the vertical keys included: nothing behind a text field moves while
            // it has the keyboard.
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::app::fixtures::*;

    fn setting_row(id: SettingKey) -> usize {
        SettingKey::ROWS.iter().position(|s| *s == id).expect("a settings row")
    }

    #[test]
    fn the_privacy_page_opens_in_the_browser_from_anywhere() {
        let mut a = app();
        let opened =
            Some(Command::OpenExternal { url: "https://anistream.tv/docs/privacy/".into() });
        assert_eq!(a.handle(Action::OpenPrivacy, 5), opened);
        // From the palette, which is where an action with no key is found.
        assert!(
            a.keymap
                .palette_entries("privacy", crate::keymap::Context::Browsing)
                .first()
                .is_some_and(|(action, _)| *action == Action::OpenPrivacy)
        );
    }

    #[test]
    fn every_number_jump_lands_on_the_stage_and_the_rail_keeps_the_rail() {
        for n in 1..=9u8 {
            let mut a = app();
            a.handle(Action::JumpSection(n), 5);
            assert_eq!(a.nav.focus(), Focus::Stage, "{n}");
        }
        let mut a = app();
        a.handle(Action::Down, 5);
        assert_eq!(a.nav.section(), Section::Calendar);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn left_reaches_the_rail_from_library_and_settings() {
        let mut a = app();
        a.handle(Action::JumpSection(5), 5);
        a.library_segment = LibrarySegment::Planning;
        assert_eq!(
            a.handle(Action::Left, 5),
            Some(Command::LoadLibrary(LibrarySegment::Watching))
        );
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);

        a.handle(Action::JumpSection(9), 5);
        a.set_cursor(setting_row(SettingKey::Motion));
        let mut owner = Owner::new(Config::default());
        owner.effective.theme.motion = true;
        owner.tell(&mut a);
        assert_eq!(a.handle(Action::Right, 5), None, "a switch that is on stays on");
        assert_eq!(setting_text(&a, SettingKey::Motion), "on");
        let task = a.handle(Action::Left, 5);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Motion, delta: -1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);
        assert_eq!(
            setting_text(&a, SettingKey::Motion),
            "off",
            "reaching the rail changed nothing"
        );

        // A ladder on its bottom rung and a typed value have nothing further left either.
        for (row, prepare) in [
            (
                SettingKey::Quality,
                (|owner: &mut Owner| owner.effective.playback.quality = 480) as fn(&mut Owner),
            ),
            (SettingKey::Subtitles, |_| {}),
        ] {
            a.nav.focus_stage();
            a.set_cursor(setting_row(row));
            prepare(&mut owner);
            owner.tell(&mut a);
            assert_eq!(a.handle(Action::Left, 5), None, "{row:?}");
            assert_eq!(a.nav.focus(), Focus::Rail, "{row:?}");
        }
        assert_eq!(a.settings.quality, 480);
    }

    #[test]
    fn the_rail_takes_every_vertical_key_while_it_has_focus() {
        // The arrows walked the rail while `g` and `G` scrolled the list behind it.
        let mut a = home_list(20);
        a.nav.focus_rail();
        a.handle(Action::Bottom, 5);
        assert_eq!(a.nav.section(), Section::Settings);
        a.handle(Action::PageUp, 5);
        assert_eq!(a.nav.section(), Section::Home);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn the_arrows_walk_the_rail_when_it_has_focus() {
        // The bug this covers: `Down` always moved the stage list, so with the rail focused —
        // which is how the app starts — the eight top-level views were reachable only by their
        // number keys. Fast if you know them, invisible if you do not.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        assert_eq!(a.nav.focus(), Focus::Rail, "the app starts on the rail");

        a.handle(Action::Down, 10);
        assert_eq!(a.nav.section(), Section::Calendar, "down should step the rail");
        a.handle(Action::Down, 10);
        assert_eq!(a.nav.section(), Section::Seasonal);
        a.handle(Action::Up, 10);
        assert_eq!(a.nav.section(), Section::Calendar);
        assert_eq!(a.nav.focus(), Focus::Rail, "stepping must not throw focus into the stage");

        // Clamped, not wrapping: holding a key should stop at the ends rather than cycle.
        for _ in 0..20 {
            a.handle(Action::Up, 10);
        }
        assert_eq!(a.nav.section(), Section::Home);
        for _ in 0..20 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.nav.section(), Section::Settings);
    }

    #[test]
    fn enter_on_the_rail_moves_into_the_stage_rather_than_opening_a_title() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, None, "it must not open a title the user never selected");
        assert_eq!(a.nav.focus(), Focus::Stage);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home), "no view was pushed");
    }

    #[test]
    fn quitting_sets_the_flag() {
        let mut a = app();
        assert!(!a.should_quit);
        a.handle(Action::Quit, 10);
        assert!(a.should_quit);
    }

    #[test]
    fn back_at_the_very_root_explains_rather_than_quitting() {
        let mut a = app();
        a.handle(Action::Back, 10);
        a.handle(Action::Back, 10);
        assert!(!a.should_quit, "back must never quit");
        assert!(
            a.toasts.iter().any(|t| t.notice.text == anistream_i18n::t::toast_at_top("Q")),
            "{:?}",
            a.toasts
        );
        // Said once and gone: written to the status line, it stood in for the state line there
        // for the rest of the session.
        assert!(a.status.is_empty(), "the status line keeps its state: {:?}", a.status);
        for _ in 0..200 {
            a.tick_toasts();
        }
        assert!(a.toasts.is_empty());
    }

    #[test]
    fn the_way_out_names_the_key_quit_is_actually_on() {
        let mut a = app();
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("quit".to_string(), "ctrl+q".to_string());
        assert!(a.keymap.apply_overrides(&keys).is_empty());
        a.handle(Action::Back, 10);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == anistream_i18n::t::toast_at_top("ctrl+q")),
            "{:?}",
            a.toasts
        );
    }
}
