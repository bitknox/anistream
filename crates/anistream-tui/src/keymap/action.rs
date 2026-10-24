//! Everything the user can ask for, and the scopes the help overlay groups it under.
//!
//! What an [`Action`] is does not depend on the keys that reach it: its label, its terse
//! hint, its scope and the name `[keys]` knows it by are the same whatever the rebinds, so
//! they live here and the binding tables only map keys onto them.

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
    /// The website's privacy page, in the browser: what anistream sends, and what it keeps.
    OpenPrivacy,
    /// A bug report: a pre-filled GitHub issue in the browser, with the last error in it.
    ReportBug,

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
        Self::OpenPrivacy,
        Self::ReportBug,
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
            Self::OpenPrivacy => "Open the privacy page",
            Self::ReportBug => "Report a bug",
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
            | Self::ShowLogs
            | Self::OpenPrivacy
            | Self::ReportBug => Scope::Global,

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

#[cfg(test)]
mod tests {
    use crate::keymap::{Context, Keymap};

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
}
