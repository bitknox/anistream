//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Update`]s, so a slow provider or a large image decode cannot stall a frame.

use std::time::{Duration, Instant};

use anistream_core::{config::Config, ids::AnilistId, traits::SourceCandidate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    eyecatch::Eyecatch,
    keymap::{Action, Context, Keymap},
    nav::{Focus, Nav, Overlay, Section, StageView},
    theme::{Palette, glyph},
};

/// A transient message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    /// Frames remaining before it disappears.
    pub ttl: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Alert,
}

impl Toast {
    pub fn info(text: impl Into<String>) -> Self {
        Self { text: text.into(), kind: ToastKind::Info, ttl: 90 }
    }

    pub fn alert(text: impl Into<String>) -> Self {
        // Failures stay up longer: they carry information the user may need to act on,
        // and provider death is this app's defining failure mode.
        Self { text: text.into(), kind: ToastKind::Alert, ttl: 180 }
    }
}

/// One row of the Settings screen.
///
/// The screen was read-only, which made it a status page wearing a settings page's name. Every
/// row here either cycles through a closed set of values or says plainly why it cannot.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingRow {
    pub label: &'static str,
    /// The heading this row sits under on screen.
    pub category: &'static str,
    /// Rendered value.
    pub value: String,
    /// `None` for rows that are shown but not editable here.
    pub editable: Option<SettingEdit>,
    /// Why a row is read-only, or what a change will not do until restart.
    pub note: Option<&'static str>,
}

/// Where a row writes and what it cycles through.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingEdit {
    /// Dotted path of the containing table, as `anistream_core::settings::write_key` wants it.
    pub table: &'static [&'static str],
    pub key: &'static str,
}

/// Every row of the Settings screen, in display order.
///
/// An enum rather than positional indices: the cycling logic and the renderer both key off this,
/// so inserting a row cannot make the screen edit the setting below the one it is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingId {
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
}

impl SettingId {
    /// Display order. Grouped by [`Self::category`] — the renderer draws a heading each
    /// time the category changes, so rows of one category must be contiguous here.
    pub const ALL: [Self; 30] = [
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

    /// The settings whose values are typed rather than cycled, with where they write.
    /// Everything else either has a closed set of values or is read-only on purpose.
    pub const fn text_edit(self) -> Option<(&'static [&'static str], &'static str)> {
        match self {
            Self::Subtitles => Some((&["playback"], "subtitle_language")),
            Self::DownloadDir => Some((&["downloads"], "directory")),
            Self::DownloadHook => Some((&["downloads"], "on_complete")),
            Self::SyncplayServer => Some((&["syncplay"], "server")),
            Self::SyncplayRoom => Some((&["syncplay"], "room")),
            _ => None,
        }
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
            | Self::UploadLimit => "sources",
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
        }
    }
}

/// The local calendar date an offset-from-now lands on. Shared with the renderer, so
/// where the calendar *opens* and where it *rules* can never disagree.
pub(crate) fn air_date(secs_from_now: i64) -> Option<chrono::NaiveDate> {
    use chrono::TimeZone;
    let epoch = chrono::Local::now().timestamp() + secs_from_now;
    chrono::Local.timestamp_opt(epoch, 0).single().map(|t| t.date_naive())
}

fn upscaling_label(mode: anistream_core::config::Upscaling) -> &'static str {
    use anistream_core::config::Upscaling as U;
    match mode {
        U::Off => "off",
        U::Natural => "natural · ArtCNN",
        U::Sharp => "sharp · Anime4K",
    }
}

/// `"4"` → one episode, `"1-12"` → inclusive range, `"7-"` → everything from 7 on.
fn parse_episode_range(input: &str) -> Option<(u32, Option<u32>)> {
    let input = input.trim();
    if let Some((from, to)) = input.split_once('-') {
        let from: u32 = from.trim().parse().ok()?;
        let to = to.trim();
        let to: Option<u32> = if to.is_empty() { None } else { Some(to.parse().ok()?) };
        if to.is_some_and(|t| t < from) {
            return None;
        }
        Some((from, to))
    } else {
        let n: u32 = input.parse().ok()?;
        Some((n, Some(n)))
    }
}

fn on_off(yes: bool) -> String {
    if yes { "on".into() } else { "off".into() }
}

/// Map a row's dotted table name onto the slice form the writer takes.
///
/// A `&'static [&'static str]` cannot be built from a runtime split, and the set of tables the
/// Settings screen writes to is closed, so they are spelled out.
fn table_path(dotted: &'static str) -> &'static [&'static str] {
    match dotted {
        "theme" => &["theme"],
        "playback" => &["playback"],
        "presence" => &["presence"],
        "providers.torrent" => &["providers", "torrent"],
        "providers.torrent.seeding" => &["providers", "torrent", "seeding"],
        "downloads" => &["downloads"],
        "syncplay" => &["syncplay"],
        "updates" => &["updates"],
        other => unreachable!("no table path for {other}"),
    }
}

/// A setting's saved value, with the session's own beside it when they differ.
fn session_marked(saved: String, current: String) -> String {
    if saved == current { saved } else { format!("{saved} · {current} this session") }
}

/// An upload cap as people read it: `unlimited`, `512 KB/s`, `2 MB/s`.
fn upload_label(kib: u32) -> String {
    match kib {
        0 => "unlimited".into(),
        k if k >= 1024 && k % 1024 == 0 => format!("{} MB/s", k / 1024),
        k => format!("{k} KB/s"),
    }
}

/// Step to the next value in an ascending ladder, clamping at both ends.
///
/// Clamps rather than wraps: arrowing past 2160p round to 480p would be a surprising way to lose
/// your quality preference, and the ends of the ladder are informative.
///
/// A value that is not on the ladder — one typed into config.toml, like 1440p — steps to the
/// nearest rung in the arrow's direction, and stays put if there is none that way. It used to
/// be read as the bottom rung, so → from 1440p saved 720p and → from 97% saved 80%.
fn step_through<T: Copy + PartialOrd>(ladder: &[T], current: T, delta: isize) -> T {
    if let Some(at) = ladder.iter().position(|v| *v == current) {
        let next = (at as isize + delta.signum()).clamp(0, ladder.len() as isize - 1) as usize;
        return ladder[next];
    }
    let toward = if delta > 0 {
        ladder.iter().find(|v| **v > current)
    } else {
        ladder.iter().rev().find(|v| **v < current)
    };
    toward.copied().unwrap_or(current)
}

/// Idle ticks a rail change waits before fetching. The idle tick is 100 ms, so this is ~300 ms —
/// long enough to coalesce a held arrow key, short enough not to feel like lag.
const RELOAD_IDLE_TICKS: u8 = 3;

/// How close to the top of the calendar the selection gets before more history is fetched.
///
/// A screenful, roughly. The fetch is one request against a 30-per-minute budget and takes a
/// few hundred milliseconds, so it has to start while there are still rows left to scroll
/// through — trigger it at row nought and the user reaches the end of the list before the
/// answer arrives, which is the jump this design exists to avoid.
const CALENDAR_BACKFILL_MARGIN: usize = 12;

/// Palette rows shown at once. Also the number the arrows can reach, so the two cannot drift.
pub const PALETTE_ROWS: usize = 12;

/// Log lines retained. Enough to cover a failover walk across every provider several times.
pub const LOG_CAPACITY: usize = 200;

/// One line in the Logs overlay.
///
/// The overlay exists because provider death is this app's defining failure mode, and a toast
/// that has already faded cannot be read. When a source breaks at 2am, the first error in the
/// sequence is the one that says why.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRow {
    pub kind: ToastKind,
    pub text: String,
    /// Idle ticks at the time it was recorded, so lines can be shown relative to each other
    /// without pulling a wall clock into the UI crate.
    pub at: u64,
    /// How many times in a row this exact line arrived. A retry loop or a burst of identical
    /// failures is one line with a count, so it cannot push the line that explains it off
    /// the screen.
    pub count: u32,
}

/// A title as the list views need it.
///
/// A flattened projection rather than the full `Media`, so rendering does not depend on the
/// metadata crate and can be exercised without one.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: AnilistId,
    pub title: String,
    pub secondary: Option<String>,
    pub format: Option<String>,
    pub episodes: Option<u32>,
    pub year: Option<u16>,
    pub score: Option<u32>,
    pub studio: Option<String>,
    pub synopsis: String,
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
    pub genres: Vec<String>,
    /// Services carrying this title, from AniList's external links.
    pub available_on: Vec<String>,
    /// Local progress: episodes completed, and the next episode to watch.
    pub progress: Option<(u32, u32)>,
    /// Seconds until *this row's* broadcast — which for a calendar entry is the episode the row
    /// is about, and runs negative once that episode has aired. Everywhere else it is the same
    /// thing as [`Self::next_airing_in`].
    pub airing_in: Option<i64>,
    /// The episode a calendar row is about — the one [`Self::airing_in`] dates. `None`
    /// everywhere else.
    ///
    /// A field of its own rather than `ep 12` appended to the title. Appended, it was the first
    /// thing truncation took, so a column of one show's episodes read as identical rows, and it
    /// followed the title into the Title and Episodes headings.
    pub airing_episode: Option<u32>,
    /// The episode the title broadcasts next, from its own schedule.
    pub next_episode: Option<u32>,
    /// Seconds until [`Self::next_episode`] airs.
    ///
    /// Separate from [`Self::airing_in`] because a calendar row overwrites that with its own
    /// slot, and the two then describe different episodes — which is how a row about an episode
    /// that had just aired came to read "EP 6 in now". This one is the title's schedule and
    /// nothing else's, so the pair is always about the same episode.
    pub next_airing_in: Option<i64>,
    /// The most recent broadcast: episode number and how many seconds ago.
    ///
    /// Answers the question a list of airing shows is actually asked — *is there something new
    /// to watch* — which neither the total episode count nor the next countdown can.
    pub last_aired: Option<(u32, i64)>,
    /// An episode left part-watched, and where in it you stopped.
    ///
    /// Deliberately independent of the commit threshold. That threshold governs when an episode is
    /// *counted* — it exists so opening one to check the subtitles does not push progress to a
    /// tracker. Picking up where you left off is a different question with a different answer:
    /// quitting halfway through an episode you fully intend to finish should be the single most
    /// visible thing on the home screen, and under the old behaviour it was invisible.
    pub resume: Option<ResumePoint>,
    /// Titles adjacent in the main watch order — prequels, the parent story, sequels.
    /// Present on the detail fetch only; list fetches leave it empty.
    pub related: Vec<RelatedTitle>,
    /// What people who liked this rated highest. Rides the detail fetch, like `related`.
    pub recommended: Vec<RelatedTitle>,
    /// A watchable trailer URL, when AniList has one. Rides the detail fetch.
    pub trailer_url: Option<String>,
    /// Where the title sits on the tracker's list, when that is known: a Library row knows its
    /// segment, and a status set in this session is remembered on the entry. `None` means not
    /// known, not "on no list" — nothing outside the Library carries the tracker's answer.
    pub list_status: Option<LibrarySegment>,
}

/// What the episode table shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EpisodeFilter {
    #[default]
    All,
    /// Only episodes not yet completed.
    Unwatched,
    /// Everything except pure filler. `mixed` stays — it carries story.
    NoFiller,
}

impl EpisodeFilter {
    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "all episodes",
            Self::Unwatched => "unwatched",
            Self::NoFiller => "no filler",
        }
    }

    const fn next(self) -> Self {
        match self {
            Self::All => Self::Unwatched,
            Self::Unwatched => Self::NoFiller,
            Self::NoFiller => Self::All,
        }
    }

    fn keeps(self, row: &EpisodeRow) -> bool {
        match self {
            Self::All => true,
            Self::Unwatched => !row.completed,
            Self::NoFiller => !row.skippable,
        }
    }
}

/// One step of a title's watch order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelatedTitle {
    pub id: AnilistId,
    pub title: String,
    /// `prequel`, `parent` or `sequel`, already lowercased for display.
    pub relation: String,
    pub format: Option<String>,
}

/// Where an unfinished episode was left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResumePoint {
    /// Seconds into the episode.
    pub position: f64,
    /// Fraction of the episode watched, when the runtime is known.
    pub fraction: Option<f64>,
}

impl ResumePoint {
    /// `12:30` — the position as a viewer thinks of it.
    pub fn clock(&self) -> String {
        let total = self.position.max(0.0) as u64;
        let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}

impl Entry {
    pub fn new(id: AnilistId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            secondary: None,
            format: None,
            episodes: None,
            year: None,
            score: None,
            studio: None,
            synopsis: String::new(),
            cover_url: None,
            banner_url: None,
            genres: Vec::new(),
            available_on: Vec::new(),
            progress: None,
            airing_in: None,
            airing_episode: None,
            next_episode: None,
            next_airing_in: None,
            last_aired: None,
            resume: None,
            related: Vec::new(),
            recommended: Vec::new(),
            trailer_url: None,
            list_status: None,
        }
    }

    /// What progress is counted against, and whether that is the season's length. AniList
    /// leaves the length empty while a show airs without an announced one, and counting against
    /// nothing read as "2 / 0"; the newest episode out is the honest stand-in until it is known.
    pub fn progress_of(&self) -> Option<(u32, bool)> {
        match self.episodes.filter(|n| *n > 0) {
            Some(total) => Some((total, true)),
            None => self.latest_aired().map(|(episode, _)| (episode, false)),
        }
    }

    /// Fraction watched, for the progress meter.
    pub fn watched_fraction(&self) -> f64 {
        match (self.progress, self.progress_of()) {
            (Some((done, _)), Some((of, _))) => {
                (f64::from(done) / f64::from(of)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// The count beside the meter: `2 / 12`, `2 / 3 aired` while the length is unannounced, or
    /// `2 watched` when there is nothing to count against.
    pub fn progress_label(&self) -> Option<String> {
        let (done, _) = self.progress?;
        Some(match self.progress_of() {
            Some((total, true)) => format!("{done} / {total}"),
            Some((aired, false)) => format!("{done} / {aired} aired"),
            None => format!("{done} watched"),
        })
    }

    /// The newest episode known to be out, with how long ago when that is known too.
    ///
    /// Reconciles two sources that disagree for a stretch right after a broadcast: the
    /// last-aired schedule row lags AniList's own `nextAiringEpisode`, which can already
    /// hold episode N+1 while the schedule query still answers N-1. Holding N+1 as *next*
    /// proves N is out, so the larger claim wins — this is what stops a freshly-launched
    /// episode from being named only by its successor. The "ago" is carried only when it
    /// belongs to the episode being named, absent rather than guessed otherwise.
    pub fn latest_aired(&self) -> Option<(u32, Option<i64>)> {
        let derived = self.next_episode.and_then(|n| n.checked_sub(1)).filter(|n| *n >= 1);
        match (self.last_aired, derived) {
            (Some((episode, _)), Some(proven)) if proven > episode => Some((proven, None)),
            (Some((episode, ago)), _) => Some((episode, Some(ago))),
            (None, proven) => proven.map(|episode| (episode, None)),
        }
    }
}

/// A device code the user must enter elsewhere to finish signing in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCodePrompt {
    pub tracker: String,
    /// The short code to type. The only thing on screen that has to be transcribed by hand, so it
    /// gets the brightest treatment the palette has rather than the dimmest.
    pub code: String,
    pub url: String,
}

/// One row of the download queue, flattened for the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct DownloadRow {
    pub id: i64,
    /// The series this file belongs to — what lets a downloaded episode carry the same
    /// history, resume and sync a streamed one does.
    pub anilist_id: AnilistId,
    pub title: String,
    pub episode: String,
    pub state: &'static str,
    pub fraction: f64,
    pub downloaded: u64,
    pub total: u64,
    /// Why it failed, when it did. Kept on the row because a toast lasts seconds and the reason
    /// still matters afterwards.
    pub error: Option<String>,
    /// Where the file is, once it exists — so a finished download can be played.
    pub path: Option<String>,
}

/// One provider, as the Providers screen shows it.
///
/// A flattened snapshot rather than the live tracker, so the UI crate stays independent of
/// the provider implementations.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRow {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    /// Whether resolution will actually try this source.
    pub usable: bool,
    /// Withheld by local policy rather than broken — a failing VPN guard, say.
    pub held_back: bool,
}

/// How one configured source stands, for the header chip that names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceState {
    /// At least one of its providers will be tried.
    Ready,
    /// Switched off in config. Said quietly: off is a choice, not a fault.
    Off,
    /// Working, but withheld by local policy — the VPN guard.
    HeldBack,
    /// Registered and not answering, or refused at startup.
    Failing,
    /// Nothing registered yet: plugins are still compiling, or there are none.
    Pending,
}

/// One rung of the metadata ladder, as the Providers screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaSourceRow {
    pub source: String,
    pub state: String,
    pub healthy: bool,
    /// The rung that answered the most recent request.
    pub active: bool,
    pub detail: Option<String>,
}

/// A plugin store this machine has joined, as the store screens show it.
///
/// Flattened by the binary from the store client's own state, so this crate knows nothing about
/// the protocol — the same arrangement as [`ProviderRow`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRow {
    pub id: String,
    pub name: String,
    pub url: String,
    /// The store key's fingerprint, in groups of four.
    pub fingerprint: String,
    /// Who this machine joined as.
    pub member: String,
    pub owner: bool,
    pub state: StoreState,
    pub plugins: Vec<StorePluginRow>,
}

impl StoreRow {
    pub fn updates(&self) -> usize {
        self.plugins
            .iter()
            .filter(|p| matches!(p.status, StorePluginStatus::Update { .. }))
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreState {
    Ready,
    /// The last refresh failed. The plugins listed are the last list that was verified.
    Offline(String),
    /// The store no longer accepts this device.
    Revoked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    /// What the plugin may contact. Shown before anything is installed.
    pub hosts: Vec<String>,
    pub status: StorePluginStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorePluginStatus {
    Available,
    Installed,
    /// A newer version is offered, and these are the hosts it adds.
    Update {
        from: String,
        new_hosts: Vec<String>,
    },
    /// A file with this id is in the plugin directory and did not come from a store.
    AddedByHand,
    /// Installed from another store, which keeps it updated.
    OtherStore(String),
}

/// A store about to be joined, for the confirmation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorePreview {
    pub link: Secret,
    pub name: String,
    pub url: String,
    pub fingerprint: String,
    pub already_joined: bool,
}

/// A store action waiting for a yes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreConfirm {
    Join(StorePreview),
    Leave { store: String, name: String },
    RemoveMember { store: String, member: String, name: String, devices: u32 },
    CancelInvite { store: String, invite: String, name: String },
    Unpublish { store: String, plugin: String, name: String },
}

/// A freshly made invite link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteLinkView {
    pub store: String,
    pub link: Secret,
    /// Who it is for. `None` is another device of your own.
    pub for_whom: Option<String>,
    pub expires_at: i64,
    /// Whether it was handed to the terminal's clipboard.
    pub copied: bool,
}

/// One member of a store, for its owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreMemberRow {
    pub id: String,
    pub name: String,
    pub owner: bool,
    pub devices: u32,
    pub last_seen: Option<i64>,
    pub you: bool,
}

/// An invite nobody has used yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreInviteRow {
    pub id: String,
    pub name: String,
    pub device: bool,
    pub expires_at: i64,
}

/// A plugin in the local plugin directory that an owner could publish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub hosts: Vec<String>,
    /// The version the store already offers, if it offers one.
    pub published: Option<String>,
}

/// One line of a store's screen: a plugin, or something to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreLine {
    Plugin(usize),
    LinkDevice,
    Invite,
    Members,
    Publish,
    /// The store's admin page, in a browser. Owners only.
    Admin,
    Leave,
}

impl StoreLine {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plugin(_) => "",
            Self::LinkDevice => "link another device",
            Self::Invite => "invite someone",
            Self::Members => "members",
            Self::Publish => "publish a plugin",
            Self::Admin => "open the admin page",
            Self::Leave => "leave this store",
        }
    }
}

/// What can be done with a store's plugin from its overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginChoice {
    Install,
    Update(String),
    /// Overwrite a file of the same name that was added by hand.
    Replace,
    Remove,
    /// Stop offering it. Owners only.
    Unpublish,
    Cancel,
}

impl PluginChoice {
    pub fn label(&self) -> String {
        match self {
            Self::Install => "install".into(),
            Self::Update(version) => format!("update to {version}"),
            Self::Replace => "replace the copy added by hand".into(),
            Self::Remove => "remove from this machine".into(),
            Self::Unpublish => "stop offering it in this store".into(),
            Self::Cancel => "cancel".into(),
        }
    }
}

/// A watched episode still seeding, as the Downloads screen lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedRow {
    pub name: String,
    pub uploaded_bytes: u64,
    /// Uploaded over what this machine has of it.
    pub ratio: f64,
    /// The ratio it is seeding towards.
    pub goal: f64,
    /// Until the time limit retires it.
    pub left_secs: u64,
}

/// One row of the timing-sheet episode table.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeRow {
    pub number: String,
    pub title: Option<String>,
    /// Still frame for this episode, when one was published.
    ///
    /// Only ever drawn for the selected row: the table is dense, and one image that follows the
    /// cursor costs a single fetch per move rather than one per visible row.
    pub thumbnail: Option<String>,
    /// One-paragraph synopsis, when the source has one.
    ///
    /// Shown for the selected row only, under its still, for the same reason the still is:
    /// the table is a timing sheet, and a paragraph per row would drown it.
    pub description: Option<String>,
    pub duration_secs: Option<u64>,
    /// How far through this episode the local history says we are.
    pub watched: f64,
    pub completed: bool,
    /// `filler`, `mixed`, `canon` — from AnimeFillerList, when the show is covered.
    ///
    /// `None` means unknown, which is the common case: most shows have no filler and are not in the
    /// index at all. Rendered only when present, so an uncovered show shows no column rather than a
    /// column of blanks.
    pub kind: Option<&'static str>,
    /// Whether offering to skip this whole episode is reasonable.
    ///
    /// Only pure filler. `mixed` episodes contain filler *and* canon, so skipping one loses story —
    /// the distinction is the whole reason this is a separate field rather than `kind == "filler"`
    /// at every call site.
    pub skippable: bool,
}

impl EpisodeRow {
    /// Runtime as `mm:ss`, which is how a timing sheet reads.
    pub fn runtime(&self) -> String {
        match self.duration_secs {
            Some(secs) => format!("{}:{:02}", secs / 60, secs % 60),
            None => "--:--".into(),
        }
    }
}

/// One candidate offered when the resolution ladder could not decide.
///
/// Carries the score it was ranked with, because "which of these is your show" is a question
/// the user can only answer if they can see *why* the app hesitated — two candidates at 0.62
/// and 0.61 is a genuinely close call, one at 0.88 against one at 0.30 means something else
/// went wrong and the list is worth distrusting.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCandidate {
    /// The provider's own name for this result.
    pub title: String,
    /// The key to pin, if this is the one.
    pub key: anistream_core::ids::ProviderKey,
    /// Title-match similarity, 0..1.
    pub similarity: f64,
    /// Why the ladder ruled it out, when it did.
    pub rejected: Option<&'static str>,
}

/// Live playback state for the Now Playing surface.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NowPlaying {
    /// Which title is playing, so a watch is reflected onto that entry and no other —
    /// the Home list can hold several shows whose next episode shares a number.
    pub id: Option<AnilistId>,
    pub title: String,
    pub episode: String,
    pub episode_title: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub paused: bool,
    pub speed: f64,
    /// Set while a skip is offered: the label and where it would seek to.
    pub skip: Option<(&'static str, f64)>,
}

impl NowPlaying {
    pub fn fraction(&self) -> f64 {
        match self.duration {
            Some(d) if d > 0.0 => (self.position / d).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// `m:ss` — the form a player uses, not a wall clock.
    pub fn clock(seconds: f64) -> String {
        let total = seconds.max(0.0) as u64;
        let (hours, minutes, secs) = (total / 3600, (total % 3600) / 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{secs:02}")
        } else {
            format!("{minutes}:{secs:02}")
        }
    }

    /// Elapsed, padded to the width of the total.
    ///
    /// `9:12 / 23:55` is ragged; ` 9:12 / 23:55` is a clock. The pair sits in a fixed field so
    /// the digits do not shuffle sideways once a minute.
    pub fn elapsed(&self) -> String {
        let elapsed = Self::clock(self.position);
        let width = self.total().chars().count();
        format!("{elapsed:>width$}")
    }

    pub fn total(&self) -> String {
        self.duration.map_or_else(|| "--:--".into(), Self::clock)
    }
}

/// How a playback came to an end.
///
/// More than `watched` can say: an episode closed past the commit threshold counts as watched,
/// and rolling on from it would open a new window on someone who just closed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// mpv played the file to its end. `next` is the episode the player kept its window open
    /// for — set only when the episode rolls on, which the playback decides: auto-next on, Up
    /// Next not cancelled, and an episode list that has a next episode. Auto-next follows this
    /// and nothing else, so a window is never kept for an episode that is not coming, and no
    /// episode is searched for that the list says does not exist.
    Complete { next: Option<String> },
    /// The viewer stopped it from the terminal.
    Stopped,
    /// The player window went away: the viewer closed it, or mpv exited on its own.
    Closed,
    /// It could not play: no stream was found, mpv would not start, or the stream failed.
    Failed,
    /// Something else took the stream: a watch party, or the browser for a licensed stream.
    Elsewhere,
}

/// One segment of the Library screen.
///
/// These are the tracker's own statuses rather than something local, because the Library screen
/// *is* a view of the tracker's list — the local equivalent is the `CONTINUE` rail on Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibrarySegment {
    #[default]
    Watching,
    Planning,
    Completed,
    Paused,
    Dropped,
}

impl LibrarySegment {
    pub const ALL: [Self; 5] =
        [Self::Watching, Self::Planning, Self::Completed, Self::Paused, Self::Dropped];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::Planning => "planning",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Dropped => "dropped",
        }
    }

    /// The tracker-side status string this segment corresponds to.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Watching => "CURRENT",
            Self::Planning => "PLANNING",
            Self::Completed => "COMPLETED",
            Self::Paused => "PAUSED",
            Self::Dropped => "DROPPED",
        }
    }

    /// The neighbouring segment, stopping at both ends.
    ///
    /// Clamps rather than wraps, the same rule as the rail, every list and every value ladder:
    /// the ends are informative, and the left end is where Left carries on to the rail.
    pub fn step(self, delta: isize) -> Self {
        let all = Self::ALL;
        let current = all.iter().position(|s| *s == self).unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, all.len() as isize - 1) as usize;
        all[next]
    }
}

/// Sync state for one tracker, as the Accounts overlay and the status badge show it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncState {
    pub tracker: String,
    pub connected: bool,
    /// Which account, once known.
    pub user: Option<String>,
    /// Where the token lives, and whether that is the degraded option.
    pub storage: Option<String>,
    pub storage_degraded: bool,
    /// Queued operations waiting to go out.
    pub outbox: u32,
    /// How many of those wait for an id mapping: titles this tracker cannot be told about
    /// until the mapping datasets name them.
    pub deferred: u32,
    /// The tracker rejected our credentials; nothing but re-authorising helps.
    pub needs_reauth: bool,
    /// Most recent sync outcome, in words.
    pub last: Option<String>,
    /// Why the last library pull failed, and when (epoch seconds). Cleared by the next pull
    /// that succeeds, so a row never goes on saying "pulled" over an outage.
    pub pull_error: Option<(String, i64)>,
}

impl SyncState {
    /// A tracker that is configured but signed out.
    pub fn new(tracker: impl Into<String>) -> Self {
        Self {
            tracker: tracker.into(),
            connected: false,
            user: None,
            storage: None,
            storage_degraded: false,
            outbox: 0,
            deferred: 0,
            needs_reauth: false,
            last: None,
            pull_error: None,
        }
    }

    /// The status-line badge.
    ///
    /// The queue depth is the number worth showing: it is the answer to "did my progress
    /// actually go anywhere?", which is the only sync question anyone asks.
    pub fn badge(&self) -> String {
        if self.needs_reauth {
            return format!("{} {} sign in", self.tracker, glyph::STATE_DOWN);
        }
        // Hollow, not the interpunct: the header puts `·` between its groups of chips, and a
        // signed-out `kitsu ·` read as a separator with a word in front of it.
        if !self.connected {
            return format!("{} {}", self.tracker, glyph::STATE_OFF);
        }
        match self.outbox {
            0 => format!("{} {}", self.tracker, glyph::SYNC),
            n => format!("{} {} {n}", self.tracker, glyph::SYNC),
        }
    }

    /// Whether the badge should use the alert role.
    pub fn is_alerting(&self) -> bool {
        self.needs_reauth
    }
}

/// One unresolved divergence, for the Conflicts overlay.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictRow {
    pub anilist_id: AnilistId,
    pub title: String,
    pub field: String,
    pub local: String,
    pub remote: String,
}

/// What the stage is currently showing.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Content {
    #[default]
    Empty,
    Loading,
    Entries(Vec<Entry>),
    /// A per-provider or network failure, shown in place of content.
    ///
    /// Never an empty list with no explanation: when something breaks the user must be
    /// able to see *what*.
    Failed(String),
}

impl Content {
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Entries(e) => e,
            _ => &[],
        }
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Which list an [`Update::Content`] answers.
///
/// Requests are not cancelled when the user moves on, and the AniList rate limiter spaces them
/// unevenly, so the answer for a section just left can land after the switch. Untagged, it was
/// shown under the new section's heading until that section's own answer replaced it. The reducer
/// takes a list only while the view it was fetched for is still the one on screen.
#[derive(Debug, Clone, PartialEq)]
pub enum ContentFor {
    Continue,
    Seasonal,
    Calendar,
    /// The query and filters as submitted, so a slow answer to an earlier search cannot replace
    /// the results of a later one.
    Search {
        query: String,
        filter: SearchFilters,
    },
    Library(LibrarySegment),
}

/// A computed change, applied on the UI thread.
///
/// Async work never touches [`App`] directly; it sends one of these instead. That is what
/// guarantees the render loop is never blocked on I/O.
#[derive(Debug)]
pub enum Update {
    /// What a running [`Job`] is held up on, and until when.
    JobNote {
        job: JobId,
        note: String,
        until: Option<Instant>,
    },
    /// The work carrying this job has ended, whether it succeeded or not.
    JobDone(JobId),
    /// A list, tagged with the view that asked for it. See [`ContentFor`].
    Content {
        view: ContentFor,
        content: Content,
    },
    /// What a list is, said above it: which season Seasonal is, and that a list stopped at the
    /// size of one page when there was more. Tagged like [`Update::Content`] and sent before it,
    /// so a caption never describes a list it did not come with.
    Caption {
        view: ContentFor,
        text: String,
    },
    /// How to continue the calendar that is arriving: the anchor its pages are numbered
    /// against, how many are loaded, and whether AniList has more behind them.
    CalendarWindow {
        anchor: i64,
        pages: u32,
        more_past: bool,
    },
    /// Older broadcasts, to go above what is already on screen.
    ///
    /// `page` is `None` when the fetch failed — nothing to splice in, and the same page is
    /// tried again on the next scroll.
    CalendarBackfill {
        page: Option<u32>,
        entries: Vec<Entry>,
        more_past: bool,
    },
    Detail(Box<Entry>),
    Status(String),
    Toast(Toast),
    Providers(Vec<ProviderRow>),
    /// The metadata ladder's condition: which rung is serving, and why.
    MetaStatus(Vec<MetaSourceRow>),
    /// A setting reached disk. The run loop republishes the configuration its background
    /// tasks read, so saved and in-effect are the same moment.
    SettingsSaved,
    /// A settings write was attempted, and this is what the file holds afterwards. `failed`
    /// names the setting whose write did not land, so the screen can put it back rather than
    /// go on showing a value that was never saved.
    SettingWritten {
        on_disk: Box<Config>,
        failed: Option<(&'static [&'static str], &'static str)>,
    },
    /// A setting changed what sources should exist. Acted on by the run loop, which owns
    /// the torrent session and the VPN guard; the reducer only needs to not choke on it.
    SourcesChanged,
    /// Why no sources are available, when none are.
    ProviderNote(String),
    /// The source pinned for the title on screen, or `None` for automatic.
    ProviderPreference(Option<String>),
    /// The ladder found candidates but could not choose between them.
    MatchChoices {
        id: AnilistId,
        provider_id: String,
        candidates: Vec<MatchCandidate>,
    },
    /// The selectable releases for the episode the user asked about, named so an answer for
    /// an episode whose picker has since closed is not offered against the next one.
    Sources {
        id: AnilistId,
        episode: String,
        candidates: Vec<SourceCandidate>,
    },
    /// The user changed the player volume.
    PlaybackVolume(f64),
    /// Which provider the current stream actually came from.
    ActiveProvider(String),
    /// A key pressed inside the player window: step to an adjacent episode.
    PlayerStepEpisode(i64),
    /// The player asked for an episode of what is playing: its Episodes menu, Up Next's
    /// "play now", N/P inside the window, or the terminal's n/N. `title` is the episode's name
    /// from the episode list the playback holds, which is that title's whatever is open here.
    PlayerPlayEpisode {
        episode: String,
        title: Option<String>,
    },
    /// A playback was handed to its successor mid-episode. Reflected like an ending, without
    /// taking down the Now Playing surface the successor has already put up.
    EpisodeLeft {
        id: AnilistId,
        episode: String,
        position: f64,
        duration: Option<f64>,
        watched: bool,
    },
    /// Sub or dub, picked in the player: the episode restarts in the other one, where it was.
    PlayerTranslation(anistream_core::media::Translation),
    /// An upscaling mode picked in the player, by its config name.
    PlayerUpscaling(String),
    /// Night mode toggled in the player.
    PlayerNightMode(bool),
    /// An HDR tone-mapping curve picked in the player.
    PlayerToneMapping(String),
    /// The episode playing has reached its ending: find the next one's streams now, so Up Next
    /// can start it without a search.
    PrefetchNext {
        id: AnilistId,
        episode: String,
    },
    /// Playhead moved.
    Playback {
        position: f64,
        duration: Option<f64>,
        paused: bool,
    },
    /// A skip is available at the current position.
    SkipAvailable {
        label: &'static str,
        to: f64,
    },
    SkipCleared,
    PlaybackSpeed(f64),
    /// A playback is over. `watched` is the tracker's verdict, for the meters; `ending` is how
    /// it came to an end, which is what decides whether anything plays next.
    PlaybackEnded {
        watched: bool,
        ending: Ending,
    },
    /// Playback started from a stored position rather than the beginning.
    Resumed {
        position: f64,
    },
    /// A tracker's sync state changed.
    Sync(Box<SyncState>),
    /// An episode's progress was queued for sync. Bumps the badge without waiting for a drain.
    ProgressQueued,
    /// Divergences from a library pull, replacing whatever was there.
    Conflicts(Vec<ConflictRow>),
    /// A title's episode table, named so a slow load for a title already left cannot fill the
    /// table of the one opened since.
    Episodes {
        id: AnilistId,
        rows: Vec<EpisodeRow>,
    },
    /// VPN state changed. `leaking` drives the alert colouring.
    Vpn {
        badge: String,
        leaking: bool,
    },
    /// A decoded cover or banner, ready to be turned into a render protocol.
    Image {
        url: String,
        image: Box<image::DynamicImage>,
    },
    /// A cover could not be fetched or decoded. Silent: the plate stays, and the URL is tried
    /// again later. `offline` means the server was not reached at all.
    ImageFailed {
        url: String,
        offline: bool,
    },
    /// The download queue, replacing whatever was there.
    Downloads(Vec<DownloadRow>),
    /// Watched episodes still seeding, replacing whatever was there.
    Seeding(Vec<SeedRow>),
    /// A device-flow code the user has to type somewhere else.
    ///
    /// Its own update rather than a status string or a toast, because it has to survive for up to
    /// fifteen minutes while the app carries on: a toast fades in nine seconds, and any background
    /// task sending a status — a rate-limit notice, a finished download — would wipe it. `None`
    /// clears it when the flow ends.
    DeviceCode(Option<DeviceCodePrompt>),
    /// Latest broadcast per title, merged into whatever is on screen.
    ///
    /// Arrives *after* the list it annotates, because it takes a second request that depends on
    /// which titles came back. The list renders immediately without it and gains the line when
    /// it lands, rather than the whole screen waiting on an embellishment.
    LastAired(Vec<(AnilistId, u32, i64)>),
    /// Every joined plugin store, replacing whatever was there.
    Stores(Vec<StoreRow>),
    /// What a pasted invite link leads to, or why it leads nowhere.
    StorePreview(Result<StorePreview, String>),
    /// A store was joined. The view moves into it if the user is still on the store list.
    StoreJoined {
        store: String,
    },
    /// An invite link to show and send.
    InviteLink(InviteLinkView),
    /// A store's members and unused invites, for its owner.
    StoreMembers {
        store: String,
        members: Vec<StoreMemberRow>,
        invites: Vec<StoreInviteRow>,
    },
    /// Local plugins an owner could publish to a store.
    LocalPlugins {
        store: String,
        plugins: Vec<LocalPluginRow>,
    },
}

/// The rows a screen's cursor walks. See [`App::rows_here`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    /// The section's list: `content`, `selected` and `offset`.
    List,
    Downloads,
    Accounts,
    Settings,
    Episodes,
    Stores,
    StoreLines,
    Members,
}

/// A title page left for one of its relations or recommendations.
///
/// Opening a related title replaces `detail` and, once its episodes are asked for, the episode
/// table; Back pops the view but nothing put them back, so the parent's page and table came back
/// showing the child. The frame keeps what the parent's view was showing, keyed by the stage
/// depth of that view so a frame can only ever be restored into the view it was taken from.
#[derive(Debug, Clone)]
struct TitleFrame {
    depth: usize,
    entry: Entry,
    episodes_all: Vec<EpisodeRow>,
    episode_selected: usize,
    provider_preference: Option<String>,
}

/// The whole application state.
pub struct App {
    pub nav: Nav,
    pub palette: Palette,
    pub keymap: Keymap,
    /// The configuration in effect: what the file says, plus anything changed for this session
    /// only — `t` for sub or dub, `--theme`.
    pub config: Config,
    /// What `config.toml` holds, read back after every settings write. The Settings screen
    /// shows these values and marks a session-only difference, and a write that fails puts
    /// its setting back to this.
    pub saved_config: Config,
    pub content: Content,
    /// The caption for the list on screen, with the view it belongs to. See [`Update::Caption`].
    pub caption: Option<(ContentFor, String)>,
    /// Selection index within the current content.
    pub selected: usize,
    /// Scroll offset, so long lists page rather than jumping.
    pub offset: usize,
    pub search_query: String,
    /// The query the results on screen belong to.
    ///
    /// Lets Enter mean "run this" while the query is new and "open this" once it is not, which is
    /// the difference between a search box you can act on and one that only ever re-submits.
    pub searched_query: String,
    /// Server-side filters the next search will carry. Sticky across queries on purpose:
    /// narrowing to a genre and then trying three titles should not mean setting it thrice.
    pub search_filter: SearchFilters,
    /// Whether the keyboard is in the search results rather than the query field.
    ///
    /// The field takes every printable key, so `j`, `k` and the title keys could never reach
    /// the results while it had focus. ↓ moves from the field into the results, where they are
    /// a list like any other; ↑ from the first row, `/` and Esc go back to typing.
    pub search_in_results: bool,
    pub palette_query: String,
    /// The Kitsu sign-in prompts' buffers. The password is taken, not copied, when the
    /// exchange is dispatched, and both are cleared on every open and dismiss.
    pub kitsu_user: String,
    pub kitsu_pass: String,
    /// The title the Title or Episodes view on top is showing, or was last showing.
    ///
    /// It outlives that view on purpose — playback reads the loaded title's episode table after
    /// the viewer has stepped back to a list — so nothing that acts on "the title here" reads it
    /// directly. [`App::target`] is that question, and it only answers with `detail` where it is
    /// the title on screen.
    pub detail: Option<Entry>,
    /// The title pages a relation was opened from, so Back shows each one again as it was.
    title_trail: Vec<TitleFrame>,
    pub status: String,
    pub toasts: Vec<Toast>,
    /// A problem that lasts the session, shown on every screen until the app is restarted —
    /// a config file that did not load, so everything on screen is the defaults. A toast is
    /// gone in seconds, and the one place this was said before was the terminal, just before
    /// the interface covered it.
    pub notice: Option<String>,
    pub should_quit: bool,
    /// Set when the synopsis is expanded on the Title screen.
    pub synopsis_expanded: bool,
    /// Decoded artwork, keyed by URL.
    pub images: crate::image::ImageStore,
    /// VPN state, shown in the header whenever torrenting is configured.
    ///
    /// Visible rather than discoverable: finding out the guard is failing when playback
    /// mysteriously refuses would be much worse than a badge.
    pub vpn_badge: Option<String>,
    /// Set when the guard is failing, so the badge can use the alert role.
    pub vpn_leaking: bool,
    /// The download queue, as the Downloads screen shows it.
    pub downloads: Vec<DownloadRow>,
    /// Watched episodes still seeding, for the Downloads screen.
    pub seeding: Vec<SeedRow>,
    /// A device code awaiting approval, shown on the Accounts screen.
    pub device_code: Option<DeviceCodePrompt>,
    /// Snapshot for the Providers screen.
    pub providers: Vec<ProviderRow>,
    /// The metadata ladder's rungs, for the Providers screen and the header chip.
    pub meta_status: Vec<MetaSourceRow>,
    /// The source pinned for the title on screen. `None` means automatic.
    pub provider_preference: Option<String>,
    /// Why there are no sources, when there are none.
    ///
    /// "You have not configured anything" and "your VPN guard failed" need different
    /// actions from the user, so the screen must not guess between them.
    pub provider_note: Option<String>,
    /// Live playback state, when something is playing.
    pub playing: Option<NowPlaying>,
    /// The wipe covering stream resolution, when one is running.
    pub eyecatch: Option<Eyecatch>,
    /// Which Library segment is showing.
    pub library_segment: LibrarySegment,
    /// Sync state per tracker, for the badge and the Accounts overlay.
    pub sync: Vec<SyncState>,
    /// Divergences the user has to settle.
    pub conflicts: Vec<ConflictRow>,
    /// List statuses given with `a` this session, by title, until a Library pull brings the
    /// tracker's own answer for it.
    ///
    /// Not only on the rows, which every reload rebuilds: Home reloaded, and `a` opened at the
    /// top of the picker rather than on the status just given.
    statuses_set: std::collections::HashMap<AnilistId, LibrarySegment>,
    /// Selection within an overlay list (Accounts, Conflicts, list status, command palette).
    pub overlay_selected: usize,
    /// The first row a list overlay showed at the last frame.
    ///
    /// Written by the renderer, which is the only place that knows how many rows fit. Kept so the
    /// window moves only when the highlight would leave it, rather than recomputed from the
    /// highlight each frame and pinning it to the bottom edge on the way back up.
    pub overlay_offset: std::cell::Cell<usize>,
    /// The help overlay's page. Its own field because Help opens on top of a picker, and paging
    /// through it must not move the picker's selection underneath.
    pub help_page: usize,
    /// How many pages the help table ran to at the last frame, written by the renderer — only it
    /// knows how the table breaks at this size — so paging stops at the last page rather than
    /// running past it and making the next few presses look dead.
    pub help_pages: std::cell::Cell<usize>,
    /// Frame counter for the loading pulse, advanced by the idle tick.
    pub pulse: u64,
    /// Idle ticks left before a pending section change is actually fetched.
    ///
    /// Rail navigation has to feel instant, but it must not spend a request per keystroke: at
    /// AniList's measured 30/minute, arrowing across eight sections twice exhausts the whole
    /// budget and the token bucket then makes *everything* crawl — which is precisely the "suddenly
    /// takes a very long time" this exists to prevent. The highlight moves immediately; the fetch
    /// waits for the selection to settle.
    reload_countdown: Option<u8>,
    /// Whether the episode table is waiting on a provider.
    ///
    /// Separate from `episodes.is_empty()`, which cannot tell "still loading" from "this source has
    /// no episodes" — and those need opposite messages.
    pub episodes_loading: bool,
    /// Running work, oldest first. See [`Job`].
    pub jobs: Vec<Job>,
    next_job: u64,
    /// The palette variant detection settled on at startup.
    ///
    /// Kept so switching *out* of immersive mode can restore the right one without re-querying
    /// the terminal's background from inside the render loop.
    adaptive_variant: crate::theme::Variant,
    /// Recent errors and notices, for the Logs overlay.
    pub logs: Vec<LogRow>,
    /// The instant the calendar on screen was fetched against, frozen so its page numbering
    /// stays meaningful while older pages are pulled in. `None` until a calendar has loaded.
    calendar_anchor: Option<i64>,
    /// Pages of past the calendar holds. The backfill asks for the one after.
    calendar_pages: u32,
    /// Whether there are older broadcasts to fetch. False once AniList says the window is spent.
    calendar_more_past: bool,
    /// A backfill is in flight. Without this a fast scroll would ask for the same page on every
    /// keystroke and empty the request budget answering one question.
    calendar_loading_past: bool,
    /// Work the reducer produced while applying an update rather than handling a key.
    ///
    /// Auto-next is one source: an episode ending has to be able to start the next one, and
    /// [`Self::apply`] has no return channel. The calendar's backfill is the other — it is
    /// triggered by scrolling, which reports no task of its own. The event loop drains this
    /// each iteration.
    pending: Option<Task>,
    /// Rows for the Episodes screen — the ones the current filter admits. Everything that
    /// reads or navigates episodes works on this list, which is what keeps the filter from
    /// needing index remapping at every call site.
    pub episodes: Vec<EpisodeRow>,
    /// Every row, unfiltered — the source [`Self::episodes`] is re-derived from.
    episodes_all: Vec<EpisodeRow>,
    /// Which title the rows are, set with them. `detail` is not enough to say: it moves to a
    /// title the moment one is opened, while the rows stay the last title's until its own load
    /// answers, and asked by `detail` they spoke for a show they were never part of.
    episodes_for: Option<AnilistId>,
    /// The active episode filter, cycled with `f`.
    pub episode_filter: EpisodeFilter,
    /// The provider that actually resolved the most recent stream. The header prefers
    /// this over the configured order: during failover the first configured source is
    /// exactly the one that is *not* serving.
    pub active_provider: Option<String>,
    /// Candidates awaiting a decision, with the provider and title they belong to.
    pub match_candidates: Vec<MatchCandidate>,
    /// Which title and provider the pending candidates are for.
    pub match_context: Option<(AnilistId, String)>,
    /// Selectable releases for the Sources overlay.
    pub sources: Vec<SourceCandidate>,
    /// Which title and episode the pending source list is for.
    pub source_context: Option<(AnilistId, String)>,
    /// What is being typed into the manual-match overlay.
    pub manual_query: String,
    /// Which title a manual search would re-match.
    pub manual_target: Option<AnilistId>,
    /// What is being typed into the download-range overlay: `4`, `1-12`, `7-`.
    pub range_query: String,
    /// What is being typed into the setting editor.
    pub edit_value: String,
    /// Which text-valued setting the editor is writing.
    pub edit_target: Option<SettingId>,
    /// Selection within the episode table, tracked separately from list selection so
    /// stepping back out of Episodes does not disturb where you were in the list.
    pub episode_selected: usize,
    /// Joined plugin stores.
    pub stores: Vec<StoreRow>,
    /// Whether the store list has arrived at least once, so "none joined" is not said early.
    pub stores_loaded: bool,
    /// Cursors for the three store screens. Separate, so stepping back out of a store finds the
    /// store list where it was.
    pub store_selected: usize,
    pub store_line_selected: usize,
    pub member_selected: usize,
    /// The store whose members are on screen, and them.
    pub members_for: Option<String>,
    pub store_members: Vec<StoreMemberRow>,
    pub store_invites: Vec<StoreInviteRow>,
    /// What is being typed into the join prompt: an invite link.
    pub join_link: String,
    /// What is being typed into the invite prompt: who it is for.
    pub invite_name: String,
    /// The store action the confirm overlay is asking about.
    pub store_confirm: Option<StoreConfirm>,
    /// The invite link overlay's link.
    pub invite_link: Option<InviteLinkView>,
    /// The plugin the plugin overlay is about: store id, plugin id.
    pub plugin_action: Option<(String, String)>,
    /// What that overlay offers, fixed when it opened. A store list arriving while it is open
    /// must not reorder the choices under the cursor and turn "stop offering it" into "remove".
    pub plugin_options: Vec<PluginChoice>,
    /// Plugins that could be published, and the store they would go to.
    pub local_plugins: Vec<LocalPluginRow>,
    pub publish_for: Option<String>,
}

impl App {
    pub fn new(config: Config, palette: Palette, keymap: Keymap) -> Self {
        Self::with_images(config, palette, keymap, crate::image::ImageEngine::disabled())
    }

    pub fn with_images(
        config: Config,
        palette: Palette,
        keymap: Keymap,
        engine: crate::image::ImageEngine,
    ) -> Self {
        Self {
            images: crate::image::ImageStore::new(engine),
            nav: Nav::new(),
            palette,
            keymap,
            saved_config: config.clone(),
            config,
            content: Content::Loading,
            caption: None,
            selected: 0,
            offset: 0,
            search_query: String::new(),
            searched_query: String::new(),
            search_filter: SearchFilters::default(),
            search_in_results: false,
            palette_query: String::new(),
            kitsu_user: String::new(),
            kitsu_pass: String::new(),
            detail: None,
            title_trail: Vec::new(),
            status: String::new(),
            toasts: Vec::new(),
            notice: None,
            should_quit: false,
            synopsis_expanded: false,
            playing: None,
            eyecatch: None,
            library_segment: LibrarySegment::default(),
            sync: Vec::new(),
            conflicts: Vec::new(),
            statuses_set: std::collections::HashMap::new(),
            overlay_selected: 0,
            overlay_offset: std::cell::Cell::new(0),
            help_page: 0,
            help_pages: std::cell::Cell::new(1),
            pulse: 0,
            downloads: Vec::new(),
            seeding: Vec::new(),
            device_code: None,
            reload_countdown: None,
            episodes_loading: false,
            jobs: Vec::new(),
            next_job: 0,
            adaptive_variant: match palette.variant {
                // Started in immersive, so there is no detected variant to remember. Dark is the
                // safer default of the two, and a restart re-runs real detection.
                crate::theme::Variant::Immersive => crate::theme::Variant::Dark,
                detected => detected,
            },
            logs: Vec::new(),
            calendar_anchor: None,
            calendar_pages: 0,
            calendar_more_past: false,
            calendar_loading_past: false,
            pending: None,
            vpn_badge: None,
            vpn_leaking: false,
            providers: Vec::new(),
            meta_status: Vec::new(),
            provider_preference: None,
            provider_note: None,
            episodes: Vec::new(),
            episodes_all: Vec::new(),
            episodes_for: None,
            episode_filter: EpisodeFilter::default(),
            active_provider: None,
            episode_selected: 0,
            match_candidates: Vec::new(),
            match_context: None,
            sources: Vec::new(),
            source_context: None,
            manual_query: String::new(),
            manual_target: None,
            range_query: String::new(),
            edit_value: String::new(),
            edit_target: None,
            stores: Vec::new(),
            stores_loaded: false,
            store_selected: 0,
            store_line_selected: 0,
            member_selected: 0,
            members_for: None,
            store_members: Vec::new(),
            store_invites: Vec::new(),
            join_link: String::new(),
            invite_name: String::new(),
            store_confirm: None,
            invite_link: None,
            plugin_action: None,
            plugin_options: Vec::new(),
            local_plugins: Vec::new(),
            publish_for: None,
        }
    }

    /// The highlighted row of the list on screen, when the screen on top is one.
    ///
    /// `content` stays put while a title is pushed over it, and screens with rows of their own
    /// (Settings, Accounts, Downloads) never replace it, so an unconditional read answered with
    /// a row nobody could see: Enter on a title page opened it, and `a` on Accounts asked for
    /// the status of a show from the list before.
    pub fn selected_entry(&self) -> Option<&Entry> {
        self.content_view()?;
        if !matches!(self.nav.current(), StageView::Section(_)) {
            return None;
        }
        self.content.entries().get(self.selected)
    }

    /// The title the screen on top is about: the one every title action means.
    ///
    /// On a list it is the highlighted row — as the fuller record of the same title when that is
    /// the one last opened, because relations, recommendations and the trailer only ride the
    /// detail fetch. On a title page or its episode table it is that title, and on Now Playing
    /// the title playing, where the app holds its record. Anywhere else it is nothing, and the
    /// action says so rather than reaching for a title off screen.
    ///
    /// Reading `detail` first, as the handlers used to, acted on whichever title was opened
    /// last: back on the list, `a`, `*`, Space and the rest all went to that show rather than
    /// the highlighted one.
    pub fn target(&self) -> Option<&Entry> {
        match self.nav.current() {
            StageView::Title(id) | StageView::Episodes(id) => {
                self.detail.as_ref().filter(|d| d.id == *id)
            }
            StageView::Section(_) => {
                let row = self.selected_entry()?;
                Some(self.detail.as_ref().filter(|d| d.id == row.id).unwrap_or(row))
            }
            StageView::NowPlaying => {
                let id = self.playing.as_ref()?.id?;
                self.detail.iter().chain(self.content.entries()).find(|e| e.id == id)
            }
            StageView::Stores | StageView::Store(_) | StageView::StoreMembers(_) => None,
        }
    }

    /// The key for `action` as a sentence names it: the first one the help overlay lists, so a
    /// rebind changes what the sentence says. Unbound, it points at the palette instead.
    pub fn key_for(&self, action: Action) -> String {
        let mut keys = self.keymap.keys_for(action);
        keys.sort();
        keys.first().map_or_else(
            || format!("\u{201c}{}\u{201d} in the palette", action.label().to_lowercase()),
            crate::keymap::Binding::render,
        )
    }

    /// The key that jumps to `section`.
    pub fn section_key(&self, section: Section) -> String {
        self.key_for(Action::JumpSection(section.index() as u8 + 1))
    }

    /// Apply an update from a background task.
    pub fn apply(&mut self, update: Update) {
        // Resolution has finished, one way or the other — let the wipe complete. An alert is
        // as much a result as a first frame is, and holding the band over a failure would
        // leave the user staring at amber.
        if matches!(
            update,
            Update::Playback { .. }
                | Update::PlaybackEnded { .. }
                | Update::Toast(Toast { kind: ToastKind::Alert, .. })
        ) && let Some(eyecatch) = &mut self.eyecatch
        {
            eyecatch.release();
        }

        match update {
            Update::CalendarWindow { anchor, pages, more_past } => {
                // Paging for a calendar the user has left would let a scroll elsewhere ask for
                // history nobody is looking at.
                if self.nav.section() != Section::Calendar {
                    return;
                }
                self.calendar_anchor = Some(anchor);
                self.calendar_pages = pages;
                self.calendar_more_past = more_past;
                self.calendar_loading_past = false;
            }
            Update::CalendarBackfill { page, entries, more_past } => {
                self.calendar_loading_past = false;
                self.calendar_more_past = more_past;
                if let Some(page) = page {
                    self.calendar_pages = self.calendar_pages.max(page);
                }
                // Only the screen that asked can take them. A backfill landing after the user
                // has moved on would otherwise splice a fortnight of broadcasts into whatever
                // list is on screen now.
                if !entries.is_empty()
                    && self.nav.section() == Section::Calendar
                    && let Content::Entries(existing) = &mut self.content
                {
                    let added = entries.len();
                    let mut merged = entries;
                    merged.append(existing);
                    self.content = Content::Entries(merged);
                    // The whole point of the exercise: the rows arrive *above* the viewport, so
                    // both the selection and the scroll position shift by exactly what was
                    // inserted and nothing moves under the user. Without this the screen would
                    // lurch a page and a half every time history was extended.
                    self.selected += added;
                    self.offset += added;
                }
            }
            Update::Caption { view, text } => {
                if self.content_view().as_ref() == Some(&view) {
                    self.caption = (!text.is_empty()).then_some((view, text));
                }
            }
            Update::Content { view, content } => {
                if self.content_view().as_ref() != Some(&view) {
                    return;
                }
                // A caption is sent just before the list it describes; one left over from
                // another list describes something no longer on screen.
                if self.caption.as_ref().is_some_and(|(captioned, _)| *captioned != view) {
                    self.caption = None;
                }
                self.content = content;
                // A Library pull is the tracker's own answer for the titles it lists, and it
                // outranks one given here earlier.
                if matches!(view, ContentFor::Library(_)) {
                    for entry in
                        self.content.entries().iter().filter(|e| e.list_status.is_some())
                    {
                        self.statuses_set.remove(&entry.id);
                    }
                }
                // Selection must stay in range or the next render would index past the end.
                self.selected = self.selected.min(self.content.len().saturating_sub(1));
                if self.content.is_empty() {
                    self.selected = 0;
                    self.offset = 0;
                }
                // The calendar is a timeline running from a week ago to a week ahead, so landing
                // on row nought would open it on the *oldest* episode in view. Today is the
                // useful place to be: the window opens at today's first entry — everything that
                // already aired today in view above — with the selection on the next thing to
                // happen. Capped so a very broadcast-heavy day cannot push the selection below
                // a short terminal's fold.
                if self.nav.section() == Section::Calendar
                    && let Content::Entries(entries) = &self.content
                    && let Some(next) =
                        entries.iter().position(|e| e.airing_in.is_some_and(|secs| secs > 0))
                {
                    let today = chrono::Local::now().date_naive();
                    let start_of_today = entries
                        .iter()
                        .position(|e| {
                            e.airing_in
                                .is_some_and(|secs| air_date(secs).is_some_and(|d| d >= today))
                        })
                        .unwrap_or(next);
                    self.selected = next;
                    self.offset =
                        next.saturating_sub((next - start_of_today.min(next)).min(12));
                }
            }
            // Every way into a title sets a stand-in `detail` for it before the fetch goes out, so
            // a full record for any other id answers a title the user has already left.
            Update::Detail(mut entry) => {
                if let Some(detail) = self.detail.as_ref().filter(|d| d.id == entry.id) {
                    // The record does not carry the tracker's status; the row it was opened
                    // from may, and losing it here would make `a` forget what it is on.
                    entry.list_status = entry.list_status.or(detail.list_status);
                    self.detail = Some(*entry);
                }
            }
            Update::DeviceCode(prompt) => self.device_code = prompt,
            Update::Seeding(rows) => self.seeding = rows,
            Update::Downloads(rows) => {
                self.downloads = rows;
                // The cursor has to stay in range: a completed row being cleared shortens the list
                // under it.
                if self.nav.section() == Section::Downloads {
                    self.selected = self.selected.min(self.downloads.len().saturating_sub(1));
                }
            }
            Update::LastAired(rows) => {
                // Applied to both the list and any open detail, since the same title can be
                // on screen twice and a line that appears in one place but not the other
                // reads as a bug.
                if let Content::Entries(entries) = &mut self.content {
                    for entry in entries.iter_mut() {
                        if let Some((_, ep, at)) =
                            rows.iter().find(|(id, _, _)| *id == entry.id)
                        {
                            entry.last_aired = Some((*ep, *at));
                        }
                    }
                }
                if let Some(detail) = &mut self.detail
                    && let Some((_, ep, at)) = rows.iter().find(|(id, _, _)| *id == detail.id)
                {
                    detail.last_aired = Some((*ep, *at));
                }
            }
            Update::Status(status) => self.status = status,
            Update::Toast(toast) => self.push_toast(toast),
            Update::Image { url, image } => {
                self.images.insert(&url, *image);
                self.images.requests().network_returned();
            }
            Update::ImageFailed { url, offline } => self.images.mark_failed(&url, offline),
            Update::Providers(rows) => self.providers = rows,
            Update::MetaStatus(rows) => self.meta_status = rows,
            Update::Stores(rows) => self.apply_stores(rows),
            Update::StorePreview(outcome) => self.apply_store_preview(outcome),
            Update::StoreJoined { store } => {
                // Into the store just joined, but only from the list it was joined from. A join
                // that finishes after the user has gone elsewhere must not drag them back.
                if self.nav.current() == &StageView::Stores && self.store(&store).is_some() {
                    self.store_line_selected = 0;
                    self.nav.push(StageView::Store(store));
                }
            }
            Update::InviteLink(view) => {
                self.invite_link = Some(view);
                self.nav.open_overlay(Overlay::InviteLink);
            }
            Update::StoreMembers { store, members, invites } => {
                if self.members_for.as_deref() == Some(store.as_str()) {
                    self.store_members = members;
                    self.store_invites = invites;
                    let rows = self.store_members.len() + self.store_invites.len();
                    self.member_selected = self.member_selected.min(rows.saturating_sub(1));
                }
            }
            Update::LocalPlugins { store, plugins } => {
                // Only for the store still on screen; a late answer for another is dropped.
                let here = self.nav.current() == &StageView::Store(store.clone());
                if self.publish_for.as_deref() != Some(store.as_str()) || !here {
                    return;
                }
                if plugins.is_empty() {
                    self.push_toast(Toast::info(
                        "no plugins in the plugin directory to publish — `anistream --plugins` shows where it is",
                    ));
                    return;
                }
                self.local_plugins = plugins;
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::PublishPick);
            }
            // Both are handled outside the reducer, by the loop that owns the background
            // tasks and the session.
            Update::SettingsSaved => {}
            Update::SettingWritten { on_disk, failed } => {
                self.saved_config = *on_disk;
                if let Some((table, key)) = failed {
                    self.roll_back_setting(table, key);
                }
            }
            Update::SourcesChanged => self.status = "reloading sources…".into(),
            Update::ProviderNote(note) => self.provider_note = Some(note),
            Update::ProviderPreference(choice) => self.provider_preference = choice,
            Update::Sources { id, episode, candidates } => {
                // A stale answer — the user has navigated away since asking, or asked about
                // another episode — is not a question worth interrupting them with, and offering
                // one episode's releases under another's context would play the wrong one.
                if self.source_context.as_ref() != Some(&(id, episode)) {
                    return;
                }
                if candidates.is_empty() {
                    self.source_context = None;
                    self.push_toast(Toast::info("no selectable sources for this episode"));
                    return;
                }
                self.sources = candidates;
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Sources);
            }
            Update::MatchChoices { id, provider_id, candidates } => {
                // An empty set is not a question worth asking; the caller reports it as a
                // plain failure instead. Nor is one about a title the user has left.
                if candidates.is_empty() || !self.detail.as_ref().is_some_and(|d| d.id == id) {
                    return;
                }
                self.episodes_loading = false;
                self.match_candidates = candidates;
                self.match_context = Some((id, provider_id));
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Disambiguate);
            }

            Update::Playback { position, duration, paused } => {
                // Playback updates arrive about once a second and must never resurrect a
                // finished session, so they only ever mutate an existing one.
                if let Some(playing) = &mut self.playing {
                    playing.position = position;
                    playing.paused = paused;
                    if duration.is_some() {
                        playing.duration = duration;
                    }
                    let (id, episode, duration) =
                        (playing.id, playing.episode.clone(), playing.duration);
                    // Keeps the table honest while detached: `q` leaves mpv running and drops you
                    // back on the episode list, which would otherwise sit frozen at the position
                    // you started from.
                    self.reflect_watch(id, &episode, position, duration, false);
                }
            }
            Update::SkipAvailable { label, to } => {
                if let Some(playing) = &mut self.playing {
                    playing.skip = Some((label, to));
                }
            }
            Update::SkipCleared => {
                if let Some(playing) = &mut self.playing {
                    playing.skip = None;
                }
            }
            Update::PlaybackSpeed(speed) => {
                // Remembered in config so the next episode starts at the same speed.
                self.config.playback.persisted_speed = Some(speed);
                if let Some(playing) = &mut self.playing {
                    playing.speed = speed;
                }
                // And written to disk, or "survives a restart" would be a lie the doc
                // comment tells. Auto-next owns `pending` for a frame at episode end;
                // losing one save then is fine — the next speed step saves again.
                if self.config.playback.persist_speed && self.pending.is_none() {
                    self.pending = Some(Task::SaveSetting {
                        table: &["playback"],
                        key: "persisted_speed",
                        value: anistream_core::settings::SettingValue::Float(speed),
                    });
                }
            }
            Update::ActiveProvider(provider) => self.active_provider = Some(provider),
            Update::PlayerPlayEpisode { episode, title: episode_title } => {
                // By the playing title rather than the open one: the viewer may be browsing
                // another show in the terminal while the player is the thing they are using.
                if let Some(playing) = &self.playing
                    && let Some(id) = playing.id
                {
                    let title = playing.title.clone();
                    self.raise_now_playing_titled(id, title, &episode);
                    self.name_playing_episode(episode_title);
                    self.pending = Some(Task::Play { id, episode });
                }
            }
            Update::EpisodeLeft { id, episode, position, duration, watched } => {
                self.reflect_watch(Some(id), &episode, position, duration, watched);
            }
            Update::PlayerTranslation(translation) => {
                self.config.playback.translation = translation;
                self.push_toast(Toast::info(format!("now using {translation}")));
                if let Some(playing) = &self.playing
                    && let Some(id) = playing.id
                {
                    let (title, episode) = (playing.title.clone(), playing.episode.clone());
                    let episode_title = playing.episode_title.clone();
                    self.raise_now_playing_titled(id, title, &episode);
                    self.name_playing_episode(episode_title);
                    self.pending = Some(Task::Play { id, episode });
                }
            }
            Update::PlayerUpscaling(name) => {
                if let Some(mode) = anistream_core::config::Upscaling::from_name(&name) {
                    self.config.playback.upscaling = mode;
                    if self.pending.is_none() {
                        self.pending = Some(Task::SaveSetting {
                            table: &["playback"],
                            key: "upscaling",
                            value: anistream_core::settings::SettingValue::Str(name),
                        });
                    }
                }
            }
            Update::PrefetchNext { id, episode } => {
                // Best effort: an ending that arrives while something else holds the slot simply
                // plays the next episode the usual way.
                if self.pending.is_none() {
                    self.pending = Some(Task::Prefetch { id, episode });
                }
            }
            Update::PlayerToneMapping(curve) => {
                self.config.playback.tone_mapping = curve.clone();
                if self.pending.is_none() {
                    self.pending = Some(Task::SaveSetting {
                        table: &["playback"],
                        key: "tone_mapping",
                        value: anistream_core::settings::SettingValue::Str(curve),
                    });
                }
            }
            Update::PlayerNightMode(on) => {
                self.config.playback.night_mode = on;
                if self.pending.is_none() {
                    self.pending = Some(Task::SaveSetting {
                        table: &["playback"],
                        key: "night_mode",
                        value: anistream_core::settings::SettingValue::Bool(on),
                    });
                }
            }
            Update::PlayerStepEpisode(delta) => {
                // Same path the terminal's n/N keys take; `pending` because `apply` has
                // no return channel, exactly like auto-next.
                if let Some(task) = self.step_episode(delta) {
                    self.pending = Some(task);
                }
            }
            Update::PlaybackVolume(volume) => {
                // Same contract as speed: remembered in config, written to disk, and never
                // allowed to fight auto-next for the pending slot.
                self.config.playback.persisted_volume = Some(volume);
                if self.config.playback.persist_volume && self.pending.is_none() {
                    self.pending = Some(Task::SaveSetting {
                        table: &["playback"],
                        key: "persisted_volume",
                        value: anistream_core::settings::SettingValue::Float(volume),
                    });
                }
            }
            Update::PlaybackEnded { watched, ending } => {
                let finished = self.playing.take();
                if let Some(playing) = &finished {
                    self.reflect_watch(
                        playing.id,
                        &playing.episode,
                        playing.position,
                        playing.duration,
                        watched,
                    );
                }
                // Leaving Now Playing on screen with nothing playing would be a dead end.
                if matches!(self.nav.current(), StageView::NowPlaying) {
                    self.nav.pop();
                }

                // Auto-next follows the playback's decision and nothing else. That side knows how
                // the episode ended — closing the window past the commit threshold is `watched`
                // too, and rolling on from it opened a new window on someone who had just closed
                // one — whether Up Next was cancelled for this episode, and whether the episode
                // list has a next one at all. It is also the side keeping a window open for the
                // next episode, so the two cannot disagree into a window left waiting for an
                // episode nobody starts.
                let next = match &ending {
                    Ending::Complete { next } => next.clone(),
                    _ => None,
                };
                if let Some(next) = next
                    && let Some(finished) = finished
                    && let Some(id) = finished.id
                {
                    // The filler walk reads this title's episode table, when it is known here.
                    let after_filler = self.next_after_filler(id, next.clone());
                    if after_filler != next {
                        self.push_toast(Toast::info(format!(
                            "skipped filler — ep {after_filler} next"
                        )));
                    }
                    // Under the name it was playing under: whichever title is open now may be
                    // another show entirely.
                    self.raise_now_playing_titled(id, finished.title, &after_filler);
                    self.pending = Some(Task::Play { id, episode: after_filler });
                } else if matches!(ending, Ending::Complete { .. }) {
                    self.push_toast(Toast::info("episode finished"));
                }
            }
            Update::Sync(state) => {
                // Re-authorisation is the one sync condition worth interrupting for: until it
                // is done, everything queues silently and the user would have no idea.
                let announce = state.needs_reauth
                    && !self.sync.iter().any(|s| s.tracker == state.tracker && s.needs_reauth);
                if announce {
                    let accounts = self.section_key(Section::Accounts);
                    self.push_toast(Toast::alert(format!(
                        "{} needs signing in again — press {accounts} for Accounts",
                        state.tracker
                    )));
                }
                match self.sync.iter_mut().find(|s| s.tracker == state.tracker) {
                    Some(existing) => *existing = *state,
                    None => self.sync.push(*state),
                }
            }
            Update::JobNote { job, note, until } => {
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job) {
                    job.note = Some((note, until));
                }
            }
            Update::JobDone(job) => self.jobs.retain(|j| j.id != job),
            Update::ProgressQueued => {
                // Optimistic, and safe to be: the row is already in SQLite by the time this
                // arrives, so the count can only be corrected downward by the next drain.
                for state in &mut self.sync {
                    state.outbox = state.outbox.saturating_add(1);
                }
            }
            Update::Conflicts(rows) => {
                let appeared = !rows.is_empty() && self.conflicts.is_empty();
                self.conflicts = rows;
                if appeared {
                    // Surfaced rather than resolved — the whole point of not guessing.
                    let settle = self.key_for(Action::ShowConflicts);
                    self.push_toast(Toast::info(format!(
                        "{} — {settle} settles them",
                        crate::widgets::plural(self.conflicts.len(), "sync disagreement")
                    )));
                }
            }
            Update::Resumed { position } => {
                // Resume happens without asking — mpv is already at the right place by the time
                // this arrives. Saying so is what keeps it from feeling like a glitch.
                self.push_toast(Toast::info(format!(
                    "resumed at {}",
                    NowPlaying::clock(position)
                )));
                if let Some(playing) = &mut self.playing {
                    playing.position = position;
                }
            }
            Update::Vpn { badge, leaking } => {
                // A newly-failing guard is worth interrupting for: torrents have just been
                // paused and the user needs to know why playback stopped.
                if leaking && !self.vpn_leaking {
                    self.push_toast(Toast::alert(format!("{badge} — torrents paused")));
                }
                self.vpn_badge = Some(badge);
                self.vpn_leaking = leaking;
            }
            Update::Episodes { id, rows } => {
                // `detail` is the title the table is a view of; rows for any other belong to a
                // load the user has since walked away from.
                if !self.detail.as_ref().is_some_and(|d| d.id == id) {
                    return;
                }
                self.episodes_all = rows;
                self.episodes_for = Some(id);
                self.apply_episode_filter();
                self.episodes_loading = false;
                // The amber bar lands where the viewer is going: the first unwatched
                // row. Row nought means "just opened" — a cursor already mid-list is a
                // position the user chose, and a reload must not move it.
                if self.episode_selected == 0
                    && let Some(first) = self.episodes.iter().position(|r| !r.completed)
                {
                    self.episode_selected = first;
                }
            }
        }
    }

    pub fn push_toast(&mut self, toast: Toast) {
        // Every toast is also a log line. A toast lives for a few seconds and the stack is
        // capped at three, so without this the *third* provider failure silently erases the
        // first — and when a source breaks, the earliest error is usually the one that explains
        // the rest. This is the whole reason the Logs overlay exists.
        self.log(toast.kind, toast.text.clone());
        // The same message again is the same message, kept up longer rather than stacked: five
        // presses of an arrow on Settings are one "saved", not a pile of identical lines. From
        // anywhere in the stack, and brought up to newest: a copy left under another line was
        // counted in the rule's "+N", so one message read as two.
        if let Some(at) =
            self.toasts.iter().position(|t| t.text == toast.text && t.kind == toast.kind)
        {
            self.toasts.remove(at);
        }
        // Cap the stack so a burst of provider failures cannot cover the screen.
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(toast);
    }

    /// Append to the log ring, newest last.
    pub fn log(&mut self, kind: ToastKind, text: String) {
        // The same line again is a count on the last one, not another line.
        if let Some(last) = self.logs.last_mut()
            && last.kind == kind
            && last.text == text
        {
            last.count += 1;
            last.at = self.pulse;
            return;
        }
        // Bounded: this runs for the life of the process and a provider retry loop could
        // otherwise grow it without limit.
        if self.logs.len() >= LOG_CAPACITY {
            self.logs.remove(0);
        }
        self.logs.push(LogRow { kind, text, at: self.pulse, count: 1 });
    }

    /// Reflect a watch into the episode table and the title's progress, without a reload.
    ///
    /// Reported from real use: finishing an episode left the table showing the old state, so it
    /// looked as though nothing had been recorded. It had been — the row was just built once by
    /// `LoadEpisodes` and never touched again.
    ///
    /// Done locally rather than by re-issuing that task, because re-issuing it would walk the
    /// provider chain over the network to re-learn something we already know: the episode, the
    /// position and whether it finished all came from this process. A network round trip to display
    /// local state would also be slower than the eye.
    /// The live value a text setting starts editing from.
    fn current_text_value(&self, id: SettingId) -> String {
        match id {
            SettingId::Subtitles => self.config.playback.subtitle_language.clone(),
            SettingId::DownloadDir => {
                self.config.downloads.directory.clone().unwrap_or_default()
            }
            SettingId::DownloadHook => {
                self.config.downloads.on_complete.clone().unwrap_or_default()
            }
            SettingId::SyncplayServer => self.config.syncplay.server.clone(),
            SettingId::SyncplayRoom => self.config.syncplay.room.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }

    /// Re-derive the visible episode list from the full one, keeping the cursor on the same
    /// episode.
    ///
    /// By episode, not by row: keeping the index put the cursor on whatever episode the new
    /// view had in that slot, so cycling the filter moved you without a key being pressed. An
    /// episode the filter now hides hands the cursor to the next one it keeps — which is also
    /// what makes marking down an unwatched-only list walk forward one episode at a time.
    fn apply_episode_filter(&mut self) {
        let filter = self.episode_filter;
        let under_cursor = self.episodes.get(self.episode_selected).map(|r| r.number.clone());
        self.episodes = self.episodes_all.iter().filter(|r| filter.keeps(r)).cloned().collect();
        let kept = under_cursor.and_then(|number| {
            let from = self.episodes_all.iter().position(|r| r.number == number)?;
            let next = self.episodes_all[from..].iter().find(|r| filter.keeps(r))?;
            self.episodes.iter().position(|r| r.number == next.number)
        });
        self.episode_selected =
            kept.unwrap_or(self.episode_selected).min(self.episodes.len().saturating_sub(1));
    }

    /// Whether the filter is hiding everything — distinct from a title with no episodes.
    pub fn episodes_all_filtered_out(&self) -> bool {
        self.episodes.is_empty() && !self.episodes_all.is_empty()
    }

    /// What the status line should call the source: the provider that last actually served,
    /// before anything has played the first configured source that is ready, and failing that
    /// the first one that is not switched off.
    ///
    /// Not simply the first entry in the order: torrents lead the default order and are off by
    /// default, so that read "torrent" over a session that only plugins could serve.
    pub fn source_label(&self) -> String {
        if let Some(active) = &self.active_provider {
            return active.clone();
        }
        let sources = self.source_states();
        sources
            .iter()
            .find(|(_, state)| *state == SourceState::Ready)
            .or_else(|| sources.iter().find(|(_, state)| *state != SourceState::Off))
            .or_else(|| sources.first())
            .map_or_else(|| "no source".into(), |(id, _)| id.clone())
    }

    /// Each configured source, in the order failover walks them, and how it stands.
    ///
    /// The header's chips. Read from the registered providers rather than the config alone:
    /// a configured source can be ready, failing, held back by the VPN guard, still loading, or
    /// off on purpose — and only the last is something the config can say by itself.
    pub fn source_states(&self) -> Vec<(String, SourceState)> {
        let providers = &self.config.providers;
        let of_kind = |kind: &str| -> SourceState {
            let rows: Vec<&ProviderRow> =
                self.providers.iter().filter(|p| p.kind == kind).collect();
            if rows.iter().any(|p| p.usable) {
                SourceState::Ready
            } else if rows.iter().any(|p| p.held_back) {
                SourceState::HeldBack
            } else if rows.is_empty() {
                SourceState::Pending
            } else {
                SourceState::Failing
            }
        };
        providers
            .order
            .iter()
            .filter(|id| !providers.disabled.contains(id))
            .filter_map(|id| {
                let state = match id.as_str() {
                    "torrent" if !providers.torrent.enabled => SourceState::Off,
                    // Configured but not registered: the guard refused to start it, and the
                    // Providers screen carries the reason.
                    "torrent" => match of_kind("torrent") {
                        SourceState::Pending if self.provider_note.is_some() => {
                            SourceState::Failing
                        }
                        state => state,
                    },
                    "plugins" => of_kind("plugin"),
                    "remote" if providers.remote_url.is_none() => return None,
                    "remote" => of_kind("remote"),
                    _ => return None,
                };
                Some((id.clone(), state))
            })
            .collect()
    }

    /// Badge counts for the rail: work in flight, questions pending.
    pub fn rail_counts(&self) -> Vec<(Section, u32)> {
        let mut counts = Vec::new();
        let active = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, "downloading" | "queued"))
            .count() as u32;
        if active > 0 {
            counts.push((Section::Downloads, active));
        }
        if !self.conflicts.is_empty() {
            counts.push((Section::Accounts, self.conflicts.len() as u32));
        }
        counts
    }

    /// Flip one episode's watched state in both the visible and the full list.
    fn set_episode_watched(&mut self, number: &str, watched: bool) {
        for list in [&mut self.episodes, &mut self.episodes_all] {
            if let Some(row) = list.iter_mut().find(|r| r.number == number) {
                row.completed = watched;
                row.watched = if watched { 1.0 } else { 0.0 };
            }
        }
    }

    /// A `u` or `M` mark, reflected wherever the title's progress shows: its table, its page and
    /// the list rows for it.
    ///
    /// Only the table used to change, so after `u` on the next episode the page still read
    /// "2 / 26 ep 3 next", and so did Home, until the title was opened again.
    fn mark_episode(&mut self, id: AnilistId, number: &str, watched: bool) {
        if watched {
            // A mark is a finished watch with no position, and the reflection of one already
            // moves the count and Continue the way the store will.
            self.reflect_watch(Some(id), number, 0.0, None, true);
            return;
        }
        let was_done = self.episodes_all.iter().any(|r| r.number == number && r.completed);
        self.set_episode_watched(number, false);
        if !was_done || self.episodes_for != Some(id) {
            return;
        }
        // As the store re-derives Continue from what survives the unmark (`data::continue_from`):
        // the episode Continue had just moved past is the one to watch again, and past the
        // table's end it falls back to the first episode not watched.
        let unmarked = number.trim().parse::<u32>().ok();
        let numbered = || {
            self.episodes_all
                .iter()
                .filter_map(|r| Some((r.number.trim().parse::<u32>().ok()?, r)))
        };
        let last = numbered().map(|(n, _)| n).max();
        let gap = numbered().find(|(_, r)| !r.completed).map(|(n, _)| n);
        for entry in self.detail.iter_mut().chain(match &mut self.content {
            Content::Entries(entries) => entries.iter_mut(),
            _ => [].iter_mut(),
        }) {
            let Some((done, next)) = entry.progress.filter(|_| entry.id == id) else {
                continue;
            };
            let moved = match unmarked {
                Some(n) if n.saturating_add(1) == next => n,
                _ if last.is_some_and(|last| next > last) => gap.unwrap_or(next),
                _ => next,
            };
            if moved != next {
                entry.resume = None;
            }
            entry.progress = Some((done.saturating_sub(1), moved));
        }
    }

    fn reflect_watch(
        &mut self,
        id: Option<AnilistId>,
        episode: &str,
        position: f64,
        duration: Option<f64>,
        completed: bool,
    ) {
        // `completed` is authoritative, and the position is not: mpv's last reported `time-pos`
        // before end-of-file is routinely a second or two short of the runtime, and a meter stuck
        // at 98% on a finished episode is exactly the "did that register?" doubt this fixes.
        let fraction = if completed {
            1.0
        } else {
            duration.filter(|d| *d > 0.0).map(|d| (position / d).clamp(0.0, 1.0)).unwrap_or(0.0)
        };

        // The episode table is one title's; while detached the viewer can be looking at
        // another show entirely, whose rows must neither take this watch nor speak for it.
        let rows_apply = match id {
            Some(id) => self.episodes_for == Some(id),
            None => true,
        };
        // What the table says about this title, read before the rows are updated. Continue is
        // worked out as the store works it out (see `data::entry_from`): the last episode
        // touched when it is unfinished, and otherwise the first one after it not yet watched.
        let rows: &[EpisodeRow] = if rows_apply { &self.episodes_all } else { &[] };
        let row = |number: &str| rows.iter().find(|r| r.number == number);
        // A rewatch, which must not move the count: the store counts distinct episodes.
        let was_done = row(episode).is_some_and(|r| r.completed);
        // An episode the table knows and has never seen finished, which Continue moves to.
        let unfinished = row(episode).is_some_and(|r| !r.completed);
        let after = episode.trim().parse::<u32>().ok().map(|n| {
            let mut next = n.saturating_add(1);
            // Bounded: an all-watched tail must still terminate.
            while next - n < 1_000 && row(&next.to_string()).is_some_and(|r| r.completed) {
                next += 1;
            }
            // Past the table's last episode, the first one in it skipped.
            let numbers =
                || rows.iter().filter_map(|r| Some((r.number.trim().parse::<u32>().ok()?, r)));
            if numbers().map(|(n, _)| n).max().is_some_and(|last| next > last)
                && let Some((skipped, _)) = numbers().find(|(_, r)| !r.completed)
            {
                next = skipped;
            }
            next
        });

        if rows_apply {
            for list in [&mut self.episodes, &mut self.episodes_all] {
                if let Some(row) = list.iter_mut().find(|r| r.number == episode) {
                    // Monotonic: a re-watch that was quit early must not erase that it was once
                    // finished.
                    row.watched = row.watched.max(fraction);
                    row.completed = row.completed || completed;
                }
            }
        }

        for entry in self.detail.iter_mut().chain(match &mut self.content {
            Content::Entries(entries) => entries.iter_mut(),
            _ => [].iter_mut(),
        }) {
            // Only the entry being played: several shows on Home can share a next-episode
            // number, and a watch reflected onto all of them corrupts every other count.
            if id.is_some_and(|id| entry.id != id) {
                continue;
            }

            let next_up = entry.progress.map_or(1, |(_, next)| next).to_string();
            if !completed {
                // A part-watched episode moves the resume point, so the Continue rail is
                // current the moment mpv exits rather than at the next reload from the store —
                // and it is where Continue now carries on, unless it is one already watched.
                if position > 0.0
                    && (episode == next_up || unfinished)
                    && let Ok(number) = episode.trim().parse::<u32>()
                {
                    entry.progress = Some((entry.progress.map_or(0, |(done, _)| done), number));
                    entry.resume = Some(ResumePoint {
                        position,
                        fraction: duration
                            .filter(|d| *d > 0.0)
                            .map(|d| (position / d).clamp(0.0, 1.0)),
                    });
                }
                continue;
            }

            // Progress counts *completed* episodes, mirroring the store's distinct count: a
            // first watch moves it — including the very first episode of a show with no
            // history, and an out-of-order catch-up the table can vouch for — where a rewatch
            // leaves it alone. Continue moves past the episode just finished either way.
            let first =
                !was_done && (entry.progress.is_none() || rows_apply || episode == next_up);
            let done = entry.progress.map_or(0, |(done, _)| done) + u32::from(first);
            let next = after.or(entry.progress.map(|(_, next)| next)).unwrap_or(done + 1);
            entry.progress = Some((done, next));
            // Where the previous Continue episode was left is no longer what Continue offers.
            if next.to_string() != next_up {
                entry.resume = None;
            }
        }
    }

    /// Whether the reducer has just queued another playback — auto-next or an in-player
    /// episode step about to fire. The event loop uses this to know an ended session is a
    /// handoff rather than a return to the terminal.
    pub fn will_play_next(&self) -> bool {
        matches!(self.pending, Some(Task::Play { .. } | Task::PlaySource { .. }))
    }

    /// Age toasts by one frame.
    pub fn tick_toasts(&mut self) {
        for toast in &mut self.toasts {
            toast.ttl = toast.ttl.saturating_sub(1);
        }
        self.toasts.retain(|t| t.ttl > 0);
        self.tick_pending_reload();
        // The loading pulse rides this tick rather than getting one of its own. The idle ticker
        // already runs at 100 ms and already forces a repaint, so the animation is free — and
        // 10 fps is more than a three-cell wave needs. A dedicated 60 fps ticker would burn a
        // frame budget all the time to move three characters.
        self.pulse = self.pulse.wrapping_add(1);
    }

    /// How many rows of content fit, used for paging.
    pub fn page_size(&self, visible_rows: usize) -> usize {
        visible_rows.max(1)
    }

    fn move_selection(&mut self, delta: isize, visible_rows: usize) {
        let len = self.content.len();
        if len == 0 {
            return;
        }
        let next = (self.selected as isize + delta).clamp(0, len as isize - 1) as usize;
        self.selected = next;

        // Keep the selection inside the viewport.
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if visible_rows > 0 && self.selected >= self.offset + visible_rows {
            self.selected = self.selected.min(len - 1);
            self.offset = self.selected + 1 - visible_rows;
        }

        self.backfill_calendar_if_near_top();
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
        if len == 0 {
            self.offset = 0;
            return;
        }
        let rows = visible_rows.max(1);
        self.selected = self.selected.min(len - 1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + rows {
            self.offset = self.selected + 1 - rows;
        }
        // A taller window shows more of the list rather than empty rows under its end.
        self.offset = self.offset.min(len.saturating_sub(rows));
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
            // A status table with nothing to select.
            StageView::Section(Section::Providers) => None,
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
    fn move_cursor(&mut self, action: Action, visible_rows: usize) -> Option<Task> {
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
            if self.search_in_results && action == Action::Up && self.selected == 0 {
                self.search_in_results = false;
                return None;
            }
            match action {
                Action::Top => {
                    self.selected = 0;
                    self.offset = 0;
                }
                Action::Bottom => {
                    self.selected = self.content.len().saturating_sub(1);
                    self.offset = self.selected.saturating_sub(visible_rows.saturating_sub(1));
                }
                _ => self.move_selection(delta, visible_rows),
            }
            return None;
        }

        let len = match rows {
            Rows::Downloads => self.downloads.len(),
            Rows::Accounts => self.sync.len(),
            Rows::Settings => SettingId::ALL.len(),
            Rows::Episodes => self.episodes.len(),
            // The last row is the one that joins another store.
            Rows::Stores => self.stores.len() + 1,
            Rows::StoreLines => self
                .store_view()
                .and_then(|id| self.store(id))
                .map_or(0, |s| self.store_lines(s).len()),
            Rows::Members => self.store_members.len() + self.store_invites.len(),
            Rows::List => 0,
        };
        if len == 0 {
            return None;
        }
        let cursor = match rows {
            Rows::Downloads | Rows::Accounts | Rows::Settings => &mut self.selected,
            Rows::Episodes => &mut self.episode_selected,
            Rows::Stores => &mut self.store_selected,
            Rows::StoreLines => &mut self.store_line_selected,
            Rows::Members => &mut self.member_selected,
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

    /// Fetch older broadcasts once the selection nears the top of the calendar.
    ///
    /// Deliberately silent, and deliberately early. The question being asked by scrolling up is
    /// "what aired before this", and answering it with a spinner and a viewport that jumps would
    /// be a worse answer than the truncated week this replaces. So the request goes out while
    /// there is still a screenful above the selection left to scroll through, and the rows land
    /// above the viewport without moving it — see [`Self::apply`].
    fn backfill_calendar_if_near_top(&mut self) {
        if self.nav.section() != Section::Calendar
            || self.calendar_loading_past
            || !self.calendar_more_past
            || self.selected > CALENDAR_BACKFILL_MARGIN
            // The slot holds one task and auto-next has the stronger claim on it. Losing a
            // backfill costs nothing: the next scroll asks again.
            || self.pending.is_some()
        {
            return;
        }
        let Some(anchor) = self.calendar_anchor else { return };
        self.calendar_loading_past = true;
        self.pending = Some(Task::LoadCalendarPast { anchor, page: self.calendar_pages + 1 });
    }

    /// Handle a keystroke: text into a focused field first, then the binding it resolves to.
    ///
    /// The whole route from key to action in one place, so what a key does in any state can be
    /// tested as a key rather than as the action someone assumed it would become.
    pub fn press(&mut self, key: KeyEvent, visible_rows: usize) -> Option<Task> {
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
    pub fn handle(&mut self, action: Action, visible_rows: usize) -> Option<Task> {
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

    /// What an open, non-text overlay does with each action. Everything not listed is
    /// swallowed.
    ///
    /// Every overlay: Back closes it, Quit quits, and Help toggles the key reference on top —
    /// stacked, so Esc returns to the question it was opened from. Beyond that there are two
    /// kinds. **Help and Logs** are references with nothing to lose by leaving, so the keys
    /// that go somewhere — the palette, search, the section numbers, Logs — close them and go,
    /// save the section numbers over a playing Now Playing; their movement keys scroll. **Every other overlay asks a question**, and only its own
    /// keys work until it is answered or dismissed: a `5` in the rating picker must not jump to
    /// Downloads.
    fn handle_in_overlay(
        &mut self,
        overlay: &Overlay,
        action: Action,
        visible_rows: usize,
    ) -> Option<Task> {
        match action {
            Action::Quit => {
                self.should_quit = true;
                return None;
            }
            Action::Back => {
                self.nav.close_overlay();
                return None;
            }
            Action::Help if *overlay == Overlay::Help => {
                self.nav.close_overlay();
                return None;
            }
            Action::Help => {
                self.open_help();
                return None;
            }
            _ => {}
        }

        let reference = matches!(overlay, Overlay::Help | Overlay::Logs);
        if reference {
            match action {
                // The same key again closes it, as `?` does for Help.
                Action::ShowLogs if *overlay == Overlay::Logs => {
                    self.nav.close_overlay();
                    return None;
                }
                // Over Now Playing the digits belong to the player — `9` and `0` are its
                // volume — so behind Help or Logs a section number does nothing, rather than
                // closing the reference and leaving the player for another screen.
                Action::JumpSection(_)
                    if self.playing.is_some()
                        && matches!(self.nav.current(), StageView::NowPlaying) =>
                {
                    return None;
                }
                Action::CommandPalette
                | Action::FocusSearch
                | Action::FilterSearch
                | Action::JumpSection(_)
                | Action::ShowLogs
                | Action::ShowAccounts
                | Action::ShowConflicts => {
                    // Replaced rather than stacked: the palette runs what it picks against the
                    // screen, and anything left open would swallow that.
                    self.nav.close_all_overlays();
                    return self.perform(action, visible_rows);
                }
                _ => {}
            }
        }

        if *overlay == Overlay::Help {
            let last = self.help_pages.get().saturating_sub(1);
            self.help_page = match action {
                Action::Down | Action::PageDown => (self.help_page + 1).min(last),
                Action::Up | Action::PageUp => self.help_page.saturating_sub(1),
                Action::Top => 0,
                Action::Bottom => last,
                _ => self.help_page,
            };
            return None;
        }

        // The store confirmations and the invite link are questions with one answer key: a
        // stray `j` must not move the cursor off the member a "remove" is about to be applied to.
        match (overlay, action) {
            (Overlay::StoreConfirm, Action::Open) => return self.confirm_store_action(),
            (Overlay::InviteLink, Action::Open) => {
                self.nav.close_overlay();
                self.invite_link = None;
                return None;
            }
            (Overlay::StoreConfirm | Overlay::InviteLink, _) => return None,
            _ => {}
        }

        // A list overlay owns movement and Enter. Without this, arrowing through the Accounts
        // list would scroll the library behind it instead.
        let len = self.overlay_len();
        let page = visible_rows.saturating_sub(4).max(1);
        match action {
            Action::Down => {
                self.overlay_selected = (self.overlay_selected + 1).min(len.saturating_sub(1));
            }
            Action::Up => self.overlay_selected = self.overlay_selected.saturating_sub(1),
            Action::PageDown => {
                self.overlay_selected =
                    (self.overlay_selected + page).min(len.saturating_sub(1));
            }
            Action::PageUp => {
                self.overlay_selected = self.overlay_selected.saturating_sub(page);
            }
            Action::Top => self.overlay_selected = 0,
            Action::Bottom => self.overlay_selected = len.saturating_sub(1),
            Action::Open if len > 0 => return self.confirm_overlay(visible_rows),
            // The filters overlay edits in place: the vertical axis picks a row and the
            // horizontal one walks its values, the same grammar as the Settings screen.
            Action::Left if *overlay == Overlay::SearchFilters => {
                self.search_filter.cycle(self.overlay_selected, false);
            }
            Action::Right if *overlay == Overlay::SearchFilters => {
                self.search_filter.cycle(self.overlay_selected, true);
            }
            _ => {}
        }
        None
    }

    /// Open the key reference at its first page.
    fn open_help(&mut self) {
        self.help_page = 0;
        self.nav.open_overlay(Overlay::Help);
    }

    /// Run an action against the screen, with no overlay or text field in the way.
    ///
    /// Separate from [`Self::handle`] so the command palette can run what it picked: opened
    /// from Search's field, the field still has focus once the palette closes, and going back
    /// through the typing gate swallowed nearly every command.
    fn perform(&mut self, action: Action, visible_rows: usize) -> Option<Task> {
        // The download queue reads Space as pause and `x` as cancel, the keys it borrows from the
        // player. Resolved before the playback table: idle, those keys arrive as the browsing
        // actions — play next, remove from Continue — so a download could never be paused or
        // cancelled at all, and while something plays they would pause the player rather than
        // the row on screen.
        if self.in_downloads_stage() {
            match action {
                Action::PlayNext | Action::PlayPause => {
                    let id = self.downloads.get(self.selected)?.id;
                    return Some(Task::DownloadPause { id });
                }
                Action::HideFromContinue | Action::StopPlayback => {
                    let row = self.downloads.get(self.selected)?;
                    // A finished download has nothing left to cancel. Cancelling it forgot the
                    // row and left the file on disk with no way back to it from here.
                    if row.state == "done" {
                        let delete = self.key_for(Action::DeleteDownload);
                        self.push_toast(Toast::info(format!(
                            "already downloaded — {delete} deletes it and its file"
                        )));
                        return None;
                    }
                    return Some(Task::DownloadCancel { id: row.id });
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
            self.push_toast(Toast::info("nothing selected"));
            return None;
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::Help => self.open_help(),
            // Goes to the screen now rather than opening the modal. The overlay stays in the
            // codebase for the one case it is still right for — being asked to sign in from
            // somewhere else — but "show me my accounts" belongs on a page.
            Action::ShowAccounts => return self.go_to_section(Section::Accounts),
            Action::ShowLogs => {
                // Rendered newest-first, so nought is the most recent line.
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Logs);
            }
            Action::ShowConflicts => {
                if self.conflicts.is_empty() {
                    self.push_toast(Toast::info("nothing to resolve"));
                } else {
                    self.overlay_selected = 0;
                    self.nav.open_overlay(Overlay::Conflicts);
                }
            }
            Action::CommandPalette => {
                self.palette_query.clear();
                self.nav.open_overlay(Overlay::CommandPalette);
            }
            // Out of the search results is back into the query they answered, one step short
            // of the rail — the same order the two were entered in.
            Action::Back if self.search_in_results && self.on_search_root() => {
                self.search_in_results = false;
            }
            Action::Back => self.go_back(),
            Action::ToggleRail => self.nav.toggle_rail(),
            Action::JumpSection(n) => {
                if let Some(section) = Section::from_index(usize::from(n).saturating_sub(1)) {
                    return self.go_to_section(section);
                }
            }
            // Already on Search, `/` means "let me type" rather than "start again": it returns
            // to the field with the query and results as they were, without asking again.
            Action::FocusSearch if self.on_search_root() => {
                self.search_in_results = false;
                self.nav.focus_stage();
            }
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
            Action::Left => {
                self.search_in_results = false;
                self.nav.focus_rail();
            }
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
                let id = SettingId::ALL.get(self.selected).copied();
                if let Some(id) = id
                    && id.text_edit().is_some()
                {
                    self.edit_target = Some(id);
                    self.edit_value = self.current_text_value(id);
                    self.nav.open_overlay(Overlay::EditSetting);
                    return None;
                }
                return self.cycle_setting(1);
            }
            Action::Open if self.in_accounts_stage() => return self.toggle_account(),
            // Plugin stores live behind Providers: Enter there opens them, from the rail too,
            // because the provider table has nothing else to open.
            Action::Open if self.on_providers() => return self.open_stores(),
            Action::Open if self.nav.focus() == Focus::Stage && self.in_stores() => {
                return self.open_store_row();
            }
            Action::Open if self.nav.focus() == Focus::Stage && self.store_view().is_some() => {
                let id = self.store_view()?.to_owned();
                return self.open_store_line(&id);
            }
            Action::Open
                if self.nav.focus() == Focus::Stage && self.members_view().is_some() =>
            {
                let id = self.members_view()?.to_owned();
                return self.open_member_row(&id);
            }
            // Enter plays a finished download from disk. Nothing else on this screen is an "open".
            Action::Open if self.in_downloads_stage() => {
                let row = self.downloads.get(self.selected)?.clone();
                match (&row.path, row.state) {
                    (Some(path), "complete") => {
                        // First-class playback: same staging, history and sync as a stream.
                        self.raise_now_playing_titled(
                            row.anilist_id,
                            row.title.clone(),
                            &row.episode,
                        );
                        return Some(Task::PlayLocal {
                            id: row.anilist_id,
                            episode: row.episode,
                            title: row.title,
                            path: path.clone(),
                        });
                    }
                    (_, "failed") => {
                        // The reason is on the row; surfacing it is more use than a dead keypress.
                        let reason =
                            row.error.clone().unwrap_or_else(|| "no reason recorded".into());
                        self.push_toast(Toast::alert(reason));
                    }
                    _ => self.push_toast(Toast::info("not finished yet")),
                }
            }
            Action::DeleteDownload if self.in_downloads_stage() => {
                let row = self.downloads.get(self.selected)?;
                return Some(Task::DownloadDelete { id: row.id });
            }
            Action::ClearCompleted if self.in_downloads_stage() => {
                return Some(Task::DownloadClearCompleted);
            }
            // From the rail, Enter means "into this section" rather than "open the title the
            // list happens to be sitting on" — which is what it used to do, opening something
            // the user never selected.
            Action::Open if self.nav.focus() == Focus::Rail => self.nav.focus_stage(),
            // On Continue, Enter does what the section is named for: a part-watched
            // episode resumes immediately, no detail screen in between. The title's other
            // views stay one key away — `e` episodes, `s` sources, `w` watch order — and
            // rows with nothing to resume still open normally.
            Action::Open if self.resumes_on_enter() => {
                let entry = self.selected_entry().expect("checked");
                let id = entry.id;
                let next = entry.progress.expect("checked").1;
                return Some(self.begin_playback(id, next.to_string()));
            }
            Action::Open => return self.open_selected(),
            // Space skips the detail screen entirely: from a list, straight into the next
            // episode you have not watched.
            Action::PlayNext => return self.play_next_unwatched(),
            Action::ShowEpisodes => {
                // On a title screen it is that title — the list underneath may be sitting on
                // something else entirely, as it is after stepping through related titles. On a
                // list it is the row under the cursor. See `target`.
                if let Some(entry) = self.target().cloned() {
                    let id = entry.id;
                    // The episode table is a view *of a title*, and `detail` is how the rest of
                    // the app asks which title that is. Reached by `e` from a list it was left
                    // unset, so everything downstream answered "none": a finished episode never
                    // reached the table it was listed in, and auto-next declined to roll on.
                    // Opening the title sets this; entering its episodes has to as well.
                    self.detail = Some(entry);
                    self.episode_selected = 0;
                    // Discard the previous title's rows *before* the screen appears. They were
                    // being left in place until the new load answered, so opening episodes for one
                    // show displayed another show's episode list — complete with its watch
                    // progress — and then silently corrected itself. Stale data presented as
                    // current is worse than no data: the loading state is honest, and it is the
                    // reason the pulse and skeleton rows exist.
                    self.episodes.clear();
                    self.episodes_loading = true;
                    self.nav.push(StageView::Episodes(id));
                    return Some(Task::LoadEpisodes(id));
                }
            }
            Action::ToggleWatched if self.in_episodes() => {
                let id = match self.nav.current() {
                    StageView::Episodes(id) => *id,
                    _ => return None,
                };
                let (number, watched) = {
                    let row = self.episodes.get(self.episode_selected)?;
                    (row.number.clone(), !row.completed)
                };
                self.mark_episode(id, &number, watched);
                // Under the unwatched filter a marked row leaves the view at once —
                // which is the binge-marking flow working, not a glitch.
                self.apply_episode_filter();
                return Some(Task::SetWatched { id, episodes: vec![number], watched });
            }
            Action::MarkAllPrevious if self.in_episodes() => {
                let id = match self.nav.current() {
                    StageView::Episodes(id) => *id,
                    _ => return None,
                };
                // Everything strictly before the selected row that is not already done —
                // the "I'm caught up to here" gesture.
                let marked: Vec<String> = self
                    .episodes
                    .iter()
                    .take(self.episode_selected)
                    .filter(|r| !r.completed)
                    .map(|r| r.number.clone())
                    .collect();
                if marked.is_empty() {
                    // Three different nothings, each said as what it is: "nothing before this
                    // episode" over an empty table pointed at an episode that was not there.
                    let why = match self.episodes.get(self.episode_selected) {
                        None => "no episodes listed to mark".to_owned(),
                        Some(_) if self.episode_selected == 0 => {
                            "nothing comes before the first episode".to_owned()
                        }
                        Some(row) => {
                            format!("everything before ep {} is already watched", row.number)
                        }
                    };
                    self.push_toast(Toast::info(why));
                    return None;
                }
                for number in &marked {
                    self.mark_episode(id, number, true);
                }
                self.apply_episode_filter();
                self.push_toast(Toast::info(format!(
                    "marked {} watched",
                    crate::widgets::plural(marked.len(), "episode")
                )));
                return Some(Task::SetWatched { id, episodes: marked, watched: true });
            }
            Action::DownloadRange if self.in_episodes() => {
                self.range_query.clear();
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::DownloadRange);
            }
            Action::DownloadRange => {
                self.push_toast(Toast::info("ranges are queued from the episode table"));
            }
            Action::Filter if self.in_episodes() => {
                self.episode_filter = self.episode_filter.next();
                self.apply_episode_filter();
                self.push_toast(Toast::info(format!(
                    "showing {}",
                    self.episode_filter.label()
                )));
            }
            // By the view, not the section: a title opened from the results is a title page,
            // and `f` there means what it means on every other title page.
            Action::Filter if self.on_search_root() => {
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::SearchFilters);
            }
            Action::FilterSearch => {
                // From anywhere: land on Search first, because the overlay's Enter runs a
                // search and the results have to have somewhere to go. The jump closes any
                // overlays, so this one is opened after.
                let task = (self.nav.section() != Section::Search)
                    .then(|| self.go_to_section(Section::Search))
                    .flatten();
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::SearchFilters);
                return task;
            }
            Action::Filter => {
                self.push_toast(Toast::info("filters live in search and the episode table"));
            }
            Action::ToggleWatched | Action::MarkAllPrevious => {
                self.push_toast(Toast::info("watched marks live in the episode table"));
            }
            Action::OpenInBrowser => {
                let Some(id) = self.target().map(|e| e.id) else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                return Some(Task::OpenExternal {
                    url: format!("https://anilist.co/anime/{}", id.get()),
                });
            }
            Action::PickProvider => {
                // The choice already in force arrives with the title's record, so only a title's
                // own page knows it. From a list the overlay would open on the pin of whichever
                // title was opened last, and confirming "automatic" there could quietly undo a
                // pin the user made.
                if self.title_on_screen().is_none() {
                    let hint = if self.target().is_some() {
                        "a source is pinned from the title's page"
                    } else {
                        "nothing selected"
                    };
                    self.push_toast(Toast::info(hint));
                    return None;
                }
                if self.providers.is_empty() {
                    // Plugins register a moment after launch, so "none yet" is a real state
                    // rather than a misconfiguration.
                    self.push_toast(Toast::info("no sources registered yet"));
                    return None;
                }
                // Start on the current choice, so the overlay opens showing where you are.
                self.overlay_selected = self
                    .provider_choices()
                    .iter()
                    .position(|(id, _)| id.as_deref() == self.provider_preference.as_deref())
                    .unwrap_or(0);
                self.nav.open_overlay(Overlay::SourceProvider);
            }
            Action::PlayTrailer => {
                let Some(entry) = self.target() else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                let Some(url) = entry.trailer_url.clone() else {
                    // Rides the detail fetch, so a list row answers after the title has
                    // been opened once — and some titles genuinely have no trailer.
                    let why = self
                        .off_the_page("the trailer is found")
                        .unwrap_or_else(|| "no trailer on record for this title".into());
                    self.push_toast(Toast::info(why));
                    return None;
                };
                return Some(Task::PlayTrailer { url, title: entry.title.clone() });
            }
            Action::Recommendations => {
                let Some(entry) = self.target() else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                if entry.recommended.is_empty() {
                    // Like relations, recommendations ride the detail fetch — and some
                    // titles genuinely have none.
                    let why = self
                        .off_the_page("recommendations are listed")
                        .unwrap_or_else(|| "no recommendations known for this title".into());
                    self.push_toast(Toast::info(why));
                    return None;
                }
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Recommendations);
            }
            Action::WatchOrder => {
                let Some(entry) = self.target() else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                if entry.related.is_empty() {
                    // Empty on list rows too — the relations ride the detail fetch. From a
                    // list this still answers after opening the title once.
                    let why = self
                        .off_the_page("connected seasons are listed")
                        .unwrap_or_else(|| "no connected seasons known for this title".into());
                    self.push_toast(Toast::info(why));
                    return None;
                }
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::WatchOrder);
            }
            Action::FixMapping => {
                // "This matched the wrong thing" — ask the user what to search for, then
                // re-enter the ordinary disambiguation flow with their words.
                //
                // From a title's page or its episodes only: the answer is a new episode table,
                // which is taken for the title on screen and nowhere else, so from a list the
                // pick would be asked for and then silently dropped — or, with a title opened
                // earlier, applied to that title instead of the highlighted one.
                let Some(id) = self.title_on_screen() else {
                    let hint = if self.target().is_some() {
                        "a match is fixed from the title's page"
                    } else {
                        "nothing selected to re-match"
                    };
                    self.push_toast(Toast::info(hint));
                    return None;
                };
                self.manual_target = Some(id);
                self.manual_query.clear();
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::ManualQuery);
            }
            Action::ShowSources => {
                // Same shape as Download: the highlighted episode from the table, the next
                // unwatched one from a title.
                let (id, episode) = match self.nav.current() {
                    StageView::Episodes(id) => {
                        (*id, self.episodes.get(self.episode_selected)?.number.clone())
                    }
                    _ => {
                        let entry = self.target()?;
                        let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                        (entry.id, episode)
                    }
                };
                self.source_context = Some((id, episode.clone()));
                return Some(Task::LoadSources { id, episode });
            }
            Action::PlayParty => {
                // The same episode Enter would play, but with the room. No Now Playing
                // view: Syncplay owns the player, and a dead session screen would lie.
                let (id, episode) = match self.nav.current() {
                    StageView::Episodes(id) => {
                        (*id, self.episodes.get(self.episode_selected)?.number.clone())
                    }
                    _ => {
                        let entry = self.target()?;
                        let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                        (entry.id, episode)
                    }
                };
                return Some(Task::PlayParty { id, episode });
            }
            Action::Download => {
                // From the episode table it is the highlighted episode; from a title it is the next
                // one you have not watched, which matches what Enter would have played.
                let (id, episode) = match self.nav.current() {
                    StageView::Episodes(id) => {
                        (*id, self.episodes.get(self.episode_selected)?.number.clone())
                    }
                    _ => {
                        let entry = self.target()?;
                        let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                        (entry.id, episode)
                    }
                };
                return Some(Task::DownloadEpisode { id, episode });
            }
            Action::ToggleSynopsis => self.synopsis_expanded = !self.synopsis_expanded,
            Action::SetListStatus => {
                if self.signed_in_trackers().is_empty() {
                    // Nothing keeps a list status but a tracker. Offering the picker anyway
                    // ended in "marked watching" for a choice that went nowhere.
                    self.push_toast(Toast::info(
                        "list status is kept on a tracker, and none is signed in — :accounts",
                    ));
                } else if let Some(entry) = self.target() {
                    // Preselect what the title is already on, so the common case is one
                    // keystroke to confirm rather than a hunt. The title's own status, where it
                    // is known — the Library tab last viewed said nothing about a show opened
                    // from search, and preselected "dropped" for it. Unknown starts at the top.
                    let current =
                        self.statuses_set.get(&entry.id).copied().or(entry.list_status);
                    self.overlay_selected = current
                        .and_then(|status| {
                            LibrarySegment::ALL.iter().position(|s| *s == status)
                        })
                        .unwrap_or(0);
                    self.nav.open_overlay(Overlay::ListStatus);
                } else {
                    self.push_toast(Toast::info("nothing selected"));
                }
            }
            Action::RateTitle => {
                if self.signed_in_trackers().is_empty() {
                    self.push_toast(Toast::info(
                        "ratings are kept on a tracker, and none is signed in — :accounts",
                    ));
                } else if self.target().is_some() {
                    // No local record of an existing score to preselect, so the cursor
                    // starts at the top and every row is one or two keystrokes away.
                    self.overlay_selected = 0;
                    self.nav.open_overlay(Overlay::Rate);
                } else {
                    self.push_toast(Toast::info("nothing selected"));
                }
            }
            Action::HideFromContinue => {
                // The Home list only: the CONTINUE rail is the one list built from your
                // history, so it is the one list a title can be dismissed from. The view
                // check matters as much as the section — a pushed detail or episode table
                // keeps the Home entries as `content`, and removing the show you are
                // *looking into* because a list you cannot see holds it would be absurd.
                // Elsewhere the key says so rather than doing nothing, which would read
                // as broken.
                let at_home_list = self.nav.section() == Section::Home
                    && matches!(self.nav.current(), StageView::Section(Section::Home));
                if !at_home_list {
                    self.push_toast(Toast::info("removing works on the Continue list"));
                    return None;
                }
                let Content::Entries(entries) = &mut self.content else {
                    return None;
                };
                if entries.is_empty() {
                    return None;
                }
                // Dropped from the screen before the write, same contract as SetWatched:
                // the list answers immediately, the store catches up.
                let entry = entries.remove(self.selected.min(entries.len() - 1));
                self.selected = self.selected.min(self.content.len().saturating_sub(1));
                self.offset = self.offset.min(self.selected);
                // Say how to undo, because there is no key for it — the way back is the
                // same thing that put it here.
                self.push_toast(Toast::info(format!(
                    "{} removed — watching it again brings it back",
                    entry.title
                )));
                return Some(Task::HideFromContinue { id: entry.id });
            }
            Action::ForceResync => {
                if self.sync.is_empty() {
                    let accounts = self.section_key(Section::Accounts);
                    self.push_toast(Toast::info(format!(
                        "no tracker connected — press {accounts} for Accounts"
                    )));
                } else {
                    return Some(Task::SyncNow);
                }
            }
            // On the store screens, refreshing means asking the stores again, not re-checking
            // the provider table they were opened from.
            Action::Refresh if self.in_stores() || self.store_view().is_some() => {
                return Some(Task::LoadStores);
            }
            Action::Refresh if self.members_view().is_some() => {
                let store = self.members_view()?.to_owned();
                return Some(Task::LoadMembers { store });
            }
            // A title page and an episode table refresh themselves, not the section's list
            // under them — which is what `r` used to fetch, saying "loading the calendar" over
            // a title page that never changed.
            Action::Refresh if matches!(self.nav.current(), StageView::Title(_)) => {
                self.images.requests().retry_failed();
                return self.title_on_screen().map(Task::LoadDetail);
            }
            Action::Refresh if self.in_episodes() => {
                // The rows stay on screen until the answer lands, and the answer keeps the
                // cursor on its episode; the activity row says the table is being asked again.
                self.images.requests().retry_failed();
                return self.title_on_screen().map(Task::LoadEpisodes);
            }
            Action::Refresh => {
                // Asking again means the covers too: one that failed while the network was
                // down should not wait out its backoff once you have said it is back.
                self.images.requests().retry_failed();
                return self.reload();
            }
            Action::ToggleTranslation => {
                // For this session: the saved choice is the Settings row's to change.
                self.config.playback.translation = self.config.playback.translation.toggled();
                self.push_toast(Toast::info(format!(
                    "now using {} for this session",
                    self.config.playback.translation
                )));
            }
            _ => {
                // An action that means nothing here — a playback key with nothing playing, a
                // Downloads key elsewhere, usually run from the palette — says where it does
                // something rather than doing nothing, which would read as the key being broken.
                let label = action.label();
                let mut chars = label.chars();
                let label = match chars.next() {
                    Some(first) => first.to_lowercase().chain(chars).collect::<String>(),
                    None => String::new(),
                };
                self.push_toast(Toast::info(format!(
                    "{label} works {}",
                    action.where_it_works()
                )));
            }
        }
        None
    }

    /// Dispatch a playback binding.
    ///
    /// Returns `None` when the action is not a playback one, so the caller falls through to
    /// normal navigation. `Some(None)` means it was handled with no async work.
    #[allow(clippy::option_option)]
    fn handle_playback(&mut self, action: Action) -> Option<Option<Task>> {
        use PlayerCommand as P;
        let command = match action {
            Action::PlayPause => P::TogglePause,
            Action::SeekBack => P::Seek(-5.0),
            Action::SeekForward => P::Seek(5.0),
            Action::SeekBackFar => P::Seek(-30.0),
            Action::SeekForwardFar => P::Seek(30.0),
            Action::SpeedDown => P::Speed(-0.25),
            Action::SpeedUp => P::Speed(0.25),
            Action::VolumeDown => P::Volume(-5.0),
            Action::VolumeUp => P::Volume(5.0),
            Action::Fullscreen => P::Fullscreen,
            Action::Detach => {
                // mpv keeps playing; we just stop looking at it. Progress recording continues,
                // because the session outlives the screen.
                self.push_toast(Toast::info("detached — mpv is still playing"));
                self.nav.pop();
                return Some(None);
            }
            Action::StopPlayback => P::Stop,
            Action::SkipOpening => {
                let Some(playing) = &self.playing else { return Some(None) };
                match playing.skip {
                    Some((_, to)) => P::SeekTo(to),
                    // Better to say nothing is offered than to seek somewhere arbitrary.
                    None => {
                        self.push_toast(Toast::info("no skip here"));
                        return Some(None);
                    }
                }
            }
            Action::NextEpisode => return Some(self.step_episode(1)),
            Action::PreviousEpisode => return Some(self.step_episode(-1)),
            _ => return None,
        };
        Some(Some(Task::Player(command)))
    }

    /// Start the episode selected in the timing sheet.
    fn play_selected_episode(&mut self) -> Option<Task> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        let episode = self.episodes.get(self.episode_selected)?.number.clone();
        Some(self.begin_playback(id, episode))
    }

    /// Play the first episode local history has not completed.
    ///
    /// Driven off the watch log rather than a tracker, so this works with no account at all.
    fn play_next_unwatched(&mut self) -> Option<Task> {
        let Some(entry) = self.target() else {
            self.push_toast(Toast::info("nothing selected"));
            return None;
        };
        let id = entry.id;
        let episode =
            entry.progress.map(|(_, next)| next.to_string()).unwrap_or_else(|| "1".to_string());
        Some(self.begin_playback(id, episode))
    }

    /// Step to an adjacent episode of what is playing, from whichever view is up.
    ///
    /// Asked of the playback rather than worked out here. The open title page can be another
    /// show entirely, or no show at all after starting from Continue, and answering from it
    /// played that show's episode, or nothing. The playback holds the episode list of the title
    /// it plays, so it is also the side that can say there is no next episode instead of
    /// handing the finale's window to a search for one.
    fn step_episode(&mut self, delta: i64) -> Option<Task> {
        self.playing.as_ref()?;
        Some(Task::Player(PlayerCommand::Step(delta)))
    }

    /// The episode table for a title, when the rows loaded are that title's — what the player's
    /// Episodes menu lists. Empty for any other title rather than another show's rows.
    pub fn episode_rows_for(&self, id: AnilistId) -> &[EpisodeRow] {
        // Every row, not the filtered view: a filter on the title screen is about what to look
        // at there, not about which episodes exist. Asked by the rows' own title rather than
        // `detail`, which moves to a newly opened title before that title's rows arrive.
        if self.episodes_for == Some(id) { &self.episodes_all } else { &[] }
    }

    /// The episode that follows `episode` of `id`, honouring the filler setting, when the
    /// episode table can vouch that it exists.
    ///
    /// `None` when the table is not this title's or has no such row: the player then shows no
    /// Up Next card and closes at the end, which is right for a finale and harmless otherwise.
    pub fn next_up(&self, id: AnilistId, episode: &str) -> Option<&EpisodeRow> {
        let rows = self.episode_rows_for(id);
        let next = episode.trim().parse::<u32>().ok()?.checked_add(1)?.to_string();
        let next = self.next_after_filler(id, next);
        rows.iter().find(|r| r.number == next)
    }

    /// Where auto-next actually lands, honouring `playback.skip_filler`.
    ///
    /// Steps over episodes marked pure filler (`mixed` is never skipped — it carries
    /// story). Only rows present in the title's episode table can be judged; an unknown
    /// episode is played rather than guessed about, and another show's table never answers.
    fn next_after_filler(&self, id: AnilistId, mut next: String) -> String {
        if !self.config.playback.skip_filler {
            return next;
        }
        let rows = self.episode_rows_for(id);
        let mut steps = 0;
        while let Some(row) = rows.iter().find(|r| r.number == next) {
            if !row.skippable {
                break;
            }
            let Ok(n) = next.trim().parse::<i64>() else { break };
            next = (n + 1).to_string();
            // An all-filler tail must still terminate.
            steps += 1;
            if steps > 100 {
                break;
            }
        }
        next
    }

    /// Push Now Playing, raise the eyecatch, and ask for the stream.
    ///
    /// The order matters: the wipe goes up *before* resolution starts, so the user never sees
    /// a frozen episode table while a provider is being tried.
    fn begin_playback(&mut self, id: AnilistId, episode: String) -> Task {
        self.raise_now_playing(id, &episode);
        Task::Play { id, episode }
    }

    /// The visual half of starting playback, shared with the Sources pick: eyecatch up,
    /// Now Playing staged, view pushed.
    fn raise_now_playing(&mut self, id: AnilistId, episode: &str) {
        // Named by the title being played, never by whichever was opened last: Space on a list
        // row put the previously opened show's name over the wipe.
        let title = [self.detail.as_ref(), self.selected_entry()]
            .into_iter()
            .flatten()
            .find(|e| e.id == id)
            .map_or_else(|| "playing".to_string(), |e| e.title.clone());
        self.raise_now_playing_titled(id, title, episode);
    }

    /// The same staging with the title stated by the caller — the Downloads screen knows
    /// what its rows are called without any list selection being involved.
    fn raise_now_playing_titled(&mut self, id: AnilistId, title: String, episode: &str) {
        self.eyecatch = Some(Eyecatch::new(format!("{title}  ·  ep {episode}")));
        self.playing = Some(NowPlaying {
            id: Some(id),
            title,
            episode: episode.to_owned(),
            // From this title's table only: the open one can be another show's.
            episode_title: self
                .episode_rows_for(id)
                .iter()
                .find(|e| e.number == episode)
                .and_then(|e| e.title.clone()),
            speed: self.config.playback.persisted_speed.unwrap_or(1.0),
            ..NowPlaying::default()
        });
        self.nav.push(StageView::NowPlaying);
    }

    /// Name the episode just staged, when the caller knows it better than the table here — the
    /// playback holds its own title's list, whichever title is open in the terminal.
    fn name_playing_episode(&mut self, title: Option<String>) {
        if let Some(playing) = &mut self.playing
            && title.is_some()
        {
            playing.episode_title = title;
        }
    }

    /// Advance the eyecatch one frame, dropping it when the wipe finishes.
    ///
    /// A wipe that reaches its deadline reveals without a word. Its clock starts at the keypress,
    /// so most of what it covers is the search for a source, which the activity row is already
    /// narrating — alerting on it said the player never started before mpv had been asked to.
    /// A player that does start and then shows nothing is the playback's to say, timed from
    /// when mpv was handed the file.
    pub fn tick_animation(&mut self) {
        if let Some(eyecatch) = &mut self.eyecatch
            && !eyecatch.advance()
        {
            self.eyecatch = None;
        }
    }

    /// Whether the loop should tick at animation rate rather than idle rate.
    pub fn is_animating(&self) -> bool {
        self.eyecatch.is_some()
    }

    /// Take any work the reducer queued while applying an update.
    pub fn take_pending(&mut self) -> Option<Task> {
        self.pending.take()
    }

    /// Whether the Library list has focus, so horizontal keys mean "step segment".
    /// Whether the Settings rows have focus.
    ///
    /// Left and Right cycle a value here rather than moving between rail and stage — the same
    /// trade Library already makes for its status segments. Esc, Tab and the number keys still
    /// leave, so nothing is trapped.
    /// Whether the download queue has focus.
    fn in_downloads_stage(&self) -> bool {
        self.nav.section() == Section::Downloads
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Downloads))
    }

    /// Whether the Accounts rows have focus.
    fn in_accounts_stage(&self) -> bool {
        self.nav.section() == Section::Accounts
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Accounts))
    }

    /// Sign the selected tracker in or out.
    ///
    /// The same decision the Accounts overlay made, kept in one place so the screen and the overlay
    /// cannot disagree about what Enter does.
    fn toggle_account(&mut self) -> Option<Task> {
        let tracker = self.sync.get(self.selected)?.clone();
        self.account_action(tracker)
    }

    /// What Enter means for one tracker row: out if in, in if out — and for Kitsu, "in"
    /// is a prompt right here rather than a browser or a trip to the command line.
    fn account_action(&mut self, tracker: SyncState) -> Option<Task> {
        if tracker.connected && !tracker.needs_reauth {
            return Some(Task::Disconnect { tracker: tracker.tracker });
        }
        if tracker.tracker == "kitsu" {
            // Cleared on open, not just on submit, so a dismissed attempt never leaves
            // half a password waiting in the next prompt.
            self.kitsu_user.clear();
            self.kitsu_pass.clear();
            self.nav.open_overlay(Overlay::KitsuUsername);
            return None;
        }
        self.status = "opening your browser to sign in…".into();
        Some(Task::Connect { tracker: tracker.tracker })
    }

    /// The trackers a status or score is queued for: those holding a credential, the same
    /// rule the queueing side applies. A tracker waiting on a fresh sign-in still counts —
    /// its queue is kept and drains once it is signed in again.
    fn signed_in_trackers(&self) -> Vec<&str> {
        self.sync.iter().filter(|s| s.connected).map(|s| s.tracker.as_str()).collect()
    }

    fn in_settings_stage(&self) -> bool {
        self.nav.section() == Section::Settings
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Settings))
    }

    fn in_library_stage(&self) -> bool {
        self.nav.section() == Section::Library
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Library))
    }

    /// Step the Library's segment, and from the first one carry Left on to the rail — the
    /// screen's left edge, where Left goes everywhere else.
    fn step_segment(&mut self, delta: isize) -> Option<Task> {
        let next = self.library_segment.step(delta);
        if next == self.library_segment {
            if delta < 0 {
                self.nav.focus_rail();
            }
            return None;
        }
        self.library_segment = next;
        self.selected = 0;
        self.offset = 0;
        self.content = Content::Loading;
        Some(Task::LoadLibrary(self.library_segment))
    }

    /// Whether the Search section's own view — the field and its results — is on top.
    fn on_search_root(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Search))
    }

    /// "… from the title's page", when the screen is not one — said for what rides the detail
    /// fetch and came back empty here, as `m` and `P` say it for what only that page can do.
    ///
    /// A list row never opened carries no relations, recommendations or trailer, so "none known
    /// for this title" off the page was a guess that read as a fact.
    fn off_the_page(&self, what: &str) -> Option<String> {
        self.title_on_screen().is_none().then(|| format!("{what} from the title's page"))
    }

    /// The title whose page or episode table is on top, by the view's own id.
    fn title_on_screen(&self) -> Option<AnilistId> {
        match self.nav.current() {
            StageView::Title(id) | StageView::Episodes(id) => Some(*id),
            _ => None,
        }
    }

    /// Back: close an overlay, pop a pushed view, or at a section's root hand focus to the rail.
    fn go_back(&mut self) {
        match self.nav.back() {
            crate::nav::BackOutcome::AtRoot => {
                // At the very root, back is a no-op rather than a quit. Said as a toast:
                // written to the status line it stayed there, over the state it shows, for
                // the rest of the session.
                let quit = self.key_for(Action::Quit);
                self.push_toast(Toast::info(format!("at the top — {quit} quits")));
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

    /// Remember the title page on screen before a relation or recommendation replaces it.
    fn leave_title(&mut self) {
        let Some(entry) = self.title_on_screen().and_then(|_| self.detail.clone()) else {
            return;
        };
        let depth = self.nav.depth();
        // Anything at this depth or deeper describes views that have since been popped.
        self.title_trail.retain(|frame| frame.depth < depth);
        self.title_trail.push(TitleFrame {
            depth,
            entry,
            episodes_all: self.episodes_all.clone(),
            episode_selected: self.episode_selected,
            provider_preference: self.provider_preference.clone(),
        });
    }

    /// After a pop, show the title page a relation was opened from as it was, when that page
    /// is where Back landed.
    fn restore_title(&mut self) {
        let depth = self.nav.depth();
        self.title_trail.retain(|frame| frame.depth <= depth);
        let Some(here) = self.title_on_screen() else { return };
        if !self.title_trail.last().is_some_and(|frame| frame.depth == depth) {
            return;
        }
        let Some(frame) = self.title_trail.pop().filter(|frame| frame.entry.id == here) else {
            return;
        };
        self.detail = Some(frame.entry);
        self.episodes_all = frame.episodes_all;
        // Cleared first so the filter places the cursor from the remembered row, not from
        // whichever row of the child's table it was on.
        self.episodes.clear();
        self.episode_selected = frame.episode_selected;
        self.apply_episode_filter();
        self.episodes_loading = false;
        self.provider_preference = frame.provider_preference;
    }

    /// Left and Right on a Settings row: step its value, stopping at the ends, and from a value
    /// with nowhere further left to go, carry Left on to the rail.
    ///
    /// The same edge rule as everywhere else, which is what makes the rail reachable from here
    /// by the key that reaches it from every other screen. A two-state value steps like a
    /// two-rung ladder — Left is off, or the first of its two choices, and Right the other —
    /// because flipping on both arrows meant Left could never arrive anywhere. Enter and Space
    /// still flip it. A row the arrows cannot step at all, typed or read-only, is at both ends.
    fn step_setting(&mut self, delta: isize) -> Option<Task> {
        let id = *SettingId::ALL.get(self.selected)?;
        let steppable = id.text_edit().is_none()
            && self.setting_rows().get(self.selected).is_some_and(|row| row.editable.is_some());
        if !steppable || self.two_state(id).is_some_and(|second| second == (delta > 0)) {
            if delta < 0 {
                self.nav.focus_rail();
                return None;
            }
            // Right on a typed or read-only row still says how that row is changed.
            return if steppable { None } else { self.cycle_setting(delta) };
        }
        let before = self.ladder_value(id);
        let task = self.cycle_setting(delta);
        // A ladder already on its last rung: nothing to write, and Left goes on to the rail.
        if before.is_some()
            && matches!(&task, Some(Task::SaveSetting { value, .. }) if Some(value) == before.as_ref())
        {
            if delta < 0 {
                self.nav.focus_rail();
            }
            return None;
        }
        task
    }

    /// Where a two-state Settings row stands: `Some(true)` on its second state — on, or the
    /// second of its two choices. `None` for every other row.
    fn two_state(&self, id: SettingId) -> Option<bool> {
        use SettingId as S;
        use anistream_core::config::{PlayerUi, ThemeMode};
        let c = &self.config;
        // Theme and sub/dub can differ for this session (`--theme`, `t`), and their arrows
        // change the saved choice, so that is the one they stand on.
        let saved = &self.saved_config;
        Some(match id {
            S::Theme => saved.theme.mode == ThemeMode::Immersive,
            S::Translation => {
                saved.playback.translation == anistream_core::media::Translation::Dub
            }
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

    /// A laddered Settings row's value as it would be written, to tell a step from a stay.
    fn ladder_value(&self, id: SettingId) -> Option<anistream_core::settings::SettingValue> {
        use SettingId as S;
        use anistream_core::settings::SettingValue as V;
        let playback = &self.config.playback;
        let seeding = &self.config.providers.torrent.seeding;
        Some(match id {
            S::Quality => V::Int(i64::from(playback.quality)),
            S::CommitThreshold => V::Float((playback.commit_threshold * 100.0).round() / 100.0),
            S::Upscaling => V::Str(playback.upscaling.name().into()),
            S::SeedRatio => V::Float(seeding.ratio),
            S::SeedHours => V::Int(i64::from(seeding.hours)),
            S::UploadLimit => V::Int(i64::from(seeding.upload_limit_kib)),
            _ => return None,
        })
    }

    /// Confirm the selection in whichever overlay is open.
    ///
    /// Overlays own their own selection index so closing one cannot disturb the list behind it.
    fn confirm_overlay(&mut self, visible_rows: usize) -> Option<Task> {
        match self.nav.overlay() {
            Some(Overlay::CommandPalette) => {
                let (action, _) = self.palette_matches().get(self.overlay_selected)?.clone();
                // Close first, then run: several palette actions open an overlay of their own,
                // and leaving the palette on the stack would bury them.
                self.nav.close_overlay();
                self.palette_query.clear();
                self.overlay_selected = 0;
                self.perform(action, visible_rows)
            }
            Some(Overlay::Logs) => None,
            Some(Overlay::ListStatus) => {
                let entry = self.target()?;
                let id = entry.id;
                let status = *LibrarySegment::ALL.get(self.overlay_selected)?;
                self.nav.close_overlay();
                self.push_toast(Toast::info(format!(
                    "now on your {} list — queued for {}",
                    status.label(),
                    self.signed_in_trackers().join(", ")
                )));
                // Remembered on every copy of the title on screen, and past the next reload,
                // so asking again opens on the answer just given.
                self.statuses_set.insert(id, status);
                let entries = match &mut self.content {
                    Content::Entries(entries) => entries.as_mut_slice(),
                    _ => &mut [],
                };
                for entry in self.detail.iter_mut().chain(entries.iter_mut()) {
                    if entry.id == id {
                        entry.list_status = Some(status);
                    }
                }
                Some(Task::SetStatus { id, status })
            }
            Some(Overlay::SearchFilters) => {
                self.nav.close_overlay();
                let query = self.search_query.trim().to_owned();
                // A filter with no query is a browse, which is a legitimate search. Both
                // empty means there is nothing to ask yet, and that is not an error.
                if query.is_empty() && self.search_filter.is_empty() {
                    return None;
                }
                self.searched_query = query.clone();
                self.content = Content::Loading;
                // New results start at their top, not at the row the old ones were left on.
                self.selected = 0;
                self.offset = 0;
                Some(Task::Search { query, filter: self.search_filter.clone() })
            }
            Some(Overlay::Rate) => {
                let entry = self.target()?;
                let id = entry.id;
                // Rows run 10 down to 1, so the row index is the distance from 10.
                let score = (10 - self.overlay_selected.min(9)) as f32;
                self.nav.close_overlay();
                self.push_toast(Toast::info(format!(
                    "rated {score}/10 — queued for {}",
                    self.signed_in_trackers().join(", ")
                )));
                Some(Task::SetScore { id, score })
            }
            Some(Overlay::Conflicts) => {
                // Enter takes the local value; the remote one is already what the tracker has,
                // so "keep remote" is just dismissing the row.
                let row = self.conflicts.get(self.overlay_selected)?.clone();
                self.conflicts.remove(self.overlay_selected);
                self.overlay_selected =
                    self.overlay_selected.min(self.conflicts.len().saturating_sub(1));
                if self.conflicts.is_empty() {
                    self.nav.close_overlay();
                }
                Some(Task::ResolveConflict { id: row.anilist_id, keep_local: true })
            }
            Some(Overlay::SourceProvider) => {
                let (provider, _) = self.provider_choices().get(self.overlay_selected)?.clone();
                let id = self.title_on_screen()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                // Reflected immediately rather than waiting for the write to come back: the
                // overlay has closed, and a row that still showed the old choice would read
                // as the keystroke having missed.
                self.provider_preference = provider.clone();
                Some(Task::SetProviderPreference { id, provider })
            }
            Some(Overlay::WatchOrder) => {
                let related = self.target()?.related.get(self.overlay_selected)?.clone();
                self.nav.close_overlay();
                self.overlay_selected = 0;
                // Kept, so Back shows this page again rather than the one about to open.
                self.leave_title();
                // A minimal entry stands in until the detail fetch answers — the same
                // stale-data rule as episodes: never render one title's data under another.
                self.detail = Some(Entry::new(related.id, related.title));
                self.nav.push(StageView::Title(related.id));
                Some(Task::LoadDetail(related.id))
            }
            Some(Overlay::Recommendations) => {
                let pick = self.target()?.recommended.get(self.overlay_selected)?.clone();
                self.nav.close_overlay();
                self.overlay_selected = 0;
                self.leave_title();
                // Same stand-in rule as the watch order: never render one title's data
                // under another while the fetch is in flight.
                self.detail = Some(Entry::new(pick.id, pick.title));
                self.nav.push(StageView::Title(pick.id));
                Some(Task::LoadDetail(pick.id))
            }
            Some(Overlay::Sources) => {
                let candidate = self.sources.get(self.overlay_selected)?.clone();
                let (id, episode) = self.source_context.clone()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                self.sources.clear();
                self.source_context = None;
                self.raise_now_playing(id, &episode);
                Some(Task::PlaySource {
                    id,
                    episode,
                    provider_id: candidate.provider_id,
                    source_id: candidate.id,
                })
            }
            Some(Overlay::Disambiguate) => {
                let candidate = self.match_candidates.get(self.overlay_selected)?.clone();
                let (id, provider_id) = self.match_context.clone()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                self.match_candidates.clear();
                self.match_context = None;
                self.episodes_loading = true;
                self.push_toast(Toast::info(format!("matched to {}", candidate.title)));
                Some(Task::FixMatch { id, provider_id, key: candidate.key })
            }
            Some(Overlay::Accounts) => {
                let tracker = self.sync.get(self.overlay_selected)?.clone();
                self.nav.close_overlay();
                self.account_action(tracker)
            }
            Some(Overlay::PluginAction) => self.confirm_plugin_choice(),
            Some(Overlay::PublishPick) => {
                let plugin = self.local_plugins.get(self.overlay_selected)?.id.clone();
                let store = self.publish_for.clone()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                Some(Task::PublishPlugin { store, plugin })
            }
            _ => None,
        }
    }

    /// Rows the currently-open overlay is listing, so movement can be bounded.
    fn overlay_len(&self) -> usize {
        match self.nav.overlay() {
            Some(Overlay::ListStatus) => LibrarySegment::ALL.len(),
            Some(Overlay::Rate) => 10,
            Some(Overlay::SearchFilters) => SearchFilters::ROWS.len(),
            Some(Overlay::Conflicts) => self.conflicts.len(),
            Some(Overlay::Accounts) => self.sync.len(),
            // The palette was missing from this list, which meant the arrows scrolled the list
            // behind it and Enter did nothing — so the app's stated discoverability mechanism
            // could filter actions but never run one.
            Some(Overlay::CommandPalette) => self.palette_matches().len(),
            Some(Overlay::Logs) => self.logs.len(),
            Some(Overlay::Disambiguate) => self.match_candidates.len(),
            Some(Overlay::Sources) => self.sources.len(),
            Some(Overlay::SourceProvider) => self.provider_choices().len(),
            Some(Overlay::WatchOrder) => self.target().map_or(0, |e| e.related.len()),
            Some(Overlay::Recommendations) => self.target().map_or(0, |e| e.recommended.len()),
            Some(Overlay::PluginAction) => self.plugin_choices().len(),
            Some(Overlay::PublishPick) => self.local_plugins.len(),
            _ => 0,
        }
    }

    /// What the source picker offers: automatic first, then every registered source.
    ///
    /// `None` is automatic — the ordinary `providers.order` chain with its failover. It leads
    /// because it is the right answer for almost every title, and pinning is the exception.
    ///
    /// Sources that are registered but currently unusable are still listed. Health is a moment's
    /// state, and hiding a source that is briefly failing would make the choice look impossible
    /// rather than temporarily unavailable — the row says so instead.
    pub fn provider_choices(&self) -> Vec<(Option<String>, String)> {
        let mut choices = vec![(None, "Automatic — best available source".to_string())];
        choices.extend(self.providers.iter().map(|row| {
            let mut label = row.id.clone();
            if !row.kind.is_empty() {
                label.push_str(&format!("  ·  {}", row.kind));
            }
            if row.held_back {
                label.push_str("  ·  held back");
            } else if !row.usable {
                label.push_str("  ·  failing");
            }
            (Some(row.id.clone()), label)
        }));
        choices
    }

    /// The Settings screen's rows, built fresh from the live config.
    ///
    /// Read from `self.config` rather than a copy taken at startup, so a row shows the value that
    /// is actually in force the instant after it is changed. The two settings that can also be
    /// changed for a session only, theme and sub/dub, show what is saved and mark the session's
    /// value beside it: the file and the next launch go by the saved one.
    pub fn setting_rows(&self) -> Vec<SettingRow> {
        use SettingId as S;
        let playback = &self.config.playback;
        let torrent = &self.config.providers.torrent;
        SettingId::ALL
            .iter()
            .map(|id| {
                let (value, editable, note) = match id {
                    S::Theme => (
                        session_marked(
                            format!("{:?}", self.saved_config.theme.mode).to_lowercase(),
                            format!("{:?}", self.config.theme.mode).to_lowercase(),
                        ),
                        Some(("theme", "mode")),
                        None,
                    ),
                    S::Motion => {
                        (on_off(self.config.theme.motion), Some(("theme", "motion")), None)
                    }
                    S::Translation => (
                        session_marked(
                            self.saved_config.playback.translation.to_string(),
                            playback.translation.to_string(),
                        ),
                        Some(("playback", "translation")),
                        None,
                    ),
                    S::Quality => {
                        (format!("{}p", playback.quality), Some(("playback", "quality")), None)
                    }
                    // Text, not a closed set — a language code cycler would be a list of every
                    // ISO 639 code, which is a config-file job rather than a screen job.
                    S::Subtitles => (
                        playback.subtitle_language.clone(),
                        Some(("playback", "subtitle_language")),
                        Some("enter types a language code — too many values to cycle"),
                    ),
                    S::CommitThreshold => (
                        format!("{}%", (playback.commit_threshold * 100.0).round()),
                        Some(("playback", "commit_threshold")),
                        None,
                    ),
                    S::AutoNext => {
                        (on_off(playback.auto_next), Some(("playback", "auto_next")), None)
                    }
                    S::SkipOpening => (
                        on_off(playback.skip_opening),
                        Some(("playback", "skip_opening")),
                        None,
                    ),
                    S::SkipFiller => {
                        (on_off(playback.skip_filler), Some(("playback", "skip_filler")), None)
                    }
                    S::MarkChapters => (
                        on_off(playback.mark_chapters),
                        Some(("playback", "mark_chapters")),
                        Some("chapter marks on the seek bar, where the stream has none"),
                    ),
                    S::Fullscreen => (
                        on_off(playback.fullscreen),
                        Some(("playback", "fullscreen")),
                        Some("mpv opens fullscreen; its own f key still leaves it"),
                    ),
                    S::Upscaling => (
                        upscaling_label(playback.upscaling).into(),
                        Some(("playback", "upscaling")),
                        // Read per playback, so the next episode uses this; the player's own
                        // menu changes the one playing.
                        Some("adapts to the source and your GPU · from the next episode"),
                    ),
                    S::PlayerUi => (
                        match playback.player_ui {
                            anistream_core::config::PlayerUi::Anistream => "anistream",
                            anistream_core::config::PlayerUi::Mpv => "your mpv setup",
                        }
                        .into(),
                        Some(("playback", "player_ui")),
                        Some("thumbnails, menus and the skip button in anistream's look"),
                    ),
                    S::NightMode => (
                        on_off(playback.night_mode),
                        Some(("playback", "night_mode")),
                        Some("evens out quiet dialogue and loud music"),
                    ),
                    S::DownloadDir => (
                        self.config
                            .downloads
                            .directory
                            .clone()
                            .unwrap_or_else(|| "kept with the torrent cache".into()),
                        Some(("downloads", "directory")),
                        Some("enter types a path — applies to the next download"),
                    ),
                    S::MergeSubtitles => (
                        on_off(self.config.downloads.merge_subtitles),
                        Some(("downloads", "merge_subtitles")),
                        Some("applies to the next download that finishes"),
                    ),
                    S::KeepSeeding => (
                        on_off(self.config.downloads.keep_seeding),
                        Some(("downloads", "keep_seeding")),
                        // The privacy dial; saying so is the point of the row.
                        Some("on keeps advertising you as a source — applies immediately"),
                    ),
                    S::DownloadHook => (
                        if self.config.downloads.on_complete.is_some() {
                            "configured".into()
                        } else {
                            "none".into()
                        },
                        Some(("downloads", "on_complete")),
                        Some("enter types a command — run after each finished download"),
                    ),
                    S::Syncplay => (
                        on_off(self.config.syncplay.enabled),
                        Some(("syncplay", "enabled")),
                        // `y` is the ordinary way in; this toggle sends *every* play there.
                        Some(
                            "y plays one episode with the room set below — on sends every play",
                        ),
                    ),
                    S::SyncplayServer => (
                        self.config.syncplay.server.clone(),
                        Some(("syncplay", "server")),
                        Some("enter types host:port"),
                    ),
                    S::SyncplayRoom => (
                        self.config
                            .syncplay
                            .room
                            .clone()
                            .unwrap_or_else(|| "unset — needed for parties".into()),
                        Some(("syncplay", "room")),
                        Some("enter types the room everyone joins"),
                    ),
                    S::UpdateCheck => (
                        on_off(self.config.updates.check),
                        Some(("updates", "check")),
                        Some("one cached request a day, checked at startup"),
                    ),
                    S::Presence => (
                        on_off(self.config.presence.enabled),
                        Some(("presence", "enabled")),
                        // Enabling it without a client id is a no-op, and a toggle that appears to
                        // work while doing nothing is worse than one that says why.
                        if self.config.presence.resolved_client_id().is_some() {
                            Some("takes effect on the next episode")
                        } else {
                            Some(
                                "needs a client id — register an app at discord.com/developers",
                            )
                        },
                    ),
                    S::PresenceTitle => (
                        on_off(self.config.presence.show_title),
                        Some(("presence", "show_title")),
                        // The privacy dial. Off still publishes that you are watching *something*,
                        // which is the point for anyone who wants the presence without the title.
                        Some("off shows \"Watching anime\" without naming the show"),
                    ),
                    S::Torrents => (
                        on_off(torrent.enabled),
                        Some(("providers.torrent", "enabled")),
                        Some("needs a VPN configured; takes effect immediately"),
                    ),
                    // Deliberately not editable. `mode = "none"` requires an explicit
                    // acknowledgement key in the file, and a screen that let you cycle past it
                    // with an arrow key would be a way around friction that exists on purpose.
                    S::VpnMode => (
                        format!("{:?}", torrent.vpn.mode).to_lowercase(),
                        None,
                        Some("edit config.toml — turning this off has to be deliberate"),
                    ),
                    S::SeedRatio => (
                        if torrent.seeding.ratio > 0.0 {
                            format!("until ratio {:.1}", torrent.seeding.ratio)
                        } else {
                            "off".into()
                        },
                        Some(("providers.torrent.seeding", "ratio")),
                        Some("keeps watched episodes seeding while anistream is open"),
                    ),
                    S::SeedHours => (
                        format!("{} h", torrent.seeding.hours),
                        Some(("providers.torrent.seeding", "hours")),
                        Some("the longest one episode keeps seeding, whatever its ratio"),
                    ),
                    S::UploadLimit => (
                        upload_label(torrent.seeding.upload_limit_kib),
                        Some(("providers.torrent.seeding", "upload_limit_kib")),
                        Some("caps all torrent upload, so seeding never slows playback"),
                    ),
                    S::TokenStorage => (
                        self.config.trackers.token_storage.clone(),
                        None,
                        Some("use --token-to-file to move an existing token"),
                    ),
                };
                SettingRow {
                    label: id.label(),
                    category: id.category(),
                    value,
                    editable: editable
                        .map(|(table, key)| SettingEdit { table: table_path(table), key }),
                    note,
                }
            })
            .collect()
    }

    /// The name the Settings screen gives the setting stored at `table`.`key`, for saying it
    /// was saved. The config key (`commit_threshold`) is the file's vocabulary, not the
    /// screen's, and the value needs no repeating: the row beside it already shows it.
    ///
    /// `None` for what is stored without a row of its own, the remembered speed and volume:
    /// saved in passing, and not something anyone asked to have confirmed.
    pub fn setting_label(&self, table: &[&str], key: &str) -> Option<&'static str> {
        self.setting_rows()
            .into_iter()
            .find(|row| row.editable.as_ref().is_some_and(|e| e.table == table && e.key == key))
            .map(|row| row.label)
    }

    /// Step the selected setting through its values.
    ///
    /// Cycling rather than free entry, because every editable setting here has a small closed set
    /// of sensible values — and a cycler cannot produce a config the app then refuses to load.
    fn cycle_setting(&mut self, delta: isize) -> Option<Task> {
        use anistream_core::config::ThemeMode;
        use anistream_core::media::Translation;
        use anistream_core::settings::SettingValue as V;

        let id = *SettingId::ALL.get(self.selected)?;
        let rows = self.setting_rows();
        let edit = rows.get(self.selected)?.editable.clone();
        let Some(edit) = edit else {
            let note = rows.get(self.selected).and_then(|r| r.note);
            self.push_toast(Toast::info(note.unwrap_or("not editable here")));
            return None;
        };

        let value = match id {
            SettingId::Theme => {
                // From the saved value, which is the one the row shows: the arrows change
                // what is saved, and a session override simply gives way to it.
                let next = match self.saved_config.theme.mode {
                    ThemeMode::Adaptive => ThemeMode::Immersive,
                    ThemeMode::Immersive => ThemeMode::Adaptive,
                };
                self.config.theme.mode = next;
                // Repaint immediately. Re-running detection would read the terminal's background
                // over OSC 11 from inside the render loop, which competes with the key reader for
                // stdin; the variant found at startup is already the right answer.
                self.palette = crate::theme::Palette::of(match next {
                    ThemeMode::Immersive => crate::theme::Variant::Immersive,
                    ThemeMode::Adaptive => self.adaptive_variant,
                });
                V::Str(
                    if next == ThemeMode::Immersive { "immersive" } else { "adaptive" }.into(),
                )
            }
            SettingId::Motion => {
                self.config.theme.motion = !self.config.theme.motion;
                V::Bool(self.config.theme.motion)
            }
            SettingId::Translation => {
                let next = match self.saved_config.playback.translation {
                    Translation::Sub => Translation::Dub,
                    Translation::Dub => Translation::Sub,
                };
                self.config.playback.translation = next;
                V::Str(next.to_string())
            }
            SettingId::Quality => {
                const LADDER: [u32; 4] = [480, 720, 1080, 2160];
                let next = step_through(&LADDER, self.config.playback.quality, delta);
                self.config.playback.quality = next;
                V::Int(i64::from(next))
            }
            SettingId::CommitThreshold => {
                const LADDER: [u32; 5] = [70, 80, 85, 90, 95];
                let current = (self.config.playback.commit_threshold * 100.0).round() as u32;
                let next = step_through(&LADDER, current, delta);
                self.config.playback.commit_threshold = f64::from(next) / 100.0;
                V::Float(f64::from(next) / 100.0)
            }
            SettingId::AutoNext => {
                self.config.playback.auto_next = !self.config.playback.auto_next;
                V::Bool(self.config.playback.auto_next)
            }
            SettingId::Upscaling => {
                use anistream_core::config::Upscaling as U;
                const LADDER: [U; 3] = [U::Off, U::Natural, U::Sharp];
                let at = LADDER
                    .iter()
                    .position(|u| *u == self.config.playback.upscaling)
                    .unwrap_or(0) as isize;
                let next = LADDER[(at + delta).clamp(0, 2) as usize];
                self.config.playback.upscaling = next;
                V::Str(next.name().into())
            }
            SettingId::PlayerUi => {
                use anistream_core::config::PlayerUi as P;
                let next = match self.config.playback.player_ui {
                    P::Anistream => P::Mpv,
                    P::Mpv => P::Anistream,
                };
                self.config.playback.player_ui = next;
                V::Str(if next == P::Anistream { "anistream" } else { "mpv" }.into())
            }
            SettingId::NightMode => {
                self.config.playback.night_mode = !self.config.playback.night_mode;
                V::Bool(self.config.playback.night_mode)
            }
            SettingId::MergeSubtitles => {
                self.config.downloads.merge_subtitles = !self.config.downloads.merge_subtitles;
                V::Bool(self.config.downloads.merge_subtitles)
            }
            SettingId::KeepSeeding => {
                self.config.downloads.keep_seeding = !self.config.downloads.keep_seeding;
                V::Bool(self.config.downloads.keep_seeding)
            }
            SettingId::Syncplay => {
                self.config.syncplay.enabled = !self.config.syncplay.enabled;
                V::Bool(self.config.syncplay.enabled)
            }
            SettingId::UpdateCheck => {
                self.config.updates.check = !self.config.updates.check;
                V::Bool(self.config.updates.check)
            }
            SettingId::SkipOpening => {
                self.config.playback.skip_opening = !self.config.playback.skip_opening;
                V::Bool(self.config.playback.skip_opening)
            }
            SettingId::SkipFiller => {
                self.config.playback.skip_filler = !self.config.playback.skip_filler;
                V::Bool(self.config.playback.skip_filler)
            }
            SettingId::MarkChapters => {
                self.config.playback.mark_chapters = !self.config.playback.mark_chapters;
                V::Bool(self.config.playback.mark_chapters)
            }
            SettingId::Fullscreen => {
                self.config.playback.fullscreen = !self.config.playback.fullscreen;
                V::Bool(self.config.playback.fullscreen)
            }
            SettingId::Presence => {
                self.config.presence.enabled = !self.config.presence.enabled;
                V::Bool(self.config.presence.enabled)
            }
            SettingId::PresenceTitle => {
                self.config.presence.show_title = !self.config.presence.show_title;
                V::Bool(self.config.presence.show_title)
            }
            SettingId::Torrents => {
                self.config.providers.torrent.enabled = !self.config.providers.torrent.enabled;
                V::Bool(self.config.providers.torrent.enabled)
            }
            SettingId::SeedRatio => {
                // Tenths, so the ladder can be matched exactly rather than as floats.
                const LADDER: [u32; 5] = [0, 5, 10, 15, 20];
                let seeding = &mut self.config.providers.torrent.seeding;
                let current = (seeding.ratio * 10.0).round() as u32;
                seeding.ratio = f64::from(step_through(&LADDER, current, delta)) / 10.0;
                V::Float(seeding.ratio)
            }
            SettingId::SeedHours => {
                const LADDER: [u32; 5] = [1, 6, 12, 24, 72];
                let seeding = &mut self.config.providers.torrent.seeding;
                seeding.hours = step_through(&LADDER, seeding.hours, delta);
                V::Int(i64::from(seeding.hours))
            }
            SettingId::UploadLimit => {
                const LADDER: [u32; 6] = [0, 256, 512, 1024, 2048, 5120];
                let seeding = &mut self.config.providers.torrent.seeding;
                seeding.upload_limit_kib =
                    step_through(&LADDER, seeding.upload_limit_kib, delta);
                V::Int(i64::from(seeding.upload_limit_kib))
            }
            SettingId::VpnMode | SettingId::TokenStorage => return None,
            // Text values are typed, not cycled — arrows have nothing to step through.
            SettingId::Subtitles
            | SettingId::DownloadDir
            | SettingId::DownloadHook
            | SettingId::SyncplayServer
            | SettingId::SyncplayRoom => {
                self.push_toast(Toast::info("enter types a value here"));
                return None;
            }
        };

        Some(Task::SaveSetting { table: edit.table, key: edit.key, value })
    }

    /// Put a setting whose write failed back to what the file holds.
    ///
    /// The reducer changes a setting before the write it asks for, so the screen answers at
    /// once; when the write was refused — enabling torrents with no VPN, an unwritable file —
    /// the screen and the status line went on showing the value that never landed.
    fn roll_back_setting(&mut self, table: &[&str], key: &str) {
        let rows = self.setting_rows();
        let Some(id) = SettingId::ALL.iter().zip(&rows).find_map(|(id, row)| {
            let edit = row.editable.as_ref()?;
            (edit.table == table && edit.key == key).then_some(*id)
        }) else {
            return;
        };
        id.copy(&self.saved_config, &mut self.config);
        if id == SettingId::Theme {
            use anistream_core::config::ThemeMode;
            self.palette = crate::theme::Palette::of(match self.config.theme.mode {
                ThemeMode::Immersive => crate::theme::Variant::Immersive,
                ThemeMode::Adaptive => self.adaptive_variant,
            });
        }
        // The status line falls back to what is in effect, rather than a message about an
        // earlier save.
        self.status.clear();
    }

    /// Actions matching the current palette query, in the order they are rendered.
    ///
    /// One source of truth shared by movement, Enter and the renderer. Deriving the selectable
    /// set separately from the drawn set is how a palette ends up running the wrong entry.
    pub fn palette_matches(&self) -> Vec<(Action, String)> {
        // The keys the screen will read once the palette is closed, not the palette's own.
        let context = if self.playing.is_some() { Context::Playing } else { Context::Browsing };
        self.keymap
            .palette_entries(&self.palette_query, context)
            .into_iter()
            .take(PALETTE_ROWS)
            .collect()
    }

    fn handle_while_typing(&mut self, action: Action, visible_rows: usize) -> Option<Task> {
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
            Action::Back => {
                let credentials_open = matches!(
                    self.nav.overlay(),
                    Some(Overlay::KitsuUsername | Overlay::KitsuPassword)
                );
                if !self.nav.close_overlay() {
                    // Nothing to dismiss — leave the field rather than trapping the user
                    // in it.
                    self.nav.focus_rail();
                }
                if credentials_open {
                    // A dismissed sign-in leaves nothing behind to resurface later.
                    self.kitsu_user.clear();
                    self.kitsu_pass.clear();
                }
                // An invite link is a one-time credential; a dismissed prompt forgets it.
                self.join_link.clear();
                self.invite_name.clear();
            }
            Action::CommandPalette => {
                self.palette_query.clear();
                self.nav.open_overlay(Overlay::CommandPalette);
            }
            Action::Help => self.open_help(),
            Action::Quit => self.should_quit = true,
            // The two sign-in steps. Enter with nothing typed stays put — an empty
            // submit is a slip of the finger, not an answer.
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuUsername) => {
                if self.kitsu_user.trim().is_empty() {
                    return None;
                }
                self.nav.close_overlay();
                self.nav.open_overlay(Overlay::KitsuPassword);
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuPassword) => {
                if self.kitsu_pass.is_empty() {
                    return None;
                }
                // Taken, not cloned: the buffer must not hold a live password after the
                // exchange is dispatched.
                let username = self.kitsu_user.trim().to_owned();
                let password = Secret(std::mem::take(&mut self.kitsu_pass));
                self.kitsu_user.clear();
                self.nav.close_overlay();
                return Some(Task::ConnectKitsu { username, password });
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::JoinStore) => {
                // A link that wrapped in a chat window arrives with spaces and line breaks in
                // it, and a URL has no place for either.
                let link: String =
                    self.join_link.chars().filter(|c| !c.is_whitespace()).collect();
                if link.is_empty() {
                    return None;
                }
                self.nav.close_overlay();
                return Some(Task::PreviewStore { link: Secret(link) });
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::InviteName) => {
                let name = self.invite_name.trim().to_owned();
                if name.is_empty() {
                    return None;
                }
                let store = self.store_view()?.to_owned();
                self.invite_name.clear();
                self.nav.close_overlay();
                return Some(Task::CreateInvite { store, name: Some(name) });
            }
            // Mid-query is exactly when filters are wanted, so ctrl+f cannot wait for the
            // box to lose focus.
            Action::FilterSearch => {
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::SearchFilters);
            }
            // The palette takes text, so it reaches this path rather than the overlay block in
            // `handle`. It used to close on Enter without running anything, and move the list
            // *behind* it on the arrows — so the one mechanism meant to make every action
            // discoverable could show you an action and then refuse to perform it.
            Action::Open if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                return self.confirm_overlay(visible_rows);
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::EditSetting) => {
                let id = self.edit_target.take()?;
                let (table, key) = id.text_edit()?;
                let value = self.edit_value.trim().to_owned();
                self.nav.close_overlay();
                if value.is_empty() {
                    // The writer sets keys; it does not delete them. Refusing beats
                    // writing an empty string that half the readers treat as a path.
                    self.push_toast(Toast::info(
                        "left unchanged — delete the line in config.toml to unset",
                    ));
                    return None;
                }
                match id {
                    SettingId::Subtitles => {
                        self.config.playback.subtitle_language = value.clone();
                    }
                    SettingId::DownloadDir => {
                        self.config.downloads.directory = Some(value.clone());
                    }
                    SettingId::DownloadHook => {
                        self.config.downloads.on_complete = Some(value.clone());
                    }
                    SettingId::SyncplayServer => {
                        self.config.syncplay.server = value.clone();
                    }
                    SettingId::SyncplayRoom => {
                        self.config.syncplay.room = Some(value.clone());
                    }
                    _ => return None,
                }
                // No toast here: the write reports itself once it has landed, which is the
                // moment "saved" is true. Saying it now as well put two confirmations up.
                return Some(Task::SaveSetting {
                    table,
                    key,
                    value: anistream_core::settings::SettingValue::Str(value),
                });
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::DownloadRange) => {
                let id = match self.nav.current() {
                    StageView::Episodes(id) => *id,
                    _ => return None,
                };
                // Resolved against episodes that actually exist, from the unfiltered
                // list — a filter narrows the view, not what can be fetched.
                let Some((from, to)) = parse_episode_range(&self.range_query) else {
                    // Cleared, with the prompt left open for another try. Kept, the retry's
                    // keys were appended to the refused text and refused again.
                    self.range_query.clear();
                    self.push_toast(Toast::info("ranges look like 4, 1-12 or 7-"));
                    return None;
                };
                let episodes: Vec<String> = self
                    .episodes_all
                    .iter()
                    .filter(|row| {
                        row.number
                            .trim()
                            .parse::<u32>()
                            .is_ok_and(|n| n >= from && to.is_none_or(|t| n <= t))
                    })
                    .map(|row| row.number.clone())
                    .collect();
                if episodes.is_empty() {
                    self.range_query.clear();
                    self.push_toast(Toast::info("no episodes in that range"));
                    return None;
                }
                self.nav.close_overlay();
                self.push_toast(Toast::info(format!(
                    "queueing {}…",
                    crate::widgets::plural(episodes.len(), "episode")
                )));
                return Some(Task::DownloadMany { id, episodes });
            }
            // Enter runs the manual search; the results come back as ordinary match
            // choices, so the whole Disambiguate tail is reused unchanged.
            Action::Open if self.nav.overlay() == Some(&Overlay::ManualQuery) => {
                let query = self.manual_query.trim().to_owned();
                let id = self.manual_target?;
                self.nav.close_overlay();
                // Empty enter is the undo: forget the pin, let the ladder decide again.
                if query.is_empty() {
                    self.episodes.clear();
                    self.episodes_loading = true;
                    return Some(Task::ClearMatch { id });
                }
                return Some(Task::ManualSearch { id, query });
            }
            Action::Down if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                let last = self.palette_matches().len().saturating_sub(1);
                self.overlay_selected = (self.overlay_selected + 1).min(last);
            }
            Action::Up if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                self.overlay_selected = self.overlay_selected.saturating_sub(1);
            }
            Action::Open if self.nav.section() == Section::Search => {
                let query = self.search_query.trim().to_owned();
                if query.is_empty() {
                    return None;
                }
                // Enter submitted the search *every* time, so a result could be highlighted and
                // never opened — the key just re-ran the same query. It submits only while there is
                // something new to ask; once the results on screen are for this query, Enter does
                // what Enter does everywhere else in the app and opens the selection.
                if self.search_opens_on_enter() {
                    return self.open_selected();
                }
                self.searched_query = query.clone();
                self.content = Content::Loading;
                // New results start at their top, not at the row the old ones were left on.
                self.selected = 0;
                self.offset = 0;
                return Some(Task::Search { query, filter: self.search_filter.clone() });
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

    /// Whether metadata is currently served by anything other than AniList.
    ///
    /// The header chip's question. Answered from the last status report, so it holds
    /// between Providers-screen visits.
    pub fn meta_degraded(&self) -> bool {
        self.meta_status.iter().any(|row| row.active && row.source != "anilist")
    }

    /// Whether Enter resumes the highlighted row rather than opening it: a part-watched title
    /// on the Home list. One answer for the key and for the hint that names it.
    pub fn resumes_on_enter(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Home))
            && self.selected_entry().is_some_and(|e| e.resume.is_some() && e.progress.is_some())
    }

    /// Whether Enter in Search's field opens the highlighted result rather than searching:
    /// the results on screen are already for the query as typed.
    pub fn search_opens_on_enter(&self) -> bool {
        let query = self.search_query.trim();
        !query.is_empty() && query == self.searched_query && !self.content.is_empty()
    }

    /// Whether the episode table currently has focus.
    pub fn in_episodes(&self) -> bool {
        matches!(self.nav.current(), StageView::Episodes(_))
    }

    /// Whether keystrokes should be treated as text rather than bindings.
    pub fn is_typing(&self) -> bool {
        self.nav.overlay().is_some_and(Overlay::takes_text_input)
            || (self.nav.section() == Section::Search
                && self.nav.focus() == Focus::Stage
                && !self.nav.has_overlay()
                && !self.search_in_results
                // Only the search section's *own* view is a text field. Without this, opening a
                // result and then typing edited the query behind the title you were looking at,
                // and `Esc` left the field instead of going back — two different surprises from
                // one missing condition.
                && matches!(self.nav.current(), StageView::Section(_)))
    }

    /// Feed a character to whichever text field is active.
    pub fn type_char(&mut self, ch: char) {
        if self.nav.overlay() == Some(&Overlay::CommandPalette) {
            self.palette_query.push(ch);
            // Filtering reorders the matches, so the old index would point at an unrelated
            // action — and the top hit is what a fuzzy filter is for.
            self.overlay_selected = 0;
        } else if self.nav.overlay() == Some(&Overlay::ManualQuery) {
            self.manual_query.push(ch);
        } else if self.nav.overlay() == Some(&Overlay::DownloadRange) {
            // A range is digits and one dash; letting anything else in would make the
            // parse failure the user's puzzle instead of the input's boundary.
            if ch.is_ascii_digit() || ch == '-' {
                self.range_query.push(ch);
            }
        } else if self.nav.overlay() == Some(&Overlay::EditSetting) {
            self.edit_value.push(ch);
        } else if self.nav.overlay() == Some(&Overlay::KitsuUsername) {
            self.kitsu_user.push(ch);
        } else if self.nav.overlay() == Some(&Overlay::KitsuPassword) {
            self.kitsu_pass.push(ch);
        } else if self.nav.overlay() == Some(&Overlay::JoinStore) {
            self.join_link.push(ch);
        } else if self.nav.overlay() == Some(&Overlay::InviteName) {
            // The server takes up to 64; stopping here says so before it is refused.
            if self.invite_name.chars().count() < 64 {
                self.invite_name.push(ch);
            }
        } else if self.nav.section() == Section::Search {
            self.search_query.push(ch);
        }
    }

    pub fn backspace(&mut self) {
        if self.nav.overlay() == Some(&Overlay::CommandPalette) {
            self.palette_query.pop();
            self.overlay_selected = 0;
        } else if self.nav.overlay() == Some(&Overlay::ManualQuery) {
            self.manual_query.pop();
        } else if self.nav.overlay() == Some(&Overlay::DownloadRange) {
            self.range_query.pop();
        } else if self.nav.overlay() == Some(&Overlay::EditSetting) {
            self.edit_value.pop();
        } else if self.nav.overlay() == Some(&Overlay::KitsuUsername) {
            self.kitsu_user.pop();
        } else if self.nav.overlay() == Some(&Overlay::KitsuPassword) {
            self.kitsu_pass.pop();
        } else if self.nav.overlay() == Some(&Overlay::JoinStore) {
            self.join_link.pop();
        } else if self.nav.overlay() == Some(&Overlay::InviteName) {
            self.invite_name.pop();
        } else if self.nav.section() == Section::Search {
            self.search_query.pop();
        }
    }

    /// Push the selected title's detail screen — what Enter does on a row with nothing to
    /// resume. Public for `--preview`, which shows a title page from a list whose Enter plays.
    pub fn open_selected(&mut self) -> Option<Task> {
        let Some(entry) = self.selected_entry().cloned() else {
            self.push_toast(Toast::info("nothing selected"));
            return None;
        };
        let id = entry.id;
        self.detail = Some(entry);
        self.nav.push(StageView::Title(id));
        Some(Task::LoadDetail(id))
    }

    /// Move the rail selection by `delta`, clamping at the ends.
    ///
    /// Clamps rather than wraps: a wrapping rail means holding ↓ cycles forever with no sense of
    /// where you are, and eight sections is short enough that hitting the end is informative.
    fn step_section(&mut self, delta: isize) -> Option<Task> {
        let current = self.nav.section().index() as isize;
        let last = (Section::ALL.len() - 1) as isize;
        let next = (current + delta).clamp(0, last) as usize;
        if next == current as usize {
            return None;
        }
        let section = Section::from_index(next)?;
        // Move now, fetch shortly. Holding the arrow key sweeps the rail at keyboard-repeat speed,
        // and firing a request per step is how a 30-per-minute budget disappears in two seconds.
        self.nav.go_to(section);
        self.nav.focus_rail();
        self.selected = 0;
        self.offset = 0;
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        // Loading only where a list is about to be asked for. A section with no list of its
        // own — or Search with nothing typed — would otherwise pulse forever, since the fetch
        // that ends the pulse never goes out.
        self.content = if self.fetches_list() { Content::Loading } else { Content::Empty };
        self.reload_countdown = Some(RELOAD_IDLE_TICKS);
        None
    }

    /// Fire a settled section change. Called from the idle tick.
    pub fn tick_pending_reload(&mut self) {
        let Some(remaining) = self.reload_countdown else { return };
        if remaining > 0 {
            self.reload_countdown = Some(remaining - 1);
            return;
        }
        self.reload_countdown = None;
        if let Some(task) = self.reload() {
            // Through `pending` because the idle tick has no return channel, the same route
            // auto-next already uses.
            self.pending = Some(task);
        }
    }

    pub fn go_to_section(&mut self, section: Section) -> Option<Task> {
        self.nav.go_to(section);
        self.selected = 0;
        self.offset = 0;
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        self.reload()
    }

    /// Start a [`Job`] for `task`, if it is work the user will wait on.
    ///
    /// Called by whoever runs the task, at the moment it does, so the job's clock starts when
    /// the work does; the runner ends it with [`Update::JobDone`] when the work stops.
    pub fn begin_job(&mut self, task: &Task) -> Option<JobId> {
        let label = task.activity()?;
        self.next_job += 1;
        let id = JobId(self.next_job);
        self.jobs.push(Job { id, label, started: Instant::now(), note: None });
        Some(id)
    }

    /// The jobs worth showing at `now`, newest first: the thing just asked for leads, and work
    /// that has been running a while trails after it.
    pub fn activity(&self, now: Instant) -> Vec<&Job> {
        self.jobs
            .iter()
            .rev()
            .filter(|job| now.saturating_duration_since(job.started) >= ACTIVITY_DELAY)
            .collect()
    }

    /// The list the current view shows, if it shows one — what an [`Update::Content`] has to
    /// be tagged with to be taken.
    pub fn content_view(&self) -> Option<ContentFor> {
        Some(match self.nav.section() {
            Section::Home => ContentFor::Continue,
            Section::Seasonal => ContentFor::Seasonal,
            Section::Calendar => ContentFor::Calendar,
            Section::Search => ContentFor::Search {
                query: self.searched_query.clone(),
                filter: self.search_filter.clone(),
            },
            Section::Library => ContentFor::Library(self.library_segment),
            _ => return None,
        })
    }

    /// The caption of the list on screen, when the binary sent one for it.
    pub fn list_caption(&self) -> Option<&str> {
        let (view, text) = self.caption.as_ref()?;
        (self.content_view().as_ref() == Some(view)).then_some(text.as_str())
    }

    /// An answer for what is on screen right now: the list for the current view, the episodes
    /// of the open title, the releases for the open picker.
    ///
    /// For callers that hold the app while they fill it — the preview, tests — and so cannot be
    /// answering a request the user has since left. Background work tags its answer with what
    /// it was asked for instead. An answer with nothing on screen to take it (no list view, no
    /// open title) is tagged so the reducer drops it, exactly as it would a late one.
    pub fn content_here(&self, content: Content) -> Update {
        // With no list view on screen the reducer drops every tag, so which one is moot.
        let view = self.content_view().unwrap_or(ContentFor::Continue);
        Update::Content { view, content }
    }

    /// [`Self::content_here`] for the open title's episode table.
    pub fn episodes_here(&self, rows: Vec<EpisodeRow>) -> Update {
        // No title open means the reducer drops the rows whatever id they carry.
        let id = self.detail.as_ref().map_or(AnilistId::new(0), |d| d.id);
        Update::Episodes { id, rows }
    }

    /// [`Self::content_here`] for the open source picker.
    pub fn sources_here(&self, candidates: Vec<SourceCandidate>) -> Update {
        let (id, episode) =
            self.source_context.clone().unwrap_or((AnilistId::new(0), String::new()));
        Update::Sources { id, episode, candidates }
    }

    /// Work needed to populate the current view.
    pub fn reload(&mut self) -> Option<Task> {
        // Whatever is fetched next replaces the content, so the calendar's paging describes a
        // list that is about to stop existing. Leaving it set would let a scroll on the new
        // screen ask for page four of a timeline nobody is looking at any more.
        self.calendar_anchor = None;
        self.calendar_pages = 0;
        self.calendar_more_past = false;
        self.calendar_loading_past = false;

        let task = match self.nav.section() {
            Section::Home => Task::LoadContinue,
            Section::Seasonal => Task::LoadSeasonal,
            Section::Calendar => Task::LoadCalendar,
            Section::Search
                if !self.search_query.trim().is_empty() || !self.search_filter.is_empty() =>
            {
                // Recorded as submitted, the same as Enter does, because the answer is tagged
                // with this query and only taken while it is still the one on screen.
                let query = self.search_query.trim().to_owned();
                self.searched_query = query.clone();
                Task::Search { query, filter: self.search_filter.clone() }
            }
            Section::Providers => Task::CheckProviders,
            Section::Downloads => Task::LoadDownloads,
            Section::Library => Task::LoadLibrary(self.library_segment),
            // Sections with nothing to fetch yet render their own placeholder.
            _ => {
                self.content = Content::Empty;
                return None;
            }
        };
        // `content` is the section's list, so a section without one — or a search with nothing
        // typed — empties it rather than leaving the last section's rows to be drawn, scrolled
        // and acted on under its heading.
        self.content = if self.fetches_list() { Content::Loading } else { Content::Empty };
        Some(task)
    }

    /// Whether the section on screen fills `content` with a list it is about to ask for.
    fn fetches_list(&self) -> bool {
        match self.nav.section() {
            Section::Home | Section::Seasonal | Section::Calendar | Section::Library => true,
            Section::Search => {
                !self.search_query.trim().is_empty() || !self.search_filter.is_empty()
            }
            _ => false,
        }
    }
}

impl App {
    /// Artwork the current view will render, so the caller can prefetch it.
    ///
    /// Returns the *visible* set only. Fetching every cover in a 40-item page would spend
    /// bandwidth on rows nobody has scrolled to.
    pub fn visible_artwork(&self, visible_rows: usize) -> Vec<String> {
        let mut urls = Vec::new();

        if let Some(detail) = &self.detail
            && matches!(self.nav.current(), StageView::Title(_))
        {
            // The Title screen leads with the banner; the cover is the fallback.
            if let Some(url) = detail.banner_url.as_ref().or(detail.cover_url.as_ref()) {
                urls.push(url.clone());
            }
        }
        if let Some(selected) = self.selected_entry()
            && let Some(url) = &selected.cover_url
        {
            urls.push(url.clone());
        }
        // The still for the episode under the cursor, plus its immediate neighbours so that
        // moving one row does not stall on a fetch.
        if matches!(self.nav.current(), StageView::Episodes(_)) {
            let from = self.episode_selected.saturating_sub(1);
            for row in self.episodes.iter().skip(from).take(3) {
                if let Some(url) = &row.thumbnail {
                    urls.push(url.clone());
                }
            }
        }
        // A small lookahead so scrolling does not stall on every step.
        for entry in
            self.content.entries().iter().skip(self.offset).take(visible_rows.saturating_add(4))
        {
            if let Some(url) = &entry.cover_url {
                urls.push(url.clone());
            }
        }
        urls.retain(|u| self.images.should_fetch(u));
        urls
    }
}

/// Plugin stores.
impl App {
    pub fn store(&self, id: &str) -> Option<&StoreRow> {
        self.stores.iter().find(|s| s.id == id)
    }

    /// Providers' own view, where Enter opens the stores.
    fn on_providers(&self) -> bool {
        self.nav.section() == Section::Providers
            && matches!(self.nav.current(), StageView::Section(Section::Providers))
    }

    fn in_stores(&self) -> bool {
        self.nav.current() == &StageView::Stores
    }

    /// The store whose page is on screen.
    pub fn store_view(&self) -> Option<&str> {
        match self.nav.current() {
            StageView::Store(id) => Some(id),
            _ => None,
        }
    }

    /// The store whose members are on screen.
    pub fn members_view(&self) -> Option<&str> {
        match self.nav.current() {
            StageView::StoreMembers(id) => Some(id),
            _ => None,
        }
    }

    /// What a store's page lists, top to bottom: its plugins, then what can be done there.
    ///
    /// The actions are rows rather than keys. They are used rarely enough that a key nobody
    /// remembers is worse than a line that says what it does.
    pub fn store_lines(&self, store: &StoreRow) -> Vec<StoreLine> {
        let mut lines: Vec<StoreLine> =
            (0..store.plugins.len()).map(StoreLine::Plugin).collect();
        if store.state != StoreState::Revoked {
            lines.push(StoreLine::LinkDevice);
            if store.owner {
                lines.extend([
                    StoreLine::Invite,
                    StoreLine::Members,
                    StoreLine::Publish,
                    StoreLine::Admin,
                ]);
            }
        }
        lines.push(StoreLine::Leave);
        lines
    }

    fn apply_stores(&mut self, rows: Vec<StoreRow>) {
        self.stores = rows;
        self.stores_loaded = true;
        // One past the last store is the "join a store" row.
        self.store_selected = self.store_selected.min(self.stores.len());

        // A store that was left takes its screens with it, rather than leaving a page about
        // something that no longer exists.
        let gone = |app: &Self, id: &str| app.store(id).is_none();
        if let Some(id) = self.members_view().map(str::to_owned)
            && gone(self, &id)
        {
            self.nav.close_all_overlays();
            self.nav.pop();
        }
        if let Some(id) = self.store_view().map(str::to_owned) {
            if gone(self, &id) {
                self.nav.close_all_overlays();
                self.nav.pop();
            } else if let Some(store) = self.store(&id) {
                let last = self.store_lines(store).len().saturating_sub(1);
                self.store_line_selected = self.store_line_selected.min(last);
            }
        }
    }

    fn apply_store_preview(&mut self, outcome: Result<StorePreview, String>) {
        match outcome {
            // Asked from the store list with nothing open. Anywhere else, a confirmation that
            // took over would be answering a question the user is not looking at — and an open
            // "leave" or "remove" would quietly become a "join" under the same Enter.
            Ok(_) if !self.in_stores() || self.nav.has_overlay() => {
                self.push_toast(Toast::info("the invite checked out — paste it again to join"));
            }
            Ok(preview) => {
                self.store_confirm = Some(StoreConfirm::Join(preview));
                self.nav.open_overlay(Overlay::StoreConfirm);
            }
            Err(reason) => {
                self.push_toast(Toast::alert(reason));
                // Back to the prompt with the link still in it, so a link cut short in the
                // copy can be finished rather than pasted again — when the user is still here.
                if self.in_stores() && !self.nav.has_overlay() {
                    self.nav.open_overlay(Overlay::JoinStore);
                }
            }
        }
    }

    fn open_stores(&mut self) -> Option<Task> {
        self.store_selected = 0;
        self.nav.push(StageView::Stores);
        Some(Task::LoadStores)
    }

    /// Enter on the store list: into a store, or the join prompt from the last row.
    fn open_store_row(&mut self) -> Option<Task> {
        match self.stores.get(self.store_selected) {
            Some(store) => {
                let id = store.id.clone();
                self.store_line_selected = 0;
                self.nav.push(StageView::Store(id));
            }
            None => {
                self.join_link.clear();
                self.nav.open_overlay(Overlay::JoinStore);
            }
        }
        None
    }

    fn open_store_line(&mut self, id: &str) -> Option<Task> {
        let store = self.store(id)?.clone();
        let line = *self.store_lines(&store).get(self.store_line_selected)?;
        match line {
            StoreLine::Plugin(i) => {
                let plugin = store.plugins.get(i)?.id.clone();
                self.show_plugin(&store.id, &plugin);
                None
            }
            StoreLine::LinkDevice => Some(Task::CreateInvite { store: store.id, name: None }),
            StoreLine::Invite => {
                self.invite_name.clear();
                self.nav.open_overlay(Overlay::InviteName);
                None
            }
            StoreLine::Members => {
                self.members_for = Some(store.id.clone());
                self.store_members.clear();
                self.store_invites.clear();
                self.member_selected = 0;
                self.nav.push(StageView::StoreMembers(store.id.clone()));
                Some(Task::LoadMembers { store: store.id })
            }
            StoreLine::Publish => {
                self.publish_for = Some(store.id.clone());
                self.local_plugins.clear();
                Some(Task::LoadLocalPlugins { store: store.id })
            }
            // No status line of its own: the activity row says it is opening, and a toast says
            // how it went. A status set here was never cleared, so it outlived both.
            StoreLine::Admin => Some(Task::OpenAdmin { store: store.id }),
            StoreLine::Leave => {
                self.store_confirm =
                    Some(StoreConfirm::Leave { store: store.id, name: store.name });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
        }
    }

    /// Open the plugin overlay for one of a store's plugins, with its choices as they stand now.
    pub fn show_plugin(&mut self, store: &str, plugin: &str) {
        self.plugin_action = Some((store.to_owned(), plugin.to_owned()));
        self.plugin_options = self.current_plugin_choices();
        self.overlay_selected = 0;
        self.nav.open_overlay(Overlay::PluginAction);
    }

    /// The store and plugin the plugin overlay is about.
    pub fn plugin_in_action(&self) -> Option<(&StoreRow, &StorePluginRow)> {
        let (store_id, plugin_id) = self.plugin_action.as_ref()?;
        let store = self.store(store_id)?;
        let plugin = store.plugins.iter().find(|p| &p.id == plugin_id)?;
        Some((store, plugin))
    }

    /// What the plugin overlay offers, in order, as it was when the overlay opened.
    pub fn plugin_choices(&self) -> &[PluginChoice] {
        &self.plugin_options
    }

    /// What the plugin overlay would offer for the plugin it is about, from the rows as they are.
    fn current_plugin_choices(&self) -> Vec<PluginChoice> {
        let Some((store, plugin)) = self.plugin_in_action() else { return Vec::new() };
        // A store that refused this device cannot be downloaded from, but what it installed
        // can still be removed.
        let live = store.state != StoreState::Revoked;
        let mut choices = Vec::new();
        match &plugin.status {
            StorePluginStatus::Available if live => choices.push(PluginChoice::Install),
            StorePluginStatus::Update { .. } if live => {
                choices.push(PluginChoice::Update(plugin.version.clone()));
            }
            StorePluginStatus::AddedByHand if live => choices.push(PluginChoice::Replace),
            _ => {}
        }
        if matches!(
            plugin.status,
            StorePluginStatus::Installed | StorePluginStatus::Update { .. }
        ) {
            choices.push(PluginChoice::Remove);
        }
        if store.owner && live {
            choices.push(PluginChoice::Unpublish);
        }
        choices.push(PluginChoice::Cancel);
        choices
    }

    fn confirm_plugin_choice(&mut self) -> Option<Task> {
        let choice = self.plugin_options.get(self.overlay_selected)?.clone();
        self.plugin_options.clear();
        let name = self.plugin_in_action().map(|(_, p)| p.name.clone()).unwrap_or_default();
        let (store, plugin) = self.plugin_action.clone()?;
        self.nav.close_overlay();
        self.overlay_selected = 0;
        match choice {
            PluginChoice::Install | PluginChoice::Update(_) | PluginChoice::Replace => {
                Some(Task::InstallPlugin { store, plugin })
            }
            PluginChoice::Remove => Some(Task::RemovePlugin { plugin }),
            PluginChoice::Unpublish => {
                self.store_confirm = Some(StoreConfirm::Unpublish { store, plugin, name });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
            PluginChoice::Cancel => None,
        }
    }

    fn confirm_store_action(&mut self) -> Option<Task> {
        let confirm = self.store_confirm.take()?;
        self.nav.close_overlay();
        Some(match confirm {
            StoreConfirm::Join(preview) => {
                self.join_link.clear();
                Task::JoinStore { link: preview.link }
            }
            StoreConfirm::Leave { store, .. } => Task::LeaveStore { store },
            StoreConfirm::RemoveMember { store, member, .. } => {
                Task::RemoveMember { store, member }
            }
            StoreConfirm::CancelInvite { store, invite, .. } => {
                Task::CancelInvite { store, invite }
            }
            StoreConfirm::Unpublish { store, plugin, .. } => {
                Task::UnpublishPlugin { store, plugin }
            }
        })
    }

    /// Enter on the members screen: remove a member, or cancel an unused invite.
    fn open_member_row(&mut self, store: &str) -> Option<Task> {
        let at = self.member_selected;
        if let Some(member) = self.store_members.get(at) {
            if member.you {
                self.push_toast(Toast::info("that is you — a store always keeps its owner"));
                return None;
            }
            self.store_confirm = Some(StoreConfirm::RemoveMember {
                store: store.to_owned(),
                member: member.id.clone(),
                name: member.name.clone(),
                devices: member.devices,
            });
        } else {
            let invite = self.store_invites.get(at - self.store_members.len())?;
            self.store_confirm = Some(StoreConfirm::CancelInvite {
                store: store.to_owned(),
                invite: invite.id.clone(),
                name: invite.name.clone(),
            });
        }
        self.nav.open_overlay(Overlay::StoreConfirm);
        None
    }

    /// Feed pasted text to whichever field is active. Control characters are dropped, so the
    /// newline a copy brings along does not arrive as Enter halfway through a link.
    pub fn paste(&mut self, text: &str) {
        if !self.is_typing() {
            return;
        }
        for ch in text.chars().filter(|c| !c.is_control()) {
            self.type_char(ch);
        }
    }
}

/// Server-side search filters, set from the FILTERS overlay on the Search screen.
///
/// Wire values rather than display values — what AniList's enums expect, lowercased only
/// when a row is drawn. `None` reads as "any" and is omitted from the query entirely,
/// because AniList treats an explicit null differently from a missing argument.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchFilters {
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub season: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    /// `None` is relevance — AniList's `SEARCH_MATCH` — which is only meaningful with a
    /// search term, so the executor substitutes popularity for a filter-only browse.
    pub sort: Option<String>,
}

impl SearchFilters {
    /// Row labels, in overlay order. The cycling and display functions index against this.
    pub const ROWS: [&'static str; 6] = ["genre", "year", "season", "format", "status", "sort"];

    /// AniList's fixed genre set. Genres are strings on the wire, but the set is closed —
    /// typing one would invite misses on casing that a cycle cannot produce.
    const GENRES: [&'static str; 18] = [
        "Action",
        "Adventure",
        "Comedy",
        "Drama",
        "Ecchi",
        "Fantasy",
        "Horror",
        "Mahou Shoujo",
        "Mecha",
        "Music",
        "Mystery",
        "Psychological",
        "Romance",
        "Sci-Fi",
        "Slice of Life",
        "Sports",
        "Supernatural",
        "Thriller",
    ];
    const SEASONS: [(&'static str, &'static str); 4] =
        [("WINTER", "winter"), ("SPRING", "spring"), ("SUMMER", "summer"), ("FALL", "fall")];
    const FORMATS: [(&'static str, &'static str); 7] = [
        ("TV", "tv"),
        // Short-form TV — a few minutes an episode — is its own format on AniList, so a
        // filter without it had no way to find one.
        ("TV_SHORT", "tv short"),
        ("MOVIE", "movie"),
        ("OVA", "ova"),
        ("ONA", "ona"),
        ("SPECIAL", "special"),
        ("MUSIC", "music"),
    ];
    const STATUSES: [(&'static str, &'static str); 3] =
        [("RELEASING", "airing"), ("FINISHED", "finished"), ("NOT_YET_RELEASED", "upcoming")];
    const SORTS: [(&'static str, &'static str); 4] = [
        ("POPULARITY_DESC", "popularity"),
        ("SCORE_DESC", "score"),
        ("TRENDING_DESC", "trending"),
        ("START_DATE_DESC", "newest"),
    ];
    /// Older than any TV anime that streams anywhere; the year cycle stops here.
    const OLDEST_YEAR: u32 = 1970;

    pub fn is_empty(&self) -> bool {
        self.genre.is_none()
            && self.year.is_none()
            && self.season.is_none()
            && self.format.is_none()
            && self.status.is_none()
            && self.sort.is_none()
    }

    /// What a row currently shows: the display form, or "any" when unset.
    pub fn row_value(&self, row: usize) -> String {
        let shown = |set: &[(&str, &str)], value: &Option<String>| {
            value.as_deref().and_then(|v| {
                set.iter().find(|(wire, _)| *wire == v).map(|(_, word)| (*word).to_string())
            })
        };
        match row {
            // A genre is a name, and keeps the casing it has everywhere else it is shown.
            0 => self.genre.clone(),
            1 => self.year.map(|y| y.to_string()),
            2 => shown(&Self::SEASONS, &self.season),
            3 => shown(&Self::FORMATS, &self.format),
            4 => shown(&Self::STATUSES, &self.status),
            5 => return shown(&Self::SORTS, &self.sort).unwrap_or_else(|| "relevance".into()),
            _ => None,
        }
        .unwrap_or_else(|| "any".into())
    }

    /// The set values joined for the search prompt line, empty when nothing is.
    pub fn summary(&self) -> String {
        (0..Self::ROWS.len())
            .filter(|row| {
                // "relevance" is the resting state of sort, not a choice worth announcing.
                match row {
                    0 => self.genre.is_some(),
                    1 => self.year.is_some(),
                    2 => self.season.is_some(),
                    3 => self.format.is_some(),
                    4 => self.status.is_some(),
                    _ => self.sort.is_some(),
                }
            })
            .map(|row| self.row_value(row))
            .collect::<Vec<_>>()
            .join(" · ")
    }

    /// Step a row's value. Forward walks the options and comes back around to "any",
    /// so both directions reach everything and nothing is a dead end.
    pub fn cycle(&mut self, row: usize, forward: bool) {
        fn step(current: &mut Option<String>, wires: &[&str], forward: bool) {
            let at = current.as_deref().and_then(|c| wires.iter().position(|w| *w == c));
            let next = match (at, forward) {
                (None, true) => Some(0),
                (None, false) => Some(wires.len() - 1),
                (Some(i), true) => (i + 1 < wires.len()).then_some(i + 1),
                (Some(0), false) => None,
                (Some(i), false) => Some(i - 1),
            };
            *current = next.map(|i| wires[i].to_owned());
        }
        match row {
            0 => step(&mut self.genre, &Self::GENRES, forward),
            1 => {
                let latest = chrono::Datelike::year(&chrono::Local::now()).max(0) as u32 + 1;
                self.year = match (self.year, forward) {
                    (None, true) => Some(latest),
                    (None, false) => Some(Self::OLDEST_YEAR),
                    (Some(y), true) => (y > Self::OLDEST_YEAR).then(|| y - 1),
                    (Some(y), false) => (y < latest).then(|| y + 1),
                };
            }
            2 => step(&mut self.season, &Self::SEASONS.map(|(wire, _)| wire), forward),
            3 => step(&mut self.format, &Self::FORMATS.map(|(wire, _)| wire), forward),
            4 => step(&mut self.status, &Self::STATUSES.map(|(wire, _)| wire), forward),
            _ => step(&mut self.sort, &Self::SORTS.map(|(wire, _)| wire), forward),
        }
    }
}

/// A credential in transit.
///
/// Exists for exactly one reason: `Task` derives `Debug` and the task router logs a task
/// when routing goes wrong. A plain `String` there would put a password in the log file;
/// this prints a placeholder instead, everywhere, unconditionally.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(«redacted»)")
    }
}

/// Identifies one [`Job`], so the work that finishes it can say which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(u64);

/// Something the user started and is waiting on, shown in the activity row until it ends.
///
/// The status line could only hold one sentence, written when work began and cleared — when it
/// was cleared at all — by whichever path remembered to. So a message outlived its work, and two
/// pieces of work overwrote each other. A job instead ends when the work that carries it ends,
/// however it ends, and every running one is listed with how long it has taken.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub label: String,
    pub started: Instant,
    /// What the work is held up on, when that is not the work itself — a rate-limit pause —
    /// and until when, so the row can count it down rather than state a figure that goes stale.
    pub note: Option<(String, Option<Instant>)>,
}

/// How long work runs before it earns a place in the activity row. Most loads answer well
/// inside this, and a row that blinked in and out on every keypress would shift the screen for
/// nothing; past it, the wait is long enough to wonder about.
pub const ACTIVITY_DELAY: Duration = Duration::from_millis(600);

impl Task {
    /// What to call this in the activity row, or `None` for work nobody waits on: writes that
    /// land in the outbox, player commands, and anything that finishes as it is asked.
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::LoadContinue => "loading continue watching".into(),
            Self::LoadSeasonal => "loading this season".into(),
            Self::LoadCalendar => "loading the calendar".into(),
            Self::LoadCalendarPast { .. } => "loading earlier broadcasts".into(),
            Self::Search { query, .. } if query.is_empty() => "browsing".into(),
            Self::Search { query, .. } => format!("searching \u{201c}{query}\u{201d}"),
            Self::LoadDetail(_) => "loading the title".into(),
            Self::LoadEpisodes(_) => "loading episodes".into(),
            Self::ClearMatch { .. } | Self::FixMatch { .. } => "rematching sources".into(),
            Self::CheckProviders => "checking providers".into(),
            Self::LoadLibrary(segment) => format!("loading your {} list", segment.label()),
            Self::SyncNow => "syncing".into(),
            Self::Connect { tracker } => format!("signing in to {tracker}"),
            Self::ConnectKitsu { .. } => "signing in to kitsu".into(),
            Self::Play { episode, .. } | Self::PlaySource { episode, .. } => {
                format!("starting ep {episode}")
            }
            Self::PlayParty { episode, .. } => format!("starting ep {episode} for the party"),
            Self::LoadSources { episode, .. } => format!("listing sources for ep {episode}"),
            Self::ManualSearch { query, .. } => {
                format!("searching sources for \u{201c}{query}\u{201d}")
            }
            Self::DownloadEpisode { episode, .. } => format!("queueing ep {episode}"),
            Self::DownloadMany { episodes, .. } => {
                format!("queueing {}", crate::widgets::plural(episodes.len(), "episode"))
            }
            Self::PlayTrailer { .. } => "opening the trailer".into(),
            Self::LoadStores => "checking plugin stores".into(),
            Self::PreviewStore { .. } => "checking the invite".into(),
            Self::JoinStore { .. } => "joining the store".into(),
            Self::LeaveStore { .. } => "leaving the store".into(),
            Self::InstallPlugin { plugin, .. } => format!("installing {plugin}"),
            Self::CreateInvite { .. } => "making an invite".into(),
            Self::LoadMembers { .. } => "loading members".into(),
            Self::RemoveMember { .. } => "removing access".into(),
            Self::LoadLocalPlugins { .. } => "reading local plugins".into(),
            Self::PublishPlugin { plugin, .. } => format!("publishing {plugin}"),
            Self::UnpublishPlugin { plugin, .. } => format!("withdrawing {plugin}"),
            Self::OpenAdmin { .. } => "opening the admin page".into(),
            _ => return None,
        })
    }
}

/// Asynchronous work requested by the reducer.
#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    /// Resolve an episode's streams ahead of playing it, quietly, for a [`Task::Play`] of the
    /// same episode to pick up.
    Prefetch {
        id: AnilistId,
        episode: String,
    },
    /// What you were watching, from local history.
    ///
    /// Named for what it is. It used to be `LoadTrending` and it fetched the current season, which
    /// made the section labelled CONTINUE a discovery screen — so the one thing it was named after,
    /// an episode left half-finished, appeared nowhere. Trending and continuing are different
    /// questions and the app has separate sections for each; substituting one for the other quietly
    /// is what made a real feature invisible.
    LoadContinue,
    LoadSeasonal,
    LoadCalendar,
    /// One page further back in the calendar, asked for because the user scrolled near the top.
    ///
    /// Carries the anchor the screen was loaded against rather than letting the fetch read the
    /// clock again. Page numbers only mean anything relative to a fixed upper bound: re-reading
    /// `now` between pages would shift every boundary and let a broadcast appear twice.
    LoadCalendarPast {
        anchor: i64,
        page: u32,
    },
    Search {
        query: String,
        filter: SearchFilters,
    },
    LoadDetail(AnilistId),
    LoadEpisodes(AnilistId),
    CheckProviders,
    /// Fetch one segment of the tracker's list.
    LoadLibrary(LibrarySegment),
    /// Drain the outbox and pull the library now, rather than waiting for the interval.
    SyncNow,
    /// Set a title's list status.
    SetStatus {
        id: AnilistId,
        status: LibrarySegment,
    },
    /// Score a title out of ten.
    SetScore {
        id: AnilistId,
        score: f32,
    },
    /// Play a title's trailer: mpv when it can stream the host, the browser otherwise.
    PlayTrailer {
        url: String,
        title: String,
    },
    /// Pin a source for one title, or return it to automatic with `None`.
    SetProviderPreference {
        id: AnilistId,
        provider: Option<String>,
    },
    /// Settle a divergence by keeping one side.
    ResolveConflict {
        id: AnilistId,
        keep_local: bool,
    },
    /// Run a tracker's sign-in flow.
    Connect {
        tracker: String,
    },
    /// Exchange Kitsu credentials for a token, from the in-app prompt.
    ConnectKitsu {
        username: String,
        password: Secret,
    },
    /// Forget a tracker's token.
    Disconnect {
        tracker: String,
    },
    /// Resolve a stream and play it. The eyecatch covers this.
    Play {
        id: AnilistId,
        episode: String,
    },
    /// Resolve an episode and hand it to a Syncplay party instead of a private session.
    PlayParty {
        id: AnilistId,
        episode: String,
    },
    /// List the selectable releases for one episode, for the Sources overlay.
    LoadSources {
        id: AnilistId,
        episode: String,
    },
    /// Search providers with the user's own words when the automatic match is wrong.
    /// Results come back as [`Update::MatchChoices`], reusing the Disambiguate flow.
    ManualSearch {
        id: AnilistId,
        query: String,
    },
    /// Forget the pinned match and cached resolution for a title, then re-resolve
    /// automatically — the undo for [`Task::FixMatch`].
    ClearMatch {
        id: AnilistId,
    },
    /// Persist a manual watched/unwatched change. The reducer flips the rows first, so
    /// the table answers immediately; this writes history and queues the tracker push.
    SetWatched {
        id: AnilistId,
        episodes: Vec<String>,
        watched: bool,
    },
    /// Take a title off the CONTINUE rail. The reducer drops the row first, so the list
    /// answers immediately; this persists the dismissal. History is untouched, and the
    /// next recorded watch brings the title back.
    HideFromContinue {
        id: AnilistId,
    },
    /// Open a URL in the system browser.
    OpenExternal {
        url: String,
    },
    /// Play the exact release the user picked from the Sources overlay. The eyecatch
    /// covers this the same way it covers [`Task::Play`].
    PlaySource {
        id: AnilistId,
        episode: String,
        provider_id: String,
        source_id: String,
    },
    /// Something for the live mpv session.
    Player(PlayerCommand),
    /// Queue an episode for offline download.
    /// Pin a title to a provider result the user picked, then load its episodes.
    FixMatch {
        id: AnilistId,
        provider_id: String,
        key: anistream_core::ids::ProviderKey,
    },
    DownloadEpisode {
        id: AnilistId,
        episode: String,
    },
    /// Queue a whole range at once. The queue's own concurrency limit paces the fetches.
    DownloadMany {
        id: AnilistId,
        episodes: Vec<String>,
    },
    /// Pause or resume one download, whichever it is not.
    DownloadPause {
        id: i64,
    },
    /// Remove a download from the queue, stopping it if it is running.
    DownloadCancel {
        id: i64,
    },
    /// Remove a download *and* delete its file from disk. Cancel keeps a finished file;
    /// this is the explicit way to not keep it.
    DownloadDelete {
        id: i64,
    },
    DownloadClearCompleted,
    /// Read the queue and publish it — for opening the screen.
    LoadDownloads,
    /// Play a file already on disk — with the identity that lets it record history,
    /// resume, and sync exactly as a streamed episode would.
    PlayLocal {
        id: AnilistId,
        episode: String,
        title: String,
        path: String,
    },
    /// Persist one changed setting to `config.toml`.
    ///
    /// The reducer changes `self.config` immediately so the screen responds at once, and the file
    /// write goes out as a task — it touches the disk, and a reducer that did IO could not be
    /// tested without one.
    SaveSetting {
        table: &'static [&'static str],
        key: &'static str,
        value: anistream_core::settings::SettingValue,
    },
    /// Read the joined stores, then refresh each one's plugin list.
    LoadStores,
    /// Check a pasted invite link against the store it names, before spending it.
    PreviewStore {
        link: Secret,
    },
    /// Spend an invite link for a token and join.
    JoinStore {
        link: Secret,
    },
    /// Forget a store and revoke this device's token there.
    LeaveStore {
        store: String,
    },
    /// Install a store's plugin, or update it.
    InstallPlugin {
        store: String,
        plugin: String,
    },
    /// Delete an installed plugin from this machine.
    RemovePlugin {
        plugin: String,
    },
    /// Make an invite link: for someone new with a name, for another device of yours without.
    CreateInvite {
        store: String,
        name: Option<String>,
    },
    LoadMembers {
        store: String,
    },
    RemoveMember {
        store: String,
        member: String,
    },
    CancelInvite {
        store: String,
        invite: String,
    },
    /// List local plugins an owner could publish.
    LoadLocalPlugins {
        store: String,
    },
    PublishPlugin {
        store: String,
        plugin: String,
    },
    UnpublishPlugin {
        store: String,
        plugin: String,
    },
    /// Ask a store for a one-time sign-in link and open its admin page in the browser.
    OpenAdmin {
        store: String,
    },
}

/// A control sent to whatever is currently playing.
///
/// Kept as data rather than a call into the player so the reducer stays pure and testable —
/// the binary routes these to the live session.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerCommand {
    TogglePause,
    /// Seek by a relative number of seconds; negative goes back.
    Seek(f64),
    /// Seek to an absolute position — how the skip prompt is taken.
    SeekTo(f64),
    /// Multiply the current speed, clamped by the player.
    Speed(f64),
    /// Step volume by a relative amount.
    Volume(f64),
    /// Leave mpv running and return to browsing.
    Detach,
    Stop,
    /// The title's episode list, loaded after playback started because the episode table on
    /// screen was not this title's — what the player's Episodes menu and Up Next read. `next`
    /// is the episode that follows the one playing, when the list has it.
    Episodes {
        rows: Vec<EpisodeRow>,
        next: Option<String>,
    },
    /// A newer playback is starting: hand the player window over to it rather than closing it.
    ///
    /// Sent by the task runner, never by a key — it is how pressing N inside the player, or
    /// picking an episode from its menu, keeps the same window instead of opening a second one.
    /// It names what is coming, so the window can say so while that is found.
    Handover(Successor),
    /// Step to the next (`1`) or previous (`-1`) episode of what is playing: the terminal's n
    /// and N. Answered by the playback, which holds the playing title's episode list.
    Step(i64),
    /// Toggle the player's fullscreen state.
    Fullscreen,
}

/// What a newer playback is about to play, as the window it takes over announces it.
#[derive(Debug, Clone, PartialEq)]
pub struct Successor {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    pub translation: anistream_core::media::Translation,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::new(Config::default(), Palette::dark(), Keymap::new())
    }

    fn entries(n: usize) -> Content {
        Content::Entries(
            (0..n)
                .map(|i| Entry::new(AnilistId::new(i as u32 + 1), format!("Title {i}")))
                .collect(),
        )
    }

    /// The anchor a test calendar is paged against. Any fixed instant will do — the point is
    /// that it does not move between pages.
    const CALENDAR_TEST_ANCHOR: i64 = 1_700_000_000;

    /// A calendar on screen with three pages of history behind it and more to be had.
    fn calendar_loaded() -> App {
        let mut a = app();
        let _ = a.go_to_section(Section::Calendar);
        a.nav.focus_stage();
        a.apply(Update::CalendarWindow {
            anchor: CALENDAR_TEST_ANCHOR,
            pages: 3,
            more_past: true,
        });
        a.apply(a.content_here(entries(150)));
        a
    }

    #[test]
    fn scrolling_into_the_calendars_past_fetches_more_of_it() {
        let mut a = calendar_loaded();

        // Deep in the list there is nothing to do — every row above is already loaded.
        a.selected = 80;
        a.offset = 61;
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            None,
            "a scroll through rows already loaded must not spend a request"
        );

        // Within a screenful of the top, the page behind is asked for — early enough that
        // there are still rows left to scroll through while it arrives.
        a.selected = CALENDAR_BACKFILL_MARGIN + 1;
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            Some(Task::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }),
            "nearing the top should reach for the page behind the ones loaded"
        );

        // And exactly once. A held arrow key asking per keystroke would empty a
        // 30-per-minute budget answering one question.
        for _ in 0..5 {
            a.handle(Action::Up, 20);
            assert_eq!(a.take_pending(), None, "a held arrow must not re-request page four");
        }
    }

    #[test]
    fn backfilled_broadcasts_land_above_the_viewport_without_moving_it() {
        let mut a = calendar_loaded();
        a.selected = 8;
        a.offset = 4;

        let older: Vec<Entry> = (0..50)
            .map(|i| Entry::new(AnilistId::new(900 + i), format!("Older {i}")))
            .collect();
        a.apply(Update::CalendarBackfill { page: Some(4), entries: older, more_past: true });

        assert_eq!(a.content.len(), 200, "the older page should have extended the timeline");
        assert_eq!(
            a.content.entries()[0].title,
            "Older 0",
            "older broadcasts belong above — the list reads downward through time"
        );
        // The whole point. Rows arrived above the viewport, so the row under the cursor and
        // its position on screen are both exactly where the user left them.
        assert_eq!(a.selected, 58, "the selection must still be on the same broadcast");
        assert_eq!(a.offset, 54, "and that broadcast must still be in the same place");
        assert_eq!(a.content.entries()[a.selected].title, "Title 8");
    }

    #[test]
    fn a_backfill_that_arrives_after_the_user_has_left_is_dropped() {
        let mut a = calendar_loaded();
        let _ = a.go_to_section(Section::Seasonal);
        a.apply(a.content_here(entries(20)));

        a.apply(Update::CalendarBackfill {
            page: Some(4),
            entries: vec![Entry::new(AnilistId::new(999), "Older")],
            more_past: true,
        });

        assert_eq!(
            a.content.len(),
            20,
            "a late backfill must not splice broadcasts into whatever screen is up now"
        );
    }

    #[test]
    fn a_failed_backfill_leaves_its_page_to_be_tried_again() {
        let mut a = calendar_loaded();
        a.selected = CALENDAR_BACKFILL_MARGIN + 1;
        a.handle(Action::Up, 20);
        assert!(a.take_pending().is_some(), "the scroll should have asked for page four");

        a.apply(Update::CalendarBackfill { page: None, entries: Vec::new(), more_past: true });
        assert_eq!(a.content.len(), 150, "a failed backfill must not disturb the list");

        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            Some(Task::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }),
            "the page that failed is the page still missing"
        );
    }

    #[test]
    fn the_calendar_stops_asking_once_its_history_runs_out() {
        let mut a = calendar_loaded();
        a.apply(Update::CalendarBackfill {
            page: Some(4),
            entries: Vec::new(),
            more_past: false,
        });

        a.selected = 2;
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            None,
            "AniList said there is nothing older; asking again is a wasted request"
        );
    }

    #[test]
    fn leaving_the_calendar_forgets_where_its_pages_were() {
        let mut a = calendar_loaded();
        let _ = a.go_to_section(Section::Seasonal);
        let _ = a.go_to_section(Section::Calendar);
        a.nav.focus_stage();
        a.apply(a.content_here(entries(150)));

        a.selected = 2;
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            None,
            "page four of the previous timeline is not worth a request against the new one"
        );
    }

    /// An app with one title selected and its episode table loaded, ready to play.
    /// The end of an episode mpv played to its end, naming the episode the player kept its
    /// window for when it rolls on.
    fn finished(next: Option<&str>) -> Update {
        Update::PlaybackEnded {
            watched: true,
            ending: Ending::Complete { next: next.map(Into::into) },
        }
    }

    fn app_at_episodes() -> App {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        // The title first: a table is only taken for the title it is a view of.
        a.detail = a.content.entries().first().cloned();
        a.apply(
            a.episodes_here(
                (1..=12)
                    .map(|n| EpisodeRow {
                        number: n.to_string(),
                        title: Some(format!("Episode {n}")),
                        duration_secs: Some(1435),
                        watched: 0.0,
                        completed: false,
                        kind: None,
                        skippable: false,
                        thumbnail: None,
                        description: None,
                    })
                    .collect(),
            ),
        );
        a.nav.push(StageView::Episodes(AnilistId::new(1)));
        a
    }

    #[test]
    fn enter_in_the_episode_table_plays_rather_than_opening() {
        // Enter opens a title everywhere else, so this is the one place it has to mean
        // something different — the table *is* the list of things to play.
        let mut a = app_at_episodes();
        a.episode_selected = 4;
        let task = a.handle(Action::Open, 20);
        assert_eq!(
            task,
            Some(Task::Play { id: AnilistId::new(1), episode: "5".into() }),
            "Enter must play the selected episode"
        );
        assert!(matches!(a.nav.current(), StageView::NowPlaying));
        assert_eq!(a.playing.as_ref().map(|p| p.episode.clone()), Some("5".into()));
    }

    fn candidate(title: &str, similarity: f64) -> MatchCandidate {
        MatchCandidate {
            title: title.into(),
            key: anistream_core::ids::ProviderKey::new(title),
            similarity,
            rejected: None,
        }
    }

    fn source(title: &str, auto: bool) -> SourceCandidate {
        SourceCandidate {
            id: format!("https://indexer.example/view/{title}"),
            provider_id: "torrent".into(),
            title: title.into(),
            quality: Some(1080),
            seeders: Some(120),
            size: Some("1.4 GiB".into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: auto,
        }
    }

    #[test]
    fn asking_for_sources_opens_the_slate_and_a_pick_plays_it() {
        let mut a = app_at_episodes();
        let task = a.handle(Action::ShowSources, 20).expect("asking must produce a task");
        assert!(matches!(task, Task::LoadSources { .. }), "got {task:?}");

        a.apply(a.sources_here(vec![
            source("[A] Show - 1 (1080p)", true),
            source("[B] Show - 1 (1080p)", false),
        ]));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Sources));

        // Move to the second release and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must play");
        match task {
            Task::PlaySource { provider_id, source_id, .. } => {
                assert_eq!(provider_id, "torrent");
                assert!(source_id.contains("[B]"), "the pick must be the selected row");
            }
            other => panic!("expected PlaySource, got {other:?}"),
        }
        // The question is put away and playback staging is up, exactly as Enter on an
        // episode row would have it.
        assert_eq!(a.nav.overlay(), None);
        assert!(a.playing.is_some(), "a pick must stage Now Playing");
        assert!(a.sources.is_empty());
    }

    #[test]
    fn an_empty_slate_answers_with_a_toast_rather_than_an_empty_overlay() {
        let mut a = app_at_episodes();
        a.handle(Action::ShowSources, 20);
        a.apply(a.sources_here(Vec::new()));
        assert_ne!(a.nav.overlay(), Some(&Overlay::Sources));
        assert!(a.source_context.is_none(), "an answered question is not left pending");
    }

    #[test]
    fn toggling_watched_flips_the_row_and_persists() {
        let mut a = app_at_episodes();
        let task = a.handle(Action::ToggleWatched, 20).expect("toggling must persist");
        assert!(a.episodes[0].completed, "the table answers immediately");
        assert!(matches!(task, Task::SetWatched { watched: true, .. }), "got {task:?}");

        let task = a.handle(Action::ToggleWatched, 20).expect("toggling back must too");
        assert!(!a.episodes[0].completed);
        assert!(matches!(task, Task::SetWatched { watched: false, .. }), "got {task:?}");
    }

    #[test]
    fn a_watched_mark_moves_the_title_and_its_list_row_too() {
        // Two watched, three next, on the page and on the list under it.
        let mut a = app_at_episodes();
        for n in ["1", "2"] {
            a.set_episode_watched(n, true);
        }
        let at = |a: &App| {
            let row = a.content.entries()[0].progress;
            (a.detail.as_ref().and_then(|d| d.progress), row)
        };
        a.detail.as_mut().expect("detail").progress = Some((2, 3));
        if let Content::Entries(rows) = &mut a.content {
            rows[0].progress = Some((2, 3));
        }

        a.episode_selected = 2;
        a.handle(Action::ToggleWatched, 20);
        assert_eq!(at(&a), (Some((3, 4)), Some((3, 4))), "marking ep 3 moves both on");
        a.handle(Action::ToggleWatched, 20);
        assert_eq!(at(&a), (Some((2, 3)), Some((2, 3))), "unmarking puts both back");

        a.episode_selected = 5;
        a.handle(Action::MarkAllPrevious, 20);
        assert_eq!(at(&a), (Some((5, 6)), Some((5, 6))), "M catches both up to the cursor");
    }

    #[test]
    fn marking_previous_marks_only_the_unwatched_before_the_cursor() {
        let mut a = app_at_episodes();
        a.episode_selected = 4;
        // Already done — must not be re-marked. Both lists, since the visible one is
        // re-derived from the full one whenever marks change.
        a.episodes[1].completed = true;
        a.episodes_all[1].completed = true;

        let task = a.handle(Action::MarkAllPrevious, 20).expect("marking must persist");
        match task {
            Task::SetWatched { episodes, watched: true, .. } => {
                assert_eq!(episodes, vec!["1".to_string(), "3".into(), "4".into()]);
            }
            other => panic!("expected SetWatched, got {other:?}"),
        }
        assert!(a.episodes[..4].iter().all(|r| r.completed));
        assert!(!a.episodes[4].completed, "the cursor row itself stays untouched");
    }

    #[test]
    fn a_text_setting_is_typed_not_pointed_at_config_toml() {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected =
            SettingId::ALL.iter().position(|s| *s == SettingId::Subtitles).expect("row");

        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::EditSetting));
        assert_eq!(a.edit_value, "eng", "prefilled with the live value");

        for _ in 0..3 {
            a.backspace();
        }
        for c in "spa".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("enter must save");
        assert!(
            matches!(task, Task::SaveSetting { key: "subtitle_language", .. }),
            "got {task:?}"
        );
        assert_eq!(a.config.playback.subtitle_language, "spa");
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn enter_on_a_resumable_continue_row_plays_immediately() {
        let mut a = app();
        let mut e = Entry::new(AnilistId::new(5), "Frieren");
        e.progress = Some((7, 8));
        e.resume = Some(ResumePoint { position: 600.0, fraction: Some(0.5) });
        a.apply(a.content_here(Content::Entries(vec![e])));
        a.nav.focus_stage();

        let task = a.handle(Action::Open, 20).expect("enter must resume");
        assert_eq!(
            task,
            Task::Play { id: AnilistId::new(5), episode: "8".into() },
            "the section is named Continue; Enter continues"
        );
        assert!(a.playing.is_some());
    }

    #[test]
    fn a_typed_range_queues_the_episodes_that_exist() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::DownloadRange));
        for c in "9-".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("a range must queue");
        match task {
            Task::DownloadMany { episodes, .. } => {
                assert_eq!(episodes, vec!["9", "10", "11", "12"], "open-ended from 9");
            }
            other => panic!("expected DownloadMany, got {other:?}"),
        }
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn a_count_of_one_is_said_in_the_singular() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        a.type_char('4');
        let task = a.handle(Action::Open, 20).expect("one episode is a range too");
        assert!(a.toasts.iter().any(|t| t.text == "queueing 1 episode…"), "{:?}", a.toasts);
        assert_eq!(task.activity().as_deref(), Some("queueing 1 episode"));
    }

    #[test]
    fn mark_all_previous_says_which_nothing_it_found() {
        let mut a = app_at_episodes();
        a.handle(Action::MarkAllPrevious, 20);
        assert!(a.toasts.iter().any(|t| t.text == "nothing comes before the first episode"));

        a.episode_selected = 3;
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.text == "marked 3 episodes watched"),
            "{:?}",
            a.toasts
        );
        a.handle(Action::MarkAllPrevious, 20);
        assert!(a.toasts.iter().any(|t| t.text == "everything before ep 4 is already watched"));

        // An empty table has no episode to be before.
        a.episodes.clear();
        a.episode_selected = 0;
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.text == "no episodes listed to mark"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_list_status_is_confirmed_as_the_list_it_is_on() {
        let mut a = app();
        // List status lives on a tracker, so one is signed in.
        a.apply(Update::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(1)));
        a.handle(Action::SetListStatus, 10);
        a.overlay_selected = 2;
        let task = a.handle(Action::Open, 10);
        assert!(matches!(
            task,
            Some(Task::SetStatus { status: LibrarySegment::Completed, .. })
        ));
        assert!(
            a.toasts
                .iter()
                .any(|t| t.text == "now on your completed list — queued for anilist"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_saved_setting_is_named_as_the_screen_names_it() {
        let a = app();
        assert_eq!(
            a.setting_label(&["playback"], "commit_threshold"),
            Some("counts as watched at"),
            "the screen's words, not the file's key"
        );
        assert_eq!(
            a.setting_label(&["providers", "torrent"], "enabled"),
            Some("torrent source")
        );
        // Kept in passing, with no row of its own: nothing to announce.
        assert_eq!(a.setting_label(&["playback"], "persisted_speed"), None);
    }

    #[test]
    fn a_typed_setting_is_confirmed_once_by_the_write_not_the_keypress() {
        let mut a = app();
        let _ = a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected = SettingId::ALL.iter().position(|id| *id == SettingId::Subtitles).unwrap();
        a.handle(Action::Open, 10);
        for c in "ger".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Task::SaveSetting { key: "subtitle_language", .. })));
        assert!(a.toasts.is_empty(), "the save says so when it lands: {:?}", a.toasts);
    }

    #[test]
    fn the_format_filter_can_find_short_tv() {
        let mut filter = SearchFilters::default();
        filter.cycle(3, true);
        assert_eq!(filter.row_value(3), "tv");
        filter.cycle(3, true);
        assert_eq!(filter.format.as_deref(), Some("TV_SHORT"), "AniList's name for it");
        assert_eq!(filter.row_value(3), "tv short");
    }

    #[test]
    fn a_genre_filter_keeps_the_genre_s_own_casing() {
        let filter =
            SearchFilters { genre: Some("Slice of Life".into()), ..Default::default() };
        assert_eq!(filter.row_value(0), "Slice of Life");
    }

    #[test]
    fn the_source_named_is_one_that_is_not_switched_off() {
        // The default order leads with torrents, which are off by default.
        let mut a = app();
        assert_eq!(a.source_label(), "plugins");
        assert_eq!(
            a.source_states(),
            vec![
                ("torrent".into(), SourceState::Off),
                ("plugins".into(), SourceState::Pending)
            ]
        );
        a.apply(Update::Providers(vec![ProviderRow {
            id: "someplugin".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        }]));
        assert_eq!(a.source_states()[1], ("plugins".into(), SourceState::Ready));
        // What actually served wins once something has played.
        a.apply(Update::ActiveProvider("someplugin".into()));
        assert_eq!(a.source_label(), "someplugin");
    }

    #[test]
    fn a_nonsense_range_is_refused_with_a_hint() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        for c in "12-4".chars() {
            a.type_char(c);
        }
        assert!(a.handle(Action::Open, 20).is_none(), "a backwards range queues nothing");
        assert!(a.toasts.iter().any(|t| t.text.contains("1-12")), "the hint names the shape");
    }

    #[test]
    fn the_filter_narrows_the_table_and_cycles_back_to_all() {
        let mut a = app_at_episodes();
        for i in [0, 1, 2] {
            a.episodes[i].completed = true;
            a.episodes_all[i].completed = true;
        }
        a.handle(Action::Filter, 20);
        assert_eq!(a.episode_filter, EpisodeFilter::Unwatched);
        assert_eq!(a.episodes.len(), 9, "three watched episodes leave the view");

        // Marking one under the unwatched filter removes it at once.
        a.handle(Action::ToggleWatched, 20);
        assert_eq!(a.episodes.len(), 8);

        a.handle(Action::Filter, 20);
        a.handle(Action::Filter, 20);
        assert_eq!(a.episode_filter, EpisodeFilter::All);
        assert_eq!(a.episodes.len(), 12, "cycling back restores every row");
    }

    #[test]
    fn a_wrong_match_can_be_re_searched_by_hand() {
        let mut a = app_at_episodes();
        a.handle(Action::FixMapping, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ManualQuery));
        assert!(a.is_typing(), "the overlay must take text");

        for c in "frieren".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("enter must search");
        match task {
            Task::ManualSearch { query, .. } => assert_eq!(query, "frieren"),
            other => panic!("expected ManualSearch, got {other:?}"),
        }
        // The overlay closes; the results return through MatchChoices and reuse the
        // Disambiguate flow, which has its own tests.
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn a_source_slate_nobody_asked_for_stays_quiet() {
        // A stale answer arriving after the user navigated away must not interrupt.
        let mut a = app_at_episodes();
        a.apply(a.sources_here(vec![source("[A] Show - 1 (1080p)", true)]));
        assert_ne!(a.nav.overlay(), Some(&Overlay::Sources));
    }

    #[test]
    fn an_undecidable_match_asks_rather_than_failing() {
        // The candidates were being thrown away and reported as "could not match this title",
        // which is a dead end for a question the user can answer at a glance.
        let mut a = app_at_episodes();
        a.apply(Update::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Frieren S1", 0.62), candidate("Frieren S2", 0.61)],
        });

        assert_eq!(a.nav.overlay(), Some(&Overlay::Disambiguate));
        assert_eq!(a.match_candidates.len(), 2);
        assert!(!a.episodes_loading, "the wait is over; a question is not a wait");
    }

    #[test]
    fn no_candidates_is_a_failure_rather_than_an_empty_question() {
        let mut a = app_at_episodes();
        a.apply(Update::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: Vec::new(),
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
    }

    #[test]
    fn picking_a_candidate_pins_it_and_reloads() {
        let mut a = app_at_episodes();
        a.apply(Update::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Frieren S1", 0.62), candidate("Frieren S2", 0.61)],
        });

        // Move to the second candidate and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must do something");
        assert_eq!(
            task,
            Task::FixMatch {
                id: AnilistId::new(1),
                provider_id: "torrent".into(),
                key: anistream_core::ids::ProviderKey::new("Frieren S2"),
            }
        );

        // And the question is put away rather than left hanging behind the reload.
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
        assert!(a.match_candidates.is_empty());
        assert!(a.episodes_loading, "the answer starts a load");
    }

    #[test]
    fn the_eyecatch_goes_up_before_resolution_starts() {
        // The wipe exists to cover resolution. Raising it after the provider walk finished
        // would put the motion in the one place it is not needed.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        let eyecatch = a.eyecatch.as_ref().expect("no eyecatch was raised");
        assert_eq!(eyecatch.stage(), crate::eyecatch::Stage::Covering);
        assert!(a.is_animating(), "the loop has to know to tick faster");
    }

    #[test]
    fn a_failure_releases_the_eyecatch_rather_than_holding_amber() {
        // Held forever, the band would leave the user staring at a solid amber screen with no
        // way to know the stream failed.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::Toast(Toast::alert("all providers failed")));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES * 3 {
            a.tick_animation();
        }
        assert!(a.eyecatch.is_none(), "the wipe never finished");
    }

    #[test]
    fn a_wipe_that_reaches_its_deadline_reveals_without_blaming_the_player() {
        // The deadline is counted from the keypress, so a slow source search reached it before
        // mpv had been started. The playback says when a started player shows nothing.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        for _ in 0..crate::eyecatch::MAX_HOLD_FRAMES + crate::eyecatch::SWEEP_FRAMES * 2 {
            a.tick_animation();
        }
        assert!(a.eyecatch.is_none(), "the wipe never ended");
        assert!(a.toasts.iter().all(|t| t.kind != ToastKind::Alert), "{:?}", a.toasts);
    }

    #[test]
    fn the_resume_toast_offers_nothing_it_cannot_do() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::Resumed { position: 87.0 });
        let toast = a.toasts.last().expect("a toast");
        assert_eq!(toast.text, "resumed at 1:27");
    }

    #[test]
    fn n_while_playing_asks_the_playback_from_any_view() {
        // From Continue there is no title open at all, and with another show open the open one
        // is the wrong show: the playback holds the playing title's list, so it answers.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Open, 20);
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(2)),
            title: "Title 1".into(),
            episode: "4".into(),
            ..NowPlaying::default()
        });
        a.nav.pop();
        assert_eq!(
            a.handle(Action::NextEpisode, 20),
            Some(Task::Player(PlayerCommand::Step(1)))
        );
        a.detail = a.content.entries().first().cloned();
        assert_eq!(
            a.handle(Action::PreviousEpisode, 20),
            Some(Task::Player(PlayerCommand::Step(-1)))
        );
    }

    #[test]
    fn an_episode_the_player_asks_for_is_of_the_show_playing() {
        // Title 0's table is loaded and open; Title 1 is the one playing.
        let mut a = app_at_episodes();
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(2)),
            title: "Title 1".into(),
            episode: "4".into(),
            ..NowPlaying::default()
        });
        a.apply(Update::PlayerPlayEpisode { episode: "5".into(), title: Some("Fifth".into()) });
        assert_eq!(
            a.take_pending(),
            Some(Task::Play { id: AnilistId::new(2), episode: "5".into() })
        );
        let playing = a.playing.as_ref().unwrap();
        assert_eq!(playing.title, "Title 1");
        assert_eq!(playing.episode_title.as_deref(), Some("Fifth"));

        // Named by nobody, it is left unnamed rather than given the open show's episode 6.
        a.apply(Update::PlayerPlayEpisode { episode: "6".into(), title: None });
        assert_eq!(a.playing.as_ref().unwrap().episode_title, None);
    }

    #[test]
    fn another_titles_rows_never_speak_for_the_one_opened_after_them() {
        // Title 0's rows are loaded; Title 1 is opened and its own rows have not arrived. The
        // open title is Title 1, and Title 0's rows must not be taken for its.
        let mut a = app_at_episodes();
        a.detail = a.content.entries().get(1).cloned();
        assert!(a.episode_rows_for(AnilistId::new(2)).is_empty());
        assert!(a.next_up(AnilistId::new(2), "1").is_none());
        assert_eq!(a.episode_rows_for(AnilistId::new(1)).len(), 12, "still Title 0's");
        a.begin_playback(AnilistId::new(2), "1".into());
        assert_eq!(a.playing.as_ref().unwrap().episode_title, None);
    }

    #[test]
    fn continue_follows_an_unfinished_episode_watched_out_of_order() {
        // Episodes 1-5 finished; episode 8 left halfway. Continue is episode 8, at 10:00, with
        // the count still five.
        let mut a = app_at_episodes();
        for list in [&mut a.episodes, &mut a.episodes_all] {
            for row in list.iter_mut().take(5) {
                row.completed = true;
            }
        }
        a.detail.as_mut().unwrap().progress = Some((5, 6));
        a.episode_selected = 7;
        a.handle(Action::Open, 20);
        a.apply(Update::Playback { position: 600.0, duration: Some(1200.0), paused: false });
        a.apply(Update::PlaybackEnded { watched: false, ending: Ending::Closed });
        let entry = a.detail.as_ref().unwrap();
        assert_eq!(entry.progress, Some((5, 8)));
        assert_eq!(entry.resume.map(|r| r.position), Some(600.0));
    }

    #[test]
    fn after_the_last_episode_continue_goes_back_to_one_skipped() {
        let mut a = app_at_episodes();
        for list in [&mut a.episodes, &mut a.episodes_all] {
            for row in list.iter_mut().take(11).filter(|r| r.number != "4") {
                row.completed = true;
            }
        }
        a.detail.as_mut().unwrap().progress = Some((10, 12));
        a.episode_selected = 11;
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert_eq!(a.detail.as_ref().unwrap().progress, Some((11, 4)));
    }

    #[test]
    fn continue_never_offers_an_episode_already_watched() {
        // Episodes 1-5 and 7 finished. Finishing 6 counts once, and Continue moves past 7.
        let mut a = app_at_episodes();
        for list in [&mut a.episodes, &mut a.episodes_all] {
            for row in list.iter_mut().take(5) {
                row.completed = true;
            }
            list[6].completed = true;
        }
        a.detail.as_mut().unwrap().progress = Some((6, 6));
        a.episode_selected = 5;
        a.handle(Action::Open, 20);
        a.apply(finished(Some("7")));
        assert_eq!(a.detail.as_ref().unwrap().progress, Some((7, 8)));

        // A rewatch of episode 3 moves neither the count nor Continue back into what is done.
        a.take_pending();
        a.apply(Update::PlaybackEnded { watched: false, ending: Ending::Stopped });
        a.nav.push(StageView::Episodes(AnilistId::new(1)));
        a.episode_selected = 2;
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert_eq!(a.detail.as_ref().unwrap().progress, Some((7, 8)));
    }

    #[test]
    fn playback_updates_never_resurrect_a_finished_session() {
        // A position tick can arrive after the `Ended` event that preceded it. Recreating
        // `playing` from it would leave a phantom player on screen.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: false, ending: Ending::Closed });
        assert!(a.playing.is_none());

        a.apply(Update::Playback { position: 100.0, duration: Some(1435.0), paused: false });
        assert!(a.playing.is_none(), "a stray tick brought the player back");
    }

    #[test]
    fn finishing_an_episode_queues_the_next_one() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(finished(Some("2")));
        assert_eq!(
            a.take_pending(),
            Some(Task::Play { id: AnilistId::new(1), episode: "2".into() }),
            "auto-next did not follow a finished episode"
        );
    }

    #[test]
    fn quitting_partway_does_not_roll_on_to_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: false, ending: Ending::Closed });
        assert_eq!(a.take_pending(), None);
    }

    #[test]
    fn closing_or_stopping_past_the_threshold_does_not_roll_on() {
        // Past the commit threshold the episode counts as watched, and auto-next used to read
        // that as finished: closing the window during the credits opened a new one on the next
        // episode. Only an episode mpv played to its end rolls on.
        for ending in [Ending::Closed, Ending::Stopped] {
            let mut a = app_at_episodes();
            a.handle(Action::Open, 20);
            a.apply(Update::PlaybackEnded { watched: true, ending: ending.clone() });
            assert_eq!(a.take_pending(), None, "{ending:?} rolled on");
            assert!(a.episodes[0].completed, "still recorded as watched after {ending:?}");
        }
    }

    #[test]
    fn auto_next_continues_the_show_that_played_under_its_own_name() {
        // Another show's page is open by the time the episode ends; the next episode is still
        // the one that played, named as it was, with its own table's episode title.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.detail = a.content.entries().get(1).cloned();
        a.apply(finished(Some("2")));
        assert_eq!(
            a.take_pending(),
            Some(Task::Play { id: AnilistId::new(1), episode: "2".into() })
        );
        let playing = a.playing.as_ref().expect("the next episode is staged");
        assert_eq!(playing.title, "Title 0");
        assert_eq!(playing.episode_title.as_deref(), Some("Episode 2"));
    }

    #[test]
    fn a_cancel_does_not_carry_over_to_the_next_episode() {
        // Up Next was cancelled on episode 1, then episode 2 was started from the player. Its
        // own ending rolls on: whether it does is the playback's to say, per episode, and
        // nothing of episode 1's choice is held here to overrule it.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlayerPlayEpisode { episode: "2".into(), title: None });
        assert!(matches!(a.take_pending(), Some(Task::Play { .. })));
        a.apply(Update::EpisodeLeft {
            id: AnilistId::new(1),
            episode: "1".into(),
            position: 300.0,
            duration: Some(1435.0),
            watched: false,
        });
        a.apply(finished(Some("3")));
        assert_eq!(
            a.take_pending(),
            Some(Task::Play { id: AnilistId::new(1), episode: "3".into() })
        );
    }

    #[test]
    fn x_on_home_dismisses_the_selected_title_from_continue() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.selected = 1;
        let task = a.handle(Action::HideFromContinue, 20);
        assert_eq!(task, Some(Task::HideFromContinue { id: AnilistId::new(2) }));
        // Gone from the screen before the store is even asked — same contract as
        // marking watched.
        assert_eq!(a.content.len(), 2);
        assert!(a.content.entries().iter().all(|e| e.id != AnilistId::new(2)));
    }

    #[test]
    fn dismissing_from_continue_only_works_on_the_home_list() {
        // A pushed episode table keeps the Home entries as `content`; the key must not
        // remove a show the viewer cannot see, least of all the one they are looking into.
        let mut a = app_at_episodes();
        assert_eq!(a.handle(Action::HideFromContinue, 20), None);
        assert_eq!(a.content.len(), 3, "a row vanished from a list not on screen");
    }

    #[test]
    fn a_first_watch_moves_progress_without_waiting_for_a_reload() {
        // A show with no history has no (done, next) pair to advance, and the old rule —
        // only advance the episode progress was waiting on — silently skipped it, so the
        // detail meter sat empty until the next load from the store.
        let mut a = app_at_episodes();
        assert_eq!(a.detail.as_ref().unwrap().progress, None);
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert_eq!(a.detail.as_ref().unwrap().progress, Some((1, 2)));
    }

    #[test]
    fn a_watch_lands_only_on_the_entry_being_played() {
        // Several shows on Home can share a next-episode number; a finished episode used
        // to advance every one of them.
        let mut a = app_at_episodes();
        if let Content::Entries(entries) = &mut a.content {
            for e in entries.iter_mut() {
                e.progress = Some((0, 1));
            }
        }
        a.detail.as_mut().unwrap().progress = Some((0, 1));
        a.handle(Action::Open, 20);
        a.apply(finished(None));

        let entries = a.content.entries();
        assert_eq!(entries[0].progress, Some((1, 2)));
        assert_eq!(entries[1].progress, Some((0, 1)), "another show took the watch");
    }

    #[test]
    fn a_rewatch_does_not_inflate_the_progress_count() {
        // The store counts *distinct* completed episodes; the in-memory mirror has to
        // hold the same line or the meter jumps on every revisit.
        let mut a = app_at_episodes();
        a.episodes[2].completed = true;
        a.episodes_all[2].completed = true;
        a.detail.as_mut().unwrap().progress = Some((3, 4));
        a.episode_selected = 2;
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert_eq!(a.detail.as_ref().unwrap().progress, Some((3, 4)));
    }

    #[test]
    fn quitting_partway_moves_the_resume_point_at_once() {
        // The Continue rail reads `entry.resume`; leaving it where the *previous* session
        // stopped made a fresh quit look unrecorded until the next load from the store.
        let mut a = app_at_episodes();
        a.detail.as_mut().unwrap().progress = Some((0, 1));
        a.handle(Action::Open, 20);
        a.apply(Update::Playback { position: 600.0, duration: Some(1200.0), paused: false });
        a.apply(Update::PlaybackEnded { watched: false, ending: Ending::Closed });

        let resume = a.detail.as_ref().unwrap().resume.expect("no resume point");
        assert_eq!(resume.position, 600.0);
        assert_eq!(resume.fraction, Some(0.5));
    }

    #[test]
    fn an_ending_asks_for_the_next_episode_without_taking_a_busy_slot() {
        let mut a = app();
        a.apply(Update::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert_eq!(
            a.take_pending(),
            Some(Task::Prefetch { id: AnilistId::new(3), episode: "6".into() })
        );
        // Best effort: a slot already holding work keeps it.
        a.apply(Update::PlayerNightMode(true));
        a.apply(Update::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert!(matches!(a.take_pending(), Some(Task::SaveSetting { .. })));
    }

    #[test]
    fn nothing_follows_an_ending_the_player_kept_no_window_for() {
        // The finale, auto-next turned off, or Up Next cancelled: the playback names no next
        // episode, and nothing is searched for. Guessing "the number after" here searched for
        // an episode the list said did not exist, after the finale.
        let mut a = app_at_episodes();
        a.episode_selected = 11;
        a.handle(Action::Open, 20);
        a.apply(finished(None));
        assert_eq!(a.take_pending(), None);
        assert!(a.toasts.iter().any(|t| t.text == "episode finished"));
    }

    #[test]
    fn a_non_numeric_episode_has_no_next() {
        // "OVA" and "Special" are real labels with no successor; guessing one would start
        // playing something unrelated.
        let a = app_at_episodes();
        assert!(a.next_up(AnilistId::new(1), "OVA").is_none());
        assert_eq!(a.next_up(AnilistId::new(1), "11").map(|r| r.number.as_str()), Some("12"));
        assert!(a.next_up(AnilistId::new(1), "12").is_none(), "the table ends at 12");
    }

    #[test]
    fn playback_keys_become_player_commands() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        for (action, expected) in [
            (Action::PlayPause, PlayerCommand::TogglePause),
            (Action::SeekForward, PlayerCommand::Seek(5.0)),
            (Action::SeekBackFar, PlayerCommand::Seek(-30.0)),
            (Action::SpeedUp, PlayerCommand::Speed(0.25)),
            (Action::StopPlayback, PlayerCommand::Stop),
        ] {
            assert_eq!(
                a.handle(action, 20),
                Some(Task::Player(expected)),
                "{action:?} did not reach the player"
            );
        }
    }

    #[test]
    fn skip_only_seeks_when_a_skip_is_actually_offered() {
        // Seeking to an arbitrary place because no interval was known would be worse than
        // saying nothing is there.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::SkipOpening, 20), None);

        a.apply(Update::SkipAvailable { label: "opening", to: 93.2 });
        assert_eq!(
            a.handle(Action::SkipOpening, 20),
            Some(Task::Player(PlayerCommand::SeekTo(93.2)))
        );

        a.apply(Update::SkipCleared);
        assert_eq!(a.handle(Action::SkipOpening, 20), None);
    }

    #[test]
    fn detaching_leaves_the_player_alone_and_pops_the_screen() {
        // The point of detach: mpv keeps playing and history keeps recording. Sending it a
        // command would defeat the whole gesture.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::Detach, 20), None, "detach must not touch the player");
        assert!(!matches!(a.nav.current(), StageView::NowPlaying));
    }

    #[test]
    fn playback_bindings_do_not_hijack_browsing() {
        // With nothing playing, `x` and `n` have to fall through to their browsing meaning
        // rather than being swallowed by a player that is not there.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        assert_eq!(a.handle(Action::NextEpisode, 20), None);
        assert!(a.playing.is_none());
    }

    #[test]
    fn auto_next_steps_over_filler_when_asked() {
        let mut a = app_at_episodes();
        a.config.playback.skip_filler = true;
        for list in [&mut a.episodes, &mut a.episodes_all] {
            for i in [1, 2] {
                list[i].skippable = true;
                list[i].kind = Some("filler");
            }
        }
        a.handle(Action::Open, 20); // plays ep 1

        a.apply(finished(Some("2")));
        let task = a.take_pending().expect("auto-next must fire");
        match task {
            Task::Play { episode, .. } => assert_eq!(episode, "4", "eps 2-3 are pure filler"),
            other => panic!("expected Play, got {other:?}"),
        }
    }

    #[test]
    fn a_remembered_speed_carries_into_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackSpeed(1.25));
        assert_eq!(a.config.playback.persisted_speed, Some(1.25));

        a.apply(finished(Some("2")));
        a.take_pending();
        assert_eq!(a.playing.as_ref().map(|p| p.speed), Some(1.25));
    }

    #[test]
    fn the_playhead_clock_is_a_fixed_field() {
        // Ragged clocks shuffle sideways once a minute. Padding elapsed to the width of total
        // is what makes the pair sit still.
        let playing =
            NowPlaying { position: 552.0, duration: Some(1435.0), ..NowPlaying::default() };
        assert_eq!(playing.elapsed(), " 9:12");
        assert_eq!(playing.total(), "23:55");
        assert_eq!(playing.elapsed().chars().count(), playing.total().chars().count());
    }

    #[test]
    fn an_unknown_duration_reads_as_unknown_rather_than_zero() {
        // Torrent streams often have no duration until enough has been fetched. Showing 0:00
        // would claim the episode is empty.
        let playing = NowPlaying { position: 30.0, duration: None, ..NowPlaying::default() };
        assert_eq!(playing.total(), "--:--");
        assert_eq!(playing.fraction(), 0.0);
    }

    #[test]
    fn the_clock_grows_to_hours_only_when_it_has_to() {
        assert_eq!(NowPlaying::clock(0.0), "0:00");
        assert_eq!(NowPlaying::clock(59.4), "0:59");
        assert_eq!(NowPlaying::clock(3599.0), "59:59");
        assert_eq!(NowPlaying::clock(3600.0), "1:00:00");
        // A negative position is a bad IPC reply, not a reason to render garbage.
        assert_eq!(NowPlaying::clock(-5.0), "0:00");
    }

    fn connected(outbox: u32) -> SyncState {
        SyncState { tracker: "anilist".into(), connected: true, outbox, ..SyncState::default() }
    }

    #[test]
    fn the_sync_badge_carries_the_queue_depth() {
        // "Did my progress actually go anywhere?" is the only sync question anyone asks, and
        // the depth is the answer.
        assert_eq!(connected(0).badge(), "anilist ⇅");
        assert_eq!(connected(3).badge(), "anilist ⇅ 3");
        assert!(!connected(3).is_alerting());
    }

    #[test]
    fn a_rejected_token_reads_as_something_to_act_on() {
        let state = SyncState { needs_reauth: true, ..connected(2) };
        assert_eq!(state.badge(), "anilist ✕ sign in");
        assert!(state.is_alerting(), "re-auth must use the alert role");
    }

    #[test]
    fn a_tracker_with_no_account_is_quiet_rather_than_alarming() {
        // Watching with no account is a supported way to use this, not a fault.
        let state = SyncState { tracker: "anilist".into(), ..SyncState::default() };
        assert_eq!(state.badge(), "anilist ○");
        assert!(!state.is_alerting());
    }

    #[test]
    fn re_authorisation_is_announced_once_not_every_tick() {
        // The drain runs on an interval; repeating the alert each time would bury everything
        // else in toasts.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        // Counted in the log, which counts every announcement: the toast stack folds a repeat
        // into the one already showing, so it cannot tell whether one was made.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        assert_eq!(announced(&a), 1);
        a.apply(Update::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        assert_eq!(announced(&a), 1, "re-announced an unchanged auth failure");
    }

    #[test]
    fn a_sync_update_replaces_rather_than_appends() {
        // Otherwise the header would grow a new chip on every drain tick.
        let mut a = app();
        for depth in [1, 2, 3] {
            a.apply(Update::Sync(Box::new(connected(depth))));
        }
        assert_eq!(a.sync.len(), 1);
        assert_eq!(a.sync[0].outbox, 3);
    }

    #[test]
    fn queueing_progress_moves_the_badge_immediately() {
        // The row is already in SQLite by the time this arrives, so counting it optimistically
        // is honest — the next drain can only correct it downward.
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        a.apply(Update::ProgressQueued);
        assert_eq!(a.sync[0].outbox, 1);
        assert_eq!(a.sync[0].badge(), "anilist ⇅ 1");
    }

    #[test]
    fn conflicts_are_announced_when_they_first_appear() {
        // Surfaced rather than resolved — the whole reason the merge refuses to guess.
        let mut a = app();
        a.apply(Update::Conflicts(vec![conflict("Frieren")]));
        assert_eq!(a.conflicts.len(), 1);
        assert!(a.toasts.iter().any(|t| t.text.contains("disagreement")));

        // A second pull returning the same set must not re-announce.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        let before = announced(&a);
        a.apply(Update::Conflicts(vec![conflict("Frieren")]));
        assert_eq!(announced(&a), before);
    }

    fn conflict(title: &str) -> ConflictRow {
        ConflictRow {
            anilist_id: AnilistId::new(1),
            title: title.into(),
            field: "status".into(),
            local: "Completed".into(),
            remote: "Current".into(),
        }
    }

    #[test]
    fn the_conflicts_overlay_refuses_to_open_with_nothing_to_resolve() {
        // An empty modal is a dead end.
        let mut a = app();
        a.handle(Action::ShowConflicts, 20);
        assert!(!a.nav.has_overlay());

        a.apply(Update::Conflicts(vec![conflict("Frieren")]));
        a.handle(Action::ShowConflicts, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Conflicts));
    }

    #[test]
    fn resolving_a_conflict_removes_it_and_closes_when_empty() {
        let mut a = app();
        a.apply(Update::Conflicts(vec![conflict("Frieren"), conflict("Dandadan")]));
        a.handle(Action::ShowConflicts, 20);

        let task = a.handle(Action::Open, 20);
        assert!(matches!(task, Some(Task::ResolveConflict { keep_local: true, .. })));
        assert_eq!(a.conflicts.len(), 1);
        assert!(a.nav.has_overlay(), "one left, so the overlay stays");

        a.handle(Action::Open, 20);
        assert!(a.conflicts.is_empty());
        assert!(!a.nav.has_overlay(), "nothing left to resolve, so it closes");
    }

    #[test]
    fn an_open_list_overlay_owns_movement() {
        // Without this, arrowing through Accounts would scroll the library behind it.
        let mut a = app();
        a.apply(a.content_here(entries(10)));
        a.apply(Update::Conflicts(vec![conflict("a"), conflict("b"), conflict("c")]));
        a.handle(Action::ShowConflicts, 20);

        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.overlay_selected, 2);
        assert_eq!(a.selected, 0, "the list behind the overlay moved");

        // And it cannot run off the end.
        a.handle(Action::Down, 20);
        assert_eq!(a.overlay_selected, 2);
    }

    fn store_row(id: &str, owner: bool, plugins: Vec<StorePluginRow>) -> StoreRow {
        StoreRow {
            id: id.into(),
            name: format!("store {id}"),
            url: "https://plugins.example".into(),
            fingerprint: "9f3c 1a2b 77de 0c41".into(),
            member: "Johan".into(),
            owner,
            state: StoreState::Ready,
            plugins,
        }
    }

    fn store_plugin(id: &str, status: StorePluginStatus) -> StorePluginRow {
        StorePluginRow {
            id: id.into(),
            name: id.into(),
            version: "2.0.0".into(),
            hosts: vec!["api.example".into()],
            status,
        }
    }

    /// An app on Providers → stores, with the given stores.
    fn at_stores(stores: Vec<StoreRow>) -> App {
        let mut a = app();
        a.go_to_section(Section::Providers);
        assert_eq!(a.handle(Action::Open, 20), Some(Task::LoadStores));
        a.apply(Update::Stores(stores));
        a
    }

    fn type_text(a: &mut App, text: &str) {
        for ch in text.chars() {
            a.type_char(ch);
        }
    }

    #[test]
    fn enter_on_providers_opens_the_stores_from_the_rail_too() {
        let mut a = app();
        a.go_to_section(Section::Providers);
        a.nav.focus_rail();
        assert_eq!(a.handle(Action::Open, 20), Some(Task::LoadStores));
        assert_eq!(a.nav.current(), &StageView::Stores);
        assert_eq!(a.nav.focus(), Focus::Stage);
    }

    #[test]
    fn joining_is_paste_check_confirm() {
        let mut a = at_stores(Vec::new());
        // With nothing joined, the only row is "join a store".
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
        assert!(a.is_typing());

        // A link that wrapped in a chat window, pasted with its line break.
        a.paste("https://plugins.example/join/abc\n  #key");
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::PreviewStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(!a.nav.has_overlay());

        a.apply(Update::StorePreview(Ok(StorePreview {
            link: Secret("https://plugins.example/join/abc#key".into()),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            fingerprint: "9f3c".into(),
            already_joined: false,
        })));
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::JoinStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(a.join_link.is_empty(), "the spent link is not kept");

        // Joined: the new store opens, because the user is still on the list it was joined from.
        a.apply(Update::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Update::StoreJoined { store: "abc".into() });
        assert_eq!(a.nav.current(), &StageView::Store("abc".into()));
    }

    #[test]
    fn a_bad_link_says_why_and_reopens_the_prompt_with_the_link_kept() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc");
        a.handle(Action::Open, 20);
        a.apply(Update::StorePreview(Err("the link is missing the store's key".into())));
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
        assert_eq!(a.join_link, "https://plugins.example/join/abc");
        assert!(a.toasts.iter().any(|t| t.text.contains("missing the store's key")));
    }

    #[test]
    fn a_dismissed_join_prompt_forgets_the_link() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc#key");
        a.handle(Action::Back, 20);
        assert!(a.join_link.is_empty());
    }

    #[test]
    fn a_join_that_finishes_elsewhere_does_not_drag_the_user_back() {
        let mut a = at_stores(Vec::new());
        a.go_to_section(Section::Home);
        a.apply(Update::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Update::StoreJoined { store: "abc".into() });
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
    }

    #[test]
    fn a_plugin_is_installed_from_its_overlay() {
        let mut a = at_stores(vec![store_row(
            "abc",
            false,
            vec![store_plugin("example", StorePluginStatus::Available)],
        )]);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.current(), &StageView::Store("abc".into()));
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::PluginAction));
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Install, PluginChoice::Cancel]);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::InstallPlugin { store: "abc".into(), plugin: "example".into() })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn an_update_offers_the_new_version_and_removal() {
        let mut a = at_stores(vec![store_row(
            "abc",
            false,
            vec![store_plugin(
                "example",
                StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
            )],
        )]);
        a.handle(Action::Open, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.plugin_choices(),
            vec![
                PluginChoice::Update("2.0.0".into()),
                PluginChoice::Remove,
                PluginChoice::Cancel
            ]
        );
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::RemovePlugin { plugin: "example".into() })
        );
    }

    #[test]
    fn a_store_that_removed_this_device_can_only_have_its_plugins_removed() {
        let mut store = store_row(
            "abc",
            true,
            vec![
                store_plugin(
                    "kept",
                    StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
                ),
                store_plugin("never", StorePluginStatus::Available),
            ],
        );
        store.state = StoreState::Revoked;
        let mut a = at_stores(vec![store.clone()]);
        a.handle(Action::Open, 20);
        // No device, invite or owner rows: only the plugins and leaving.
        assert_eq!(
            a.store_lines(&store),
            vec![StoreLine::Plugin(0), StoreLine::Plugin(1), StoreLine::Leave]
        );
        a.handle(Action::Open, 20);
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Remove, PluginChoice::Cancel]);
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.plugin_choices(), vec![PluginChoice::Cancel]);
    }

    #[test]
    fn an_owner_invites_someone_by_name() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        let lines = a.store_lines(a.store("abc").unwrap());
        assert_eq!(
            lines,
            vec![
                StoreLine::LinkDevice,
                StoreLine::Invite,
                StoreLine::Members,
                StoreLine::Publish,
                StoreLine::Admin,
                StoreLine::Leave
            ]
        );
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::InviteName));
        // An empty name is a slip, not an answer.
        assert_eq!(a.handle(Action::Open, 20), None);
        type_text(&mut a, "Alex");
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::CreateInvite { store: "abc".into(), name: Some("Alex".into()) })
        );

        a.apply(Update::InviteLink(InviteLinkView {
            store: "abc".into(),
            link: Secret("https://plugins.example/join/x#y".into()),
            for_whom: Some("Alex".into()),
            expires_at: 0,
            copied: true,
        }));
        assert_eq!(a.nav.overlay(), Some(&Overlay::InviteLink));
        a.handle(Action::Open, 20);
        assert!(!a.nav.has_overlay());
        assert!(a.invite_link.is_none());
    }

    #[test]
    fn an_owner_opens_the_admin_page_from_the_store() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..4 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(a.handle(Action::Open, 20), Some(Task::OpenAdmin { store: "abc".into() }));
    }

    #[test]
    fn a_member_has_no_admin_page_row() {
        let a = at_stores(vec![store_row("abc", false, Vec::new())]);
        assert!(!a.store_lines(a.store("abc").unwrap()).contains(&StoreLine::Admin));
    }

    #[test]
    fn linking_a_device_is_one_enter() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::CreateInvite { store: "abc".into(), name: None })
        );
    }

    #[test]
    fn removing_a_member_asks_first_and_ignores_stray_keys() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.handle(Action::Open, 20), Some(Task::LoadMembers { store: "abc".into() }));
        assert_eq!(a.nav.current(), &StageView::StoreMembers("abc".into()));

        let member = |id: &str, you| StoreMemberRow {
            id: id.into(),
            name: id.into(),
            owner: you,
            devices: 1,
            last_seen: None,
            you,
        };
        a.apply(Update::StoreMembers {
            store: "abc".into(),
            members: vec![member("me", true), member("alex", false)],
            invites: vec![StoreInviteRow {
                id: "inv".into(),
                name: "sam".into(),
                device: false,
                expires_at: 0,
            }],
        });

        // Yourself: refused with a reason, not a confirmation.
        assert_eq!(a.handle(Action::Open, 20), None);
        assert!(!a.nav.has_overlay());

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        // A stray key must not move the cursor off the person being removed.
        a.handle(Action::Down, 20);
        assert_eq!(a.member_selected, 1);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::RemoveMember { store: "abc".into(), member: "alex".into() })
        );

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::CancelInvite { store: "abc".into(), invite: "inv".into() })
        );
    }

    #[test]
    fn leaving_asks_first_and_the_page_goes_with_the_store() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(a.handle(Action::Open, 20), Some(Task::LeaveStore { store: "abc".into() }));

        a.apply(Update::Stores(Vec::new()));
        assert_eq!(a.nav.current(), &StageView::Stores);
    }

    #[test]
    fn publishing_picks_from_local_plugins_for_the_store_on_screen() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..3 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::LoadLocalPlugins { store: "abc".into() })
        );
        let local = LocalPluginRow {
            id: "example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            hosts: Vec::new(),
            published: None,
        };
        // An answer for another store is not offered here.
        a.apply(Update::LocalPlugins { store: "other".into(), plugins: vec![local.clone()] });
        assert!(!a.nav.has_overlay());

        a.apply(Update::LocalPlugins { store: "abc".into(), plugins: vec![local] });
        assert_eq!(a.nav.overlay(), Some(&Overlay::PublishPick));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::PublishPlugin { store: "abc".into(), plugin: "example".into() })
        );
    }

    #[test]
    fn nothing_to_publish_says_so() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        for _ in 0..3 {
            a.handle(Action::Down, 20);
        }
        a.handle(Action::Open, 20);
        a.apply(Update::LocalPlugins { store: "abc".into(), plugins: Vec::new() });
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.text.contains("no plugins")));
    }

    #[test]
    fn the_store_cursor_stops_at_the_join_row() {
        let mut a = at_stores(vec![
            store_row("a", false, Vec::new()),
            store_row("b", false, Vec::new()),
        ]);
        for _ in 0..5 {
            a.handle(Action::Down, 20);
        }
        assert_eq!(a.store_selected, 2, "one past the stores is the join row");
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
    }

    #[test]
    fn refreshing_a_store_screen_asks_the_stores_again() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        assert_eq!(a.handle(Action::Refresh, 20), Some(Task::LoadStores));
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::Refresh, 20), Some(Task::LoadStores));
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Refresh, 20),
            Some(Task::LoadMembers { store: "abc".into() })
        );
    }

    #[test]
    fn a_store_list_arriving_does_not_reorder_an_open_plugin_overlay() {
        let mut a = at_stores(vec![store_row(
            "abc",
            true,
            vec![store_plugin("example", StorePluginStatus::Installed)],
        )]);
        a.handle(Action::Open, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.plugin_choices(),
            [PluginChoice::Remove, PluginChoice::Unpublish, PluginChoice::Cancel]
        );
        a.handle(Action::Down, 20);
        // The refresh that opening the screen asked for lands now, with an update waiting.
        a.apply(Update::Stores(vec![store_row(
            "abc",
            true,
            vec![store_plugin(
                "example",
                StorePluginStatus::Update { from: "1.0.0".into(), new_hosts: Vec::new() },
            )],
        )]));
        a.handle(Action::Open, 20);
        assert_eq!(
            a.store_confirm,
            Some(StoreConfirm::Unpublish {
                store: "abc".into(),
                plugin: "example".into(),
                name: "example".into()
            }),
            "the choice under the cursor is still the one that was there"
        );
    }

    #[test]
    fn a_late_preview_does_not_take_over_an_open_question() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        let leave = a.store_confirm.clone();
        assert!(matches!(leave, Some(StoreConfirm::Leave { .. })));
        a.apply(Update::StorePreview(Ok(StorePreview {
            link: Secret("x".into()),
            name: "Other".into(),
            url: "https://other.example".into(),
            fingerprint: "0000".into(),
            already_joined: false,
        })));
        assert_eq!(a.store_confirm, leave);
        assert_eq!(a.handle(Action::Open, 20), Some(Task::LeaveStore { store: "abc".into() }));
    }

    #[test]
    fn a_paste_outside_a_text_field_does_nothing() {
        let mut a = at_stores(Vec::new());
        a.paste("dddd");
        assert!(a.join_link.is_empty());
        assert!(a.toasts.is_empty(), "no stray bindings fired");
    }

    #[test]
    fn the_accounts_screen_offers_sign_in_or_sign_out_by_state() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: false,
            ..SyncState::default()
        })));
        a.handle(Action::ShowAccounts, 20);
        // A section, so the rail holds focus until you step into it — the same as everywhere else.
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::Connect { tracker: "anilist".into() })
        );

        a.apply(Update::Sync(Box::new(connected(0))));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::Disconnect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn a_tracker_needing_reauth_offers_sign_in_not_sign_out() {
        // It is technically connected but useless, so offering "sign out" would be a dead end.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        a.handle(Action::ShowAccounts, 20);
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::Connect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn library_segments_step_both_ways_and_stop_at_the_ends() {
        // The same rule as the rail and every list: the ends clamp rather than wrap.
        assert_eq!(LibrarySegment::Watching.step(1), LibrarySegment::Planning);
        assert_eq!(LibrarySegment::Planning.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Watching.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Dropped.step(1), LibrarySegment::Dropped);
    }

    #[test]
    fn stepping_a_library_segment_reloads_and_resets_the_selection() {
        // A stale selection index into a different list would point at the wrong title.
        let mut a = app();
        a.go_to_section(Section::Library);
        a.apply(a.content_here(entries(10)));
        a.selected = 7;
        a.nav.focus_stage();

        let task = a.handle(Action::Right, 20);
        assert_eq!(a.library_segment, LibrarySegment::Planning);
        assert_eq!(task, Some(Task::LoadLibrary(LibrarySegment::Planning)));
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn horizontal_keys_still_move_focus_outside_the_library() {
        // The segment behaviour is Library-specific; everywhere else h/l crosses the divider.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Left, 20);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn the_segment_wire_names_are_the_trackers_own() {
        // These strings are matched against what AniList returns, so a rename here would
        // silently empty a segment.
        assert_eq!(LibrarySegment::Watching.wire(), "CURRENT");
        assert_eq!(LibrarySegment::Completed.wire(), "COMPLETED");
    }

    #[test]
    fn setting_a_list_status_preselects_the_current_one() {
        // One keystroke to confirm in the common case, rather than a hunt.
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        let mut rows = entries(3);
        if let Content::Entries(rows) = &mut rows {
            // As the Library's own fetch marks its rows.
            rows.iter_mut().for_each(|row| row.list_status = Some(LibrarySegment::Completed));
        }
        a.apply(a.content_here(rows));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::ListStatus));
        assert_eq!(a.overlay_selected, 2);

        let task = a.handle(Action::Open, 20);
        assert_eq!(
            task,
            Some(Task::SetStatus { id: AnilistId::new(1), status: LibrarySegment::Completed })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn a_status_given_this_session_outlives_a_reload_until_the_tracker_answers() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        let planning = 1;
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Task::SetStatus { id: AnilistId::new(1), status: LibrarySegment::Planning })
        );

        // Home again: the rows are rebuilt and carry no status of their own.
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.overlay_selected, planning, "the status given before the reload");
        a.handle(Action::Back, 20);

        // A pull that lists the title is the tracker's answer, and wins.
        a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        let mut rows = entries(3);
        if let Content::Entries(rows) = &mut rows {
            rows.iter_mut().for_each(|row| row.list_status = Some(LibrarySegment::Completed));
        }
        a.apply(a.content_here(rows));
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        assert_eq!(a.overlay_selected, 0, "nothing given since the pull, so nothing known");
    }

    #[test]
    fn rating_a_title_queues_a_score_out_of_ten() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::RateTitle, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Rate));

        // Rows run 10 down to 1, so two steps down from the top is an 8.
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20);
        assert_eq!(task, Some(Task::SetScore { id: AnilistId::new(1), score: 8.0 }));
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn recommendations_open_a_list_and_enter_walks_into_the_pick() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        let mut detail = Entry::new(AnilistId::new(1), "Frieren");
        detail.recommended = vec![RelatedTitle {
            id: AnilistId::new(99),
            title: "Mushishi".into(),
            relation: String::new(),
            format: Some("TV".into()),
        }];
        a.detail = Some(detail);

        a.handle(Action::Recommendations, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Recommendations));
        let task = a.handle(Action::Open, 20);
        assert_eq!(task, Some(Task::LoadDetail(AnilistId::new(99))));
        assert!(matches!(a.nav.current(), StageView::Title(id) if id.get() == 99));
    }

    #[test]
    fn the_trailer_key_plays_when_known_and_says_so_when_not() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));

        // A list row never opened: the trailer is on the record its page fetches.
        a.handle(Action::PlayTrailer, 20);
        assert!(a.toasts.iter().any(|t| t.text.contains("from the title's page")));

        let mut detail = Entry::new(AnilistId::new(1), "Frieren");
        detail.trailer_url = Some("https://www.youtube.com/watch?v=x".into());
        a.detail = Some(detail);
        let task = a.handle(Action::PlayTrailer, 20);
        assert_eq!(
            task,
            Some(Task::PlayTrailer {
                url: "https://www.youtube.com/watch?v=x".into(),
                title: "Frieren".into(),
            })
        );
    }

    #[test]
    fn recommendations_with_none_known_say_so_rather_than_opening_nothing() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        a.handle(Action::Recommendations, 20);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.text.contains("from the title's page")));
    }

    #[test]
    fn enter_on_the_kitsu_row_asks_for_credentials_in_place() {
        // No browser to open and no command line to visit: the prompt is the flow.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState::new("kitsu"))));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();

        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuUsername));

        for ch in "nagare".chars() {
            a.type_char(ch);
        }
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuPassword));

        for ch in "hunter2".chars() {
            a.type_char(ch);
        }
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Task::ConnectKitsu {
                username: "nagare".into(),
                password: Secret("hunter2".into()),
            })
        );
        assert!(!a.nav.has_overlay());
        // Dispatched means gone: neither half of the credential stays in the app state.
        assert!(a.kitsu_user.is_empty());
        assert!(a.kitsu_pass.is_empty());
    }

    #[test]
    fn an_empty_submit_stays_in_the_prompt_and_esc_wipes_it() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState::new("kitsu"))));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();
        a.handle(Action::Open, 10);

        // Enter on an empty field is a slip, not an answer.
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuUsername));

        for ch in "nagare".chars() {
            a.type_char(ch);
        }
        a.handle(Action::Open, 10);
        for ch in "half".chars() {
            a.type_char(ch);
        }
        a.handle(Action::Back, 10);
        assert!(!a.nav.has_overlay());
        assert!(a.kitsu_pass.is_empty(), "a dismissed sign-in must leave nothing behind");
        assert!(a.kitsu_user.is_empty());
    }

    #[test]
    fn a_password_never_renders_through_debug() {
        // The task router logs tasks on a routing mistake; this is what keeps that log
        // safe to read.
        let task = Task::ConnectKitsu {
            username: "nagare".into(),
            password: Secret("hunter2".into()),
        };
        let printed = format!("{task:?}");
        assert!(!printed.contains("hunter2"), "leaked: {printed}");
        assert!(printed.contains("redacted"));
    }

    #[test]
    fn the_meta_chip_appears_only_while_a_fallback_is_serving() {
        let mut a = app();
        assert!(!a.meta_degraded(), "no status yet is not degraded");

        let row = |source: &str, active: bool| MetaSourceRow {
            source: source.into(),
            state: String::new(),
            healthy: true,
            active,
            detail: None,
        };
        a.apply(Update::MetaStatus(vec![row("anilist", true), row("tenrai", false)]));
        assert!(!a.meta_degraded(), "anilist serving is the resting state");

        a.apply(Update::MetaStatus(vec![row("anilist", false), row("tenrai", true)]));
        assert!(a.meta_degraded());
    }

    #[test]
    fn status_and_rating_with_no_tracker_signed_in_say_so_instead_of_pretending() {
        // "marked watching" and "rated 8/10" used to be said for choices that went nowhere.
        // A tracker that is configured but signed out does not count.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));

        for action in [Action::SetListStatus, Action::RateTitle] {
            a.toasts.clear();
            assert_eq!(a.handle(action, 20), None);
            assert!(!a.nav.has_overlay(), "{action:?} offered a picker that goes nowhere");
            let said = &a.toasts.last().expect("nothing said").text;
            assert!(said.contains("none is signed in") && said.contains(":accounts"), "{said}");
        }
    }

    #[test]
    fn a_status_says_which_trackers_it_was_queued_for() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        a.apply(Update::Sync(Box::new(SyncState { connected: true, ..SyncState::new("mal") })));
        a.apply(Update::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Open, 20);
        let said = &a.toasts.last().unwrap().text;
        assert!(said.ends_with("queued for anilist, mal"), "{said}");
    }

    #[test]
    fn rating_with_nothing_selected_says_so() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::RateTitle, 20), None);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.text.contains("nothing selected")));
    }

    #[test]
    fn a_resync_with_no_tracker_says_so_rather_than_doing_nothing() {
        let mut a = app();
        assert_eq!(a.handle(Action::ForceResync, 20), None);
        assert!(a.toasts.iter().any(|t| t.text.contains("no tracker")));

        a.apply(Update::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::ForceResync, 20), Some(Task::SyncNow));
    }

    #[test]
    fn selection_cannot_move_outside_the_content() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        // The rail owns the vertical axis while it has focus, and it holds focus at startup.
        a.nav.focus_stage();

        for _ in 0..10 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.selected, 2, "clamped at the last item");

        for _ in 0..10 {
            a.handle(Action::Up, 10);
        }
        assert_eq!(a.selected, 0, "clamped at the first");
    }

    #[test]
    fn moving_in_empty_content_is_a_no_op() {
        let mut a = app();
        a.apply(a.content_here(Content::Entries(vec![])));
        a.handle(Action::Down, 10);
        a.handle(Action::Bottom, 10);
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn shrinking_content_keeps_the_selection_in_range() {
        // The crash this prevents: selecting item 40, a refresh returning 3, and the next
        // render indexing past the end.
        let mut a = app();
        a.apply(a.content_here(entries(50)));
        a.nav.focus_stage();
        a.handle(Action::Bottom, 10);
        assert_eq!(a.selected, 49);

        a.apply(a.content_here(entries(3)));
        assert!(a.selected < 3, "selection {} out of range", a.selected);
        assert!(a.selected_entry().is_some());
    }

    #[test]
    fn emptying_content_resets_selection_and_scroll() {
        let mut a = app();
        a.apply(a.content_here(entries(50)));
        a.handle(Action::Bottom, 10);
        a.apply(a.content_here(Content::Entries(vec![])));
        assert_eq!(a.selected, 0);
        assert_eq!(a.offset, 0);
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
        assert_eq!(a.selected, 25);
        assert!(a.selected >= a.offset, "selection scrolled off the top");
        assert!(
            a.selected < a.offset + rows,
            "selection {} outside viewport starting {}",
            a.selected,
            a.offset
        );
    }

    /// Continue with `n` titles on screen and the keyboard in the list, as `1` leaves it.
    fn home_list(n: usize) -> App {
        let mut a = app();
        a.handle(Action::JumpSection(1), 5);
        a.apply(a.content_here(entries(n)));
        a
    }

    fn related(id: u32, title: &str) -> RelatedTitle {
        RelatedTitle {
            id: AnilistId::new(id),
            title: title.into(),
            relation: "sequel".into(),
            format: None,
        }
    }

    fn setting_row(id: SettingId) -> usize {
        SettingId::ALL.iter().position(|s| *s == id).expect("a settings row")
    }

    #[test]
    fn a_title_page_leaves_the_list_under_it_where_it_was() {
        // Every movement key on a title page used to move the list hidden behind it, so Back
        // landed rows away from the title just looked at.
        let mut a = home_list(8);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Task::LoadDetail(AnilistId::new(3))));
        for action in [
            Action::Down,
            Action::Down,
            Action::Up,
            Action::Bottom,
            Action::Top,
            Action::PageDown,
            Action::PageUp,
            Action::Bottom,
        ] {
            assert_eq!(a.handle(action, 5), None, "{action:?}");
        }
        assert_eq!((a.selected, a.offset), (2, 0), "the hidden list moved");
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(3)));

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
        assert_eq!(a.selected_entry().map(|e| e.id), Some(AnilistId::new(3)));
    }

    #[test]
    fn enter_on_a_title_page_opens_its_episodes_rather_than_another_title() {
        let mut a = home_list(8);
        a.handle(Action::Open, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Task::LoadEpisodes(AnilistId::new(1))));
        assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
    }

    #[test]
    fn refresh_on_a_title_or_its_episodes_asks_for_that_title() {
        // It used to reload the section's list instead, under a page that never changed.
        let mut a = home_list(3);
        a.handle(Action::Open, 5);
        assert_eq!(a.handle(Action::Refresh, 5), Some(Task::LoadDetail(AnilistId::new(1))));
        a.handle(Action::ShowEpisodes, 5);
        assert_eq!(a.handle(Action::Refresh, 5), Some(Task::LoadEpisodes(AnilistId::new(1))));
        assert_eq!(a.content.len(), 3, "the list under them is left alone");
    }

    #[test]
    fn f_on_a_title_opened_from_search_means_what_it_means_on_any_title() {
        let mut a = app();
        a.handle(Action::JumpSection(4), 5);
        type_text(&mut a, "query");
        assert!(matches!(a.handle(Action::Open, 5), Some(Task::Search { .. })));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(1)));

        a.handle(Action::Filter, 5);
        assert_eq!(a.nav.overlay(), None, "the search filters opened over a title page");
    }

    #[test]
    fn a_list_rows_actions_go_to_that_row_not_the_title_opened_last() {
        // Back on a list, every title key read the last title opened before the highlighted
        // row, so `a`, `*`, `d`, Space and the rest all went to the wrong show.
        let mut a = home_list(5);
        // List status lives on a tracker, so one is signed in.
        a.apply(Update::Sync(Box::new(connected(0))));
        a.handle(Action::Open, 5);
        // The record the title page fetched, with everything that rides that fetch.
        let mut opened = Entry::new(AnilistId::new(1), "Title 0");
        opened.trailer_url = Some("https://video.example/trailer".into());
        opened.related = vec![related(90, "Its sequel")];
        opened.recommended = vec![related(91, "Something like it")];
        a.apply(Update::Detail(Box::new(opened)));
        a.handle(Action::Back, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        let row = AnilistId::new(3);
        let first = || "1".to_string();

        assert_eq!(
            a.handle(Action::Download, 5),
            Some(Task::DownloadEpisode { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Task::LoadSources { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::PlayParty, 5),
            Some(Task::PlayParty { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::OpenInBrowser, 5),
            Some(Task::OpenExternal { url: "https://anilist.co/anime/3".into() })
        );

        a.handle(Action::SetListStatus, 5);
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Task::SetStatus { id: row, status: LibrarySegment::Watching })
        );
        a.handle(Action::RateTitle, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Task::SetScore { id: row, score: 10.0 }));

        // What rides the detail fetch is the opened title's, not this row's.
        assert_eq!(a.handle(Action::PlayTrailer, 5), None);
        for action in [Action::WatchOrder, Action::Recommendations] {
            a.handle(action, 5);
            assert_eq!(a.nav.overlay(), None, "{action:?} offered the opened title's list");
        }
        // Pinning a source and fixing a match need the title's own page; from a list they
        // say so instead of opening on the other title's state.
        for action in [Action::PickProvider, Action::FixMapping] {
            assert_eq!(a.handle(action, 5), None);
            assert_eq!(a.nav.overlay(), None, "{action:?}");
        }

        assert_eq!(a.handle(Action::ShowEpisodes, 5), Some(Task::LoadEpisodes(row)));
        a.handle(Action::Back, 5);
        assert_eq!(
            a.handle(Action::PlayNext, 5),
            Some(Task::Play { id: row, episode: first() })
        );
        assert_eq!(a.playing.as_ref().map(|p| p.title.as_str()), Some("Title 2"));
    }

    #[test]
    fn on_now_playing_the_title_keys_mean_the_title_playing() {
        // Space from a list plays the row's next episode while an earlier title is still the
        // last one opened; Now Playing is about the one playing.
        let mut a = home_list(5);
        a.handle(Action::Open, 5);
        a.handle(Action::Back, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::PlayNext, 5);
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(3)));
        assert_eq!(
            a.handle(Action::OpenInBrowser, 5),
            Some(Task::OpenExternal { url: "https://anilist.co/anime/3".into() })
        );
    }

    #[test]
    fn back_from_a_related_title_shows_the_page_it_was_opened_from() {
        let mut a = home_list(3);
        a.handle(Action::Open, 5);
        let parent = AnilistId::new(1);
        let mut full = Entry::new(parent, "Title 0");
        full.synopsis = "What the parent is about.".into();
        full.related = vec![related(90, "Its sequel")];
        a.apply(Update::Detail(Box::new(full.clone())));

        a.handle(Action::WatchOrder, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Task::LoadDetail(AnilistId::new(90))));
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(90)));

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Title(parent));
        assert_eq!(a.target(), Some(&full), "the parent's page came back showing the child");
    }

    #[test]
    fn back_through_a_related_titles_episodes_finds_the_parents_table() {
        let mut a = app_at_episodes();
        if let Some(detail) = a.detail.as_mut() {
            detail.related = vec![related(90, "Its sequel")];
        }
        a.episode_selected = 4;
        a.handle(Action::WatchOrder, 5);
        a.handle(Action::Open, 5);
        a.handle(Action::ShowEpisodes, 5);
        a.apply(Update::Episodes {
            id: AnilistId::new(90),
            rows: vec![EpisodeRow { number: "1".into(), ..a.episodes_all[0].clone() }],
        });
        assert_eq!(a.episodes.len(), 1);

        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(90)));
        a.handle(Action::Back, 5);
        assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
        assert_eq!(a.target().map(|e| e.id), Some(AnilistId::new(1)));
        assert_eq!(a.episodes.len(), 12, "the parent's table, not the child's");
        assert_eq!(a.episodes[a.episode_selected].number, "5", "and where it was left");
    }

    #[test]
    fn every_movement_key_moves_the_episode_table() {
        // ↑ and ↓ did; `g`, `G` and the pages moved the list behind the table instead.
        let mut a = app_at_episodes();
        a.handle(Action::Bottom, 5);
        assert_eq!(a.episode_selected, 11);
        a.handle(Action::Top, 5);
        assert_eq!(a.episode_selected, 0);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.episode_selected, 5);
        a.handle(Action::PageUp, 5);
        assert_eq!(a.episode_selected, 0);
        assert_eq!((a.selected, a.offset), (0, 0), "the list under the table stays put");
    }

    #[test]
    fn the_filter_keeps_the_same_episode_under_the_cursor() {
        let mut a = app_at_episodes();
        for n in ["1", "2", "3"] {
            a.set_episode_watched(n, true);
        }
        a.apply_episode_filter();
        let under = |a: &App| a.episodes[a.episode_selected].number.clone();

        a.episode_selected = 4;
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::Unwatched);
        assert_eq!(under(&a), "5", "the cursor kept its row index, not its episode");
        a.handle(Action::Filter, 5);
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::All);
        assert_eq!(under(&a), "5");

        // An episode the filter hides hands the cursor to the next one it keeps.
        a.episode_selected = 1;
        a.handle(Action::Filter, 5);
        assert_eq!(under(&a), "4");
    }

    #[test]
    fn settings_and_accounts_move_their_own_rows_and_act_on_no_title() {
        // With Continue loaded before them, `G` landed on the row matching that list's length
        // and the title keys opened overlays for one of its shows.
        let mut a = home_list(5);
        a.handle(Action::JumpSection(9), 5);
        a.handle(Action::Bottom, 5);
        assert_eq!(a.selected, SettingId::ALL.len() - 1);
        a.handle(Action::Top, 5);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.selected, 5);
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
        a.apply(Update::Sync(Box::new(SyncState::new("anilist"))));
        a.apply(Update::Sync(Box::new(SyncState::new("mal"))));
        a.handle(Action::Bottom, 5);
        assert_eq!(a.selected, 1);
        a.handle(Action::PageDown, 5);
        assert_eq!(a.selected, 1, "past the last tracker");
        for action in title_keys.into_iter().chain([Action::PlayNext]) {
            assert_eq!(a.handle(action, 5), None, "{action:?}");
            assert_eq!(a.nav.overlay(), None, "{action:?}");
            assert_eq!(a.nav.current(), &StageView::Section(Section::Accounts), "{action:?}");
        }
    }

    #[test]
    fn search_with_nothing_typed_shows_its_own_empty_state() {
        // Jumped to, it showed the previous section's list; walked to on the rail, a pulse
        // that never ended, because the fetch that would end it never went out.
        let mut a = home_list(5);
        assert_eq!(a.handle(Action::JumpSection(4), 5), None);
        assert_eq!(a.content, Content::Empty);
        assert!(a.target().is_none());

        let mut a = home_list(5);
        a.handle(Action::JumpSection(7), 5);
        a.nav.focus_rail();
        for _ in 0..3 {
            a.handle(Action::Up, 5);
        }
        assert_eq!(a.nav.section(), Section::Search);
        for _ in 0..=RELOAD_IDLE_TICKS {
            a.tick_pending_reload();
        }
        assert_eq!(a.content, Content::Empty);
        assert_eq!(a.take_pending(), None);
    }

    #[test]
    fn down_from_the_search_field_moves_into_results_that_scroll() {
        let mut a = app();
        a.handle(Action::JumpSection(4), 5);
        type_text(&mut a, "query");
        a.handle(Action::Open, 5);
        a.apply(a.content_here(entries(40)));
        assert!(a.is_typing());

        a.handle(Action::Down, 5);
        assert!(!a.is_typing(), "the results have the keyboard");
        assert_eq!(a.selected, 0);
        for _ in 0..12 {
            a.handle(Action::Down, 5);
        }
        assert_eq!(a.selected, 12);
        assert!(a.offset > 0 && a.selected < a.offset + 5, "the window follows");
        a.handle(Action::Bottom, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.selected, 39, "bounded at the last result");
        // A result is a list row like any other.
        assert_eq!(
            a.handle(Action::Download, 5),
            Some(Task::DownloadEpisode { id: AnilistId::new(40), episode: "1".into() })
        );

        // Back to typing: Esc, ↑ from the first result, or `/`, which does not ask again.
        a.handle(Action::Back, 5);
        assert!(a.is_typing());
        a.handle(Action::Down, 5);
        a.handle(Action::Top, 5);
        a.handle(Action::Up, 5);
        assert!(a.is_typing());
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::FocusSearch, 5), None);
        assert!(a.is_typing());
        assert_eq!(a.content.len(), 40);
    }

    #[test]
    fn a_shorter_window_brings_the_selection_back_into_view() {
        let mut a = home_list(50);
        for _ in 0..30 {
            a.handle(Action::Down, 40);
        }
        assert_eq!((a.selected, a.offset), (30, 0));
        a.fit_to(10);
        assert!(a.selected >= a.offset && a.selected < a.offset + 10, "offset {}", a.offset);

        // And a taller one shows more of the list rather than empty rows under its end.
        a.handle(Action::Bottom, 10);
        assert_eq!(a.offset, 40);
        a.fit_to(30);
        assert_eq!(a.offset, 20);
    }

    #[test]
    fn the_status_picker_opens_on_the_titles_status_not_the_tab_last_viewed() {
        let mut a = home_list(3);
        // List status lives on a tracker, so one is signed in.
        a.apply(Update::Sync(Box::new(connected(0))));
        a.library_segment = LibrarySegment::Dropped;
        a.handle(Action::SetListStatus, 5);
        assert_eq!(a.overlay_selected, 0, "nothing known about it, so the top");
        a.overlay_selected = 3;
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Task::SetStatus { id: AnilistId::new(1), status: LibrarySegment::Paused })
        );
        a.handle(Action::SetListStatus, 5);
        assert_eq!(a.overlay_selected, 3, "the answer just given");
    }

    #[test]
    fn space_and_x_pause_and_cancel_the_highlighted_download() {
        // Idle they arrived as play-next and remove-from-Continue, so nothing in the queue
        // could be paused or cancelled; playing, they paused the player instead.
        let download = |id: i64| DownloadRow {
            id,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: id.to_string(),
            state: "downloading",
            fraction: 0.5,
            downloaded: 1,
            total: 2,
            error: None,
            path: None,
        };
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Update::Downloads(vec![download(7), download(8)]));
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::PlayNext, 5), Some(Task::DownloadPause { id: 8 }));
        assert_eq!(a.handle(Action::HideFromContinue, 5), Some(Task::DownloadCancel { id: 8 }));

        a.playing = Some(NowPlaying::default());
        assert_eq!(a.handle(Action::PlayPause, 5), Some(Task::DownloadPause { id: 8 }));
        assert_eq!(a.handle(Action::StopPlayback, 5), Some(Task::DownloadCancel { id: 8 }));
    }

    #[test]
    fn x_on_a_finished_download_keeps_it() {
        // Cancelling a finished row forgot it and left its file behind, unreachable from here.
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Update::Downloads(vec![DownloadRow {
            id: 3,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: "1".into(),
            state: "done",
            fraction: 1.0,
            downloaded: 2,
            total: 2,
            error: None,
            path: Some("/tmp/title-0-1.mkv".into()),
        }]));
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert!(
            a.toasts.iter().any(|t| t.text == "already downloaded — X deletes it and its file"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_digit_behind_help_on_now_playing_stays_with_the_player() {
        // `9` is volume on Now Playing; behind Help it closed Help and went to Settings.
        let mut a = home_list(3);
        let _ = a.begin_playback(AnilistId::new(1), "1".into());
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        for overlay in [Action::Help, Action::ShowLogs] {
            a.handle(overlay, 5);
            let nine = KeyEvent::new(KeyCode::Char('9'), KeyModifiers::NONE);
            assert_eq!(a.press(nine, 5), None);
            assert!(a.nav.has_overlay(), "{overlay:?} must stay open");
            assert_eq!(a.nav.current(), &StageView::NowPlaying);
            a.handle(Action::Back, 5);
        }
    }

    #[test]
    fn what_rides_the_detail_fetch_is_pointed_at_the_title_page_from_a_list() {
        // A row never opened has no relations, recommendations or trailer to go on, so
        // "none known for this title" there was a guess. `m` and `P` already say where to go.
        let mut a = home_list(3);
        let last = |a: &App| a.toasts.last().map(|t| t.text.clone()).unwrap_or_default();
        for (action, said) in [
            (Action::WatchOrder, "connected seasons are listed from the title's page"),
            (Action::Recommendations, "recommendations are listed from the title's page"),
            (Action::PlayTrailer, "the trailer is found from the title's page"),
        ] {
            assert_eq!(a.handle(action, 5), None);
            assert_eq!(last(&a), said);
        }
        // On the page itself, empty is the answer.
        a.handle(Action::Open, 5);
        a.handle(Action::WatchOrder, 5);
        assert_eq!(last(&a), "no connected seasons known for this title");
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
    fn left_on_a_title_goes_back_rather_than_stranding_it_behind_the_rail() {
        // `h` focused the rail over the title page, and the next `j` switched section and
        // threw the whole stack away.
        let mut a = home_list(5);
        a.handle(Action::Open, 5);
        a.handle(Action::Left, 5);
        assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
        assert_eq!(a.nav.focus(), Focus::Stage);
        a.handle(Action::Down, 5);
        assert_eq!((a.nav.section(), a.selected), (Section::Home, 1));
        a.handle(Action::Left, 5);
        assert_eq!(a.nav.focus(), Focus::Rail);
    }

    #[test]
    fn left_reaches_the_rail_from_library_and_settings() {
        let mut a = app();
        a.handle(Action::JumpSection(5), 5);
        a.library_segment = LibrarySegment::Planning;
        assert_eq!(
            a.handle(Action::Left, 5),
            Some(Task::LoadLibrary(LibrarySegment::Watching))
        );
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);

        a.handle(Action::JumpSection(9), 5);
        a.selected = setting_row(SettingId::Motion);
        a.config.theme.motion = true;
        assert_eq!(a.handle(Action::Right, 5), None, "a switch that is on stays on");
        assert!(a.config.theme.motion);
        assert!(matches!(
            a.handle(Action::Left, 5),
            Some(Task::SaveSetting {
                value: anistream_core::settings::SettingValue::Bool(false),
                ..
            })
        ));
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);
        assert!(!a.config.theme.motion, "reaching the rail changed nothing");

        // A ladder on its bottom rung and a typed value have nothing further left either.
        for (row, prepare) in [
            (
                SettingId::Quality,
                (|a: &mut App| a.config.playback.quality = 480) as fn(&mut App),
            ),
            (SettingId::Subtitles, |_| {}),
        ] {
            a.nav.focus_stage();
            a.selected = setting_row(row);
            prepare(&mut a);
            assert_eq!(a.handle(Action::Left, 5), None, "{row:?}");
            assert_eq!(a.nav.focus(), Focus::Rail, "{row:?}");
        }
        assert_eq!(a.config.playback.quality, 480);
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
    fn finishing_an_episode_updates_the_table_without_a_reload() {
        // Reported from real use: the episode table still showed the old state after an episode
        // finished, so it looked as though nothing had been recorded.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let id = a.selected_entry().expect("an entry").id;
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        // Push the view first: it clears any previous title's rows, which is the point.
        a.handle(Action::ShowEpisodes, 10);
        a.apply(a.episodes_here(vec![
            EpisodeRow {
                number: "1".into(),
                title: None,
                duration_secs: Some(1440),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                thumbnail: None,
                description: None,
            },
            EpisodeRow {
                number: "2".into(),
                title: None,
                duration_secs: Some(1440),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                thumbnail: None,
                description: None,
            },
        ]));

        a.apply(Update::Playback { position: 0.0, duration: Some(1440.0), paused: false });
        // Nothing is playing yet, so that update is correctly ignored rather than inventing state.
        assert_eq!(a.episodes[0].watched, 0.0);

        a.begin_playback(id, "1".into());

        // Halfway through, then detached: the row must show where you actually are.
        a.apply(Update::Playback { position: 720.0, duration: Some(1440.0), paused: false });
        assert!((a.episodes[0].watched - 0.5).abs() < 1e-6, "got {}", a.episodes[0].watched);
        assert!(!a.episodes[0].completed, "halfway is not finished");

        a.apply(finished(None));
        assert!(a.episodes[0].completed, "a finished episode must show as finished");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6);
        assert_eq!(a.episodes[1].watched, 0.0, "only the episode watched should change");
    }

    /// Reported from real use: an episode finished, the toast said so, and its row stayed empty
    /// and unmarked until the show was left and re-entered. Reaching the table with `e` rather
    /// than by opening the title left the app with no idea which title was on screen, so the
    /// watch had nowhere to land — and auto-next, asking the same question, stayed put.
    #[test]
    fn reaching_episodes_by_key_still_lands_a_finished_watch_on_the_table() {
        let mut a = app();
        a.config.playback.auto_next = true;
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let id = a.selected_entry().expect("an entry").id;
        // Straight from the list — no detail screen in between, which is the whole point.
        a.handle(Action::ShowEpisodes, 10);
        a.apply(
            a.episodes_here(
                (1..=3)
                    .map(|n| EpisodeRow {
                        number: n.to_string(),
                        title: None,
                        duration_secs: Some(1440),
                        watched: 0.0,
                        completed: false,
                        kind: None,
                        skippable: false,
                        thumbnail: None,
                        description: None,
                    })
                    .collect(),
            ),
        );

        a.begin_playback(id, "1".into());
        a.apply(finished(Some("2")));

        assert!(a.episodes[0].completed, "the finished episode must be marked in the table");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6, "and its meter filled");
        assert_eq!(
            a.pending,
            Some(Task::Play { id, episode: "2".into() }),
            "auto-next follows the episode that just played"
        );
    }

    /// Reported from real use: after viewing one show's episodes, every other show opened on
    /// that first one's list. `detail` outlives the screen that sets it, so reading it ahead of
    /// the cursor pinned the table to whatever was seen first — until the section was left and
    /// re-entered, which is the one thing that clears it.
    #[test]
    fn each_show_in_a_list_opens_its_own_episodes() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();

        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Task::LoadEpisodes(AnilistId::new(1))),
            "the selected row"
        );
        a.handle(Action::Back, 10);
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Task::LoadEpisodes(AnilistId::new(2))),
            "the next row, not the one already seen"
        );

        // From a title screen the subject is that title, whatever the list beneath is on.
        a.handle(Action::Back, 10);
        a.detail = Some(Entry::new(AnilistId::new(99), "a related title"));
        a.nav.push(StageView::Title(AnilistId::new(99)));
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Task::LoadEpisodes(AnilistId::new(99))),
            "a title screen speaks for itself"
        );
    }

    #[test]
    fn opening_episodes_never_shows_the_previous_title_s_rows() {
        // Reported from real use: the table displayed the last title's episodes — with their watch
        // progress — until the new load answered, then corrected itself. Stale content presented as
        // current is worse than an honest wait.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        a.apply(a.episodes_here(vec![EpisodeRow {
            number: "7".into(),
            title: Some("from the first title".into()),
            duration_secs: Some(1440),
            watched: 0.9,
            completed: true,
            kind: None,
            skippable: false,
            thumbnail: None,
            description: None,
        }]));
        assert_eq!(a.episodes.len(), 1);
        assert!(!a.episodes_loading);

        // Back out and into a different title.
        a.handle(Action::Back, 10);
        a.handle(Action::Down, 10);
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        let task = a.handle(Action::ShowEpisodes, 10);

        assert!(a.episodes.is_empty(), "the old rows must be gone before the screen appears");
        assert!(a.episodes_loading, "and the wait must be visible as a wait");
        assert!(matches!(task, Some(Task::LoadEpisodes(_))));
    }

    #[test]
    fn a_source_with_no_episodes_reads_differently_from_one_still_loading() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        assert!(a.episodes_loading);
        // An empty answer is still an answer, and must clear the wait.
        a.apply(a.episodes_here(Vec::new()));
        assert!(!a.episodes_loading, "an empty result is not a permanent loading state");
    }

    #[test]
    fn a_re_watch_quit_early_does_not_erase_that_it_was_finished() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));
        let id = a.selected_entry().expect("an entry").id;
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.apply(a.episodes_here(vec![EpisodeRow {
            number: "1".into(),
            title: None,
            duration_secs: Some(1440),
            watched: 1.0,
            completed: true,
            kind: None,
            skippable: false,
            thumbnail: None,
            description: None,
        }]));
        a.begin_playback(id, "1".into());
        a.apply(Update::Playback { position: 60.0, duration: Some(1440.0), paused: false });
        assert!(a.episodes[0].completed, "still finished");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6, "progress is monotonic");
    }

    #[test]
    fn enter_opens_a_search_result_instead_of_re_running_the_search() {
        // Reported from real use: it was impossible to open a result. Enter submitted the query
        // every time, so a highlighted row could never be entered.
        let mut a = app();
        let task = a.handle(Action::FocusSearch, 10);
        assert_eq!(task, None, "an empty query has nothing to submit");
        for ch in "frieren".chars() {
            a.type_char(ch);
        }

        // First Enter runs it.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Task::Search { query: "frieren".into(), filter: SearchFilters::default() })
        );
        a.apply(a.content_here(entries(3)));

        // Second Enter opens the highlighted result rather than asking again.
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Task::LoadDetail(_))), "got {task:?}");
        assert!(matches!(a.nav.current(), StageView::Title(_)));

        // Editing the query makes it submittable again.
        a.handle(Action::Back, 10);
        a.type_char('x');
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Task::Search { query: "frierenx".into(), filter: SearchFilters::default() })
        );
    }

    #[test]
    fn search_filters_cycle_and_apply_without_a_query() {
        let mut a = app();
        a.handle(Action::FilterSearch, 10);
        assert_eq!(a.nav.section(), Section::Search, "filters land on the search screen");
        assert_eq!(a.nav.overlay(), Some(&Overlay::SearchFilters));

        // Right on the genre row picks the first genre; a filter alone is a browse.
        a.handle(Action::Right, 10);
        assert_eq!(a.search_filter.genre.as_deref(), Some("Action"));
        let task = a.handle(Action::Open, 10);
        match task {
            Some(Task::Search { query, filter }) => {
                assert_eq!(query, "");
                assert_eq!(filter.genre.as_deref(), Some("Action"));
            }
            other => panic!("expected a search, got {other:?}"),
        }
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn search_filters_step_back_to_any_rather_than_sticking() {
        let mut a = app();
        a.handle(Action::FilterSearch, 10);

        a.handle(Action::Right, 10);
        a.handle(Action::Left, 10);
        assert_eq!(a.search_filter.genre, None, "left from the first option is back to any");

        // Both empty: Enter closes the overlay and asks nothing.
        assert_eq!(a.handle(Action::Open, 10), None);
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn search_filters_open_mid_query_without_stealing_the_letters() {
        // ctrl+f is the whole reason FilterSearch is its own action: `f` belongs to the
        // query text while the search box has focus.
        let mut a = app();
        a.handle(Action::FocusSearch, 10);
        for ch in "mecha".chars() {
            a.type_char(ch);
        }
        assert!(a.is_typing());
        a.handle(Action::FilterSearch, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::SearchFilters));
        assert_eq!(a.search_query, "mecha", "the query survives opening the overlay");
    }

    #[test]
    fn the_filter_key_outside_search_and_episodes_says_where_filters_live() {
        let mut a = app();
        a.handle(Action::Filter, 10);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.text.contains("filters live")));
    }

    #[test]
    fn accounts_is_a_screen_with_a_cursor_and_enter_acts_on_the_row() {
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        a.apply(Update::Sync(Box::new(SyncState::new("mal"))));

        a.handle(Action::ShowAccounts, 10);
        assert_eq!(a.nav.section(), Section::Accounts, "accounts is a section, not a modal");
        assert!(!a.nav.has_overlay());

        a.nav.focus_stage();
        // A connected tracker signs out; an unconnected one signs in.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Task::Disconnect { tracker: "anilist".into() })
        );
        a.handle(Action::Down, 10);
        assert_eq!(a.handle(Action::Open, 10), Some(Task::Connect { tracker: "mal".into() }));
        // Clamped at the last row rather than running off the end.
        for _ in 0..10 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.selected, 1);
    }

    #[test]
    fn signing_out_is_reflected_the_moment_it_happens() {
        // Clearing the stored token used to be the whole of signing out, so the badge and the
        // account list went on showing a connected account until the next restart.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        assert!(a.sync[0].connected);

        a.apply(Update::Sync(Box::new(SyncState::new("anilist"))));
        assert!(!a.sync[0].connected, "the UI must agree immediately");
        assert_eq!(a.sync.len(), 1, "the tracker is replaced, not duplicated");
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
    fn the_palette_can_be_moved_through_and_run() {
        // It could filter but never pick: the arrows moved the list behind it and Enter closed
        // it without running anything, so the app's discoverability mechanism was a dead end.
        let mut a = app();
        assert!(a.handle(Action::CommandPalette, 10).is_none());
        assert!(a.is_typing(), "the palette takes text");

        let first = a.palette_matches().first().cloned().expect("some actions match");
        a.handle(Action::Down, 10);
        assert_eq!(a.overlay_selected, 1, "down moves within the palette");
        a.handle(Action::Up, 10);
        assert_eq!(a.overlay_selected, 0);

        // Typing resets the cursor, since filtering reorders the matches under it.
        a.handle(Action::Down, 10);
        a.type_char('s');
        assert_eq!(a.overlay_selected, 0, "a filtered list must not keep a stale index");

        a.backspace();
        a.handle(Action::Open, 10);
        assert_ne!(
            a.nav.overlay(),
            Some(&Overlay::CommandPalette),
            "Enter must dismiss the palette"
        );
        assert!(a.palette_query.is_empty(), "the query must not persist into the next open");
        // Whatever ran, it was the row under the cursor rather than nothing at all.
        assert_ne!(first.0, Action::CommandPalette, "sanity: the fixture is a real action");
    }

    #[test]
    fn the_palette_actually_performs_the_selected_action() {
        let mut a = app();
        a.handle(Action::CommandPalette, 10);
        a.palette_query = "keys".into();
        let matched = a.palette_matches();
        assert!(
            matched.iter().any(|(action, _)| *action == Action::Help),
            "expected the help action to match 'keys', got {matched:?}"
        );
        a.overlay_selected =
            matched.iter().position(|(action, _)| *action == Action::Help).unwrap();
        a.handle(Action::Open, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Help), "the chosen action must run");
    }

    #[test]
    fn a_toast_already_up_is_refreshed_rather_than_counted_twice() {
        let mut a = app();
        for text in ["saved", "saved", "queued", "saved"] {
            a.push_toast(Toast::info(text));
        }
        let up: Vec<&str> = a.toasts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(up, ["queued", "saved"], "one of each, the repeat newest");
        // The log keeps every arrival, a run of repeats as one counted line.
        let logged: Vec<(&str, u32)> =
            a.logs.iter().map(|l| (l.text.as_str(), l.count)).collect();
        assert_eq!(logged, [("saved", 2), ("queued", 1), ("saved", 1)]);
    }

    #[test]
    fn a_toast_is_also_written_to_the_log() {
        // The toast stack is capped at three and each lives for seconds; when a provider chain
        // fails the *first* error is usually the one that explains the rest.
        let mut a = app();
        for i in 0..5 {
            a.push_toast(Toast::alert(format!("failure {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "the stack is still capped");
        assert_eq!(a.logs.len(), 5, "but nothing was lost");
        assert!(a.logs[0].text.contains("failure 0"));

        a.handle(Action::ShowLogs, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Logs));
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
    fn the_settings_screen_steps_an_off_ladder_quality_up_not_down() {
        use anistream_core::settings::SettingValue;
        let mut config = Config::default();
        config.playback.quality = 1440;
        config.playback.commit_threshold = 0.97;
        let mut a = App::new(config, Palette::dark(), Keymap::new());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        a.selected = SettingId::ALL.iter().position(|id| *id == SettingId::Quality).unwrap();
        let task = a.handle(Action::Right, 10);
        assert_eq!(a.config.playback.quality, 2160);
        assert!(matches!(task, Some(Task::SaveSetting { value: SettingValue::Int(2160), .. })));

        a.selected =
            SettingId::ALL.iter().position(|id| *id == SettingId::CommitThreshold).unwrap();
        a.handle(Action::Right, 10);
        assert_eq!(a.config.playback.commit_threshold, 0.97, "→ never lowers it");
        a.handle(Action::Left, 10);
        assert_eq!(a.config.playback.commit_threshold, 0.95);
    }

    fn setting_text(a: &App, id: SettingId) -> String {
        let at = SettingId::ALL.iter().position(|row| *row == id).unwrap();
        a.setting_rows()[at].value.clone()
    }

    fn settings_at(id: SettingId) -> App {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected = SettingId::ALL.iter().position(|row| *row == id).unwrap();
        a
    }

    #[test]
    fn a_refused_settings_write_puts_the_setting_back() {
        // The value changed before the write and nothing undid it, so a refused write — torrents
        // on with no VPN configured — went on showing "on" and the status line the new value.
        let mut a = settings_at(SettingId::Torrents);
        let Some(Task::SaveSetting { table, key, .. }) = a.handle(Action::Right, 10) else {
            panic!("no write asked for")
        };
        assert!(a.config.providers.torrent.enabled, "the screen answers at once");
        a.status = "saved quality".into();

        a.apply(Update::SettingWritten {
            on_disk: Box::new(Config::default()),
            failed: Some((table, key)),
        });
        assert!(!a.config.providers.torrent.enabled, "back to what the file holds");
        assert_eq!(setting_text(&a, SettingId::Torrents), "off");
        assert!(a.status.is_empty(), "the status line shows what is in effect again");
    }

    #[test]
    fn a_rollback_leaves_other_settings_alone() {
        // A second setting changed while the first write was refused has its own write, which
        // landed; undoing everything would undo that too.
        let mut a = settings_at(SettingId::Quality);
        a.handle(Action::Right, 10);
        let mut on_disk = Config::default();
        on_disk.playback.quality = 2160;
        a.selected = SettingId::ALL.iter().position(|row| *row == SettingId::Torrents).unwrap();
        a.handle(Action::Right, 10);

        a.apply(Update::SettingWritten {
            on_disk: Box::new(on_disk),
            failed: Some((&["providers", "torrent"], "enabled")),
        });
        assert_eq!(a.config.playback.quality, 2160);
        assert!(!a.config.providers.torrent.enabled);
    }

    #[test]
    fn sub_or_dub_for_the_session_shows_beside_the_saved_choice() {
        // `t` is for this session, and Settings showed it as if saved while the file and the
        // next launch said otherwise.
        let mut a = app();
        a.handle(Action::ToggleTranslation, 10);
        assert_eq!(setting_text(&a, SettingId::Translation), "sub · dub this session");
        assert!(a.toasts.last().unwrap().text.contains("for this session"));

        // The arrows change the saved value, the one the row leads with.
        let mut a = settings_at(SettingId::Translation);
        a.handle(Action::ToggleTranslation, 10);
        let task = a.handle(Action::Right, 10);
        assert!(matches!(
            task,
            Some(Task::SaveSetting { key: "translation", value: anistream_core::settings::SettingValue::Str(ref v), .. }) if v == "dub"
        ));
        let mut on_disk = Config::default();
        on_disk.playback.translation = anistream_core::media::Translation::Dub;
        a.apply(Update::SettingWritten { on_disk: Box::new(on_disk), failed: None });
        assert_eq!(setting_text(&a, SettingId::Translation), "dub");
    }

    #[test]
    fn a_theme_from_the_command_line_shows_as_this_session_only() {
        use anistream_core::config::ThemeMode;
        let mut config = Config::default();
        config.theme.mode = ThemeMode::Immersive;
        let mut a = App::new(config, Palette::dark(), Keymap::new());
        a.saved_config.theme.mode = ThemeMode::Adaptive;
        assert_eq!(setting_text(&a, SettingId::Theme), "adaptive · immersive this session");
    }

    #[test]
    fn settings_rows_cycle_and_ask_to_be_persisted() {
        use anistream_core::settings::SettingValue;
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        // Quality steps up the ladder and clamps, rather than wrapping round to 480p.
        let quality_row =
            SettingId::ALL.iter().position(|id| *id == SettingId::Quality).unwrap();
        a.selected = quality_row;
        assert_eq!(a.config.playback.quality, 1080);
        let task = a.handle(Action::Right, 10);
        assert_eq!(a.config.playback.quality, 2160, "the change must apply immediately");
        assert_eq!(
            task,
            Some(Task::SaveSetting {
                table: &["playback"],
                key: "quality",
                value: SettingValue::Int(2160),
            })
        );
        a.handle(Action::Right, 10);
        assert_eq!(a.config.playback.quality, 2160, "clamped at the top of the ladder");
        a.handle(Action::Left, 10);
        assert_eq!(a.config.playback.quality, 1080);

        // And the rows that cannot be edited here say so instead of silently ignoring the key.
        let vpn_row = SettingId::ALL.iter().position(|id| *id == SettingId::VpnMode).unwrap();
        a.selected = vpn_row;
        let before = format!("{:?}", a.config.providers.torrent.vpn.mode);
        assert_eq!(a.handle(Action::Right, 10), None);
        assert_eq!(format!("{:?}", a.config.providers.torrent.vpn.mode), before);
        assert!(
            a.toasts.iter().any(|t| t.text.contains("config.toml")),
            "a read-only row must explain itself"
        );
    }

    #[test]
    fn seeding_is_set_from_the_settings_screen() {
        use anistream_core::settings::SettingValue;
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        let at = |id| SettingId::ALL.iter().position(|row| *row == id).unwrap();

        // Off by default; one step right turns it on, at a modest ratio.
        a.selected = at(SettingId::SeedRatio);
        assert_eq!(a.config.providers.torrent.seeding.ratio, 0.0);
        assert_eq!(
            a.handle(Action::Right, 10),
            Some(Task::SaveSetting {
                table: &["providers", "torrent", "seeding"],
                key: "ratio",
                value: SettingValue::Float(0.5),
            })
        );
        assert!(a.config.providers.torrent.seeding.enabled());

        a.selected = at(SettingId::SeedHours);
        assert_eq!(
            a.handle(Action::Right, 10),
            Some(Task::SaveSetting {
                table: &["providers", "torrent", "seeding"],
                key: "hours",
                value: SettingValue::Int(72),
            })
        );

        a.selected = at(SettingId::UploadLimit);
        let rows = a.setting_rows();
        assert_eq!(rows[a.selected].value, "unlimited");
        a.handle(Action::Right, 10);
        assert_eq!(a.setting_rows()[a.selected].value, "256 KB/s");
        for _ in 0..3 {
            a.handle(Action::Right, 10);
        }
        assert_eq!(a.setting_rows()[a.selected].value, "2 MB/s");
    }

    #[test]
    fn discord_presence_can_be_toggled_from_settings() {
        use anistream_core::settings::SettingValue;
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected = SettingId::ALL.iter().position(|id| *id == SettingId::Presence).unwrap();

        assert!(!a.config.presence.enabled, "off by default: it publishes to a third party");
        let task = a.handle(Action::Open, 10);
        assert!(a.config.presence.enabled);
        assert_eq!(
            task,
            Some(Task::SaveSetting {
                table: &["presence"],
                key: "enabled",
                value: SettingValue::Bool(true),
            })
        );
        a.handle(Action::Open, 10);
        assert!(!a.config.presence.enabled, "and back off again");
    }

    #[test]
    fn enabling_presence_without_a_client_id_says_so() {
        // A toggle that flips and then does nothing is worse than one that explains itself.
        let mut a = app();
        a.go_to_section(Section::Settings);
        let rows = a.setting_rows();
        let row = rows
            .iter()
            .find(|r| r.label == SettingId::Presence.label())
            .expect("the presence row");
        assert!(row.editable.is_some(), "it must still be toggleable");
        // A client id ships by default, so the row says when the change applies rather than what is
        // missing. A Discord id is public, so there is nothing to withhold.
        assert!(row.note.is_some_and(|n| n.contains("next episode")), "got {:?}", row.note);

        // Blanking the field must still resolve, via the shipped default — otherwise clearing it
        // instead of deleting the line would silently disable the feature.
        a.config.presence.client_id = Some(String::new());
        assert!(a.config.presence.resolved_client_id().is_some());
    }

    #[test]
    fn the_settings_cursor_is_not_clamped_by_an_empty_content_list() {
        // Settings is built from `SettingId::ALL`, not from `content`, so the shared clamp would
        // have pinned the cursor to the first row and made everything below it unreachable.
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        for _ in 0..3 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.selected, 3);
        for _ in 0..50 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.selected, SettingId::ALL.len() - 1, "clamped at the last row");
    }

    #[test]
    fn opening_an_entry_pushes_the_title_view_and_asks_for_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Task::LoadDetail(AnilistId::new(1))));
        assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(1)));
        // The already-known entry is shown immediately so the screen is never blank while
        // the detail request is in flight.
        assert!(a.detail.is_some());
    }

    #[test]
    fn typing_suppresses_single_key_bindings() {
        // Otherwise "d" in a search box would trigger a download.
        let mut a = app();
        a.go_to_section(Section::Search);
        assert!(a.is_typing());

        a.handle(Action::Download, 10);
        assert!(a.toasts.is_empty(), "a binding fired while typing");

        a.type_char('f');
        a.type_char('r');
        assert_eq!(a.search_query, "fr");
    }

    #[test]
    fn the_command_palette_captures_typing_over_the_search_box() {
        let mut a = app();
        a.go_to_section(Section::Search);
        a.handle(Action::CommandPalette, 10);
        a.type_char('s');
        assert_eq!(a.palette_query, "s");
        assert!(a.search_query.is_empty(), "text went to the wrong field");
    }

    #[test]
    fn escape_while_typing_closes_the_overlay_rather_than_navigating() {
        let mut a = app();
        a.handle(Action::CommandPalette, 10);
        a.handle(Action::Back, 10);
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn backspace_edits_the_active_field_only() {
        let mut a = app();
        a.go_to_section(Section::Search);
        a.type_char('a');
        a.type_char('b');
        a.backspace();
        assert_eq!(a.search_query, "a");
        a.backspace();
        a.backspace();
        assert_eq!(a.search_query, "", "must not underflow");
    }

    #[test]
    fn switching_section_clears_stale_selection_and_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(10)));
        a.handle(Action::Bottom, 5);
        a.handle(Action::Open, 5);

        a.go_to_section(Section::Seasonal);
        assert_eq!(a.selected, 0);
        assert!(a.detail.is_none(), "stale detail from another section");
        assert_eq!(a.nav.depth(), 1);
    }

    /// Started long enough ago to be shown.
    fn aged(a: &mut App, id: JobId, by: Duration) {
        let job = a.jobs.iter_mut().find(|j| j.id == id).expect("a running job");
        job.started = Instant::now().checked_sub(by).expect("a clock that far back");
    }

    #[test]
    fn only_work_someone_waits_on_becomes_a_job() {
        let mut a = app();
        assert!(a.begin_job(&Task::LoadSeasonal).is_some());
        assert!(
            a.begin_job(&Task::LoadSources { id: AnilistId::new(1), episode: "3".into() })
                .is_some()
        );
        // Outbox writes and player commands finish as they are asked; a row for them would only
        // ever flash.
        assert!(a.begin_job(&Task::SetScore { id: AnilistId::new(1), score: 8.0 }).is_none());
        assert!(a.begin_job(&Task::Player(PlayerCommand::TogglePause)).is_none());
        assert_eq!(a.jobs.len(), 2);
    }

    #[test]
    fn a_job_runs_until_its_work_says_it_ended() {
        let mut a = app();
        let id = a.begin_job(&Task::CheckProviders).unwrap();
        a.apply(Update::JobNote { job: id, note: "AniList limit".into(), until: None });
        assert_eq!(a.jobs[0].note.as_ref().map(|(n, _)| n.as_str()), Some("AniList limit"));

        a.apply(Update::JobDone(id));
        assert!(a.jobs.is_empty());
        // A second end, or one for a job long gone, is harmless.
        a.apply(Update::JobDone(id));
    }

    #[test]
    fn work_earns_the_row_only_once_it_has_taken_a_while() {
        let mut a = app();
        let older = a.begin_job(&Task::LoadCalendar).unwrap();
        let newer = a.begin_job(&Task::CheckProviders).unwrap();
        assert!(a.activity(Instant::now()).is_empty(), "quick answers never show");

        aged(&mut a, older, Duration::from_secs(5));
        aged(&mut a, newer, Duration::from_secs(1));
        let shown: Vec<_> = a.activity(Instant::now()).iter().map(|j| j.id).collect();
        assert_eq!(shown, [newer, older], "the thing just asked for leads");
    }

    // Requests are never cancelled, so every answer below is one the app really receives when
    // the user moves faster than the network: it has to be recognised as late and left alone.

    #[test]
    fn a_late_list_for_the_section_just_left_is_not_shown_under_the_next() {
        let mut a = app();
        let _ = a.go_to_section(Section::Seasonal);
        let _ = a.go_to_section(Section::Calendar);
        a.apply(Update::Content { view: ContentFor::Seasonal, content: entries(5) });
        assert!(matches!(a.content, Content::Loading), "the calendar showed seasonal's list");

        a.apply(Update::Content { view: ContentFor::Calendar, content: entries(3) });
        assert_eq!(a.content.len(), 3, "the calendar's own answer must still land");
    }

    #[test]
    fn a_slow_answer_to_an_earlier_search_does_not_replace_a_later_one() {
        let mut a = app();
        let _ = a.go_to_section(Section::Search);
        a.nav.focus_stage();
        for query in ["fri", "frieren"] {
            a.search_query = query.into();
            assert!(matches!(a.handle(Action::Open, 20), Some(Task::Search { .. })));
        }
        let answer = |query: &str, n| Update::Content {
            view: ContentFor::Search { query: query.into(), filter: SearchFilters::default() },
            content: entries(n),
        };
        a.apply(answer("frieren", 2));
        a.apply(answer("fri", 9));
        assert_eq!(
            a.content.len(),
            2,
            "the results for \"fri\" replaced those for \"frieren\""
        );
    }

    #[test]
    fn a_late_list_for_another_library_segment_is_not_shown() {
        let mut a = app();
        let _ = a.go_to_section(Section::Library);
        a.library_segment = LibrarySegment::Completed;
        a.apply(Update::Content {
            view: ContentFor::Library(LibrarySegment::Watching),
            content: entries(4),
        });
        assert!(a.content.is_empty(), "completed showed the watching list");
    }

    #[test]
    fn a_late_record_for_a_title_already_left_is_not_shown_on_the_next() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::Open, 20); // title 1
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20); // title 2
        a.apply(Update::Detail(Box::new(Entry::new(AnilistId::new(1), "Title one, in full"))));
        assert_eq!(a.detail.as_ref().map(|d| d.id), Some(AnilistId::new(2)));

        a.apply(Update::Detail(Box::new(Entry::new(AnilistId::new(2), "Title two, in full"))));
        assert_eq!(a.detail.as_ref().map(|d| d.title.as_str()), Some("Title two, in full"));
    }

    #[test]
    fn late_episodes_for_a_title_already_left_do_not_fill_the_next_ones_table() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        a.handle(Action::ShowEpisodes, 20); // title 1
        a.handle(Action::Back, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::ShowEpisodes, 20); // title 2
        let row = |n: &str| EpisodeRow {
            number: n.into(),
            title: None,
            duration_secs: None,
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            thumbnail: None,
            description: None,
        };
        a.apply(Update::Episodes { id: AnilistId::new(1), rows: vec![row("1")] });
        assert!(a.episodes.is_empty(), "title two's table showed title one's episodes");
        assert!(a.episodes_loading, "title two's own load is still outstanding");

        a.apply(Update::Episodes { id: AnilistId::new(2), rows: vec![row("1"), row("2")] });
        assert_eq!(a.episodes.len(), 2);
    }

    #[test]
    fn a_match_question_about_a_title_already_left_is_not_asked() {
        let mut a = app_at_episodes();
        a.apply(Update::MatchChoices {
            id: AnilistId::new(7),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Something else", 0.6)],
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
    }

    #[test]
    fn releases_for_another_episode_are_not_offered_against_this_one() {
        let mut a = app_at_episodes();
        a.handle(Action::ShowSources, 20);
        let (id, asked) = a.source_context.clone().expect("a pending question");
        a.apply(Update::Sources {
            id,
            episode: format!("{asked}0"),
            candidates: vec![source("[A] Show - 10 (1080p)", true)],
        });
        assert_ne!(
            a.nav.overlay(),
            Some(&Overlay::Sources),
            "offered another episode's releases"
        );

        a.apply(a.sources_here(vec![source("[A] Show - 1 (1080p)", true)]));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Sources));
    }

    #[test]
    fn each_browsable_section_requests_its_own_data() {
        let mut a = app();
        assert_eq!(a.go_to_section(Section::Home), Some(Task::LoadContinue));
        assert_eq!(a.go_to_section(Section::Seasonal), Some(Task::LoadSeasonal));
        assert_eq!(a.go_to_section(Section::Calendar), Some(Task::LoadCalendar));
        // Search with no query has nothing to fetch yet.
        assert_eq!(a.go_to_section(Section::Search), None);
        // Sections with no remote data render a placeholder rather than spinning.
        assert_eq!(a.go_to_section(Section::Settings), None);
    }

    #[test]
    fn loading_state_is_set_before_the_request_goes_out() {
        // The screen must never look empty while data is in flight — an empty list with no
        // explanation is the failure mode this whole design avoids.
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.go_to_section(Section::Home);
        assert_eq!(a.content, Content::Loading);
    }

    #[test]
    fn a_failure_is_content_rather_than_an_empty_list() {
        let mut a = app();
        a.apply(a.content_here(Content::Failed("anilist unreachable".into())));
        assert!(matches!(a.content, Content::Failed(_)));
        assert!(a.content.is_empty());
    }

    #[test]
    fn toasts_expire_and_never_stack_beyond_three() {
        let mut a = app();
        for i in 0..6 {
            a.push_toast(Toast::info(format!("message {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "a burst of failures must not cover the screen");
        assert!(a.toasts[0].text.contains('3'), "oldest are dropped first");

        for _ in 0..200 {
            a.tick_toasts();
        }
        assert!(a.toasts.is_empty(), "toasts must expire");
    }

    #[test]
    fn alerts_stay_up_longer_than_info() {
        assert!(Toast::alert("provider down").ttl > Toast::info("switched").ttl);
    }

    #[test]
    fn watch_order_says_so_when_there_is_nothing_to_show() {
        // Silence would read as the key being broken.
        let mut a = app();
        a.handle(Action::WatchOrder, 10);
        assert_eq!(a.toasts.len(), 1);
        assert!(a.toasts[0].text.contains("nothing selected"));
    }

    #[test]
    fn watch_order_lists_relations_and_a_pick_opens_the_title() {
        let mut a = app_at_episodes();
        if let Some(detail) = a.detail.as_mut() {
            detail.related = vec![RelatedTitle {
                id: AnilistId::new(9),
                title: "Season 2".into(),
                relation: "sequel".into(),
                format: Some("TV".into()),
            }];
        }
        a.handle(Action::WatchOrder, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::WatchOrder));

        let task = a.handle(Action::Open, 10).expect("a pick must open the title");
        assert_eq!(task, Task::LoadDetail(AnilistId::new(9)));
        assert_eq!(a.nav.overlay(), None);
        assert!(
            matches!(a.nav.current(), StageView::Title(id) if *id == AnilistId::new(9)),
            "the picked title's view must be up"
        );
    }

    #[test]
    fn toggling_translation_updates_config_and_says_so() {
        let mut a = app();
        let before = a.config.playback.translation;
        a.handle(Action::ToggleTranslation, 10);
        assert_ne!(a.config.playback.translation, before);
        assert!(a.toasts[0].text.contains("dub"));
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
        assert!(a.toasts.iter().any(|t| t.text == "at the top — Q quits"), "{:?}", a.toasts);
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
            a.toasts.iter().any(|t| t.text == "at the top — ctrl+q quits"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_repeated_toast_is_kept_up_rather_than_stacked() {
        let mut a = app();
        for _ in 0..5 {
            a.push_toast(Toast::info("saved quality"));
        }
        assert_eq!(a.toasts.len(), 1, "one message, said once");
        assert_eq!(a.logs[0].count, 5, "the log still counts every one");
        a.push_toast(Toast::info("saved theme"));
        assert_eq!(a.toasts.len(), 2);
    }

    #[test]
    fn watched_fraction_is_bounded_and_safe_without_data() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        assert_eq!(e.watched_fraction(), 0.0);

        e.episodes = Some(28);
        e.progress = Some((14, 15));
        assert!((e.watched_fraction() - 0.5).abs() < 1e-9);

        // More progress than episodes (a rewatch, or bad remote data) must not exceed 1.
        e.progress = Some((99, 100));
        assert_eq!(e.watched_fraction(), 1.0);

        e.episodes = Some(0);
        assert_eq!(e.watched_fraction(), 0.0, "must not divide by zero");
    }

    #[test]
    fn progress_counts_against_what_is_known() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        e.progress = Some((2, 3));
        assert_eq!(
            e.progress_label().as_deref(),
            Some("2 watched"),
            "nothing to count against"
        );
        assert_eq!(e.watched_fraction(), 0.0);

        // Airing, with no announced length: episode 3 next means 2 are out.
        e.next_episode = Some(3);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 2 aired"));
        assert_eq!(e.watched_fraction(), 1.0, "caught up is a full meter, not an empty one");

        e.episodes = Some(12);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 12"));
        assert!((e.watched_fraction() - 2.0 / 12.0).abs() < 1e-9);
    }

    #[test]
    fn the_next_episode_number_proves_its_predecessor_aired() {
        // Right after a broadcast AniList's `nextAiringEpisode` can already say 9 while
        // the last-aired schedule row still answers 7 — during that window the app named
        // the fresh episode only by its successor. Episode 8 is out, provably.
        let mut e = Entry::new(AnilistId::new(1), "Frieren");
        e.last_aired = Some((7, 7 * 24 * 3600));
        e.next_episode = Some(9);
        assert_eq!(e.latest_aired(), Some((8, None)), "the stale schedule row won");

        // When the sources agree, the schedule row's own timestamp is kept.
        e.last_aired = Some((8, 3600));
        assert_eq!(e.latest_aired(), Some((8, Some(3600))));

        // A premiere still ahead proves nothing aired.
        let mut fresh = Entry::new(AnilistId::new(2), "Not Yet Out");
        fresh.next_episode = Some(1);
        assert_eq!(fresh.latest_aired(), None);
    }
}
