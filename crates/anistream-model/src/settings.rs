//! Settings: each one a front end shows or changes, named for what it is.

use anistream_core::config::Config;

/// Every setting a front end shows or changes, named for what it is rather than for where the
/// file keeps it: the Settings screen's rows, in display order in [`Self::ROWS`], and the one
/// with no row there, the source order.
///
/// An enum rather than positional indices: the cycling logic and the renderer both key off this,
/// so inserting a row cannot make the screen edit the setting below the one it is showing. And
/// rather than the file's table and key, which are where it is written and nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingKey {
    Theme,
    Motion,
    Translation,
    Quality,
    Subtitles,
    CommitThreshold,
    AutoNext,
    SkipOpening,
    SkipFiller,
    MarkChapters,
    Fullscreen,
    Upscaling,
    PlayerUi,
    NightMode,
    DownloadDir,
    MergeSubtitles,
    KeepSeeding,
    DownloadHook,
    Presence,
    PresenceTitle,
    Torrents,
    VpnMode,
    SeedRatio,
    SeedHours,
    UploadLimit,
    TokenStorage,
    Syncplay,
    SyncplayServer,
    SyncplayRoom,
    UpdateCheck,
    /// The order failover walks the sources in. No row on Settings: `K` and `J` on Providers
    /// move a source up and down it.
    SourceOrder,
}

impl SettingKey {
    /// The Settings screen's rows, in display order. Grouped by [`Self::category`] — the
    /// renderer draws a heading each time the category changes, so rows of one category must
    /// be contiguous here.
    pub const ROWS: [Self; 30] = [
        Self::Theme,
        Self::Motion,
        Self::Translation,
        Self::Quality,
        Self::Subtitles,
        Self::CommitThreshold,
        Self::AutoNext,
        Self::SkipOpening,
        Self::SkipFiller,
        Self::MarkChapters,
        Self::Fullscreen,
        Self::Upscaling,
        Self::PlayerUi,
        Self::NightMode,
        Self::DownloadDir,
        Self::MergeSubtitles,
        Self::KeepSeeding,
        Self::DownloadHook,
        Self::Torrents,
        Self::VpnMode,
        Self::SeedRatio,
        Self::SeedHours,
        Self::UploadLimit,
        Self::Presence,
        Self::PresenceTitle,
        Self::TokenStorage,
        Self::Syncplay,
        Self::SyncplayServer,
        Self::SyncplayRoom,
        Self::UpdateCheck,
    ];

    /// Where the file keeps it: the containing table, as the dotted path
    /// `anistream_core::settings::write_key` takes, and the key. `None` for the two shown and
    /// never written from here, the VPN mode and where tokens are kept: the file's to change.
    pub const fn path(self) -> Option<(&'static [&'static str], &'static str)> {
        Some(match self {
            Self::Theme => (&["theme"], "mode"),
            Self::Motion => (&["theme"], "motion"),
            Self::Translation => (&["playback"], "translation"),
            Self::Quality => (&["playback"], "quality"),
            Self::Subtitles => (&["playback"], "subtitle_language"),
            Self::CommitThreshold => (&["playback"], "commit_threshold"),
            Self::AutoNext => (&["playback"], "auto_next"),
            Self::SkipOpening => (&["playback"], "skip_opening"),
            Self::SkipFiller => (&["playback"], "skip_filler"),
            Self::MarkChapters => (&["playback"], "mark_chapters"),
            Self::Fullscreen => (&["playback"], "fullscreen"),
            Self::Upscaling => (&["playback"], "upscaling"),
            Self::PlayerUi => (&["playback"], "player_ui"),
            Self::NightMode => (&["playback"], "night_mode"),
            Self::DownloadDir => (&["downloads"], "directory"),
            Self::MergeSubtitles => (&["downloads"], "merge_subtitles"),
            Self::KeepSeeding => (&["downloads"], "keep_seeding"),
            Self::DownloadHook => (&["downloads"], "on_complete"),
            Self::Presence => (&["presence"], "enabled"),
            Self::PresenceTitle => (&["presence"], "show_title"),
            Self::Torrents => (&["providers", "torrent"], "enabled"),
            Self::SeedRatio => (&["providers", "torrent", "seeding"], "ratio"),
            Self::SeedHours => (&["providers", "torrent", "seeding"], "hours"),
            Self::UploadLimit => (&["providers", "torrent", "seeding"], "upload_limit_kib"),
            Self::Syncplay => (&["syncplay"], "enabled"),
            Self::SyncplayServer => (&["syncplay"], "server"),
            Self::SyncplayRoom => (&["syncplay"], "room"),
            Self::UpdateCheck => (&["updates"], "check"),
            Self::SourceOrder => (&["providers"], "order"),
            Self::VpnMode | Self::TokenStorage => return None,
        })
    }

    /// The setting the file keeps at `table`.`key`, when it is one a front end changes.
    pub fn at(table: &[&str], key: &str) -> Option<Self> {
        Self::ROWS
            .into_iter()
            .chain([Self::SourceOrder])
            .find(|setting| setting.path().is_some_and(|(t, k)| t == table && k == key))
    }

    /// Whether its value is typed rather than cycled. Everything else either has a closed set
    /// of values or is read-only on purpose.
    pub const fn typed(self) -> bool {
        matches!(
            self,
            Self::Subtitles
                | Self::DownloadDir
                | Self::DownloadHook
                | Self::SyncplayServer
                | Self::SyncplayRoom
        )
    }

    /// Copy this setting's value from one configuration to another.
    ///
    /// What lets one setting be put back to what the file holds after its write failed,
    /// without disturbing any other — including one changed a moment later whose own write
    /// worked.
    pub fn copy(self, from: &Config, to: &mut Config) {
        let (src, dst) = (&from.playback, &mut to.playback);
        match self {
            Self::Theme => to.theme.mode = from.theme.mode,
            Self::Motion => to.theme.motion = from.theme.motion,
            Self::Translation => dst.translation = src.translation,
            Self::Quality => dst.quality = src.quality,
            Self::Subtitles => dst.subtitle_language.clone_from(&src.subtitle_language),
            Self::CommitThreshold => dst.commit_threshold = src.commit_threshold,
            Self::AutoNext => dst.auto_next = src.auto_next,
            Self::SkipOpening => dst.skip_opening = src.skip_opening,
            Self::SkipFiller => dst.skip_filler = src.skip_filler,
            Self::MarkChapters => dst.mark_chapters = src.mark_chapters,
            Self::Fullscreen => dst.fullscreen = src.fullscreen,
            Self::Upscaling => dst.upscaling = src.upscaling,
            Self::PlayerUi => dst.player_ui = src.player_ui,
            Self::NightMode => dst.night_mode = src.night_mode,
            Self::DownloadDir => to.downloads.directory.clone_from(&from.downloads.directory),
            Self::MergeSubtitles => {
                to.downloads.merge_subtitles = from.downloads.merge_subtitles
            }
            Self::KeepSeeding => to.downloads.keep_seeding = from.downloads.keep_seeding,
            Self::DownloadHook => {
                to.downloads.on_complete.clone_from(&from.downloads.on_complete)
            }
            Self::Presence => to.presence.enabled = from.presence.enabled,
            Self::PresenceTitle => to.presence.show_title = from.presence.show_title,
            Self::Torrents => to.providers.torrent.enabled = from.providers.torrent.enabled,
            Self::SeedRatio => {
                to.providers.torrent.seeding.ratio = from.providers.torrent.seeding.ratio;
            }
            Self::SeedHours => {
                to.providers.torrent.seeding.hours = from.providers.torrent.seeding.hours;
            }
            Self::UploadLimit => {
                to.providers.torrent.seeding.upload_limit_kib =
                    from.providers.torrent.seeding.upload_limit_kib;
            }
            Self::Syncplay => to.syncplay.enabled = from.syncplay.enabled,
            Self::SyncplayServer => to.syncplay.server.clone_from(&from.syncplay.server),
            Self::SyncplayRoom => to.syncplay.room.clone_from(&from.syncplay.room),
            Self::UpdateCheck => to.updates.check = from.updates.check,
            Self::SourceOrder => to.providers.order.clone_from(&from.providers.order),
            // Not editable here, so never written and never rolled back.
            Self::VpnMode | Self::TokenStorage => {}
        }
    }

    /// Which heading a row sits under. Purely visual grouping — navigation walks the
    /// whole list, so nothing is ever hidden behind a tab.
    pub const fn category(self) -> &'static str {
        match self {
            Self::Theme | Self::Motion => "appearance",
            Self::Translation
            | Self::Quality
            | Self::Subtitles
            | Self::CommitThreshold
            | Self::AutoNext
            | Self::SkipOpening
            | Self::SkipFiller
            | Self::MarkChapters
            | Self::Fullscreen
            | Self::Upscaling
            | Self::PlayerUi
            | Self::NightMode => "playback",
            Self::DownloadDir
            | Self::MergeSubtitles
            | Self::KeepSeeding
            | Self::DownloadHook => "downloads",
            Self::Torrents
            | Self::VpnMode
            | Self::SeedRatio
            | Self::SeedHours
            | Self::UploadLimit
            | Self::SourceOrder => "sources",
            Self::Presence
            | Self::PresenceTitle
            | Self::TokenStorage
            | Self::Syncplay
            | Self::SyncplayServer
            | Self::SyncplayRoom
            | Self::UpdateCheck => "integrations",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Motion => "motion",
            Self::Translation => "translation",
            Self::Quality => "quality",
            Self::Subtitles => "subtitles",
            Self::CommitThreshold => "counts as watched at",
            Self::AutoNext => "auto-play next",
            // "offer to" was wrong: with this on, the opening is skipped, not proposed. mpv shows
            // a note on its own OSD as it happens, which is a courtesy, not a prompt.
            Self::SkipOpening => "skip opening automatically",
            Self::SkipFiller => "skip filler automatically",
            Self::MarkChapters => "mark opening and ending",
            Self::Fullscreen => "start fullscreen",
            Self::Upscaling => "upscaling",
            Self::PlayerUi => "player interface",
            Self::NightMode => "night mode",
            Self::DownloadDir => "download folder",
            Self::MergeSubtitles => "merge subtitles in",
            Self::KeepSeeding => "keep seeding",
            Self::DownloadHook => "after each download",
            Self::Syncplay => "syncplay parties",
            Self::SyncplayServer => "syncplay server",
            Self::SyncplayRoom => "syncplay room",
            Self::UpdateCheck => "check for updates",
            Self::Presence => "discord presence",
            Self::PresenceTitle => "presence shows the title",
            Self::Torrents => "torrent source",
            Self::SeedRatio => "seed after watching",
            Self::SeedHours => "seed for at most",
            Self::UploadLimit => "upload limit",
            Self::VpnMode => "vpn mode",
            Self::TokenStorage => "token storage",
            Self::SourceOrder => "source order",
        }
    }
}
