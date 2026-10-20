//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Event`]s, so a slow provider or a large image decode cannot stall a frame.

use std::time::{Duration, Instant};

use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_model::{
    catalog::{Content, ContentFor, Entry, LibrarySegment, SearchFilters},
    command::{Command, PlayerCommand},
    downloads::{DownloadRow, DownloadState, SeedRow},
    episodes::EpisodeRow,
    event::Event,
    jobs::JobId,
    notice::{Notice, NoticeHint, NoticeKind},
    playback::{Ending, SkipKind},
    settings::{SettingChange, SettingKey, SettingsView},
    sources::{
        GuardView, MatchCandidate, MetaSourceRow, ProviderRow, SourceChip, SourceState,
        order_with_source_moved, source_states,
    },
    stores::{
        InviteLinkView, LocalPluginRow, Secret, StoreInviteRow, StoreMemberRow, StorePluginRow,
        StorePluginStatus, StorePreview, StoreRow, StoreState,
    },
    sync::{ConflictRow, DeviceCodePrompt, SyncState},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{
    clock::Clock,
    eyecatch::Eyecatch,
    keymap::{Action, Context, Keymap},
    nav::{Focus, Nav, Overlay, Section, StageView},
    theme::{Palette, glyph},
};

/// A notice on screen, for as long as it stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub notice: Notice,
    /// Frames remaining before it disappears.
    pub ttl: u16,
}

impl From<Notice> for Toast {
    fn from(notice: Notice) -> Self {
        // Failures stay up longer: they carry information the user may need to act on,
        // and provider death is this app's defining failure mode.
        let ttl = match notice.kind {
            NoticeKind::Info => 90,
            NoticeKind::Alert => 180,
        };
        Self { notice, ttl }
    }
}

/// One row of the Settings screen, as the terminal shows it: the row the config makes, under its
/// heading, with the terminal's note.
///
/// The screen was read-only, which made it a status page wearing a settings page's name. Every
/// row here either cycles through a closed set of values or says plainly why it cannot.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingLine {
    pub label: &'static str,
    /// The heading this row sits under on screen.
    pub category: &'static str,
    /// Rendered value.
    pub value: String,
    /// The setting it shows.
    pub key: SettingKey,
    /// `false` for rows that are shown but not editable here.
    pub editable: bool,
    /// Why a row is read-only, or what a change will not do until restart.
    pub note: Option<&'static str>,
}

/// The local calendar date an offset-from-now lands on. Shared with the renderer, so
/// where the calendar *opens* and where it *rules* can never disagree.
pub(crate) fn air_date(clock: &Clock, secs_from_now: i64) -> Option<chrono::NaiveDate> {
    clock.date_of(clock.now_epoch() + secs_from_now)
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
    pub kind: NoticeKind,
    pub text: String,
    /// Idle ticks at the time it was recorded, so lines can be shown relative to each other
    /// without pulling a wall clock into the UI crate.
    pub at: u64,
    /// How many times in a row this exact line arrived. A retry loop or a burst of identical
    /// failures is one line with a count, so it cannot push the line that explains it off
    /// the screen.
    pub count: u32,
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

/// The header's `vpn` chip for a guard's verdict, and whether it alerts.
///
/// A guard still checking does not: it is not yet failing anyone. Nor does one the
/// configuration runs unprotected, which is a choice rather than a fault.
pub fn vpn_badge(guard: &GuardView) -> (String, bool) {
    match guard {
        GuardView::Checking => (format!("vpn {}  checking", glyph::STATE_UNKNOWN), false),
        GuardView::Protected { exit } => (format!("vpn {}  {exit}", glyph::STATE_READY), false),
        GuardView::Leaking => (format!("vpn {}  LEAK", glyph::STATE_DOWN), true),
        GuardView::Unreachable => (format!("vpn {}  unreachable", glyph::STATE_DOWN), true),
        GuardView::Unprotected => (format!("vpn {}  unprotected", glyph::STATE_DOWN), false),
    }
}

/// A notice as the terminal says it: what can be done about it, if the work said, put in the
/// terminal's own words and keys.
///
/// Said where the work used to say the key itself, so the line reads exactly as it always has:
/// after the text's first " — ", with a "; " before the rest of it, or at the end after a " — "
/// of its own. Its key is the one bound now, so a rebind changes what the line says. Enter is
/// named in words, as these lines always named it, where the binding would draw it as `↵`.
pub fn worded(notice: Notice, keymap: &Keymap) -> Notice {
    let Some(hint) = notice.hint else { return notice };
    let said = match hint {
        NoticeHint::StopPlayback => {
            format!("press {} to stop", keymap.key_for(Action::StopPlayback))
        }
        NoticeHint::OpenStores => "Providers, Enter".into(),
        NoticeHint::KitsuCredentials => "enter on its row asks for credentials".into(),
    };
    let text = match notice.text.split_once(" — ") {
        Some((head, rest)) => format!("{head} — {said}; {rest}"),
        None => format!("{} — {said}", notice.text),
    };
    Notice { kind: notice.kind, text, hint: None }
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
    /// Set while a skip is offered: past what, and where it would seek to.
    pub skip: Option<(SkipKind, f64)>,
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

/// A tracker's status-line badge.
///
/// The queue depth is the number worth showing: it is the answer to "did my progress
/// actually go anywhere?", which is the only sync question anyone asks.
pub fn sync_badge(state: &SyncState) -> String {
    if state.needs_reauth {
        return format!("{} {} sign in", state.tracker, glyph::STATE_DOWN);
    }
    // Hollow, not the interpunct: the header puts `·` between its groups of chips, and a
    // signed-out `kitsu ·` read as a separator with a word in front of it.
    if !state.connected {
        return format!("{} {}", state.tracker, glyph::STATE_OFF);
    }
    match state.outbox {
        0 => format!("{} {}", state.tracker, glyph::SYNC),
        n => format!("{} {} {n}", state.tracker, glyph::SYNC),
    }
}

/// The rows a screen's cursor walks. See [`App::rows_here`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    /// The section's list: `content`, `selected` and `offset`.
    List,
    Downloads,
    Accounts,
    Settings,
    /// The Providers table, in the order failover walks it.
    Providers,
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
    /// The config, as the engine last said: the Settings rows, and what else on screen the
    /// config decides. The engine owns the config, and the interface keeps no copy: a change is
    /// asked for, and shows once the engine says it is in effect.
    pub settings: SettingsView,
    pub content: Content,
    /// The caption for the list on screen, with the view it belongs to. See [`Event::Caption`].
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
    /// The playback still in the window while the one staged over it is looked up. A newer
    /// playback takes the window only once it has something to play, so until then this one's
    /// reports are its own, and if the lookup fails it is what Now Playing shows again.
    superseded: Option<NowPlaying>,
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
    /// Where the reducer and the renderer read the time and the zone: the system's, unless a
    /// test stops it so that what a screen shows does not depend on when it was drawn.
    pub clock: Clock,
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
    /// The title a mark made here is waiting on the engine's word for. The engine writes the
    /// mark and says where the title stands, at once; until it has, the loop holds the next
    /// frame and key, so the frame after the key shows the mark and a second press reads it.
    marking: Option<AnilistId>,
    /// An ending rolled on, and the next episode's staging is on its way: the engine says it
    /// right after the ending. Until it comes, Now Playing stays as it was and the loop holds
    /// the next frame and key, so the frame after the ending shows the next episode staged, and
    /// no key acts on the one that has just ended.
    rolling: bool,
    /// Work the reducer produced outside a key's answer, which the event loop drains each
    /// iteration: what the viewer asked for, without a key that could carry it. The calendar's
    /// backfill is triggered by scrolling near its top, which reports no task of its own; and
    /// a section's reload waits for the rail to settle, and goes out from the idle tick.
    ///
    /// What a playback leads to on its own, the next episode and the rest, is not here: the
    /// engine starts it, and says what it staged.
    pending: Option<Command>,
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
    pub edit_target: Option<SettingKey>,
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
    /// Whether that link went to the terminal's clipboard: set by whoever runs the app, once
    /// a copy has gone through. Every new link starts uncopied.
    pub invite_copied: bool,
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
    /// The app on `settings`, as the config it was launched on makes them, painted with
    /// `palette` and driven by `keymap`.
    pub fn new(settings: SettingsView, palette: Palette, keymap: Keymap) -> Self {
        Self::with_images(settings, palette, keymap, crate::image::ImageEngine::disabled())
    }

    pub fn with_images(
        settings: SettingsView,
        palette: Palette,
        keymap: Keymap,
        engine: crate::image::ImageEngine,
    ) -> Self {
        Self {
            images: crate::image::ImageStore::new(engine),
            nav: Nav::new(),
            palette,
            keymap,
            settings,
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
            superseded: None,
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
            clock: Clock::system(),
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
            marking: None,
            rolling: false,
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
            invite_copied: false,
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

    /// The key for `action` as a sentence names it: see [`Keymap::key_for`].
    pub fn key_for(&self, action: Action) -> String {
        self.keymap.key_for(action)
    }

    /// The key that jumps to `section`.
    pub fn section_key(&self, section: Section) -> String {
        self.key_for(Action::JumpSection(section.index() as u8 + 1))
    }

    /// Apply an update from a background task.
    pub fn apply(&mut self, update: Event) {
        // Resolution has finished, one way or the other — let the wipe complete. An alert is
        // as much a result as a first frame is, and holding the band over a failure would
        // leave the user staring at amber.
        // A report from the playback still in the window says nothing about the one being found.
        let ours = self.superseded.is_none();
        if (matches!(update, Event::Playback { .. } | Event::PlaybackEnded { .. } if ours)
            || matches!(update, Event::Toast(Notice { kind: NoticeKind::Alert, .. })))
            && let Some(eyecatch) = &mut self.eyecatch
        {
            eyecatch.release();
        }

        match update {
            Event::CalendarWindow { anchor, pages, more_past } => {
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
            Event::CalendarBackfill { page, entries, more_past } => {
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
            Event::Caption { view, text } => {
                if self.content_view().as_ref() == Some(&view) {
                    self.caption = (!text.is_empty()).then_some((view, text));
                }
            }
            Event::Content { view, content } => {
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
                    let today = self.clock.today();
                    let start_of_today = entries
                        .iter()
                        .position(|e| {
                            e.airing_in.is_some_and(|secs| {
                                air_date(&self.clock, secs).is_some_and(|d| d >= today)
                            })
                        })
                        .unwrap_or(next);
                    self.selected = next;
                    self.offset =
                        next.saturating_sub((next - start_of_today.min(next)).min(12));
                }
            }
            // Every way into a title sets a stand-in `detail` for it before the fetch goes out, so
            // a full record for any other id answers a title the user has already left.
            Event::Detail(mut entry) => {
                if let Some(detail) = self.detail.as_ref().filter(|d| d.id == entry.id) {
                    // The record does not carry the tracker's status; the row it was opened
                    // from may, and losing it here would make `a` forget what it is on.
                    entry.list_status = entry.list_status.or(detail.list_status);
                    self.detail = Some(*entry);
                }
            }
            Event::DeviceCode(prompt) => self.device_code = prompt,
            Event::Seeding(rows) => self.seeding = rows,
            Event::Downloads(rows) => {
                self.downloads = rows;
                // The cursor has to stay in range: a completed row being cleared shortens the list
                // under it.
                if self.nav.section() == Section::Downloads {
                    self.selected = self.selected.min(self.downloads.len().saturating_sub(1));
                }
            }
            Event::LastAired(rows) => {
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
            Event::Status(status) => self.status = status,
            Event::Toast(notice) => self.push_toast(notice),
            // Decoded off the loop, and back through `artwork_decoded`.
            Event::ArtworkReady { .. } => {}
            Event::ImageFailed { url, offline } => self.images.mark_failed(&url, offline),
            Event::Providers(rows) => {
                self.providers = rows;
                // The cursor stays in range: a removed plugin shortens the table under it.
                if self.nav.section() == Section::Providers {
                    self.selected = self.selected.min(self.providers.len().saturating_sub(1));
                }
            }
            Event::MetaStatus(rows) => self.meta_status = rows,
            Event::Stores(rows) => self.apply_stores(rows),
            Event::StorePreview(outcome) => self.apply_store_preview(outcome),
            Event::StoreJoined { store } => {
                // Into the store just joined, but only from the list it was joined from. A join
                // that finishes after the user has gone elsewhere must not drag them back.
                if self.nav.current() == &StageView::Stores && self.store(&store).is_some() {
                    self.store_line_selected = 0;
                    self.nav.push(StageView::Store(store));
                }
            }
            Event::InviteLink(view) => {
                self.invite_link = Some(view);
                self.invite_copied = false;
                self.nav.open_overlay(Overlay::InviteLink);
            }
            Event::StoreMembers { store, members, invites } => {
                if self.members_for.as_deref() == Some(store.as_str()) {
                    self.store_members = members;
                    self.store_invites = invites;
                    let rows = self.store_members.len() + self.store_invites.len();
                    self.member_selected = self.member_selected.min(rows.saturating_sub(1));
                }
            }
            Event::LocalPlugins { store, plugins } => {
                // Only for the store still on screen; a late answer for another is dropped.
                let here = self.nav.current() == &StageView::Store(store.clone());
                if self.publish_for.as_deref() != Some(store.as_str()) || !here {
                    return;
                }
                if plugins.is_empty() {
                    self.push_toast(Notice::info(
                        "no plugins in the plugin directory to publish — `anistream --plugins` shows where it is",
                    ));
                    return;
                }
                self.local_plugins = plugins;
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::PublishPick);
            }
            Event::Settings(view) => self.take_settings(view),
            Event::SourcesChanged => self.status = "reloading sources…".into(),
            Event::ProviderNote(note) => self.provider_note = Some(note),
            Event::ProviderPreference(choice) => self.provider_preference = choice,
            Event::Sources { id, episode, candidates } => {
                // A stale answer — the user has navigated away since asking, or asked about
                // another episode — is not a question worth interrupting them with, and offering
                // one episode's releases under another's context would play the wrong one.
                if self.source_context.as_ref() != Some(&(id, episode)) {
                    return;
                }
                if candidates.is_empty() {
                    self.source_context = None;
                    self.push_toast(Notice::info("no selectable sources for this episode"));
                    return;
                }
                self.sources = candidates;
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Sources);
            }
            Event::MatchChoices { id, provider_id, candidates } => {
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

            Event::Playback { position, duration, paused } => {
                // Playback updates arrive about once a second and must never resurrect a
                // finished session, so they only ever mutate an existing one.
                // What the playhead moves of the title's progress, the engine says ahead of it.
                if let Some(playing) = self.superseded.as_mut().or(self.playing.as_mut()) {
                    playing.position = position;
                    playing.paused = paused;
                    if duration.is_some() {
                        playing.duration = duration;
                    }
                }
            }
            Event::SkipAvailable { kind, to } => {
                if let Some(playing) = &mut self.playing {
                    playing.skip = Some((kind, to));
                }
            }
            Event::SkipCleared => {
                if let Some(playing) = &mut self.playing {
                    playing.skip = None;
                }
            }
            // What the player changes of the config on its own is the engine's to keep and to
            // save, as it passes: see `anistream_engine::settings::remember`. It says the
            // settings after, the speed a new playback starts at among them. Saved from here,
            // they waited in `pending`, and a save that found the slot taken was skipped.
            Event::PlaybackSpeed(speed) => {
                if let Some(playing) = &mut self.playing {
                    playing.speed = speed;
                }
            }
            Event::ActiveProvider(provider) => self.active_provider = Some(provider),
            Event::PlaybackKept => {
                if let Some(kept) = self.superseded.take() {
                    self.playing = Some(kept);
                }
            }
            Event::PlaybackStaged { id, title, episode, episode_title, speed } => {
                self.take_staged(id, title, episode, episode_title, speed);
            }
            // The window has changed hands, so what reports from here on is the successor. What
            // the episode left moves of its title, the engine says ahead of this.
            Event::EpisodeLeft { id, episode, .. } => {
                if self
                    .superseded
                    .as_ref()
                    .is_some_and(|s| s.id == Some(id) && s.episode == episode)
                {
                    self.superseded = None;
                }
            }
            // What a playback leads to on its own is the engine's to start: an episode picked in
            // the player, the episode again in the other translation, the next one's streams.
            // What it started comes as `PlaybackStaged`, and the notice that it now uses the
            // other translation with it. And what the player changes of the config is the
            // engine's to keep.
            Event::PlayerPlayEpisode { .. }
            | Event::PlayerTranslation(_)
            | Event::PrefetchNext { .. }
            | Event::PlayerUpscaling(_)
            | Event::PlayerToneMapping(_)
            | Event::PlayerNightMode(_)
            | Event::PlaybackVolume(_) => {}
            // The playback still in the window ended while its successor was being found: the
            // successor is what was asked for, so nothing comes down and nothing rolls on.
            //
            // What either ending moves of its title, the engine says ahead of it: the screen
            // the viewer drops back to shows the finished episode as finished.
            Event::PlaybackEnded { .. } if self.superseded.is_some() => self.superseded = None,
            // Rolling on: the engine has started the next episode in this window, and stages it
            // with the `PlaybackStaged` that follows. Now Playing stays up as it is until then,
            // and the loop draws and reads keys again once it has come: see `rolling`.
            //
            // Whether it rolls on is the playback's decision, which the engine follows. That side
            // knows how the episode ended — closing the window past the commit threshold is
            // `watched` too, and rolling on from it opened a new window on someone who had just
            // closed one — whether Up Next was cancelled for this episode, and whether the
            // episode list has a next one at all. It is also the side keeping a window open for
            // the next episode, so the two cannot disagree into a window left waiting for an
            // episode nobody starts.
            Event::PlaybackEnded { rolling_on: true, .. } => self.rolling = true,
            Event::PlaybackEnded { ending, .. } => {
                self.playing = None;
                // Leaving Now Playing on screen with nothing playing would be a dead end.
                if matches!(self.nav.current(), StageView::NowPlaying) {
                    self.nav.pop();
                }
                if matches!(ending, Ending::Complete { .. }) {
                    self.push_toast(Notice::info("episode finished"));
                }
            }
            Event::Sync(state) => {
                // Re-authorisation is the one sync condition worth interrupting for: until it
                // is done, everything queues silently and the user would have no idea.
                let announce = state.needs_reauth
                    && !self.sync.iter().any(|s| s.tracker == state.tracker && s.needs_reauth);
                if announce {
                    let accounts = self.section_key(Section::Accounts);
                    self.push_toast(Notice::alert(format!(
                        "{} needs signing in again — press {accounts} for Accounts",
                        state.tracker
                    )));
                }
                match self.sync.iter_mut().find(|s| s.tracker == state.tracker) {
                    Some(existing) => *existing = *state,
                    None => self.sync.push(*state),
                }
            }
            Event::JobStarted { job, activity } => self.start_job(job, activity),
            Event::JobNote { job, note, wait } => {
                let until = wait.map(|wait| self.clock.now_instant() + wait);
                if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job) {
                    job.note = Some((note, until));
                }
            }
            Event::JobDone(job) => self.jobs.retain(|j| j.id != job),
            Event::ProgressQueued => {
                // Optimistic, and safe to be: the row is already in SQLite by the time this
                // arrives, so the count can only be corrected downward by the next drain.
                for state in &mut self.sync {
                    state.outbox = state.outbox.saturating_add(1);
                }
            }
            Event::Conflicts(rows) => {
                let appeared = !rows.is_empty() && self.conflicts.is_empty();
                self.conflicts = rows;
                if appeared {
                    // Surfaced rather than resolved — the whole point of not guessing.
                    let settle = self.key_for(Action::ShowConflicts);
                    self.push_toast(Notice::info(format!(
                        "{} — {settle} settles them",
                        anistream_model::wording::plural(
                            self.conflicts.len(),
                            "sync disagreement"
                        )
                    )));
                }
            }
            Event::Resumed { position } => {
                // Resume happens without asking — mpv is already at the right place by the time
                // this arrives. Saying so is what keeps it from feeling like a glitch.
                self.push_toast(Notice::info(format!(
                    "resumed at {}",
                    NowPlaying::clock(position)
                )));
                if let Some(playing) = &mut self.playing {
                    playing.position = position;
                }
            }
            Event::Vpn(guard) => {
                // No guard empties the badge rather than removing it.
                let (badge, leaking) =
                    guard.as_ref().map_or_else(|| (String::new(), false), vpn_badge);
                // A newly-failing guard is worth interrupting for: torrents have just been
                // paused and the user needs to know why playback stopped.
                if leaking && !self.vpn_leaking {
                    self.push_toast(Notice::alert(format!("{badge} — torrents paused")));
                }
                self.vpn_badge = Some(badge);
                self.vpn_leaking = leaking;
            }
            Event::Episodes { id, rows } => {
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
            Event::Progress { id, progress, resume, episodes } => {
                // Every entry of the title on screen says it alike: its page and its rows in the
                // list under it. Only this title's: several shows on Home can share a
                // next-episode number.
                for entry in self.detail.iter_mut().chain(match &mut self.content {
                    Content::Entries(entries) => entries.iter_mut(),
                    _ => [].iter_mut(),
                }) {
                    if entry.id == id {
                        entry.progress = progress.map(|p| (p.done, p.next));
                        entry.resume = resume;
                    }
                }
                // The table, row by row, when it is this title's: while detached the viewer can
                // be looking at another show entirely, whose rows must not take this one's.
                // In place, so the cursor and the filter's view stay as they are.
                if self.episodes_for == Some(id) {
                    for list in [&mut self.episodes, &mut self.episodes_all] {
                        for row in list.iter_mut() {
                            if let Some(said) = episodes.iter().find(|e| e.number == row.number)
                            {
                                row.watched = said.watched;
                                row.completed = said.completed;
                            }
                        }
                    }
                }
                // The answer to a mark made here. Under the unwatched filter a marked row
                // leaves the view at once — which is the binge-marking flow working.
                if self.marking == Some(id) {
                    self.marking = None;
                    self.apply_episode_filter();
                }
            }
        }
    }

    /// A cover decoded from the bytes an [`Event::ArtworkReady`] brought, or `None` when they
    /// were not an image — a failure like any other, tried again after the backoff.
    pub fn artwork_decoded(&mut self, url: &str, image: Option<image::DynamicImage>) {
        match image {
            Some(image) => {
                self.images.insert(url, image);
                self.images.requests().network_returned();
            }
            None => self.images.mark_failed(url, false),
        }
    }

    pub fn push_toast(&mut self, notice: Notice) {
        // In the terminal's words, before it is logged or compared: the line on screen and in
        // the log is the one with the key in it.
        let notice = worded(notice, &self.keymap);
        // Every toast is also a log line. A toast lives for a few seconds and the stack is
        // capped at three, so without this the *third* provider failure silently erases the
        // first — and when a source breaks, the earliest error is usually the one that explains
        // the rest. This is the whole reason the Logs overlay exists.
        self.log(notice.kind, notice.text.clone());
        // The same message again is the same message, kept up longer rather than stacked: five
        // presses of an arrow on Settings are one "saved", not a pile of identical lines. From
        // anywhere in the stack, and brought up to newest: a copy left under another line was
        // counted in the rule's "+N", so one message read as two.
        if let Some(at) = self.toasts.iter().position(|t| t.notice == notice) {
            self.toasts.remove(at);
        }
        // Cap the stack so a burst of provider failures cannot cover the screen.
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast::from(notice));
    }

    /// Append to the log ring, newest last.
    pub fn log(&mut self, kind: NoticeKind, text: String) {
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
            .find(|chip| chip.state == SourceState::Ready)
            .or_else(|| sources.iter().find(|chip| chip.state != SourceState::Off))
            .or_else(|| sources.first())
            .map_or_else(|| "no source".into(), |chip| chip.id.clone())
    }

    /// Each configured source, in the order failover walks them, and how it stands: see
    /// [`source_states`].
    pub fn source_states(&self) -> Vec<SourceChip> {
        source_states(&self.settings.sources, &self.providers, self.provider_note.is_some())
    }

    /// Badge counts for the rail: work in flight, questions pending.
    pub fn rail_counts(&self) -> Vec<(Section, u32)> {
        let mut counts = Vec::new();
        let active = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Active | DownloadState::Queued))
            .count() as u32;
        if active > 0 {
            counts.push((Section::Downloads, active));
        }
        if !self.conflicts.is_empty() {
            counts.push((Section::Accounts, self.conflicts.len() as u32));
        }
        counts
    }

    /// Whether a mark made here is waiting on the engine's word: see `marking`.
    pub fn awaiting_mark(&self) -> bool {
        self.marking.is_some()
    }

    /// Whether the screen is waiting on the engine's word before it is drawn again or reads
    /// another key: a mark made here, or the staging of the episode an ending rolled on to.
    /// See `marking` and `rolling`.
    pub fn awaiting_engine(&self) -> bool {
        self.marking.is_some() || self.rolling
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
            // The cursor is the row `K` and `J` move up and down the chain.
            StageView::Section(Section::Providers) => Some(Rows::Providers),
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
    fn move_cursor(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
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
            Rows::Settings => SettingKey::ROWS.len(),
            Rows::Providers => self.providers.len(),
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
            Rows::Downloads | Rows::Accounts | Rows::Settings | Rows::Providers => {
                &mut self.selected
            }
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
            // The slot holds one task, and a section's reload waiting in it is not displaced.
            // Losing a backfill costs nothing: the next scroll asks again.
            || self.pending.is_some()
        {
            return;
        }
        let Some(anchor) = self.calendar_anchor else { return };
        self.calendar_loading_past = true;
        self.pending =
            Some(Command::LoadCalendarPast { anchor, page: self.calendar_pages + 1 });
    }

    /// Handle a keystroke: text into a focused field first, then the binding it resolves to.
    ///
    /// The whole route from key to action in one place, so what a key does in any state can be
    /// tested as a key rather than as the action someone assumed it would become.
    pub fn press(&mut self, key: KeyEvent, visible_rows: usize) -> Option<Command> {
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
    pub fn handle(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
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
    ) -> Option<Command> {
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
                cycle_search_filter(
                    &mut self.search_filter,
                    self.overlay_selected,
                    false,
                    &self.clock,
                );
            }
            Action::Right if *overlay == Overlay::SearchFilters => {
                cycle_search_filter(
                    &mut self.search_filter,
                    self.overlay_selected,
                    true,
                    &self.clock,
                );
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
    fn perform(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
        // The download queue reads Space as pause and `x` as cancel, the keys it borrows from the
        // player. Resolved before the playback table: idle, those keys arrive as the browsing
        // actions — play next, remove from Continue — so a download could never be paused or
        // cancelled at all, and while something plays they would pause the player rather than
        // the row on screen.
        if self.in_downloads_stage() {
            match action {
                Action::PlayNext | Action::PlayPause => {
                    let id = self.downloads.get(self.selected)?.id;
                    return Some(Command::DownloadPause { id });
                }
                Action::HideFromContinue | Action::StopPlayback => {
                    let row = self.downloads.get(self.selected)?;
                    // A finished download has nothing left to cancel. Cancelling it forgot the
                    // row and left the file on disk with no way back to it from here.
                    if row.state == DownloadState::Done {
                        let delete = self.key_for(Action::DeleteDownload);
                        self.push_toast(Notice::info(format!(
                            "already downloaded — {delete} deletes it and its file"
                        )));
                        return None;
                    }
                    return Some(Command::DownloadCancel { id: row.id });
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
            self.push_toast(Notice::info("nothing selected"));
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
                    self.push_toast(Notice::info("nothing to resolve"));
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
                if let Some(row) = self.settings.rows.get(self.selected)
                    && let SettingChange::Typed { text } = &row.change
                {
                    self.edit_target = Some(row.key);
                    self.edit_value = text.clone();
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
                    (Some(path), DownloadState::Done) => {
                        // First-class playback: same staging, history and sync as a stream.
                        self.raise_now_playing_titled(
                            row.anilist_id,
                            row.title.clone(),
                            &row.episode,
                        );
                        return Some(Command::PlayLocal {
                            id: row.anilist_id,
                            episode: row.episode,
                            title: row.title,
                            path: path.clone(),
                        });
                    }
                    (_, DownloadState::Failed) => {
                        // The reason is on the row; surfacing it is more use than a dead keypress.
                        let reason =
                            row.error.clone().unwrap_or_else(|| "no reason recorded".into());
                        self.push_toast(Notice::alert(reason));
                    }
                    _ => self.push_toast(Notice::info("not finished yet")),
                }
            }
            Action::DeleteDownload if self.in_downloads_stage() => {
                let row = self.downloads.get(self.selected)?;
                return Some(Command::DownloadDelete { id: row.id });
            }
            Action::ClearCompleted if self.in_downloads_stage() => {
                return Some(Command::DownloadClearCompleted);
            }
            // From the rail too, like Enter here: the table is the only thing on the screen.
            Action::RaiseSource if self.on_providers() => return self.move_source(-1),
            Action::LowerSource if self.on_providers() => return self.move_source(1),
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
                    return Some(Command::LoadEpisodes(id));
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
                // Shown once the engine has written it and says where the title now stands.
                self.marking = Some(id);
                return Some(Command::SetWatched { id, episodes: vec![number], watched });
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
                    self.push_toast(Notice::info(why));
                    return None;
                }
                self.marking = Some(id);
                self.push_toast(Notice::info(format!(
                    "marked {} watched",
                    anistream_model::wording::plural(marked.len(), "episode")
                )));
                return Some(Command::SetWatched { id, episodes: marked, watched: true });
            }
            Action::DownloadRange if self.in_episodes() => {
                self.range_query.clear();
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::DownloadRange);
            }
            Action::DownloadRange => {
                self.push_toast(Notice::info("ranges are queued from the episode table"));
            }
            Action::Filter if self.in_episodes() => {
                self.episode_filter = self.episode_filter.next();
                self.apply_episode_filter();
                self.push_toast(Notice::info(format!(
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
                self.push_toast(Notice::info("filters live in search and the episode table"));
            }
            Action::ToggleWatched | Action::MarkAllPrevious => {
                self.push_toast(Notice::info("watched marks live in the episode table"));
            }
            Action::OpenInBrowser => {
                let Some(id) = self.target().map(|e| e.id) else {
                    self.push_toast(Notice::info("nothing selected"));
                    return None;
                };
                return Some(Command::OpenExternal {
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
                    self.push_toast(Notice::info(hint));
                    return None;
                }
                if self.providers.is_empty() {
                    // Plugins register a moment after launch, so "none yet" is a real state
                    // rather than a misconfiguration.
                    self.push_toast(Notice::info("no sources registered yet"));
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
                    self.push_toast(Notice::info("nothing selected"));
                    return None;
                };
                let Some(url) = entry.trailer_url.clone() else {
                    // Rides the detail fetch, so a list row answers after the title has
                    // been opened once — and some titles genuinely have no trailer.
                    let why = self
                        .off_the_page("the trailer is found")
                        .unwrap_or_else(|| "no trailer on record for this title".into());
                    self.push_toast(Notice::info(why));
                    return None;
                };
                return Some(Command::PlayTrailer { url, title: entry.title.clone() });
            }
            Action::Recommendations => {
                let Some(entry) = self.target() else {
                    self.push_toast(Notice::info("nothing selected"));
                    return None;
                };
                if entry.recommended.is_empty() {
                    // Like relations, recommendations ride the detail fetch — and some
                    // titles genuinely have none.
                    let why = self
                        .off_the_page("recommendations are listed")
                        .unwrap_or_else(|| "no recommendations known for this title".into());
                    self.push_toast(Notice::info(why));
                    return None;
                }
                self.overlay_selected = 0;
                self.nav.open_overlay(Overlay::Recommendations);
            }
            Action::WatchOrder => {
                let Some(entry) = self.target() else {
                    self.push_toast(Notice::info("nothing selected"));
                    return None;
                };
                if entry.related.is_empty() {
                    // Empty on list rows too — the relations ride the detail fetch. From a
                    // list this still answers after opening the title once.
                    let why = self
                        .off_the_page("connected seasons are listed")
                        .unwrap_or_else(|| "no connected seasons known for this title".into());
                    self.push_toast(Notice::info(why));
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
                    self.push_toast(Notice::info(hint));
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
                return Some(Command::LoadSources { id, episode });
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
                return Some(Command::PlayParty { id, episode });
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
                return Some(Command::DownloadEpisode { id, episode });
            }
            Action::ToggleSynopsis => self.synopsis_expanded = !self.synopsis_expanded,
            Action::SetListStatus => {
                if self.signed_in_trackers().is_empty() {
                    // Nothing keeps a list status but a tracker. Offering the picker anyway
                    // ended in "marked watching" for a choice that went nowhere.
                    self.push_toast(Notice::info(
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
                    self.push_toast(Notice::info("nothing selected"));
                }
            }
            Action::RateTitle => {
                if self.signed_in_trackers().is_empty() {
                    self.push_toast(Notice::info(
                        "ratings are kept on a tracker, and none is signed in — :accounts",
                    ));
                } else if self.target().is_some() {
                    // No local record of an existing score to preselect, so the cursor
                    // starts at the top and every row is one or two keystrokes away.
                    self.overlay_selected = 0;
                    self.nav.open_overlay(Overlay::Rate);
                } else {
                    self.push_toast(Notice::info("nothing selected"));
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
                    self.push_toast(Notice::info("removing works on the Continue list"));
                    return None;
                }
                let Content::Entries(entries) = &mut self.content else {
                    return None;
                };
                if entries.is_empty() {
                    return None;
                }
                // Dropped from the screen before the write: the list answers immediately, and
                // the store catches up.
                let entry = entries.remove(self.selected.min(entries.len() - 1));
                self.selected = self.selected.min(self.content.len().saturating_sub(1));
                self.offset = self.offset.min(self.selected);
                // Say how to undo, because there is no key for it — the way back is the
                // same thing that put it here.
                self.push_toast(Notice::info(format!(
                    "{} removed — watching it again brings it back",
                    entry.title
                )));
                return Some(Command::HideFromContinue { id: entry.id });
            }
            Action::ForceResync => {
                if self.sync.is_empty() {
                    let accounts = self.section_key(Section::Accounts);
                    self.push_toast(Notice::info(format!(
                        "no tracker connected — press {accounts} for Accounts"
                    )));
                } else {
                    return Some(Command::SyncNow);
                }
            }
            // On the store screens, refreshing means asking the stores again, not re-checking
            // the provider table they were opened from.
            Action::Refresh if self.in_stores() || self.store_view().is_some() => {
                return Some(Command::LoadStores);
            }
            Action::Refresh if self.members_view().is_some() => {
                let store = self.members_view()?.to_owned();
                return Some(Command::LoadMembers { store });
            }
            // A title page and an episode table refresh themselves, not the section's list
            // under them — which is what `r` used to fetch, saying "loading the calendar" over
            // a title page that never changed.
            Action::Refresh if matches!(self.nav.current(), StageView::Title(_)) => {
                self.images.requests().retry_failed();
                return self.title_on_screen().map(Command::LoadDetail);
            }
            Action::Refresh if self.in_episodes() => {
                // The rows stay on screen until the answer lands, and the answer keeps the
                // cursor on its episode; the activity row says the table is being asked again.
                self.images.requests().retry_failed();
                return self.title_on_screen().map(Command::LoadEpisodes);
            }
            Action::Refresh => {
                // Asking again means the covers too: one that failed while the network was
                // down should not wait out its backoff once you have said it is back.
                self.images.requests().retry_failed();
                return self.reload();
            }
            Action::ToggleTranslation => {
                // For this session: the saved choice is the Settings row's to change.
                let translation = self.settings.translation.toggled();
                self.push_toast(Notice::info(format!(
                    "now using {translation} for this session"
                )));
                return Some(Command::SetTranslation { translation });
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
                self.push_toast(Notice::info(format!(
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
    fn handle_playback(&mut self, action: Action) -> Option<Option<Command>> {
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
                self.push_toast(Notice::info("detached — mpv is still playing"));
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
                        self.push_toast(Notice::info("no skip here"));
                        return Some(None);
                    }
                }
            }
            Action::NextEpisode => return Some(self.step_episode(1)),
            Action::PreviousEpisode => return Some(self.step_episode(-1)),
            _ => return None,
        };
        Some(Some(Command::Player(command)))
    }

    /// Start the episode selected in the timing sheet.
    fn play_selected_episode(&mut self) -> Option<Command> {
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
    fn play_next_unwatched(&mut self) -> Option<Command> {
        let Some(entry) = self.target() else {
            self.push_toast(Notice::info("nothing selected"));
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
    fn step_episode(&mut self, delta: i64) -> Option<Command> {
        self.playing.as_ref()?;
        Some(Command::Player(PlayerCommand::Step(delta)))
    }

    /// The episode table for a title, when the rows loaded are that title's — what the player's
    /// Episodes menu lists. Empty for any other title rather than another show's rows.
    pub fn episode_rows_for(&self, id: AnilistId) -> &[EpisodeRow] {
        // Every row, not the filtered view: a filter on the title screen is about what to look
        // at there, not about which episodes exist. Asked by the rows' own title rather than
        // `detail`, which moves to a newly opened title before that title's rows arrive.
        if self.episodes_for == Some(id) { &self.episodes_all } else { &[] }
    }

    /// Push Now Playing, raise the eyecatch, and ask for the stream.
    ///
    /// The order matters: the wipe goes up *before* resolution starts, so the user never sees
    /// a frozen episode table while a provider is being tried.
    fn begin_playback(&mut self, id: AnilistId, episode: String) -> Command {
        self.raise_now_playing(id, &episode);
        Command::Play { id, episode }
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
        let playing = NowPlaying {
            id: Some(id),
            title,
            episode: episode.to_owned(),
            // From this title's table only: the open one can be another show's.
            episode_title: self
                .episode_rows_for(id)
                .iter()
                .find(|e| e.number == episode)
                .and_then(|e| e.title.clone()),
            speed: self.settings.speed.unwrap_or(1.0),
            ..NowPlaying::default()
        };
        self.put_up(playing);
    }

    /// Now Playing for a playback just staged: the eyecatch over it while its stream is found,
    /// and the view pushed.
    fn put_up(&mut self, playing: NowPlaying) {
        self.eyecatch =
            Some(Eyecatch::new(format!("{}  ·  ep {}", playing.title, playing.episode)));
        // One that has reported keeps playing until this one is found; one merely staged before
        // this was never going to play.
        let replaced = self.playing.take().filter(|p| p.duration.is_some());
        if replaced.is_some() && self.superseded.is_none() {
            self.superseded = replaced;
        }
        self.playing = Some(playing);
        self.nav.push(StageView::NowPlaying);
    }

    /// A playback the engine has staged, staged here as it says: see [`Event::PlaybackStaged`].
    ///
    /// Idempotent. A playback staged here already — as this asked for it, or as the engine said
    /// it once before — is left as it is, eyecatch and all: it is the same playback while it has
    /// not started playing, and the same title and episode.
    ///
    /// The episode an ending rolled on to takes the finished one's place: that one comes down
    /// as any ending takes it, Now Playing with it, and goes back up for this one at once.
    fn take_staged(
        &mut self,
        id: AnilistId,
        title: String,
        episode: String,
        episode_title: Option<String>,
        speed: f64,
    ) {
        if std::mem::take(&mut self.rolling) {
            self.playing = None;
            if matches!(self.nav.current(), StageView::NowPlaying) {
                self.nav.pop();
            }
        } else if self.playing.as_ref().is_some_and(|playing| {
            playing.id == Some(id) && playing.episode == episode && playing.duration.is_none()
        }) {
            return;
        }
        let playing = NowPlaying {
            id: Some(id),
            title,
            episode,
            episode_title,
            speed,
            ..NowPlaying::default()
        };
        self.put_up(playing);
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

    /// Take the work the reducer queued outside a key's answer: see `pending`.
    pub fn take_pending(&mut self) -> Option<Command> {
        self.pending.take()
    }

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
    fn toggle_account(&mut self) -> Option<Command> {
        let tracker = self.sync.get(self.selected)?.clone();
        self.account_action(tracker)
    }

    /// What Enter means for one tracker row: out if in, in if out — and for Kitsu, "in"
    /// is a prompt right here rather than a browser or a trip to the command line.
    fn account_action(&mut self, tracker: SyncState) -> Option<Command> {
        if tracker.connected && !tracker.needs_reauth {
            return Some(Command::Disconnect { tracker: tracker.tracker });
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
        Some(Command::Connect { tracker: tracker.tracker })
    }

    /// The trackers a status or score is queued for: those holding a credential, the same
    /// rule the queueing side applies. A tracker waiting on a fresh sign-in still counts —
    /// its queue is kept and drains once it is signed in again.
    fn signed_in_trackers(&self) -> Vec<&str> {
        self.sync.iter().filter(|s| s.connected).map(|s| s.tracker.as_str()).collect()
    }

    /// Whether the Settings rows have focus.
    ///
    /// Left and Right cycle a value here rather than moving between rail and stage — the same
    /// trade Library already makes for its status segments. Esc, Tab and the number keys still
    /// leave, so nothing is trapped.
    fn in_settings_stage(&self) -> bool {
        self.nav.section() == Section::Settings
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Settings))
    }

    /// Whether the Library list has focus, so horizontal keys mean "step segment".
    fn in_library_stage(&self) -> bool {
        self.nav.section() == Section::Library
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Library))
    }

    /// Step the Library's segment, and from the first one carry Left on to the rail — the
    /// screen's left edge, where Left goes everywhere else.
    fn step_segment(&mut self, delta: isize) -> Option<Command> {
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
        Some(Command::LoadLibrary(self.library_segment))
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
                self.push_toast(Notice::info(format!("at the top — {quit} quits")));
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
    fn step_setting(&mut self, delta: isize) -> Option<Command> {
        let moves = match self.settings.rows.get(self.selected)?.change {
            SettingChange::Steps { back, forward } => {
                if delta < 0 {
                    back
                } else {
                    forward
                }
            }
            // Typed or read-only: Right still says how that row is changed.
            SettingChange::Typed { .. } | SettingChange::Fixed if delta > 0 => {
                return self.cycle_setting(delta);
            }
            SettingChange::Typed { .. } | SettingChange::Fixed => false,
        };
        if moves {
            return self.cycle_setting(delta);
        }
        // At the end it was stepping toward: nothing to write, and Left goes on to the rail.
        if delta < 0 {
            self.nav.focus_rail();
        }
        None
    }

    /// Confirm the selection in whichever overlay is open.
    ///
    /// Overlays own their own selection index so closing one cannot disturb the list behind it.
    fn confirm_overlay(&mut self, visible_rows: usize) -> Option<Command> {
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
                self.push_toast(Notice::info(format!(
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
                Some(Command::SetStatus { id, status })
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
                Some(Command::Search { query, filter: self.search_filter.clone() })
            }
            Some(Overlay::Rate) => {
                let entry = self.target()?;
                let id = entry.id;
                // Rows run 10 down to 1, so the row index is the distance from 10.
                let score = (10 - self.overlay_selected.min(9)) as f32;
                self.nav.close_overlay();
                self.push_toast(Notice::info(format!(
                    "rated {score}/10 — queued for {}",
                    self.signed_in_trackers().join(", ")
                )));
                Some(Command::SetScore { id, score })
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
                Some(Command::ResolveConflict { id: row.anilist_id, keep_local: true })
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
                Some(Command::SetProviderPreference { id, provider })
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
                Some(Command::LoadDetail(related.id))
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
                Some(Command::LoadDetail(pick.id))
            }
            Some(Overlay::Sources) => {
                let candidate = self.sources.get(self.overlay_selected)?.clone();
                let (id, episode) = self.source_context.clone()?;
                self.nav.close_overlay();
                self.overlay_selected = 0;
                self.sources.clear();
                self.source_context = None;
                self.raise_now_playing(id, &episode);
                Some(Command::PlaySource {
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
                self.push_toast(Notice::info(format!("matched to {}", candidate.title)));
                Some(Command::FixMatch { id, provider_id, key: candidate.key })
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
                Some(Command::PublishPlugin { store, plugin })
            }
            _ => None,
        }
    }

    /// Rows the currently-open overlay is listing, so movement can be bounded.
    fn overlay_len(&self) -> usize {
        match self.nav.overlay() {
            Some(Overlay::ListStatus) => LibrarySegment::ALL.len(),
            Some(Overlay::Rate) => 10,
            Some(Overlay::SearchFilters) => SEARCH_FILTER_ROWS.len(),
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

    /// The Settings screen's rows, as the config in effect and the file's make them, each
    /// under its heading and with the terminal's note.
    pub fn setting_rows(&self) -> Vec<SettingLine> {
        self.settings
            .rows
            .iter()
            .map(|row| SettingLine {
                label: row.key.label(),
                category: row.key.category(),
                editable: row.change.editable(),
                note: self.setting_note(row.key),
                key: row.key,
                value: row.value.clone(),
            })
            .collect()
    }

    /// What the terminal says under a Settings row: how it is changed here, why it is not, or
    /// when a change takes effect. The terminal's words, with its keys in them.
    fn setting_note(&self, key: SettingKey) -> Option<&'static str> {
        use SettingKey as S;
        Some(match key {
            S::Subtitles => "enter types a language code — too many values to cycle",
            S::MarkChapters => "chapter marks on the seek bar, where the stream has none",
            S::Fullscreen => "mpv opens fullscreen; its own f key still leaves it",
            // Read per playback, so the next episode uses this; the player's own menu changes
            // the one playing.
            S::Upscaling => "adapts to the source and your GPU · from the next episode",
            S::PlayerUi => "thumbnails, menus and the skip button in anistream's look",
            S::NightMode => "evens out quiet dialogue and loud music",
            S::DownloadDir => "enter types a path — applies to the next download",
            S::MergeSubtitles => "applies to the next download that finishes",
            // The privacy dial; saying so is the point of the row.
            S::KeepSeeding => "on keeps advertising you as a source — applies immediately",
            S::DownloadHook => "enter types a command — run after each finished download",
            // `y` is the ordinary way in; this toggle sends *every* play there.
            S::Syncplay => "y plays one episode with the room set below — on sends every play",
            S::SyncplayServer => "enter types host:port",
            S::SyncplayRoom => "enter types the room everyone joins",
            S::UpdateCheck => "one cached request a day, checked at startup",
            // Enabling it without a client id is a no-op, and a toggle that appears to work
            // while doing nothing is worse than one that says why.
            S::Presence if self.settings.presence_client => "takes effect on the next episode",
            S::Presence => "needs a client id — register an app at discord.com/developers",
            // The privacy dial. Off still publishes that you are watching *something*, which
            // is the point for anyone who wants the presence without the title.
            S::PresenceTitle => "off shows \"Watching anime\" without naming the show",
            S::Torrents => "needs a VPN configured; takes effect immediately",
            S::VpnMode => "edit config.toml — turning this off has to be deliberate",
            S::SeedRatio => "keeps watched episodes seeding while anistream is open",
            S::SeedHours => "the longest one episode keeps seeding, whatever its ratio",
            S::UploadLimit => "caps all torrent upload, so seeding never slows playback",
            S::TokenStorage => "use --token-to-file to move an existing token",
            S::Theme
            | S::Motion
            | S::Translation
            | S::Quality
            | S::CommitThreshold
            | S::AutoNext
            | S::SkipOpening
            | S::SkipFiller
            | S::SourceOrder => return None,
        })
    }

    /// The name the Settings screen gives the setting stored at `table`.`key`, for saying it
    /// was saved. The config key (`commit_threshold`) is the file's vocabulary, not the
    /// screen's, and the value needs no repeating: the row beside it already shows it.
    ///
    /// `None` for what is stored without a row of its own, the remembered speed and volume:
    /// saved in passing, and not something anyone asked to have confirmed.
    pub fn setting_label(&self, table: &[&str], key: &str) -> Option<&'static str> {
        SettingKey::at(table, key).map(SettingKey::label)
    }

    /// Step the selected setting through its values.
    ///
    /// Cycling rather than free entry, because every editable setting here has a small closed set
    /// of sensible values — and a cycler cannot produce a config the app then refuses to load.
    fn cycle_setting(&mut self, delta: isize) -> Option<Command> {
        let id = *SettingKey::ROWS.get(self.selected)?;
        let rows = self.setting_rows();
        match id.path() {
            Some(_) if rows.get(self.selected)?.editable => {}
            _ => {
                let note = rows.get(self.selected).and_then(|r| r.note);
                self.push_toast(Notice::info(note.unwrap_or("not editable here")));
                return None;
            }
        };
        // Text values are typed, not cycled — arrows have nothing to step through.
        if id.typed() {
            self.push_toast(Notice::info("enter types a value here"));
            return None;
        }
        // The engine steps it, by the rules every front end shares, and says the outcome.
        Some(Command::StepSetting { key: id, delta: delta as i64 })
    }

    /// The settings the engine says are in effect, in place of the last.
    ///
    /// A new theme repaints, from the variant detection found at startup: re-running detection
    /// would read the terminal's background over OSC 11 from inside the render loop, which
    /// competes with the key reader for stdin. A refused write has had its setting put back
    /// already; the status line falls back to what is in effect, rather than a message about an
    /// earlier save, and a refused theme repaints as the one in effect. The order has no row:
    /// its Providers rows come back with the chain's own, sent with the refusal.
    fn take_settings(&mut self, view: SettingsView) {
        use anistream_core::config::ThemeMode;
        let refused = view.refused.filter(|key| *key != SettingKey::SourceOrder);
        let repaint = view.theme != self.settings.theme || refused == Some(SettingKey::Theme);
        self.settings = view;
        if repaint {
            self.palette = crate::theme::Palette::of(match self.settings.theme {
                ThemeMode::Immersive => crate::theme::Variant::Immersive,
                ThemeMode::Adaptive => self.adaptive_variant,
            });
        }
        if refused.is_some() {
            self.status.clear();
        }
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

    fn handle_while_typing(&mut self, action: Action, visible_rows: usize) -> Option<Command> {
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
                return Some(Command::ConnectKitsu { username, password });
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
                return Some(Command::PreviewStore { link: Secret(link) });
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::InviteName) => {
                let name = self.invite_name.trim().to_owned();
                if name.is_empty() {
                    return None;
                }
                let store = self.store_view()?.to_owned();
                self.invite_name.clear();
                self.nav.close_overlay();
                return Some(Command::CreateInvite { store, name: Some(name) });
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
                if !id.typed() {
                    return None;
                }
                let value = self.edit_value.trim().to_owned();
                self.nav.close_overlay();
                if value.is_empty() {
                    // The writer sets keys; it does not delete them. Refusing beats
                    // writing an empty string that half the readers treat as a path.
                    self.push_toast(Notice::info(
                        "left unchanged — delete the line in config.toml to unset",
                    ));
                    return None;
                }
                // No toast here: the write reports itself once it has landed, which is the
                // moment "saved" is true. Saying it now as well put two confirmations up.
                let value = anistream_core::settings::SettingValue::Str(value);
                return Some(Command::SetSetting { key: id, value });
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
                    self.push_toast(Notice::info("ranges look like 4, 1-12 or 7-"));
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
                    self.push_toast(Notice::info("no episodes in that range"));
                    return None;
                }
                self.nav.close_overlay();
                self.push_toast(Notice::info(format!(
                    "queueing {}…",
                    anistream_model::wording::plural(episodes.len(), "episode")
                )));
                return Some(Command::DownloadMany { id, episodes });
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
                    return Some(Command::ClearMatch { id });
                }
                return Some(Command::ManualSearch { id, query });
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
                return Some(Command::Search { query, filter: self.search_filter.clone() });
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
    pub fn open_selected(&mut self) -> Option<Command> {
        let Some(entry) = self.selected_entry().cloned() else {
            self.push_toast(Notice::info("nothing selected"));
            return None;
        };
        let id = entry.id;
        self.detail = Some(entry);
        self.nav.push(StageView::Title(id));
        Some(Command::LoadDetail(id))
    }

    /// Move the rail selection by `delta`, clamping at the ends.
    ///
    /// Clamps rather than wraps: a wrapping rail means holding ↓ cycles forever with no sense of
    /// where you are, and eight sections is short enough that hitting the end is informative.
    fn step_section(&mut self, delta: isize) -> Option<Command> {
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
            // Through `pending` because the idle tick has no return channel, the same route a
            // scroll's backfill takes.
            self.pending = Some(task);
        }
    }

    pub fn go_to_section(&mut self, section: Section) -> Option<Command> {
        self.nav.go_to(section);
        self.selected = 0;
        self.offset = 0;
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        self.reload()
    }

    /// Start a [`Job`] for `task`, numbered here, if it is work the user will wait on.
    ///
    /// For a still frame or a test that stages one. The running app's jobs are numbered where
    /// the work runs, which says so with [`Event::JobStarted`]; the runner ends each with
    /// [`Event::JobDone`] when the work stops.
    pub fn begin_job(&mut self, task: &Command) -> Option<JobId> {
        let label = task.activity()?;
        self.next_job += 1;
        let id = JobId::new(self.next_job);
        self.start_job(id, label);
        Some(id)
    }

    /// A job begun now, by this clock, so the time it shows counts from the moment the
    /// interface learned of it.
    fn start_job(&mut self, id: JobId, label: String) {
        self.jobs.push(Job { id, label, started: self.clock.now_instant(), note: None });
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

    /// The list the current view shows, if it shows one — what an [`Event::Content`] has to
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
    pub fn content_here(&self, content: Content) -> Event {
        // With no list view on screen the reducer drops every tag, so which one is moot.
        let view = self.content_view().unwrap_or(ContentFor::Continue);
        Event::Content { view, content }
    }

    /// [`Self::content_here`] for the open title's episode table.
    pub fn episodes_here(&self, rows: Vec<EpisodeRow>) -> Event {
        // No title open means the reducer drops the rows whatever id they carry.
        let id = self.detail.as_ref().map_or(AnilistId::new(0), |d| d.id);
        Event::Episodes { id, rows }
    }

    /// [`Self::content_here`] for the open source picker.
    pub fn sources_here(&self, candidates: Vec<SourceCandidate>) -> Event {
        let (id, episode) =
            self.source_context.clone().unwrap_or((AnilistId::new(0), String::new()));
        Event::Sources { id, episode, candidates }
    }

    /// Work needed to populate the current view.
    pub fn reload(&mut self) -> Option<Command> {
        // Whatever is fetched next replaces the content, so the calendar's paging describes a
        // list that is about to stop existing. Leaving it set would let a scroll on the new
        // screen ask for page four of a timeline nobody is looking at any more.
        self.calendar_anchor = None;
        self.calendar_pages = 0;
        self.calendar_more_past = false;
        self.calendar_loading_past = false;

        let task = match self.nav.section() {
            Section::Home => Command::LoadContinue,
            Section::Seasonal => Command::LoadSeasonal,
            Section::Calendar => Command::LoadCalendar,
            Section::Search
                if !self.search_query.trim().is_empty() || !self.search_filter.is_empty() =>
            {
                // Recorded as submitted, the same as Enter does, because the answer is tagged
                // with this query and only taken while it is still the one on screen.
                let query = self.search_query.trim().to_owned();
                self.searched_query = query.clone();
                Command::Search { query, filter: self.search_filter.clone() }
            }
            Section::Providers => Command::CheckProviders,
            Section::Downloads => Command::LoadDownloads,
            Section::Library => Command::LoadLibrary(self.library_segment),
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

    /// `K` or `J` on Providers: the selected source one place up or down the chain, saved.
    ///
    /// The rows move at once and the cursor with its row; the run loop puts the live chain in
    /// the same order once the file has it. At either end nothing moves and nothing is saved.
    fn move_source(&mut self, step: isize) -> Option<Command> {
        let id = self.providers.get(self.selected)?.id.clone();
        let order =
            order_with_source_moved(&self.settings.sources.order, &self.providers, &id, step)?;
        // Every row is named in the new order, so its place there is its place in the chain.
        let place = |row: &ProviderRow| order.iter().position(|entry| *entry == row.id);
        self.providers.sort_by_key(|row| place(row).unwrap_or(order.len()));
        self.selected = self.providers.iter().position(|row| row.id == id).unwrap_or(0);
        // Taken as the order here before the engine says it is, so a second press moves on
        // from this one rather than from the order before it. The engine's answer is the same
        // order, or on a refusal the file's, with the chain's own rows.
        self.settings.sources.order.clone_from(&order);
        Some(Command::SetSetting {
            key: SettingKey::SourceOrder,
            value: anistream_core::settings::SettingValue::List(order),
        })
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
                self.push_toast(Notice::info(
                    "the invite checked out — paste it again to join",
                ));
            }
            Ok(preview) => {
                self.store_confirm = Some(StoreConfirm::Join(preview));
                self.nav.open_overlay(Overlay::StoreConfirm);
            }
            Err(reason) => {
                self.push_toast(Notice::alert(reason));
                // Back to the prompt with the link still in it, so a link cut short in the
                // copy can be finished rather than pasted again — when the user is still here.
                if self.in_stores() && !self.nav.has_overlay() {
                    self.nav.open_overlay(Overlay::JoinStore);
                }
            }
        }
    }

    fn open_stores(&mut self) -> Option<Command> {
        self.store_selected = 0;
        self.nav.push(StageView::Stores);
        Some(Command::LoadStores)
    }

    /// Enter on the store list: into a store, or the join prompt from the last row.
    fn open_store_row(&mut self) -> Option<Command> {
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

    fn open_store_line(&mut self, id: &str) -> Option<Command> {
        let store = self.store(id)?.clone();
        let line = *self.store_lines(&store).get(self.store_line_selected)?;
        match line {
            StoreLine::Plugin(i) => {
                let plugin = store.plugins.get(i)?.id.clone();
                self.show_plugin(&store.id, &plugin);
                None
            }
            StoreLine::LinkDevice => {
                Some(Command::CreateInvite { store: store.id, name: None })
            }
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
                Some(Command::LoadMembers { store: store.id })
            }
            StoreLine::Publish => {
                self.publish_for = Some(store.id.clone());
                self.local_plugins.clear();
                Some(Command::LoadLocalPlugins { store: store.id })
            }
            // No status line of its own: the activity row says it is opening, and a toast says
            // how it went. A status set here was never cleared, so it outlived both.
            StoreLine::Admin => Some(Command::OpenAdmin { store: store.id }),
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

    fn confirm_plugin_choice(&mut self) -> Option<Command> {
        let choice = self.plugin_options.get(self.overlay_selected)?.clone();
        self.plugin_options.clear();
        let name = self.plugin_in_action().map(|(_, p)| p.name.clone()).unwrap_or_default();
        let (store, plugin) = self.plugin_action.clone()?;
        self.nav.close_overlay();
        self.overlay_selected = 0;
        match choice {
            PluginChoice::Install | PluginChoice::Update(_) | PluginChoice::Replace => {
                Some(Command::InstallPlugin { store, plugin })
            }
            PluginChoice::Remove => Some(Command::RemovePlugin { plugin }),
            PluginChoice::Unpublish => {
                self.store_confirm = Some(StoreConfirm::Unpublish { store, plugin, name });
                self.nav.open_overlay(Overlay::StoreConfirm);
                None
            }
            PluginChoice::Cancel => None,
        }
    }

    fn confirm_store_action(&mut self) -> Option<Command> {
        let confirm = self.store_confirm.take()?;
        self.nav.close_overlay();
        Some(match confirm {
            StoreConfirm::Join(preview) => {
                self.join_link.clear();
                Command::JoinStore { link: preview.link }
            }
            StoreConfirm::Leave { store, .. } => Command::LeaveStore { store },
            StoreConfirm::RemoveMember { store, member, .. } => {
                Command::RemoveMember { store, member }
            }
            StoreConfirm::CancelInvite { store, invite, .. } => {
                Command::CancelInvite { store, invite }
            }
            StoreConfirm::Unpublish { store, plugin, .. } => {
                Command::UnpublishPlugin { store, plugin }
            }
        })
    }

    /// Enter on the members screen: remove a member, or cancel an unused invite.
    fn open_member_row(&mut self, store: &str) -> Option<Command> {
        let at = self.member_selected;
        if let Some(member) = self.store_members.get(at) {
            if member.you {
                self.push_toast(Notice::info("that is you — a store always keeps its owner"));
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

/// The FILTERS overlay's row labels, in order: one for each of [`SearchFilters`]' fields. The
/// cycling and display functions index against this.
///
/// The overlay's rather than the filters' own: which rows there are, the words a value is shown in
/// and the ladder each one steps through are how this screen offers a search. The filters it sends
/// hold wire values only.
pub const SEARCH_FILTER_ROWS: [&str; 6] =
    ["genre", "year", "season", "format", "status", "sort"];

/// AniList's fixed genre set. Genres are strings on the wire, but the set is closed —
/// typing one would invite misses on casing that a cycle cannot produce.
const SEARCH_GENRES: [&str; 18] = [
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
const SEARCH_SEASONS: [(&str, &str); 4] =
    [("WINTER", "winter"), ("SPRING", "spring"), ("SUMMER", "summer"), ("FALL", "fall")];
const SEARCH_FORMATS: [(&str, &str); 7] = [
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
const SEARCH_STATUSES: [(&str, &str); 3] =
    [("RELEASING", "airing"), ("FINISHED", "finished"), ("NOT_YET_RELEASED", "upcoming")];
const SEARCH_SORTS: [(&str, &str); 4] = [
    ("POPULARITY_DESC", "popularity"),
    ("SCORE_DESC", "score"),
    ("TRENDING_DESC", "trending"),
    ("START_DATE_DESC", "newest"),
];
/// Older than any TV anime that streams anywhere; the year cycle stops here.
const OLDEST_SEARCH_YEAR: u32 = 1970;

/// What a filter row currently shows: the display form, or "any" when unset.
pub fn search_filter_value(filter: &SearchFilters, row: usize) -> String {
    let shown = |set: &[(&str, &str)], value: &Option<String>| {
        value.as_deref().and_then(|v| {
            set.iter().find(|(wire, _)| *wire == v).map(|(_, word)| (*word).to_string())
        })
    };
    match row {
        // A genre is a name, and keeps the casing it has everywhere else it is shown.
        0 => filter.genre.clone(),
        1 => filter.year.map(|y| y.to_string()),
        2 => shown(&SEARCH_SEASONS, &filter.season),
        3 => shown(&SEARCH_FORMATS, &filter.format),
        4 => shown(&SEARCH_STATUSES, &filter.status),
        5 => return shown(&SEARCH_SORTS, &filter.sort).unwrap_or_else(|| "relevance".into()),
        _ => None,
    }
    .unwrap_or_else(|| "any".into())
}

/// The set filter values joined for the search prompt line, empty when nothing is.
pub fn search_filter_summary(filter: &SearchFilters) -> String {
    (0..SEARCH_FILTER_ROWS.len())
        .filter(|row| {
            // "relevance" is the resting state of sort, not a choice worth announcing.
            match row {
                0 => filter.genre.is_some(),
                1 => filter.year.is_some(),
                2 => filter.season.is_some(),
                3 => filter.format.is_some(),
                4 => filter.status.is_some(),
                _ => filter.sort.is_some(),
            }
        })
        .map(|row| search_filter_value(filter, row))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Step a filter row's value. Forward walks the options and comes back around to "any",
/// so both directions reach everything and nothing is a dead end. The year ladder tops out
/// at next year by `clock`.
pub fn cycle_search_filter(
    filter: &mut SearchFilters,
    row: usize,
    forward: bool,
    clock: &Clock,
) {
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
        0 => step(&mut filter.genre, &SEARCH_GENRES, forward),
        1 => {
            let latest = chrono::Datelike::year(&clock.today()).max(0) as u32 + 1;
            filter.year = match (filter.year, forward) {
                (None, true) => Some(latest),
                (None, false) => Some(OLDEST_SEARCH_YEAR),
                (Some(y), true) => (y > OLDEST_SEARCH_YEAR).then(|| y - 1),
                (Some(y), false) => (y < latest).then(|| y + 1),
            };
        }
        2 => step(&mut filter.season, &SEARCH_SEASONS.map(|(wire, _)| wire), forward),
        3 => step(&mut filter.format, &SEARCH_FORMATS.map(|(wire, _)| wire), forward),
        4 => step(&mut filter.status, &SEARCH_STATUSES.map(|(wire, _)| wire), forward),
        _ => step(&mut filter.sort, &SEARCH_SORTS.map(|(wire, _)| wire), forward),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::{config::Config, settings::SettingValue};
    use anistream_model::{
        catalog::{RelatedTitle, ResumePoint},
        progress::{EpisodeProgress, TitleProgress, mark_row},
        settings,
    };

    fn app() -> App {
        App::new(SettingsView::default(), Palette::dark(), Keymap::new())
    }

    /// An app launched on `config`, as both what is in effect and what the file holds.
    fn app_on(config: &Config) -> App {
        App::new(settings::view(config, config), Palette::dark(), Keymap::new())
    }

    /// The engine's part in a change of settings, played by the model's own rules and with no
    /// file: what each command puts in effect, and the settings it answers with. A write here
    /// always lands, so what is in effect and what is saved move together; [`Self::refuse`]
    /// answers one that does not.
    struct Owner {
        effective: Config,
        saved: Config,
    }

    impl Owner {
        fn new(config: Config) -> Self {
            Self { saved: config.clone(), effective: config }
        }

        /// Do what `task` asks of the config, as the engine does, and tell the App the outcome.
        /// Anything else is not the config's, and nothing is said.
        fn answer(&mut self, a: &mut App, task: Option<Command>) {
            let (key, value) = match task {
                Some(Command::StepSetting { key, delta }) => {
                    let Some(value) = settings::step(&self.effective, &self.saved, key, delta)
                    else {
                        return;
                    };
                    (key, value)
                }
                Some(Command::SetSetting { key, value }) => (key, value),
                Some(Command::SetTranslation { translation }) => {
                    self.effective.playback.translation = translation;
                    return self.tell(a);
                }
                _ => return,
            };
            if settings::put(&mut self.effective, key, &value) {
                settings::put(&mut self.saved, key, &value);
            }
            self.tell(a);
        }

        /// Refuse the write `task` asks for, as the engine does: the setting stays what the file
        /// holds, and the App is told which one was put back.
        fn refuse(&mut self, a: &mut App, task: Option<Command>) {
            let key = match task {
                Some(Command::StepSetting { key, .. } | Command::SetSetting { key, .. }) => key,
                other => panic!("not a write: {other:?}"),
            };
            let view = settings::view(&self.effective, &self.saved);
            a.apply(Event::Settings(SettingsView { refused: Some(key), ..view }));
        }

        /// Tell the App the settings as they stand.
        fn tell(&self, a: &mut App) {
            a.apply(Event::Settings(settings::view(&self.effective, &self.saved)));
        }
    }

    /// One episode marked watched or not on the table, in both the visible and the full list,
    /// as a test sets the stage.
    fn set_watched(a: &mut App, number: &str, watched: bool) {
        for list in [&mut a.episodes, &mut a.episodes_all] {
            mark_row(list, number, watched);
        }
    }

    /// The engine's word on a mark, as far as the table goes: each episode the mark asks for
    /// marked or not, with its meter full or empty to match, said back as the engine says a
    /// title's progress. Where the title then stands is the engine's to work out, and its
    /// tests' to check; here it is said as it stood. Returns the command, to be checked.
    fn answer_mark(a: &mut App, task: Option<Command>) -> Option<Command> {
        let Some(Command::SetWatched { id, episodes, watched }) = &task else { return task };
        let mut rows = a.episodes_all.clone();
        for episode in episodes {
            mark_row(&mut rows, episode, *watched);
        }
        let entry = a.detail.as_ref().filter(|d| d.id == *id);
        a.apply(Event::Progress {
            id: *id,
            progress: entry
                .and_then(|e| e.progress)
                .map(|(done, next)| TitleProgress { done, next }),
            resume: entry.and_then(|e| e.resume),
            episodes: rows
                .iter()
                .map(|r| EpisodeProgress {
                    number: r.number.clone(),
                    watched: r.watched,
                    completed: r.completed,
                })
                .collect(),
        });
        task
    }

    /// What the engine says of a title's episodes, each by number with its meter and whether
    /// it is finished, and of the title as the App's page for it stands.
    fn episodes_said(a: &App, id: AnilistId, rows: &[(&str, f64, bool)]) -> Event {
        let entry = a.detail.as_ref().filter(|d| d.id == id);
        Event::Progress {
            id,
            progress: entry
                .and_then(|e| e.progress)
                .map(|(done, next)| TitleProgress { done, next }),
            resume: entry.and_then(|e| e.resume),
            episodes: rows
                .iter()
                .map(|(number, watched, completed)| EpisodeProgress {
                    number: (*number).into(),
                    watched: *watched,
                    completed: *completed,
                })
                .collect(),
        }
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
        a.apply(Event::CalendarWindow {
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
            Some(Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }),
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
        a.apply(Event::CalendarBackfill { page: Some(4), entries: older, more_past: true });

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

        a.apply(Event::CalendarBackfill {
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

        a.apply(Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true });
        assert_eq!(a.content.len(), 150, "a failed backfill must not disturb the list");

        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_pending(),
            Some(Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }),
            "the page that failed is the page still missing"
        );
    }

    #[test]
    fn the_calendar_stops_asking_once_its_history_runs_out() {
        let mut a = calendar_loaded();
        a.apply(Event::CalendarBackfill {
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
    /// window for when it rolls on, as the engine says it of the playback on screen: rolling on
    /// when there is one.
    fn finished(next: Option<&str>) -> Event {
        let ending = Ending::Complete { next: next.map(Into::into) };
        Event::PlaybackEnded { watched: true, ending, rolling_on: next.is_some() }
    }

    /// A playback the engine staged: `episode` of title `id`, under `title`, at the speed
    /// nothing remembered.
    fn staged(id: u32, title: &str, episode: &str, episode_title: Option<&str>) -> Event {
        Event::PlaybackStaged {
            id: AnilistId::new(id),
            title: title.into(),
            episode: episode.into(),
            episode_title: episode_title.map(Into::into),
            speed: 1.0,
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
            Some(Command::Play { id: AnilistId::new(1), episode: "5".into() }),
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
        assert!(matches!(task, Command::LoadSources { .. }), "got {task:?}");

        a.apply(a.sources_here(vec![
            source("[A] Show - 1 (1080p)", true),
            source("[B] Show - 1 (1080p)", false),
        ]));
        assert_eq!(a.nav.overlay(), Some(&Overlay::Sources));

        // Move to the second release and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must play");
        match task {
            Command::PlaySource { provider_id, source_id, .. } => {
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
        let task = a_toggle(&mut a);
        let task = answer_mark(&mut a, task);
        assert!(a.episodes[0].completed, "the table shows the mark once the engine has it");
        assert!(
            matches!(task, Some(Command::SetWatched { watched: true, .. })),
            "got {task:?}"
        );
        assert!(!a.awaiting_mark(), "answered");

        let task = a_toggle(&mut a);
        let task = answer_mark(&mut a, task);
        assert!(!a.episodes[0].completed);
        assert!(
            matches!(task, Some(Command::SetWatched { watched: false, .. })),
            "got {task:?}"
        );
    }

    /// `u` on the table: the mark it asks for, with the App left waiting on the engine's word.
    fn a_toggle(a: &mut App) -> Option<Command> {
        let task = a.handle(Action::ToggleWatched, 20);
        assert!(a.awaiting_mark(), "a mark waits on the engine's word");
        task
    }

    #[test]
    fn a_watched_mark_moves_the_title_and_its_list_row_too() {
        // Where a title stands is the engine's to work out, from the mark; the App shows it
        // wherever the title is on screen: its page and its row in the list under it, and only
        // that title's, though another shares its next-episode number.
        let mut a = app_at_episodes();
        let at = |a: &App, row: usize| {
            (a.detail.as_ref().and_then(|d| d.progress), a.content.entries()[row].progress)
        };
        if let Content::Entries(rows) = &mut a.content {
            rows[1].progress = Some((2, 3));
        }
        a.apply(Event::Progress {
            id: AnilistId::new(1),
            progress: Some(TitleProgress { done: 3, next: 4 }),
            resume: None,
            episodes: Vec::new(),
        });
        assert_eq!(at(&a, 0), (Some((3, 4)), Some((3, 4))), "the page and the list row move");
        assert_eq!(at(&a, 1).1, Some((2, 3)), "another show is not touched");
    }

    #[test]
    fn marking_previous_marks_only_the_unwatched_before_the_cursor() {
        let mut a = app_at_episodes();
        a.episode_selected = 4;
        // Already done — must not be re-marked. Both lists, since the visible one is
        // re-derived from the full one whenever marks change.
        a.episodes[1].completed = true;
        a.episodes_all[1].completed = true;

        let task = a.handle(Action::MarkAllPrevious, 20);
        let task = answer_mark(&mut a, task);
        match task.expect("marking must persist") {
            Command::SetWatched { episodes, watched: true, .. } => {
                assert_eq!(episodes, vec!["1".to_string(), "3".into(), "4".into()]);
            }
            other => panic!("expected SetWatched, got {other:?}"),
        }
        assert!(a.episodes[..4].iter().all(|r| r.completed));
        assert!(!a.episodes[4].completed, "the cursor row itself stays untouched");
    }

    #[test]
    fn a_hint_is_said_in_the_terminal_s_words_where_the_key_used_to_be() {
        // Each line as the work wrote it with the key in it, word for word.
        let keymap = Keymap::new();
        let said = |notice: Notice| worded(notice, &keymap).text;
        let stalled = "mpv has not started playing after 20s — the source may have no data, \
                       or check `mpv` plays a file on its own";
        assert_eq!(
            said(Notice::alert(stalled).with_hint(NoticeHint::StopPlayback)),
            "mpv has not started playing after 20s — press x to stop; the source may have no \
             data, or check `mpv` plays a file on its own"
        );
        assert_eq!(
            said(Notice::info("plugin update ready: alpha").with_hint(NoticeHint::OpenStores)),
            "plugin update ready: alpha — Providers, Enter"
        );
        assert_eq!(
            said(
                Notice::info("kitsu signs in from the Accounts screen")
                    .with_hint(NoticeHint::KitsuCredentials)
            ),
            "kitsu signs in from the Accounts screen — enter on its row asks for credentials"
        );
        assert_eq!(said(Notice::info("no hint — said as sent")), "no hint — said as sent");

        // The key bound now, not the default.
        let mut keymap = Keymap::new();
        let rebound = [("stop_playback".to_owned(), "s".to_owned())].into_iter().collect();
        assert!(keymap.apply_overrides(&rebound).is_empty());
        let notice = Notice::alert("stuck — try again").with_hint(NoticeHint::StopPlayback);
        assert_eq!(worded(notice, &keymap).text, "stuck — press s to stop; try again");
    }

    #[test]
    fn a_hinted_notice_is_shown_and_logged_as_the_terminal_words_it() {
        let mut a = app();
        let notice =
            Notice::info("plugin update ready: alpha").with_hint(NoticeHint::OpenStores);
        a.apply(Event::Toast(notice.clone()));
        a.apply(Event::Toast(notice));
        assert_eq!(a.toasts.len(), 1, "the same notice again is the same line");
        assert_eq!(a.toasts[0].notice.text, "plugin update ready: alpha — Providers, Enter");
        assert_eq!(
            a.logs.last().unwrap().text,
            "plugin update ready: alpha — Providers, Enter"
        );
    }

    #[test]
    fn a_text_setting_is_typed_not_pointed_at_config_toml() {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected =
            SettingKey::ROWS.iter().position(|s| *s == SettingKey::Subtitles).expect("row");

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
            matches!(
                &task,
                Command::SetSetting { key: SettingKey::Subtitles, value: SettingValue::Str(v) }
                    if v == "spa"
            ),
            "got {task:?}"
        );
        Owner::new(Config::default()).answer(&mut a, Some(task));
        assert_eq!(setting_text(&a, SettingKey::Subtitles), "spa");
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
            Command::Play { id: AnilistId::new(5), episode: "8".into() },
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
            Command::DownloadMany { episodes, .. } => {
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
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "queueing 1 episode…"),
            "{:?}",
            a.toasts
        );
        assert_eq!(task.activity().as_deref(), Some("queueing 1 episode"));
    }

    #[test]
    fn mark_all_previous_says_which_nothing_it_found() {
        let mut a = app_at_episodes();
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "nothing comes before the first episode")
        );

        a.episode_selected = 3;
        let task = a.handle(Action::MarkAllPrevious, 20);
        answer_mark(&mut a, task);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "marked 3 episodes watched"),
            "{:?}",
            a.toasts
        );
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "everything before ep 4 is already watched")
        );

        // An empty table has no episode to be before.
        a.episodes.clear();
        a.episode_selected = 0;
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "no episodes listed to mark"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_list_status_is_confirmed_as_the_list_it_is_on() {
        let mut a = app();
        // List status lives on a tracker, so one is signed in.
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(1)));
        a.handle(Action::SetListStatus, 10);
        a.overlay_selected = 2;
        let task = a.handle(Action::Open, 10);
        assert!(matches!(
            task,
            Some(Command::SetStatus { status: LibrarySegment::Completed, .. })
        ));
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "now on your completed list — queued for anilist"),
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
        a.selected =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Subtitles).unwrap();
        a.handle(Action::Open, 10);
        for c in "ger".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Command::SetSetting { key: SettingKey::Subtitles, .. })));
        assert!(a.toasts.is_empty(), "the save says so when it lands: {:?}", a.toasts);
    }

    #[test]
    fn the_format_filter_can_find_short_tv() {
        let mut filter = SearchFilters::default();
        let clock = Clock::system();
        cycle_search_filter(&mut filter, 3, true, &clock);
        assert_eq!(search_filter_value(&filter, 3), "tv");
        cycle_search_filter(&mut filter, 3, true, &clock);
        assert_eq!(filter.format.as_deref(), Some("TV_SHORT"), "AniList's name for it");
        assert_eq!(search_filter_value(&filter, 3), "tv short");
    }

    #[test]
    fn a_genre_filter_keeps_the_genre_s_own_casing() {
        let filter =
            SearchFilters { genre: Some("Slice of Life".into()), ..Default::default() };
        assert_eq!(search_filter_value(&filter, 0), "Slice of Life");
    }

    #[test]
    fn the_source_named_is_one_that_is_not_switched_off() {
        // The default order leads with torrents, which are off by default.
        let mut a = app();
        assert_eq!(a.source_label(), "plugins");
        assert_eq!(
            a.source_states(),
            chips(&[("torrent", SourceState::Off), ("plugins", SourceState::Pending)])
        );
        a.apply(Event::Providers(vec![ProviderRow {
            id: "someplugin".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        }]));
        assert_eq!(a.source_states()[1], chips(&[("plugins", SourceState::Ready)])[0]);
        // What actually served wins once something has played.
        a.apply(Event::ActiveProvider("someplugin".into()));
        assert_eq!(a.source_label(), "someplugin");
    }

    fn chips(states: &[(&str, SourceState)]) -> Vec<SourceChip> {
        states
            .iter()
            .map(|(id, state)| SourceChip { id: (*id).into(), state: *state })
            .collect()
    }

    fn source_row(id: &str, kind: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            kind: kind.into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| (*e).to_owned()).collect()
    }

    #[test]
    fn a_named_plugin_adds_no_chip_of_its_own() {
        // The `"plugins"` chip speaks for every plugin, named in the order or not.
        let mut a = app();
        a.settings.sources.order = strings(&["alpha", "torrent", "plugins"]);
        a.apply(Event::Providers(vec![source_row("alpha", "plugin")]));
        assert_eq!(
            a.source_states(),
            chips(&[("torrent", SourceState::Off), ("plugins", SourceState::Ready)])
        );
    }

    /// An app on Providers with the stage focused, showing a torrent source and two plugins.
    fn at_providers() -> App {
        let mut a = app();
        a.settings.sources.order = strings(&["torrent", "plugins"]);
        a.go_to_section(Section::Providers);
        a.nav.focus_stage();
        a.apply(Event::Providers(vec![
            source_row("torrent", "torrent"),
            source_row("alpha", "plugin"),
            source_row("beta", "plugin"),
        ]));
        a
    }

    fn press(a: &mut App, ch: char) -> Option<Command> {
        a.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), 20)
    }

    /// The order a task saves, for a task that must be the order being saved.
    fn saved_order(task: Option<Command>) -> Vec<String> {
        match task {
            Some(Command::SetSetting {
                key: SettingKey::SourceOrder,
                value: SettingValue::List(order),
            }) => order,
            other => panic!("not an order save: {other:?}"),
        }
    }

    fn row_ids(a: &App) -> Vec<&str> {
        a.providers.iter().map(|row| row.id.as_str()).collect()
    }

    #[test]
    fn shift_k_and_j_move_the_selected_source_and_save_the_order() {
        let mut a = at_providers();
        press(&mut a, 'j');
        assert_eq!(a.selected, 1, "j moves the cursor, as on every table");

        let order = saved_order(press(&mut a, 'K'));
        assert_eq!(order, strings(&["alpha", "torrent", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"], "the rows move at once");
        assert_eq!(a.selected, 0, "the cursor goes with its row");
        assert_eq!(a.settings.sources.order, order, "the next press starts from it");

        // At the top: nothing moves and nothing is saved.
        assert_eq!(press(&mut a, 'K'), None);
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"]);

        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["torrent", "alpha", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["torrent", "alpha", "beta"]);
        assert_eq!(a.selected, 1);
    }

    #[test]
    fn the_order_keys_work_while_something_plays() {
        // The playback table takes neither key, so they still reach the Providers table.
        let mut a = at_providers();
        a.playing = Some(NowPlaying::default());
        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["alpha", "torrent", "beta", "plugins"]));
    }

    #[test]
    fn the_order_keys_move_nothing_off_the_providers_table() {
        let mut a = at_providers();
        a.nav.push(StageView::Stores);
        assert_eq!(press(&mut a, 'J'), None);
        assert_eq!(row_ids(&a), ["torrent", "alpha", "beta"]);

        let mut a = app();
        assert_eq!(a.handle(Action::RaiseSource, 20), None);
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "move source up works on the Providers screen"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn the_order_is_saved_under_a_name_and_put_back_when_the_write_fails() {
        let mut a = at_providers();
        assert_eq!(a.setting_label(&["providers"], "order"), Some("source order"));

        let mut owner = Owner::new(Config::default());
        owner.effective.providers.order = a.settings.sources.order.clone();
        owner.saved.providers.order = a.settings.sources.order.clone();
        let before = a.settings.sources.order.clone();
        let task = press(&mut a, 'J');
        assert_ne!(a.settings.sources.order, before);
        owner.refuse(&mut a, task);
        assert_eq!(a.settings.sources.order, before);
    }

    #[test]
    fn a_new_order_does_not_say_the_sources_are_reloading() {
        // Nothing is rebuilt for a reorder, so the status line must not claim a reload. All a
        // reorder sends the interface is the chain's own rows, in their new order.
        let mut a = at_providers();
        let before = a.status.clone();
        a.apply(Event::Providers(vec![
            source_row("alpha", "plugin"),
            source_row("torrent", "torrent"),
            source_row("beta", "plugin"),
        ]));
        assert_eq!(a.status, before);
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"]);
    }

    #[test]
    fn a_shorter_providers_table_keeps_the_cursor_on_a_row() {
        let mut a = at_providers();
        a.handle(Action::Bottom, 20);
        assert_eq!(a.selected, 2);
        a.apply(Event::Providers(vec![source_row("torrent", "torrent")]));
        assert_eq!(a.selected, 0);
    }

    #[test]
    fn a_nonsense_range_is_refused_with_a_hint() {
        let mut a = app_at_episodes();
        a.handle(Action::DownloadRange, 20);
        for c in "12-4".chars() {
            a.type_char(c);
        }
        assert!(a.handle(Action::Open, 20).is_none(), "a backwards range queues nothing");
        assert!(
            a.toasts.iter().any(|t| t.notice.text.contains("1-12")),
            "the hint names the shape"
        );
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

        // Marking one under the unwatched filter removes it at once, once it is written.
        let task = a.handle(Action::ToggleWatched, 20);
        answer_mark(&mut a, task);
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
            Command::ManualSearch { query, .. } => assert_eq!(query, "frieren"),
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
        a.apply(Event::MatchChoices {
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
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: Vec::new(),
        });
        assert_ne!(a.nav.overlay(), Some(&Overlay::Disambiguate));
    }

    #[test]
    fn picking_a_candidate_pins_it_and_reloads() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![candidate("Frieren S1", 0.62), candidate("Frieren S2", 0.61)],
        });

        // Move to the second candidate and take it.
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20).expect("picking must do something");
        assert_eq!(
            task,
            Command::FixMatch {
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
        a.apply(Event::Toast(Notice::alert("all providers failed")));
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
        assert!(a.toasts.iter().all(|t| t.notice.kind != NoticeKind::Alert), "{:?}", a.toasts);
    }

    #[test]
    fn the_resume_toast_offers_nothing_it_cannot_do() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::Resumed { position: 87.0 });
        let toast = a.toasts.last().expect("a toast");
        assert_eq!(toast.notice.text, "resumed at 1:27");
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
            Some(Command::Player(PlayerCommand::Step(1)))
        );
        a.detail = a.content.entries().first().cloned();
        assert_eq!(
            a.handle(Action::PreviousEpisode, 20),
            Some(Command::Player(PlayerCommand::Step(-1)))
        );
    }

    #[test]
    fn an_episode_that_cannot_be_found_leaves_the_one_playing_on_screen() {
        let mut a = app();
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(2)),
            title: "Title 1".into(),
            episode: "4".into(),
            ..NowPlaying::default()
        });
        a.apply(Event::Playback { position: 300.0, duration: Some(1440.0), paused: false });
        a.apply(staged(2, "Title 1", "9", None));
        assert_eq!(a.playing.as_ref().unwrap().episode, "9", "staged while it is found");

        // Episode 4 keeps the window meanwhile, and its reports are its own.
        a.apply(Event::Playback { position: 301.0, duration: Some(1440.0), paused: false });
        assert_eq!(a.playing.as_ref().unwrap().position, 0.0);
        assert!(a.eyecatch.is_some(), "a report from 4 is not 9 starting");

        a.apply(Event::Toast(Notice::alert("a: timed out")));
        a.apply(Event::PlaybackKept);
        let playing = a.playing.as_ref().unwrap();
        assert_eq!((playing.episode.as_str(), playing.position), ("4", 301.0));

        // Found, the window changes hands and what reports from then on is the successor.
        a.apply(staged(2, "Title 1", "5", None));
        a.apply(Event::EpisodeLeft {
            id: AnilistId::new(2),
            episode: "4".into(),
            position: 302.0,
            duration: Some(1440.0),
            watched: false,
        });
        a.apply(Event::Playback { position: 1.0, duration: Some(1420.0), paused: false });
        let playing = a.playing.as_ref().unwrap();
        assert_eq!((playing.episode.as_str(), playing.position), ("5", 1.0));
    }

    #[test]
    fn a_playback_the_engine_stages_is_put_up_as_it_says() {
        // An episode picked in the player, of Title 1, while Title 0's table is open: staged
        // under the name and the episode title the engine gives it, not the open table's.
        let mut a = app_at_episodes();
        a.apply(Event::PlaybackStaged {
            id: AnilistId::new(2),
            title: "Title 1".into(),
            episode: "5".into(),
            episode_title: Some("Fifth".into()),
            speed: 1.25,
        });
        let playing = a.playing.as_ref().expect("staged");
        assert_eq!(
            (playing.id, playing.title.as_str(), playing.episode.as_str()),
            (Some(AnilistId::new(2)), "Title 1", "5")
        );
        assert_eq!(playing.episode_title.as_deref(), Some("Fifth"));
        assert_eq!((playing.speed, playing.duration), (1.25, None));
        assert_eq!(a.eyecatch.as_ref().map(|e| e.label.as_str()), Some("Title 1  ·  ep 5"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
    }

    #[test]
    fn a_staging_made_here_or_said_before_changes_nothing() {
        // Enter stages episode 1 here as it asks for it; the engine says it staged the same,
        // once and then again. Nothing moves, the wipe least of all.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        let eyecatch = a.eyecatch.as_mut().expect("the wipe is up");
        eyecatch.advance();
        let (before, wipe) = (a.playing.clone(), a.eyecatch.clone());
        for _ in 0..2 {
            a.apply(staged(1, "Title 0", "1", Some("Episode 1")));
            assert_eq!(a.playing, before);
            assert_eq!(a.eyecatch, wipe, "the wipe started over");
        }
        // Once it plays, the same episode staged again is a new playback of it: the restart in
        // the other translation.
        a.apply(Event::Playback { position: 60.0, duration: Some(1435.0), paused: false });
        a.apply(staged(1, "Title 0", "1", Some("Episode 1")));
        assert_eq!(a.playing.as_ref().map(|p| p.duration), Some(None), "staged anew");
        assert_ne!(a.eyecatch, wipe);
    }
    #[test]
    fn another_titles_rows_never_speak_for_the_one_opened_after_them() {
        // Title 0's rows are loaded; Title 1 is opened and its own rows have not arrived. The
        // open title is Title 1, and Title 0's rows must not be taken for its.
        let mut a = app_at_episodes();
        a.detail = a.content.entries().get(1).cloned();
        assert!(a.episode_rows_for(AnilistId::new(2)).is_empty());
        assert_eq!(a.episode_rows_for(AnilistId::new(1)).len(), 12, "still Title 0's");
        a.begin_playback(AnilistId::new(2), "1".into());
        assert_eq!(a.playing.as_ref().unwrap().episode_title, None);
    }

    #[test]
    fn playback_updates_never_resurrect_a_finished_session() {
        // A position tick can arrive after the `Ended` event that preceded it. Recreating
        // `playing` from it would leave a phantom player on screen.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::playback_ended(false, Ending::Closed));
        assert!(a.playing.is_none());

        a.apply(Event::Playback { position: 100.0, duration: Some(1435.0), paused: false });
        assert!(a.playing.is_none(), "a stray tick brought the player back");
    }

    #[test]
    fn quitting_partway_does_not_roll_on_to_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::playback_ended(false, Ending::Closed));
        assert_eq!(a.take_pending(), None);
        assert!(a.playing.is_none() && !a.awaiting_engine(), "Now Playing came down");
    }

    #[test]
    fn an_ending_that_rolls_on_keeps_now_playing_up_for_the_next_episode() {
        // Another show's page is open by the time the episode ends. Until the engine stages the
        // episode it rolled on to, Now Playing stays as it was, and the loop waits for it; then
        // the next one is up under the name and the episode title the engine gives it.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.apply(Event::Playback { position: 1430.0, duration: Some(1435.0), paused: false });
        a.detail = a.content.entries().get(1).cloned();
        a.apply(finished(Some("2")));
        assert_eq!(a.playing.as_ref().map(|p| p.episode.as_str()), Some("1"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        assert!(a.awaiting_engine(), "nothing is drawn or read until the next one is staged");

        a.apply(staged(1, "Title 0", "2", Some("Episode 2")));
        assert!(!a.awaiting_engine());
        let playing = a.playing.as_ref().expect("the next episode is staged");
        assert_eq!((playing.title.as_str(), playing.episode.as_str()), ("Title 0", "2"));
        assert_eq!(playing.episode_title.as_deref(), Some("Episode 2"));
        assert_eq!(a.eyecatch.as_ref().map(|e| e.label.as_str()), Some("Title 0  ·  ep 2"));
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
        // The finished one is gone, not kept in the window: what reports now is the next one.
        a.apply(Event::Playback { position: 1.0, duration: Some(1420.0), paused: false });
        assert_eq!(a.playing.as_ref().map(|p| p.position), Some(1.0));
        assert_eq!(a.take_pending(), None, "the engine started it; nothing is asked for here");
    }

    #[test]
    fn rolling_on_hands_the_keys_back_to_the_screen_as_an_ending_does() {
        // The rail had the keyboard over Now Playing. The finished episode came down, and the
        // next one went up, so Now Playing has it again, as it had when the interface rolled on.
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        a.nav.focus_rail();
        a.apply(finished(Some("2")));
        a.apply(staged(1, "Title 0", "2", Some("Episode 2")));
        assert_eq!(a.nav.focus(), Focus::Stage);
        assert_eq!(a.nav.current(), &StageView::NowPlaying);
    }
    #[test]
    fn x_on_home_dismisses_the_selected_title_from_continue() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.selected = 1;
        let task = a.handle(Action::HideFromContinue, 20);
        assert_eq!(task, Some(Command::HideFromContinue { id: AnilistId::new(2) }));
        // Gone from the screen before the store is even asked.
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
    fn an_ending_asks_for_the_next_episode_without_taking_a_busy_slot() {
        // The next episode's streams found ahead, and an episode picked in the player, are the
        // engine's to start: neither takes the slot, so nothing waiting in it is displaced.
        let mut a = app();
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert_eq!(a.take_pending(), None);
        a.playing = Some(NowPlaying {
            id: Some(AnilistId::new(3)),
            title: "Hoshikuzu no Tabibito".into(),
            episode: "5".into(),
            ..NowPlaying::default()
        });
        a.apply(Event::PlayerPlayEpisode { episode: "7".into(), title: None });
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        assert_eq!(a.take_pending(), None);
        assert_eq!(a.playing.as_ref().map(|p| p.episode.as_str()), Some("5"), "until staged");
    }
    #[test]
    fn a_choice_made_in_the_player_takes_no_slot_and_waits_behind_none() {
        // The engine keeps and saves what the player changes, as it passes. Queued here, each
        // save waited in the one slot, and one that found it taken was skipped.
        let mut a = app();
        a.apply(Event::PrefetchNext { id: AnilistId::new(3), episode: "6".into() });
        for choice in [
            Event::PlayerNightMode(true),
            Event::PlayerUpscaling("sharp".into()),
            Event::PlayerToneMapping("bt.2390".into()),
            Event::PlaybackVolume(80.0),
            Event::PlaybackSpeed(1.5),
        ] {
            a.apply(choice);
        }
        assert_eq!(
            a.take_pending(),
            None,
            "the ending's work is the engine's, and no save waits"
        );
        // Nor kept here: what is in effect is the engine's to say, in `Settings`.
        assert_eq!(a.settings, SettingsView::default());
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
        assert!(a.toasts.iter().any(|t| t.notice.text == "episode finished"));
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
                Some(Command::Player(expected)),
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

        a.apply(Event::SkipAvailable { kind: SkipKind::Opening, to: 93.2 });
        assert_eq!(
            a.handle(Action::SkipOpening, 20),
            Some(Command::Player(PlayerCommand::SeekTo(93.2)))
        );

        a.apply(Event::SkipCleared);
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
    fn a_remembered_speed_carries_into_the_next_episode() {
        let mut a = app_at_episodes();
        a.handle(Action::Open, 20);
        // The speed, then what the engine makes of it: remembered, for the next episode.
        a.apply(Event::PlaybackSpeed(1.25));
        let mut owner = Owner::new(Config::default());
        owner.effective.playback.persisted_speed = Some(1.25);
        owner.tell(&mut a);
        assert_eq!(a.settings.speed, Some(1.25));

        // Staged by the engine at it, when an ending rolls on...
        a.apply(finished(Some("2")));
        a.apply(Event::PlaybackStaged {
            id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: "2".into(),
            episode_title: Some("Episode 2".into()),
            speed: 1.25,
        });
        assert_eq!(a.playing.as_ref().map(|p| p.speed), Some(1.25));
        // ...and here, when Enter asks for one.
        a.apply(Event::playback_ended(false, Ending::Stopped));
        a.episode_selected = 2;
        a.handle(Action::Open, 20);
        assert_eq!(
            a.playing.as_ref().map(|p| (p.episode.as_str(), p.speed)),
            Some(("3", 1.25))
        );
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
        assert_eq!(sync_badge(&connected(0)), "anilist ⇅");
        assert_eq!(sync_badge(&connected(3)), "anilist ⇅ 3");
        assert!(!connected(3).is_alerting());
    }

    #[test]
    fn a_rejected_token_reads_as_something_to_act_on() {
        let state = SyncState { needs_reauth: true, ..connected(2) };
        assert_eq!(sync_badge(&state), "anilist ✕ sign in");
        assert!(state.is_alerting(), "re-auth must use the alert role");
    }

    #[test]
    fn a_tracker_with_no_account_is_quiet_rather_than_alarming() {
        // Watching with no account is a supported way to use this, not a fault.
        let state = SyncState { tracker: "anilist".into(), ..SyncState::default() };
        assert_eq!(sync_badge(&state), "anilist ○");
        assert!(!state.is_alerting());
    }

    #[test]
    fn re_authorisation_is_announced_once_not_every_tick() {
        // The drain runs on an interval; repeating the alert each time would bury everything
        // else in toasts.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        // Counted in the log, which counts every announcement: the toast stack folds a repeat
        // into the one already showing, so it cannot tell whether one was made.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        assert_eq!(announced(&a), 1);
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        assert_eq!(announced(&a), 1, "re-announced an unchanged auth failure");
    }

    #[test]
    fn a_sync_update_replaces_rather_than_appends() {
        // Otherwise the header would grow a new chip on every drain tick.
        let mut a = app();
        for depth in [1, 2, 3] {
            a.apply(Event::Sync(Box::new(connected(depth))));
        }
        assert_eq!(a.sync.len(), 1);
        assert_eq!(a.sync[0].outbox, 3);
    }

    #[test]
    fn queueing_progress_moves_the_badge_immediately() {
        // The row is already in SQLite by the time this arrives, so counting it optimistically
        // is honest — the next drain can only correct it downward.
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(Event::ProgressQueued);
        assert_eq!(a.sync[0].outbox, 1);
        assert_eq!(sync_badge(&a.sync[0]), "anilist ⇅ 1");
    }

    #[test]
    fn conflicts_are_announced_when_they_first_appear() {
        // Surfaced rather than resolved — the whole reason the merge refuses to guess.
        let mut a = app();
        a.apply(Event::Conflicts(vec![conflict("Frieren")]));
        assert_eq!(a.conflicts.len(), 1);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("disagreement")));

        // A second pull returning the same set must not re-announce.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        let before = announced(&a);
        a.apply(Event::Conflicts(vec![conflict("Frieren")]));
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

        a.apply(Event::Conflicts(vec![conflict("Frieren")]));
        a.handle(Action::ShowConflicts, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Conflicts));
    }

    #[test]
    fn resolving_a_conflict_removes_it_and_closes_when_empty() {
        let mut a = app();
        a.apply(Event::Conflicts(vec![conflict("Frieren"), conflict("Dandadan")]));
        a.handle(Action::ShowConflicts, 20);

        let task = a.handle(Action::Open, 20);
        assert!(matches!(task, Some(Command::ResolveConflict { keep_local: true, .. })));
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
        a.apply(Event::Conflicts(vec![conflict("a"), conflict("b"), conflict("c")]));
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
        assert_eq!(a.handle(Action::Open, 20), Some(Command::LoadStores));
        a.apply(Event::Stores(stores));
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
        assert_eq!(a.handle(Action::Open, 20), Some(Command::LoadStores));
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
            Some(Command::PreviewStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(!a.nav.has_overlay());

        a.apply(Event::StorePreview(Ok(StorePreview {
            link: Secret("https://plugins.example/join/abc#key".into()),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            fingerprint: "9f3c".into(),
            already_joined: false,
        })));
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::JoinStore {
                link: Secret("https://plugins.example/join/abc#key".into())
            })
        );
        assert!(a.join_link.is_empty(), "the spent link is not kept");

        // Joined: the new store opens, because the user is still on the list it was joined from.
        a.apply(Event::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Event::StoreJoined { store: "abc".into() });
        assert_eq!(a.nav.current(), &StageView::Store("abc".into()));
    }

    #[test]
    fn a_bad_link_says_why_and_reopens_the_prompt_with_the_link_kept() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc");
        a.handle(Action::Open, 20);
        a.apply(Event::StorePreview(Err("the link is missing the store's key".into())));
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinStore));
        assert_eq!(a.join_link, "https://plugins.example/join/abc");
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("missing the store's key")));
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
        a.apply(Event::Stores(vec![store_row("abc", false, Vec::new())]));
        a.apply(Event::StoreJoined { store: "abc".into() });
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
            Some(Command::InstallPlugin { store: "abc".into(), plugin: "example".into() })
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
            Some(Command::RemovePlugin { plugin: "example".into() })
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
    fn a_new_invite_link_starts_uncopied() {
        // Copied is said of the link on screen; the next one has not been anywhere yet.
        let link = |n: &str| InviteLinkView {
            store: "abc".into(),
            link: Secret(format!("https://plugins.example/join/{n}#k")),
            for_whom: None,
            expires_at: 0,
        };
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.apply(Event::InviteLink(link("one")));
        a.invite_copied = true;
        a.apply(Event::InviteLink(link("two")));
        assert!(!a.invite_copied);
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
            Some(Command::CreateInvite { store: "abc".into(), name: Some("Alex".into()) })
        );

        a.apply(Event::InviteLink(InviteLinkView {
            store: "abc".into(),
            link: Secret("https://plugins.example/join/x#y".into()),
            for_whom: Some("Alex".into()),
            expires_at: 0,
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
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::OpenAdmin { store: "abc".into() })
        );
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
            Some(Command::CreateInvite { store: "abc".into(), name: None })
        );
    }

    #[test]
    fn removing_a_member_asks_first_and_ignores_stray_keys() {
        let mut a = at_stores(vec![store_row("abc", true, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LoadMembers { store: "abc".into() })
        );
        assert_eq!(a.nav.current(), &StageView::StoreMembers("abc".into()));

        let member = |id: &str, you| StoreMemberRow {
            id: id.into(),
            name: id.into(),
            owner: you,
            devices: 1,
            last_seen: None,
            you,
        };
        a.apply(Event::StoreMembers {
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
            Some(Command::RemoveMember { store: "abc".into(), member: "alex".into() })
        );

        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::CancelInvite { store: "abc".into(), invite: "inv".into() })
        );
    }

    #[test]
    fn leaving_asks_first_and_the_page_goes_with_the_store() {
        let mut a = at_stores(vec![store_row("abc", false, Vec::new())]);
        a.handle(Action::Open, 20);
        a.handle(Action::Down, 20);
        assert_eq!(a.handle(Action::Open, 20), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::StoreConfirm));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LeaveStore { store: "abc".into() })
        );

        a.apply(Event::Stores(Vec::new()));
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
            Some(Command::LoadLocalPlugins { store: "abc".into() })
        );
        let local = LocalPluginRow {
            id: "example".into(),
            name: "Example".into(),
            version: "1.0.0".into(),
            hosts: Vec::new(),
            published: None,
        };
        // An answer for another store is not offered here.
        a.apply(Event::LocalPlugins { store: "other".into(), plugins: vec![local.clone()] });
        assert!(!a.nav.has_overlay());

        a.apply(Event::LocalPlugins { store: "abc".into(), plugins: vec![local] });
        assert_eq!(a.nav.overlay(), Some(&Overlay::PublishPick));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::PublishPlugin { store: "abc".into(), plugin: "example".into() })
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
        a.apply(Event::LocalPlugins { store: "abc".into(), plugins: Vec::new() });
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("no plugins")));
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
        assert_eq!(a.handle(Action::Refresh, 20), Some(Command::LoadStores));
        a.handle(Action::Open, 20);
        assert_eq!(a.handle(Action::Refresh, 20), Some(Command::LoadStores));
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(
            a.handle(Action::Refresh, 20),
            Some(Command::LoadMembers { store: "abc".into() })
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
        a.apply(Event::Stores(vec![store_row(
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
        a.apply(Event::StorePreview(Ok(StorePreview {
            link: Secret("x".into()),
            name: "Other".into(),
            url: "https://other.example".into(),
            fingerprint: "0000".into(),
            already_joined: false,
        })));
        assert_eq!(a.store_confirm, leave);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::LeaveStore { store: "abc".into() })
        );
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
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: false,
            ..SyncState::default()
        })));
        a.handle(Action::ShowAccounts, 20);
        // A section, so the rail holds focus until you step into it — the same as everywhere else.
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Connect { tracker: "anilist".into() })
        );

        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Disconnect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn a_tracker_needing_reauth_offers_sign_in_not_sign_out() {
        // It is technically connected but useless, so offering "sign out" would be a dead end.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        a.handle(Action::ShowAccounts, 20);
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Connect { tracker: "anilist".into() })
        );
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
        assert_eq!(task, Some(Command::LoadLibrary(LibrarySegment::Planning)));
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
    fn setting_a_list_status_preselects_the_current_one() {
        // One keystroke to confirm in the common case, rather than a hunt.
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
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
            Some(Command::SetStatus {
                id: AnilistId::new(1),
                status: LibrarySegment::Completed
            })
        );
        assert!(!a.nav.has_overlay());
    }

    #[test]
    fn a_status_given_this_session_outlives_a_reload_until_the_tracker_answers() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.go_to_section(Section::Home);
        a.apply(a.content_here(entries(3)));
        let planning = 1;
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Down, 20);
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::SetStatus {
                id: AnilistId::new(1),
                status: LibrarySegment::Planning
            })
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
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::RateTitle, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Rate));

        // Rows run 10 down to 1, so two steps down from the top is an 8.
        a.handle(Action::Down, 20);
        a.handle(Action::Down, 20);
        let task = a.handle(Action::Open, 20);
        assert_eq!(task, Some(Command::SetScore { id: AnilistId::new(1), score: 8.0 }));
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
        assert_eq!(task, Some(Command::LoadDetail(AnilistId::new(99))));
        assert!(matches!(a.nav.current(), StageView::Title(id) if id.get() == 99));
    }

    #[test]
    fn the_trailer_key_plays_when_known_and_says_so_when_not() {
        let mut a = app();
        a.apply(a.content_here(entries(1)));

        // A list row never opened: the trailer is on the record its page fetches.
        a.handle(Action::PlayTrailer, 20);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("from the title's page")));

        let mut detail = Entry::new(AnilistId::new(1), "Frieren");
        detail.trailer_url = Some("https://www.youtube.com/watch?v=x".into());
        a.detail = Some(detail);
        let task = a.handle(Action::PlayTrailer, 20);
        assert_eq!(
            task,
            Some(Command::PlayTrailer {
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
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("from the title's page")));
    }

    #[test]
    fn enter_on_the_kitsu_row_asks_for_credentials_in_place() {
        // No browser to open and no command line to visit: the prompt is the flow.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
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
            Some(Command::ConnectKitsu {
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
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
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
    fn the_meta_chip_appears_only_while_a_fallback_is_serving() {
        let mut a = app();
        assert!(!a.meta_degraded(), "no status yet is not degraded");

        let row = |source: &str, active: bool| MetaSourceRow {
            source: source.into(),
            state: String::new(),
            healthy: true,
            active,
            detail: None,
            retry_at: None,
        };
        a.apply(Event::MetaStatus(vec![row("anilist", true), row("tenrai", false)]));
        assert!(!a.meta_degraded(), "anilist serving is the resting state");

        a.apply(Event::MetaStatus(vec![row("anilist", false), row("tenrai", true)]));
        assert!(a.meta_degraded());
    }

    #[test]
    fn status_and_rating_with_no_tracker_signed_in_say_so_instead_of_pretending() {
        // "marked watching" and "rated 8/10" used to be said for choices that went nowhere.
        // A tracker that is configured but signed out does not count.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));

        for action in [Action::SetListStatus, Action::RateTitle] {
            a.toasts.clear();
            assert_eq!(a.handle(action, 20), None);
            assert!(!a.nav.has_overlay(), "{action:?} offered a picker that goes nowhere");
            let said = &a.toasts.last().expect("nothing said").notice.text;
            assert!(said.contains("none is signed in") && said.contains(":accounts"), "{said}");
        }
    }

    #[test]
    fn a_status_says_which_trackers_it_was_queued_for() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(Event::Sync(Box::new(SyncState { connected: true, ..SyncState::new("mal") })));
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.apply(a.content_here(entries(3)));
        a.handle(Action::SetListStatus, 20);
        a.handle(Action::Open, 20);
        let said = &a.toasts.last().unwrap().notice.text;
        assert!(said.ends_with("queued for anilist, mal"), "{said}");
    }

    #[test]
    fn rating_with_nothing_selected_says_so() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::RateTitle, 20), None);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("nothing selected")));
    }

    #[test]
    fn a_resync_with_no_tracker_says_so_rather_than_doing_nothing() {
        let mut a = app();
        assert_eq!(a.handle(Action::ForceResync, 20), None);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("no tracker")));

        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::ForceResync, 20), Some(Command::SyncNow));
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

    fn setting_row(id: SettingKey) -> usize {
        SettingKey::ROWS.iter().position(|s| *s == id).expect("a settings row")
    }

    #[test]
    fn a_title_page_leaves_the_list_under_it_where_it_was() {
        // Every movement key on a title page used to move the list hidden behind it, so Back
        // landed rows away from the title just looked at.
        let mut a = home_list(8);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadDetail(AnilistId::new(3))));
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
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadEpisodes(AnilistId::new(1))));
        assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
    }

    #[test]
    fn refresh_on_a_title_or_its_episodes_asks_for_that_title() {
        // It used to reload the section's list instead, under a page that never changed.
        let mut a = home_list(3);
        a.handle(Action::Open, 5);
        assert_eq!(a.handle(Action::Refresh, 5), Some(Command::LoadDetail(AnilistId::new(1))));
        a.handle(Action::ShowEpisodes, 5);
        assert_eq!(
            a.handle(Action::Refresh, 5),
            Some(Command::LoadEpisodes(AnilistId::new(1)))
        );
        assert_eq!(a.content.len(), 3, "the list under them is left alone");
    }

    #[test]
    fn f_on_a_title_opened_from_search_means_what_it_means_on_any_title() {
        let mut a = app();
        a.handle(Action::JumpSection(4), 5);
        type_text(&mut a, "query");
        assert!(matches!(a.handle(Action::Open, 5), Some(Command::Search { .. })));
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
        a.apply(Event::Sync(Box::new(connected(0))));
        a.handle(Action::Open, 5);
        // The record the title page fetched, with everything that rides that fetch.
        let mut opened = Entry::new(AnilistId::new(1), "Title 0");
        opened.trailer_url = Some("https://video.example/trailer".into());
        opened.related = vec![related(90, "Its sequel")];
        opened.recommended = vec![related(91, "Something like it")];
        a.apply(Event::Detail(Box::new(opened)));
        a.handle(Action::Back, 5);
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        let row = AnilistId::new(3);
        let first = || "1".to_string();

        assert_eq!(
            a.handle(Action::Download, 5),
            Some(Command::DownloadEpisode { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::LoadSources { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::PlayParty, 5),
            Some(Command::PlayParty { id: row, episode: first() })
        );
        assert_eq!(
            a.handle(Action::OpenInBrowser, 5),
            Some(Command::OpenExternal { url: "https://anilist.co/anime/3".into() })
        );

        a.handle(Action::SetListStatus, 5);
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SetStatus { id: row, status: LibrarySegment::Watching })
        );
        a.handle(Action::RateTitle, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::SetScore { id: row, score: 10.0 }));

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

        assert_eq!(a.handle(Action::ShowEpisodes, 5), Some(Command::LoadEpisodes(row)));
        a.handle(Action::Back, 5);
        assert_eq!(
            a.handle(Action::PlayNext, 5),
            Some(Command::Play { id: row, episode: first() })
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
            Some(Command::OpenExternal { url: "https://anilist.co/anime/3".into() })
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
        a.apply(Event::Detail(Box::new(full.clone())));

        a.handle(Action::WatchOrder, 5);
        assert_eq!(a.handle(Action::Open, 5), Some(Command::LoadDetail(AnilistId::new(90))));
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
        a.apply(Event::Episodes {
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
            set_watched(&mut a, n, true);
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
        assert_eq!(a.selected, SettingKey::ROWS.len() - 1);
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
        a.apply(Event::Sync(Box::new(SyncState::new("anilist"))));
        a.apply(Event::Sync(Box::new(SyncState::new("mal"))));
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
            Some(Command::DownloadEpisode { id: AnilistId::new(40), episode: "1".into() })
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
        a.apply(Event::Sync(Box::new(connected(0))));
        a.library_segment = LibrarySegment::Dropped;
        a.handle(Action::SetListStatus, 5);
        assert_eq!(a.overlay_selected, 0, "nothing known about it, so the top");
        a.overlay_selected = 3;
        assert_eq!(
            a.handle(Action::Open, 5),
            Some(Command::SetStatus { id: AnilistId::new(1), status: LibrarySegment::Paused })
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
            state: DownloadState::Active,
            fraction: 0.5,
            downloaded: 1,
            total: 2,
            error: None,
            path: None,
        };
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Event::Downloads(vec![download(7), download(8)]));
        a.handle(Action::Down, 5);
        assert_eq!(a.handle(Action::PlayNext, 5), Some(Command::DownloadPause { id: 8 }));
        assert_eq!(
            a.handle(Action::HideFromContinue, 5),
            Some(Command::DownloadCancel { id: 8 })
        );

        a.playing = Some(NowPlaying::default());
        assert_eq!(a.handle(Action::PlayPause, 5), Some(Command::DownloadPause { id: 8 }));
        assert_eq!(a.handle(Action::StopPlayback, 5), Some(Command::DownloadCancel { id: 8 }));
    }

    #[test]
    fn x_on_a_finished_download_keeps_it() {
        // Cancelling a finished row forgot it and left its file behind, unreachable from here.
        let mut a = app();
        a.handle(Action::JumpSection(6), 5);
        a.apply(Event::Downloads(vec![DownloadRow {
            id: 3,
            anilist_id: AnilistId::new(1),
            title: "Title 0".into(),
            episode: "1".into(),
            state: DownloadState::Done,
            fraction: 1.0,
            downloaded: 2,
            total: 2,
            error: None,
            path: Some("/tmp/title-0-1.mkv".into()),
        }]));
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "already downloaded — X deletes it and its file"),
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
        let last = |a: &App| a.toasts.last().map(|t| t.notice.text.clone()).unwrap_or_default();
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
            Some(Command::LoadLibrary(LibrarySegment::Watching))
        );
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);

        a.handle(Action::JumpSection(9), 5);
        a.selected = setting_row(SettingKey::Motion);
        let mut owner = Owner::new(Config::default());
        owner.effective.theme.motion = true;
        owner.tell(&mut a);
        assert_eq!(a.handle(Action::Right, 5), None, "a switch that is on stays on");
        assert_eq!(setting_text(&a, SettingKey::Motion), "on");
        let task = a.handle(Action::Left, 5);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Motion, delta: -1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.handle(Action::Left, 5), None);
        assert_eq!(a.nav.focus(), Focus::Rail);
        assert_eq!(
            setting_text(&a, SettingKey::Motion),
            "off",
            "reaching the rail changed nothing"
        );

        // A ladder on its bottom rung and a typed value have nothing further left either.
        for (row, prepare) in [
            (
                SettingKey::Quality,
                (|owner: &mut Owner| owner.effective.playback.quality = 480) as fn(&mut Owner),
            ),
            (SettingKey::Subtitles, |_| {}),
        ] {
            a.nav.focus_stage();
            a.selected = setting_row(row);
            prepare(&mut owner);
            owner.tell(&mut a);
            assert_eq!(a.handle(Action::Left, 5), None, "{row:?}");
            assert_eq!(a.nav.focus(), Focus::Rail, "{row:?}");
        }
        assert_eq!(a.settings.quality, 480);
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

        a.episode_selected = 1;
        a.begin_playback(id, "1".into());
        a.nav.pop();

        // What a watch moves is the engine's to work out — see its record's test of the same
        // name — and it says so ahead of what moved it. The table takes it in place, row by
        // row, with the cursor where it was. Halfway through, then detached:
        a.apply(episodes_said(&a, id, &[("1", 0.5, false), ("2", 0.0, false)]));
        a.apply(Event::Playback { position: 720.0, duration: Some(1440.0), paused: false });
        assert!((a.episodes[0].watched - 0.5).abs() < 1e-6, "got {}", a.episodes[0].watched);
        assert!(!a.episodes[0].completed, "halfway is not finished");

        a.apply(episodes_said(&a, id, &[("1", 1.0, true), ("2", 0.0, false)]));
        a.apply(finished(None));
        assert!(a.episodes[0].completed, "a finished episode must show as finished");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6);
        assert_eq!(a.episodes[1].watched, 0.0, "only the episode watched should change");
        assert_eq!(a.episode_selected, 1, "and the cursor stays put");

        // Another title's word never lands on this table.
        let other = AnilistId::new(3);
        a.apply(episodes_said(&a, other, &[("2", 1.0, true)]));
        assert!(!a.episodes[1].completed);
    }

    /// Reported from real use: an episode finished, the toast said so, and its row stayed empty
    /// and unmarked until the show was left and re-entered. Reaching the table with `e` rather
    /// than by opening the title left the app with no idea which title was on screen, so the
    /// watch had nowhere to land — and auto-next, asking the same question, stayed put.
    #[test]
    fn reaching_episodes_by_key_still_lands_a_finished_watch_on_the_table() {
        // Auto-next is on by default, and the playback's to decide: it names the next episode
        // in the ending it reports.
        let mut a = app();
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
        // The engine says where the title stands ahead of the ending; reached by `e`, the
        // table is the title's, so that lands on it.
        a.apply(episodes_said(
            &a,
            id,
            &[("1", 1.0, true), ("2", 0.0, false), ("3", 0.0, false)],
        ));
        a.apply(finished(Some("2")));

        assert!(a.episodes[0].completed, "the finished episode must be marked in the table");
        assert!((a.episodes[0].watched - 1.0).abs() < 1e-6, "and its meter filled");
        // Auto-next is the engine's, which stages the next episode of the title that played.
        a.apply(staged(1, "Title 0", "2", None));
        assert_eq!(
            a.playing.as_ref().map(|p| (p.id, p.episode.as_str())),
            Some((Some(id), "2")),
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
            Some(Command::LoadEpisodes(AnilistId::new(1))),
            "the selected row"
        );
        a.handle(Action::Back, 10);
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Command::LoadEpisodes(AnilistId::new(2))),
            "the next row, not the one already seen"
        );

        // From a title screen the subject is that title, whatever the list beneath is on.
        a.handle(Action::Back, 10);
        a.detail = Some(Entry::new(AnilistId::new(99), "a related title"));
        a.nav.push(StageView::Title(AnilistId::new(99)));
        assert_eq!(
            a.handle(Action::ShowEpisodes, 10),
            Some(Command::LoadEpisodes(AnilistId::new(99))),
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
        assert!(matches!(task, Some(Command::LoadEpisodes(_))));
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
            Some(Command::Search { query: "frieren".into(), filter: SearchFilters::default() })
        );
        a.apply(a.content_here(entries(3)));

        // Second Enter opens the highlighted result rather than asking again.
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Command::LoadDetail(_))), "got {task:?}");
        assert!(matches!(a.nav.current(), StageView::Title(_)));

        // Editing the query makes it submittable again.
        a.handle(Action::Back, 10);
        a.type_char('x');
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Search {
                query: "frierenx".into(),
                filter: SearchFilters::default()
            })
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
            Some(Command::Search { query, filter }) => {
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
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("filters live")));
    }

    #[test]
    fn accounts_is_a_screen_with_a_cursor_and_enter_acts_on_the_row() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        a.apply(Event::Sync(Box::new(SyncState::new("mal"))));

        a.handle(Action::ShowAccounts, 10);
        assert_eq!(a.nav.section(), Section::Accounts, "accounts is a section, not a modal");
        assert!(!a.nav.has_overlay());

        a.nav.focus_stage();
        // A connected tracker signs out; an unconnected one signs in.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Disconnect { tracker: "anilist".into() })
        );
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Connect { tracker: "mal".into() })
        );
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
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        assert!(a.sync[0].connected);

        a.apply(Event::Sync(Box::new(SyncState::new("anilist"))));
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
            a.push_toast(Notice::info(text));
        }
        let up: Vec<&str> = a.toasts.iter().map(|t| t.notice.text.as_str()).collect();
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
            a.push_toast(Notice::alert(format!("failure {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "the stack is still capped");
        assert_eq!(a.logs.len(), 5, "but nothing was lost");
        assert!(a.logs[0].text.contains("failure 0"));

        a.handle(Action::ShowLogs, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Logs));
    }

    #[test]
    fn the_settings_screen_steps_an_off_ladder_quality_up_not_down() {
        let mut config = Config::default();
        config.playback.quality = 1440;
        config.playback.commit_threshold = 0.97;
        let mut a = app_on(&config);
        let mut owner = Owner::new(config);
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        a.selected = SettingKey::ROWS.iter().position(|id| *id == SettingKey::Quality).unwrap();
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Quality, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 2160);
        assert_eq!(owner.saved.playback.quality, 2160, "the step written is up the ladder");

        a.selected =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::CommitThreshold).unwrap();
        assert_eq!(a.handle(Action::Right, 10), None, "→ never lowers it");
        assert_eq!(setting_text(&a, SettingKey::CommitThreshold), "97%");
        let task = a.handle(Action::Left, 10);
        owner.answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::CommitThreshold), "95%");
        assert_eq!(owner.saved.playback.commit_threshold, 0.95);
    }

    fn setting_text(a: &App, id: SettingKey) -> String {
        let at = SettingKey::ROWS.iter().position(|row| *row == id).unwrap();
        a.setting_rows()[at].value.clone()
    }

    fn settings_at(id: SettingKey) -> App {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected = SettingKey::ROWS.iter().position(|row| *row == id).unwrap();
        a
    }

    #[test]
    fn a_refused_settings_write_puts_the_setting_back() {
        // The value changed before the write and nothing undid it, so a refused write — torrents
        // on with no VPN configured — went on showing "on" and the status line the new value.
        let mut a = settings_at(SettingKey::Torrents);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Torrents, delta: 1 }));
        a.status = "saved quality".into();

        Owner::new(Config::default()).refuse(&mut a, task);
        assert!(!a.settings.sources.torrent, "back to what the file holds");
        assert_eq!(setting_text(&a, SettingKey::Torrents), "off");
        assert!(a.status.is_empty(), "the status line shows what is in effect again");
    }

    #[test]
    fn a_rollback_leaves_other_settings_alone() {
        // A second setting changed while the first write was refused has its own write, which
        // landed; undoing everything would undo that too.
        let mut a = settings_at(SettingKey::Quality);
        let mut owner = Owner::new(Config::default());
        let task = a.handle(Action::Right, 10);
        owner.answer(&mut a, task);
        a.selected =
            SettingKey::ROWS.iter().position(|row| *row == SettingKey::Torrents).unwrap();
        let task = a.handle(Action::Right, 10);

        owner.refuse(&mut a, task);
        assert_eq!(a.settings.quality, 2160);
        assert!(!a.settings.sources.torrent);
    }

    #[test]
    fn sub_or_dub_for_the_session_shows_beside_the_saved_choice() {
        // `t` is for this session, and Settings showed it as if saved while the file and the
        // next launch said otherwise.
        let mut a = app();
        let task = a.handle(Action::ToggleTranslation, 10);
        Owner::new(Config::default()).answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::Translation), "sub · dub this session");
        assert!(a.toasts.last().unwrap().notice.text.contains("for this session"));

        // The arrows change the saved value, the one the row leads with.
        let mut a = settings_at(SettingKey::Translation);
        let mut owner = Owner::new(Config::default());
        let task = a.handle(Action::ToggleTranslation, 10);
        owner.answer(&mut a, task);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Translation, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(
            owner.saved.playback.translation,
            anistream_core::media::Translation::Dub,
            "dub is what is written"
        );
        assert_eq!(setting_text(&a, SettingKey::Translation), "dub");
    }

    #[test]
    fn a_theme_from_the_command_line_shows_as_this_session_only() {
        use anistream_core::config::ThemeMode;
        let mut config = Config::default();
        config.theme.mode = ThemeMode::Immersive;
        let saved = Config::default();
        let a = App::new(settings::view(&config, &saved), Palette::dark(), Keymap::new());
        assert_eq!(setting_text(&a, SettingKey::Theme), "adaptive · immersive this session");
    }

    #[test]
    fn settings_rows_cycle_and_ask_to_be_persisted() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        // Quality steps up the ladder and clamps, rather than wrapping round to 480p.
        let quality_row =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Quality).unwrap();
        a.selected = quality_row;
        assert_eq!(a.settings.quality, 1080);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Quality, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 2160, "the change applies once the engine has it");
        assert_eq!(owner.saved.playback.quality, 2160, "and is written");
        assert_eq!(a.handle(Action::Right, 10), None, "clamped at the top of the ladder");
        let task = a.handle(Action::Left, 10);
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 1080);

        // And the rows that cannot be edited here say so instead of silently ignoring the key.
        let vpn_row =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::VpnMode).unwrap();
        a.selected = vpn_row;
        let before = setting_text(&a, SettingKey::VpnMode);
        assert_eq!(a.handle(Action::Right, 10), None);
        assert_eq!(setting_text(&a, SettingKey::VpnMode), before);
        assert!(
            a.toasts.iter().any(|t| t.notice.text.contains("config.toml")),
            "a read-only row must explain itself"
        );
    }

    #[test]
    fn seeding_is_set_from_the_settings_screen() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        let at = |id| SettingKey::ROWS.iter().position(|row| *row == id).unwrap();

        // Off by default; one step right turns it on, at a modest ratio.
        a.selected = at(SettingKey::SeedRatio);
        assert_eq!(setting_text(&a, SettingKey::SeedRatio), "off");
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::SeedRatio, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(owner.saved.providers.torrent.seeding.ratio, 0.5);
        assert!(owner.effective.providers.torrent.seeding.enabled());

        a.selected = at(SettingKey::SeedHours);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::SeedHours, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(owner.saved.providers.torrent.seeding.hours, 72);

        a.selected = at(SettingKey::UploadLimit);
        let rows = a.setting_rows();
        assert_eq!(rows[a.selected].value, "unlimited");
        let task = a.handle(Action::Right, 10);
        owner.answer(&mut a, task);
        // 256 KiB/s in the file, said in the decimal units every other size uses.
        assert_eq!(a.setting_rows()[a.selected].value, "262 kB/s");
        for _ in 0..3 {
            let task = a.handle(Action::Right, 10);
            owner.answer(&mut a, task);
        }
        assert_eq!(a.setting_rows()[a.selected].value, "2.1 MB/s");
    }

    #[test]
    fn discord_presence_can_be_toggled_from_settings() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.selected =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Presence).unwrap();

        assert_eq!(
            setting_text(&a, SettingKey::Presence),
            "off",
            "off by default: it publishes to a third party"
        );
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Presence, delta: 1 }));
        owner.answer(&mut a, task);
        assert!(owner.saved.presence.enabled, "written on");
        assert_eq!(setting_text(&a, SettingKey::Presence), "on");
        let task = a.handle(Action::Open, 10);
        owner.answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::Presence), "off", "and back off again");
    }

    #[test]
    fn enabling_presence_without_a_client_id_says_so() {
        // A toggle that flips and then does nothing is worse than one that explains itself.
        let mut a = app();
        a.go_to_section(Section::Settings);
        let rows = a.setting_rows();
        let row = rows
            .iter()
            .find(|r| r.label == SettingKey::Presence.label())
            .expect("the presence row");
        assert!(row.editable, "it must still be toggleable");
        // A client id ships by default, so the row says when the change applies rather than what is
        // missing. A Discord id is public, so there is nothing to withhold.
        assert!(row.note.is_some_and(|n| n.contains("next episode")), "got {:?}", row.note);

        // Blanking the field must still resolve, via the shipped default — otherwise clearing it
        // instead of deleting the line would silently disable the feature.
        let mut config = Config::default();
        config.presence.client_id = Some(String::new());
        assert!(config.presence.resolved_client_id().is_some());
        assert!(settings::view(&config, &config).presence_client);
    }

    #[test]
    fn the_settings_cursor_is_not_clamped_by_an_empty_content_list() {
        // Settings is built from `SettingKey::ROWS`, not from `content`, so the shared clamp would
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
        assert_eq!(a.selected, SettingKey::ROWS.len() - 1, "clamped at the last row");
    }

    #[test]
    fn opening_an_entry_pushes_the_title_view_and_asks_for_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.nav.focus_stage();
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Command::LoadDetail(AnilistId::new(1))));
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
        let now = a.clock.now_instant();
        let job = a.jobs.iter_mut().find(|j| j.id == id).expect("a running job");
        job.started = now.checked_sub(by).expect("a clock that far back");
    }

    #[test]
    fn only_work_someone_waits_on_becomes_a_job() {
        let mut a = app();
        assert!(a.begin_job(&Command::LoadSeasonal).is_some());
        assert!(
            a.begin_job(&Command::LoadSources { id: AnilistId::new(1), episode: "3".into() })
                .is_some()
        );
        // Outbox writes and player commands finish as they are asked; a row for them would only
        // ever flash.
        assert!(
            a.begin_job(&Command::SetScore { id: AnilistId::new(1), score: 8.0 }).is_none()
        );
        assert!(a.begin_job(&Command::Player(PlayerCommand::TogglePause)).is_none());
        assert_eq!(a.jobs.len(), 2);
    }

    #[test]
    fn a_job_numbered_where_the_work_runs_is_shown_as_one_begun_here() {
        let mut a = app();
        let id = JobId::new(41);
        let before = a.clock.now_instant();
        a.apply(Event::JobStarted { job: id, activity: "checking providers".into() });
        assert_eq!(a.jobs.len(), 1);
        assert_eq!((a.jobs[0].id, a.jobs[0].label.as_str()), (id, "checking providers"));
        let started = a.jobs[0].started;
        assert!(before <= started && started <= a.clock.now_instant(), "timed from then");
        a.apply(Event::JobNote { job: id, note: "AniList limit".into(), wait: None });
        assert!(a.jobs[0].note.is_some());
        a.apply(Event::JobDone(id));
        assert!(a.jobs.is_empty());
    }

    #[test]
    fn a_job_runs_until_its_work_says_it_ended() {
        let mut a = app();
        let id = a.begin_job(&Command::CheckProviders).unwrap();
        a.apply(Event::JobNote { job: id, note: "AniList limit".into(), wait: None });
        assert_eq!(a.jobs[0].note.as_ref().map(|(n, _)| n.as_str()), Some("AniList limit"));

        a.apply(Event::JobDone(id));
        assert!(a.jobs.is_empty());
        // A second end, or one for a job long gone, is harmless.
        a.apply(Event::JobDone(id));
    }

    #[test]
    fn work_earns_the_row_only_once_it_has_taken_a_while() {
        let mut a = app();
        let older = a.begin_job(&Command::LoadCalendar).unwrap();
        let newer = a.begin_job(&Command::CheckProviders).unwrap();
        assert!(a.activity(a.clock.now_instant()).is_empty(), "quick answers never show");

        aged(&mut a, older, Duration::from_secs(5));
        aged(&mut a, newer, Duration::from_secs(1));
        let shown: Vec<_> = a.activity(a.clock.now_instant()).iter().map(|j| j.id).collect();
        assert_eq!(shown, [newer, older], "the thing just asked for leads");
    }

    // Requests are never cancelled, so every answer below is one the app really receives when
    // the user moves faster than the network: it has to be recognised as late and left alone.

    #[test]
    fn a_late_list_for_the_section_just_left_is_not_shown_under_the_next() {
        let mut a = app();
        let _ = a.go_to_section(Section::Seasonal);
        let _ = a.go_to_section(Section::Calendar);
        a.apply(Event::Content { view: ContentFor::Seasonal, content: entries(5) });
        assert!(matches!(a.content, Content::Loading), "the calendar showed seasonal's list");

        a.apply(Event::Content { view: ContentFor::Calendar, content: entries(3) });
        assert_eq!(a.content.len(), 3, "the calendar's own answer must still land");
    }

    #[test]
    fn a_slow_answer_to_an_earlier_search_does_not_replace_a_later_one() {
        let mut a = app();
        let _ = a.go_to_section(Section::Search);
        a.nav.focus_stage();
        for query in ["fri", "frieren"] {
            a.search_query = query.into();
            assert!(matches!(a.handle(Action::Open, 20), Some(Command::Search { .. })));
        }
        let answer = |query: &str, n| Event::Content {
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
        a.apply(Event::Content {
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
        a.apply(Event::Detail(Box::new(Entry::new(AnilistId::new(1), "Title one, in full"))));
        assert_eq!(a.detail.as_ref().map(|d| d.id), Some(AnilistId::new(2)));

        a.apply(Event::Detail(Box::new(Entry::new(AnilistId::new(2), "Title two, in full"))));
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
        a.apply(Event::Episodes { id: AnilistId::new(1), rows: vec![row("1")] });
        assert!(a.episodes.is_empty(), "title two's table showed title one's episodes");
        assert!(a.episodes_loading, "title two's own load is still outstanding");

        a.apply(Event::Episodes { id: AnilistId::new(2), rows: vec![row("1"), row("2")] });
        assert_eq!(a.episodes.len(), 2);
    }

    #[test]
    fn a_match_question_about_a_title_already_left_is_not_asked() {
        let mut a = app_at_episodes();
        a.apply(Event::MatchChoices {
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
        a.apply(Event::Sources {
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
        assert_eq!(a.go_to_section(Section::Home), Some(Command::LoadContinue));
        assert_eq!(a.go_to_section(Section::Seasonal), Some(Command::LoadSeasonal));
        assert_eq!(a.go_to_section(Section::Calendar), Some(Command::LoadCalendar));
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
            a.push_toast(Notice::info(format!("message {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "a burst of failures must not cover the screen");
        assert!(a.toasts[0].notice.text.contains('3'), "oldest are dropped first");

        for _ in 0..200 {
            a.tick_toasts();
        }
        assert!(a.toasts.is_empty(), "toasts must expire");
    }

    #[test]
    fn alerts_stay_up_longer_than_info() {
        assert!(
            Toast::from(Notice::alert("provider down")).ttl
                > Toast::from(Notice::info("switched")).ttl
        );
    }

    #[test]
    fn watch_order_says_so_when_there_is_nothing_to_show() {
        // Silence would read as the key being broken.
        let mut a = app();
        a.handle(Action::WatchOrder, 10);
        assert_eq!(a.toasts.len(), 1);
        assert!(a.toasts[0].notice.text.contains("nothing selected"));
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
        assert_eq!(task, Command::LoadDetail(AnilistId::new(9)));
        assert_eq!(a.nav.overlay(), None);
        assert!(
            matches!(a.nav.current(), StageView::Title(id) if *id == AnilistId::new(9)),
            "the picked title's view must be up"
        );
    }

    #[test]
    fn toggling_translation_updates_config_and_says_so() {
        let mut a = app();
        let before = a.settings.translation;
        let task = a.handle(Action::ToggleTranslation, 10);
        assert_eq!(task, Some(Command::SetTranslation { translation: before.toggled() }));
        Owner::new(Config::default()).answer(&mut a, task);
        assert_ne!(a.settings.translation, before);
        assert!(a.toasts[0].notice.text.contains("dub"));
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
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "at the top — Q quits"),
            "{:?}",
            a.toasts
        );
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
            a.toasts.iter().any(|t| t.notice.text == "at the top — ctrl+q quits"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn a_repeated_toast_is_kept_up_rather_than_stacked() {
        let mut a = app();
        for _ in 0..5 {
            a.push_toast(Notice::info("saved quality"));
        }
        assert_eq!(a.toasts.len(), 1, "one message, said once");
        assert_eq!(a.logs[0].count, 5, "the log still counts every one");
        a.push_toast(Notice::info("saved theme"));
        assert_eq!(a.toasts.len(), 2);
    }
}
