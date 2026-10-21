//! Commands: the work a front end asks for, and the controls it sends to whatever is playing.

use anistream_core::ids::AnilistId;

use crate::{
    catalog::{LibrarySegment, SearchFilters},
    episodes::EpisodeRow,
    hosted::{PlayerChoice, PlayerReport},
    playback::{PlaybackId, Successor},
    protection::{FailedCheck, TorrentProtection, TorrentProxy},
    settings::SettingKey,
    show::ShowSettingKey,
    stores::Secret,
};

impl Command {
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
