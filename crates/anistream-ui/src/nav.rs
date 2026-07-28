//! Navigation state: the rail, the stage stack and the overlay stack.
//!
//! The model is a persistent rail as the spine, with the stage as a stack. The rail is
//! always reachable so you cannot get lost; the stage pushes deeper (Title → Episodes →
//! Sources) and pops back. That avoids both the top-tab-bar default and the disorientation
//! of a pure view stack.
//!
//! Overlays live on their own stack *above* the stage. The ordering matters and is the
//! subtlest rule here: `Esc` must dismiss an overlay before it pops the stage, or closing a
//! dialog would also navigate you backwards — losing a screen you never asked to leave.

use std::fmt;

/// A rail section. Always one keystroke away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Home,
    Calendar,
    Seasonal,
    Search,
    Library,
    Downloads,
    Providers,
    /// Tracker sign-in and sync state.
    ///
    /// A section rather than an overlay. It was a modal, which was fine for one account and stops
    /// being fine at two — sync state, outbox depth, token storage and per-tracker errors are
    /// something you *inspect*, and an overlay you have to dismiss to see anything else is the
    /// wrong container for that.
    Accounts,
    Settings,
}

impl Section {
    pub const ALL: [Self; 9] = [
        Self::Home,
        Self::Calendar,
        Self::Seasonal,
        Self::Search,
        Self::Library,
        Self::Downloads,
        Self::Providers,
        Self::Accounts,
        Self::Settings,
    ];

    /// Label as rendered in the rail, in caps but *not* letterspaced.
    ///
    /// Tracking is for eyebrows and metadata. Applied to eight stacked navigation labels it
    /// nearly doubles their width and makes the one piece of chrome you read most often the
    /// hardest thing on the screen to read — measured on a real terminal, not theorised.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "CONTINUE",
            Self::Calendar => "CALENDAR",
            Self::Seasonal => "SEASONAL",
            Self::Search => "SEARCH",
            Self::Library => "LIBRARY",
            Self::Downloads => "DOWNLOADS",
            Self::Providers => "PROVIDERS",
            Self::Accounts => "ACCOUNTS",
            Self::Settings => "SETTINGS",
        }
    }

    /// The section's mark, shown beside the label in the rail and alone when it collapses.
    ///
    /// Not an icon set bolted on for decoration, and deliberately not a nerd font — every
    /// glyph here is plain Unicode from the same blocks the rest of the design already uses.
    /// The four browse sections share one family of ruled squares whose ruling *is* that
    /// screen's own layout: rows for the day-by-day calendar, a grid for the cover grid,
    /// vertical rules for the library's spines on a shelf. So the mark encodes something
    /// true about where it takes you rather than illustrating the word next to it.
    pub const fn glyph(self) -> char {
        match self {
            Self::Home => '▸',     // resume — a playhead
            Self::Calendar => '▤', // ruled rows — the airing timeline
            Self::Seasonal => '▦', // the cover grid
            Self::Search => '/',   // the key that opens it, and idiomatic everywhere
            Self::Library => '▥',  // spines on a shelf
            Self::Downloads => '↓',
            Self::Providers => '◇',
            Self::Accounts => '◎',
            Self::Settings => '⚙',
        }
    }

    /// Whether this section's content needs the stage at full width.
    ///
    /// No section does. Downloads and Providers used to, on the theory that their tables needed the
    /// columns — but the rail jumping between widths as you arrow past two of eight sections reads
    /// as a glitch, not as chrome yielding to content, and neither table is actually wide enough to
    /// justify it. Only *pushed* views collapse the rail now, where the transition is the result of
    /// a deliberate step deeper rather than a side effect of moving the cursor.
    pub const fn wants_wide_stage(self) -> bool {
        false
    }

    /// Index for the `1`–`9` jump keys.
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

/// A view pushed onto the stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageView {
    /// The section's own content.
    Section(Section),
    /// A title's detail screen.
    Title(anistream_core::ids::AnilistId),
    /// The timing-sheet episode table.
    Episodes(anistream_core::ids::AnilistId),
    /// mpv control surface.
    NowPlaying,
}

impl StageView {
    /// Whether this view needs the rail out of the way.
    pub const fn wants_wide_stage(&self) -> bool {
        match self {
            Self::Episodes(_) | Self::NowPlaying => true,
            Self::Section(s) => s.wants_wide_stage(),
            Self::Title(_) => false,
        }
    }

    /// Whether the rail should be hidden entirely rather than merely collapsed.
    pub const fn hides_rail(&self) -> bool {
        matches!(self, Self::NowPlaying)
    }
}

/// A modal overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    CommandPalette,
    Help,
    Sources,
    Disambiguate,
    ManualQuery,
    WatchOrder,
    ListStatus,
    Accounts,
    Conflicts,
    Logs,
}

impl Overlay {
    pub const fn title(&self) -> &'static str {
        match self {
            Self::CommandPalette => "RUN",
            Self::Help => "KEYS",
            Self::Sources => "SOURCES",
            Self::Disambiguate => "WHICH ONE",
            Self::ManualQuery => "FIND MANUALLY",
            Self::WatchOrder => "WATCH ORDER",
            Self::ListStatus => "LIST STATUS",
            Self::Accounts => "ACCOUNTS",
            Self::Conflicts => "CONFLICTS",
            Self::Logs => "LOGS",
        }
    }

    /// Whether this overlay takes text input, which suppresses single-key bindings.
    pub const fn takes_text_input(&self) -> bool {
        matches!(self, Self::CommandPalette | Self::ManualQuery)
    }
}

/// How much horizontal room the rail occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailWidth {
    Expanded,
    /// Obi plus the section's mark.
    Collapsed,
    Hidden,
}

impl RailWidth {
    pub const fn cells(self) -> u16 {
        match self {
            Self::Expanded => 28,
            Self::Collapsed => 3,
            Self::Hidden => 0,
        }
    }
}

impl fmt::Display for RailWidth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Expanded => "expanded",
            Self::Collapsed => "collapsed",
            Self::Hidden => "hidden",
        };
        f.write_str(s)
    }
}

/// Which pane has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Rail,
    Stage,
}

/// The whole navigation state.
#[derive(Debug, Clone)]
pub struct Nav {
    section: Section,
    /// Always non-empty; index 0 is the current section's own view.
    stage: Vec<StageView>,
    overlays: Vec<Overlay>,
    focus: Focus,
    /// Set when the user has overridden the automatic rail width with `Tab`.
    rail_override: Option<RailWidth>,
}

impl Default for Nav {
    fn default() -> Self {
        Self::new()
    }
}

impl Nav {
    pub fn new() -> Self {
        Self {
            section: Section::Home,
            stage: vec![StageView::Section(Section::Home)],
            overlays: Vec::new(),
            focus: Focus::Rail,
            rail_override: None,
        }
    }

    pub fn section(&self) -> Section {
        self.section
    }

    pub fn focus(&self) -> Focus {
        self.focus
    }

    /// The view currently on top of the stage stack.
    pub fn current(&self) -> &StageView {
        // The stack is never emptied — `pop` refuses to remove the root.
        self.stage.last().unwrap_or(&StageView::Section(Section::Home))
    }

    pub fn depth(&self) -> usize {
        self.stage.len()
    }

    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlays.last()
    }

    pub fn has_overlay(&self) -> bool {
        !self.overlays.is_empty()
    }

    /// Effective rail width: an explicit `Tab` override wins, otherwise the current view
    /// decides.
    pub fn rail_width(&self) -> RailWidth {
        if self.current().hides_rail() {
            return RailWidth::Hidden;
        }
        if let Some(override_width) = self.rail_override {
            return override_width;
        }
        if self.current().wants_wide_stage() {
            RailWidth::Collapsed
        } else {
            RailWidth::Expanded
        }
    }

    /// Jump to a rail section, resetting the stage.
    ///
    /// Also clears any manual rail override: the user asked for a different place, so the
    /// automatic width for that place is the right starting point.
    pub fn go_to(&mut self, section: Section) {
        self.section = section;
        self.stage = vec![StageView::Section(section)];
        self.rail_override = None;
        self.focus = if section == Section::Search { Focus::Stage } else { Focus::Rail };
    }

    /// Push a view onto the stage.
    pub fn push(&mut self, view: StageView) {
        // Pushing the view already on top is a no-op rather than a duplicate, so a double
        // keypress does not require two pops to undo.
        if self.current() == &view {
            return;
        }
        self.stage.push(view);
        self.rail_override = None;
        self.focus = Focus::Stage;
    }

    /// Pop one level of the stage. Refuses to remove the root view.
    ///
    /// Returns whether anything moved, so the caller can distinguish "went back" from
    /// "already at the root" — the latter should hand focus to the rail rather than quit.
    pub fn pop(&mut self) -> bool {
        if self.stage.len() <= 1 {
            // At the root, popping means "return to the rail" rather than exiting. Quitting
            // from a back key would be a nasty surprise.
            if self.focus == Focus::Stage {
                self.focus = Focus::Rail;
                return true;
            }
            return false;
        }
        self.stage.pop();
        self.rail_override = None;
        true
    }

    pub fn open_overlay(&mut self, overlay: Overlay) {
        // Re-opening the same overlay should not stack it.
        if self.overlays.last() == Some(&overlay) {
            return;
        }
        self.overlays.push(overlay);
    }

    pub fn close_overlay(&mut self) -> bool {
        self.overlays.pop().is_some()
    }

    pub fn close_all_overlays(&mut self) {
        self.overlays.clear();
    }

    /// Handle the back/dismiss key.
    ///
    /// **Overlays first.** Dismissing a dialog must not also navigate backwards, or the
    /// user loses a screen they never asked to leave.
    pub fn back(&mut self) -> BackOutcome {
        if self.close_overlay() {
            return BackOutcome::ClosedOverlay;
        }
        if self.pop() { BackOutcome::Popped } else { BackOutcome::AtRoot }
    }

    /// Cycle the rail width manually.
    pub fn toggle_rail(&mut self) {
        // Never toggle into a state the current view forbids.
        if self.current().hides_rail() {
            return;
        }
        let next = match self.rail_width() {
            RailWidth::Expanded => RailWidth::Collapsed,
            RailWidth::Collapsed | RailWidth::Hidden => RailWidth::Expanded,
        };
        self.rail_override = Some(next);
    }

    /// Move focus between rail and stage.
    pub fn focus_stage(&mut self) {
        self.focus = Focus::Stage;
    }

    pub fn focus_rail(&mut self) {
        if !matches!(self.rail_width(), RailWidth::Hidden) {
            self.focus = Focus::Rail;
        }
    }
}

/// What the back key actually did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackOutcome {
    ClosedOverlay,
    Popped,
    /// Nothing left to go back to.
    AtRoot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;

    const FRIEREN: AnilistId = AnilistId::new(154_587);

    #[test]
    fn starts_on_home_with_the_rail_focused_and_expanded() {
        let nav = Nav::new();
        assert_eq!(nav.section(), Section::Home);
        assert_eq!(nav.focus(), Focus::Rail);
        assert_eq!(nav.rail_width(), RailWidth::Expanded);
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn escape_closes_an_overlay_before_it_pops_the_stage() {
        // The subtlest rule in the whole navigation model: dismissing a dialog must not
        // also navigate backwards.
        let mut nav = Nav::new();
        nav.push(StageView::Title(FRIEREN));
        nav.open_overlay(Overlay::Sources);

        assert_eq!(nav.back(), BackOutcome::ClosedOverlay);
        assert_eq!(nav.current(), &StageView::Title(FRIEREN), "stage must not have moved");

        assert_eq!(nav.back(), BackOutcome::Popped);
        assert_eq!(nav.current(), &StageView::Section(Section::Home));
    }

    #[test]
    fn overlays_pop_one_at_a_time_in_reverse_order() {
        let mut nav = Nav::new();
        nav.open_overlay(Overlay::Sources);
        nav.open_overlay(Overlay::Disambiguate);
        assert_eq!(nav.overlay(), Some(&Overlay::Disambiguate));
        nav.back();
        assert_eq!(nav.overlay(), Some(&Overlay::Sources));
        nav.back();
        assert!(!nav.has_overlay());
    }

    #[test]
    fn popping_past_the_root_lands_on_the_rail_rather_than_exiting() {
        // A back key that quits the application would be a nasty surprise.
        let mut nav = Nav::new();
        nav.focus_stage();
        assert_eq!(nav.back(), BackOutcome::Popped, "first back returns focus to the rail");
        assert_eq!(nav.focus(), Focus::Rail);
        assert_eq!(nav.back(), BackOutcome::AtRoot, "and then reports there is nowhere to go");
        assert_eq!(nav.depth(), 1, "the root view is never removed");
    }

    #[test]
    fn the_rail_collapses_entering_episodes_and_restores_on_pop() {
        let mut nav = Nav::new();
        nav.push(StageView::Title(FRIEREN));
        assert_eq!(nav.rail_width(), RailWidth::Expanded, "title screen keeps the rail");

        nav.push(StageView::Episodes(FRIEREN));
        assert_eq!(
            nav.rail_width(),
            RailWidth::Collapsed,
            "the timing-sheet table needs the columns"
        );

        nav.pop();
        assert_eq!(nav.rail_width(), RailWidth::Expanded, "and gets them back on the way out");
    }

    #[test]
    fn now_playing_hides_the_rail_entirely() {
        let mut nav = Nav::new();
        nav.push(StageView::NowPlaying);
        assert_eq!(nav.rail_width(), RailWidth::Hidden);
        assert_eq!(RailWidth::Hidden.cells(), 0);
    }

    #[test]
    fn tab_overrides_the_automatic_width_until_you_navigate() {
        let mut nav = Nav::new();
        nav.toggle_rail();
        assert_eq!(nav.rail_width(), RailWidth::Collapsed);
        nav.toggle_rail();
        assert_eq!(nav.rail_width(), RailWidth::Expanded);

        // Navigating clears the override — the new destination's own preference applies.
        nav.toggle_rail();
        nav.push(StageView::Episodes(FRIEREN));
        nav.pop();
        assert_eq!(nav.rail_width(), RailWidth::Expanded);
    }

    #[test]
    fn tab_cannot_reveal_the_rail_where_a_view_forbids_it() {
        let mut nav = Nav::new();
        nav.push(StageView::NowPlaying);
        nav.toggle_rail();
        assert_eq!(nav.rail_width(), RailWidth::Hidden);
    }

    #[test]
    fn jumping_to_a_section_resets_the_stage() {
        let mut nav = Nav::new();
        nav.push(StageView::Title(FRIEREN));
        nav.push(StageView::Episodes(FRIEREN));
        assert_eq!(nav.depth(), 3);

        nav.go_to(Section::Seasonal);
        assert_eq!(nav.depth(), 1);
        assert_eq!(nav.current(), &StageView::Section(Section::Seasonal));
        assert_eq!(nav.section(), Section::Seasonal);
    }

    #[test]
    fn search_focuses_the_stage_so_typing_goes_to_the_query() {
        // Landing on the rail would mean the first keystrokes are swallowed as navigation.
        let mut nav = Nav::new();
        nav.go_to(Section::Search);
        assert_eq!(nav.focus(), Focus::Stage);

        nav.go_to(Section::Seasonal);
        assert_eq!(nav.focus(), Focus::Rail);
    }

    #[test]
    fn pushing_the_same_view_twice_is_a_no_op() {
        // Otherwise a double keypress needs two pops to undo.
        let mut nav = Nav::new();
        nav.push(StageView::Title(FRIEREN));
        nav.push(StageView::Title(FRIEREN));
        assert_eq!(nav.depth(), 2);
    }

    #[test]
    fn reopening_the_same_overlay_does_not_stack_it() {
        let mut nav = Nav::new();
        nav.open_overlay(Overlay::Help);
        nav.open_overlay(Overlay::Help);
        nav.close_overlay();
        assert!(!nav.has_overlay());
    }

    #[test]
    fn every_section_is_reachable_by_index_and_round_trips() {
        for (i, section) in Section::ALL.iter().enumerate() {
            assert_eq!(Section::from_index(i), Some(*section));
            assert_eq!(section.index(), i);
        }
        assert_eq!(Section::ALL.len(), 9, "the 1-9 jump keys must cover every section");
        assert!(Section::from_index(9).is_none());
    }

    #[test]
    fn section_marks_are_distinct_and_single_cell() {
        // A duplicate mark would make the collapsed rail ambiguous, and a wide or emoji
        // glyph would shunt the label a column right on that row alone.
        let mut marks: Vec<char> = Section::ALL.iter().map(|s| s.glyph()).collect();
        let count = marks.len();
        for mark in &marks {
            assert!(
                (*mark as u32) < 0x1F000,
                "{mark:?} is in the emoji planes and will break the character grid"
            );
        }
        marks.sort_unstable();
        marks.dedup();
        assert_eq!(marks.len(), count, "collapsed-rail marks must be unique");
    }

    #[test]
    fn focus_cannot_move_to_a_hidden_rail() {
        let mut nav = Nav::new();
        nav.push(StageView::NowPlaying);
        nav.focus_rail();
        assert_eq!(nav.focus(), Focus::Stage, "there is no rail to focus");
    }

    #[test]
    fn text_input_overlays_are_identified() {
        // Single-key bindings must be suppressed while typing, or "d" would trigger
        // download instead of entering a letter.
        assert!(Overlay::CommandPalette.takes_text_input());
        assert!(Overlay::ManualQuery.takes_text_input());
        assert!(!Overlay::Help.takes_text_input());
        assert!(!Overlay::Sources.takes_text_input());
    }
}
