//! Application state and the reducer over it.
//!
//! State lives here and rendering reads it; nothing in [`crate::screens`] mutates. That
//! split is what keeps the event loop honest — the UI thread only ever applies already-
//! computed [`Event`]s, so a slow provider or a large image decode cannot stall a frame.

use anistream_core::{ids::AnilistId, traits::SourceCandidate};
use anistream_model::{
    catalog::{Content, ContentFor, Entry, LibrarySegment, SearchFilters},
    command::Command,
    downloads::{DownloadRow, DownloadState, SeedRow},
    episodes::EpisodeRow,
    event::Event,
    notice::{Notice, NoticeKind},
    settings::{SettingKey, SettingsView},
    sources::{MatchCandidate, MetaSourceRow, ProviderRow},
    stores::{InviteLinkView, LocalPluginRow, StoreInviteRow, StoreMemberRow, StoreRow},
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
mod episodes;
#[cfg(test)]
mod fixtures;
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
    stores::{PluginChoice, StoreConfirm, StoreLine},
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

    /// Take the work the reducer queued outside a key's answer, oldest first: see `commands`.
    pub fn take_commands(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.commands)
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

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    fn setting_row(id: SettingKey) -> usize {
        SettingKey::ROWS.iter().position(|s| *s == id).expect("a settings row")
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

    // Requests are never cancelled, so every answer below is one the app really receives when
    // the user moves faster than the network: it has to be recognised as late and left alone.

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
