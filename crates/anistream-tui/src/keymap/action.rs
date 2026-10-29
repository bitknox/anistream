//! Everything the user can ask for, and the scopes the help overlay groups it under.
//!
//! What an [`Action`] is does not depend on the keys that reach it: its label, its terse
//! hint, its scope and the name `[keys]` knows it by are the same whatever the rebinds, so
//! they live here and the binding tables only map keys onto them.

use super::said;

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
    /// The calendar: back to the next broadcast, with today's ruling in view.
    Today,
    /// The calendar: to the start of this day, or of the day before when already there.
    PreviousDay,
    /// The calendar: to the first broadcast of a later day.
    NextDay,
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
    /// Play an episode with the watch party set up in the settings, instead of a private session.
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
        Self::Today,
        Self::PreviousDay,
        Self::NextDay,
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
    pub fn label(self) -> &'static str {
        use anistream_i18n::t;
        said(match self {
            Self::Help => t::keys_action_show_keys(),
            Self::CommandPalette => t::keys_action_command_palette(),
            Self::Quit => t::keys_action_quit(),
            Self::Back => t::keys_action_back(),
            Self::ToggleRail => t::keys_action_toggle_rail(),
            Self::Refresh => t::keys_action_refresh(),
            Self::ForceResync => t::keys_action_force_resync(),
            Self::ToggleTranslation => t::keys_action_toggle_translation(),
            // Named for the section, in the rail's order, so the palette can offer each one.
            Self::JumpSection(1) => t::keys_action_jump_continue(),
            Self::JumpSection(2) => t::keys_action_jump_calendar(),
            Self::JumpSection(3) => t::keys_action_jump_seasonal(),
            Self::JumpSection(4) => t::keys_action_jump_search(),
            Self::JumpSection(5) => t::keys_action_jump_library(),
            Self::JumpSection(6) => t::keys_action_jump_downloads(),
            Self::JumpSection(7) => t::keys_action_jump_sources(),
            Self::JumpSection(8) => t::keys_action_jump_accounts(),
            Self::JumpSection(9) => t::keys_action_jump_settings(),
            Self::JumpSection(_) => t::keys_action_jump_section(),
            Self::FocusSearch => t::keys_action_focus_search(),
            Self::ShowAccounts => t::keys_action_show_accounts(),
            Self::ShowConflicts => t::keys_action_show_conflicts(),
            Self::ShowLogs => t::keys_action_show_logs(),
            Self::OpenPrivacy => t::keys_action_open_privacy(),
            Self::ReportBug => t::keys_action_report_bug(),
            Self::Up => t::keys_action_up(),
            Self::Down => t::keys_action_down(),
            Self::Left => t::keys_action_left(),
            Self::Right => t::keys_action_right(),
            Self::Top => t::keys_action_top(),
            Self::Bottom => t::keys_action_bottom(),
            Self::Today => t::keys_action_today(),
            Self::PreviousDay => t::keys_action_previous_day(),
            Self::NextDay => t::keys_action_next_day(),
            Self::PageUp => t::keys_action_page_up(),
            Self::PageDown => t::keys_action_page_down(),
            Self::Open => t::keys_action_open(),
            Self::PlayNext => t::keys_action_play_next(),
            Self::SetListStatus => t::keys_action_set_list_status(),
            Self::RateTitle => t::keys_action_rate_title(),
            Self::ToggleSynopsis => t::keys_action_toggle_synopsis(),
            Self::HideFromContinue => t::keys_action_hide_from_continue(),
            Self::ShowEpisodes => t::keys_action_show_episodes(),
            Self::ShowSources => t::sources_title(),
            Self::PickProvider => t::source_picker_title(),
            Self::FixMapping => t::keys_action_fix_mapping(),
            Self::Download => t::keys_action_download(),
            Self::WatchOrder => t::keys_action_watch_order(),
            Self::Recommendations => t::keys_action_recommendations(),
            Self::PlayTrailer => t::keys_action_play_trailer(),
            Self::OpenInBrowser => t::keys_action_open_in_browser(),
            Self::ClearCompleted => t::keys_action_clear_completed(),
            Self::DeleteDownload => t::keys_action_delete_download(),
            Self::RaiseSource => t::keys_action_raise_source(),
            Self::LowerSource => t::keys_action_lower_source(),
            Self::DownloadRange => t::keys_action_download_range(),
            Self::PlayParty => t::keys_action_play_party(),
            Self::ToggleWatched => t::keys_action_toggle_watched(),
            Self::MarkAllPrevious => t::keys_action_mark_all_previous(),
            Self::Filter => t::keys_action_filter(),
            Self::FilterSearch => t::keys_action_filter_search(),
            Self::PlayPause => t::keys_action_play_pause(),
            Self::SeekBack => t::keys_action_seek_back(),
            Self::SeekForward => t::keys_action_seek_forward(),
            Self::SeekBackFar => t::keys_action_seek_back_far(),
            Self::SeekForwardFar => t::keys_action_seek_forward_far(),
            Self::NextEpisode => t::keys_action_next_episode(),
            Self::PreviousEpisode => t::keys_action_previous_episode(),
            Self::SkipOpening => t::keys_action_skip_opening(),
            Self::SpeedDown => t::keys_action_speed_down(),
            Self::SpeedUp => t::keys_action_speed_up(),
            Self::VolumeDown => t::keys_action_volume_down(),
            Self::VolumeUp => t::keys_action_volume_up(),
            Self::Fullscreen => t::keys_action_fullscreen(),
            Self::Detach => t::keys_action_detach(),
            Self::StopPlayback => t::keys_action_stop_playback(),
        })
    }

    /// The terse form used in the status line.
    ///
    /// The status line is not documentation — it has room for three hints, and a hint that gets
    /// dropped for being too long is worse than a curt one. "Leave player running" explains
    /// itself in the help overlay; here it is `detach`.
    pub fn hint(self) -> &'static str {
        use anistream_i18n::t;
        match self {
            Self::PlayPause => said(t::keys_hint_pause()),
            Self::SkipOpening => said(t::keys_hint_skip()),
            Self::Detach => said(t::keys_hint_detach()),
            Self::StopPlayback => said(t::keys_hint_stop()),
            Self::PlayNext => said(t::keys_hint_play_next()),
            Self::ShowEpisodes => said(t::keys_hint_episodes()),
            Self::OpenInBrowser => said(t::keys_hint_on_anilist()),
            Self::MarkAllPrevious => said(t::keys_hint_mark_previous()),
            Self::FilterSearch => said(t::keys_hint_filters()),
            Self::ForceResync => said(t::keys_hint_resync()),
            Self::Help => said(t::keys_hint_keys()),
            Self::Refresh => said(t::keys_hint_refresh()),
            Self::ToggleWatched => said(t::keys_hint_watched()),
            Self::ClearCompleted => said(t::keys_hint_clear_finished()),
            Self::DeleteDownload => said(t::keys_hint_delete()),
            Self::RaiseSource => said(t::keys_hint_move_up()),
            Self::LowerSource => said(t::keys_hint_move_down()),
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
            | Self::Today
            | Self::PreviousDay
            | Self::NextDay
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
    pub fn where_it_works(self) -> &'static str {
        use anistream_i18n::t;
        said(match self.scope() {
            Scope::Playback => t::keys_where_playback(),
            Scope::Downloads => t::keys_where_downloads(),
            Scope::Providers => t::keys_where_sources(),
            Scope::Episodes => t::keys_where_episodes(),
            _ => t::keys_where_elsewhere(),
        })
    }

    /// Stable machine name, used as the config key and the palette's search text.
    pub fn config_key(self) -> String {
        match self {
            Self::JumpSection(n) => format!("jump_section_{n}"), // i18n-skip
            other => {
                let label = format!("{other:?}"); // i18n-skip
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

    pub fn heading(self) -> &'static str {
        use anistream_i18n::t;
        said(match self {
            Self::Global => t::keys_scope_global(),
            Self::Movement => t::keys_scope_movement(),
            Self::Lists => t::keys_scope_lists(),
            Self::Title => t::keys_scope_title(),
            Self::Episodes => t::keys_scope_episodes(),
            Self::Downloads => t::keys_scope_downloads(),
            Self::Providers => t::keys_scope_sources(),
            Self::Playback => t::keys_scope_playback(),
        })
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
