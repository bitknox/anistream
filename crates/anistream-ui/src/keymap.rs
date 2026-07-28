//! Actions, key bindings, and the help text generated from them.
//!
//! Two rules shape this module.
//!
//! **The help overlay is generated from the resolved keymap**, never written by hand. A
//! rebind that leaves the documentation stale is worse than no documentation, so there is
//! exactly one source of truth and a test that proves they agree after a rebind.
//!
//! **There is no fat footer.** A permanent `q quit  j/k move  h/l back` strip is the
//! clearest templated-TUI tell. Discoverability comes from `?` and the command palette
//! instead, with the status line carrying at most three contextual hints.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::nav::Section;

/// Everything the user can ask for.
///
/// One flat enum rather than per-screen enums: the command palette needs to enumerate every
/// action in one list, and a flat set makes "is this action available here?" a single
/// function rather than a matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Action {
    // Global
    Help,
    CommandPalette,
    Quit,
    Back,
    ToggleRail,
    Refresh,
    ForceResync,
    ToggleTranslation,
    JumpSection(u8),
    FocusSearch,
    ShowAccounts,
    ShowConflicts,
    ShowLogs,

    // Movement
    Up,
    Down,
    Left,
    Right,
    Top,
    Bottom,
    PageUp,
    PageDown,

    // Lists and grids
    Open,
    PlayNext,
    SetListStatus,
    ToggleSynopsis,

    // Title
    ShowEpisodes,
    ShowSources,
    FixMapping,
    Download,
    WatchOrder,
    OpenInBrowser,

    // Downloads
    ClearCompleted,
    /// Remove the selected download *and* its file on disk. Distinct from cancel (`x`),
    /// which for a finished download keeps the file and would otherwise orphan it.
    DeleteDownload,

    // Episodes
    /// Queue a typed range of episodes for download: `4`, `1-12`, `7-`.
    DownloadRange,
    /// Play an episode in a Syncplay watch party instead of a private session.
    PlayParty,
    ToggleWatched,
    MarkAllPrevious,
    Filter,

    // Playback
    PlayPause,
    SeekBack,
    SeekForward,
    SeekBackFar,
    SeekForwardFar,
    NextEpisode,
    PreviousEpisode,
    SkipOpening,
    SpeedDown,
    SpeedUp,
    VolumeDown,
    VolumeUp,
    Fullscreen,
    Detach,
    StopPlayback,
}

impl Action {
    /// Every action, in the order the command palette lists them.
    ///
    /// Written out rather than derived, because the palette is the app's discoverability
    /// mechanism and an action missing from it is an action nobody can find. `JumpSection` is
    /// represented once — eight near-identical rows would bury everything else.
    pub const ALL: &'static [Self] = &[
        Self::Help,
        Self::CommandPalette,
        Self::Quit,
        Self::Back,
        Self::ToggleRail,
        Self::Refresh,
        Self::ForceResync,
        Self::ToggleTranslation,
        Self::JumpSection(1),
        Self::FocusSearch,
        Self::ShowAccounts,
        Self::ShowConflicts,
        Self::ShowLogs,
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Top,
        Self::Bottom,
        Self::PageUp,
        Self::PageDown,
        Self::Open,
        Self::PlayNext,
        Self::SetListStatus,
        Self::ToggleSynopsis,
        Self::ShowEpisodes,
        Self::ShowSources,
        Self::FixMapping,
        Self::Download,
        Self::WatchOrder,
        Self::OpenInBrowser,
        Self::ClearCompleted,
        Self::DeleteDownload,
        Self::DownloadRange,
        Self::PlayParty,
        Self::ToggleWatched,
        Self::MarkAllPrevious,
        Self::Filter,
        Self::PlayPause,
        Self::SeekBack,
        Self::SeekForward,
        Self::SeekBackFar,
        Self::SeekForwardFar,
        Self::NextEpisode,
        Self::PreviousEpisode,
        Self::SkipOpening,
        Self::SpeedDown,
        Self::SpeedUp,
        Self::VolumeDown,
        Self::VolumeUp,
        Self::Fullscreen,
        Self::Detach,
        Self::StopPlayback,
    ];

    /// Label shown in the help overlay and the command palette.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Help => "Show keys",
            Self::CommandPalette => "Run a command",
            Self::Quit => "Quit",
            Self::Back => "Back",
            Self::ToggleRail => "Toggle rail width",
            Self::Refresh => "Refresh this view",
            Self::ForceResync => "Force a full resync",
            Self::ToggleTranslation => "Switch sub / dub",
            Self::JumpSection(_) => "Go to section",
            Self::FocusSearch => "Search",
            Self::ShowAccounts => "Accounts and sync",
            Self::ShowConflicts => "Resolve sync conflicts",
            Self::ShowLogs => "Recent errors and traces",
            Self::Up => "Move up",
            Self::Down => "Move down",
            Self::Left => "Move left",
            Self::Right => "Move right",
            Self::Top => "Jump to top",
            Self::Bottom => "Jump to bottom",
            Self::PageUp => "Page up",
            Self::PageDown => "Page down",
            Self::Open => "Open",
            Self::PlayNext => "Play next unwatched",
            Self::SetListStatus => "Set list status",
            Self::ToggleSynopsis => "Expand synopsis",
            Self::ShowEpisodes => "Episodes",
            Self::ShowSources => "Sources",
            Self::FixMapping => "Fix this match",
            Self::Download => "Download",
            Self::WatchOrder => "Watch order",
            Self::OpenInBrowser => "Open on AniList",
            Self::ClearCompleted => "Clear finished downloads",
            Self::DeleteDownload => "Delete download and its file",
            Self::DownloadRange => "Download a range of episodes",
            Self::PlayParty => "Play in a Syncplay party",
            Self::ToggleWatched => "Toggle watched",
            Self::MarkAllPrevious => "Mark all previous watched",
            Self::Filter => "Filter",
            Self::PlayPause => "Play / pause",
            Self::SeekBack => "Back 5s",
            Self::SeekForward => "Forward 5s",
            Self::SeekBackFar => "Back 30s",
            Self::SeekForwardFar => "Forward 30s",
            Self::NextEpisode => "Next episode",
            Self::PreviousEpisode => "Previous episode",
            Self::SkipOpening => "Skip opening",
            Self::SpeedDown => "Slower",
            Self::SpeedUp => "Faster",
            Self::VolumeDown => "Volume down",
            Self::VolumeUp => "Volume up",
            Self::Fullscreen => "Fullscreen",
            Self::Detach => "Leave player running",
            Self::StopPlayback => "Stop playback",
        }
    }

    /// The terse form used in the status line.
    ///
    /// The status line is not documentation — it has room for three hints, and a hint that gets
    /// dropped for being too long is worse than a curt one. "Leave player running" explains
    /// itself in the help overlay; here it is `detach`.
    pub const fn hint(self) -> &'static str {
        match self {
            Self::PlayPause => "pause",
            Self::SkipOpening => "skip",
            Self::Detach => "detach",
            Self::StopPlayback => "stop",
            Self::PlayNext => "play next",
            Self::ShowEpisodes => "episodes",
            Self::OpenInBrowser => "on AniList",
            Self::MarkAllPrevious => "mark previous",
            Self::ForceResync => "resync",
            Self::Help => "keys",
            other => other.label(),
        }
    }

    /// Which part of the UI this action belongs to. Groups the help overlay.
    pub const fn scope(self) -> Scope {
        match self {
            Self::Help
            | Self::CommandPalette
            | Self::Quit
            | Self::Back
            | Self::ToggleRail
            | Self::Refresh
            | Self::ForceResync
            | Self::ToggleTranslation
            | Self::JumpSection(_)
            | Self::FocusSearch
            | Self::ShowAccounts
            | Self::ShowConflicts
            | Self::ShowLogs => Scope::Global,

            Self::Up
            | Self::Down
            | Self::Left
            | Self::Right
            | Self::Top
            | Self::Bottom
            | Self::PageUp
            | Self::PageDown => Scope::Movement,

            Self::Open | Self::PlayNext | Self::SetListStatus | Self::ToggleSynopsis => {
                Scope::Lists
            }

            Self::ShowEpisodes
            | Self::ShowSources
            | Self::FixMapping
            | Self::Download
            | Self::WatchOrder
            | Self::OpenInBrowser
            | Self::PlayParty => Scope::Title,

            Self::ToggleWatched
            | Self::MarkAllPrevious
            | Self::Filter
            | Self::DownloadRange => Scope::Episodes,

            Self::ClearCompleted | Self::DeleteDownload => Scope::Downloads,

            _ => Scope::Playback,
        }
    }

    /// Whether this action stays live while a text field has focus.
    ///
    /// Most bindings must be suppressed while typing, or `d` in a search box would trigger
    /// a download. But the ones reachable only through a modified or special key are not
    /// ambiguous with text, so suppressing them would make the palette and Escape
    /// mysteriously stop working mid-search.
    pub const fn works_while_typing(self) -> bool {
        matches!(
            self,
            Self::CommandPalette
                | Self::Quit
                | Self::Back
                | Self::Open
                | Self::Up
                | Self::Down
                | Self::PageUp
                | Self::PageDown
                | Self::Help
        )
    }

    /// Stable machine name, used as the config key and the palette's search text.
    pub fn config_key(self) -> String {
        match self {
            Self::JumpSection(n) => format!("jump_section_{n}"),
            other => {
                let label = format!("{other:?}");
                let mut out = String::with_capacity(label.len() + 4);
                for (i, ch) in label.chars().enumerate() {
                    if ch.is_uppercase() {
                        if i > 0 {
                            out.push('_');
                        }
                        out.extend(ch.to_lowercase());
                    } else {
                        out.push(ch);
                    }
                }
                out
            }
        }
    }
}

/// Grouping for the help overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scope {
    Global,
    Movement,
    Lists,
    Title,
    Episodes,
    Downloads,
    Playback,
}

impl Scope {
    pub const ALL: [Self; 7] = [
        Self::Global,
        Self::Movement,
        Self::Lists,
        Self::Title,
        Self::Episodes,
        Self::Downloads,
        Self::Playback,
    ];

    pub const fn heading(self) -> &'static str {
        match self {
            Self::Global => "GLOBAL",
            Self::Movement => "MOVEMENT",
            Self::Lists => "LISTS AND GRIDS",
            Self::Title => "TITLE",
            Self::Episodes => "EPISODES",
            Self::Downloads => "DOWNLOADS",
            Self::Playback => "NOW PLAYING",
        }
    }
}

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

    fn from_event(event: KeyEvent) -> Self {
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

/// The resolved binding table.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: BTreeMap<Binding, Action>,
    /// Bindings that only apply while something is playing.
    ///
    /// A second table rather than entries in the first, because the useful playback keys are
    /// exactly the ones browsing has already claimed: `Space` is *play next* in a list and
    /// *pause* in the player, `q` is *back* and *detach*, the arrows move a selection and seek.
    /// Resolving by context is the only way both sets can have the obvious key.
    playback: BTreeMap<Binding, Action>,
}

/// Which binding table a keystroke resolves against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Browsing,
    /// Something is playing, so the playback table is consulted first.
    Playing,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    /// The built-in bindings. Vim-first, arrows always aliased.
    pub fn new() -> Self {
        use Action as A;
        use KeyCode::{Char, Down, Enter, Esc, Left, Right, Tab, Up};

        let mut bindings = BTreeMap::new();
        {
            let mut bind = |b: Binding, a: Action| {
                bindings.insert(b, a);
            };

            // Global
            bind(Binding::plain(Char('?')), A::Help);
            bind(Binding::plain(Char(':')), A::CommandPalette);
            bind(Binding::ctrl(Char('k')), A::CommandPalette);
            bind(Binding::plain(Char('Q')), A::Quit);
            bind(Binding::ctrl(Char('c')), A::Quit);
            bind(Binding::plain(Esc), A::Back);
            bind(Binding::plain(Char('q')), A::Back);
            bind(Binding::plain(Tab), A::ToggleRail);
            bind(Binding::plain(Char('r')), A::Refresh);
            bind(Binding::plain(Char('R')), A::ForceResync);
            bind(Binding::plain(Char('t')), A::ToggleTranslation);
            bind(Binding::plain(Char('/')), A::FocusSearch);
            // Logs get a real key rather than palette-only reachability. They are wanted at
            // exactly the moment something has broken, which is the worst moment to make someone
            // remember an indirection.
            bind(Binding::plain(Char('L')), A::ShowLogs);
            // Downloads. `d` queues (Title/Episodes), `c` clears finished, and the rest reuse the
            // playback keys the screen already reads naturally: Space pauses, `x` cancels.
            // `X` deletes the file too — the destructive twin earns the shifted key.
            bind(Binding::plain(Char('c')), A::ClearCompleted);
            bind(Binding::plain(Char('X')), A::DeleteDownload);
            for n in 1..=9u8 {
                bind(
                    Binding::plain(Char(char::from_digit(u32::from(n), 10).unwrap_or('1'))),
                    A::JumpSection(n),
                );
            }

            // Movement
            bind(Binding::plain(Char('j')), A::Down);
            bind(Binding::plain(Down), A::Down);
            bind(Binding::plain(Char('k')), A::Up);
            bind(Binding::plain(Up), A::Up);
            bind(Binding::plain(Char('h')), A::Left);
            bind(Binding::plain(Left), A::Left);
            bind(Binding::plain(Char('l')), A::Right);
            bind(Binding::plain(Right), A::Right);
            bind(Binding::plain(Char('g')), A::Top);
            bind(Binding::plain(Char('G')), A::Bottom);
            bind(Binding::ctrl(Char('u')), A::PageUp);
            bind(Binding::ctrl(Char('d')), A::PageDown);

            // Lists
            bind(Binding::plain(Enter), A::Open);
            bind(Binding::plain(Char(' ')), A::PlayNext);
            bind(Binding::plain(Char('a')), A::SetListStatus);
            bind(Binding::plain(Char('i')), A::ToggleSynopsis);

            // Title
            bind(Binding::plain(Char('e')), A::ShowEpisodes);
            bind(Binding::plain(Char('s')), A::ShowSources);
            bind(Binding::plain(Char('m')), A::FixMapping);
            bind(Binding::plain(Char('d')), A::Download);
            bind(Binding::plain(Char('w')), A::WatchOrder);
            bind(Binding::plain(Char('o')), A::OpenInBrowser);
            // `y` — you, plural: the same episode Enter would play, but with the room.
            bind(Binding::plain(Char('y')), A::PlayParty);

            // Episodes. `u` reads as "(un)watched" — Space belongs to the playback table
            // and `m` to fixing the match.
            bind(Binding::plain(Char('u')), A::ToggleWatched);
            bind(Binding::plain(Char('M')), A::MarkAllPrevious);
            bind(Binding::plain(Char('f')), A::Filter);
            // `d` queues one episode; its shifted twin queues a typed range.
            bind(Binding::plain(Char('D')), A::DownloadRange);
        }

        // Playback, in its own table: these keys take priority while something is playing and
        // mean nothing when it is not.
        let mut playback = BTreeMap::new();
        {
            let mut bind = |b: Binding, a: Action| {
                playback.insert(b, a);
            };
            bind(Binding::plain(Char(' ')), A::PlayPause);
            bind(Binding::plain(Left), A::SeekBack);
            bind(Binding::plain(Right), A::SeekForward);
            bind(Binding::plain(Char('H')), A::SeekBackFar);
            bind(Binding::plain(Char('L')), A::SeekForwardFar);
            bind(Binding::plain(Char('n')), A::NextEpisode);
            bind(Binding::plain(Char('N')), A::PreviousEpisode);
            bind(Binding::plain(Char('S')), A::SkipOpening);
            bind(Binding::plain(Char('[')), A::SpeedDown);
            bind(Binding::plain(Char(']')), A::SpeedUp);
            bind(Binding::plain(Char('9')), A::VolumeDown);
            bind(Binding::plain(Char('0')), A::VolumeUp);
            bind(Binding::plain(Char('F')), A::Fullscreen);
            // `q` leaves mpv running; `x` ends it. Two different intentions, so two keys — and
            // conflating them is how you lose your place in an episode.
            bind(Binding::plain(Char('q')), A::Detach);
            bind(Binding::plain(Char('x')), A::StopPlayback);
        }

        Self { bindings, playback }
    }

    /// Apply `[keys]` overrides from config.
    ///
    /// Returns the specs it could not understand, so a typo is reported rather than
    /// silently leaving the default in place and looking like the rebind was ignored.
    pub fn apply_overrides(&mut self, overrides: &BTreeMap<String, String>) -> Vec<String> {
        // Playback actions are rebindable too, and each has to land back in the table it came
        // from — putting `pause` in the browsing table would break `Space` in every list.
        let by_key: BTreeMap<String, (Action, bool)> = self
            .bindings
            .values()
            .map(|a| (a.config_key(), (*a, false)))
            .chain(self.playback.values().map(|a| (a.config_key(), (*a, true))))
            .collect();

        let mut rejected = Vec::new();
        for (action_name, spec) in overrides {
            let Some((action, is_playback)) = by_key.get(action_name) else {
                rejected.push(format!("unknown action {action_name:?}"));
                continue;
            };
            let Some(binding) = Binding::parse(spec) else {
                rejected.push(format!("cannot parse key {spec:?} for {action_name}"));
                continue;
            };
            let table = if *is_playback { &mut self.playback } else { &mut self.bindings };
            // Drop the action's previous bindings so a rebind moves it rather than
            // adding a second key.
            table.retain(|_, a| a != action);
            table.insert(binding, *action);
        }
        rejected
    }

    /// Resolve a keystroke in a context.
    ///
    /// Playback wins where the two tables overlap, and falls through where it does not — so
    /// `?`, `:` and `Esc` keep working while an episode plays.
    pub fn action_for(&self, event: KeyEvent, context: Context) -> Option<Action> {
        let binding = Binding::from_event(event);
        if context == Context::Playing
            && let Some(action) = self.playback.get(&binding)
        {
            return Some(*action);
        }
        self.bindings.get(&binding).copied()
    }

    /// Every key bound to an action, for display.
    pub fn keys_for(&self, action: Action) -> Vec<Binding> {
        self.bindings
            .iter()
            .chain(self.playback.iter())
            .filter(|(_, a)| **a == action)
            .map(|(b, _)| *b)
            .collect()
    }

    /// The help overlay's content, grouped by scope.
    ///
    /// Generated from the live table — this *is* the documentation, so it cannot drift.
    pub fn help(&self) -> Vec<(Scope, Vec<(String, &'static str)>)> {
        let mut grouped: BTreeMap<Scope, BTreeMap<Action, Vec<Binding>>> = BTreeMap::new();
        for (binding, action) in self.bindings.iter().chain(self.playback.iter()) {
            // The eight jump keys are one concept; listing them individually would bury
            // everything else.
            if matches!(action, Action::JumpSection(n) if *n != 1) {
                continue;
            }
            grouped
                .entry(action.scope())
                .or_default()
                .entry(*action)
                .or_default()
                .push(*binding);
        }

        Scope::ALL
            .into_iter()
            .filter_map(|scope| {
                let actions = grouped.remove(&scope)?;
                let rows = actions
                    .into_iter()
                    .map(|(action, mut keys)| {
                        keys.sort();
                        let rendered = if matches!(action, Action::JumpSection(_)) {
                            "1–8".to_string()
                        } else {
                            keys.iter().map(Binding::render).collect::<Vec<_>>().join(" / ")
                        };
                        (rendered, action.label())
                    })
                    .collect::<Vec<_>>();
                Some((scope, rows))
            })
            .collect()
    }

    /// Actions offered by the command palette, filtered by a fuzzy-ish query.
    pub fn palette_entries(&self, query: &str) -> Vec<(Action, String)> {
        let needle = query.trim().to_lowercase();
        // Driven off `Action::ALL`, not the binding table: an action with no key — Accounts and
        // Conflicts deliberately have none — must still be reachable, and the palette is the
        // only place it can be reached from.
        Action::ALL
            .iter()
            .filter(|action| {
                needle.is_empty()
                    || action.label().to_lowercase().contains(&needle)
                    || action.config_key().contains(&needle)
            })
            .map(|action| {
                let mut keys = self.keys_for(*action);
                keys.sort();
                (*action, keys.first().map(Binding::render).unwrap_or_default())
            })
            .collect()
    }
}

/// The two or three contextual hints shown at the right of the status line.
///
/// Deliberately short. A full footer of bindings is the templated look this design avoids;
/// these are only the actions most likely to be wanted right now.
pub fn status_hints(
    keymap: &Keymap,
    view: &crate::nav::StageView,
    in_stage: bool,
) -> Vec<(String, &'static str)> {
    use crate::nav::StageView as V;
    let wanted: &[Action] = match (view, in_stage) {
        // Now Playing has its own vocabulary, and it is the one view where the browsing
        // hints would be actively wrong.
        (V::NowPlaying, _) => &[Action::PlayPause, Action::SkipOpening, Action::Detach],
        (V::Episodes(_), _) => &[Action::Open, Action::Back],
        (V::Section(Section::Search), _) => &[Action::Open, Action::Back],
        (V::Section(Section::Providers), _) => &[Action::Refresh, Action::Open],
        // Settings has nothing to open and no episodes, so offering those keys would be a lie.
        // Left and Right are what actually do something here.
        (V::Section(Section::Settings), true) => &[Action::Left, Action::Right],
        (_, true) => &[Action::Open, Action::ShowEpisodes],
        // Focus is on the rail: the arrows walk it, which is exactly the thing nobody could
        // discover when they did not.
        (_, false) => &[Action::Down, Action::Right],
    };
    wanted
        .iter()
        .filter_map(|action| {
            let key = keymap.keys_for(*action).first().map(Binding::render)?;
            Some((key, action.hint()))
        })
        .chain(std::iter::once(("?".to_string(), Action::Help.hint())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn no_binding_is_claimed_by_two_actions() {
        // A duplicate would make one of the two actions unreachable, silently.
        let keymap = Keymap::new();
        // BTreeMap keys are unique by construction, so the real risk is a later bind
        // overwriting an earlier one. Check that everything we expect still resolves.
        for (ch, expected) in [
            ('?', Action::Help),
            ('e', Action::ShowEpisodes),
            ('j', Action::Down),
            ('G', Action::Bottom),
            ('g', Action::Top),
            ('s', Action::ShowSources),
        ] {
            assert_eq!(
                keymap.action_for(key(ch), Context::Browsing),
                Some(expected),
                "for {ch:?}"
            );
        }
    }

    #[test]
    fn playback_keys_only_resolve_while_playing() {
        // The whole reason for two tables: `Space` is *play next* in a list and *pause* in the
        // player, and both need to be the obvious key.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key(' '), Context::Browsing), Some(Action::PlayNext));
        assert_eq!(keymap.action_for(key(' '), Context::Playing), Some(Action::PlayPause));
        assert_eq!(keymap.action_for(key('q'), Context::Browsing), Some(Action::Back));
        assert_eq!(keymap.action_for(key('q'), Context::Playing), Some(Action::Detach));

        // `S` seeks past an opening, which means nothing when nothing is playing. Firing some
        // other action instead would be worse than doing nothing.
        assert_eq!(keymap.action_for(key('S'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key('S'), Context::Playing), Some(Action::SkipOpening));
    }

    #[test]
    fn global_keys_survive_playback() {
        // Losing `?`, `:` or `Esc` the moment an episode starts would trap the user.
        let keymap = Keymap::new();
        for (event, expected) in [
            (key('?'), Action::Help),
            (key(':'), Action::CommandPalette),
            (KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), Action::Back),
            (key('Q'), Action::Quit),
        ] {
            assert_eq!(keymap.action_for(event, Context::Playing), Some(expected));
        }
    }

    #[test]
    fn a_rebound_playback_key_stays_in_the_playback_table() {
        // Moving `pause` into the browsing table would break `Space` in every list, which is
        // the kind of breakage a rebind must never cause.
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([("play_pause".to_string(), "p".to_string())]);
        assert!(keymap.apply_overrides(&overrides).is_empty());

        assert_eq!(keymap.action_for(key('p'), Context::Playing), Some(Action::PlayPause));
        assert_eq!(keymap.action_for(key('p'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key(' '), Context::Browsing), Some(Action::PlayNext));
        assert_eq!(
            keymap.action_for(key(' '), Context::Playing),
            Some(Action::PlayNext),
            "the old pause key must fall through, not keep pausing"
        );
    }

    #[test]
    fn case_distinguishes_bindings() {
        // `g`/`G` and `s`/`S` are different actions; merging them would be a real bug.
        let keymap = Keymap::new();
        assert_ne!(
            keymap.action_for(key('g'), Context::Browsing),
            keymap.action_for(key('G'), Context::Browsing)
        );
        assert_ne!(
            keymap.action_for(key('s'), Context::Browsing),
            keymap.action_for(key('S'), Context::Browsing)
        );
        assert_ne!(
            keymap.action_for(key('q'), Context::Browsing),
            keymap.action_for(key('Q'), Context::Browsing)
        );
    }

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
    fn arrows_alias_the_vim_keys() {
        let keymap = Keymap::new();
        for (arrow, vim) in [
            (KeyCode::Down, 'j'),
            (KeyCode::Up, 'k'),
            (KeyCode::Left, 'h'),
            (KeyCode::Right, 'l'),
        ] {
            assert_eq!(
                keymap.action_for(KeyEvent::new(arrow, KeyModifiers::NONE), Context::Browsing),
                keymap.action_for(key(vim), Context::Browsing)
            );
        }
    }

    #[test]
    fn all_eight_section_jumps_are_bound() {
        let keymap = Keymap::new();
        for n in 1..=8u8 {
            let ch = char::from_digit(u32::from(n), 10).unwrap();
            assert_eq!(
                keymap.action_for(key(ch), Context::Browsing),
                Some(Action::JumpSection(n))
            );
        }
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

    #[test]
    fn rebinding_moves_an_action_instead_of_adding_a_second_key() {
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([("show_episodes".to_string(), "ctrl+e".to_string())]);
        assert!(keymap.apply_overrides(&overrides).is_empty());

        assert_eq!(
            keymap.action_for(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
                Context::Browsing
            ),
            Some(Action::ShowEpisodes)
        );
        assert_ne!(
            keymap.action_for(key('e'), Context::Browsing),
            Some(Action::ShowEpisodes),
            "the old key must be released, not kept as an alias"
        );
    }

    #[test]
    fn a_bad_override_is_reported_rather_than_ignored() {
        // Silently keeping the default would look like the rebind was applied.
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([
            ("show_episodes".to_string(), "hyper+q".to_string()),
            ("not_an_action".to_string(), "x".to_string()),
        ]);
        let rejected = keymap.apply_overrides(&overrides);
        assert_eq!(rejected.len(), 2);
        assert!(rejected.iter().any(|r| r.contains("cannot parse")));
        assert!(rejected.iter().any(|r| r.contains("unknown action")));
    }

    #[test]
    fn help_is_generated_from_the_live_keymap_and_follows_a_rebind() {
        // The rule this module exists to enforce: documentation cannot drift from
        // behaviour, because it is derived from it.
        let mut keymap = Keymap::new();
        let before = keymap.help();
        let title_rows = |help: &Vec<(Scope, Vec<(String, &'static str)>)>| {
            help.iter()
                .find(|(s, _)| *s == Scope::Title)
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default()
        };
        assert!(title_rows(&before).iter().any(|(k, _)| k == "e"));

        keymap.apply_overrides(&BTreeMap::from([(
            "show_episodes".to_string(),
            "ctrl+e".to_string(),
        )]));
        let after = title_rows(&keymap.help());
        assert!(after.iter().any(|(k, l)| k == "ctrl+e" && *l == "Episodes"));
        assert!(
            !after.iter().any(|(k, l)| k == "e" && *l == "Episodes"),
            "stale binding still documented"
        );
    }

    #[test]
    fn help_collapses_the_eight_jump_keys_into_one_row() {
        // Eight near-identical rows would bury everything else in the global section.
        let keymap = Keymap::new();
        let global = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Global)
            .map(|(_, rows)| rows)
            .unwrap();
        let jump_rows: Vec<_> =
            global.iter().filter(|(_, label)| *label == "Go to section").collect();
        assert_eq!(jump_rows.len(), 1);
        assert_eq!(jump_rows[0].0, "1–8");
    }

    #[test]
    fn help_covers_every_scope_that_has_bindings() {
        let help = Keymap::new().help();
        let scopes: Vec<Scope> = help.iter().map(|(s, _)| *s).collect();
        for expected in [Scope::Global, Scope::Movement, Scope::Lists, Scope::Title] {
            assert!(scopes.contains(&expected), "{expected:?} missing from help");
        }
        assert!(help.iter().all(|(_, rows)| !rows.is_empty()));
    }

    #[test]
    fn the_command_palette_filters_by_label_and_by_config_key() {
        let keymap = Keymap::new();
        let all = keymap.palette_entries("");
        assert!(all.len() > 20, "palette should expose everything, got {}", all.len());

        let episodes = keymap.palette_entries("episode");
        assert!(episodes.iter().any(|(a, _)| *a == Action::ShowEpisodes));

        // Config-key search works too, so `show_episodes` finds it.
        assert!(
            keymap.palette_entries("show_ep").iter().any(|(a, _)| *a == Action::ShowEpisodes)
        );
        assert!(keymap.palette_entries("zzzz").is_empty());
    }

    #[test]
    fn the_palette_lists_every_action_bound_or_not() {
        // The palette is the app's discoverability mechanism, so an action missing from it is
        // an action nobody can reach. Accounts and Conflicts deliberately have no key — they
        // exist *only* here — which is why this cannot be driven off the binding table.
        let keymap = Keymap::new();
        let listed: Vec<Action> =
            keymap.palette_entries("").into_iter().map(|(a, _)| a).collect();
        for action in Action::ALL {
            assert!(listed.contains(action), "{action:?} is unreachable from the palette");
        }
        assert_eq!(listed.len(), Action::ALL.len(), "the palette listed something twice");
    }

    #[test]
    fn a_keyless_action_shows_no_key_rather_than_a_wrong_one() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("accounts");
        let (action, rendered) = entries.first().expect("Accounts is not in the palette");
        assert_eq!(*action, Action::ShowAccounts);
        assert!(rendered.is_empty(), "showed {rendered:?} for an unbound action");
    }

    #[test]
    fn a_bound_action_still_shows_its_key_in_the_palette() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("episodes");
        let (_, rendered) = entries
            .iter()
            .find(|(a, _)| *a == Action::ShowEpisodes)
            .expect("Episodes is not in the palette");
        assert_eq!(rendered, "e");
    }

    #[test]
    fn a_playback_action_shows_its_key_from_the_second_table() {
        // `keys_for` spans both tables, so the palette has to show `space` for pause even
        // though pause lives outside the browsing map.
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("pause");
        let (_, rendered) = entries
            .iter()
            .find(|(a, _)| *a == Action::PlayPause)
            .expect("Play / pause is not in the palette");
        assert!(!rendered.is_empty(), "playback bindings must still display a key");
    }

    #[test]
    fn config_keys_are_snake_case_and_unique() {
        let keymap = Keymap::new();
        let mut keys: Vec<String> =
            keymap.palette_entries("").iter().map(|(a, _)| a.config_key()).collect();
        let count = keys.len();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), count, "duplicate config key would make rebinding ambiguous");
        assert!(keys.iter().all(|k| {
            k.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit())
        }));
        assert!(keys.contains(&"show_episodes".to_string()));
    }

    #[test]
    fn status_hints_stay_short_and_always_offer_help() {
        // The whole point of not having a footer: at most a few hints, never a wall.
        let keymap = Keymap::new();
        let views: Vec<crate::nav::StageView> = Section::ALL
            .into_iter()
            .map(crate::nav::StageView::Section)
            .chain([
                crate::nav::StageView::NowPlaying,
                crate::nav::StageView::Episodes(anistream_core::ids::AnilistId::new(1)),
            ])
            .collect();
        for view in &views {
            for in_stage in [true, false] {
                let hints = status_hints(&keymap, view, in_stage);
                assert!(hints.len() <= 4, "{view:?} produced {} hints", hints.len());
                assert!(
                    hints.iter().any(|(k, _)| k == "?"),
                    "discoverability depends on ? always being offered"
                );
            }
        }
    }
}
