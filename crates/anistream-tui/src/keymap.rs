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
    /// Give the selected title a score. Deliberate and user-initiated only — finishing a
    /// show never asks for one, because a prompt would turn rating into a chore.
    RateTitle,
    ToggleSynopsis,
    /// Take the selected title off the CONTINUE rail. A dismissal, not a deletion — the
    /// watch history survives, and watching the title again brings it back.
    HideFromContinue,

    // Title
    ShowEpisodes,
    ShowSources,
    PickProvider,
    FixMapping,
    Download,
    WatchOrder,
    /// Titles rated highest by people who liked this one.
    Recommendations,
    /// The promotional video, in mpv when it can stream the host and the browser otherwise.
    PlayTrailer,
    OpenInBrowser,

    // Downloads
    ClearCompleted,
    /// Remove the selected download *and* its file on disk. Distinct from cancel (`x`),
    /// which for a finished download keeps the file and would otherwise orphan it.
    DeleteDownload,

    // Providers. Both save `providers.order`, so they change the order failover walks from then
    // on, not just how the table looks.
    /// Try the selected source before the one above it.
    RaiseSource,
    /// Try the selected source after the one below it.
    LowerSource,

    // Episodes
    /// Queue a typed range of episodes for download: `4`, `1-12`, `7-`.
    DownloadRange,
    /// Play an episode in a Syncplay watch party instead of a private session.
    PlayParty,
    ToggleWatched,
    MarkAllPrevious,
    Filter,
    /// Open the search filter overlay, from anywhere. A modified key on purpose: the
    /// search box eats plain letters, and filters are wanted mid-query.
    FilterSearch,

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
    /// mechanism and an action missing from it is an action nobody can find. Each section has
    /// its own entry, after the rest of the global actions: one "Go to section" row could only
    /// ever go to the first, and nine rows at the top would bury everything else in an
    /// unfiltered list. Typed, they are found by name — `sett` reaches Settings.
    pub const ALL: &'static [Self] = &[
        Self::Help,
        Self::CommandPalette,
        Self::Quit,
        Self::Back,
        Self::ToggleRail,
        Self::Refresh,
        Self::ForceResync,
        Self::ToggleTranslation,
        Self::FocusSearch,
        Self::ShowAccounts,
        Self::ShowConflicts,
        Self::ShowLogs,
        Self::JumpSection(1),
        Self::JumpSection(2),
        Self::JumpSection(3),
        Self::JumpSection(4),
        Self::JumpSection(5),
        Self::JumpSection(6),
        Self::JumpSection(7),
        Self::JumpSection(8),
        Self::JumpSection(9),
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
        Self::RateTitle,
        Self::ToggleSynopsis,
        Self::HideFromContinue,
        Self::ShowEpisodes,
        Self::ShowSources,
        Self::PickProvider,
        Self::FixMapping,
        Self::Download,
        Self::WatchOrder,
        Self::Recommendations,
        Self::PlayTrailer,
        Self::OpenInBrowser,
        Self::ClearCompleted,
        Self::DeleteDownload,
        Self::RaiseSource,
        Self::LowerSource,
        Self::DownloadRange,
        Self::PlayParty,
        Self::ToggleWatched,
        Self::MarkAllPrevious,
        Self::Filter,
        Self::FilterSearch,
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
            // Named for the section, in the rail's order, so the palette can offer each one.
            Self::JumpSection(1) => "Go to Continue",
            Self::JumpSection(2) => "Go to Calendar",
            Self::JumpSection(3) => "Go to Seasonal",
            Self::JumpSection(4) => "Go to Search",
            Self::JumpSection(5) => "Go to Library",
            Self::JumpSection(6) => "Go to Downloads",
            Self::JumpSection(7) => "Go to Providers",
            Self::JumpSection(8) => "Go to Accounts",
            Self::JumpSection(9) => "Go to Settings",
            Self::JumpSection(_) => "Go to section",
            Self::FocusSearch => "Search",
            Self::ShowAccounts => "Accounts and sync",
            Self::ShowConflicts => "Resolve sync conflicts",
            Self::ShowLogs => "Recent messages",
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
            Self::RateTitle => "Rate this title",
            Self::ToggleSynopsis => "Expand synopsis",
            Self::HideFromContinue => "Remove from continue watching",
            Self::ShowEpisodes => "Episodes",
            Self::ShowSources => "Sources",
            Self::PickProvider => "Source for this title",
            Self::FixMapping => "Fix this match",
            Self::Download => "Download",
            Self::WatchOrder => "Watch order",
            Self::Recommendations => "Recommendations",
            Self::PlayTrailer => "Play the trailer",
            Self::OpenInBrowser => "Open on AniList",
            Self::ClearCompleted => "Clear finished downloads",
            Self::DeleteDownload => "Delete download and its file",
            Self::RaiseSource => "Move source up",
            Self::LowerSource => "Move source down",
            Self::DownloadRange => "Download a range of episodes",
            Self::PlayParty => "Play in a Syncplay party",
            Self::ToggleWatched => "Toggle watched",
            Self::MarkAllPrevious => "Mark all previous watched",
            Self::Filter => "Filter",
            Self::FilterSearch => "Search filters",
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
            Self::FilterSearch => "filters",
            Self::ForceResync => "resync",
            Self::Help => "keys",
            Self::Refresh => "refresh",
            Self::ToggleWatched => "watched",
            Self::ClearCompleted => "clear finished",
            Self::DeleteDownload => "delete",
            Self::RaiseSource => "move up",
            Self::LowerSource => "move down",
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

            Self::Open
            | Self::PlayNext
            | Self::SetListStatus
            | Self::RateTitle
            | Self::ToggleSynopsis
            | Self::FilterSearch
            | Self::HideFromContinue => Scope::Lists,

            Self::ShowEpisodes
            | Self::ShowSources
            | Self::PickProvider
            | Self::FixMapping
            | Self::Download
            | Self::WatchOrder
            | Self::Recommendations
            | Self::PlayTrailer
            | Self::OpenInBrowser
            | Self::PlayParty => Scope::Title,

            Self::ToggleWatched
            | Self::MarkAllPrevious
            | Self::Filter
            | Self::DownloadRange => Scope::Episodes,

            Self::ClearCompleted | Self::DeleteDownload => Scope::Downloads,

            Self::RaiseSource | Self::LowerSource => Scope::Providers,

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
                | Self::FilterSearch
        )
    }

    /// Whether this action is about one title, so it has nothing to do when no title is on
    /// screen or highlighted.
    ///
    /// Asked once, before any of them runs, so every one of them answers "nothing selected" the
    /// same way rather than some saying so, some saying something else and some doing nothing.
    pub const fn acts_on_a_title(self) -> bool {
        matches!(self.scope(), Scope::Title)
            || matches!(self, Self::SetListStatus | Self::RateTitle)
    }

    /// Where an action that does nothing here does something, for the toast that says so.
    ///
    /// The palette offers every action from everywhere, so it can run one on a screen where it
    /// means nothing. Naming the place answers the question that leaves, which "not available"
    /// would not.
    pub const fn where_it_works(self) -> &'static str {
        match self.scope() {
            Scope::Playback => "while something is playing",
            Scope::Downloads => "on the Downloads screen",
            Scope::Providers => "on the Providers screen",
            Scope::Episodes => "in the episode table",
            _ => "on another screen",
        }
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
    Providers,
    Playback,
}

impl Scope {
    pub const ALL: [Self; 8] = [
        Self::Global,
        Self::Movement,
        Self::Lists,
        Self::Title,
        Self::Episodes,
        Self::Downloads,
        Self::Providers,
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
            Self::Providers => "PROVIDERS",
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
    /// An overlay or a text field has the keyboard. The playback table is skipped — a dialog
    /// open over Now Playing answers `q` by closing, not by detaching the player under it — and
    /// a few keys mean the same thing whatever `[keys]` says; see [`modal_action`].
    Modal,
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
            bind(Binding::ctrl(Char('f')), A::FilterSearch);
            // Logs get a real key rather than palette-only reachability. They are wanted at
            // exactly the moment something has broken, which is the worst moment to make someone
            // remember an indirection.
            bind(Binding::plain(Char('L')), A::ShowLogs);
            // Downloads. `d` queues (Title/Episodes), `c` clears finished, and the rest reuse the
            // playback keys the screen already reads naturally: Space pauses, `x` cancels.
            // `X` deletes the file too — the destructive twin earns the shifted key.
            bind(Binding::plain(Char('c')), A::ClearCompleted);
            bind(Binding::plain(Char('X')), A::DeleteDownload);
            // Providers. The shifted twins of the movement keys move the row rather than the
            // cursor: `K` takes the selected source up the order, `J` down.
            bind(Binding::plain(Char('K')), A::RaiseSource);
            bind(Binding::plain(Char('J')), A::LowerSource);
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
            // `*` — a star, for the star you are giving it.
            bind(Binding::plain(Char('*')), A::RateTitle);
            bind(Binding::plain(Char('i')), A::ToggleSynopsis);
            // `x` removes, matching what it means for downloads and playback. Browsing
            // only — while something plays, the playback table's `x` (stop) wins.
            bind(Binding::plain(Char('x')), A::HideFromContinue);

            // Title
            bind(Binding::plain(Char('e')), A::ShowEpisodes);
            bind(Binding::plain(Char('s')), A::ShowSources);
            // `P` for provider. Neither `S` nor lowercase `p` is available: `S` is *skip
            // opening* while something is playing, and `p` is the canonical rebind target for
            // *pause* — both tables keep a key meaning one thing, so neither is borrowed here.
            bind(Binding::plain(Char('P')), A::PickProvider);
            bind(Binding::plain(Char('m')), A::FixMapping);
            bind(Binding::plain(Char('d')), A::Download);
            bind(Binding::plain(Char('w')), A::WatchOrder);
            // `b` — because you liked this.
            bind(Binding::plain(Char('b')), A::Recommendations);
            // `v` — video, the one thing about a title that is not text.
            bind(Binding::plain(Char('v')), A::PlayTrailer);
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
    /// Returns the specs it refused, so a typo is reported rather than silently leaving the
    /// default in place and looking like the rebind was ignored. A rebind onto a key another
    /// action keeps is refused the same way: taking it would leave that action with no key and
    /// nothing said, which is how `back = "x"` quietly removed *remove from continue watching*.
    /// Moving the other action too frees the key, so swapping two keys works.
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
        // Every rebind is read before any is applied: whether a key is free depends on the
        // others, since a key one rebind takes may be one another rebind moves away from.
        let mut wanted: Vec<(&str, Action, bool, Binding, &str)> = Vec::new();
        for (action_name, spec) in overrides {
            let Some((action, is_playback)) = by_key.get(action_name) else {
                rejected.push(format!("unknown action {action_name:?}"));
                continue;
            };
            let Some(binding) = Binding::parse(spec) else {
                rejected.push(format!("cannot parse key {spec:?} for {action_name}"));
                continue;
            };
            wanted.push((action_name, *action, *is_playback, binding, spec));
        }

        let mut accepted = vec![true; wanted.len()];
        // Two rebinds asking for one key: neither is more right than the other, so neither
        // gets it.
        for i in 0..wanted.len() {
            for j in 0..i {
                let (a, b) = (&wanted[j], &wanted[i]);
                if a.2 == b.2 && a.3 == b.3 {
                    if accepted[i] {
                        rejected.push(format!(
                            "key {:?} is given to both {} and {}",
                            b.4, a.0, b.0
                        ));
                    }
                    accepted[i] = false;
                    accepted[j] = false;
                }
            }
        }
        // A key is free when nobody holds it, or when its holder is being moved elsewhere by a
        // rebind that is itself accepted. Refusing one rebind can leave its action on a key a
        // second rebind was counting on, so this runs until nothing changes.
        loop {
            let mut changed = false;
            for i in 0..wanted.len() {
                let (name, action, is_playback, binding, spec) = wanted[i];
                if !accepted[i] {
                    continue;
                }
                let table = if is_playback { &self.playback } else { &self.bindings };
                let Some(holder) = table.get(&binding).copied().filter(|h| *h != action) else {
                    continue;
                };
                let holder_moves =
                    wanted.iter().zip(&accepted).any(|(w, ok)| *ok && w.1 == holder);
                if !holder_moves {
                    rejected.push(format!(
                        "key {spec:?} for {name} is already bound to {}",
                        holder.config_key()
                    ));
                    accepted[i] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Release first, then bind: a swap only works if neither side's new key is cleared by
        // the other's release.
        let moving: Vec<_> = wanted.iter().zip(&accepted).filter(|(_, ok)| **ok).collect();
        for ((_, action, is_playback, _, _), _) in &moving {
            let table = if *is_playback { &mut self.playback } else { &mut self.bindings };
            // Drop the action's previous bindings so a rebind moves it rather than
            // adding a second key.
            table.retain(|_, a| a != action);
        }
        for ((_, action, is_playback, binding, _), _) in moving {
            let table = if *is_playback { &mut self.playback } else { &mut self.bindings };
            table.insert(*binding, *action);
        }
        rejected
    }

    /// Resolve a keystroke in a context.
    ///
    /// Playback wins where the two tables overlap, and falls through where it does not — so
    /// `?`, `:` and `Esc` keep working while an episode plays.
    pub fn action_for(&self, event: KeyEvent, context: Context) -> Option<Action> {
        let binding = Binding::from_event(event);
        if context == Context::Modal
            && let Some(action) = modal_action(binding)
        {
            return Some(action);
        }
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

    /// The key for `action` as a sentence names it: the first one the help overlay lists, so a
    /// rebind changes what the sentence says. Unbound, it points at the palette instead.
    pub fn key_for(&self, action: Action) -> String {
        let mut keys = self.keys_for(action);
        keys.sort();
        keys.first().map_or_else(
            || format!("\u{201c}{}\u{201d} in the palette", action.label().to_lowercase()),
            Binding::render,
        )
    }

    /// The keys that reach `action` once the screen has the keyboard in `context`: every key
    /// bound to it, less any browsing key the playback table takes while something plays.
    ///
    /// `q` is *back* on a list and *detach* while an episode plays, so naming every bound key
    /// told a viewer to press the one that let go of the player.
    pub fn keys_in(&self, action: Action, context: Context) -> Vec<Binding> {
        self.keys_for(action)
            .into_iter()
            .filter(|b| {
                context != Context::Playing || self.playback.get(b).is_none_or(|a| *a == action)
            })
            .collect()
    }

    /// The help overlay's content, grouped by scope.
    ///
    /// Generated from the live table — this *is* the documentation, so it cannot drift.
    pub fn help(&self) -> Vec<(Scope, Vec<(String, &'static str)>)> {
        // The jump keys are one concept, collapsed to a range rather than listed individually —
        // and the range is read off the live table rather than written down, because a hardcoded
        // one silently went stale when a ninth section was added.
        let jump_range = {
            let mut numbers: Vec<u8> = self
                .bindings
                .values()
                .filter_map(|action| match action {
                    Action::JumpSection(n) => Some(*n),
                    _ => None,
                })
                .collect();
            numbers.sort_unstable();
            match (numbers.first(), numbers.last()) {
                (Some(first), Some(last)) if first != last => format!("{first}–{last}"),
                (Some(only), _) => only.to_string(),
                _ => String::new(),
            }
        };

        let mut grouped: BTreeMap<Scope, BTreeMap<Action, Vec<Binding>>> = BTreeMap::new();
        for (binding, action) in self.bindings.iter().chain(self.playback.iter()) {
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
                        if matches!(action, Action::JumpSection(_)) {
                            // One row for all of them, so it is labelled for all of them.
                            return (jump_range.clone(), "Go to section");
                        }
                        let rendered =
                            keys.iter().map(Binding::render).collect::<Vec<_>>().join(" / ");
                        (rendered, action.label())
                    })
                    .collect::<Vec<_>>();
                Some((scope, rows))
            })
            .collect()
    }

    /// Actions offered by the command palette, best match first, each with the key that runs
    /// it in `context` — the screen's, once the palette is closed. While something plays, `L`
    /// seeks rather than opening Logs, and the palette said `L` for Logs.
    ///
    /// The query matches as a subsequence, so `epsd` finds Episodes and `sett` finds Settings —
    /// a palette is typed fast and abbreviated, and substring matching made a missed letter
    /// mean no result at all. Ranked by [`match_rank`], ties kept in [`Action::ALL`]'s order.
    pub fn palette_entries(&self, query: &str, context: Context) -> Vec<(Action, String)> {
        let needle = query.trim().to_lowercase();
        // Driven off `Action::ALL`, not the binding table: an action with no key — Accounts and
        // Conflicts deliberately have none — must still be reachable, and the palette is the
        // only place it can be reached from.
        let mut ranked: Vec<(u32, usize, Action)> = Action::ALL
            .iter()
            .enumerate()
            .filter_map(|(order, action)| {
                if needle.is_empty() {
                    return Some((0, order, *action));
                }
                let by_label = match_rank(&needle, &action.label().to_lowercase());
                let by_key = match_rank(&needle, &action.config_key());
                let rank = match (by_label, by_key) {
                    (Some(a), Some(b)) => a.min(b),
                    (a, b) => a.or(b)?,
                };
                Some((rank, order, *action))
            })
            .collect();
        ranked.sort_unstable();
        ranked
            .into_iter()
            .map(|(_, _, action)| {
                let mut keys = self.keys_in(action, context);
                keys.sort();
                (action, keys.first().map(Binding::render).unwrap_or_default())
            })
            .collect()
    }
}

/// How well `needle` matches `haystack`, lower is better; `None` when it does not match.
///
/// Four tiers, so a near-exact hit is never outranked by a scattered one: a prefix, then a
/// whole-word start, then a substring, then a subsequence ranked by how spread out its letters
/// are. Both strings are expected lowercase.
fn match_rank(needle: &str, haystack: &str) -> Option<u32> {
    if haystack.starts_with(needle) {
        return Some(0);
    }
    if let Some(at) = haystack.find(needle) {
        let word_start = haystack[..at].ends_with([' ', '_', '/']);
        return Some(if word_start { 1 } else { 2 });
    }
    // Earliest-match subsequence. The span it covers, less the letters themselves, is the gap
    // the eye has to jump; the start breaks ties toward matches near the front.
    let mut chars = haystack.char_indices();
    let mut first = None;
    let mut last = 0;
    let mut letters = 0;
    for wanted in needle.chars().filter(|c| !c.is_whitespace()) {
        let (at, _) = chars.find(|(_, c)| *c == wanted)?;
        first.get_or_insert(at);
        last = at;
        letters += 1;
    }
    let first = first?;
    let gaps = (last - first + 1).saturating_sub(letters);
    Some(10 + 2 * gaps as u32 + first as u32)
}

/// The keys every overlay and text field answers to, whatever the rebinds are.
///
/// A rebind is a preference about browsing; it must not be able to take away the way out of a
/// dialog. With `back` moved to a letter, Esc stopped closing the command palette and the letter
/// was typed into it instead, so there was no way out at all short of quitting. So Esc always
/// cancels, Enter always confirms, the arrows always move, and ctrl+c always quits — and the
/// overlay hints can name these keys without asking the keymap.
fn modal_action(binding: Binding) -> Option<Action> {
    if binding.modifiers == KeyModifiers::CONTROL {
        return (binding.code == KeyCode::Char('c')).then_some(Action::Quit);
    }
    if !binding.modifiers.is_empty() {
        return None;
    }
    match binding.code {
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Enter => Some(Action::Open),
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Left => Some(Action::Left),
        KeyCode::Right => Some(Action::Right),
        KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::PageDown => Some(Action::PageDown),
        _ => None,
    }
}

/// The two or three contextual hints shown at the right of the status line.
///
/// Deliberately short. A full footer of bindings is the templated look this design avoids;
/// these are only the actions most likely to be wanted right now — and each has to be true
/// where it is shown, so they follow the state the keys are read in, not just the screen:
/// an open overlay shows its own keys, a text field shows no plain keys at all (they would be
/// typed), and Enter is named for what it does here. A narrow status line drops hints from the
/// front, so the last — `? keys`, or an overlay's way out — is the one that always shows.
pub fn status_hints(app: &crate::app::App) -> Vec<(String, &'static str)> {
    status_hints_paged(app, app.help_pages())
}

/// [`status_hints`], with the help table running to `help_pages`: the frame being drawn says,
/// once it has laid the table out, where the App still holds what the last frame said.
pub(crate) fn status_hints_paged(
    app: &crate::app::App,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::StageView as V;
    if let Some(overlay) = app.nav.overlay() {
        return overlay_hints(app, overlay, help_pages);
    }
    if app.is_typing() {
        // Search's field: letters are the query, so only the keys that cannot be typed are
        // offered. Esc and Enter are fixed for every field; see `modal_action`.
        let enter = if app.search_opens_on_enter() { "open" } else { "search" };
        let mut hints = vec![("↵".to_string(), enter)];
        // ↓ is how the keyboard gets from the field into the results; with none it does nothing.
        if !app.content.is_empty() {
            hints.push(("↓".to_string(), "results"));
        }
        if let Some(key) = app.keymap.keys_for(Action::FilterSearch).first() {
            hints.push((key.render(), Action::FilterSearch.hint()));
        }
        hints.push(("esc".to_string(), "back"));
        return hints;
    }

    let keymap = &app.keymap;
    // By the table the keys are read from now, so a hint is never a key the player has taken.
    let context = app.key_context();
    let key = |action: Action| keymap.keys_in(action, context).first().map(Binding::render);
    let in_stage = app.nav.focus() == crate::nav::Focus::Stage;
    let wanted: Vec<(Action, &'static str)> = match app.nav.current() {
        // Now Playing has its own vocabulary, and it is the one view where the browsing
        // hints would be actively wrong.
        V::NowPlaying => vec![
            (Action::PlayPause, Action::PlayPause.hint()),
            (Action::SkipOpening, Action::SkipOpening.hint()),
            (Action::Detach, Action::Detach.hint()),
        ],
        // Enter plays in the table; that is the table's whole point.
        V::Episodes(_) => vec![
            (Action::Open, "play"),
            (Action::ToggleWatched, Action::ToggleWatched.hint()),
            (Action::Back, Action::Back.hint()),
        ],
        // Enter on a title page goes one deeper, into its episodes. Space is play next only
        // while nothing plays; then it pauses the player, as on every other screen.
        V::Title(_) => vec![
            if app.playing.is_some() {
                (Action::PlayPause, Action::PlayPause.hint())
            } else {
                (Action::PlayNext, Action::PlayNext.hint())
            },
            (Action::Open, Action::ShowEpisodes.hint()),
            (Action::Back, Action::Back.hint()),
        ],
        V::Stores | V::Store(_) | V::StoreMembers(_) => {
            vec![(Action::Open, Action::Open.hint()), (Action::Back, Action::Back.hint())]
        }
        // Focus is on the rail: the arrows walk it, which is exactly the thing nobody could
        // discover when they did not.
        V::Section(_) if !in_stage => {
            vec![(Action::Down, Action::Down.hint()), (Action::Right, Action::Right.hint())]
        }
        V::Section(Section::Home) => vec![
            // A part-watched row resumes on Enter rather than opening.
            (Action::Open, if app.resumes_on_enter() { "play" } else { "open" }),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
        // The segment keys go first so they are dropped first; Enter is what a row is for.
        V::Section(Section::Library) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            if let (Some(left), Some(right)) = (key(Action::Left), key(Action::Right)) {
                hints.push((format!("{left}/{right}"), "list"));
            }
            hints.extend(key(Action::Open).map(|k| (k, "open")));
            hints.extend(key(Action::ShowEpisodes).map(|k| (k, Action::ShowEpisodes.hint())));
            return with_help(keymap, hints);
        }
        // Enter plays a finished download; nothing on this screen is an "open". Space and `x`
        // pause and cancel the highlighted row — the play-next and remove keys while idle, the
        // player's pause and stop while something plays — so they are named by whichever table
        // the keys are read from now, and only where the row can take them.
        V::Section(Section::Downloads) => {
            use anistream_model::downloads::DownloadState;
            let playing = app.playing.is_some();
            let (pause, cancel) = if playing {
                (Action::PlayPause, Action::StopPlayback)
            } else {
                (Action::PlayNext, Action::HideFromContinue)
            };
            let clear = (Action::ClearCompleted, Action::ClearCompleted.hint());
            match app.downloads.get(app.selected).map(|row| row.state) {
                Some(DownloadState::Done) => vec![
                    clear,
                    (Action::DeleteDownload, Action::DeleteDownload.hint()),
                    (Action::Open, "play"),
                ],
                Some(DownloadState::Failed) => vec![clear, (cancel, "cancel")],
                Some(DownloadState::Paused) => {
                    vec![clear, (cancel, "cancel"), (pause, "resume")]
                }
                Some(_) => vec![clear, (cancel, "cancel"), (pause, "pause")],
                None => Vec::new(),
            }
        }
        // Enter on Providers leads to the plugin stores; the table itself opens nothing. The
        // order keys are one hint, the way Settings names its pair, offered once there are two
        // sources to order, and first so a narrow line drops them before the way into the stores.
        V::Section(Section::Providers) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            if app.providers.len() > 1
                && let (Some(up), Some(down)) =
                    (key(Action::RaiseSource), key(Action::LowerSource))
            {
                hints.push((format!("{up}/{down}"), "order"));
            }
            hints.extend(key(Action::Refresh).map(|k| (k, Action::Refresh.hint())));
            hints.extend(key(Action::Open).map(|k| (k, "stores")));
            return with_help(keymap, hints);
        }
        // Enter signs the highlighted tracker in or out, so it is named for which.
        V::Section(Section::Accounts) => match app.sync.get(app.selected) {
            Some(tracker) if tracker.connected && !tracker.needs_reauth => {
                vec![(Action::Open, "sign out")]
            }
            Some(_) => vec![(Action::Open, "sign in")],
            None => Vec::new(),
        },
        // Settings has nothing to open and no episodes, so offering those keys would be a lie.
        // Left and Right are what actually do something here.
        V::Section(Section::Settings) => {
            let hints = match (key(Action::Left), key(Action::Right)) {
                (Some(left), Some(right)) => vec![(format!("{left}/{right}"), "change")],
                _ => Vec::new(),
            };
            return with_help(keymap, hints);
        }
        V::Section(_) => vec![
            (Action::Open, Action::Open.hint()),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
    };
    let hints =
        wanted.into_iter().filter_map(|(action, label)| Some((key(action)?, label))).collect();
    with_help(keymap, hints)
}

/// `? keys` last, so it is the hint a narrow status line keeps.
fn with_help(
    keymap: &Keymap,
    mut hints: Vec<(String, &'static str)>,
) -> Vec<(String, &'static str)> {
    if let Some(key) = keymap.keys_for(Action::Help).first() {
        hints.push((key.render(), Action::Help.hint()));
    }
    hints
}

/// An overlay's own hints. Its keys are the fixed ones from [`modal_action`], so they hold
/// whatever `[keys]` says. Esc goes last: it is the one a narrow line must keep.
fn overlay_hints(
    app: &crate::app::App,
    overlay: &crate::nav::Overlay,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::Overlay as O;
    let hint = |key: &str, label: &'static str| (key.to_string(), label);
    match overlay {
        O::Help if help_pages > 1 => vec![hint("↑↓", "page"), hint("esc", "close")],
        O::Help => vec![hint("esc", "close")],
        O::InviteLink => vec![hint("↵", "close")],
        O::Logs if app.logs.is_empty() => vec![hint("esc", "close")],
        O::Logs => vec![hint("↑↓", "move"), hint("esc", "close")],
        O::SearchFilters => {
            vec![hint("←→", "change"), hint("↵", "search"), hint("esc", "keep")]
        }
        O::CommandPalette => vec![hint("↑↓", "move"), hint("↵", "run"), hint("esc", "close")],
        other => {
            let verb = match other {
                O::ListStatus => "set",
                O::Rate => "rate",
                O::Sources => "play",
                O::SourceProvider => "pin",
                O::Disambiguate => "use",
                O::WatchOrder | O::Recommendations => "open",
                O::Accounts => "choose",
                O::Conflicts => "resolve",
                O::PluginAction => "choose",
                O::PublishPick => "publish",
                O::StoreConfirm => "confirm",
                O::ManualQuery => "search",
                O::DownloadRange => "queue",
                O::EditSetting => "save",
                O::KitsuUsername => "next",
                O::KitsuPassword => "sign in",
                O::JoinStore => "check",
                O::InviteName => "make link",
                _ => "choose",
            };
            vec![hint("↵", verb), hint("esc", "cancel")]
        }
    }
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
            ('K', Action::RaiseSource),
            ('J', Action::LowerSource),
        ] {
            assert_eq!(
                keymap.action_for(key(ch), Context::Browsing),
                Some(expected),
                "for {ch:?}"
            );
        }
    }

    #[test]
    fn the_order_keys_are_not_taken_while_something_plays() {
        // The player's table shadows browsing keys; `K` and `J` must reach Providers anyway.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key('K'), Context::Playing), Some(Action::RaiseSource));
        assert_eq!(keymap.action_for(key('J'), Context::Playing), Some(Action::LowerSource));
    }

    #[test]
    fn the_order_keys_have_a_providers_heading_and_palette_entries() {
        let keymap = Keymap::new();
        let providers = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Providers)
            .map(|(_, rows)| rows)
            .expect("a PROVIDERS group");
        assert_eq!(Scope::Providers.heading(), "PROVIDERS");
        assert_eq!(
            providers,
            [("K".to_string(), "Move source up"), ("J".to_string(), "Move source down")]
        );
        let entries = keymap.palette_entries("move source", Context::Browsing);
        assert!(entries.contains(&(Action::RaiseSource, "K".into())), "{entries:?}");
        assert!(entries.contains(&(Action::LowerSource, "J".into())), "{entries:?}");
        assert_eq!(Action::LowerSource.config_key(), "lower_source");
        assert_eq!(Action::RaiseSource.where_it_works(), "on the Providers screen");
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
        // Read off the live table, so adding a section cannot leave the overlay claiming a
        // range that stops short of it.
        let sections = crate::nav::Section::ALL.len();
        assert_eq!(jump_rows[0].0, format!("1–{sections}"));
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
        let all = keymap.palette_entries("", Context::Browsing);
        assert!(all.len() > 20, "palette should expose everything, got {}", all.len());

        let episodes = keymap.palette_entries("episode", Context::Browsing);
        assert!(episodes.iter().any(|(a, _)| *a == Action::ShowEpisodes));

        // Config-key search works too, so `show_episodes` finds it.
        assert!(
            keymap
                .palette_entries("show_ep", Context::Browsing)
                .iter()
                .any(|(a, _)| *a == Action::ShowEpisodes)
        );
        assert!(keymap.palette_entries("zzzz", Context::Browsing).is_empty());
    }

    #[test]
    fn the_palette_lists_every_action_bound_or_not() {
        // The palette is the app's discoverability mechanism, so an action missing from it is
        // an action nobody can reach. Accounts and Conflicts deliberately have no key — they
        // exist *only* here — which is why this cannot be driven off the binding table.
        let keymap = Keymap::new();
        let listed: Vec<Action> =
            keymap.palette_entries("", Context::Browsing).into_iter().map(|(a, _)| a).collect();
        for action in Action::ALL {
            assert!(listed.contains(action), "{action:?} is unreachable from the palette");
        }
        assert_eq!(listed.len(), Action::ALL.len(), "the palette listed something twice");
    }

    #[test]
    fn a_keyless_action_shows_no_key_rather_than_a_wrong_one() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("accounts", Context::Browsing);
        let (action, rendered) = entries.first().expect("Accounts is not in the palette");
        assert_eq!(*action, Action::ShowAccounts);
        assert!(rendered.is_empty(), "showed {rendered:?} for an unbound action");
    }

    #[test]
    fn a_bound_action_still_shows_its_key_in_the_palette() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("episodes", Context::Browsing);
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
        let entries = keymap.palette_entries("pause", Context::Browsing);
        let (_, rendered) = entries
            .iter()
            .find(|(a, _)| *a == Action::PlayPause)
            .expect("Play / pause is not in the palette");
        assert!(!rendered.is_empty(), "playback bindings must still display a key");
    }

    #[test]
    fn the_palette_names_no_key_the_player_has_taken() {
        // `L` opens Logs on a list and seeks thirty seconds on while an episode plays.
        let keymap = Keymap::new();
        let logs = |context| {
            keymap
                .palette_entries("logs", context)
                .into_iter()
                .find(|(a, _)| *a == Action::ShowLogs)
                .map(|(_, key)| key)
        };
        assert_eq!(logs(Context::Browsing).as_deref(), Some("L"));
        assert_eq!(logs(Context::Playing).as_deref(), Some(""), "no key opens Logs then");
    }

    #[test]
    fn config_keys_are_snake_case_and_unique() {
        let keymap = Keymap::new();
        let mut keys: Vec<String> = keymap
            .palette_entries("", Context::Browsing)
            .iter()
            .map(|(a, _)| a.config_key())
            .collect();
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
        use crate::nav::StageView;
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        let check = |app: &crate::app::App, what: &str| {
            let hints = status_hints(app);
            assert!(hints.len() <= 4, "{what} produced {} hints", hints.len());
            assert!(
                hints.last().is_some_and(|(k, _)| k == "?"),
                "{what}: discoverability depends on ? always being offered, last: {hints:?}"
            );
        };
        for section in Section::ALL {
            app.go_to_section(section);
            app.nav.focus_rail();
            check(&app, &format!("{section:?} rail"));
            app.nav.focus_stage();
            // Search's own view is a text field, where `?` would be typed; see the typing test.
            if !app.is_typing() {
                check(&app, &format!("{section:?} stage"));
            }
        }
        for view in [
            StageView::NowPlaying,
            StageView::Episodes(anistream_core::ids::AnilistId::new(1)),
            StageView::Title(anistream_core::ids::AnilistId::new(1)),
        ] {
            app.go_to_section(Section::Home);
            app.nav.push(view.clone());
            check(&app, &format!("{view:?}"));
        }
    }

    #[test]
    fn providers_offers_the_order_keys_as_one_hint_once_there_is_something_to_order() {
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.go_to_section(Section::Providers);
        app.nav.focus_stage();
        let row = |id: &str| anistream_model::sources::ProviderRow {
            id: id.into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        };
        app.apply(anistream_model::event::Event::Providers(vec![row("alpha")]));
        let hints = status_hints(&app);
        assert!(hints.iter().all(|(_, label)| *label != "order"), "{hints:?}");

        app.apply(anistream_model::event::Event::Providers(vec![row("alpha"), row("beta")]));
        let hints = status_hints(&app);
        assert_eq!(
            hints,
            [
                ("K/J".to_string(), "order"),
                ("r".to_string(), "refresh"),
                ("↵".to_string(), "stores"),
                ("?".to_string(), "keys"),
            ]
        );

        // Named by the live table, so a rebind shows.
        app.keymap.apply_overrides(&BTreeMap::from([("raise_source".into(), "U".into())]));
        assert_eq!(status_hints(&app)[0], ("U/J".to_string(), "order"));
    }

    #[test]
    fn a_title_page_under_playback_hints_the_keys_the_player_leaves() {
        // Space pauses and `q` detaches while something plays, so "space play next" and
        // "q back" were both wrong there.
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.nav.push(crate::nav::StageView::Title(anistream_core::ids::AnilistId::new(1)));
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "play next")), "{hints:?}");

        app.playing = Some(crate::app::NowPlaying::default());
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "pause")), "{hints:?}");
        assert!(hints.iter().all(|(_, label)| *label != "play next"), "{hints:?}");
        assert!(hints.contains(&("esc".into(), "Back")), "{hints:?}");
    }

    #[test]
    fn a_rebind_onto_a_key_another_action_keeps_is_refused_and_reported() {
        // `back = "x"` used to take `x` from *remove from continue watching* without a word,
        // leaving that action with no key at all.
        let mut keymap = Keymap::new();
        let rejected =
            keymap.apply_overrides(&BTreeMap::from([("back".to_string(), "x".to_string())]));
        assert_eq!(rejected.len(), 1, "{rejected:?}");
        assert!(rejected[0].contains("already bound to hide_from_continue"), "{rejected:?}");
        // Refused like an unknown name: nothing changed.
        assert_eq!(
            keymap.action_for(key('x'), Context::Browsing),
            Some(Action::HideFromContinue)
        );
        assert_eq!(keymap.action_for(key('q'), Context::Browsing), Some(Action::Back));
    }

    #[test]
    fn two_actions_can_swap_keys_when_both_are_rebound() {
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("back".to_string(), "x".to_string()),
            ("hide_from_continue".to_string(), "q".to_string()),
        ]));
        assert!(rejected.is_empty(), "{rejected:?}");
        assert_eq!(keymap.action_for(key('x'), Context::Browsing), Some(Action::Back));
        assert_eq!(
            keymap.action_for(key('q'), Context::Browsing),
            Some(Action::HideFromContinue)
        );
    }

    #[test]
    fn two_rebinds_asking_for_one_key_both_lose_it() {
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("refresh".to_string(), "z".to_string()),
            ("toggle_rail".to_string(), "z".to_string()),
        ]));
        assert_eq!(rejected.len(), 1, "{rejected:?}");
        assert!(rejected[0].contains("given to both"), "{rejected:?}");
        assert_eq!(keymap.action_for(key('z'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key('r'), Context::Browsing), Some(Action::Refresh));
    }

    #[test]
    fn a_refusal_releases_the_key_a_second_rebind_was_counting_on() {
        // `quit` cannot have `r`, so Refresh keeps it, so `reload` cannot have `R` freed for it
        // ... in short: a chain of rebinds settles, rather than half-applying.
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            // Wants `x`, which nobody frees.
            ("refresh".to_string(), "x".to_string()),
            // Wants `r`, which only the refused rebind above would have freed.
            ("toggle_translation".to_string(), "r".to_string()),
        ]));
        assert_eq!(rejected.len(), 2, "{rejected:?}");
        assert_eq!(keymap.action_for(key('r'), Context::Browsing), Some(Action::Refresh));
        assert_eq!(
            keymap.action_for(key('t'), Context::Browsing),
            Some(Action::ToggleTranslation)
        );
    }

    #[test]
    fn modals_keep_esc_enter_and_the_arrows_whatever_the_rebinds() {
        // With `back` moved to a letter, Esc stopped closing the palette and the letter was
        // typed into it: no way out at all.
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("back".to_string(), "z".to_string()),
            ("open".to_string(), "ctrl+o".to_string()),
            ("down".to_string(), "ctrl+n".to_string()),
        ]));
        assert!(rejected.is_empty(), "{rejected:?}");
        let special = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(keymap.action_for(special(KeyCode::Esc), Context::Browsing), None);
        assert_eq!(
            keymap.action_for(special(KeyCode::Esc), Context::Modal),
            Some(Action::Back)
        );
        assert_eq!(
            keymap.action_for(special(KeyCode::Enter), Context::Modal),
            Some(Action::Open)
        );
        assert_eq!(
            keymap.action_for(special(KeyCode::Down), Context::Modal),
            Some(Action::Down)
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(keymap.action_for(ctrl_c, Context::Modal), Some(Action::Quit));
        // The rebinds still work in a modal too.
        assert_eq!(keymap.action_for(key('z'), Context::Modal), Some(Action::Back));
    }

    #[test]
    fn a_modal_reads_the_browsing_table_not_the_playback_one() {
        // Over Now Playing, `q` in a dialog closes it rather than detaching the player.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key('q'), Context::Modal), Some(Action::Back));
        assert_eq!(keymap.action_for(key('q'), Context::Playing), Some(Action::Detach));
    }

    #[test]
    fn the_palette_matches_abbreviations_best_first() {
        let keymap = Keymap::new();
        let top = |query: &str| {
            keymap.palette_entries(query, Context::Browsing).first().map(|(a, _)| *a)
        };
        assert_eq!(top("epsd"), Some(Action::ShowEpisodes));
        assert_eq!(top("sett"), Some(Action::JumpSection(9)));
        assert_eq!(top("cal"), Some(Action::JumpSection(2)));
        assert_eq!(top("dwnld"), Some(Action::Download));
        // A word start outranks a scattered match of the same letters.
        assert_eq!(top("trailer"), Some(Action::PlayTrailer));
        assert!(keymap.palette_entries("qqqq", Context::Browsing).is_empty());
    }

    #[test]
    fn every_section_has_its_own_palette_command() {
        // One "Go to section" entry could only ever go to the first.
        let keymap = Keymap::new();
        let all: Vec<Action> =
            keymap.palette_entries("", Context::Browsing).into_iter().map(|(a, _)| a).collect();
        for n in 1..=Section::ALL.len() as u8 {
            assert!(all.contains(&Action::JumpSection(n)), "section {n} missing");
        }
        let (_, key) = keymap
            .palette_entries("settings", Context::Browsing)
            .into_iter()
            .find(|(a, _)| *a == Action::JumpSection(9))
            .expect("Go to Settings");
        assert_eq!(key, "9");
    }
}
