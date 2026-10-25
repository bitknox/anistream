//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Event`]s, so a slow provider or a large image decode cannot stall a frame.
//!
//! [`App`] is defined here, every field of it, so the module of each feature below reaches the
//! state it works on directly. [`App::apply`] hands each event to the feature it is about, and
//! [`App::press`] starts a key on its way: into a text field, an open overlay, or the screen.
//! The features are the sections' lists, the title page and its episode table, Now Playing,
//! downloads, the Providers table, plugin stores, trackers, settings, notices, the overlays and
//! the text fields. Each module's tests are the ones that exercise it, and what they share is
//! in `fixtures`.

use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_model::{
    catalog::{ContentFor, ContentList, Entry, LibrarySegment},
    command::Command,
    downloads::{DownloadRow, SeedRow},
    episodes::{DubExtent, EpisodeRow},
    event::Event,
    notice::{Notice, NoticeKind},
    settings::{SettingKey, SettingsView},
    sources::{MatchCandidate, MetaSourceRow, ProviderRow},
    stores::{
        InviteLinkView, LocalPluginRow, OpenJoinView, StoreInviteRow, StoreMemberRow, StoreRow,
    },
    sync::{ConflictRow, DeviceCodePrompt, SyncState},
};

use crate::{
    clock::Clock,
    eyecatch::Eyecatch,
    keymap::{Action, Keymap},
    nav::{Nav, Section, StageView},
    theme::Palette,
};

mod accounts;
mod cursor;
mod downloads;
mod episodes;
#[cfg(test)]
mod fixtures;
mod input;
mod lists;
mod notices;
mod overlays;
mod playback;
mod providers;
mod settings;
mod stores;
mod text;
mod title;

pub(crate) use self::lists::air_date;
use self::title::TitleFrame;
pub(crate) use self::title::watch_order;
pub use self::{
    accounts::sync_badge,
    cursor::{Cursor, Cursors},
    episodes::EpisodeFilter,
    lists::{
        SEARCH_FILTER_ROWS, cycle_search_filter, search_filter_summary, search_filter_value,
    },
    notices::{ACTIVITY_DELAY, Job, LOG_CAPACITY, LogRow, Toast, worded},
    overlays::{FrameReport, PALETTE_ROWS},
    playback::NowPlaying,
    providers::vpn_badge,
    settings::SettingLine,
    stores::{MemberLine, PluginChoice, StoreConfirm, StoreLine},
    text::{Field, TextField},
};

/// The whole application state.
pub struct App {
    pub nav: Nav,
    pub palette: Palette,
    pub keymap: Keymap,
    /// The config, as the engine last said: the Settings rows, and what else on screen the
    /// config decides. The engine owns the config, and the interface keeps no copy: a change is
    /// asked for, and shows once the engine says it is in effect.
    pub settings: SettingsView,
    pub content: ContentList,
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
    pub searched_query: anistream_model::search::SearchQuery,
    /// Server-side filters the next search will carry, its text aside. Sticky across queries on
    /// purpose: narrowing to a genre and then trying three titles should not mean setting it
    /// thrice.
    pub search_filter: anistream_model::search::SearchQuery,
    /// The results' paging: the last page on screen, whether AniList has more, and whether the
    /// next is on its way.
    search_pages: u32,
    search_more: bool,
    search_loading_more: bool,
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
    /// How far a title's dub goes, as the engine last said for it: the Episodes screen's
    /// "Dub to episode 6", for the title it names alone.
    pub dub: Option<(AnilistId, DubExtent)>,
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
    /// Who can join the store whose members are on screen, as its server last said.
    pub open_join: Option<(String, OpenJoinView)>,
    /// Whether the open link overlay's link went to the clipboard, as `invite_copied` says it.
    pub open_link_copied: bool,
    /// The store just opened from its page, whose link is shown when the answer comes.
    pub(crate) opening: Option<String>,
    /// Text waiting to go to the terminal's clipboard: see [`App::take_clipboard`].
    pub(crate) clipboard: Option<String>,
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
            content: ContentList::Loading,
            caption: None,
            cursors: Cursors::default(),
            texts: Default::default(),
            searched_query: Default::default(),
            search_filter: Default::default(),
            search_pages: 0,
            search_more: false,
            search_loading_more: false,
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
            dub: None,
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
            open_join: None,
            open_link_copied: false,
            opening: None,
            clipboard: None,
            plugin_action: None,
            plugin_options: Vec::new(),
            local_plugins: Vec::new(),
            publish_for: None,
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
                self.apply_calendar_window(anchor, pages, more_past)
            }
            Event::CalendarBackfill { page, entries, more_past } => {
                self.apply_calendar_backfill(page, entries, more_past)
            }
            Event::Caption { view, text } => self.apply_caption(view, text),
            Event::SearchPaging { query, has_next } => {
                self.apply_search_paging(query, has_next)
            }
            Event::SearchMore { query, page, entries, has_next } => {
                self.apply_search_more(query, page, entries, has_next)
            }
            // The terminal's Search has no rails, no tokens, no live search and no recents:
            // an empty Enter browses, with the filters it has.
            Event::LiveSearch { .. }
            | Event::Browse(_)
            | Event::Facets(_)
            | Event::RecentSearches(_) => {}
            Event::Content { view, content } => self.apply_content(view, content),
            // Every way into a title sets a stand-in `detail` for it before the fetch goes out, so
            // a full record for any other id answers a title the user has already left.
            Event::Detail(entry) => self.apply_detail(entry),
            // The engine names every title as the setting says; nothing here asks the viewer's.
            Event::ViewerTitleLanguage(_) => {}
            Event::TitlesChanged(_) => self.titles_changed(),
            Event::DeviceCode(prompt) => self.device_code = prompt,
            // A native app's sign-in sheet: the terminal signs in on a loopback port.
            Event::Authorize { .. } => {}
            // A native app's notifications and widgets, which the terminal never asks for: what
            // aired while it was closed is the digest's to say, at launch.
            Event::NewEpisodes(_) | Event::WidgetSnapshot(_) | Event::UpcomingNotices(_) => {}
            // What the update check found, for an app's Settings: the terminal's own notice
            // comes as a toast, naming `anistream --update`.
            Event::Update(_) => {}
            // A TV's remote, which the terminal never runs.
            Event::Remote(_) => {}
            // A native app's network monitor, which the terminal has none of: it never says the
            // path, so the engine never says it changed.
            Event::Connectivity { .. } => {}
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
            Event::OpenJoin { store, view } => self.apply_open_join(store, view),
            Event::Settings(view) => self.take_settings(view),
            // A native app's show sheet, which the terminal has none of.
            Event::ShowSettings(_) => {}
            Event::TitleDub { id, dub } => self.apply_dub(id, dub),
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
            // For a front end that hosts the player; the terminal's is mpv, which the engine
            // drives itself.
            Event::PlayerOps { .. }
            | Event::PlayerUi { .. }
            | Event::PlayerTimeline { .. }
            | Event::PlayerBuffered { .. } => {}
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

    /// Take the work the reducer queued outside a key's answer, oldest first: see `commands`.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
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
