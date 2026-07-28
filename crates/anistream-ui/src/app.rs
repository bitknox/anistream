//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Update`]s, so a slow provider or a large image decode cannot stall a frame.

use anistream_core::{config::Config, ids::AnilistId, traits::SourceCandidate};

use crate::{
    eyecatch::Eyecatch,
    keymap::{Action, Keymap},
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
    Presence,
    PresenceTitle,
    Torrents,
    VpnMode,
    TokenStorage,
}

impl SettingId {
    /// Display order. Grouped by [`Self::category`] — the renderer draws a heading each
    /// time the category changes, so rows of one category must be contiguous here.
    pub const ALL: [Self; 14] = [
        Self::Theme,
        Self::Motion,
        Self::Translation,
        Self::Quality,
        Self::Subtitles,
        Self::CommitThreshold,
        Self::AutoNext,
        Self::SkipOpening,
        Self::SkipFiller,
        Self::Torrents,
        Self::VpnMode,
        Self::Presence,
        Self::PresenceTitle,
        Self::TokenStorage,
    ];

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
            | Self::SkipFiller => "playback",
            Self::Torrents | Self::VpnMode => "sources",
            Self::Presence | Self::PresenceTitle | Self::TokenStorage => "integrations",
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
            Self::Presence => "discord presence",
            Self::PresenceTitle => "presence shows the title",
            Self::Torrents => "torrent source",
            Self::VpnMode => "vpn mode",
            Self::TokenStorage => "token storage",
        }
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
        other => unreachable!("no table path for {other}"),
    }
}

/// Step to the next value in a ladder, clamping at both ends.
///
/// Clamps rather than wraps: arrowing past 2160p round to 480p would be a surprising way to lose
/// your quality preference, and the ends of the ladder are informative.
fn step_through<T: Copy + PartialEq>(ladder: &[T], current: T, delta: isize) -> T {
    let at = ladder.iter().position(|v| *v == current).unwrap_or(0) as isize;
    let next = (at + delta.signum()).clamp(0, ladder.len() as isize - 1) as usize;
    ladder[next]
}

/// Idle ticks a rail change waits before fetching. The idle tick is 100 ms, so this is ~300 ms —
/// long enough to coalesce a held arrow key, short enough not to feel like lag.
const RELOAD_IDLE_TICKS: u8 = 3;

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
    /// Seconds until the next broadcast.
    pub airing_in: Option<i64>,
    /// The episode that next broadcast will be.
    pub next_episode: Option<u32>,
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
            next_episode: None,
            last_aired: None,
            resume: None,
            related: Vec::new(),
        }
    }

    /// Fraction watched, for the progress meter.
    pub fn watched_fraction(&self) -> f64 {
        match (self.progress, self.episodes) {
            (Some((done, _)), Some(total)) if total > 0 => {
                (f64::from(done) / f64::from(total)).clamp(0.0, 1.0)
            }
            _ => 0.0,
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

    /// The next episode number, for auto-next.
    ///
    /// `None` for a non-numeric label: "OVA" and "Special" have no successor, and guessing one
    /// would start playing something unrelated.
    pub fn next_episode(&self) -> Option<String> {
        self.episode.trim().parse::<u32>().ok().map(|n| (n + 1).to_string())
    }
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

    pub fn step(self, delta: isize) -> Self {
        let all = Self::ALL;
        let current = all.iter().position(|s| *s == self).unwrap_or(0) as isize;
        let next = (current + delta).rem_euclid(all.len() as isize) as usize;
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
    /// The tracker rejected our credentials; nothing but re-authorising helps.
    pub needs_reauth: bool,
    /// Most recent sync outcome, in words.
    pub last: Option<String>,
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
            needs_reauth: false,
            last: None,
        }
    }

    /// The status-line badge.
    ///
    /// The queue depth is the number worth showing: it is the answer to "did my progress
    /// actually go anywhere?", which is the only sync question anyone asks.
    pub fn badge(&self) -> String {
        if self.needs_reauth {
            return format!("{} ✕ sign in", self.tracker);
        }
        if !self.connected {
            return format!("{} ·", self.tracker);
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

/// A computed change, applied on the UI thread.
///
/// Async work never touches [`App`] directly; it sends one of these instead. That is what
/// guarantees the render loop is never blocked on I/O.
#[derive(Debug)]
pub enum Update {
    Content(Content),
    Detail(Box<Entry>),
    Status(String),
    Toast(Toast),
    Providers(Vec<ProviderRow>),
    /// Why no sources are available, when none are.
    ProviderNote(String),
    /// The ladder found candidates but could not choose between them.
    MatchChoices {
        id: AnilistId,
        provider_id: String,
        candidates: Vec<MatchCandidate>,
    },
    /// The selectable releases for the episode the user asked about.
    Sources(Vec<SourceCandidate>),
    /// The user changed the player volume.
    PlaybackVolume(f64),
    /// Which provider the current stream actually came from.
    ActiveProvider(String),
    /// A key pressed inside the player window: step to an adjacent episode.
    PlayerStepEpisode(i64),
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
    PlaybackEnded {
        watched: bool,
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
    Episodes(Vec<EpisodeRow>),
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
    /// The download queue, replacing whatever was there.
    Downloads(Vec<DownloadRow>),
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
}

/// The whole application state.
pub struct App {
    pub nav: Nav,
    pub palette: Palette,
    pub keymap: Keymap,
    pub config: Config,
    pub content: Content,
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
    pub palette_query: String,
    pub detail: Option<Entry>,
    pub status: String,
    pub toasts: Vec<Toast>,
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
    /// A device code awaiting approval, shown on the Accounts screen.
    pub device_code: Option<DeviceCodePrompt>,
    /// Snapshot for the Providers screen.
    pub providers: Vec<ProviderRow>,
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
    /// Selection within an overlay list (Accounts, Conflicts, list status, command palette).
    pub overlay_selected: usize,
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
    /// The palette variant detection settled on at startup.
    ///
    /// Kept so switching *out* of immersive mode can restore the right one without re-querying
    /// the terminal's background from inside the render loop.
    adaptive_variant: crate::theme::Variant,
    /// Recent errors and notices, for the Logs overlay.
    pub logs: Vec<LogRow>,
    /// Work the reducer produced while applying an update rather than handling a key.
    ///
    /// Auto-next is the only source: an episode ending has to be able to start the next one, and
    /// [`Self::apply`] has no return channel. The event loop drains this each iteration.
    pending: Option<Task>,
    /// Rows for the Episodes screen — the ones the current filter admits. Everything that
    /// reads or navigates episodes works on this list, which is what keeps the filter from
    /// needing index remapping at every call site.
    pub episodes: Vec<EpisodeRow>,
    /// Every row, unfiltered — the source [`Self::episodes`] is re-derived from.
    episodes_all: Vec<EpisodeRow>,
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
    /// Selection within the episode table, tracked separately from list selection so
    /// stepping back out of Episodes does not disturb where you were in the list.
    pub episode_selected: usize,
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
            config,
            content: Content::Loading,
            selected: 0,
            offset: 0,
            search_query: String::new(),
            searched_query: String::new(),
            palette_query: String::new(),
            detail: None,
            status: String::new(),
            toasts: Vec::new(),
            should_quit: false,
            synopsis_expanded: false,
            playing: None,
            eyecatch: None,
            library_segment: LibrarySegment::default(),
            sync: Vec::new(),
            conflicts: Vec::new(),
            overlay_selected: 0,
            pulse: 0,
            downloads: Vec::new(),
            device_code: None,
            reload_countdown: None,
            episodes_loading: false,
            adaptive_variant: match palette.variant {
                // Started in immersive, so there is no detected variant to remember. Dark is the
                // safer default of the two, and a restart re-runs real detection.
                crate::theme::Variant::Immersive => crate::theme::Variant::Dark,
                detected => detected,
            },
            logs: Vec::new(),
            pending: None,
            vpn_badge: None,
            vpn_leaking: false,
            providers: Vec::new(),
            provider_note: None,
            episodes: Vec::new(),
            episodes_all: Vec::new(),
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
        }
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.content.entries().get(self.selected)
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
            Update::Content(content) => {
                self.content = content;
                // Selection must stay in range or the next render would index past the end.
                self.selected = self.selected.min(self.content.len().saturating_sub(1));
                if self.content.is_empty() {
                    self.selected = 0;
                    self.offset = 0;
                }
                // The calendar is a timeline running from a week ago to a week ahead, so landing
                // on row nought would open it on the *oldest* episode in view. Now is the useful
                // place to be: what just aired is right above, what is coming is right below.
                if self.nav.section() == Section::Calendar
                    && let Content::Entries(entries) = &self.content
                    && let Some(now) =
                        entries.iter().position(|e| e.airing_in.is_some_and(|secs| secs > 0))
                {
                    self.selected = now;
                    self.offset = now.saturating_sub(2);
                }
            }
            Update::Detail(entry) => self.detail = Some(*entry),
            Update::DeviceCode(prompt) => self.device_code = prompt,
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
            Update::Image { url, image } => self.images.insert(&url, *image),
            Update::Providers(rows) => self.providers = rows,
            Update::ProviderNote(note) => self.provider_note = Some(note),
            Update::Sources(candidates) => {
                // A stale answer — the user has navigated away since asking — is not a
                // question worth interrupting them with.
                if self.source_context.is_none() {
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
                // plain failure instead.
                if candidates.is_empty() {
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
                    let (episode, duration) = (playing.episode.clone(), playing.duration);
                    // Keeps the table honest while detached: `q` leaves mpv running and drops you
                    // back on the episode list, which would otherwise sit frozen at the position
                    // you started from.
                    self.reflect_watch(&episode, position, duration, false);
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
            Update::PlaybackEnded { watched } => {
                let finished = self.playing.take();
                if let Some(playing) = &finished {
                    self.reflect_watch(
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

                // Auto-next only follows a *finished* episode. Rolling on after someone quit
                // ten minutes in would be the opposite of what they asked for.
                if watched
                    && self.config.playback.auto_next
                    && let Some(finished) = finished
                    && let Some(next) = finished.next_episode()
                    && let Some(id) = self.detail.as_ref().map(|e| e.id)
                {
                    let after_filler = self.next_after_filler(next.clone());
                    if after_filler != next {
                        self.push_toast(Toast::info(format!(
                            "skipped filler — ep {after_filler} next"
                        )));
                    }
                    self.pending = Some(self.begin_playback(id, after_filler));
                } else if watched {
                    self.push_toast(Toast::info("episode finished"));
                }
            }
            Update::Sync(state) => {
                // Re-authorisation is the one sync condition worth interrupting for: until it
                // is done, everything queues silently and the user would have no idea.
                let announce = state.needs_reauth
                    && !self.sync.iter().any(|s| s.tracker == state.tracker && s.needs_reauth);
                if announce {
                    self.push_toast(Toast::alert(format!(
                        "{} needs signing in again — :accounts",
                        state.tracker
                    )));
                }
                match self.sync.iter_mut().find(|s| s.tracker == state.tracker) {
                    Some(existing) => *existing = *state,
                    None => self.sync.push(*state),
                }
            }
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
                    self.push_toast(Toast::info(format!(
                        "{} sync disagreement(s) — :conflicts",
                        self.conflicts.len()
                    )));
                }
            }
            Update::Resumed { position } => {
                // Resume happens without asking — mpv is already at the right place by the time
                // this arrives. Saying so is what keeps it from feeling like a glitch, and names
                // the key that starts over.
                self.push_toast(Toast::info(format!(
                    "resumed at {}  ·  r to start over",
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
            Update::Episodes(rows) => {
                self.episodes_all = rows;
                self.apply_episode_filter();
                self.episodes_loading = false;
            }
        }
    }

    pub fn push_toast(&mut self, toast: Toast) {
        // Every toast is also a log line. A toast lives for a few seconds and the stack is
        // capped at three, so without this the *third* provider failure silently erases the
        // first — and when a source breaks, the earliest error is usually the one that explains
        // the rest. This is the whole reason the Logs overlay exists.
        self.log(toast.kind, toast.text.clone());
        // Cap the stack so a burst of provider failures cannot cover the screen.
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(toast);
    }

    /// Append to the log ring, newest last.
    pub fn log(&mut self, kind: ToastKind, text: String) {
        // Bounded: this runs for the life of the process and a provider retry loop could
        // otherwise grow it without limit.
        if self.logs.len() >= LOG_CAPACITY {
            self.logs.remove(0);
        }
        self.logs.push(LogRow { kind, text, at: self.pulse });
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
    /// Re-derive the visible episode list from the full one, keeping the cursor in range.
    fn apply_episode_filter(&mut self) {
        let filter = self.episode_filter;
        self.episodes = self.episodes_all.iter().filter(|r| filter.keeps(r)).cloned().collect();
        self.episode_selected =
            self.episode_selected.min(self.episodes.len().saturating_sub(1));
    }

    /// Whether the filter is hiding everything — distinct from a title with no episodes.
    pub fn episodes_all_filtered_out(&self) -> bool {
        self.episodes.is_empty() && !self.episodes_all.is_empty()
    }

    /// What the header should call the source: the provider that last actually served,
    /// falling back to the configured first choice before anything has played.
    pub fn source_label(&self) -> String {
        self.active_provider
            .clone()
            .or_else(|| self.config.providers.order.first().cloned())
            .unwrap_or_else(|| "no source".into())
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

    fn reflect_watch(
        &mut self,
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

        for list in [&mut self.episodes, &mut self.episodes_all] {
            if let Some(row) = list.iter_mut().find(|r| r.number == episode) {
                // Monotonic: a re-watch that was quit early must not erase that it was once
                // finished.
                row.watched = row.watched.max(fraction);
                row.completed = row.completed || completed;
            }
        }

        if !completed {
            return;
        }
        // Progress is the count of *completed* episodes, so only a finish moves it — and only when
        // this episode is the one it was waiting on, or watching an old episode out of order would
        // inflate the count.
        for entry in self.detail.iter_mut().chain(match &mut self.content {
            Content::Entries(entries) => entries.iter_mut(),
            _ => [].iter_mut(),
        }) {
            if let Some((done, next)) = entry.progress
                && episode == next.to_string()
            {
                entry.progress = Some((done + 1, next + 1));
                // The episode just finished is no longer something to resume.
                entry.resume = None;
            }
        }
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
    }

    /// Handle an action. Returns work for the caller to perform asynchronously.
    pub fn handle(&mut self, action: Action, visible_rows: usize) -> Option<Task> {
        // While typing, most single-key bindings must not fire, or "d" would trigger a
        // download instead of entering a letter.
        if self.is_typing() {
            return self.handle_while_typing(action, visible_rows);
        }

        // Playback bindings only mean anything while something is playing, and they collide
        // with browsing keys (`n`, `x`, digits), so they are dispatched first and only then.
        if self.playing.is_some()
            && let Some(task) = self.handle_playback(action)
        {
            return task;
        }

        // An open list overlay owns movement and Enter. Without this, arrowing through the
        // Accounts list would scroll the library behind it instead.
        if self.overlay_len() > 0 {
            match action {
                Action::Down => {
                    self.overlay_selected =
                        (self.overlay_selected + 1).min(self.overlay_len() - 1);
                    return None;
                }
                Action::Up => {
                    self.overlay_selected = self.overlay_selected.saturating_sub(1);
                    return None;
                }
                Action::Open => return self.confirm_overlay(visible_rows),
                _ => {}
            }
        }

        match action {
            Action::Quit => self.should_quit = true,
            Action::Help => self.nav.open_overlay(Overlay::Help),
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
            Action::Back => {
                if self.nav.back() == crate::nav::BackOutcome::AtRoot {
                    // At the very root, back is a no-op rather than a quit.
                    self.status = "at the top — Q to quit".into();
                }
            }
            Action::ToggleRail => self.nav.toggle_rail(),
            Action::JumpSection(n) => {
                if let Some(section) = Section::from_index(usize::from(n).saturating_sub(1)) {
                    return self.go_to_section(section);
                }
            }
            Action::FocusSearch => return self.go_to_section(Section::Search),
            // The rail owns the vertical axis when it has focus. Without this the arrows
            // scrolled the list *behind* the focused rail, so the eight top-level views were
            // reachable only by their number keys — fast once you know them, and undiscoverable
            // until then. Stepping the rail switches section immediately, exactly as pressing
            // the number does; a cursor you had to confirm with Enter would make the first
            // press look like nothing happened.
            Action::Down if self.nav.focus() == Focus::Rail => return self.step_section(1),
            Action::Up if self.nav.focus() == Focus::Rail => return self.step_section(-1),
            // Settings keeps its own cursor: the screen is built from `SettingId::ALL` rather
            // than from `content`, which is empty here, so the shared clamp would pin it at row
            // nought and nothing below the first setting would ever be reachable.
            Action::Down if self.in_downloads_stage() => {
                self.selected = (self.selected + 1).min(self.downloads.len().saturating_sub(1));
            }
            Action::Up if self.in_downloads_stage() => {
                self.selected = self.selected.saturating_sub(1);
            }
            Action::Down if self.in_accounts_stage() => {
                self.selected = (self.selected + 1).min(self.sync.len().saturating_sub(1));
            }
            Action::Up if self.in_accounts_stage() => {
                self.selected = self.selected.saturating_sub(1);
            }
            Action::Down if self.in_settings_stage() => {
                self.selected = (self.selected + 1).min(SettingId::ALL.len() - 1);
            }
            Action::Up if self.in_settings_stage() => {
                self.selected = self.selected.saturating_sub(1);
            }
            Action::Down if self.in_episodes() => {
                self.episode_selected =
                    (self.episode_selected + 1).min(self.episodes.len().saturating_sub(1));
            }
            Action::Up if self.in_episodes() => {
                self.episode_selected = self.episode_selected.saturating_sub(1);
            }
            Action::Down => self.move_selection(1, visible_rows),
            Action::Up => self.move_selection(-1, visible_rows),
            Action::PageDown => self.move_selection(visible_rows as isize, visible_rows),
            Action::PageUp => self.move_selection(-(visible_rows as isize), visible_rows),
            Action::Top => {
                self.selected = 0;
                self.offset = 0;
            }
            Action::Bottom => {
                self.selected = self.content.len().saturating_sub(1);
                self.offset = self.selected.saturating_sub(visible_rows.saturating_sub(1));
            }
            // Screens with a horizontal axis claim Left/Right for it: Settings cycles the
            // selected value, Library steps the status segment. Everywhere else the keys
            // move focus between rail and stage.
            Action::Right if self.in_settings_stage() => return self.cycle_setting(1),
            Action::Left if self.in_settings_stage() => return self.cycle_setting(-1),
            Action::Right if self.in_library_stage() => return self.step_segment(1),
            Action::Left if self.in_library_stage() => return self.step_segment(-1),
            Action::Right => self.nav.focus_stage(),
            Action::Left => self.nav.focus_rail(),
            // In the episode table Enter plays; everywhere else it opens the title.
            Action::Open if self.in_episodes() => return self.play_selected_episode(),
            // On Settings there is nothing to open, so Enter and Space do the obvious thing.
            Action::Open | Action::PlayNext if self.in_settings_stage() => {
                return self.cycle_setting(1);
            }
            Action::Open if self.in_accounts_stage() => return self.toggle_account(),
            // Enter plays a finished download from disk. Nothing else on this screen is an "open".
            Action::Open if self.in_downloads_stage() => {
                let row = self.downloads.get(self.selected)?.clone();
                match (&row.path, row.state) {
                    (Some(path), "complete") => {
                        // First-class playback: same staging, history and sync as a stream.
                        self.raise_now_playing_titled(row.title.clone(), &row.episode);
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
            Action::PlayPause if self.in_downloads_stage() => {
                let id = self.downloads.get(self.selected)?.id;
                return Some(Task::DownloadPause { id });
            }
            Action::StopPlayback if self.in_downloads_stage() => {
                let id = self.downloads.get(self.selected)?.id;
                return Some(Task::DownloadCancel { id });
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
            Action::Open => return self.open_selected(),
            // Space skips the detail screen entirely: from a list, straight into the next
            // episode you have not watched.
            Action::PlayNext => return self.play_next_unwatched(),
            Action::ShowEpisodes => {
                if let Some(entry) = self.detail.as_ref().or(self.selected_entry()) {
                    let id = entry.id;
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
                self.set_episode_watched(&number, watched);
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
                    self.push_toast(Toast::info("nothing before this episode to mark"));
                    return None;
                }
                for number in &marked {
                    self.set_episode_watched(number, true);
                }
                self.apply_episode_filter();
                self.push_toast(Toast::info(format!("marked {} watched", marked.len())));
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
            Action::Filter => {
                self.push_toast(Toast::info("the filter lives in the episode table"));
            }
            Action::ToggleWatched | Action::MarkAllPrevious => {
                self.push_toast(Toast::info("watched marks live in the episode table"));
            }
            Action::OpenInBrowser => {
                let id = match self.nav.current() {
                    StageView::Episodes(id) | StageView::Title(id) => Some(*id),
                    _ => self.detail.as_ref().or(self.selected_entry()).map(|e| e.id),
                };
                let Some(id) = id else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                return Some(Task::OpenExternal {
                    url: format!("https://anilist.co/anime/{}", id.get()),
                });
            }
            Action::WatchOrder => {
                let Some(entry) = self.detail.as_ref().or(self.selected_entry()) else {
                    self.push_toast(Toast::info("nothing selected"));
                    return None;
                };
                if entry.related.is_empty() {
                    // Empty on list rows too — the relations ride the detail fetch. From a
                    // list this still answers after opening the title once.
                    self.push_toast(Toast::info("no connected seasons known for this title"));
                    return None;
                }
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::WatchOrder);
            }
            Action::FixMapping => {
                // "This matched the wrong thing" — ask the user what to search for, then
                // re-enter the ordinary disambiguation flow with their words.
                let id = match self.nav.current() {
                    StageView::Episodes(id) | StageView::Title(id) => Some(*id),
                    _ => self.detail.as_ref().or(self.selected_entry()).map(|e| e.id),
                };
                let Some(id) = id else {
                    self.push_toast(Toast::info("nothing selected to re-match"));
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
                        let entry = self.detail.as_ref().or(self.selected_entry())?;
                        let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                        (entry.id, episode)
                    }
                };
                self.source_context = Some((id, episode.clone()));
                self.status = format!("listing sources for ep {episode}…");
                return Some(Task::LoadSources { id, episode });
            }
            Action::Download => {
                // From the episode table it is the highlighted episode; from a title it is the next
                // one you have not watched, which matches what Enter would have played.
                let (id, episode) = match self.nav.current() {
                    StageView::Episodes(id) => {
                        (*id, self.episodes.get(self.episode_selected)?.number.clone())
                    }
                    _ => {
                        let entry = self.detail.as_ref().or(self.selected_entry())?;
                        let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                        (entry.id, episode)
                    }
                };
                self.status = format!("queueing ep {episode}…");
                return Some(Task::DownloadEpisode { id, episode });
            }
            Action::ToggleSynopsis => self.synopsis_expanded = !self.synopsis_expanded,
            Action::SetListStatus => {
                if self.detail.as_ref().or(self.selected_entry()).is_some() {
                    // Preselect what the title is already on, so the common case is one
                    // keystroke to confirm rather than a hunt.
                    self.overlay_selected = LibrarySegment::ALL
                        .iter()
                        .position(|s| *s == self.library_segment)
                        .unwrap_or(0);
                    self.nav.open_overlay(Overlay::ListStatus);
                } else {
                    self.push_toast(Toast::info("nothing selected"));
                }
            }
            Action::ForceResync => {
                if self.sync.is_empty() {
                    self.push_toast(Toast::info("no tracker connected — :accounts"));
                } else {
                    self.status = "resyncing…".into();
                    return Some(Task::SyncNow);
                }
            }
            Action::Refresh => return self.reload(),
            Action::ToggleTranslation => {
                self.config.playback.translation = self.config.playback.translation.toggled();
                self.push_toast(Toast::info(format!(
                    "now using {}",
                    self.config.playback.translation
                )));
            }
            _ => {
                // Unimplemented actions announce themselves rather than doing nothing,
                // which would read as the key being broken.
                self.push_toast(Toast::info(format!("{} — not wired up yet", action.label())));
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
        let entry = self.detail.as_ref().or(self.selected_entry())?;
        let id = entry.id;
        let episode =
            entry.progress.map(|(_, next)| next.to_string()).unwrap_or_else(|| "1".to_string());
        Some(self.begin_playback(id, episode))
    }

    /// Step to an adjacent episode of what is playing.
    fn step_episode(&mut self, delta: i64) -> Option<Task> {
        let id = match self.nav.current() {
            StageView::NowPlaying => self.detail.as_ref()?.id,
            _ => return None,
        };
        let current: i64 = self.playing.as_ref()?.episode.trim().parse().ok()?;
        let next = current + delta;
        if next < 1 {
            self.push_toast(Toast::info("already at the first episode"));
            return None;
        }
        Some(self.begin_playback(id, next.to_string()))
    }

    /// Where auto-next actually lands, honouring `playback.skip_filler`.
    ///
    /// Steps over episodes marked pure filler (`mixed` is never skipped — it carries
    /// story). Only rows present in the loaded episode table can be judged; an unknown
    /// episode is played rather than guessed about.
    fn next_after_filler(&self, mut next: String) -> String {
        if !self.config.playback.skip_filler {
            return next;
        }
        let mut steps = 0;
        while let Some(row) = self.episodes.iter().find(|r| r.number == next) {
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
        self.raise_now_playing(&episode);
        Task::Play { id, episode }
    }

    /// The visual half of starting playback, shared with the Sources pick: eyecatch up,
    /// Now Playing staged, view pushed.
    fn raise_now_playing(&mut self, episode: &str) {
        let title = self
            .detail
            .as_ref()
            .or(self.selected_entry())
            .map_or_else(|| "playing".to_string(), |e| e.title.clone());
        self.raise_now_playing_titled(title, episode);
    }

    /// The same staging with the title stated by the caller — the Downloads screen knows
    /// what its rows are called without any list selection being involved.
    fn raise_now_playing_titled(&mut self, title: String, episode: &str) {
        self.eyecatch = Some(Eyecatch::new(format!("{title}  ·  ep {episode}")));
        self.playing = Some(NowPlaying {
            title,
            episode: episode.to_owned(),
            episode_title: self
                .episodes
                .iter()
                .find(|e| e.number == episode)
                .and_then(|e| e.title.clone()),
            speed: self.config.playback.persisted_speed.unwrap_or(1.0),
            ..NowPlaying::default()
        });
        self.nav.push(StageView::NowPlaying);
    }

    /// Advance the eyecatch one frame, dropping it when the wipe finishes.
    pub fn tick_animation(&mut self) {
        if let Some(eyecatch) = &mut self.eyecatch {
            let gave_up = eyecatch.timed_out();
            if !eyecatch.advance() {
                self.eyecatch = None;
                // A wipe that simply ends reads as success. This one is not: the player never
                // reported a position, so say so rather than dropping the user onto a screen
                // that looks like nothing happened.
                if gave_up {
                    self.push_toast(Toast::alert(
                        "the player never started — press x to stop, and check `mpv` runs on its own",
                    ));
                }
            }
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
        if tracker.connected && !tracker.needs_reauth {
            Some(Task::Disconnect { tracker: tracker.tracker })
        } else {
            self.status = "opening your browser to sign in…".into();
            Some(Task::Connect { tracker: tracker.tracker })
        }
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

    fn step_segment(&mut self, delta: isize) -> Option<Task> {
        self.library_segment = self.library_segment.step(delta);
        self.selected = 0;
        self.offset = 0;
        self.content = Content::Loading;
        Some(Task::LoadLibrary(self.library_segment))
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
                self.handle(action, visible_rows)
            }
            Some(Overlay::Logs) => None,
            Some(Overlay::ListStatus) => {
                let entry = self.detail.as_ref().or(self.selected_entry())?;
                let id = entry.id;
                let status = *LibrarySegment::ALL.get(self.overlay_selected)?;
                self.nav.close_overlay();
                self.push_toast(Toast::info(format!("marked {}", status.label())));
                Some(Task::SetStatus { id, status })
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
            Some(Overlay::WatchOrder) => {
                let related = self.detail.as_ref()?.related.get(self.overlay_selected)?.clone();
                self.nav.close_overlay();
                self.overlay_selected = 0;
                // A minimal entry stands in until the detail fetch answers — the same
                // stale-data rule as episodes: never render one title's data under another.
                self.detail = Some(Entry::new(related.id, related.title));
                self.nav.push(StageView::Title(related.id));
                Some(Task::LoadDetail(related.id))
            }
            Some(Overlay::Sources) => {
                let candidate = self.sources.get(self.overlay_selected)?.clone();
                let (id, episode) = self.source_context.clone()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                self.sources.clear();
                self.source_context = None;
                self.raise_now_playing(&episode);
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
                if tracker.connected && !tracker.needs_reauth {
                    Some(Task::Disconnect { tracker: tracker.tracker })
                } else {
                    self.status = "opening your browser to sign in…".into();
                    Some(Task::Connect { tracker: tracker.tracker })
                }
            }
            _ => None,
        }
    }

    /// Rows the currently-open overlay is listing, so movement can be bounded.
    fn overlay_len(&self) -> usize {
        match self.nav.overlay() {
            Some(Overlay::ListStatus) => LibrarySegment::ALL.len(),
            Some(Overlay::Conflicts) => self.conflicts.len(),
            Some(Overlay::Accounts) => self.sync.len(),
            // The palette was missing from this list, which meant the arrows scrolled the list
            // behind it and Enter did nothing — so the app's stated discoverability mechanism
            // could filter actions but never run one.
            Some(Overlay::CommandPalette) => self.palette_matches().len(),
            Some(Overlay::Logs) => self.logs.len(),
            Some(Overlay::Disambiguate) => self.match_candidates.len(),
            Some(Overlay::Sources) => self.sources.len(),
            Some(Overlay::WatchOrder) => self.detail.as_ref().map_or(0, |e| e.related.len()),
            _ => 0,
        }
    }

    /// The Settings screen's rows, built fresh from the live config.
    ///
    /// Read from `self.config` rather than a copy taken at startup, so a row shows the value that
    /// is actually in force the instant after it is changed.
    pub fn setting_rows(&self) -> Vec<SettingRow> {
        use SettingId as S;
        let playback = &self.config.playback;
        let torrent = &self.config.providers.torrent;
        SettingId::ALL
            .iter()
            .map(|id| {
                let (value, editable, note) = match id {
                    S::Theme => (
                        format!("{:?}", self.config.theme.mode).to_lowercase(),
                        Some(("theme", "mode")),
                        None,
                    ),
                    S::Motion => {
                        (on_off(self.config.theme.motion), Some(("theme", "motion")), None)
                    }
                    S::Translation => (
                        playback.translation.to_string(),
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
                        None,
                        Some("edit config.toml — too many values to cycle"),
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
                        Some("needs a VPN configured, and a restart to take effect"),
                    ),
                    // Deliberately not editable. `mode = "none"` requires an explicit
                    // acknowledgement key in the file, and a screen that let you cycle past it
                    // with an arrow key would be a way around friction that exists on purpose.
                    S::VpnMode => (
                        format!("{:?}", torrent.vpn.mode).to_lowercase(),
                        None,
                        Some("edit config.toml — turning this off has to be deliberate"),
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
                let next = match self.config.theme.mode {
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
                let next = match self.config.playback.translation {
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
            SettingId::SkipOpening => {
                self.config.playback.skip_opening = !self.config.playback.skip_opening;
                V::Bool(self.config.playback.skip_opening)
            }
            SettingId::SkipFiller => {
                self.config.playback.skip_filler = !self.config.playback.skip_filler;
                V::Bool(self.config.playback.skip_filler)
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
            SettingId::Subtitles | SettingId::VpnMode | SettingId::TokenStorage => return None,
        };

        Some(Task::SaveSetting { table: edit.table, key: edit.key, value })
    }

    /// Actions matching the current palette query, in the order they are rendered.
    ///
    /// One source of truth shared by movement, Enter and the renderer. Deriving the selectable
    /// set separately from the drawn set is how a palette ends up running the wrong entry.
    pub fn palette_matches(&self) -> Vec<(Action, String)> {
        self.keymap
            .palette_entries(&self.palette_query)
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
        match action {
            Action::Back => {
                if !self.nav.close_overlay() {
                    // Nothing to dismiss — leave the field rather than trapping the user
                    // in it.
                    self.nav.focus_rail();
                }
            }
            Action::CommandPalette => {
                self.palette_query.clear();
                self.nav.open_overlay(Overlay::CommandPalette);
            }
            Action::Help => self.nav.open_overlay(Overlay::Help),
            Action::Quit => self.should_quit = true,
            // The palette takes text, so it reaches this path rather than the overlay block in
            // `handle`. It used to close on Enter without running anything, and move the list
            // *behind* it on the arrows — so the one mechanism meant to make every action
            // discoverable could show you an action and then refuse to perform it.
            Action::Open if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                return self.confirm_overlay(visible_rows);
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::DownloadRange) => {
                let id = match self.nav.current() {
                    StageView::Episodes(id) => *id,
                    _ => return None,
                };
                // Resolved against episodes that actually exist, from the unfiltered
                // list — a filter narrows the view, not what can be fetched.
                let Some((from, to)) = parse_episode_range(&self.range_query) else {
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
                self.nav.close_overlay();
                if episodes.is_empty() {
                    self.push_toast(Toast::info("no episodes in that range"));
                    return None;
                }
                self.push_toast(Toast::info(format!("queueing {} episodes…", episodes.len())));
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
                    self.status = "reset to the automatic match…".into();
                    return Some(Task::ClearMatch { id });
                }
                self.status = format!("searching sources for {query:?}…");
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
                if query == self.searched_query && !self.content.is_empty() {
                    return self.open_selected();
                }
                self.searched_query = query.clone();
                self.content = Content::Loading;
                return Some(Task::Search(query));
            }
            Action::Down => self.selected = self.selected.saturating_add(1),
            Action::Up => self.selected = self.selected.saturating_sub(1),
            _ => {}
        }
        None
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
        } else if self.nav.section() == Section::Search {
            self.search_query.pop();
        }
    }

    /// Push the selected title's detail screen.
    fn open_selected(&mut self) -> Option<Task> {
        let entry = self.selected_entry()?.clone();
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
        self.content = Content::Loading;
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
        self.reload()
    }

    /// Work needed to populate the current view.
    pub fn reload(&mut self) -> Option<Task> {
        let task = match self.nav.section() {
            Section::Home => Task::LoadContinue,
            Section::Seasonal => Task::LoadSeasonal,
            Section::Calendar => Task::LoadCalendar,
            Section::Search if !self.search_query.trim().is_empty() => {
                Task::Search(self.search_query.clone())
            }
            Section::Providers => Task::CheckProviders,
            Section::Downloads => Task::LoadDownloads,
            Section::Library => Task::LoadLibrary(self.library_segment),
            // Sections with nothing to fetch yet render their own placeholder.
            _ => return None,
        };
        self.content = Content::Loading;
        Some(task)
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

/// Asynchronous work requested by the reducer.
#[derive(Debug, Clone, PartialEq)]
pub enum Task {
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
    Search(String),
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
    /// Settle a divergence by keeping one side.
    ResolveConflict {
        id: AnilistId,
        keep_local: bool,
    },
    /// Run a tracker's sign-in flow.
    Connect {
        tracker: String,
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
}

/// A control sent to whatever is currently playing.
///
/// Kept as data rather than a call into the player so the reducer stays pure and testable —
/// the binary routes these to the live session.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Toggle the player's fullscreen state.
    Fullscreen,
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

    /// An app with one title selected and its episode table loaded, ready to play.
    fn app_at_episodes() -> App {
        let mut a = app();
        a.apply(Update::Content(entries(3)));
        a.apply(Update::Episodes(
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
                })
                .collect(),
        ));
        a.detail = a.content.entries().first().cloned();
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

        a.apply(Update::Sources(vec![
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
        a.apply(Update::Sources(Vec::new()));
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
        a.apply(Update::Sources(vec![source("[A] Show - 1 (1080p)", true)]));
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
            id: AnilistId::new(7),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Frieren S1", 0.62), candidate("Frieren S2", 0.61)],
        });

        // Move to the second candidate and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must do something");
        assert_eq!(
            task,
            Task::FixMatch {
                id: AnilistId::new(7),
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
    fn playback_updates_never_resurrect_a_finished_session() {
        // A position tick can arrive after the `Ended` event that preceded it. Recreating
        // `playing` from it would leave a phantom player on screen.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: false });
        assert!(a.playing.is_none());

        a.apply(Update::Playback { position: 100.0, duration: Some(1435.0), paused: false });
        assert!(a.playing.is_none(), "a stray tick brought the player back");
    }

    #[test]
    fn finishing_an_episode_queues_the_next_one() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: true });
        assert_eq!(
            a.take_pending(),
            Some(Task::Play { id: AnilistId::new(1), episode: "2".into() }),
            "auto-next did not follow a finished episode"
        );
    }

    #[test]
    fn quitting_partway_does_not_roll_on_to_the_next_episode() {
        // The distinction auto-next lives or dies on: `watched` means the episode ran to the
        // commit threshold, not that mpv exited.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: false });
        assert_eq!(a.take_pending(), None);
    }

    #[test]
    fn auto_next_can_be_turned_off() {
        let mut a = app_at_episodes();
        a.config.playback.auto_next = false;
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: true });
        assert_eq!(a.take_pending(), None);
    }

    #[test]
    fn a_non_numeric_episode_has_no_next() {
        // "OVA" and "Special" are real labels with no successor; guessing one would start
        // playing something unrelated.
        let mut a = app_at_episodes();
        a.episodes[0].number = "OVA".into();
        a.episode_selected = 0;
        a.handle(Action::Open, 20);
        a.apply(Update::PlaybackEnded { watched: true });
        assert_eq!(a.take_pending(), None);
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
        a.apply(Update::Content(entries(3)));
        assert_eq!(a.handle(Action::NextEpisode, 20), None);
        assert!(a.playing.is_none());
    }

    #[test]
    fn auto_next_steps_over_filler_when_asked() {
        let mut a = app_at_episodes();
        a.config.playback.skip_filler = true;
        for i in [1, 2] {
            a.episodes[i].skippable = true;
            a.episodes[i].kind = Some("filler");
        }
        a.handle(Action::Open, 20); // plays ep 1

        a.apply(Update::PlaybackEnded { watched: true });
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

        a.apply(Update::PlaybackEnded { watched: true });
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
        assert_eq!(state.badge(), "anilist ·");
        assert!(!state.is_alerting());
    }

    #[test]
    fn re_authorisation_is_announced_once_not_every_tick() {
        // The drain runs on an interval; repeating the alert each time would bury everything
        // else in toasts.
        let mut a = app();
        a.apply(Update::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        let after_first = a.toasts.len();
        assert_eq!(after_first, 1);
        a.apply(Update::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        assert_eq!(a.toasts.len(), after_first, "re-announced an unchanged auth failure");
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
        let before = a.toasts.len();
        a.apply(Update::Conflicts(vec![conflict("Frieren")]));
        assert_eq!(a.toasts.len(), before);
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
        a.apply(Update::Content(entries(10)));
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
    fn library_segments_cycle_in_both_directions() {
        assert_eq!(LibrarySegment::Watching.step(1), LibrarySegment::Planning);
        assert_eq!(LibrarySegment::Watching.step(-1), LibrarySegment::Dropped);
        assert_eq!(LibrarySegment::Dropped.step(1), LibrarySegment::Watching);
    }

    #[test]
    fn stepping_a_library_segment_reloads_and_resets_the_selection() {
        // A stale selection index into a different list would point at the wrong title.
        let mut a = app();
        a.go_to_section(Section::Library);
        a.apply(Update::Content(entries(10)));
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
        a.apply(Update::Content(entries(3)));
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
        a.apply(Update::Content(entries(3)));
        a.library_segment = LibrarySegment::Completed;
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
        a.apply(Update::Content(entries(3)));
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
        a.apply(Update::Content(Content::Entries(vec![])));
        a.handle(Action::Down, 10);
        a.handle(Action::Bottom, 10);
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn shrinking_content_keeps_the_selection_in_range() {
        // The crash this prevents: selecting item 40, a refresh returning 3, and the next
        // render indexing past the end.
        let mut a = app();
        a.apply(Update::Content(entries(50)));
        a.handle(Action::Bottom, 10);
        assert_eq!(a.selected, 49);

        a.apply(Update::Content(entries(3)));
        assert!(a.selected < 3, "selection {} out of range", a.selected);
        assert!(a.selected_entry().is_some());
    }

    #[test]
    fn emptying_content_resets_selection_and_scroll() {
        let mut a = app();
        a.apply(Update::Content(entries(50)));
        a.handle(Action::Bottom, 10);
        a.apply(Update::Content(Content::Entries(vec![])));
        assert_eq!(a.selected, 0);
        assert_eq!(a.offset, 0);
        assert!(a.selected_entry().is_none());
    }

    #[test]
    fn scrolling_keeps_the_selection_inside_the_viewport() {
        let mut a = app();
        a.apply(Update::Content(entries(100)));
        let rows = 10;
        for _ in 0..25 {
            a.handle(Action::Down, rows);
        }
        assert!(a.selected >= a.offset, "selection scrolled off the top");
        assert!(
            a.selected < a.offset + rows,
            "selection {} outside viewport starting {}",
            a.selected,
            a.offset
        );
    }

    #[test]
    fn finishing_an_episode_updates_the_table_without_a_reload() {
        // Reported from real use: the episode table still showed the old state after an episode
        // finished, so it looked as though nothing had been recorded.
        let mut a = app();
        a.apply(Update::Content(entries(3)));
        a.nav.focus_stage();
        let id = a.selected_entry().expect("an entry").id;
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        // Push the view first: it clears any previous title's rows, which is the point.
        a.handle(Action::ShowEpisodes, 10);
        a.apply(Update::Episodes(vec![
            EpisodeRow {
                number: "1".into(),
                title: None,
                duration_secs: Some(1440),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                thumbnail: None,
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

        a.apply(Update::PlaybackEnded { watched: true });
        assert!(a.episodes[0].completed, "a finished episode must show as finished");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6);
        assert_eq!(a.episodes[1].watched, 0.0, "only the episode watched should change");
    }

    #[test]
    fn opening_episodes_never_shows_the_previous_title_s_rows() {
        // Reported from real use: the table displayed the last title's episodes — with their watch
        // progress — until the new load answered, then corrected itself. Stale content presented as
        // current is worse than an honest wait.
        let mut a = app();
        a.apply(Update::Content(entries(3)));
        a.nav.focus_stage();
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        a.apply(Update::Episodes(vec![EpisodeRow {
            number: "7".into(),
            title: Some("from the first title".into()),
            duration_secs: Some(1440),
            watched: 0.9,
            completed: true,
            kind: None,
            skippable: false,
            thumbnail: None,
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
        a.apply(Update::Content(entries(1)));
        a.detail = Some(a.selected_entry().expect("an entry").clone());
        a.handle(Action::ShowEpisodes, 10);
        assert!(a.episodes_loading);
        // An empty answer is still an answer, and must clear the wait.
        a.apply(Update::Episodes(Vec::new()));
        assert!(!a.episodes_loading, "an empty result is not a permanent loading state");
    }

    #[test]
    fn a_re_watch_quit_early_does_not_erase_that_it_was_finished() {
        let mut a = app();
        a.apply(Update::Content(entries(1)));
        a.apply(Update::Episodes(vec![EpisodeRow {
            number: "1".into(),
            title: None,
            duration_secs: Some(1440),
            watched: 1.0,
            completed: true,
            kind: None,
            skippable: false,
            thumbnail: None,
        }]));
        let id = a.selected_entry().expect("an entry").id;
        a.detail = Some(a.selected_entry().expect("an entry").clone());
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
        assert_eq!(a.handle(Action::Open, 10), Some(Task::Search("frieren".into())));
        a.apply(Update::Content(entries(3)));

        // Second Enter opens the highlighted result rather than asking again.
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Task::LoadDetail(_))), "got {task:?}");
        assert!(matches!(a.nav.current(), StageView::Title(_)));

        // Editing the query makes it submittable again.
        a.handle(Action::Back, 10);
        a.type_char('x');
        assert_eq!(a.handle(Action::Open, 10), Some(Task::Search("frierenx".into())));
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
        a.apply(Update::Content(entries(3)));
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
        a.apply(Update::Content(entries(3)));
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
        a.apply(Update::Content(entries(3)));
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
        a.apply(Update::Content(entries(10)));
        a.handle(Action::Bottom, 5);
        a.handle(Action::Open, 5);

        a.go_to_section(Section::Seasonal);
        assert_eq!(a.selected, 0);
        assert!(a.detail.is_none(), "stale detail from another section");
        assert_eq!(a.nav.depth(), 1);
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
        a.apply(Update::Content(entries(3)));
        a.go_to_section(Section::Home);
        assert_eq!(a.content, Content::Loading);
    }

    #[test]
    fn a_failure_is_content_rather_than_an_empty_list() {
        let mut a = app();
        a.apply(Update::Content(Content::Failed("anilist unreachable".into())));
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
        assert!(a.status.contains("Q to quit"));
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
}
