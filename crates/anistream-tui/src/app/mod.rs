//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Event`]s, so a slow provider or a large image decode cannot stall a frame.

use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_model::{
    catalog::{Content, ContentFor, Entry, LibrarySegment, ResumePoint, SearchFilters},
    command::Command,
    downloads::{DownloadRow, DownloadState, SeedRow},
    episodes::EpisodeRow,
    event::Event,
    notice::{Notice, NoticeKind},
    progress::{EpisodeProgress, TitleProgress},
    settings::{SettingKey, SettingsView},
    sources::{MatchCandidate, MetaSourceRow, ProviderRow},
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
    theme::Palette,
};

mod accounts;
mod cursor;
mod downloads;
#[cfg(test)]
mod fixtures;
mod notices;
mod overlays;
mod playback;
mod providers;
mod settings;
mod text;
mod title;

use self::title::TitleFrame;
pub use self::{
    accounts::sync_badge,
    cursor::{Cursor, Cursors},
    notices::{ACTIVITY_DELAY, Job, LOG_CAPACITY, LogRow, Toast, worded},
    overlays::{FrameReport, PALETTE_ROWS},
    playback::NowPlaying,
    providers::vpn_badge,
    settings::SettingLine,
    text::{Field, TextField},
};

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
    /// Where each view's highlight is, and the window of each list that keeps one.
    pub cursors: Cursors,
    /// Every text field's line: Search's query and each prompt's. See [`Field`].
    ///
    /// The Kitsu sign-in's two are cleared on every open and dismiss, and the password is
    /// taken, not copied, when the exchange is dispatched.
    texts: [TextField; Field::ALL.len()],
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
    /// The help overlay's page. Its own field because Help opens on top of a picker, and paging
    /// through it must not move the picker's selection underneath.
    pub help_page: usize,
    /// How many pages the help table ran to at the last frame, as the renderer reported it — only
    /// it knows how the table breaks at this size — so paging stops at the last page rather than
    /// running past it and making the next few presses look dead.
    help_pages: usize,
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
    /// Work the reducer produced outside a key's answer, in the order it was produced, which the
    /// event loop drains each iteration: what the viewer asked for, without a key that could
    /// carry it. The calendar's backfill is triggered by scrolling near its top, which reports
    /// no task of its own; and a section's reload waits for the rail to settle, and goes out
    /// from the idle tick.
    ///
    /// A queue rather than one slot, so nothing put here can displace what is already waiting.
    /// What a playback leads to on its own, the next episode and the rest, is not here: the
    /// engine starts it, and says what it staged.
    commands: Vec<Command>,
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
    /// Which title a manual search would re-match.
    pub manual_target: Option<AnilistId>,
    /// Which text-valued setting the editor is writing.
    pub edit_target: Option<SettingKey>,
    /// Joined plugin stores.
    pub stores: Vec<StoreRow>,
    /// Whether the store list has arrived at least once, so "none joined" is not said early.
    pub stores_loaded: bool,
    /// The store whose members are on screen, and them.
    pub members_for: Option<String>,
    pub store_members: Vec<StoreMemberRow>,
    pub store_invites: Vec<StoreInviteRow>,
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
            cursors: Cursors::default(),
            texts: Default::default(),
            searched_query: String::new(),
            search_filter: SearchFilters::default(),
            search_in_results: false,
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
            help_page: 0,
            help_pages: 1,
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
            commands: Vec::new(),
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
            match_candidates: Vec::new(),
            match_context: None,
            sources: Vec::new(),
            source_context: None,
            manual_target: None,
            edit_target: None,
            stores: Vec::new(),
            stores_loaded: false,
            members_for: None,
            store_members: Vec::new(),
            store_invites: Vec::new(),
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
        self.content.entries().get(self.section_cursor().selected)
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
                self.apply_calendar_window(anchor, pages, more_past)
            }
            Event::CalendarBackfill { page, entries, more_past } => {
                self.apply_calendar_backfill(page, entries, more_past)
            }
            Event::Caption { view, text } => self.apply_caption(view, text),
            Event::Content { view, content } => self.apply_content(view, content),
            // Every way into a title sets a stand-in `detail` for it before the fetch goes out, so
            // a full record for any other id answers a title the user has already left.
            Event::Detail(entry) => self.apply_detail(entry),
            Event::DeviceCode(prompt) => self.device_code = prompt,
            Event::Seeding(rows) => self.seeding = rows,
            Event::Downloads(rows) => self.apply_downloads(rows),
            Event::LastAired(rows) => self.apply_last_aired(rows),
            Event::Status(status) => self.status = status,
            Event::Toast(notice) => self.push_toast(notice),
            // Decoded off the loop, and back through `artwork_decoded`.
            Event::ArtworkReady { .. } => {}
            Event::ImageFailed { url, offline } => self.images.mark_failed(&url, offline),
            Event::Providers(rows) => self.apply_providers(rows),
            Event::MetaStatus(rows) => self.meta_status = rows,
            Event::Stores(rows) => self.apply_stores(rows),
            Event::StorePreview(outcome) => self.apply_store_preview(outcome),
            Event::StoreJoined { store } => self.apply_store_joined(store),
            Event::InviteLink(view) => self.apply_invite_link(view),
            Event::StoreMembers { store, members, invites } => {
                self.apply_store_members(store, members, invites)
            }
            Event::LocalPlugins { store, plugins } => self.apply_local_plugins(store, plugins),
            Event::Settings(view) => self.take_settings(view),
            Event::SourcesChanged => self.status = "reloading sources…".into(),
            Event::ProviderNote(note) => self.provider_note = Some(note),
            Event::ProviderPreference(choice) => self.provider_preference = choice,
            Event::Sources { id, episode, candidates } => {
                self.apply_sources(id, episode, candidates)
            }
            Event::MatchChoices { id, provider_id, candidates } => {
                self.apply_match_choices(id, provider_id, candidates)
            }

            Event::Playback { position, duration, paused } => {
                self.apply_playback(position, duration, paused)
            }
            Event::SkipAvailable { kind, to } => self.apply_skip_available(kind, to),
            Event::SkipCleared => self.apply_skip_cleared(),
            // What the player changes of the config on its own is the engine's to keep and to
            // save, as it passes: see `anistream_engine::settings::remember`. It says the
            // settings after, the speed a new playback starts at among them. Saved from here,
            // they waited in a single slot, and a save that found it taken was skipped.
            Event::PlaybackSpeed(speed) => self.apply_playback_speed(speed),
            Event::ActiveProvider(provider) => self.active_provider = Some(provider),
            Event::PlaybackKept => self.apply_playback_kept(),
            Event::PlaybackStaged { id, title, episode, episode_title, speed } => {
                self.take_staged(id, title, episode, episode_title, speed);
            }
            // The window has changed hands, so what reports from here on is the successor. What
            // the episode left moves of its title, the engine says ahead of this.
            Event::EpisodeLeft { id, episode, .. } => self.apply_episode_left(id, episode),
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
            Event::PlaybackEnded { ending, .. } => self.apply_playback_ended(ending),
            Event::Sync(state) => self.apply_sync(*state),
            Event::JobStarted { job, activity } => self.start_job(job, activity),
            Event::JobNote { job, note, wait } => self.apply_job_note(job, note, wait),
            Event::JobDone(job) => self.jobs.retain(|j| j.id != job),
            Event::ProgressQueued => self.apply_progress_queued(),
            Event::Conflicts(rows) => self.apply_conflicts(rows),
            Event::Resumed { position } => self.apply_resumed(position),
            Event::Vpn(guard) => self.apply_vpn(guard),
            Event::Episodes { id, rows } => self.apply_episodes(id, rows),
            Event::Progress { id, progress, resume, episodes } => {
                self.apply_progress(id, progress, resume, episodes)
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

    /// Re-derive the visible episode list from the full one, keeping the cursor on the same
    /// episode.
    ///
    /// By episode, not by row: keeping the index put the cursor on whatever episode the new
    /// view had in that slot, so cycling the filter moved you without a key being pressed. An
    /// episode the filter now hides hands the cursor to the next one it keeps — which is also
    /// what makes marking down an unwatched-only list walk forward one episode at a time.
    fn apply_episode_filter(&mut self) {
        let filter = self.episode_filter;
        let under_cursor =
            self.episodes.get(self.cursors.episodes.selected).map(|r| r.number.clone());
        self.episodes = self.episodes_all.iter().filter(|r| filter.keeps(r)).cloned().collect();
        let kept = under_cursor.and_then(|number| {
            let from = self.episodes_all.iter().position(|r| r.number == number)?;
            let next = self.episodes_all[from..].iter().find(|r| filter.keeps(r))?;
            self.episodes.iter().position(|r| r.number == next.number)
        });
        self.cursors.episodes.selected = kept
            .unwrap_or(self.cursors.episodes.selected)
            .min(self.episodes.len().saturating_sub(1));
    }

    /// Whether the filter is hiding everything — distinct from a title with no episodes.
    pub fn episodes_all_filtered_out(&self) -> bool {
        self.episodes.is_empty() && !self.episodes_all.is_empty()
    }

    /// Releases to pick from, for the episode the source picker was asked about.
    fn apply_sources(
        &mut self,
        id: AnilistId,
        episode: String,
        candidates: Vec<SourceCandidate>,
    ) {
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
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Sources);
    }

    /// What a provider might have the title as, when it could not tell on its own: the question
    /// the Disambiguate overlay asks.
    fn apply_match_choices(
        &mut self,
        id: AnilistId,
        provider_id: String,
        candidates: Vec<MatchCandidate>,
    ) {
        // An empty set is not a question worth asking; the caller reports it as a
        // plain failure instead. Nor is one about a title the user has left.
        if candidates.is_empty() || !self.detail.as_ref().is_some_and(|d| d.id == id) {
            return;
        }
        self.episodes_loading = false;
        self.match_candidates = candidates;
        self.match_context = Some((id, provider_id));
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::Disambiguate);
    }

    /// A title's episode table.
    fn apply_episodes(&mut self, id: AnilistId, rows: Vec<EpisodeRow>) {
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
        if self.cursors.episodes.selected == 0
            && let Some(first) = self.episodes.iter().position(|r| !r.completed)
        {
            self.cursors.episodes.selected = first;
        }
    }

    /// Where a title stands, and each of its episodes, as the engine says once a watch or a mark
    /// has been written.
    fn apply_progress(
        &mut self,
        id: AnilistId,
        progress: Option<TitleProgress>,
        resume: Option<ResumePoint>,
        episodes: Vec<EpisodeProgress>,
    ) {
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
                    if let Some(said) = episodes.iter().find(|e| e.number == row.number) {
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
            || self.cursors.calendar.selected > CALENDAR_BACKFILL_MARGIN
        {
            return;
        }
        let Some(anchor) = self.calendar_anchor else { return };
        self.calendar_loading_past = true;
        self.commands.push(Command::LoadCalendarPast { anchor, page: self.calendar_pages + 1 });
    }

    /// The paging of the calendar just loaded: the instant its pages count back from, how many
    /// pages of the past it holds, and whether there are more.
    fn apply_calendar_window(&mut self, anchor: i64, pages: u32, more_past: bool) {
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

    /// A page of older broadcasts, as [`Self::backfill_calendar_if_near_top`] asked for, or word
    /// that it could not be had.
    fn apply_calendar_backfill(
        &mut self,
        page: Option<u32>,
        entries: Vec<Entry>,
        more_past: bool,
    ) {
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
            self.cursors.calendar.selected += added;
            self.cursors.calendar.offset += added;
        }
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
                    let id = self.downloads.get(self.cursors.downloads.selected)?.id;
                    return Some(Command::DownloadPause { id });
                }
                Action::HideFromContinue | Action::StopPlayback => {
                    let row = self.downloads.get(self.cursors.downloads.selected)?;
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
            Action::ShowLogs => self.show_logs(),
            Action::ShowConflicts => self.show_conflicts(),
            Action::CommandPalette => self.open_palette(),
            // Out of the search results is back into the query they answered, one step short
            // of the rail — the same order the two were entered in.
            Action::Back if self.search_in_results && self.on_search_root() => {
                self.search_in_results = false;
            }
            Action::Back => self.go_back(),
            Action::ToggleRail => self.nav.toggle_rail(),
            Action::JumpSection(n) => return self.jump_section(n),
            // Already on Search, `/` means "let me type" rather than "start again": it returns
            // to the field with the query and results as they were, without asking again.
            Action::FocusSearch if self.on_search_root() => self.focus_search_field(),
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
            Action::Left => self.left_to_rail(),
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
                return self.open_setting();
            }
            Action::Open if self.in_accounts_stage() => return self.toggle_account(),
            // Plugin stores live behind Providers: Enter there opens them, from the rail too,
            // because the provider table has nothing else to open.
            Action::Open if self.on_providers() => return self.open_stores(),
            Action::Open if self.nav.focus() == Focus::Stage && self.in_stores() => {
                return self.open_store_row();
            }
            Action::Open if self.nav.focus() == Focus::Stage && self.store_view().is_some() => {
                return self.enter_store_line();
            }
            Action::Open
                if self.nav.focus() == Focus::Stage && self.members_view().is_some() =>
            {
                return self.enter_member_row();
            }
            // Enter plays a finished download from disk. Nothing else on this screen is an "open".
            Action::Open if self.in_downloads_stage() => return self.open_download(),
            Action::DeleteDownload if self.in_downloads_stage() => {
                return self.delete_download();
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
            Action::Open if self.resumes_on_enter() => return self.resume_selected(),
            Action::Open => return self.open_selected(),
            // Space skips the detail screen entirely: from a list, straight into the next
            // episode you have not watched.
            Action::PlayNext => return self.play_next_unwatched(),
            Action::ShowEpisodes => return self.show_episodes(),
            Action::ToggleWatched if self.in_episodes() => return self.toggle_watched(),
            Action::MarkAllPrevious if self.in_episodes() => return self.mark_all_previous(),
            Action::DownloadRange if self.in_episodes() => self.open_download_range(),
            Action::DownloadRange => {
                self.push_toast(Notice::info("ranges are queued from the episode table"));
            }
            Action::Filter if self.in_episodes() => self.cycle_episode_filter(),
            // By the view, not the section: a title opened from the results is a title page,
            // and `f` there means what it means on every other title page.
            Action::Filter if self.on_search_root() => self.open_search_filters(),
            Action::FilterSearch => return self.filter_search(),
            Action::Filter => {
                self.push_toast(Notice::info("filters live in search and the episode table"));
            }
            Action::ToggleWatched | Action::MarkAllPrevious => {
                self.push_toast(Notice::info("watched marks live in the episode table"));
            }
            Action::OpenInBrowser => return self.open_in_browser(),
            Action::PickProvider => return self.pick_provider(),
            Action::PlayTrailer => return self.play_trailer(),
            Action::Recommendations => return self.show_recommendations(),
            Action::WatchOrder => return self.show_watch_order(),
            Action::FixMapping => return self.fix_mapping(),
            Action::ShowSources => return self.show_sources(),
            Action::PlayParty => return self.play_party(),
            Action::Download => return self.download_episode(),
            Action::ToggleSynopsis => self.synopsis_expanded = !self.synopsis_expanded,
            Action::SetListStatus => self.pick_list_status(),
            Action::RateTitle => self.pick_rating(),
            Action::HideFromContinue => return self.hide_from_continue(),
            Action::ForceResync => return self.force_resync(),
            // On the store screens, refreshing means asking the stores again, not re-checking
            // the provider table they were opened from.
            Action::Refresh if self.in_stores() || self.store_view().is_some() => {
                return Some(Command::LoadStores);
            }
            Action::Refresh if self.members_view().is_some() => return self.refresh_members(),
            // A title page and an episode table refresh themselves, not the section's list
            // under them — which is what `r` used to fetch, saying "loading the calendar" over
            // a title page that never changed.
            Action::Refresh if matches!(self.nav.current(), StageView::Title(_)) => {
                return self.refresh_title();
            }
            Action::Refresh if self.in_episodes() => return self.refresh_episodes(),
            Action::Refresh => return self.refresh_section(),
            Action::ToggleTranslation => return self.toggle_translation(),
            _ => self.works_elsewhere(action),
        }
        None
    }

    /// Start the episode selected in the timing sheet.
    fn play_selected_episode(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        let episode = self.episodes.get(self.cursors.episodes.selected)?.number.clone();
        Some(self.begin_playback(id, episode))
    }

    /// The episode table of the title here, asked for afresh.
    fn show_episodes(&mut self) -> Option<Command> {
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
            self.cursors.episodes.selected = 0;
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
        None
    }

    /// The highlighted episode marked watched, or unwatched if it was.
    fn toggle_watched(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        let (number, watched) = {
            let row = self.episodes.get(self.cursors.episodes.selected)?;
            (row.number.clone(), !row.completed)
        };
        // Shown once the engine has written it and says where the title now stands.
        self.marking = Some(id);
        Some(Command::SetWatched { id, episodes: vec![number], watched })
    }

    /// Every episode before the highlighted one that is not finished, marked watched.
    fn mark_all_previous(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        // Everything strictly before the selected row that is not already done —
        // the "I'm caught up to here" gesture.
        let marked: Vec<String> = self
            .episodes
            .iter()
            .take(self.cursors.episodes.selected)
            .filter(|r| !r.completed)
            .map(|r| r.number.clone())
            .collect();
        if marked.is_empty() {
            // Three different nothings, each said as what it is: "nothing before this
            // episode" over an empty table pointed at an episode that was not there.
            let why = match self.episodes.get(self.cursors.episodes.selected) {
                None => "no episodes listed to mark".to_owned(),
                Some(_) if self.cursors.episodes.selected == 0 => {
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
        Some(Command::SetWatched { id, episodes: marked, watched: true })
    }

    /// The prompt for a range of episodes to download, empty.
    fn open_download_range(&mut self) {
        self.field_mut(Field::Range).clear();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::DownloadRange);
    }

    /// Enter on a range of episodes: those of them that exist, queued for download.
    fn submit_range(&mut self) -> Option<Command> {
        let id = match self.nav.current() {
            StageView::Episodes(id) => *id,
            _ => return None,
        };
        // Resolved against episodes that actually exist, from the unfiltered
        // list — a filter narrows the view, not what can be fetched.
        let Some((from, to)) = parse_episode_range(self.text(Field::Range)) else {
            // Cleared, with the prompt left open for another try. Kept, the retry's
            // keys were appended to the refused text and refused again.
            self.field_mut(Field::Range).clear();
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
            self.field_mut(Field::Range).clear();
            self.push_toast(Notice::info("no episodes in that range"));
            return None;
        }
        self.nav.close_overlay();
        self.push_toast(Notice::info(format!(
            "queueing {}…",
            anistream_model::wording::plural(episodes.len(), "episode")
        )));
        Some(Command::DownloadMany { id, episodes })
    }

    /// The episode table's next filter, said as it is applied.
    fn cycle_episode_filter(&mut self) {
        self.episode_filter = self.episode_filter.next();
        self.apply_episode_filter();
        self.push_toast(Notice::info(format!("showing {}", self.episode_filter.label())));
    }

    /// The prompt for a provider's search by hand, for a title it matched wrongly.
    fn fix_mapping(&mut self) -> Option<Command> {
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
        self.field_mut(Field::ManualQuery).clear();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::ManualQuery);
        None
    }

    /// Enter on a search by hand: the search, or with nothing typed the pin forgotten.
    fn submit_manual_query(&mut self) -> Option<Command> {
        let query = self.text(Field::ManualQuery).trim().to_owned();
        let id = self.manual_target?;
        self.nav.close_overlay();
        // Empty enter is the undo: forget the pin, let the ladder decide again.
        if query.is_empty() {
            self.episodes.clear();
            self.episodes_loading = true;
            return Some(Command::ClearMatch { id });
        }
        Some(Command::ManualSearch { id, query })
    }

    /// The candidate picked, pinned as the title's match.
    fn confirm_match(&mut self) -> Option<Command> {
        let candidate = self.match_candidates.get(self.cursors.overlay.selected)?.clone();
        let (id, provider_id) = self.match_context.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        self.match_candidates.clear();
        self.match_context = None;
        self.episodes_loading = true;
        self.push_toast(Notice::info(format!("matched to {}", candidate.title)));
        Some(Command::FixMatch { id, provider_id, key: candidate.key })
    }

    /// An episode's releases to pick from: the highlighted one in the table, or the next
    /// unwatched one from a title.
    fn show_sources(&mut self) -> Option<Command> {
        // Same shape as Download: the highlighted episode from the table, the next
        // unwatched one from a title.
        let (id, episode) = match self.nav.current() {
            StageView::Episodes(id) => {
                (*id, self.episodes.get(self.cursors.episodes.selected)?.number.clone())
            }
            _ => {
                let entry = self.target()?;
                let episode = entry.progress.map_or(1, |(_, next)| next).to_string();
                (entry.id, episode)
            }
        };
        self.source_context = Some((id, episode.clone()));
        Some(Command::LoadSources { id, episode })
    }

    /// The release picked, played.
    fn confirm_source(&mut self) -> Option<Command> {
        let candidate = self.sources.get(self.cursors.overlay.selected)?.clone();
        let (id, episode) = self.source_context.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
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

    /// The episode table, asked for again.
    fn refresh_episodes(&mut self) -> Option<Command> {
        // The rows stay on screen until the answer lands, and the answer keeps the
        // cursor on its episode; the activity row says the table is being asked again.
        self.images.requests().retry_failed();
        self.title_on_screen().map(Command::LoadEpisodes)
    }

    /// The episode table for a title, when the rows loaded are that title's — what the player's
    /// Episodes menu lists. Empty for any other title rather than another show's rows.
    pub fn episode_rows_for(&self, id: AnilistId) -> &[EpisodeRow] {
        // Every row, not the filtered view: a filter on the title screen is about what to look
        // at there, not about which episodes exist. Asked by the rows' own title rather than
        // `detail`, which moves to a newly opened title before that title's rows arrive.
        if self.episodes_for == Some(id) { &self.episodes_all } else { &[] }
    }

    /// Take the work the reducer queued outside a key's answer, oldest first: see `commands`.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
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
        self.cursors.library = Cursor::default();
        self.content = Content::Loading;
        Some(Command::LoadLibrary(self.library_segment))
    }

    /// Whether the Search section's own view — the field and its results — is on top.
    fn on_search_root(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Search))
    }

    /// Back into Search's field, with the query and its results as they were.
    fn focus_search_field(&mut self) {
        self.search_in_results = false;
        self.nav.focus_stage();
    }

    /// The FILTERS overlay, on its first row.
    fn open_search_filters(&mut self) {
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::SearchFilters);
    }

    /// The FILTERS overlay over Search, from wherever it is asked for.
    fn filter_search(&mut self) -> Option<Command> {
        // From anywhere: land on Search first, because the overlay's Enter runs a
        // search and the results have to have somewhere to go. The jump closes any
        // overlays, so this one is opened after.
        let task = (self.nav.section() != Section::Search)
            .then(|| self.go_to_section(Section::Search))
            .flatten();
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::SearchFilters);
        task
    }

    /// Enter on FILTERS: the search, with the filters as they are set.
    fn confirm_search_filters(&mut self) -> Option<Command> {
        self.nav.close_overlay();
        let query = self.text(Field::Search).trim().to_owned();
        // A filter with no query is a browse, which is a legitimate search. Both
        // empty means there is nothing to ask yet, and that is not an error.
        if query.is_empty() && self.search_filter.is_empty() {
            return None;
        }
        self.searched_query = query.clone();
        self.content = Content::Loading;
        // New results start at their top, not at the row the old ones were left on.
        self.cursors.search = Cursor::default();
        Some(Command::Search { query, filter: self.search_filter.clone() })
    }

    /// Enter in Search's field: the search, or the highlighted result once the results are for
    /// the query as typed.
    fn submit_search(&mut self) -> Option<Command> {
        let query = self.text(Field::Search).trim().to_owned();
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
        self.cursors.search = Cursor::default();
        Some(Command::Search { query, filter: self.search_filter.clone() })
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

    /// A section's number, counted from one: that section, at the top of its own view.
    fn jump_section(&mut self, n: u8) -> Option<Command> {
        if let Some(section) = Section::from_index(usize::from(n).saturating_sub(1)) {
            return self.go_to_section(section);
        }
        None
    }

    /// Left with nothing further left on the screen: onto the rail, and out of the search
    /// results, so coming back lands in the field.
    fn left_to_rail(&mut self) {
        self.search_in_results = false;
        self.nav.focus_rail();
    }

    /// An action that means nothing here, saying where it does.
    fn works_elsewhere(&mut self, action: Action) {
        // An action that means nothing here — a playback key with nothing playing, a
        // Downloads key elsewhere, usually run from the palette — says where it does
        // something rather than doing nothing, which would read as the key being broken.
        let label = action.label();
        let mut chars = label.chars();
        let label = match chars.next() {
            Some(first) => first.to_lowercase().chain(chars).collect::<String>(),
            None => String::new(),
        };
        self.push_toast(Notice::info(format!("{label} works {}", action.where_it_works())));
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
            Action::Back => self.dismiss_field(),
            Action::CommandPalette => self.open_palette(),
            Action::Help => self.open_help(),
            Action::Quit => self.should_quit = true,
            // The two sign-in steps. Enter with nothing typed stays put — an empty
            // submit is a slip of the finger, not an answer.
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuUsername) => {
                return self.submit_kitsu_user();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::KitsuPassword) => {
                return self.submit_kitsu_password();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::JoinStore) => {
                return self.submit_join_link();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::InviteName) => {
                return self.submit_invite_name();
            }
            // Mid-query is exactly when filters are wanted, so ctrl+f cannot wait for the
            // box to lose focus.
            Action::FilterSearch => self.open_search_filters(),
            // The palette takes text, so it reaches this path rather than the overlay block in
            // `handle`. It used to close on Enter without running anything, and move the list
            // *behind* it on the arrows — so the one mechanism meant to make every action
            // discoverable could show you an action and then refuse to perform it.
            Action::Open if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                return self.confirm_overlay(visible_rows);
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::EditSetting) => {
                return self.submit_setting();
            }
            Action::Open if self.nav.overlay() == Some(&Overlay::DownloadRange) => {
                return self.submit_range();
            }
            // Enter runs the manual search; the results come back as ordinary match
            // choices, so the whole Disambiguate tail is reused unchanged.
            Action::Open if self.nav.overlay() == Some(&Overlay::ManualQuery) => {
                return self.submit_manual_query();
            }
            Action::Down if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                self.palette_down()
            }
            Action::Up if self.nav.overlay() == Some(&Overlay::CommandPalette) => {
                self.cursors.overlay.selected = self.cursors.overlay.selected.saturating_sub(1);
            }
            Action::Open if self.nav.section() == Section::Search => {
                return self.submit_search();
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

    /// Whether Enter resumes the highlighted row rather than opening it: a part-watched title
    /// on the Home list. One answer for the key and for the hint that names it.
    pub fn resumes_on_enter(&self) -> bool {
        matches!(self.nav.current(), StageView::Section(Section::Home))
            && self.selected_entry().is_some_and(|e| e.resume.is_some() && e.progress.is_some())
    }

    /// The highlighted Continue row's episode, resumed where it was left.
    fn resume_selected(&mut self) -> Option<Command> {
        let entry = self.selected_entry().expect("checked");
        let id = entry.id;
        let next = entry.progress.expect("checked").1;
        Some(self.begin_playback(id, next.to_string()))
    }

    /// The highlighted title taken off the Continue list.
    fn hide_from_continue(&mut self) -> Option<Command> {
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
        let home = &mut self.cursors.home;
        let entry = entries.remove(home.selected.min(entries.len() - 1));
        home.selected = home.selected.min(entries.len().saturating_sub(1));
        home.offset = home.offset.min(home.selected);
        // Say how to undo, because there is no key for it — the way back is the
        // same thing that put it here.
        self.push_toast(Notice::info(format!(
            "{} removed — watching it again brings it back",
            entry.title
        )));
        Some(Command::HideFromContinue { id: entry.id })
    }

    /// Whether Enter in Search's field opens the highlighted result rather than searching:
    /// the results on screen are already for the query as typed.
    pub fn search_opens_on_enter(&self) -> bool {
        let query = self.text(Field::Search).trim();
        !query.is_empty() && query == self.searched_query && !self.content.is_empty()
    }

    /// Whether the episode table currently has focus.
    pub fn in_episodes(&self) -> bool {
        matches!(self.nav.current(), StageView::Episodes(_))
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
        // Entered at its top, as every section is: see `Cursors`.
        *self.cursors.section_mut(section) = Cursor::default();
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
            // Queued because the idle tick has no return channel, the same route a scroll's
            // backfill takes.
            self.commands.push(task);
        }
    }

    /// Jump to `section`, at the top of its own view, and ask for what it shows.
    ///
    /// Every way into a section is through here or the rail's step, and both put its cursor at
    /// the top: the sections shared one cursor once, which a change of section reset, and
    /// keeping that is what lets each have its own with nothing on screen telling them apart.
    pub fn go_to_section(&mut self, section: Section) -> Option<Command> {
        self.nav.go_to(section);
        *self.cursors.section_mut(section) = Cursor::default();
        self.detail = None;
        self.title_trail.clear();
        self.search_in_results = false;
        self.reload()
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

    /// The caption for a list, taken while that list is the one on screen. An empty one clears
    /// it.
    fn apply_caption(&mut self, view: ContentFor, text: String) {
        if self.content_view().as_ref() == Some(&view) {
            self.caption = (!text.is_empty()).then_some((view, text));
        }
    }

    /// A list for a view, taken only while that view is on screen: see [`Self::content_view`].
    fn apply_content(&mut self, view: ContentFor, content: Content) {
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
            for entry in self.content.entries().iter().filter(|e| e.list_status.is_some()) {
                self.statuses_set.remove(&entry.id);
            }
        }
        // Selection must stay in range or the next render would index past the end.
        let len = self.content.len();
        let cursor = self.cursors.section_mut(self.nav.section());
        cursor.selected = cursor.selected.min(len.saturating_sub(1));
        if len == 0 {
            *cursor = Cursor::default();
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
            self.cursors.calendar = Cursor {
                selected: next,
                offset: next.saturating_sub((next - start_of_today.min(next)).min(12)),
            };
        }
    }

    /// When each title's latest episode aired, and which episode it was.
    fn apply_last_aired(&mut self, rows: Vec<(AnilistId, u32, i64)>) {
        // Applied to both the list and any open detail, since the same title can be
        // on screen twice and a line that appears in one place but not the other
        // reads as a bug.
        if let Content::Entries(entries) = &mut self.content {
            for entry in entries.iter_mut() {
                if let Some((_, ep, at)) = rows.iter().find(|(id, _, _)| *id == entry.id) {
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
                if !self.text(Field::Search).trim().is_empty()
                    || !self.search_filter.is_empty() =>
            {
                // Recorded as submitted, the same as Enter does, because the answer is tagged
                // with this query and only taken while it is still the one on screen.
                let query = self.text(Field::Search).trim().to_owned();
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

    /// The section's list, asked for again.
    fn refresh_section(&mut self) -> Option<Command> {
        // Asking again means the covers too: one that failed while the network was
        // down should not wait out its backoff once you have said it is back.
        self.images.requests().retry_failed();
        self.reload()
    }

    /// Whether the section on screen fills `content` with a list it is about to ask for.
    fn fetches_list(&self) -> bool {
        match self.nav.section() {
            Section::Home | Section::Seasonal | Section::Calendar | Section::Library => true,
            Section::Search => {
                !self.text(Field::Search).trim().is_empty() || !self.search_filter.is_empty()
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
            let from = self.cursors.episodes.selected.saturating_sub(1);
            for row in self.episodes.iter().skip(from).take(3) {
                if let Some(url) = &row.thumbnail {
                    urls.push(url.clone());
                }
            }
        }
        // A small lookahead so scrolling does not stall on every step.
        for entry in self
            .content
            .entries()
            .iter()
            .skip(self.section_cursor().offset)
            .take(visible_rows.saturating_add(4))
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
        self.cursors.stores.selected = self.cursors.stores.selected.min(self.stores.len());

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
                self.cursors.store_lines.selected = self.cursors.store_lines.selected.min(last);
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

    /// A store just joined.
    fn apply_store_joined(&mut self, store: String) {
        // Into the store just joined, but only from the list it was joined from. A join
        // that finishes after the user has gone elsewhere must not drag them back.
        if self.nav.current() == &StageView::Stores && self.store(&store).is_some() {
            self.cursors.store_lines.selected = 0;
            self.nav.push(StageView::Store(store));
        }
    }

    /// An invite link just made, shown until it is dismissed.
    fn apply_invite_link(&mut self, view: InviteLinkView) {
        self.invite_link = Some(view);
        self.invite_copied = false;
        self.nav.open_overlay(Overlay::InviteLink);
    }

    /// A store's members and its unused invites, for the members screen.
    fn apply_store_members(
        &mut self,
        store: String,
        members: Vec<StoreMemberRow>,
        invites: Vec<StoreInviteRow>,
    ) {
        if self.members_for.as_deref() == Some(store.as_str()) {
            self.store_members = members;
            self.store_invites = invites;
            let rows = self.store_members.len() + self.store_invites.len();
            self.cursors.members.selected =
                self.cursors.members.selected.min(rows.saturating_sub(1));
        }
    }

    /// The plugins that could be published to a store, for the publish picker.
    fn apply_local_plugins(&mut self, store: String, plugins: Vec<LocalPluginRow>) {
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
        self.cursors.overlay.selected = 0;
        self.nav.open_overlay(Overlay::PublishPick);
    }

    fn open_stores(&mut self) -> Option<Command> {
        self.cursors.stores.selected = 0;
        self.nav.push(StageView::Stores);
        Some(Command::LoadStores)
    }

    /// Enter on the store list: into a store, or the join prompt from the last row.
    fn open_store_row(&mut self) -> Option<Command> {
        match self.stores.get(self.cursors.stores.selected) {
            Some(store) => {
                let id = store.id.clone();
                self.cursors.store_lines.selected = 0;
                self.nav.push(StageView::Store(id));
            }
            None => {
                self.field_mut(Field::JoinLink).clear();
                self.nav.open_overlay(Overlay::JoinStore);
            }
        }
        None
    }

    /// Enter on a pasted invite link: the store it names, checked before anything is joined.
    fn submit_join_link(&mut self) -> Option<Command> {
        // A link that wrapped in a chat window arrives with spaces and line breaks in
        // it, and a URL has no place for either.
        let link: String =
            self.text(Field::JoinLink).chars().filter(|c| !c.is_whitespace()).collect();
        if link.is_empty() {
            return None;
        }
        self.nav.close_overlay();
        Some(Command::PreviewStore { link: Secret(link) })
    }

    /// Enter on an invite's name: the invite, made for the store on screen.
    fn submit_invite_name(&mut self) -> Option<Command> {
        let name = self.text(Field::InviteName).trim().to_owned();
        if name.is_empty() {
            return None;
        }
        let store = self.store_view()?.to_owned();
        self.field_mut(Field::InviteName).clear();
        self.nav.close_overlay();
        Some(Command::CreateInvite { store, name: Some(name) })
    }

    fn open_store_line(&mut self, id: &str) -> Option<Command> {
        let store = self.store(id)?.clone();
        let line = *self.store_lines(&store).get(self.cursors.store_lines.selected)?;
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
                self.field_mut(Field::InviteName).clear();
                self.nav.open_overlay(Overlay::InviteName);
                None
            }
            StoreLine::Members => {
                self.members_for = Some(store.id.clone());
                self.store_members.clear();
                self.store_invites.clear();
                self.cursors.members.selected = 0;
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

    /// Enter on a store's page: its highlighted line.
    fn enter_store_line(&mut self) -> Option<Command> {
        let id = self.store_view()?.to_owned();
        self.open_store_line(&id)
    }

    /// Open the plugin overlay for one of a store's plugins, with its choices as they stand now.
    pub fn show_plugin(&mut self, store: &str, plugin: &str) {
        self.plugin_action = Some((store.to_owned(), plugin.to_owned()));
        self.plugin_options = self.current_plugin_choices();
        self.cursors.overlay.selected = 0;
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
        let choice = self.plugin_options.get(self.cursors.overlay.selected)?.clone();
        self.plugin_options.clear();
        let name = self.plugin_in_action().map(|(_, p)| p.name.clone()).unwrap_or_default();
        let (store, plugin) = self.plugin_action.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
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
                self.field_mut(Field::JoinLink).clear();
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

    /// The plugin picked, published to the store on screen.
    fn confirm_publish(&mut self) -> Option<Command> {
        let plugin = self.local_plugins.get(self.cursors.overlay.selected)?.id.clone();
        let store = self.publish_for.clone()?;
        self.nav.close_overlay();
        self.cursors.overlay.selected = 0;
        Some(Command::PublishPlugin { store, plugin })
    }

    /// Enter on the members screen: remove a member, or cancel an unused invite.
    fn open_member_row(&mut self, store: &str) -> Option<Command> {
        let at = self.cursors.members.selected;
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

    /// Enter on a store's members: the highlighted row.
    fn enter_member_row(&mut self) -> Option<Command> {
        let id = self.members_view()?.to_owned();
        self.open_member_row(&id)
    }

    /// A store's members, asked for again.
    fn refresh_members(&mut self) -> Option<Command> {
        let store = self.members_view()?.to_owned();
        Some(Command::LoadMembers { store })
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

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

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
        a.set_cursor(80);
        a.set_offset(61);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
            "a scroll through rows already loaded must not spend a request"
        );

        // Within a screenful of the top, the page behind is asked for — early enough that
        // there are still rows left to scroll through while it arrives.
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_commands(),
            [Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }],
            "nearing the top should reach for the page behind the ones loaded"
        );

        // And exactly once. A held arrow key asking per keystroke would empty a
        // 30-per-minute budget answering one question.
        for _ in 0..5 {
            a.handle(Action::Up, 20);
            assert!(a.take_commands().is_empty(), "a held arrow must not re-request page four");
        }
    }

    #[test]
    fn backfilled_broadcasts_land_above_the_viewport_without_moving_it() {
        let mut a = calendar_loaded();
        a.set_cursor(8);
        a.set_offset(4);

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
        assert_eq!(a.cursor(), 58, "the selection must still be on the same broadcast");
        assert_eq!(a.offset(), 54, "and that broadcast must still be in the same place");
        assert_eq!(a.content.entries()[a.cursor()].title, "Title 8");
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
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        assert!(!a.take_commands().is_empty(), "the scroll should have asked for page four");

        a.apply(Event::CalendarBackfill { page: None, entries: Vec::new(), more_past: true });
        assert_eq!(a.content.len(), 150, "a failed backfill must not disturb the list");

        a.handle(Action::Up, 20);
        assert_eq!(
            a.take_commands(),
            [Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 }],
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

        a.set_cursor(2);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
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

        a.set_cursor(2);
        a.handle(Action::Up, 20);
        assert!(
            a.take_commands().is_empty(),
            "page four of the previous timeline is not worth a request against the new one"
        );
    }

    /// What the reducer asks for without a key waits in a queue, in the order it was asked for,
    /// and nothing put there displaces what is already waiting: a backfill, then the reload of
    /// the section the rail was stepped to after it, both go out. In one slot the reload took
    /// the backfill's place, which the loop never let happen only because it took the slot
    /// between the two.
    #[test]
    fn work_asked_for_without_a_key_waits_in_order_and_displaces_nothing() {
        let mut a = calendar_loaded();
        a.set_cursor(CALENDAR_BACKFILL_MARGIN + 1);
        a.handle(Action::Up, 20);
        a.nav.focus_rail();
        a.handle(Action::Down, 20);
        assert_eq!(a.nav.section(), Section::Seasonal);
        // The rail settles, and the section's list is asked for from the idle tick.
        for _ in 0..=RELOAD_IDLE_TICKS {
            a.tick_toasts();
        }
        assert_eq!(
            a.take_commands(),
            [
                Command::LoadCalendarPast { anchor: CALENDAR_TEST_ANCHOR, page: 4 },
                Command::LoadSeasonal
            ]
        );
        assert!(a.take_commands().is_empty(), "taken once, and gone");
    }

    #[test]
    fn enter_in_the_episode_table_plays_rather_than_opening() {
        // Enter opens a title everywhere else, so this is the one place it has to mean
        // something different — the table *is* the list of things to play.
        let mut a = app_at_episodes();
        a.set_episode_cursor(4);
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
        a.set_episode_cursor(4);
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

        a.set_episode_cursor(3);
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
        a.set_episode_cursor(0);
        a.handle(Action::MarkAllPrevious, 20);
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "no episodes listed to mark"),
            "{:?}",
            a.toasts
        );
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
    fn x_on_home_dismisses_the_selected_title_from_continue() {
        let mut a = app();
        a.apply(a.content_here(entries(3)));
        a.set_cursor(1);
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
        assert!(a.text(Field::JoinLink).is_empty(), "the spent link is not kept");

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
        assert_eq!(a.text(Field::JoinLink), "https://plugins.example/join/abc");
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("missing the store's key")));
    }

    #[test]
    fn a_dismissed_join_prompt_forgets_the_link() {
        let mut a = at_stores(Vec::new());
        a.handle(Action::Open, 20);
        type_text(&mut a, "https://plugins.example/join/abc#key");
        a.handle(Action::Back, 20);
        assert!(a.text(Field::JoinLink).is_empty());
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
        assert_eq!(a.member_cursor(), 1);
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

    /// The store screens each keep a cursor of their own: stepping back out of a store finds the
    /// store list where it was, and a store's page opens at its top.
    #[test]
    fn stepping_back_out_of_a_store_finds_the_store_list_where_it_was() {
        let mut a = at_stores(vec![
            store_row("a", false, Vec::new()),
            store_row("b", false, Vec::new()),
        ]);
        a.handle(Action::Down, 20);
        a.handle(Action::Open, 20);
        assert_eq!(a.nav.current(), &StageView::Store("b".into()));
        a.handle(Action::Down, 20);
        assert_eq!(a.store_line_cursor(), 1);
        a.handle(Action::Back, 20);
        assert_eq!(a.nav.current(), &StageView::Stores);
        assert_eq!(a.store_cursor(), 1, "the store list is where it was left");
        a.handle(Action::Open, 20);
        assert_eq!(a.store_line_cursor(), 0, "a store's page opens at its top");
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
        assert_eq!(a.store_cursor(), 2, "one past the stores is the join row");
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
    fn stepping_a_library_segment_reloads_and_resets_the_selection() {
        // A stale selection index into a different list would point at the wrong title.
        let mut a = app();
        a.go_to_section(Section::Library);
        a.apply(a.content_here(entries(10)));
        a.set_cursor(7);
        a.nav.focus_stage();

        let task = a.handle(Action::Right, 20);
        assert_eq!(a.library_segment, LibrarySegment::Planning);
        assert_eq!(task, Some(Command::LoadLibrary(LibrarySegment::Planning)));
        assert_eq!(a.cursor(), 0);
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

    fn setting_row(id: SettingKey) -> usize {
        SettingKey::ROWS.iter().position(|s| *s == id).expect("a settings row")
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
    fn back_through_a_related_titles_episodes_finds_the_parents_table() {
        let mut a = app_at_episodes();
        if let Some(detail) = a.detail.as_mut() {
            detail.related = vec![related(90, "Its sequel")];
        }
        a.set_episode_cursor(4);
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
        assert_eq!(a.episodes[a.episode_cursor()].number, "5", "and where it was left");
    }

    #[test]
    fn the_filter_keeps_the_same_episode_under_the_cursor() {
        let mut a = app_at_episodes();
        for n in ["1", "2", "3"] {
            set_watched(&mut a, n, true);
        }
        a.apply_episode_filter();
        let under = |a: &App| a.episodes[a.episode_cursor()].number.clone();

        a.set_episode_cursor(4);
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::Unwatched);
        assert_eq!(under(&a), "5", "the cursor kept its row index, not its episode");
        a.handle(Action::Filter, 5);
        a.handle(Action::Filter, 5);
        assert_eq!(a.episode_filter, EpisodeFilter::All);
        assert_eq!(under(&a), "5");

        // An episode the filter hides hands the cursor to the next one it keeps.
        a.set_episode_cursor(1);
        a.handle(Action::Filter, 5);
        assert_eq!(under(&a), "4");
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
        assert!(a.take_commands().is_empty());
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
        assert_eq!(a.cursor(), 0);
        for _ in 0..12 {
            a.handle(Action::Down, 5);
        }
        assert_eq!(a.cursor(), 12);
        assert!(a.offset() > 0 && a.cursor() < a.offset() + 5, "the window follows");
        a.handle(Action::Bottom, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.cursor(), 39, "bounded at the last result");
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
        a.set_cursor(setting_row(SettingKey::Motion));
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
            a.set_cursor(setting_row(row));
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

        a.set_episode_cursor(1);
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
        assert_eq!(a.episode_cursor(), 1, "and the cursor stays put");

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
        assert_eq!(a.text(Field::Search), "mecha", "the query survives opening the overlay");
    }

    #[test]
    fn the_filter_key_outside_search_and_episodes_says_where_filters_live() {
        let mut a = app();
        a.handle(Action::Filter, 10);
        assert!(!a.nav.has_overlay());
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("filters live")));
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
    fn switching_section_clears_stale_selection_and_detail() {
        let mut a = app();
        a.apply(a.content_here(entries(10)));
        a.handle(Action::Bottom, 5);
        a.handle(Action::Open, 5);

        a.go_to_section(Section::Seasonal);
        assert_eq!(a.cursor(), 0);
        assert!(a.detail.is_none(), "stale detail from another section");
        assert_eq!(a.nav.depth(), 1);
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
            a.set_text(Field::Search, query.into());
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
}
