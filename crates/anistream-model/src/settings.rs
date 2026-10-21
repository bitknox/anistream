//! Settings: each one a front end shows or changes, named for what it is; the rules for what a
//! row shows and what a step makes of it; and what a front end is told of the config.
//!
//! The engine owns the config and runs these rules over it. A front end asks for a change with
//! a command and is told the outcome as a [`SettingsView`]: the config itself never crosses.

use anistream_core::{
    config::{Config, ThemeMode},
    media::Translation,
    settings::SettingValue,
};

use crate::sources::ConfiguredSources;

/// Every setting a front end shows or changes, named for what it is rather than for where the
/// file keeps it: the Settings screen's rows, in display order in [`Self::ROWS`], and the one
/// with no row there, the source order.
///
/// An enum rather than positional indices: the cycling logic and the renderer both key off this,
/// so inserting a row cannot make the screen edit the setting below the one it is showing. And
/// rather than the file's table and key, which are where it is written and nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
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
            Self::TorrentIndex => (&["providers", "torrent"], "rss_url"),
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
            | Self::TorrentIndex
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
            Self::TorrentIndex => "torrent index",
            Self::SeedRatio => "seed after watching",
            Self::SeedHours => "seed for at most",
            Self::UploadLimit => "upload limit",
            Self::VpnMode => "vpn mode",
            Self::TokenStorage => "token storage",
            Self::SourceOrder => "source order",
        }
    }
}

/// Whether writing this setting changes which sources exist, and so needs the chain rebuilt.
///
/// Listed rather than inferred: a rebuild tears down a live torrent session, which is far
/// too blunt to trigger on a guess about a setting. The VPN mode is listed though nothing here
/// writes it, since it decides whether torrents run at all; the sources switched off by name
/// and the remote's address, which the file alone sets, have no key to list.
pub fn redefines_sources(setting: SettingKey) -> bool {
    // The index too: the torrent source reads it as it comes up.
    matches!(
        setting,
        SettingKey::Torrents
            | SettingKey::TorrentIndex
            | SettingKey::VpnMode
            | SettingKey::SourceOrder
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
    pub rows: Vec<SettingRow>,
    /// The setting whose write was just refused and put back to what the file holds: on the
    /// view that answers the refusal, and on no other. The notice saying why comes after it.
    pub refused: Option<SettingKey>,
    /// Sub or dub, in effect: what the status line says, and what `t` turns over.
    pub translation: Translation,
    /// The quality asked of the sources, in effect, for the status line.
    pub quality: u32,
    /// The theme in effect, which the terminal paints with.
    pub theme: ThemeMode,
    /// The sources the config sets up: the header's chips, the source the status line names,
    /// and the order Providers moves a source up and down.
    pub sources: ConfiguredSources,
    /// The speed a playback starts at, when one is remembered: what Now Playing shows of a
    /// playback the front end stages as it asks for it, until the player says. One the engine
    /// starts on its own comes staged with it.
    pub speed: Option<f64>,
    /// Whether Discord presence has a client to publish under, which its row's note depends on.
    pub presence_client: bool,
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
        sources: ConfiguredSources::of(&effective.providers),
        speed: effective.playback.persisted_speed,
        presence_client: effective.presence.resolved_client_id().is_some(),
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
    SettingKey::ROWS
        .into_iter()
        .map(|key| SettingRow {
            key,
            value: value(effective, saved, key),
            change: change(effective, saved, key),
        })
        .collect()
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
    use anistream_core::config::{PlayerUi, ThemeMode, Upscaling};
    use anistream_core::media::Translation;

    let playback = &effective.playback;
    let seeding = &effective.providers.torrent.seeding;
    Some(match key {
        SettingKey::Theme => V::Str(
            match saved.theme.mode {
                ThemeMode::Adaptive => "immersive",
                ThemeMode::Immersive => "adaptive",
            }
            .into(),
        ),
        SettingKey::Motion => V::Bool(!effective.theme.motion),
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
        SettingKey::SkipOpening => V::Bool(!playback.skip_opening),
        SettingKey::SkipFiller => V::Bool(!playback.skip_filler),
        SettingKey::MarkChapters => V::Bool(!playback.mark_chapters),
        SettingKey::Fullscreen => V::Bool(!playback.fullscreen),
        SettingKey::Presence => V::Bool(!effective.presence.enabled),
        SettingKey::PresenceTitle => V::Bool(!effective.presence.show_title),
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
        // Typed, read-only, or moved on Providers: nothing to step through.
        SettingKey::Subtitles
        | SettingKey::DownloadDir
        | SettingKey::TorrentIndex
        | SettingKey::DownloadHook
        | SettingKey::SyncplayServer
        | SettingKey::SyncplayRoom
        | SettingKey::VpnMode
        | SettingKey::TokenStorage
        | SettingKey::SourceOrder => return None,
    })
}

/// Put `value`, as written for `key`, into `config`: what a write puts in effect.
///
/// `false`, with nothing changed, for a value the setting does not take: the wrong kind, a
/// name it does not know, or a setting that is not written from here.
pub fn put(config: &mut Config, key: SettingKey, value: &SettingValue) -> bool {
    use SettingValue as V;
    use anistream_core::config::{PlayerUi, ThemeMode, Upscaling};
    use anistream_core::media::Translation;

    let playback = &mut config.playback;
    let seeding = &mut config.providers.torrent.seeding;
    match (key, value) {
        (SettingKey::Theme, V::Str(mode)) => {
            config.theme.mode = match mode.as_str() {
                "adaptive" => ThemeMode::Adaptive,
                "immersive" => ThemeMode::Immersive,
                _ => return false,
            };
        }
        (SettingKey::Motion, V::Bool(on)) => config.theme.motion = *on,
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
        (SettingKey::SourceOrder, V::Strings(order)) => {
            config.providers.order.clone_from(order);
        }
        _ => return false,
    }
    true
}

/// A row's value, as it shows it.
fn value(effective: &Config, saved: &Config, key: SettingKey) -> String {
    use anistream_core::config::PlayerUi;

    let playback = &effective.playback;
    let torrent = &effective.providers.torrent;
    match key {
        SettingKey::Theme => session_marked(
            format!("{:?}", saved.theme.mode).to_lowercase(),
            format!("{:?}", effective.theme.mode).to_lowercase(),
        ),
        SettingKey::Motion => on_off(effective.theme.motion),
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
        SettingKey::Presence => on_off(effective.presence.enabled),
        SettingKey::PresenceTitle => on_off(effective.presence.show_title),
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
        S::Theme => saved.theme.mode == ThemeMode::Immersive,
        S::Translation => saved.playback.translation == anistream_core::media::Translation::Dub,
        S::PlayerUi => c.playback.player_ui == PlayerUi::Mpv,
        S::Motion => c.theme.motion,
        S::AutoNext => c.playback.auto_next,
        S::SkipOpening => c.playback.skip_opening,
        S::SkipFiller => c.playback.skip_filler,
        S::MarkChapters => c.playback.mark_chapters,
        S::Fullscreen => c.playback.fullscreen,
        S::NightMode => c.playback.night_mode,
        S::MergeSubtitles => c.downloads.merge_subtitles,
        S::KeepSeeding => c.downloads.keep_seeding,
        S::Presence => c.presence.enabled,
        S::PresenceTitle => c.presence.show_title,
        S::Torrents => c.providers.torrent.enabled,
        S::Syncplay => c.syncplay.enabled,
        S::UpdateCheck => c.updates.check,
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
        S::SeedRatio => V::Float(seeding.ratio),
        S::SeedHours => V::Int(i64::from(seeding.hours)),
        S::UploadLimit => V::Int(i64::from(seeding.upload_limit_kib)),
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
        _ => String::new(),
    }
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
    fn every_setting_written_is_found_again_where_the_file_keeps_it() {
        // So a write named by the file's table and key names one setting, and only one.
        let unlisted = [SettingKey::SourceOrder, SettingKey::TorrentIndex];
        for setting in SettingKey::ROWS.into_iter().chain(unlisted) {
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

        assert!(!redefines_sources(SettingKey::Upscaling));
        assert!(!redefines_sources(SettingKey::Quality));
        assert!(!redefines_sources(SettingKey::DownloadDir));
        assert!(!redefines_sources(SettingKey::Theme));
    }
}
