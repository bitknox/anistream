//! Events: everything the work tells a front end, as it happens.

use std::time::Duration;

use anistream_core::{ids::AnilistId, traits::SourceCandidate};

use crate::{
    catalog::{ContentFor, ContentList, Entry, ResumePoint, TitleAired, TitleLanguage},
    downloads::{DownloadRow, SeedRow},
    episodes::EpisodeRow,
    hosted::{PlayerOp, PlayerUi},
    jobs::JobId,
    notice::Notice,
    playback::{Ending, PlaybackId, SkipKind},
    progress::{EpisodeProgress, TitleProgress},
    settings::SettingsView,
    show::ShowSettingsView,
    sources::{GuardView, MatchCandidate, MetaSourceRow, ProviderRow},
    stores::{
        InviteLinkView, InvitePreview, LocalPluginRow, StoreInviteRow, StoreMemberRow, StoreRow,
    },
    sync::{ConflictRow, DeviceCodePrompt, SyncState},
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
    Detail(Box<Entry>),
    /// The title language the signed-in AniList viewer reads in, in answer to
    /// [`Command::LoadTitleLanguage`](crate::command::Command::LoadTitleLanguage). `None` when
    /// nobody is signed in to AniList, or AniList did not say.
    ViewerTitleLanguage(Option<TitleLanguage>),
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
    /// Where a title stands now, whole, after something moved it: a mark, the playhead of an
    /// episode playing, or how one ended. Sent ahead of the event that moved it, when an event
    /// did, so that whatever that event brings up shows the title as it now stands.
    ///
    /// `progress` and `resume` are what every entry of the title says, on its page and in any
    /// list; `episodes` is each episode's watched state, from the episode table loaded last when
    /// it is this title's, and otherwise empty. A table is only ever loaded with
    /// [`Event::Episodes`]: this says how far through it the viewer is, never which episodes
    /// there are.
    Progress {
        id: AnilistId,
        progress: Option<TitleProgress>,
        resume: Option<ResumePoint>,
        episodes: Vec<EpisodeProgress>,
    },
    /// The VPN guard's verdict changed, or `None`: there is no guard any more, because
    /// torrents were switched off.
    Vpn(Option<GuardView>),
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
}

impl Event {
    /// A playback's ending as the playback says it: whether it rolls on is not its to know,
    /// and the engine says it on the way. See [`Event::PlaybackEnded`].
    pub fn playback_ended(watched: bool, ending: Ending) -> Self {
        Self::PlaybackEnded { watched, ending, rolling_on: false }
    }
}
