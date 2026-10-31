//! Settings: each one a front end shows or changes, named for what it is; the rules for what a
//! row shows and what a step makes of it; and what a front end is told of the config.
//!
//! The engine owns the config and runs these rules over it. A front end asks for a change with
//! a command and is told the outcome as a [`SettingsView`]: the config itself never crosses.

use anistream_core::{
    config::{Config, ThemeMode, TitleChoice, TrailerAutoplay},
    media::Translation,
    settings::SettingValue,
};

use crate::sources::ConfiguredSources;

/// Every setting a front end shows or changes, named for what it is rather than for where the
/// file keeps it: the Settings screen's rows, in display order in [`Self::ROWS`]; the native
/// apps' own, in [`Self::NATIVE_ROWS`]; and the one with no row on either, the source order.
///
/// An enum rather than positional indices: the cycling logic and the renderer both key off this,
/// so inserting a row cannot make the screen edit the setting below the one it is showing. And
/// rather than the file's table and key, which are where it is written and nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingKey {
    Theme,
    Motion,
    /// Which form of each title is shown: the signed-in AniList account's own choice, English
    /// or romaji. One setting for every front end, which shows what the engine names.
    Titles,
    /// When a title's trailer plays by itself in place of its banner: always, on Wi-Fi only, or
    /// never. The terminal shows it and plays none. See [`crate::trailer::autoplay`].
    Trailers,
    Translation,
    Quality,
    Subtitles,
    CommitThreshold,
    AutoNext,
    SkipOpening,
    SkipFiller,
    MarkChapters,
    /// Offer a skip button while the opening or ending plays, where the skip is not automatic.
    SkipButton,
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
    /// Discord's "Watching" with the cover, a progress bar, a link to AniList and the player's
    /// state. Needs the title shown, since every part of it names the show.
    PresenceRich,
    Torrents,
    /// The torrent index: the RSS search address the torrent source asks, which anistream
    /// ships none of. No row on the terminal's Settings screen, which leaves it to the file; a
    /// native app's torrent sheet types it, and reads it back from the configured sources.
    TorrentIndex,
    VpnMode,
    SeedRatio,
    SeedHours,
    UploadLimit,
    TokenStorage,
    Syncplay,
    SyncplayServer,
    SyncplayRoom,
    UpdateCheck,
    /// Whether the update check follows the beta channel. Off by default.
    UpdateBeta,
    /// The order failover walks the sources in. No row on Settings: `K` and `J` on Providers
    /// move a source up and down it.
    SourceOrder,
    /// How many seconds Up Next counts down before the next episode, `0` for none: a native
    /// app's row, as are the rest below.
    UpNextCountdown,
    /// Blur the stills of unwatched episodes after the next one.
    SpoilerProtection,
    /// Which version of an episode a download takes: standard or higher.
    DownloadQuality,
    /// How many of a show's next episodes downloads keep, `0` for none.
    KeepNextEpisodes,
    /// Whether downloads run over a cellular connection.
    DownloadsOnCellular,
    /// Whether torrents run over a cellular connection.
    TorrentsOnCellular,
    /// What a failed connection check does to torrents: pause them, stop them, or only warn.
    /// Warn needs an acknowledgement, which only [`Command::SetTorrentProtection`] carries.
    ///
    /// [`Command::SetTorrentProtection`]: crate::command::Command::SetTorrentProtection
    VpnOnFailure,
    /// Whether the app notifies of new episodes of the shows being watched, from
    /// [`Command::CheckNewEpisodes`]. Off by default. A show can turn it off for itself, or on
    /// when it is only planned, with [`ShowSettingKey::NotifyNewEpisodes`]; with this off, no
    /// show is notified of.
    ///
    /// [`Command::CheckNewEpisodes`]: crate::command::Command::CheckNewEpisodes
    /// [`ShowSettingKey::NotifyNewEpisodes`]: crate::show::ShowSettingKey::NotifyNewEpisodes
    EpisodeNotifications,
    /// The viewer's own mpv options, `playback.mpv_args`, each as mpv's command line takes it
    /// (`--sub-font-size=40`): passed after anistream's own, so one here wins over its flags.
    /// The Mac's Advanced table.
    MpvOptions,
    /// Whether Search and the lists show titles AniList marks adult, with its adult genre and
    /// tags among the filters. Off by default.
    AdultTitles,
    /// Episode discussions: an episode's AniList forum thread, looked up once it counts as
    /// watched. On by default.
    EpisodeDiscussions,
    /// Whether images in discussion comments load inline rather than as a line that opens them
    /// in the browser. Off by default; shown while discussions are on.
    CommentImages,
    /// Drive the app with a connected game controller. On by default; the Linux app's, and any
    /// other that takes one.
    ControllerInput,
    /// Swap A and B, for a controller laid out as Nintendo's are. Off by default.
    SwapFaceButtons,
    /// Whether Calendar shows only the shows on the person's list rather than everything
    /// airing. Off by default, so Calendar opens on All, and remembered from the last pick.
    CalendarMine,
    /// Whether the TV picker lists TVs with Chromecast built in, and looks for them. On by
    /// default; under Play on TV.
    CastTvs,
    /// Whether a cast plays from torrents, which this device serves for the whole episode. Off
    /// on phones and tablets and on elsewhere, until chosen; under Sources, shown while
    /// torrents are on. Off, a torrent is passed over for a cast and the next source tried.
    TorrentsWhileCasting,
    /// Whether a cover the pointer rests on grows into a card with the title's details and its
    /// trailer, where trailers may play: the Mac's grids, and the desktop apps' later. On by
    /// default. With it off, a cover opens its title on a click and shows nothing on hover.
    HoverCards,
}

impl SettingKey {
    /// The Settings screen's rows, in display order. Grouped by [`Self::category`] — the
    /// renderer draws a heading each time the category changes, so rows of one category must
    /// be contiguous here.
    pub const ROWS: [Self; 35] = [
        Self::Theme,
        Self::Motion,
        Self::Titles,
        Self::Trailers,
        Self::Translation,
        Self::Quality,
        Self::Subtitles,
        Self::CommitThreshold,
        Self::AutoNext,
        Self::SkipOpening,
        Self::SkipFiller,
        Self::MarkChapters,
        Self::SkipButton,
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
        Self::PresenceRich,
        Self::TokenStorage,
        Self::Syncplay,
        Self::SyncplayServer,
        Self::SyncplayRoom,
        Self::UpdateCheck,
        Self::UpdateBeta,
    ];

    /// The native apps' own rows, which the terminal's Settings screen does not show: a front
    /// end finds the rows both show among [`Self::ROWS`]. In no order of their own, since each
    /// app lays them out on its own panes; grouped by [`Self::category`] all the same.
    pub const NATIVE_ROWS: [Self; 18] = [
        Self::UpNextCountdown,
        Self::SpoilerProtection,
        Self::DownloadQuality,
        Self::KeepNextEpisodes,
        Self::DownloadsOnCellular,
        Self::TorrentsOnCellular,
        Self::VpnOnFailure,
        Self::EpisodeNotifications,
        Self::MpvOptions,
        Self::AdultTitles,
        Self::EpisodeDiscussions,
        Self::CommentImages,
        Self::ControllerInput,
        Self::SwapFaceButtons,
        Self::CalendarMine,
        Self::CastTvs,
        Self::TorrentsWhileCasting,
        Self::HoverCards,
    ];

    /// Where the file keeps it: the containing table, as the dotted path
    /// `anistream_core::settings::write_key` takes, and the key. `None` for the two shown and
    /// never written from here, the VPN mode and where tokens are kept: the file's to change.
    pub const fn path(self) -> Option<(&'static [&'static str], &'static str)> {
        Some(match self {
            Self::Theme => (&["theme"], "mode"),
            Self::Motion => (&["theme"], "motion"),
            Self::Titles => (&["meta"], "title_language"),
            Self::Trailers => (&["theme"], "trailers"),
            Self::AdultTitles => (&["meta"], "adult"),
            Self::EpisodeDiscussions => (&["meta"], "discussions"),
            Self::CommentImages => (&["meta"], "discussion_images"),
            Self::ControllerInput => (&["controller"], "enabled"),
            Self::SwapFaceButtons => (&["controller"], "swap_face"),
            Self::CalendarMine => (&["meta"], "calendar_mine"),
            Self::CastTvs => (&["cast"], "show_tvs"),
            Self::TorrentsWhileCasting => (&["providers", "torrent"], "while_casting"),
            Self::HoverCards => (&["theme"], "hover_cards"),
            Self::Translation => (&["playback"], "translation"),
            Self::Quality => (&["playback"], "quality"),
            Self::Subtitles => (&["playback"], "subtitle_language"),
            Self::CommitThreshold => (&["playback"], "commit_threshold"),
            Self::AutoNext => (&["playback"], "auto_next"),
            Self::SkipOpening => (&["playback"], "skip_opening"),
            Self::SkipFiller => (&["playback"], "skip_filler"),
            Self::MarkChapters => (&["playback"], "mark_chapters"),
            Self::SkipButton => (&["playback"], "skip_button"),
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
            Self::PresenceRich => (&["presence"], "rich"),
            Self::Torrents => (&["providers", "torrent"], "enabled"),
            Self::TorrentIndex => (&["providers", "torrent"], "rss_url"),
            Self::SeedRatio => (&["providers", "torrent", "seeding"], "ratio"),
            Self::SeedHours => (&["providers", "torrent", "seeding"], "hours"),
            Self::UploadLimit => (&["providers", "torrent", "seeding"], "upload_limit_kib"),
            Self::Syncplay => (&["syncplay"], "enabled"),
            Self::SyncplayServer => (&["syncplay"], "server"),
            Self::SyncplayRoom => (&["syncplay"], "room"),
            Self::UpdateCheck => (&["updates"], "check"),
            Self::UpdateBeta => (&["updates"], "beta"),
            Self::SourceOrder => (&["providers"], "order"),
            Self::UpNextCountdown => (&["playback"], "up_next_countdown"),
            Self::SpoilerProtection => (&["playback"], "spoiler_protection"),
            Self::DownloadQuality => (&["downloads"], "quality"),
            Self::KeepNextEpisodes => (&["downloads"], "keep_next"),
            Self::DownloadsOnCellular => (&["downloads"], "on_cellular"),
            Self::TorrentsOnCellular => (&["providers", "torrent"], "on_cellular"),
            Self::VpnOnFailure => (&["providers", "torrent", "vpn"], "on_leak"),
            Self::EpisodeNotifications => (&["notifications"], "episodes"),
            Self::MpvOptions => (&["playback"], "mpv_args"),
            Self::VpnMode | Self::TokenStorage => return None,
        })
    }

    /// The setting the file keeps at `table`.`key`, when it is one a front end changes.
    pub fn at(table: &[&str], key: &str) -> Option<Self> {
        Self::ROWS
            .into_iter()
            .chain(Self::NATIVE_ROWS)
            .chain([Self::SourceOrder, Self::TorrentIndex])
            .find(|setting| setting.path().is_some_and(|(t, k)| t == table && k == key))
    }

    /// Whether its value is typed rather than cycled. Everything else either has a closed set
    /// of values or is read-only on purpose.
    pub const fn typed(self) -> bool {
        matches!(
            self,
            Self::Subtitles
                | Self::TorrentIndex
                | Self::DownloadDir
                | Self::DownloadHook
                | Self::SyncplayServer
                | Self::SyncplayRoom
                | Self::MpvOptions
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
            Self::Titles => to.meta.title_language = from.meta.title_language,
            Self::Trailers => to.theme.trailers = from.theme.trailers,
            Self::Translation => dst.translation = src.translation,
            Self::Quality => dst.quality = src.quality,
            Self::Subtitles => dst.subtitle_language.clone_from(&src.subtitle_language),
            Self::CommitThreshold => dst.commit_threshold = src.commit_threshold,
            Self::AutoNext => dst.auto_next = src.auto_next,
            Self::SkipOpening => dst.skip_opening = src.skip_opening,
            Self::SkipFiller => dst.skip_filler = src.skip_filler,
            Self::MarkChapters => dst.mark_chapters = src.mark_chapters,
            Self::SkipButton => dst.skip_button = src.skip_button,
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
            Self::PresenceRich => to.presence.rich = from.presence.rich,
            Self::Torrents => to.providers.torrent.enabled = from.providers.torrent.enabled,
            Self::TorrentIndex => {
                to.providers.torrent.rss_url.clone_from(&from.providers.torrent.rss_url);
            }
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
            Self::UpdateBeta => to.updates.beta = from.updates.beta,
            Self::AdultTitles => to.meta.adult = from.meta.adult,
            Self::EpisodeDiscussions => to.meta.discussions = from.meta.discussions,
            Self::CommentImages => to.meta.discussion_images = from.meta.discussion_images,
            Self::ControllerInput => to.controller.enabled = from.controller.enabled,
            Self::SwapFaceButtons => to.controller.swap_face = from.controller.swap_face,
            Self::CalendarMine => to.meta.calendar_mine = from.meta.calendar_mine,
            Self::CastTvs => to.cast.show_tvs = from.cast.show_tvs,
            Self::TorrentsWhileCasting => {
                to.providers.torrent.while_casting = from.providers.torrent.while_casting;
            }
            Self::HoverCards => to.theme.hover_cards = from.theme.hover_cards,
            Self::MpvOptions => dst.mpv_args.clone_from(&src.mpv_args),
            Self::SourceOrder => to.providers.order.clone_from(&from.providers.order),
            Self::UpNextCountdown => dst.up_next_countdown = src.up_next_countdown,
            Self::SpoilerProtection => dst.spoiler_protection = src.spoiler_protection,
            Self::DownloadQuality => to.downloads.quality = from.downloads.quality,
            Self::KeepNextEpisodes => to.downloads.keep_next = from.downloads.keep_next,
            Self::DownloadsOnCellular => to.downloads.on_cellular = from.downloads.on_cellular,
            Self::TorrentsOnCellular => {
                to.providers.torrent.on_cellular = from.providers.torrent.on_cellular;
            }
            Self::VpnOnFailure => {
                to.providers.torrent.vpn.on_leak = from.providers.torrent.vpn.on_leak;
            }
            Self::EpisodeNotifications => {
                to.notifications.episodes = from.notifications.episodes;
            }
            // Not editable here, so never written and never rolled back.
            Self::VpnMode | Self::TokenStorage => {}
        }
    }

    /// Which heading a row sits under. Purely visual grouping — navigation walks the
    /// whole list, so nothing is ever hidden behind a tab.
    pub const fn category(self) -> &'static str {
        match self {
            Self::Theme
            | Self::Motion
            | Self::Titles
            | Self::Trailers
            | Self::AdultTitles
            | Self::ControllerInput
            | Self::SwapFaceButtons
            | Self::CalendarMine
            | Self::HoverCards => "appearance",
            Self::Translation
            | Self::Quality
            | Self::Subtitles
            | Self::CommitThreshold
            | Self::AutoNext
            | Self::SkipOpening
            | Self::SkipFiller
            | Self::MarkChapters
            | Self::SkipButton
            | Self::Fullscreen
            | Self::Upscaling
            | Self::PlayerUi
            | Self::NightMode
            | Self::UpNextCountdown
            | Self::SpoilerProtection => "playback",
            Self::DownloadDir
            | Self::MergeSubtitles
            | Self::KeepSeeding
            | Self::DownloadHook
            | Self::DownloadQuality
            | Self::KeepNextEpisodes
            | Self::DownloadsOnCellular => "downloads",
            Self::Torrents
            | Self::TorrentIndex
            | Self::VpnMode
            | Self::SeedRatio
            | Self::SeedHours
            | Self::UploadLimit
            | Self::SourceOrder
            | Self::TorrentsOnCellular
            | Self::TorrentsWhileCasting
            | Self::VpnOnFailure => "sources",
            Self::Presence
            | Self::PresenceTitle
            | Self::PresenceRich
            | Self::TokenStorage
            | Self::Syncplay
            | Self::SyncplayServer
            | Self::SyncplayRoom
            | Self::UpdateCheck
            | Self::UpdateBeta
            | Self::EpisodeDiscussions
            | Self::CommentImages
            | Self::CastTvs => "integrations",
            Self::EpisodeNotifications => "notifications",
            Self::MpvOptions => "advanced",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Motion => "motion",
            Self::Titles => "titles",
            Self::Trailers => "play trailers",
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
            Self::SkipButton => "show skip button",
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
            Self::UpdateBeta => "beta updates",
            Self::Presence => "discord presence",
            Self::PresenceTitle => "presence shows the title",
            Self::PresenceRich => "rich presence",
            Self::Torrents => "torrent source",
            Self::TorrentIndex => "torrent index",
            Self::SeedRatio => "seed after watching",
            Self::SeedHours => "seed for at most",
            Self::UploadLimit => "upload limit",
            Self::VpnMode => "vpn mode",
            Self::TokenStorage => "token storage",
            Self::SourceOrder => "source order",
            Self::UpNextCountdown => "up next countdown",
            Self::SpoilerProtection => "spoiler protection",
            // Not "quality", which is playback's: the toast that says it was saved would not
            // say which.
            Self::DownloadQuality => "download quality",
            Self::KeepNextEpisodes => "keep next episodes",
            Self::DownloadsOnCellular => "downloads on cellular",
            Self::TorrentsOnCellular => "torrents on cellular",
            Self::VpnOnFailure => "if a check fails",
            Self::AdultTitles => "adult titles",
            Self::EpisodeDiscussions => "episode discussions",
            Self::CommentImages => "load images in comments",
            Self::ControllerInput => "use a controller",
            Self::SwapFaceButtons => "swap a and b",
            Self::CalendarMine => "calendar shows only my list",
            Self::CastTvs => "show tvs with chromecast built in",
            Self::TorrentsWhileCasting => "torrents while casting",
            Self::HoverCards => "expand covers on hover",
            Self::EpisodeNotifications => "new episode notifications",
            Self::MpvOptions => "mpv options",
        }
    }
}

/// Whether a cast plays from torrents. The engine fills the setting in at launch from the
/// device it runs on; unset here is a computer's default, on.
pub fn while_casting(config: &Config) -> bool {
    config.providers.torrent.while_casting.unwrap_or(true)
}

/// Whether writing this setting changes which sources exist, and so needs the chain rebuilt.
///
/// Listed rather than inferred: a rebuild tears down a live torrent session, which is far
/// too blunt to trigger on a guess about a setting. The VPN mode is listed though nothing here
/// writes it, since it decides whether torrents run at all; the sources switched off by name
/// and the remote's address, which the file alone sets, have no key to list. What a failed
/// check does is listed too: the guard reads it as the source comes up, and a choice to stop
/// rather than pause that waited for the next launch would not be the one in force.
pub fn redefines_sources(setting: SettingKey) -> bool {
    // The index too: the torrent source reads it as it comes up.
    matches!(
        setting,
        SettingKey::Torrents
            | SettingKey::TorrentIndex
            | SettingKey::VpnMode
            | SettingKey::SourceOrder
            | SettingKey::VpnOnFailure
    )
}

// The Settings screen's rules, over the config in effect and the file's: what each row shows and
// how it changes, and what a step or a typed value makes of a setting.
//
// Pure, so they can run wherever the config is owned, and a front end shows what they say: the
// ladders, the two-state values and their ends, and the wording of each value.

/// What a front end is told of the config: the Settings screen's rows, and everything else it
/// shows or acts on that the config decides.
///
/// Sent whole whenever any of it changes, so a front end never holds a config of its own: what
/// it shows is what the engine has in effect.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingsView {
    /// The Settings screen's rows, in display order.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<SettingRow>,
    /// The setting whose write was just refused and put back to what the file holds: on the
    /// view that answers the refusal, and on no other. The notice saying why comes after it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub refused: Option<SettingKey>,
    /// Sub or dub, in effect: what the status line says, and what `t` turns over.
    pub translation: Translation,
    /// The quality asked of the sources, in effect, for the status line.
    pub quality: u32,
    /// The theme in effect, which the terminal paints with.
    pub theme: ThemeMode,
    /// Whether the front end may animate: off, the terminal's wipe cuts straight to its card and
    /// its pulses hold still.
    #[cfg_attr(feature = "ffi", uniffi(default = true))]
    pub motion: bool,
    /// The sources the config sets up: the header's chips, the source the status line names,
    /// and the order Providers moves a source up and down.
    pub sources: ConfiguredSources,
    /// The speed a playback starts at, when one is remembered: what Now Playing shows of a
    /// playback the front end stages as it asks for it, until the player says. One the engine
    /// starts on its own comes staged with it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub speed: Option<f64>,
    /// Whether Discord presence has a client to publish under, which its row's note depends on.
    pub presence_client: bool,
    /// Whether this engine can publish a presence at all: it is a desktop's, not a phone's. A
    /// front end shows no Discord setting or preview where it is not. The terminal's own is
    /// always so; the engine says what it was built with.
    #[cfg_attr(feature = "ffi", uniffi(default = true))]
    pub presence_available: bool,
    /// The native apps' own rows, in [`SettingKey::NATIVE_ROWS`] order. The terminal draws
    /// none of them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub native_rows: Vec<SettingRow>,
    /// The settings some shows set for themselves, and for how many: what "Overridden for 3
    /// shows" says under a row. Only those at least one show sets, each once.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub overridden: Vec<SettingOverride>,
    /// How many shows switch new-episode notifications on for themselves, which notify whether
    /// [`SettingKey::EpisodeNotifications`] is on or not. A native app keeps its background
    /// check scheduled while either is, and cancels it only when neither is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub notify_switched_on: u32,
    /// How torrent traffic is kept off the viewer's own address, as the torrent sheet and
    /// Sources show it. Boxed, as the largest part of a view every settings event carries:
    /// the bindings see the view itself.
    pub torrent: Box<crate::protection::ProtectionView>,
}

/// How many shows set a setting for themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingOverride {
    /// The setting: the Settings row the count goes under.
    pub key: SettingKey,
    /// How many shows set it, one or more.
    pub shows: u32,
}

impl Default for SettingsView {
    /// The view of the defaults, in effect and in the file alike.
    fn default() -> Self {
        let config = Config::default();
        view(&config, &config)
    }
}

/// What a front end is told of the config, from the config in effect and the file's.
pub fn view(effective: &Config, saved: &Config) -> SettingsView {
    SettingsView {
        rows: rows(effective, saved),
        refused: None,
        translation: effective.playback.translation,
        quality: effective.playback.quality,
        theme: effective.theme.mode,
        motion: effective.theme.motion,
        sources: ConfiguredSources::of(&effective.providers),
        speed: effective.playback.persisted_speed,
        presence_client: effective.presence.resolved_client_id().is_some(),
        presence_available: true,
        native_rows: native_rows(effective, saved),
        // The shows' own are the database's, which the engine counts and puts in.
        overridden: Vec::new(),
        notify_switched_on: 0,
        torrent: Box::new(crate::protection::view(&effective.providers.torrent.vpn)),
    }
}

/// One row of the Settings screen, as the config makes it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingRow {
    /// The setting it shows.
    pub key: SettingKey,
    /// Its value as the row shows it: `on`, `1080p`, `adaptive · immersive this session`.
    pub value: String,
    /// How it is changed from here.
    pub change: SettingChange,
}

/// How a row's value is changed.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingChange {
    /// Stepped through a closed set of values: a ladder, or two states. `back` and `forward`
    /// say whether a step that way would change it — not at a ladder's end, and not toward
    /// the side a two-state value already stands on.
    Steps { back: bool, forward: bool },
    /// Typed, starting from `text`: too many values to step through.
    Typed { text: String },
    /// Not changed from here: the file is where, on purpose.
    Fixed,
}

impl SettingChange {
    /// Whether the row can be changed from here at all.
    pub fn editable(&self) -> bool {
        !matches!(self, Self::Fixed)
    }
}

/// The Settings screen's rows, in display order, from the config in effect and the file's.
///
/// Each shows the value in effect, so a row says what is actually in force the instant after
/// it is changed. The two that can also be changed for a session only, theme and sub/dub,
/// show what is saved and mark the session's value beside it: the file and the next launch
/// go by the saved one.
pub fn rows(effective: &Config, saved: &Config) -> Vec<SettingRow> {
    SettingKey::ROWS.into_iter().map(|key| row(effective, saved, key)).collect()
}

/// The native apps' own rows, in [`SettingKey::NATIVE_ROWS`] order, as [`rows`] makes the
/// Settings screen's.
pub fn native_rows(effective: &Config, saved: &Config) -> Vec<SettingRow> {
    SettingKey::NATIVE_ROWS.into_iter().map(|key| row(effective, saved, key)).collect()
}

fn row(effective: &Config, saved: &Config, key: SettingKey) -> SettingRow {
    SettingRow {
        key,
        value: value(effective, saved, key),
        change: change(effective, saved, key),
    }
}

/// The value a step of `delta` makes of `key`, as the file would be given it, or `None` for
/// a setting no step changes: typed, read-only, or the order.
///
/// A ladder steps one rung toward `delta`'s sign and stops at its ends, where the value comes
/// back as it is; a two-state value flips whichever way. Theme and sub/dub step from the
/// saved value, the one their row leads with: the arrows change what is saved, and a session
/// override simply gives way to it.
pub fn step(
    effective: &Config,
    saved: &Config,
    key: SettingKey,
    delta: i64,
) -> Option<SettingValue> {
    use SettingValue as V;
    use anistream_core::config::{DownloadQuality, LeakAction, PlayerUi, ThemeMode, Upscaling};
    use anistream_core::media::Translation;

    let playback = &effective.playback;
    let seeding = &effective.providers.torrent.seeding;
    Some(match key {
        // Round the four, either way: adaptive first, as it is the one most want.
        SettingKey::Theme => {
            const RING: [ThemeMode; 4] =
                [ThemeMode::Adaptive, ThemeMode::Immersive, ThemeMode::Light, ThemeMode::Dark];
            let at = RING.iter().position(|m| *m == saved.theme.mode).unwrap_or(0) as i64;
            let next = RING[(at + delta.signum()).rem_euclid(RING.len() as i64) as usize];
            V::Str(format!("{next:?}").to_lowercase())
        }
        SettingKey::Motion => V::Bool(!effective.theme.motion),
        SettingKey::Titles => {
            const LADDER: [TitleChoice; 3] =
                [TitleChoice::Anilist, TitleChoice::English, TitleChoice::Romaji];
            let current = effective.meta.title_language;
            let at = LADDER.iter().position(|c| *c == current).unwrap_or(0) as i64;
            V::Str(LADDER[(at + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::Trailers => {
            const LADDER: [TrailerAutoplay; 3] = TrailerAutoplay::ALL;
            let at = LADDER.iter().position(|c| *c == effective.theme.trailers).unwrap_or(0);
            V::Str(LADDER[(at as i64 + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::Translation => V::Str(
            match saved.playback.translation {
                Translation::Sub => Translation::Dub,
                Translation::Dub => Translation::Sub,
            }
            .to_string(),
        ),
        SettingKey::Quality => {
            const LADDER: [u32; 4] = [480, 720, 1080, 2160];
            V::Int(i64::from(step_through(&LADDER, playback.quality, delta)))
        }
        SettingKey::CommitThreshold => {
            const LADDER: [u32; 5] = [70, 80, 85, 90, 95];
            let current = (playback.commit_threshold * 100.0).round() as u32;
            V::Float(f64::from(step_through(&LADDER, current, delta)) / 100.0)
        }
        SettingKey::AutoNext => V::Bool(!playback.auto_next),
        SettingKey::Upscaling => {
            const LADDER: [Upscaling; 3] =
                [Upscaling::Off, Upscaling::Natural, Upscaling::Sharp];
            let at = LADDER.iter().position(|u| *u == playback.upscaling).unwrap_or(0) as i64;
            V::Str(LADDER[(at + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::PlayerUi => V::Str(
            match playback.player_ui {
                PlayerUi::Anistream => "mpv",
                PlayerUi::Mpv => "anistream",
            }
            .into(),
        ),
        SettingKey::NightMode => V::Bool(!playback.night_mode),
        SettingKey::MergeSubtitles => V::Bool(!effective.downloads.merge_subtitles),
        SettingKey::KeepSeeding => V::Bool(!effective.downloads.keep_seeding),
        SettingKey::Syncplay => V::Bool(!effective.syncplay.enabled),
        SettingKey::UpdateCheck => V::Bool(!effective.updates.check),
        SettingKey::UpdateBeta => V::Bool(!effective.updates.beta),
        SettingKey::SkipOpening => V::Bool(!playback.skip_opening),
        SettingKey::SkipFiller => V::Bool(!playback.skip_filler),
        SettingKey::MarkChapters => V::Bool(!playback.mark_chapters),
        SettingKey::SkipButton => V::Bool(!playback.skip_button),
        SettingKey::Fullscreen => V::Bool(!playback.fullscreen),
        SettingKey::Presence => V::Bool(!effective.presence.enabled),
        SettingKey::PresenceTitle => V::Bool(!effective.presence.show_title),
        SettingKey::PresenceRich => V::Bool(!effective.presence.rich),
        SettingKey::Torrents => V::Bool(!effective.providers.torrent.enabled),
        SettingKey::SeedRatio => {
            // Tenths, so the ladder can be matched exactly rather than as floats.
            const LADDER: [u32; 5] = [0, 5, 10, 15, 20];
            let current = (seeding.ratio * 10.0).round() as u32;
            V::Float(f64::from(step_through(&LADDER, current, delta)) / 10.0)
        }
        SettingKey::SeedHours => {
            const LADDER: [u32; 5] = [1, 6, 12, 24, 72];
            V::Int(i64::from(step_through(&LADDER, seeding.hours, delta)))
        }
        SettingKey::UploadLimit => {
            const LADDER: [u32; 6] = [0, 256, 512, 1024, 2048, 5120];
            V::Int(i64::from(step_through(&LADDER, seeding.upload_limit_kib, delta)))
        }
        SettingKey::UpNextCountdown => {
            const LADDER: [u32; 6] = [0, 5, 10, 15, 20, 30];
            V::Int(i64::from(step_through(&LADDER, playback.up_next_countdown, delta)))
        }
        SettingKey::SpoilerProtection => V::Bool(!playback.spoiler_protection),
        SettingKey::DownloadQuality => V::Str(
            match effective.downloads.quality {
                DownloadQuality::Standard => "higher",
                DownloadQuality::Higher => "standard",
            }
            .into(),
        ),
        SettingKey::KeepNextEpisodes => {
            const LADDER: [u32; 6] = [0, 1, 2, 3, 5, 10];
            V::Int(i64::from(step_through(&LADDER, effective.downloads.keep_next, delta)))
        }
        SettingKey::DownloadsOnCellular => V::Bool(!effective.downloads.on_cellular),
        SettingKey::TorrentsOnCellular => V::Bool(!effective.providers.torrent.on_cellular),
        SettingKey::EpisodeNotifications => V::Bool(!effective.notifications.episodes),
        SettingKey::AdultTitles => V::Bool(!effective.meta.adult),
        SettingKey::EpisodeDiscussions => V::Bool(!effective.meta.discussions),
        SettingKey::CommentImages => V::Bool(!effective.meta.discussion_images),
        SettingKey::ControllerInput => V::Bool(!effective.controller.enabled),
        SettingKey::SwapFaceButtons => V::Bool(!effective.controller.swap_face),
        SettingKey::CalendarMine => V::Bool(!effective.meta.calendar_mine),
        SettingKey::CastTvs => V::Bool(!effective.cast.show_tvs),
        SettingKey::TorrentsWhileCasting => V::Bool(!while_casting(effective)),
        SettingKey::HoverCards => V::Bool(!effective.theme.hover_cards),
        // Between the two that need no confirming. Warn is never a step away: it is asked for
        // with its acknowledgement, and a step from it goes back to the default.
        SettingKey::VpnOnFailure => V::Str(
            match effective.providers.torrent.vpn.on_leak {
                LeakAction::Pause => "stop",
                LeakAction::Stop | LeakAction::Warn => "pause",
            }
            .into(),
        ),
        // Typed, read-only, or moved on Providers: nothing to step through.
        SettingKey::Subtitles
        | SettingKey::DownloadDir
        | SettingKey::TorrentIndex
        | SettingKey::DownloadHook
        | SettingKey::SyncplayServer
        | SettingKey::SyncplayRoom
        | SettingKey::VpnMode
        | SettingKey::TokenStorage
        | SettingKey::SourceOrder
        | SettingKey::MpvOptions => return None,
    })
}

/// Put `value`, as written for `key`, into `config`: what a write puts in effect.
///
/// `false`, with nothing changed, for a value the setting does not take: the wrong kind, a
/// name it does not know, or a setting that is not written from here.
pub fn put(config: &mut Config, key: SettingKey, value: &SettingValue) -> bool {
    use SettingValue as V;
    use anistream_core::config::{DownloadQuality, LeakAction, PlayerUi, ThemeMode, Upscaling};
    use anistream_core::media::Translation;

    let playback = &mut config.playback;
    let seeding = &mut config.providers.torrent.seeding;
    match (key, value) {
        (SettingKey::Theme, V::Str(mode)) => {
            config.theme.mode = match mode.as_str() {
                "adaptive" => ThemeMode::Adaptive,
                "immersive" => ThemeMode::Immersive,
                "light" => ThemeMode::Light,
                "dark" => ThemeMode::Dark,
                _ => return false,
            };
        }
        (SettingKey::Motion, V::Bool(on)) => config.theme.motion = *on,
        (SettingKey::Titles, V::Str(name)) => {
            let Some(choice) = TitleChoice::from_name(name) else { return false };
            config.meta.title_language = choice;
        }
        (SettingKey::Trailers, V::Str(name)) => {
            let Some(choice) = TrailerAutoplay::from_name(name) else { return false };
            config.theme.trailers = choice;
        }
        (SettingKey::Translation, V::Str(name)) => {
            let Some(translation) =
                [Translation::Sub, Translation::Dub].into_iter().find(|t| t.as_str() == name)
            else {
                return false;
            };
            playback.translation = translation;
        }
        (SettingKey::Quality, V::Int(quality)) => {
            let Ok(quality) = u32::try_from(*quality) else { return false };
            playback.quality = quality;
        }
        (SettingKey::Subtitles, V::Str(language)) => {
            playback.subtitle_language.clone_from(language);
        }
        (SettingKey::CommitThreshold, V::Float(threshold)) => {
            playback.commit_threshold = *threshold;
        }
        (SettingKey::AutoNext, V::Bool(on)) => playback.auto_next = *on,
        (SettingKey::SkipOpening, V::Bool(on)) => playback.skip_opening = *on,
        (SettingKey::SkipFiller, V::Bool(on)) => playback.skip_filler = *on,
        (SettingKey::MarkChapters, V::Bool(on)) => playback.mark_chapters = *on,
        (SettingKey::SkipButton, V::Bool(on)) => playback.skip_button = *on,
        (SettingKey::Fullscreen, V::Bool(on)) => playback.fullscreen = *on,
        (SettingKey::Upscaling, V::Str(name)) => {
            let Some(mode) = Upscaling::from_name(name) else { return false };
            playback.upscaling = mode;
        }
        (SettingKey::PlayerUi, V::Str(name)) => {
            playback.player_ui = match name.as_str() {
                "anistream" => PlayerUi::Anistream,
                "mpv" => PlayerUi::Mpv,
                _ => return false,
            };
        }
        (SettingKey::NightMode, V::Bool(on)) => playback.night_mode = *on,
        (SettingKey::DownloadDir, V::Str(dir)) => {
            config.downloads.directory = Some(dir.clone());
        }
        (SettingKey::MergeSubtitles, V::Bool(on)) => config.downloads.merge_subtitles = *on,
        (SettingKey::KeepSeeding, V::Bool(on)) => config.downloads.keep_seeding = *on,
        (SettingKey::DownloadHook, V::Str(command)) => {
            config.downloads.on_complete = Some(command.clone());
        }
        (SettingKey::Presence, V::Bool(on)) => config.presence.enabled = *on,
        (SettingKey::PresenceTitle, V::Bool(on)) => config.presence.show_title = *on,
        (SettingKey::PresenceRich, V::Bool(on)) => config.presence.rich = *on,
        (SettingKey::Torrents, V::Bool(on)) => config.providers.torrent.enabled = *on,
        // Blank is no index, as the file reads it: the source needs one before it turns on.
        (SettingKey::TorrentIndex, V::Str(url)) => {
            config.providers.torrent.rss_url = Some(url.trim().to_owned());
        }
        (SettingKey::SeedRatio, V::Float(ratio)) => seeding.ratio = *ratio,
        (SettingKey::SeedHours, V::Int(hours)) => {
            let Ok(hours) = u32::try_from(*hours) else { return false };
            seeding.hours = hours;
        }
        (SettingKey::UploadLimit, V::Int(kib)) => {
            let Ok(kib) = u32::try_from(*kib) else { return false };
            seeding.upload_limit_kib = kib;
        }
        (SettingKey::Syncplay, V::Bool(on)) => config.syncplay.enabled = *on,
        (SettingKey::SyncplayServer, V::Str(server)) => {
            config.syncplay.server.clone_from(server);
        }
        (SettingKey::SyncplayRoom, V::Str(room)) => config.syncplay.room = Some(room.clone()),
        (SettingKey::UpdateCheck, V::Bool(on)) => config.updates.check = *on,
        (SettingKey::UpdateBeta, V::Bool(on)) => config.updates.beta = *on,
        (SettingKey::SourceOrder, V::Strings(order)) => {
            config.providers.order.clone_from(order);
        }
        (SettingKey::MpvOptions, V::Strings(options)) => {
            let Some(options) = mpv_options(options.iter().map(String::as_str)) else {
                return false;
            };
            playback.mpv_args = options;
        }
        // Typed, one to a line, as the row's text gives them.
        (SettingKey::MpvOptions, V::Str(lines)) => {
            let Some(options) = mpv_options(lines.lines()) else { return false };
            playback.mpv_args = options;
        }
        (SettingKey::UpNextCountdown, V::Int(seconds)) => {
            let Ok(seconds) = u32::try_from(*seconds) else { return false };
            playback.up_next_countdown = seconds;
        }
        (SettingKey::SpoilerProtection, V::Bool(on)) => playback.spoiler_protection = *on,
        (SettingKey::DownloadQuality, V::Str(name)) => {
            config.downloads.quality = match name.as_str() {
                "standard" => DownloadQuality::Standard,
                "higher" => DownloadQuality::Higher,
                _ => return false,
            };
        }
        (SettingKey::KeepNextEpisodes, V::Int(episodes)) => {
            let Ok(episodes) = u32::try_from(*episodes) else { return false };
            config.downloads.keep_next = episodes;
        }
        (SettingKey::DownloadsOnCellular, V::Bool(on)) => config.downloads.on_cellular = *on,
        (SettingKey::EpisodeNotifications, V::Bool(on)) => config.notifications.episodes = *on,
        (SettingKey::AdultTitles, V::Bool(on)) => config.meta.adult = *on,
        (SettingKey::EpisodeDiscussions, V::Bool(on)) => config.meta.discussions = *on,
        (SettingKey::CommentImages, V::Bool(on)) => config.meta.discussion_images = *on,
        (SettingKey::ControllerInput, V::Bool(on)) => config.controller.enabled = *on,
        (SettingKey::SwapFaceButtons, V::Bool(on)) => config.controller.swap_face = *on,
        (SettingKey::CalendarMine, V::Bool(on)) => config.meta.calendar_mine = *on,
        (SettingKey::CastTvs, V::Bool(on)) => config.cast.show_tvs = *on,
        (SettingKey::TorrentsWhileCasting, V::Bool(on)) => {
            config.providers.torrent.while_casting = Some(*on);
        }
        (SettingKey::HoverCards, V::Bool(on)) => config.theme.hover_cards = *on,
        (SettingKey::TorrentsOnCellular, V::Bool(on)) => {
            config.providers.torrent.on_cellular = *on;
        }
        (SettingKey::VpnOnFailure, V::Str(name)) => {
            config.providers.torrent.vpn.on_leak = match name.as_str() {
                "pause" => LeakAction::Pause,
                "stop" => LeakAction::Stop,
                "warn" => LeakAction::Warn,
                _ => return false,
            };
        }
        _ => return false,
    }
    true
}

/// Whether `value` for `key` asks for something that needs confirming first, which a setting
/// alone cannot carry: warn only, on a failed connection check, which leaves torrents running
/// on what may be the viewer's own address. Asked for with
/// [`Command::SetTorrentProtection`](crate::command::Command::SetTorrentProtection) instead,
/// and refused as a setting.
pub fn needs_acknowledgement(key: SettingKey, value: &SettingValue) -> bool {
    matches!((key, value), (SettingKey::VpnOnFailure, SettingValue::Str(name)) if name == "warn")
}

/// A row's value, as it shows it.
pub(crate) fn value(effective: &Config, saved: &Config, key: SettingKey) -> String {
    use anistream_core::config::{DownloadQuality, LeakAction, PlayerUi};

    let playback = &effective.playback;
    let torrent = &effective.providers.torrent;
    match key {
        SettingKey::Theme => session_marked(
            format!("{:?}", saved.theme.mode).to_lowercase(),
            format!("{:?}", effective.theme.mode).to_lowercase(),
        ),
        SettingKey::Motion => on_off(effective.theme.motion),
        SettingKey::Titles => match effective.meta.title_language {
            TitleChoice::Anilist => "AniList's choice",
            TitleChoice::English => "English",
            TitleChoice::Romaji => "Romaji",
        }
        .into(),
        SettingKey::Trailers => match effective.theme.trailers {
            TrailerAutoplay::Always => "always",
            TrailerAutoplay::WifiOnly => "Wi-Fi only",
            TrailerAutoplay::Off => "off",
        }
        .into(),
        SettingKey::Translation => session_marked(
            saved.playback.translation.to_string(),
            playback.translation.to_string(),
        ),
        SettingKey::Quality => format!("{}p", playback.quality),
        // Text, not a closed set — a language code cycler would be a list of every ISO 639
        // code, which is a config-file job rather than a screen job.
        SettingKey::Subtitles => playback.subtitle_language.clone(),
        SettingKey::CommitThreshold => {
            format!("{}%", (playback.commit_threshold * 100.0).round())
        }
        SettingKey::AutoNext => on_off(playback.auto_next),
        SettingKey::SkipOpening => on_off(playback.skip_opening),
        SettingKey::SkipFiller => on_off(playback.skip_filler),
        SettingKey::MarkChapters => on_off(playback.mark_chapters),
        SettingKey::SkipButton => on_off(playback.skip_button),
        SettingKey::Fullscreen => on_off(playback.fullscreen),
        SettingKey::Upscaling => upscaling_label(playback.upscaling).into(),
        SettingKey::PlayerUi => match playback.player_ui {
            PlayerUi::Anistream => "anistream",
            PlayerUi::Mpv => "your mpv setup",
        }
        .into(),
        SettingKey::NightMode => on_off(playback.night_mode),
        SettingKey::DownloadDir => effective
            .downloads
            .directory
            .clone()
            .unwrap_or_else(|| "kept with the torrent cache".into()),
        SettingKey::MergeSubtitles => on_off(effective.downloads.merge_subtitles),
        SettingKey::KeepSeeding => on_off(effective.downloads.keep_seeding),
        SettingKey::DownloadHook => {
            if effective.downloads.on_complete.is_some() { "configured" } else { "none" }.into()
        }
        SettingKey::Syncplay => on_off(effective.syncplay.enabled),
        SettingKey::SyncplayServer => effective.syncplay.server.clone(),
        SettingKey::SyncplayRoom => effective
            .syncplay
            .room
            .clone()
            .unwrap_or_else(|| "unset — needed for parties".into()),
        SettingKey::UpdateCheck => on_off(effective.updates.check),
        SettingKey::UpdateBeta => on_off(effective.updates.beta),
        SettingKey::Presence => on_off(effective.presence.enabled),
        SettingKey::PresenceTitle => on_off(effective.presence.show_title),
        SettingKey::PresenceRich => on_off(effective.presence.rich),
        SettingKey::Torrents => on_off(torrent.enabled),
        // By its host, which is what tells one index from another at a glance.
        SettingKey::TorrentIndex => torrent.indexer_host().unwrap_or_else(|| "none".into()),
        SettingKey::VpnMode => format!("{:?}", torrent.vpn.mode).to_lowercase(),
        SettingKey::SeedRatio => {
            if torrent.seeding.ratio > 0.0 {
                format!("until ratio {:.1}", torrent.seeding.ratio)
            } else {
                "off".into()
            }
        }
        SettingKey::SeedHours => format!("{} h", torrent.seeding.hours),
        SettingKey::UploadLimit => upload_label(torrent.seeding.upload_limit_kib),
        SettingKey::TokenStorage => effective.trackers.token_storage.clone(),
        // Not among the rows: Providers shows the order, as its table.
        SettingKey::SourceOrder => String::new(),
        SettingKey::MpvOptions => match effective.playback.mpv_args.len() {
            0 => "none".into(),
            n => crate::wording::plural(n, "option"),
        },
        SettingKey::UpNextCountdown => match playback.up_next_countdown {
            0 => "off".into(),
            seconds => format!("{seconds} s"),
        },
        SettingKey::SpoilerProtection => on_off(playback.spoiler_protection),
        SettingKey::DownloadQuality => match effective.downloads.quality {
            DownloadQuality::Standard => "standard",
            DownloadQuality::Higher => "higher",
        }
        .into(),
        SettingKey::KeepNextEpisodes => match effective.downloads.keep_next {
            0 => "off".into(),
            n => crate::wording::plural(n as usize, "episode"),
        },
        SettingKey::DownloadsOnCellular => on_off(effective.downloads.on_cellular),
        SettingKey::TorrentsOnCellular => on_off(torrent.on_cellular),
        SettingKey::EpisodeNotifications => on_off(effective.notifications.episodes),
        SettingKey::AdultTitles => on_off(effective.meta.adult),
        SettingKey::EpisodeDiscussions => on_off(effective.meta.discussions),
        SettingKey::CommentImages => on_off(effective.meta.discussion_images),
        SettingKey::ControllerInput => on_off(effective.controller.enabled),
        SettingKey::SwapFaceButtons => on_off(effective.controller.swap_face),
        SettingKey::CalendarMine => on_off(effective.meta.calendar_mine),
        SettingKey::CastTvs => on_off(effective.cast.show_tvs),
        SettingKey::TorrentsWhileCasting => on_off(while_casting(effective)),
        SettingKey::HoverCards => on_off(effective.theme.hover_cards),
        SettingKey::VpnOnFailure => match torrent.vpn.on_leak {
            LeakAction::Pause => "pause torrents",
            LeakAction::Stop => "stop torrents",
            LeakAction::Warn => "warn only",
        }
        .into(),
    }
}

/// How a row changes: typed from the value in effect, stepped with its ends marked, or not
/// at all.
fn change(effective: &Config, saved: &Config, key: SettingKey) -> SettingChange {
    if key.typed() {
        return SettingChange::Typed { text: text(effective, key) };
    }
    // Deliberately not editable: the VPN mode's `none` requires an explicit
    // acknowledgement key in the file, and a screen that let you cycle past it with an
    // arrow key would be a way around friction that exists on purpose.
    if key.path().is_none() {
        return SettingChange::Fixed;
    }
    // A two-state value steps like a two-rung ladder: back is off, or the first of its two
    // choices, and forward the other.
    if let Some(second) = two_state(effective, saved, key) {
        return SettingChange::Steps { back: second, forward: !second };
    }
    let now = ladder_value(effective, key);
    let moves = |delta| now.is_some() && step(effective, saved, key, delta) != now;
    SettingChange::Steps { back: moves(-1), forward: moves(1) }
}

/// Where a two-state row stands: `Some(true)` on its second state — on, or the second of
/// its two choices. `None` for every other row.
fn two_state(effective: &Config, saved: &Config, key: SettingKey) -> Option<bool> {
    use SettingKey as S;
    use anistream_core::config::{PlayerUi, ThemeMode};
    let c = effective;
    Some(match key {
        // Theme and sub/dub can differ for this session (`--theme`, `t`), and their arrows
        // change the saved choice, so that is the one they stand on.
        S::Theme => saved.theme.mode != ThemeMode::Adaptive,
        S::Translation => saved.playback.translation == anistream_core::media::Translation::Dub,
        S::PlayerUi => c.playback.player_ui == PlayerUi::Mpv,
        S::Motion => c.theme.motion,
        S::AutoNext => c.playback.auto_next,
        S::SkipOpening => c.playback.skip_opening,
        S::SkipFiller => c.playback.skip_filler,
        S::MarkChapters => c.playback.mark_chapters,
        S::SkipButton => c.playback.skip_button,
        S::Fullscreen => c.playback.fullscreen,
        S::NightMode => c.playback.night_mode,
        S::MergeSubtitles => c.downloads.merge_subtitles,
        S::KeepSeeding => c.downloads.keep_seeding,
        S::Presence => c.presence.enabled,
        S::PresenceTitle => c.presence.show_title,
        S::PresenceRich => c.presence.rich,
        S::Torrents => c.providers.torrent.enabled,
        S::Syncplay => c.syncplay.enabled,
        S::UpdateCheck => c.updates.check,
        S::UpdateBeta => c.updates.beta,
        S::SpoilerProtection => c.playback.spoiler_protection,
        S::DownloadQuality => {
            c.downloads.quality == anistream_core::config::DownloadQuality::Higher
        }
        S::DownloadsOnCellular => c.downloads.on_cellular,
        S::TorrentsOnCellular => c.providers.torrent.on_cellular,
        S::EpisodeNotifications => c.notifications.episodes,
        S::AdultTitles => c.meta.adult,
        S::EpisodeDiscussions => c.meta.discussions,
        S::CommentImages => c.meta.discussion_images,
        S::ControllerInput => c.controller.enabled,
        S::SwapFaceButtons => c.controller.swap_face,
        S::CalendarMine => c.meta.calendar_mine,
        S::CastTvs => c.cast.show_tvs,
        S::TorrentsWhileCasting => while_casting(c),
        S::HoverCards => c.theme.hover_cards,
        // Pause, then stop; warn stands beside stop, a step back from the default.
        S::VpnOnFailure => {
            c.providers.torrent.vpn.on_leak != anistream_core::config::LeakAction::Pause
        }
        _ => return None,
    })
}

/// A laddered row's value as it would be written, to tell a step from a stay.
fn ladder_value(effective: &Config, key: SettingKey) -> Option<SettingValue> {
    use SettingKey as S;
    use SettingValue as V;
    let playback = &effective.playback;
    let seeding = &effective.providers.torrent.seeding;
    Some(match key {
        S::Quality => V::Int(i64::from(playback.quality)),
        S::CommitThreshold => V::Float((playback.commit_threshold * 100.0).round() / 100.0),
        S::Upscaling => V::Str(playback.upscaling.name().into()),
        S::Titles => V::Str(effective.meta.title_language.name().into()),
        S::Trailers => V::Str(effective.theme.trailers.name().into()),
        S::SeedRatio => V::Float(seeding.ratio),
        S::SeedHours => V::Int(i64::from(seeding.hours)),
        S::UploadLimit => V::Int(i64::from(seeding.upload_limit_kib)),
        S::UpNextCountdown => V::Int(i64::from(playback.up_next_countdown)),
        S::KeepNextEpisodes => V::Int(i64::from(effective.downloads.keep_next)),
        _ => return None,
    })
}

/// The live value a typed setting starts editing from.
fn text(effective: &Config, key: SettingKey) -> String {
    match key {
        SettingKey::Subtitles => effective.playback.subtitle_language.clone(),
        SettingKey::DownloadDir => effective.downloads.directory.clone().unwrap_or_default(),
        SettingKey::DownloadHook => effective.downloads.on_complete.clone().unwrap_or_default(),
        SettingKey::SyncplayServer => effective.syncplay.server.clone(),
        SettingKey::SyncplayRoom => effective.syncplay.room.clone().unwrap_or_default(),
        SettingKey::TorrentIndex => {
            effective.providers.torrent.rss_url.clone().unwrap_or_default()
        }
        SettingKey::MpvOptions => effective.playback.mpv_args.join("\n"),
        _ => String::new(),
    }
}

/// The options a viewer wrote, each as mpv's command line takes one, or `None` when one is not
/// an option: `--name` or `--name=value`, the name without spaces. Blank lines are passed over
/// and the rest kept as written, since mpv is the judge of what its options mean.
fn mpv_options<'a>(written: impl Iterator<Item = &'a str>) -> Option<Vec<String>> {
    written
        .map(str::trim)
        .filter(|option| !option.is_empty())
        .map(|option| {
            let name = option.strip_prefix("--")?.split('=').next()?;
            let named = !name.is_empty() && !name.contains(char::is_whitespace);
            named.then(|| option.to_owned())
        })
        .collect()
}

fn upscaling_label(mode: anistream_core::config::Upscaling) -> &'static str {
    use anistream_core::config::Upscaling as U;
    match mode {
        U::Off => "off",
        U::Natural => "natural · ArtCNN",
        U::Sharp => "sharp · Anime4K",
    }
}

fn on_off(yes: bool) -> String {
    if yes { "on".into() } else { "off".into() }
}

/// A setting's saved value, with the session's own beside it when they differ.
fn session_marked(saved: String, current: String) -> String {
    if saved == current { saved } else { format!("{saved} · {current} this session") }
}

/// An upload cap as people read it: `unlimited`, `524 kB/s`, `2.1 MB/s`.
///
/// The config counts KiB/s, and this said `KB/s` and `MB/s` of those — binary values under
/// the decimal names every other size here uses. Converted rather than relabelled, so the
/// cap reads in the same units as the download sizes beside it.
fn upload_label(kib: u32) -> String {
    match kib {
        0 => "unlimited".into(),
        k => format!("{}/s", crate::wording::human_bytes(u64::from(k) * 1024)),
    }
}

/// Step to the next value in an ascending ladder, clamping at both ends.
///
/// Clamps rather than wraps: arrowing past 2160p round to 480p would be a surprising way to
/// lose your quality preference, and the ends of the ladder are informative.
///
/// A value that is not on the ladder — one typed into config.toml, like 1440p — steps to the
/// nearest rung in the arrow's direction, and stays put if there is none that way. It used
/// to be read as the bottom rung, so → from 1440p saved 720p and → from 97% saved 80%.
fn step_through<T: Copy + PartialOrd>(ladder: &[T], current: T, delta: i64) -> T {
    if let Some(at) = ladder.iter().position(|v| *v == current) {
        let next = (at as i64 + delta.signum()).clamp(0, ladder.len() as i64 - 1) as usize;
        return ladder[next];
    }
    let toward = if delta > 0 {
        ladder.iter().find(|v| **v > current)
    } else {
        ladder.iter().rev().find(|v| **v < current)
    };
    toward.copied().unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_step_from_anilists_choice_to_english_to_romaji() {
        let default = Config::default();
        let titles = |config: &Config| value(config, config, SettingKey::Titles);
        assert_eq!(titles(&default), "AniList's choice");
        assert_eq!(SettingKey::at(&["meta"], "title_language"), Some(SettingKey::Titles));
        assert_eq!(SettingKey::Titles.category(), "appearance");
        let step_to = |config: &Config, delta| step(config, config, SettingKey::Titles, delta);
        assert_eq!(step_to(&default, 1), Some(SettingValue::Str("english".into())));
        assert_eq!(step_to(&default, -1), Some(SettingValue::Str("anilist".into())), "an end");
        assert_eq!(
            change(&default, &default, SettingKey::Titles),
            SettingChange::Steps { back: false, forward: true }
        );

        let mut romaji = Config::default();
        assert!(put(&mut romaji, SettingKey::Titles, &SettingValue::Str("romaji".into())));
        assert_eq!(titles(&romaji), "Romaji");
        assert_eq!(
            step_to(&romaji, 1),
            Some(SettingValue::Str("romaji".into())),
            "the other end"
        );
        assert_eq!(
            change(&romaji, &romaji, SettingKey::Titles),
            SettingChange::Steps { back: true, forward: false }
        );
        assert!(put(&mut romaji, SettingKey::Titles, &SettingValue::Str("english".into())));
        assert_eq!(titles(&romaji), "English");
        assert!(!put(&mut romaji, SettingKey::Titles, &SettingValue::Str("native".into())));
        assert_eq!(titles(&romaji), "English", "a value it does not take changes nothing");

        let mut copied = Config::default();
        SettingKey::Titles.copy(&romaji, &mut copied);
        assert_eq!(titles(&copied), "English");
    }

    #[test]
    fn hover_cards_are_a_switch_of_their_own_beside_trailers() {
        let default = Config::default();
        assert_eq!(value(&default, &default, SettingKey::HoverCards), "on");
        assert_eq!(SettingKey::at(&["theme"], "hover_cards"), Some(SettingKey::HoverCards));
        assert_eq!(SettingKey::HoverCards.category(), "appearance");
        assert_eq!(
            step(&default, &default, SettingKey::HoverCards, 1),
            Some(SettingValue::Bool(false))
        );

        let mut off = Config::default();
        assert!(put(&mut off, SettingKey::HoverCards, &SettingValue::Bool(false)));
        assert!(!off.theme.hover_cards);
        assert_eq!(value(&off, &off, SettingKey::HoverCards), "off");
        assert_eq!(off.theme.trailers, TrailerAutoplay::Always, "trailers stay as they were");
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("off".into())));
        assert!(!off.theme.hover_cards, "and the other way round");

        let mut copied = Config::default();
        SettingKey::HoverCards.copy(&off, &mut copied);
        assert!(!copied.theme.hover_cards);
    }

    #[test]
    fn trailers_step_from_always_to_wifi_only_to_off_under_the_titles_row() {
        let default = Config::default();
        let trailers = |config: &Config| value(config, config, SettingKey::Trailers);
        assert_eq!(trailers(&default), "always");
        assert_eq!(SettingKey::at(&["theme"], "trailers"), Some(SettingKey::Trailers));
        let at = |key| SettingKey::ROWS.iter().position(|row| *row == key);
        assert_eq!(at(SettingKey::Trailers), at(SettingKey::Titles).map(|i| i + 1));
        assert_eq!(SettingKey::Trailers.category(), "appearance");

        let step_to =
            |config: &Config, delta| step(config, config, SettingKey::Trailers, delta);
        assert_eq!(step_to(&default, 1), Some(SettingValue::Str("wifi-only".into())));
        assert_eq!(step_to(&default, -1), Some(SettingValue::Str("always".into())), "an end");
        assert_eq!(
            change(&default, &default, SettingKey::Trailers),
            SettingChange::Steps { back: false, forward: true }
        );

        let mut off = Config::default();
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("wifi-only".into())));
        assert_eq!(trailers(&off), "Wi-Fi only");
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("off".into())));
        assert_eq!(trailers(&off), "off");
        assert_eq!(
            change(&off, &off, SettingKey::Trailers),
            SettingChange::Steps { back: true, forward: false }
        );
        assert!(!put(&mut off, SettingKey::Trailers, &SettingValue::Str("sometimes".into())));
        assert_eq!(trailers(&off), "off", "a value it does not take changes nothing");

        let mut copied = Config::default();
        SettingKey::Trailers.copy(&off, &mut copied);
        assert_eq!(trailers(&copied), "off");
    }

    #[test]
    fn every_setting_written_is_found_again_where_the_file_keeps_it() {
        // So a write named by the file's table and key names one setting, and only one.
        let every = SettingKey::ROWS.into_iter().chain(SettingKey::NATIVE_ROWS);
        for setting in every.chain([SettingKey::SourceOrder, SettingKey::TorrentIndex]) {
            if let Some((table, key)) = setting.path() {
                assert_eq!(SettingKey::at(table, key), Some(setting), "{table:?}.{key}");
            }
        }
        // Shown and never written from here: the file's to change.
        assert_eq!(SettingKey::VpnMode.path(), None);
        assert_eq!(SettingKey::TokenStorage.path(), None);
        // What the player remembers in passing is no setting of the screen's.
        for key in ["persisted_speed", "persisted_volume", "tone_mapping"] {
            assert_eq!(SettingKey::at(&["playback"], key), None, "{key}");
        }
        // Every row once, and the order, which has none.
        let rows: std::collections::HashSet<_> = SettingKey::ROWS.into_iter().collect();
        assert_eq!(rows.len(), SettingKey::ROWS.len());
        assert!(!SettingKey::ROWS.contains(&SettingKey::SourceOrder));
        // The native apps' rows: each once, and none the terminal draws.
        let native: std::collections::HashSet<_> =
            SettingKey::NATIVE_ROWS.into_iter().collect();
        assert_eq!(native.len(), SettingKey::NATIVE_ROWS.len());
        assert!(
            native.iter().all(|key| !rows.contains(key) && *key != SettingKey::SourceOrder)
        );
    }

    #[test]
    fn the_native_rows_step_put_and_read_back_as_the_file_takes_them() {
        use anistream_core::config::DownloadQuality;
        let config = Config::default();
        let view = view(&config, &config);
        let shown: Vec<_> =
            view.native_rows.iter().map(|row| (row.key, row.value.as_str())).collect();
        assert_eq!(
            shown,
            [
                (SettingKey::UpNextCountdown, "10 s"),
                (SettingKey::SpoilerProtection, "on"),
                (SettingKey::DownloadQuality, "standard"),
                (SettingKey::KeepNextEpisodes, "off"),
                (SettingKey::DownloadsOnCellular, "off"),
                (SettingKey::TorrentsOnCellular, "off"),
                (SettingKey::VpnOnFailure, "pause torrents"),
                (SettingKey::EpisodeNotifications, "off"),
                (SettingKey::MpvOptions, "none"),
                (SettingKey::AdultTitles, "off"),
                (SettingKey::EpisodeDiscussions, "on"),
                (SettingKey::CommentImages, "off"),
                (SettingKey::ControllerInput, "on"),
                (SettingKey::SwapFaceButtons, "off"),
                (SettingKey::CalendarMine, "off"),
                (SettingKey::CastTvs, "on"),
                (SettingKey::TorrentsWhileCasting, "on"),
                (SettingKey::HoverCards, "on"),
            ]
        );
        assert!(view.rows.iter().all(|row| !SettingKey::NATIVE_ROWS.contains(&row.key)));

        // Each one stepped is a value it takes, and changes it. The typed ones are typed.
        for key in SettingKey::NATIVE_ROWS.into_iter().filter(|key| !key.typed()) {
            let stepped = step(&config, &config, key, 1).expect("a step");
            let mut changed = config.clone();
            assert!(put(&mut changed, key, &stepped), "{key:?} takes {stepped:?}");
            assert_ne!(changed, config, "{key:?} changed nothing");
            let row = native_rows(&config, &config).into_iter().find(|r| r.key == key).unwrap();
            let SettingChange::Steps { back, forward } = row.change else { panic!("{key:?}") };
            assert!(back || forward, "{key:?} steps neither way");
        }
        assert_eq!(
            step(&config, &config, SettingKey::KeepNextEpisodes, 1),
            Some(SettingValue::Int(1))
        );
        assert_eq!(
            step(&config, &config, SettingKey::UpNextCountdown, -1),
            Some(SettingValue::Int(5))
        );

        let mut config = config;
        assert!(put(
            &mut config,
            SettingKey::DownloadQuality,
            &SettingValue::Str("higher".into())
        ));
        assert_eq!(config.downloads.quality, DownloadQuality::Higher);
        assert!(!put(
            &mut config,
            SettingKey::DownloadQuality,
            &SettingValue::Str("best".into())
        ));
        assert!(put(&mut config, SettingKey::UpNextCountdown, &SettingValue::Int(0)));
        assert_eq!(value(&config, &config, SettingKey::UpNextCountdown), "off");
        assert!(put(&mut config, SettingKey::KeepNextEpisodes, &SettingValue::Int(3)));
        assert_eq!(value(&config, &config, SettingKey::KeepNextEpisodes), "3 episodes");
    }

    #[test]
    fn warn_only_is_never_a_step_and_needs_confirming_as_a_setting() {
        use anistream_core::config::LeakAction;
        let mut config = Config::default();
        let warn = SettingValue::Str("warn".into());
        assert!(needs_acknowledgement(SettingKey::VpnOnFailure, &warn));
        assert!(!needs_acknowledgement(
            SettingKey::VpnOnFailure,
            &SettingValue::Str("stop".into())
        ));
        // Pause and stop step between each other; from warn a step goes back to the default.
        assert_eq!(
            step(&config, &config, SettingKey::VpnOnFailure, 1),
            Some(SettingValue::Str("stop".into()))
        );
        config.providers.torrent.vpn.on_leak = LeakAction::Warn;
        assert_eq!(
            step(&config, &config, SettingKey::VpnOnFailure, -1),
            Some(SettingValue::Str("pause".into()))
        );
        assert_eq!(value(&config, &config, SettingKey::VpnOnFailure), "warn only");
    }

    #[test]
    fn a_row_says_which_way_a_step_would_change_it() {
        let mut config = Config::default();
        let row = |config: &Config, key| {
            rows(config, config).into_iter().find(|row| row.key == key).unwrap().change
        };
        // A ladder, mid-way and at its top: 1080p, then 2160p.
        assert_eq!(
            row(&config, SettingKey::Quality),
            SettingChange::Steps { back: true, forward: true }
        );
        config.playback.quality = 2160;
        assert_eq!(
            row(&config, SettingKey::Quality),
            SettingChange::Steps { back: true, forward: false }
        );
        assert_eq!(
            step(&config, &config, SettingKey::Quality, 1),
            Some(SettingValue::Int(2160)),
            "a step past the end comes back as it is"
        );
        // Two states: on stands on its second, so only back changes it.
        assert_eq!(
            row(&config, SettingKey::AutoNext),
            SettingChange::Steps { back: true, forward: false }
        );
        // Typed from the value in effect; read-only not at all.
        assert_eq!(
            row(&config, SettingKey::Subtitles),
            SettingChange::Typed { text: "eng".into() }
        );
        assert_eq!(row(&config, SettingKey::VpnMode), SettingChange::Fixed);
    }

    #[test]
    fn a_value_a_setting_does_not_take_changes_nothing() {
        let mut config = Config::default();
        let before = config.clone();
        assert!(!put(&mut config, SettingKey::Quality, &SettingValue::Str("hd".into())));
        assert!(!put(&mut config, SettingKey::Upscaling, &SettingValue::Str("max".into())));
        assert!(!put(&mut config, SettingKey::Quality, &SettingValue::Int(-1)));
        assert!(!put(&mut config, SettingKey::VpnMode, &SettingValue::Str("none".into())));
        assert_eq!(config, before);

        assert!(put(&mut config, SettingKey::Upscaling, &SettingValue::Str("sharp".into())));
        assert_eq!(config.playback.upscaling, anistream_core::config::Upscaling::Sharp);
        let order = SettingValue::Strings(vec!["plugins".into()]);
        assert!(put(&mut config, SettingKey::SourceOrder, &order));
        assert_eq!(config.providers.order, ["plugins"]);
    }

    #[test]
    fn the_torrent_index_is_typed_and_shown_by_its_host() {
        let mut config = Config::default();
        assert!(
            !SettingKey::ROWS.contains(&SettingKey::TorrentIndex),
            "the file's, on the terminal"
        );
        assert!(SettingKey::TorrentIndex.typed());
        assert_eq!(step(&config, &config, SettingKey::TorrentIndex, 1), None);
        assert_eq!(value(&config, &config, SettingKey::TorrentIndex), "none");

        let url = SettingValue::Str(" https://Indexer.Example/rss?q={query} ".into());
        assert!(put(&mut config, SettingKey::TorrentIndex, &url));
        let set = config.providers.torrent.rss_url.as_deref();
        assert_eq!(set, Some("https://Indexer.Example/rss?q={query}"), "trimmed");
        assert_eq!(value(&config, &config, SettingKey::TorrentIndex), "indexer.example");
        assert_eq!(
            change(&config, &config, SettingKey::TorrentIndex),
            SettingChange::Typed { text: "https://Indexer.Example/rss?q={query}".into() }
        );
        assert_eq!(view(&config, &config).sources.index.as_deref(), set);
        assert!(redefines_sources(SettingKey::TorrentIndex));
    }

    #[test]
    fn a_value_off_the_ladder_steps_to_the_nearest_rung_the_arrow_points_at() {
        // From config.toml, not the arrows. → from 1440p saved 720p, and → from 97% saved 80%.
        const QUALITY: [u32; 4] = [480, 720, 1080, 2160];
        assert_eq!(step_through(&QUALITY, 1440, 1), 2160);
        assert_eq!(step_through(&QUALITY, 1440, -1), 1080);
        assert_eq!(step_through(&QUALITY, 360, -1), 360, "nothing lower: stay put");
        assert_eq!(step_through(&QUALITY, 360, 1), 480);

        const THRESHOLD: [u32; 5] = [70, 80, 85, 90, 95];
        assert_eq!(step_through(&THRESHOLD, 97, 1), 97, "nothing higher: stay put");
        assert_eq!(step_through(&THRESHOLD, 97, -1), 95);
        assert_eq!(step_through(&THRESHOLD, 82, 1), 85);
        assert_eq!(step_through(&THRESHOLD, 82, -1), 80);
    }

    #[test]
    fn only_source_defining_keys_force_a_rebuild() {
        // A rebuild tears down a live torrent session, so it must not fire on a setting
        // that has nothing to do with which sources exist.
        assert!(redefines_sources(SettingKey::Torrents));
        assert!(redefines_sources(SettingKey::VpnMode));
        assert!(redefines_sources(SettingKey::SourceOrder));
        assert!(redefines_sources(SettingKey::VpnOnFailure));

        assert!(!redefines_sources(SettingKey::Upscaling));
        assert!(!redefines_sources(SettingKey::Quality));
        assert!(!redefines_sources(SettingKey::DownloadDir));
        assert!(!redefines_sources(SettingKey::Theme));
        assert!(!redefines_sources(SettingKey::TorrentsOnCellular));
    }

    #[test]
    fn the_viewer_s_own_mpv_options_are_kept_as_written_and_shown_as_a_count() {
        let mut config = Config::default();
        assert_eq!(value(&config, &config, SettingKey::MpvOptions), "none");
        let options = SettingValue::Strings(vec![
            "--sub-font-size=40".into(),
            " --no-audio-display ".into(),
            String::new(),
        ]);
        assert!(put(&mut config, SettingKey::MpvOptions, &options));
        assert_eq!(config.playback.mpv_args, ["--sub-font-size=40", "--no-audio-display"]);
        assert_eq!(value(&config, &config, SettingKey::MpvOptions), "2 options");
        assert_eq!(
            change(&config, &config, SettingKey::MpvOptions),
            SettingChange::Typed { text: "--sub-font-size=40\n--no-audio-display".into() },
            "typed, one to a line"
        );
        // As typed: one to a line.
        let typed = SettingValue::Str("--hwdec=no\n\n--sub-scale=1.2".into());
        assert!(put(&mut config, SettingKey::MpvOptions, &typed));
        assert_eq!(config.playback.mpv_args, ["--hwdec=no", "--sub-scale=1.2"]);
        // Not options, so nothing changes.
        for wrong in ["sub-font-size=40", "--", "--=1", "--sub font=1"] {
            let refused = SettingValue::Strings(vec![wrong.into()]);
            assert!(!put(&mut config, SettingKey::MpvOptions, &refused), "{wrong}");
        }
        assert_eq!(config.playback.mpv_args, ["--hwdec=no", "--sub-scale=1.2"]);
        assert_eq!(SettingKey::at(&["playback"], "mpv_args"), Some(SettingKey::MpvOptions));
        assert_eq!(SettingKey::MpvOptions.category(), "advanced");
    }
}
