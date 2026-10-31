//! The engine as the window sees it, on GTK's main loop: commands go out through [`Hub::send`],
//! which only queues them for the engine's runtime, and the engine's events come back in
//! batches, are kept where several screens read them, and are handed to every screen that
//! listens. Covers and banners are fetched by the engine and decoded on a worker thread.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::Weak,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_engine::engine::Engine;
use anistream_model::jobs::JobId;
use anistream_model::{
    catalog::{ContentFor, Entry},
    command::Command,
    downloads::{DownloadRow, DownloadState, SeedRow},
    event::Event,
    party::{PartyForm, PartyMember, PartyView},
    settings::{SettingKey, SettingRow, SettingsView},
    sources::{GuardView, ProviderOrigin, ProviderRow, SourceOption},
    stores::{StorePluginRow, StorePluginStatus, StoreRow},
    sync::SyncState,
};
use gtk::{gdk, gio, glib};

/// A tracker's sign-in under way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignIn {
    /// Waiting on the engine's work: the device code's wait, Kitsu's password, or the
    /// browser's redirect being finished. Over when the job is.
    Job(Option<JobId>),
    /// The sign-in page is open in the browser, until its redirect comes back or the viewer
    /// cancels.
    Browser,
}

/// What several screens read, kept as the engine last said it.
#[derive(Default)]
pub struct Known {
    /// Every title seen in a list or on its page, by id, as it last stood.
    pub entries: HashMap<u32, Entry>,
    pub settings: Option<SettingsView>,
    /// Episodes with a discussion on AniList, by title and episode, with the comments in it.
    pub discussions: HashMap<(u32, u32), u32>,
    pub sync: HashMap<String, SyncState>,
    pub downloads: Vec<DownloadRow>,
    pub seeding: Vec<SeedRow>,
    pub providers: Vec<ProviderRow>,
    pub stores: Vec<StoreRow>,
    /// Whether the device has no network, as the engine last said.
    pub offline: bool,
    /// Whether trailers may start by themselves, as the engine last said.
    pub autoplay: Option<anistream_model::trailer::AutoplayView>,
    /// Whether the engine has said what is downloading yet.
    pub downloads_said: bool,
    /// Whether the engine has said which providers there are yet.
    pub providers_said: bool,
    /// Why no sources are available, in the engine's words, while none are.
    pub provider_note: Option<String>,
    /// The jobs running, by number, with their activity: "syncing".
    pub jobs: HashMap<JobId, String>,
    /// The trackers being signed in to, and how.
    pub signing: HashMap<String, SignIn>,
    /// The torrent connection check's last verdict, and when it was heard; `None` while
    /// torrents are off or before the first.
    pub guard: Option<(GuardView, std::time::Instant)>,
    /// The update check's last word.
    pub update: Option<anistream_model::updates::UpdateStatus>,
    /// The watch party this device is in, while it is in one.
    pub party: Option<PartyView>,
    /// The party's members, this device's first, once the room has listed them.
    pub party_members: Option<Vec<PartyMember>>,
    /// What the form that starts a party opens with, once the engine has said.
    pub party_form: Option<PartyForm>,
    /// What each source has of a title, as the engine last listed them for the source picker, in
    /// the order they are tried.
    pub source_options: HashMap<u32, Vec<SourceOption>>,
    /// The match picker's state per title: the questions sources asked, the searches by hand and
    /// what each title is matched to.
    pub matches: crate::matching::MatchBook,
    /// The pins the engine could not use that were said, by title and source, so each is told
    /// once until the title's source is chosen again.
    pin_said: std::collections::HashSet<(u32, String)>,
}

impl Known {
    pub fn setting(&self, key: SettingKey) -> Option<SettingRow> {
        let view = self.settings.as_ref()?;
        view.rows.iter().chain(&view.native_rows).find(|r| r.key == key).cloned()
    }

    /// The account the sidebar shows: AniList when signed in, else the first other tracker
    /// that is, in the order Accounts lists them.
    pub fn account(&self) -> Option<&SyncState> {
        self.anilist().or_else(|| {
            ["mal", "kitsu", "simkl", "trakt"]
                .iter()
                .find_map(|t| self.sync.get(*t).filter(|s| s.connected))
        })
    }

    /// The AniList account, when signed in.
    pub fn anilist(&self) -> Option<&SyncState> {
        self.sync.get("anilist").filter(|s| s.connected)
    }

    /// The sources in the order failover tries them: by where each stands in the order setting,
    /// a plugin it does not name where `"plugins"` stands, or last.
    pub fn ordered_providers(&self) -> Vec<ProviderRow> {
        let order = self.settings.as_ref().map(|s| s.sources.order.clone()).unwrap_or_default();
        sorted(&self.providers, &order)
    }

    /// The order setting as it stands.
    pub fn source_order(&self) -> Vec<String> {
        self.settings.as_ref().map(|s| s.sources.order.clone()).unwrap_or_default()
    }

    /// Where an installed plugin came from: the store that keeps it updated and its row there,
    /// or the first store that lists it as added by hand.
    pub fn origin(&self, plugin: &str) -> Option<(StoreRow, StorePluginRow)> {
        let mut by_hand = None;
        for store in &self.stores {
            let Some(row) = store.plugins.iter().find(|p| p.id == plugin) else { continue };
            match row.status {
                StorePluginStatus::Installed | StorePluginStatus::Update { .. } => {
                    return Some((store.clone(), row.clone()));
                }
                StorePluginStatus::AddedByHand if by_hand.is_none() => {
                    by_hand = Some((store.clone(), row.clone()));
                }
                _ => {}
            }
        }
        by_hand.or_else(|| {
            // A plugin from a store the list has not come back from yet.
            let row = self.providers.iter().find(|p| p.id == plugin)?;
            let ProviderOrigin::Store { id, .. } = &row.origin else { return None };
            let store = self.stores.iter().find(|s| &s.id == id)?;
            Some((store.clone(), store.plugins.iter().find(|p| p.id == plugin)?.clone()))
        })
    }

    /// Whether a sync is running.
    pub fn syncing(&self) -> bool {
        self.jobs.values().any(|activity| activity == "syncing")
    }

    /// Whether this install has been used: a source, a tracker signed in, or a watch history.
    /// First launch is shown only to an install that has none of them.
    pub fn used(&self) -> bool {
        self.providers.iter().any(|p| p.kind != "torrent")
            || !self.stores.is_empty()
            || self.sync.values().any(|s| s.connected)
            || self.entries.values().any(|e| e.progress.is_some_and(|p| p.done > 0))
    }

    /// Episodes downloading or waiting to, and those seeding: what closing the window stops.
    pub fn running(&self) -> (usize, usize) {
        let downloading = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Active | DownloadState::Queued))
            .count();
        (downloading, self.seeding.len())
    }

    /// A source, or a release of one, picked for a title; `None` for Automatic. Shown at once, so
    /// the picker reads right before the engine's next listing, and a pin the engine cannot use
    /// is told again.
    pub fn choose(&mut self, id: AnilistId, provider: Option<&str>, release: Option<&str>) {
        self.pin_said.retain(|(at, _)| *at != id.get());
        if let Some(options) = self.source_options.get_mut(&id.get()) {
            crate::choices::chose(options, provider, release);
        }
    }

    /// Whether the notice for a pin the engine could not use is new, and so said.
    pub fn say_pin(&mut self, id: AnilistId, provider: &str) -> bool {
        self.pin_said.insert((id.get(), provider.to_owned()))
    }

    /// A title page opens: a pin that cannot be used is told again for this visit.
    pub fn visit(&mut self, id: AnilistId) {
        self.pin_said.retain(|(at, _)| *at != id.get());
    }

    /// Whether there is a source to match a title on, for Fix this match.
    pub fn can_fix_match(&self) -> bool {
        self.providers.iter().any(|p| p.usable)
    }

    fn take(&mut self, event: &Event) {
        self.matches.take(event);
        match event {
            Event::Content { view, content } => {
                // Continue's list carries the local watch history. The other views' rows may
                // carry a tracker's progress, or none, so they refresh a known title's details
                // and leave what this device watched alone.
                let local = matches!(view, ContentFor::Continue);
                for entry in content.entries() {
                    let mut entry = with_fixture_trailer(entry.clone());
                    if !local && let Some(known) = self.entries.get(&entry.id.get()) {
                        entry.progress = known.progress;
                        entry.resume = known.resume;
                        entry.last_aired = known.last_aired.or(entry.last_aired);
                    }
                    self.entries.insert(entry.id.get(), entry);
                }
            }
            Event::Detail(entry) => {
                self.entries.insert(entry.id.get(), with_fixture_trailer((**entry).clone()));
            }
            Event::Progress { id, progress, resume, .. } => {
                if let Some(entry) = self.entries.get_mut(&id.get()) {
                    entry.progress = *progress;
                    entry.resume = *resume;
                }
            }
            Event::LastAired(aired) => {
                for title in aired {
                    if let Some(entry) = self.entries.get_mut(&title.id.get()) {
                        entry.last_aired = Some(title.aired);
                    }
                }
            }
            Event::Settings(view) => self.settings = Some(view.clone()),
            Event::Sync(state) => {
                if state.connected {
                    self.signing.remove(&state.tracker);
                }
                self.sync.insert(state.tracker.clone(), (**state).clone());
            }
            Event::Authorize { tracker, .. } => {
                self.signing.insert(tracker.clone(), SignIn::Browser);
            }
            Event::Downloads(rows) => {
                self.downloads = rows.clone();
                self.downloads_said = true;
            }
            Event::Seeding(rows) => self.seeding = rows.clone(),
            Event::Providers(rows) => {
                self.providers = rows.clone();
                self.providers_said = true;
                if !rows.is_empty() {
                    self.provider_note = None;
                }
            }
            Event::ProviderNote(note) => {
                self.provider_note = Some(crate::say::msg(note));
            }
            Event::Stores(rows) => self.stores = rows.clone(),
            Event::SourceOptions { id, options } => {
                self.source_options.insert(id.get(), options.clone());
            }
            Event::Connectivity { offline } => self.offline = *offline,
            Event::Autoplay(view) => self.autoplay = Some(*view),
            Event::DiscussionFound { media, episode, comments } => {
                self.discussions.insert((media.get(), *episode), *comments);
            }
            Event::Update(status) => self.update = Some(status.clone()),
            Event::Vpn(guard) => {
                self.guard = guard.clone().map(|g| (g, std::time::Instant::now()));
            }
            Event::JobStarted { job, activity } => {
                self.jobs.insert(*job, activity.clone());
            }
            Event::PartyForm(form) => self.party_form = Some(form.clone()),
            Event::PartyJoined(view) => {
                // A room joined afresh lists its members afresh.
                let same = self
                    .party
                    .as_ref()
                    .is_some_and(|p| p.server == view.server && p.room == view.room);
                if !same {
                    self.party_members = None;
                }
                self.party = Some(view.clone());
            }
            Event::PartyState(view) => self.party = Some(view.clone()),
            Event::PartyMembers(members) => self.party_members = Some(members.clone()),
            Event::PartyLeft { .. } => {
                self.party = None;
                self.party_members = None;
            }
            Event::JobDone(job) => {
                self.jobs.remove(job);
                self.signing.retain(|_, way| *way != SignIn::Job(Some(*job)));
            }
            _ => {}
        }
    }
}

/// `rows` in the order failover tries them, by `order`.
pub fn sorted(rows: &[ProviderRow], order: &[String]) -> Vec<ProviderRow> {
    let rank = |row: &ProviderRow| {
        order
            .iter()
            .position(|entry| *entry == row.id)
            .or_else(|| {
                (row.kind == "plugin").then(|| order.iter().position(|e| e == "plugins"))?
            })
            .unwrap_or(order.len())
    };
    let mut ranked: Vec<_> =
        rows.iter().enumerate().map(|(at, row)| (rank(row), at, row)).collect();
    ranked.sort_by_key(|(rank, at, _)| (*rank, *at));
    ranked.into_iter().map(|(_, _, row)| row.clone()).collect()
}

type Listener = Rc<dyn Fn(&Event)>;

/// The main loop's side of the engine.
pub struct Hub {
    engine: Weak<Engine>,
    pub known: RefCell<Known>,
    listeners: RefCell<Vec<(u64, Listener)>>,
    next: Cell<u64>,
    art: Art,
    /// Kinds of event the engine's own are set aside for, while a showcase shows its own.
    #[cfg(feature = "fixtures")]
    held: RefCell<Vec<&'static str>>,
    /// The hub itself, for a showcase's answers, which arrive a moment after they are asked.
    #[cfg(feature = "fixtures")]
    me: std::rc::Weak<Hub>,
    /// For a shot: what a protection check answers in place of the engine, `Some(None)` for
    /// never, so the form's checking and failed states hold still.
    #[cfg(feature = "fixtures")]
    pub check_answer: RefCell<Option<Option<GuardView>>>,
    /// For a shot: what the sources have of a title in place of the engine's listing, and
    /// whether it never comes, so the picker's loading state holds still.
    #[cfg(feature = "fixtures")]
    pub listing: RefCell<Option<(Vec<SourceOption>, bool)>>,
    /// For a shot: a title and the episodes of it, first to last, that no source lists.
    #[cfg(feature = "fixtures")]
    pub unsourced: Cell<Option<(u32, u32, u32)>>,
    /// For a shot: a search by hand that fails, or a pick that never comes back.
    #[cfg(feature = "fixtures")]
    pub match_hold: Cell<MatchHold>,
}

/// What a match picker shot holds still in place of the engine's answer.
#[cfg(feature = "fixtures")]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MatchHold {
    #[default]
    Nothing,
    /// A search by hand fails as if the source timed out.
    SearchFails,
    /// A pick is never answered, so its row keeps saying Matching.
    PickWaits,
    /// A search by hand is never answered.
    SearchWaits,
}

impl Hub {
    pub fn new(engine: Weak<Engine>) -> Rc<Self> {
        Rc::new_cyclic(|_me| Self {
            engine,
            known: RefCell::default(),
            listeners: RefCell::default(),
            next: Cell::new(1),
            art: Art::default(),
            #[cfg(feature = "fixtures")]
            held: RefCell::default(),
            #[cfg(feature = "fixtures")]
            me: _me.clone(),
            #[cfg(feature = "fixtures")]
            check_answer: RefCell::default(),
            #[cfg(feature = "fixtures")]
            listing: RefCell::default(),
            #[cfg(feature = "fixtures")]
            unsourced: Cell::default(),
            #[cfg(feature = "fixtures")]
            match_hold: Cell::default(),
        })
    }

    /// The engine, for what talks to it off the main loop: the player's reports.
    pub fn engine_link(&self) -> Weak<Engine> {
        self.engine.clone()
    }

    /// Ask the engine for `command`. Never waits: the engine queues it on its runtime.
    pub fn send(&self, command: Command) {
        self.send_job(command);
    }

    /// [`Hub::send`], and the job it starts when it is work someone waits on, which ends with
    /// `Event::JobDone` of this number.
    pub fn send_job(&self, command: Command) -> Option<JobId> {
        // The showcase's stores are not the engine's: their owners' tools are answered here.
        #[cfg(feature = "fixtures")]
        if self.held.borrow().contains(&"stores")
            && let Some(events) = crate::showcase::answer(&command)
        {
            let me = self.me.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if let Some(hub) = me.upgrade() {
                    hub.take(events);
                }
            });
            return None;
        }
        // A discussion on the fixture engine has no AniList to read: the showcase's stands in.
        #[cfg(feature = "fixtures")]
        if crate::fixtures::active()
            && let Command::OpenDiscussion { media, episode } = &command
        {
            let (me, media, episode) = (self.me.clone(), *media, *episode);
            glib::timeout_add_local_once(std::time::Duration::from_millis(300), move || {
                if let Some(hub) = me.upgrade() {
                    hub.take(vec![Event::Discussion(crate::showcase::discussion(
                        media, episode,
                    ))]);
                }
            });
            return None;
        }
        #[cfg(feature = "fixtures")]
        if let Command::LoadSourceOptions(id) = &command
            && let Some((options, silent)) = self.listing.borrow().clone()
        {
            if !silent {
                let (me, id) = (self.me.clone(), *id);
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        if let Some(hub) = me.upgrade() {
                            hub.take(vec![Event::SourceOptions { id, options }]);
                        }
                    },
                );
            }
            return None;
        }
        #[cfg(feature = "fixtures")]
        match (&command, self.match_hold.get()) {
            (Command::ManualSearch { id, query, .. }, MatchHold::SearchFails) => {
                let (me, id, query) = (self.me.clone(), *id, query.clone());
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        if let Some(hub) = me.upgrade() {
                            hub.take(vec![Event::MatchSearchFailed {
                                id,
                                query,
                                reason: anistream_model::Msg::MatchSearchTimedOut,
                            }]);
                        }
                    },
                );
                return None;
            }
            (Command::ManualSearch { .. }, MatchHold::SearchWaits)
            | (Command::FixMatch { .. }, MatchHold::PickWaits) => return None,
            _ => {}
        }
        #[cfg(feature = "fixtures")]
        if matches!(command, Command::CheckTorrentProtection { .. })
            && let Some(answer) = self.check_answer.borrow().clone()
        {
            if let Some(view) = answer {
                let me = self.me.clone();
                glib::timeout_add_local_once(
                    std::time::Duration::from_millis(300),
                    move || {
                        if let Some(hub) = me.upgrade() {
                            hub.take(vec![Event::ProtectionChecked(view)]);
                        }
                    },
                );
            }
            return None;
        }
        self.engine.upgrade().and_then(|engine| engine.send(command))
    }

    /// Hear every event from now on, until [`Hub::forget`] with the number this returns.
    pub fn listen(&self, listener: impl Fn(&Event) + 'static) -> u64 {
        let id = self.next.get();
        self.next.set(id + 1);
        self.listeners.borrow_mut().push((id, Rc::new(listener)));
        id
    }

    pub fn forget(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(at, _)| *at != id);
    }

    /// A batch of the engine's events, in order.
    pub fn take(self: &Rc<Self>, batch: Vec<Event>) {
        for event in batch {
            #[cfg(feature = "fixtures")]
            let event = self.unsourced_in(event);
            match &event {
                Event::ArtworkReady { url, bytes } => {
                    self.art.arrived(url.clone(), bytes.clone());
                    continue;
                }
                Event::ImageFailed { url, .. } => {
                    self.art.failed(url);
                    continue;
                }
                _ => {}
            }
            #[cfg(feature = "fixtures")]
            if showcase_kind(&event).is_some_and(|kind| self.held.borrow().contains(&kind)) {
                continue;
            }
            self.known.borrow_mut().take(&event);
            // Cloned out, so a listener may listen or forget as it hears.
            let listeners: Vec<_> =
                self.listeners.borrow().iter().map(|(_, l)| l.clone()).collect();
            for listener in listeners {
                guarded("a page listener", || listener(&event)); // i18n-skip
            }
        }
    }

    /// A shot's table as if the sources fell short of it.
    #[cfg(feature = "fixtures")]
    fn unsourced_in(&self, mut event: Event) -> Event {
        if let (Some((title, first, last)), Event::Episodes { id, rows, .. }) =
            (self.unsourced.get(), &mut event)
            && id.get() == title
        {
            for row in rows.iter_mut() {
                row.unsourced =
                    row.number.parse::<u32>().is_ok_and(|n| (first..=last).contains(&n));
            }
        }
        event
    }

    /// Show `events` in place of the engine's own of their kinds, from now on: rows the fixture
    /// engine has none of, for the screens that show them.
    #[cfg(feature = "fixtures")]
    pub fn show_instead(self: &Rc<Self>, events: Vec<Event>) {
        let kinds: Vec<_> = events.iter().filter_map(showcase_kind).collect();
        self.held.borrow_mut().retain(|kind| !kinds.contains(kind));
        self.take(events);
        self.held.borrow_mut().extend(kinds);
    }

    /// The title as it last stood, when it has been seen.
    pub fn entry(&self, id: AnilistId) -> Option<Entry> {
        self.known.borrow().entries.get(&id.get()).cloned()
    }

    /// Show `url`'s art in `picture` once it is here, fetching it once.
    pub fn art(&self, picture: &gtk::Picture, url: Option<&str>) {
        let Some(url) = url.filter(|u| !u.is_empty()) else { return };
        if self.art.bind(url, picture) {
            self.send(Command::FetchArtwork { url: url.to_owned() });
        }
    }
}

/// Decoded art by address, and the pictures waiting for it.
#[derive(Default)]
struct Art {
    ready: Rc<RefCell<HashMap<String, gdk::Texture>>>,
    waiting: RefCell<HashMap<String, Vec<glib::WeakRef<gtk::Picture>>>>,
}

impl Art {
    /// Show `url` in `picture`: now when it is decoded, or when it arrives. True when it has to
    /// be fetched.
    fn bind(&self, url: &str, picture: &gtk::Picture) -> bool {
        if let Some(texture) = self.ready.borrow().get(url) {
            picture.set_paintable(Some(texture));
            picture.set_opacity(1.0);
            return false;
        }
        let mut waiting = self.waiting.borrow_mut();
        let first = !waiting.contains_key(url);
        waiting.entry(url.to_owned()).or_default().push(picture.downgrade());
        first
    }

    fn arrived(&self, url: String, bytes: Vec<u8>) {
        let waiting = self.waiting.borrow_mut().remove(&url).unwrap_or_default();
        let ready = Rc::clone(&self.ready);
        glib::spawn_future_local(async move {
            let decoded = gio::spawn_blocking(move || decode(&bytes)).await.ok().flatten();
            let Some((width, height, rgba)) = decoded else { return };
            let texture: gdk::Texture = gdk::MemoryTexture::new(
                width as i32,
                height as i32,
                gdk::MemoryFormat::R8g8b8a8,
                &glib::Bytes::from_owned(rgba),
                width as usize * 4,
            )
            .upcast();
            // Art that arrives fades in over its placeholder; it never pops.
            for picture in waiting.iter().filter_map(glib::WeakRef::upgrade) {
                picture.set_paintable(Some(&texture));
                crate::motion::fade_in(&picture);
            }
            ready.borrow_mut().insert(url, texture);
        });
    }

    fn failed(&self, url: &str) {
        // Tried again the next time a screen shows it.
        self.waiting.borrow_mut().remove(url);
    }
}

/// An image's pixels, RGBA, off the main loop.
fn decode(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let image = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = image.dimensions();
    Some((w, h, image.into_raw()))
}

/// The kind of event a showcase sets the engine's own aside by.
#[cfg(feature = "fixtures")]
fn showcase_kind(event: &Event) -> Option<&'static str> {
    Some(match event {
        Event::Providers(_) => "providers",
        Event::Stores(_) => "stores",
        Event::Seeding(_) => "seeding",
        Event::StorePreview(_) => "store-preview",
        _ => return None,
    })
}

/// The fixture scene's titles carry no trailer of their own: most are given a made-up one, so the
/// banner and the preview have a video to play, and one in five none, so the state shows too.
#[cfg(feature = "fixtures")]
fn with_fixture_trailer(mut entry: Entry) -> Entry {
    use anistream_model::catalog::{Trailer, TrailerHost};
    if entry.trailer.is_none() && entry.id.get() % 5 != 4 {
        entry.trailer = Some(Trailer { host: TrailerHost::YouTube, id: "fixtureclip1".into() });
        entry.trailer_url = Some("https://video.example/watch?v=fixtureclip1".into());
    }
    entry
}

thread_local! {
    /// Whether a panic here would be caught: the panic hook leaves the engine running then.
    static GUARDED: Cell<u32> = const { Cell::new(0) };
}

/// Whether the code running now is inside [`guarded`].
pub fn in_guard() -> bool {
    GUARDED.with(|depth| depth.get() > 0)
}

/// Run `work`, and when it panics log that and go on: one page's bug in a callback from GTK's
/// main loop would otherwise abort the whole app, mid-episode.
pub fn guarded(what: &str, work: impl FnOnce()) -> bool {
    GUARDED.with(|depth| depth.set(depth.get() + 1));
    let done = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)).is_ok();
    GUARDED.with(|depth| depth.set(depth.get() - 1));
    if !done {
        tracing::error!(what, "a callback panicked and was skipped");
    }
    done
}

#[cfg(not(feature = "fixtures"))]
fn with_fixture_trailer(entry: Entry) -> Entry {
    entry
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::catalog::{ContentList, ResumePoint};
    use anistream_model::progress::TitleProgress;

    #[test]
    fn a_panic_in_one_listener_is_caught_and_the_next_one_still_runs() {
        let mut heard = Vec::new();
        assert!(!guarded("a test", || panic!("a page broke")));
        assert!(!in_guard(), "the guard is left");
        assert!(guarded("a test", || heard.push(2)));
        assert_eq!(heard, [2]);
    }

    #[test]
    fn progress_moves_the_title_wherever_it_was_seen() {
        let mut known = Known::default();
        let id = AnilistId::new(7);
        known.take(&Event::Content {
            view: ContentFor::Continue,
            content: ContentList::Entries(vec![Entry::new(id, "A title")]),
        });
        known.take(&Event::Progress {
            id,
            progress: Some(TitleProgress { done: 3, next: 4 }),
            resume: Some(ResumePoint { position: 60.0, fraction: None }),
            episodes: Vec::new(),
        });
        let entry = &known.entries[&7];
        assert_eq!(entry.progress, Some(TitleProgress { done: 3, next: 4 }));
        assert_eq!(entry.resume.map(|r| r.position), Some(60.0));
    }

    #[test]
    fn the_library_list_does_not_move_continue_back_to_episode_one() {
        use anistream_model::catalog::LibrarySegment;
        let mut known = Known::default();
        let id = AnilistId::new(900_001);
        let mut local = Entry::new(id, "A title");
        local.episodes = Some(12);
        local.progress = Some(TitleProgress { done: 6, next: 7 });
        local.resume = Some(ResumePoint { position: 606.0, fraction: Some(0.42) });
        known.take(&Event::Content {
            view: ContentFor::Continue,
            content: ContentList::Entries(vec![local.clone()]),
        });
        let mut tracked = local.clone();
        tracked.progress = Some(TitleProgress { done: 12, next: 1 });
        known.take(&Event::Content {
            view: ContentFor::Library(LibrarySegment::Completed),
            content: ContentList::Entries(vec![tracked]),
        });
        let queue =
            crate::say::queue(&[known.entries[&900_001].clone()], &[], &Default::default());
        assert_eq!(
            queue.in_progress[0].episode, 7,
            "Continue says: {}",
            queue.in_progress[0].action
        );
    }

    #[test]
    fn a_view_without_air_dates_keeps_what_the_title_already_aired() {
        use anistream_model::catalog::Aired;
        let mut known = Known::default();
        let id = AnilistId::new(5);
        let mut local = Entry::new(id, "A title");
        local.last_aired = Some(Aired { episode: 3, ago: 600 });
        known.take(&Event::Content {
            view: ContentFor::Continue,
            content: ContentList::Entries(vec![local]),
        });
        known.take(&Event::Content {
            view: ContentFor::Calendar,
            content: ContentList::Entries(vec![Entry::new(id, "A title")]),
        });
        assert_eq!(known.entries[&5].last_aired.map(|a| a.episode), Some(3));
    }

    #[test]
    fn a_title_first_seen_in_another_view_is_kept_as_it_came() {
        let mut known = Known::default();
        let id = AnilistId::new(6);
        let mut tracked = Entry::new(id, "A title");
        tracked.progress = Some(TitleProgress { done: 2, next: 3 });
        known.take(&Event::Content {
            view: ContentFor::Calendar,
            content: ContentList::Entries(vec![tracked]),
        });
        assert_eq!(known.entries[&6].progress.map(|p| p.next), Some(3));
    }

    #[test]
    fn the_reason_there_are_no_sources_stands_until_there_are_some() {
        let mut known = Known::default();
        known.take(&Event::ProviderNote(anistream_model::Msg::PluginsLoadFailed {
            reason: "a folder".into(),
        }));
        assert!(known.provider_note.as_deref().is_some_and(|note| note.contains("a folder")));
        known.take(&Event::Providers(Vec::new()));
        assert!(known.provider_note.is_some(), "an empty list keeps it");
    }

    #[test]
    fn a_sign_in_waits_through_the_browser_and_ends_with_its_job_or_the_account() {
        let mut known = Known::default();
        let connect = JobId::new(1);
        known.signing.insert("mal".into(), SignIn::Job(Some(connect)));
        known.take(&Event::Authorize {
            tracker: "mal".into(),
            url: "https://tracker.example/authorize".into(),
            callback_scheme: "anistream".into(),
        });
        // The job that opened the browser ends; the sign-in goes on.
        known.take(&Event::JobDone(connect));
        assert_eq!(known.signing.get("mal"), Some(&SignIn::Browser));
        let finish = JobId::new(2);
        known.signing.insert("mal".into(), SignIn::Job(Some(finish)));
        known.take(&Event::JobDone(finish));
        assert!(known.signing.is_empty(), "a finished redirect ends it");

        known.signing.insert("simkl".into(), SignIn::Job(Some(JobId::new(3))));
        let mut state = anistream_model::sync::SyncState::new("simkl");
        state.connected = true;
        known.take(&Event::Sync(Box::new(state)));
        assert!(known.signing.is_empty(), "signed in ends it");
    }

    #[test]
    fn a_choice_changes_the_listing_and_a_pin_that_cannot_be_used_is_told_once() {
        let mut known = Known::default();
        let id = AnilistId::new(7);
        let option = |provider: &str| SourceOption {
            provider_id: provider.into(),
            name: provider.into(),
            covered: 6,
            of: 13,
            pinned: provider == "a",
            note: None,
            releases: Vec::new(),
            pinned_release: None,
        };
        known.take(&Event::SourceOptions { id, options: vec![option("a"), option("b")] });
        known.choose(id, Some("b"), None);
        let listed = &known.source_options[&7];
        assert!(!listed[0].pinned && listed[1].pinned);

        assert!(known.say_pin(id, "a"));
        assert!(!known.say_pin(id, "a"), "told once");
        known.choose(id, None, None);
        assert!(known.say_pin(id, "a"), "a new choice may meet it again");
        assert!(!known.say_pin(id, "a"));
        known.visit(id);
        assert!(known.say_pin(id, "a"), "a new visit tells it again");
    }

    #[test]
    fn closing_counts_what_downloads_and_what_seeds() {
        let row = |state| DownloadRow {
            id: 1,
            anilist_id: AnilistId::new(1),
            title: "A title".into(),
            episode: "1".into(),
            state,
            fraction: 0.0,
            downloaded: 0,
            total: 0,
            error: None,
            path: None,
            ahead: false,
        };
        let mut known = Known::default();
        known.take(&Event::Downloads(vec![
            row(DownloadState::Active),
            row(DownloadState::Queued),
            row(DownloadState::Done),
            row(DownloadState::Paused),
        ]));
        assert_eq!(known.running(), (2, 0));
    }
}
