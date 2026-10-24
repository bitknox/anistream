//! Commands: the work a front end asks for, and the controls it sends to whatever is playing.

use anistream_core::ids::AnilistId;

use crate::{
    catalog::LibrarySegment,
    episodes::EpisodeRow,
    hosted::{PlayerChoice, PlayerReport},
    playback::{PlaybackId, Successor},
    protection::{FailedCheck, TorrentProtection, TorrentProxy},
    search::SearchQuery,
    settings::SettingKey,
    show::ShowSettingKey,
    stores::Secret,
};

impl Command {
    /// Play `episode` of `id` from where it was left, or the start when it was not: see
    /// [`Command::Play`].
    pub fn play(id: AnilistId, episode: impl Into<String>) -> Self {
        Self::Play { id, episode: episode.into(), from_start: false }
    }

    /// What to call this in the activity row, or `None` for work nobody waits on: writes that
    /// land in the outbox, player commands, and anything that finishes as it is asked.
    pub fn activity(&self) -> Option<String> {
        Some(match self {
            Self::LoadContinue => "loading continue watching".into(),
            Self::LoadSeasonal => "loading this season".into(),
            Self::LoadCalendar => "loading the calendar".into(),
            Self::LoadCalendarPast { .. } => "loading earlier broadcasts".into(),
            // A live search has no job: it is sent on every keystroke, and most are replaced
            // before they ask anything.
            Self::Search { query } | Self::SearchMore { query, .. } if query.is_browse() => {
                "browsing".into()
            }
            Self::Search { query } | Self::SearchMore { query, .. } => {
                format!("searching \u{201c}{}\u{201d}", query.trimmed_text())
            }
            Self::LoadBrowse => "loading trending titles".into(),
            Self::LoadDetail(_) => "loading the title".into(),
            Self::LoadEpisodes(_) => "loading episodes".into(),
            Self::ClearMatch { .. } | Self::FixMatch { .. } => "rematching sources".into(),
            Self::CheckProviders => "checking providers".into(),
            Self::LoadLibrary(segment) => format!("loading your {} list", segment.label()),
            Self::SyncNow => "syncing".into(),
            Self::Connect { tracker } | Self::FinishSignIn { tracker, .. } => {
                format!("signing in to {tracker}")
            }
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
                format!("queueing {}", crate::wording::plural(episodes.len(), "episode"))
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
            Self::SetOpenJoin { open: true, .. } => "opening the store to its link".into(),
            Self::SetOpenJoin { open: false, .. } => "limiting the store to invites".into(),
            Self::RemoveAnonymous { .. } => "removing devices".into(),
            _ => return None,
        })
    }
}

/// Asynchronous work requested by the reducer.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Command {
    /// Resolve an episode's streams ahead of playing it, quietly, for a [`Command::Play`] of the
    /// same episode to pick up.
    ///
    /// The next episode the playing one rolls on to is found ahead by the engine itself, when
    /// the playback says its ending has begun ([`Event::PrefetchNext`]): a front end never has
    /// to ask for that one.
    ///
    /// [`Event::PrefetchNext`]: crate::event::Event::PrefetchNext
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
    /// Search now: Return in the field, a recent search, a rail's See All. Always runs, even
    /// when the AniList budget is low. Answered with [`Event::SearchPaging`] and then the
    /// [`Event::Content`] for [`ContentFor::Search`], and remembered as a recent search.
    ///
    /// [`Event::SearchPaging`]: crate::event::Event::SearchPaging
    /// [`Event::Content`]: crate::event::Event::Content
    /// [`ContentFor::Search`]: crate::catalog::ContentFor::Search
    Search {
        query: SearchQuery,
    },
    /// Search as the query changes: sent on every keystroke and every filter change. The engine
    /// waits until the changes stop (400 ms after the text changed, 250 ms after a filter did)
    /// and searches once, and a newer one replaces any still waiting. Text shorter than two
    /// characters (one for Chinese, Japanese and Korean) is not searched yet. While the AniList
    /// budget is low, text changes wait for a [`Command::Search`], and
    /// [`Event::LiveSearch`](crate::event::Event::LiveSearch) says so.
    SearchLive {
        query: SearchQuery,
    },
    /// The next page of the results on screen, asked for as the end comes near. Answered with
    /// [`Event::SearchMore`](crate::event::Event::SearchMore).
    SearchMore {
        query: SearchQuery,
        page: u32,
    },
    /// Search's start, as rails, in one request: answered with
    /// [`Event::Browse`](crate::event::Event::Browse), at once from what was kept and again once
    /// AniList answers.
    LoadBrowse,
    /// What the filters choose from: answered with
    /// [`Event::Facets`](crate::event::Event::Facets). Asked of AniList at most once a week.
    LoadSearchFacets,
    /// The recent searches, answered with
    /// [`Event::RecentSearches`](crate::event::Event::RecentSearches).
    LoadRecentSearches,
    /// Keep a search among the recent ones, as when a title is opened from a live search's
    /// results. A [`Command::Search`] is kept by itself.
    RememberSearch(SearchQuery),
    ForgetRecentSearch(SearchQuery),
    ClearRecentSearches,
    LoadDetail(AnilistId),
    /// Ask the signed-in AniList viewer's title language, answered by
    /// [`Event::ViewerTitleLanguage`](crate::event::Event::ViewerTitleLanguage). Costs no request
    /// when the viewer was already looked up for their library.
    LoadTitleLanguage,
    LoadEpisodes(AnilistId),
    /// How far a title's dub goes, for the title page's "Dub to episode 6": answered by one
    /// [`Event::TitleDub`](crate::event::Event::TitleDub), with no job. Sent when the page
    /// opens, beside [`Command::LoadEpisodes`], and again when it comes back into view.
    ///
    /// Never in the way of the page: it waits for the title's episode table to load when one is
    /// loading, so the two never ask the source at once, and a play, or the page of another
    /// title asking, stops it. It lists the title in the dub from the source a dubbed play would
    /// use, the show's own pick where it has one, within 30 seconds, and the answer is kept for
    /// the title and that source: six hours for a show still airing, a day for any other, and a
    /// quarter of an hour for a source that could not say. Asked again within that, it answers
    /// from what was kept and asks nothing; asked while offline, it answers from what was kept
    /// or with `None`, and asks again once back.
    LoadDub(AnilistId),
    CheckProviders,
    /// Check where torrent traffic leaves from now, as Sources' Check Now asks, rather than at
    /// the next of the guard's own checks. Answered with
    /// [`Event::Vpn`](crate::event::Event::Vpn): `Checking`, then the verdict, or `None` when
    /// torrents are off. A failed check does what any of the guard's does.
    CheckConnection,
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
    /// Run a tracker's sign-in flow. For a device-code tracker, sent again while its code is up
    /// it asks for a new code, which replaces the one before: what "Get a new code" sends, and a
    /// TV once the code's [`DeviceCodePrompt::expires_at`] passes.
    ///
    /// [`DeviceCodePrompt::expires_at`]: crate::sync::DeviceCodePrompt::expires_at
    Connect {
        tracker: String,
    },
    /// Finish signing in to `tracker` with the redirect the system's sign-in sheet came back
    /// with, after [`Event::Authorize`](crate::event::Event::Authorize): the whole URL, its query
    /// and its fragment. A secret, since it can carry the token itself.
    FinishSignIn {
        tracker: String,
        callback: Secret,
    },
    /// The sign-in sheet for `tracker` was closed without signing in, or its device code's
    /// screen was: that flow stops, and its code comes down with `Event::DeviceCode(None)`.
    CancelSignIn {
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
    ///
    /// It opens where the episode was left, said with
    /// [`Event::Resumed`](crate::event::Event::Resumed), unless `from_start`: then it opens at
    /// the beginning, as "Play from the beginning" asks, and says nothing of where it was left.
    /// Either way the stored position follows the playback from there, as it plays.
    Play {
        id: AnilistId,
        episode: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        from_start: bool,
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
    /// Results come back as [`Event::MatchChoices`](crate::event::Event::MatchChoices),
    /// reusing the Disambiguate flow.
    ManualSearch {
        id: AnilistId,
        query: String,
    },
    /// Forget the pinned match and cached resolution for a title, then re-resolve
    /// automatically — the undo for [`Command::FixMatch`].
    ClearMatch {
        id: AnilistId,
    },
    /// Mark episodes watched or not: written to history, with the tracker push queued when
    /// marking, then answered with [`Event::Progress`](crate::event::Event::Progress) for the
    /// title, as the store then holds it, when the engine knows the title.
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
    /// covers this the same way it covers [`Command::Play`].
    PlaySource {
        id: AnilistId,
        episode: String,
        provider_id: String,
        source_id: String,
    },
    /// Something for the live mpv session.
    Player(PlayerCommand),
    /// What the player a front end hosts says of `playback`: see [`crate::hosted`]. Taken in
    /// as it is sent, ahead of the commands waiting, as mpv's reports are read off its socket.
    PlayerReport {
        playback: PlaybackId,
        report: PlayerReport,
    },
    /// Pin a title to a provider result the user picked, then load its episodes.
    FixMatch {
        id: AnilistId,
        provider_id: String,
        key: anistream_core::ids::ProviderKey,
    },
    /// Queue an episode for offline download.
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
    /// Step a setting through its values, toward `delta`'s sign: a ladder one rung, stopping at
    /// its ends, and a two-state value over to the other. The arrows on a Settings row, and Enter.
    ///
    /// The engine puts the new value in effect and writes it, and answers with
    /// [`Event::Settings`](crate::event::Event::Settings): what the file then holds, with the
    /// setting put back when the write was refused, and a notice saying so.
    StepSetting {
        key: SettingKey,
        delta: i64,
    },
    /// Set a setting to `value`, as the file takes it: a typed one, or the source order moved
    /// on Providers. Answered as [`Command::StepSetting`] is; a value the setting does not take
    /// changes nothing.
    SetSetting {
        key: SettingKey,
        value: anistream_core::settings::SettingValue,
    },
    /// Sub or dub for this session only: put in effect at once and not written, so the next
    /// launch goes by the saved choice. Answered with
    /// [`Event::Settings`](crate::event::Event::Settings).
    SetTranslation {
        translation: anistream_core::media::Translation,
    },
    /// The app moved to the foreground or the background, as the system says. In the
    /// background torrents wait, but for the episode playing when `playing`; back in the
    /// foreground the connection check runs again before they resume, a loopback server the
    /// system took is bound again on its own port, and what plays is opened again where it was.
    AppPhase {
        phase: AppState,
    },
    /// The network the device uses now, as the platform's monitor says each time it changes:
    /// `cellular`, `constrained` for Low Data Mode, and `offline` when there is no path at all.
    /// On cellular, torrents and downloads run only as `TorrentsOnCellular` and
    /// `DownloadsOnCellular` let them, the torrent source being unavailable there while the
    /// first is off; in Low Data Mode downloads and seeding wait, whatever those say.
    ///
    /// The engine owns being offline: it says each change with
    /// [`Event::Connectivity`](crate::event::Event::Connectivity), and when the path comes
    /// back it asks once more for what the screens asked for, or failed to load, while it was
    /// gone. Nothing is refused while offline. The engine counts as online until told it is not.
    NetworkPath {
        cellular: bool,
        constrained: bool,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        offline: bool,
    },
    /// Protect torrent traffic with `mode`, through `proxy` when it is one, and have a failed
    /// connection check do `on_failure`: written to the file as one change, the proxy's
    /// password to the host's secret store and never to the file. No protection and warn only
    /// need `acknowledged`, the viewer having confirmed it, unless the file holds that choice
    /// already; asked for without it, nothing is written and a notice says why.
    ///
    /// Answered as [`Command::SetSetting`] is, with the torrent source brought back up on what
    /// was written.
    SetTorrentProtection {
        mode: TorrentProtection,
        proxy: Option<TorrentProxy>,
        on_failure: FailedCheck,
        acknowledged: bool,
    },
    /// Read one show's settings sheet: answered with
    /// [`Event::ShowSettings`](crate::event::Event::ShowSettings).
    LoadShowSettings {
        id: AnilistId,
    },
    /// Set `key` for show `id` to `value`, or with `None` have it follow Settings again.
    /// Answered with the show's sheet, and the settings with their counts of shows that set
    /// each; a value the setting does not take changes nothing and is said.
    SetShowSetting {
        id: AnilistId,
        key: ShowSettingKey,
        value: Option<anistream_core::settings::SettingValue>,
    },
    /// Put show `id` back on Settings: everything it sets for itself, the source it keeps and
    /// the tracks the player remembers for it, forgotten. Answered as
    /// [`Command::SetShowSetting`] is.
    ResetShowSettings {
        id: AnilistId,
    },
    /// Choose the subtitles show `id` plays with in `translation`, outside the player: one of
    /// the choices its sheet lists as remembered, or `None` to forget the one remembered, so
    /// Settings' subtitles apply again. The audio and the delay remembered are kept. Answered
    /// with the show's sheet.
    SetShowSubtitles {
        id: AnilistId,
        translation: anistream_core::media::Translation,
        subtitles: Option<crate::show::RememberedSubtitles>,
    },
    /// Steer the playback `playback`, and only it: a command for one that is no longer the
    /// playback the engine started last is let go. What a hosted player sends, which names the
    /// playback its interface shows, so a Play Now or an Undo that arrives as one episode rolls
    /// on to the next cannot reach the next one. [`Command::Player`] steers whatever plays.
    Control {
        playback: PlaybackId,
        command: PlayerCommand,
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
        /// What the store's owner sees this device called: the confirmation's field, which
        /// starts as [`InvitedStore::device_label`]. `None`, or only spaces, for that kind of
        /// device. An open link sends no name, and this is not sent with it.
        ///
        /// [`InvitedStore::device_label`]: crate::stores::InvitedStore::device_label
        #[cfg_attr(feature = "ffi", uniffi(default))]
        label: Option<String>,
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
    /// Switch the plugin `id` off, or back on, without removing it: it keeps its place and its
    /// row, and is not tried while off. Answered with the settings, the Providers rows and a
    /// notice; anything but a plugin is refused in words.
    DisablePlugin {
        id: String,
        disabled: bool,
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
    /// Let anyone with the store's link join, or only people its owners invite. Opening makes a
    /// new link; going back to invites only stops new joins and keeps the devices that joined.
    /// Answered with [`Event::OpenJoin`](crate::event::Event::OpenJoin) and a notice.
    SetOpenJoin {
        store: String,
        open: bool,
    },
    /// Remove one device that joined with the store's link, or all of them when `device` is
    /// `None`. Answered with [`Event::OpenJoin`](crate::event::Event::OpenJoin) and a notice.
    RemoveAnonymous {
        store: String,
        device: Option<String>,
    },
    /// Fetch a cover or banner the front end is about to show, from the cache when it is fresh
    /// there. Answered with [`Event::ArtworkReady`](crate::event::Event::ArtworkReady), or with
    /// [`Event::ImageFailed`](crate::event::Event::ImageFailed), and never with a job: covers
    /// arrive while the screen is already usable.
    ///
    /// Asked for by the front end, because only it knows what is on screen. It asks once per
    /// URL, so a redraw never fetches what is already on its way.
    FetchArtwork {
        url: String,
    },
    /// Look for episodes of the shows followed that aired since they were last looked at:
    /// answered with exactly one [`Event::NewEpisodes`], empty when there is nothing new, with
    /// nothing to follow, or when AniList could not be asked. Never with a job.
    ///
    /// The shows followed are those on the signed-in tracker's list as watching or rewatching,
    /// as its last pull left the list here, and those watched here in the last 30 days, unless
    /// the list has them as dropped or paused. A show planned is not, unless it turns
    /// [`ShowSettingKey::NotifyNewEpisodes`] on for itself, which takes in any show; one that
    /// turns it off is left out. While [`SettingKey::EpisodeNotifications`] is off, only the
    /// shows switched on for themselves are followed, and with none of those either, the check
    /// asks nothing and forgets what it kept.
    ///
    /// A show watched dubbed is said when its dub is out rather than at the broadcast, which is
    /// all AniList dates: once a broadcast is ahead of the dub said, each check lists the show in
    /// the dub from the source a play would use, as [`Command::LoadDub`] does, and says the
    /// episodes it lists up to that broadcast, with [`Translation::Dub`]. One listing per such
    /// show, cut off at 8 seconds, and none for a show whose dub has caught up.
    ///
    /// Each episode is said once, and only one that aired after its show was first followed:
    /// the first check after notifications are turned on, or after a show is first followed,
    /// only notes where things stand and says nothing, so a new install never fires a backlog.
    /// An episode already watched here is not said, and nor is one that aired more than a week
    /// ago, after a long time without a check.
    ///
    /// It costs AniList one request at most, for every show at once, and none when nothing
    /// can have aired: when the last check was under five minutes ago, or when the metadata
    /// cache has every show's next episode still to come. It answers within 25 seconds whatever
    /// the network does, 15 for AniList and 8 for the dubs, so it fits in a background task's
    /// time.
    ///
    /// **In the background.** On iOS, from a `BGAppRefreshTask`; on Android, from a periodic
    /// `WorkManager` job, scheduled while the setting is on or any show is switched on for
    /// itself, as [`SettingsView::notify_switched_on`] counts them, and cancelled only when
    /// neither is. Start the engine if the app is not running, with every
    /// `FfiBackground` switch off, send this, and read events until the
    /// [`Event::NewEpisodes`]. Post one notification for each episode in it, identified by the
    /// show and the episode, so that one posted twice replaces itself. Then send
    /// [`Command::WidgetSnapshot`] and write what it answers for the widgets, before completing
    /// the task. Nothing else needs to run: the check reads the list the last pull in the
    /// foreground kept, and the settings and title form the engine has on file.
    ///
    /// [`Event::NewEpisodes`]: crate::event::Event::NewEpisodes
    /// [`SettingsView::notify_switched_on`]: crate::settings::SettingsView::notify_switched_on
    /// [`Translation::Dub`]: anistream_core::media::Translation::Dub
    CheckNewEpisodes,
    /// The record the home-screen widgets show: answered with one
    /// [`Event::WidgetSnapshot`](crate::event::Event::WidgetSnapshot), never with a job.
    ///
    /// Continue's first five titles, from local history, with their titles and art from the
    /// metadata cache, asking AniList only for one the cache does not have; and the broadcasts
    /// from the start of today in the device's time zone to a week on, a request for each page
    /// of fifty, used again for five minutes. What could not be fetched is left out rather than
    /// failing the rest.
    ///
    /// Ask after anything that changes Continue, such as an episode ending, as the app goes to
    /// the background, and from the background task after [`Command::CheckNewEpisodes`].
    WidgetSnapshot,
    /// What will notify of new episodes in the next seven days, and what is left out and why:
    /// answered with one [`Event::UpcomingNotices`](crate::event::Event::UpcomingNotices), never
    /// with a job.
    ///
    /// For an app that schedules its notifications ahead, to fire at air time with no code of
    /// its own running then, as Apple's local notifications do; and for its Notifications pane's
    /// list. The week's broadcasts are the widgets' record's, fetched once for both and used
    /// again for a few minutes, so asking after [`Command::WidgetSnapshot`] costs AniList
    /// nothing. The shows are those [`Command::CheckNewEpisodes`] follows with the setting on,
    /// each with why: being watched, or switched on or off for itself; and whether it notifies,
    /// which with the setting off only a show switched on does. Each carries the translation
    /// the show is watched in: one watched dubbed is the check's to say, when its dub is out, and
    /// not to schedule at the broadcast.
    ///
    /// Ask on launch, on coming to the foreground, after a sync or a change of progress, and
    /// from the background task, after [`Command::CheckNewEpisodes`].
    UpcomingNotices,
    /// Ask GitHub for the newest release now, rather than waiting for the daily check: Settings'
    /// Check Now. Answered with [`Event::Update`](crate::event::Event::Update), `Checking` and
    /// then what it found, never with a job. Asks whether or not the daily check is switched on,
    /// and keeps what it found for the daily check to go by.
    CheckForUpdates,
}

/// A control sent to whatever is currently playing.
///
/// Kept as data rather than a call into the player so the reducer stays pure and testable —
/// the binary routes these to the live session.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerCommand {
    TogglePause,
    /// Seek by a relative number of seconds; negative goes back.
    Seek(f64),
    /// Seek to an absolute position — how the skip prompt is taken.
    SeekTo(f64),
    /// Multiply the current speed by this factor, clamped by the player: a step faster or
    /// slower, as the terminal's `]` and `[` take it (1.1 and 1/1.1, mpv's own steps).
    Speed(f64),
    /// Set the speed, clamped by the player: what a player's menu of speeds picks, 1.5 for 1.5×,
    /// whatever the speed was.
    SpeedTo(f64),
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
    /// Say something on the video. Sent by the task runner when the episode asked for from the
    /// player could not be found, so the viewer, who is looking at the player, learns why the
    /// episode they were watching is still on.
    Alert(String),
    /// Take the skip on offer.
    TakeSkip,
    /// Undo an automatic skip, back to `to`, where it started.
    UndoSkip {
        to: f64,
    },
    /// Play the next episode now: Up Next's button, and what a hosted player sends when its
    /// countdown reaches zero.
    PlayNext,
    /// Do not roll on when this episode ends.
    CancelNext,
    /// A pick in the player's menus.
    Choose(PlayerChoice),
    /// The stall prompt's answers: another stream, or keep waiting.
    StallSwitch,
    StallWait,
    /// Night mode on or off.
    NightMode(bool),
    /// An upscaling look, by id: `off`, `natural` or `sharp`.
    Upscaling(String),
    /// The viewer picked a track by its id in the file's `track-list`, or with `None` turned
    /// the subtitles off. The engine selects it and remembers it for the series, as it
    /// remembers a pick made with mpv's own keys, without waiting for the player to report it.
    ChooseTrack {
        kind: crate::hosted::TrackKind,
        id: Option<i64>,
    },
    /// The viewer set the subtitle delay to `seconds`. Remembered for the series once it stops
    /// moving, as a delay set with mpv's own keys is.
    SubtitleDelay {
        seconds: f64,
    },
    /// Open the file again where it plays now: the loopback server it is read from was bound
    /// again, after the system took it while the app was in the background.
    Reload,
}

/// Where the app is, as the system has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AppState {
    /// On screen.
    #[default]
    Foreground,
    /// Off screen, still playing the episode on when `playing`: its audio, or a screen locked
    /// on the player.
    Background { playing: bool },
    /// Off screen while the system lets the app's downloads go on, as iOS's continued-processing
    /// task does: downloads and what plays go on, seeding waits. Downloads on a metered network
    /// wait, as on screen. Sent when the window opens, and `Background` again when it ends.
    BackgroundDownloads,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_never_renders_through_debug() {
        // The task router logs tasks on a routing mistake; this is what keeps that log
        // safe to read.
        let task = Command::ConnectKitsu {
            username: "nagare".into(),
            password: Secret("hunter2".into()),
        };
        let printed = format!("{task:?}");
        assert!(!printed.contains("hunter2"), "leaked: {printed}");
        assert!(printed.contains("redacted"));
    }
}
