//! Events: everything the work tells a front end, as it happens.

use std::time::Duration;

use anistream_core::{ids::AnilistId, traits::SourceCandidate};

use crate::{
    catalog::{ContentFor, ContentList, Entry, ResumePoint, TitleAired, TitleLanguage},
    downloads::{DownloadRow, SeedRow},
    episodes::{DubExtent, EpisodeRow},
    hosted::{PlayerOp, PlayerStats, PlayerUi},
    jobs::JobId,
    notice::Notice,
    notifications::{NewEpisode, UpcomingNotices},
    party::{NoSourceWhy, PartyForm, PartyMember, PartyMessage, PartyView},
    playback::{Ending, PlaybackId, SkipKind, TimeRange, TimelineMark},
    progress::{EpisodeProgress, TitleProgress},
    remote::FrontIntent,
    search::{BrowseRail, SearchFacets, SearchQuery},
    settings::SettingsView,
    show::ShowSettingsView,
    sources::{GuardView, MatchCandidate, MetaSourceRow, ProviderRow},
    stores::{
        InviteLinkView, InvitePreview, LocalPluginRow, OpenJoinView, StoreInviteRow,
        StoreMemberRow, StoreRow,
    },
    sync::{ConflictRow, DeviceCodePrompt, SyncState},
    updates::UpdateStatus,
    widgets::WidgetData,
};

/// A computed change, applied on the UI thread.
///
/// Async work never touches the interface's `App` directly; it sends one of these instead. That
/// is what guarantees the render loop is never blocked on I/O.
#[derive(Debug)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Event {
    /// Work someone waits on has started, as a job named for the activity row. Sent ahead of
    /// anything else the work sends, so its notes and its end always find it.
    ///
    /// Jobs are numbered where the work runs, once each per run, so a front end never has to.
    JobStarted {
        job: JobId,
        activity: String,
    },
    /// What a running job is held up on, and for how long from now, when that is known.
    ///
    /// A span rather than a moment: a moment is read off a clock, and the one that counts the
    /// wait down is the front end's.
    JobNote {
        job: JobId,
        note: String,
        wait: Option<Duration>,
    },
    /// The work carrying this job has ended, whether it succeeded or not.
    JobDone(JobId),
    /// A list, tagged with the view that asked for it. See [`ContentFor`].
    Content {
        view: ContentFor,
        content: ContentList,
    },
    /// What a list is, said above it: which season Seasonal is, and that a list stopped at the
    /// size of one page when there was more. Tagged like [`Event::Content`] and sent before it,
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
    /// Whether the search arriving has more pages: sent with each first page, before its
    /// [`Event::Content`], so the screen knows to ask for the next as the end comes near.
    SearchPaging {
        query: SearchQuery,
        has_next: bool,
    },
    /// A further page of a search's results, to go after what is on screen, in answer to
    /// [`Command::SearchMore`](crate::command::Command::SearchMore). Taken only while `query`
    /// is the search on screen. `entries` is empty when the page failed, and the same page is
    /// asked for again on the next scroll.
    SearchMore {
        query: SearchQuery,
        page: u32,
        entries: Vec<Entry>,
        has_next: bool,
    },
    /// Whether live search is waiting for Return: `true` once the AniList budget runs low and
    /// a change of text was not searched, `false` once one is again. Filter changes and Return
    /// always search.
    LiveSearch {
        paused: bool,
    },
    /// Search's start: the rails, each a preset and its titles. Sent from what was kept, then
    /// again when AniList answers.
    Browse(Vec<BrowseRail>),
    /// What the filters choose from.
    Facets(SearchFacets),
    /// The recent searches, newest first: at launch when asked, and after each change.
    RecentSearches(Vec<SearchQuery>),
    Detail(Box<Entry>),
    /// The title language the signed-in AniList viewer reads in, in answer to
    /// [`Command::LoadTitleLanguage`](crate::command::Command::LoadTitleLanguage). `None` when
    /// nobody is signed in to AniList, or AniList did not say.
    ViewerTitleLanguage(Option<TitleLanguage>),
    /// The form every title is named in changed, to this: the setting was changed, or the
    /// signed-in AniList viewer it follows signed in or out. What a front end has on screen
    /// names titles the old way, so it loads what it shows again; what arrives from now on is
    /// named the new way.
    TitlesChanged(TitleLanguage),
    Status(String),
    Toast(Notice),
    Providers(Vec<ProviderRow>),
    /// The metadata ladder's condition: which rung is serving, and why.
    MetaStatus(Vec<MetaSourceRow>),
    /// The config, as a front end shows it, whenever any of it changed: a setting written or
    /// refused, a session's choice, or what the player changed on its own.
    Settings(SettingsView),
    /// One show's settings sheet: asked for, or changed.
    ShowSettings(ShowSettingsView),
    /// The sources are being rebuilt, because a setting changed which ones should exist. Sent
    /// once the torrent stack is stood down and the chain swapped, ahead of what the rebuild
    /// reports: whatever reloads them has done so, and the screen only has to say it.
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
    /// The sources that could serve a title, each with how much of the title's episodes it has,
    /// in the order they are tried, for a source picker: the answer to
    /// [`Command::LoadSourceOptions`](crate::command::Command::LoadSourceOptions).
    SourceOptions {
        id: AnilistId,
        options: Vec<crate::sources::SourceOption>,
    },
    /// The user changed the player volume.
    PlaybackVolume(f64),
    /// Which provider the current stream actually came from.
    ActiveProvider(String),
    /// The player asked for an episode of what is playing: its Episodes menu, Up Next's
    /// "play now", N/P inside the window, or the terminal's n/N. `title` is the episode's name
    /// from the episode list the playback holds, which is that title's whatever is open here.
    ///
    /// The engine starts it, of the title Now Playing shows, and says so with the
    /// [`Event::PlaybackStaged`] that follows this: there is nothing here for a front end to do.
    PlayerPlayEpisode {
        episode: String,
        title: Option<String>,
    },
    /// The playback just staged found nothing to play, and the one it was to replace keeps
    /// playing: Now Playing goes back to that one.
    PlaybackKept,
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
    /// The engine puts it in effect for the session, restarts the episode Now Playing shows and
    /// says so, with a notice and the [`Event::PlaybackStaged`] that follow this.
    PlayerTranslation(anistream_core::media::Translation),
    /// An upscaling mode picked in the player, by its config name.
    PlayerUpscaling(String),
    /// Night mode toggled in the player.
    PlayerNightMode(bool),
    /// An HDR tone-mapping curve picked in the player.
    PlayerToneMapping(String),
    /// The episode playing has reached its ending, and the engine is finding the next one's
    /// streams now, so Up Next can start it without a search.
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
    /// A skip is available at the current position: past what, and where it would seek to.
    SkipAvailable {
        kind: SkipKind,
        to: f64,
    },
    SkipCleared,
    PlaybackSpeed(f64),
    /// A playback is over. `watched` is the tracker's verdict, for the meters; `ending` is how
    /// it came to an end, which is what decides whether anything plays next.
    ///
    /// `rolling_on` is whether it did: the engine has started the next episode in the same
    /// window, and stages it with the [`Event::PlaybackStaged`] that follows this, so Now
    /// Playing stays up for it rather than coming down. Only an ending of the playback Now
    /// Playing shows rolls on, when mpv played it to its end and the playback kept its window
    /// for the next. A playback cannot know this of itself, and says `false`
    /// ([`Event::playback_ended`]); the engine says it on the way.
    PlaybackEnded {
        watched: bool,
        ending: Ending,
        rolling_on: bool,
    },
    /// A playback the engine has started, as Now Playing shows it while its stream is found:
    /// whose it is, what it is called, which episode and the episode's own name, and the speed
    /// it starts at. Said for every play Now Playing shows, the ones a front end asked for and
    /// the ones the engine starts on its own: the next episode an ending rolls on to, an episode
    /// picked in the player, and an episode restarted in the other translation.
    ///
    /// Applied idempotently: a front end that staged this playback itself, as it asked for it,
    /// or was told it once already, changes nothing. A watch party is not staged, and is not
    /// said: the party's player is not the app's.
    PlaybackStaged {
        id: AnilistId,
        title: String,
        episode: String,
        episode_title: Option<String>,
        speed: f64,
    },
    /// Whether the device has a network, as the front end's monitor last said with
    /// [`Command::NetworkPath`](crate::command::Command::NetworkPath): said each time that
    /// changes, and never before the first change, the engine starting online. While
    /// `offline`, a screen shows what it has with the offline line, and one with nothing to
    /// show says it is offline. Back online, the engine asks again for what failed meanwhile,
    /// once, and the screens take the answers as they come.
    Connectivity {
        offline: bool,
    },
    /// Playback started from a stored position rather than the beginning.
    Resumed {
        position: f64,
    },
    /// What the player a front end hosts is to do for `playback`, in order: see
    /// [`crate::hosted`]. Only for a front end that hosts the player.
    PlayerOps {
        playback: PlaybackId,
        ops: Vec<PlayerOp>,
    },
    /// What a hosted player's own interface is to show for `playback`: see [`crate::hosted`].
    PlayerUi {
        playback: PlaybackId,
        ui: PlayerUi,
    },
    /// The openings and endings the timeline of `playback` marks, whole, replacing what was
    /// said before: the times the skip button and auto-skip use, from the file's own chapters
    /// when they name them and otherwise from community times. Said when the file starts and
    /// again when they change, as when the file's chapters or its length arrive. Empty when
    /// nothing is known, and the timeline marks nothing.
    PlayerTimeline {
        playback: PlaybackId,
        marks: Vec<TimelineMark>,
    },
    /// What the player of `playback` has buffered ahead and behind, whole, replacing what was
    /// said before, in seconds of the file: drawn on the timeline under the played part. At
    /// most twice a second, only when it moved by a second or more, and never while paused.
    /// Read from mpv's `demuxer-cache-state`, which a hosted player reports among its
    /// properties.
    PlayerBuffered {
        playback: PlaybackId,
        ranges: Vec<TimeRange>,
    },
    /// Stats for nerds for the player of `playback`: the panel's rows once a second while it is
    /// open, and the TV's summary lines whenever they change. An event of its own rather than a
    /// `PlayerUi`, so an app that does not show it yet passes over it.
    PlayerStats {
        playback: PlaybackId,
        stats: PlayerStats,
    },
    /// Text the front end puts on the clipboard: what Copy stats copies. The app says "Copied".
    Clipboard {
        text: String,
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
    /// How far title `id`'s dub goes, as the source a dubbed play would use lists it: the
    /// answer to [`Command::LoadDub`]. `None` when that source has no dub of it, and when it
    /// could not say; either way the page shows nothing. [`present::dub_line`] words it.
    ///
    /// [`Command::LoadDub`]: crate::command::Command::LoadDub
    /// [`present::dub_line`]: crate::present::dub_line
    TitleDub {
        id: AnilistId,
        dub: Option<DubExtent>,
    },
    /// Where a title stands now, whole, after something moved it: a mark, the playhead of an
    /// episode playing, or how one ended. Sent ahead of the event that moved it, when an event
    /// did, so that whatever that event brings up shows the title as it now stands.
    ///
    /// `progress` and `resume` are what every entry of the title says, on its page and in any
    /// list; `episodes` is each episode's watched state, from the title's table as it was last
    /// loaded, whether or not another title's has loaded since, and empty when none of it was.
    /// A table is only ever loaded with [`Event::Episodes`]: this says how far through it the
    /// viewer is, never which episodes there are.
    Progress {
        id: AnilistId,
        progress: Option<TitleProgress>,
        resume: Option<ResumePoint>,
        episodes: Vec<EpisodeProgress>,
    },
    /// The VPN guard's verdict changed, or `None`: there is no guard any more, because
    /// torrents were switched off.
    Vpn(Option<GuardView>),
    /// The verdict on settings [`Command::CheckTorrentProtection`] asked about, none of them
    /// saved: `Protected` when they may be saved, anything else when they should not.
    ///
    /// [`Command::CheckTorrentProtection`]: crate::command::Command::CheckTorrentProtection
    ProtectionChecked(GuardView),
    /// A cover or banner as it was fetched or read from the cache, not yet decoded.
    ///
    /// Bytes rather than pixels: what a cover becomes on screen is the front end's to decide.
    /// The reducer leaves them alone. Decoding takes long enough to stutter a frame, so whoever
    /// runs the app decodes them off its loop and hands the image over, in the terminal with
    /// `App::artwork_decoded`.
    ArtworkReady {
        url: String,
        bytes: Vec<u8>,
    },
    /// A cover could not be fetched. Silent: the plate stays, and the URL is tried again
    /// later. `offline` means the server was not reached at all.
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
    /// Sign in to `tracker` through the system's sign-in sheet: open `url` in it, and hand back
    /// the redirect it ends on, one with `callback_scheme` for its scheme, with
    /// [`Command::FinishSignIn`], or say it was closed with [`Command::CancelSignIn`]. What a
    /// native app's own sign-in clients start; the terminal's sign-in catches its redirect on
    /// a loopback port instead.
    ///
    /// [`Command::FinishSignIn`]: crate::command::Command::FinishSignIn
    /// [`Command::CancelSignIn`]: crate::command::Command::CancelSignIn
    Authorize {
        tracker: String,
        url: String,
        callback_scheme: String,
    },
    /// Latest broadcast per title, merged into whatever is on screen.
    ///
    /// Arrives *after* the list it annotates, because it takes a second request that depends on
    /// which titles came back. The list renders immediately without it and gains the line when
    /// it lands, rather than the whole screen waiting on an embellishment.
    LastAired(Vec<TitleAired>),
    /// Every joined plugin store, replacing whatever was there.
    Stores(Vec<StoreRow>),
    /// What a pasted invite link leads to, or why it leads nowhere.
    StorePreview(InvitePreview),
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
    /// What [`Command::CheckNewEpisodes`] found: the episodes to notify of, in the order they
    /// aired, each said once and never again. Empty when there is nothing new.
    ///
    /// [`Command::CheckNewEpisodes`]: crate::command::Command::CheckNewEpisodes
    NewEpisodes(Vec<NewEpisode>),
    /// The home-screen widgets' record, in answer to
    /// [`Command::WidgetSnapshot`](crate::command::Command::WidgetSnapshot): for the app to
    /// write where its widgets read it.
    WidgetSnapshot(WidgetData),
    /// The week's broadcasts that will notify and those left out, in answer to
    /// [`Command::UpcomingNotices`](crate::command::Command::UpcomingNotices): what an app
    /// schedules ahead, and its Notifications pane's "Coming up".
    UpcomingNotices(UpcomingNotices),
    /// Who can join a store, its open link, and the devices that joined with it, for its owner.
    /// Follows a store's members, and answers every change to who can join. Never sent for a
    /// store whose server has no open joining.
    OpenJoin {
        store: String,
        view: OpenJoinView,
    },
    /// Where the update check stands: a newer release and whether to tell of it now, or that
    /// there is none. Sent at launch by the daily check when it is switched on, from what it
    /// kept within the day or a request past it, and for each
    /// [`Command::CheckForUpdates`](crate::command::Command::CheckForUpdates). The daily check
    /// says nothing when it fails.
    ///
    /// The terminal app tells of a release with a notice of its own, which names
    /// `anistream --update`; an app tells of it when `notify` says to.
    Update(UpdateStatus),
    /// What a phone's remote asks of a TV's front end: a title to open, text for the focused
    /// field, a key, a volume step, or a pairing code to show or hide. Sent only on a TV that
    /// started its remote, and never by the engine itself, which has no screen to act on.
    Remote(FrontIntent),
    /// What the form that starts a party opens with, for
    /// [`Command::LoadPartyForm`](crate::command::Command::LoadPartyForm).
    PartyForm(PartyForm),
    /// In a party's room: the form becomes the party panel. Followed by
    /// [`Event::PartyMembers`], and [`Event::PartyState`] as the party changes.
    PartyJoined(PartyView),
    /// The party as it stands, after any change: ready, the episode and its invite link, the
    /// connection lost and made again, the round trip.
    PartyState(PartyView),
    /// Who is in the room, this device's member first.
    PartyMembers(Vec<PartyMember>),
    /// What the room said: someone joining, leaving, pausing, resuming or jumping, a
    /// correction to stay with the room, everyone ready, or chat. Already shown on the player
    /// as a notice; this is for a party panel.
    PartyMessage(PartyMessage),
    /// The party is over: left on purpose with `None`, or for the reason given, worded for the
    /// user ("party.example:8999 refused the party: …"). What is playing goes on as a private
    /// session.
    PartyLeft {
        reason: Option<String>,
    },
    /// The party's episode has no source here: this device is in the room without playing,
    /// for the dialog that says why and offers what fits, Choose source among it. It carries
    /// the lookup's own words, and no [`Event::Toast`] says them a second time.
    PartyNoSource {
        id: AnilistId,
        episode: String,
        why: NoSourceWhy,
        reason: String,
    },
}

impl Event {
    /// A playback's ending as the playback says it: whether it rolls on is not its to know,
    /// and the engine says it on the way. See [`Event::PlaybackEnded`].
    pub fn playback_ended(watched: bool, ending: Ending) -> Self {
        Self::PlaybackEnded { watched, ending, rolling_on: false }
    }
}
