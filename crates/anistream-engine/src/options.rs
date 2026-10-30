//! What an engine is started with.

use anistream_core::config::{Config, Paths};
use anistream_model::playback::PlayerTheme;
use anistream_net::HttpClient;
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

/// Everything [`crate::engine::Engine::start`] needs to stand the engine up.
///
/// [`Self::new`] gives the app's own launch: the source chain config describes, built by the
/// engine with its torrent source and plugins coming up behind it, and the work the engine does
/// on its own. A still frame or a test changes the fields after.
pub struct EngineOptions {
    /// Where the config, the database, the caches and the tokens are.
    pub paths: Paths,
    /// The config the app was launched with: the file's, with whatever the command line
    /// changed for this session.
    pub config: Config,
    /// The config the file holds, when the front end has read it already, or `None` for the
    /// engine to read it from `paths`. What tells the session's own choices from the file's on
    /// the Settings screen.
    pub saved: Option<Config>,
    /// The client every request goes through, with the configured proxy and timeouts.
    pub http: HttpClient,
    /// The local database.
    pub store: Store,
    /// How the player's own interface is dressed: the front end's choice, from its palette.
    pub player_theme: PlayerTheme,
    /// Whether a play opens the player the front end hosts, driven through `Event::PlayerOps`,
    /// rather than mpv's process: always so without the desktop feature, where there is no other
    /// player, and on the desktop only when the front end asks.
    pub hosted_player: bool,
    /// Whether `syncplay.enabled` sends every play to the watch party the settings name: the
    /// terminal's way in, on for it alone. An app starts parties when asked, with `PartyJoin`
    /// or `PlayParty`, and the setting does nothing there. Off unless the front end turns it on.
    pub every_play_a_party: bool,
    /// The sign-in clients a native app ships with, none for the terminal app, whose user
    /// registers their own in the config.
    pub app_clients: AppClients,
    /// The kind of device the front end runs on, which a plugin store calls it when it joins
    /// with an invite unless the person joining names it otherwise: "iPad", "Android phone".
    /// Never the device's own name, which is often its owner's. [`COMPUTER`] for the terminal.
    ///
    /// [`COMPUTER`]: anistream_model::stores::COMPUTER
    pub device_label: String,
    /// The source chain, whole, for an engine that is not to build its own: a still frame
    /// waits for every source before it starts, and a test brings sources of its own.
    ///
    /// `None` for the app's launch, where the engine builds the chain config describes. The
    /// torrent source joins it once its VPN guard has passed, and the plugins once compiled,
    /// so that the first frame waits for neither. A chain given whole is final: nothing joins
    /// it, and its rows are said at launch.
    pub sources: Option<ProviderRegistry>,
    /// The work the engine does on its own beside the commands, each piece on its own switch:
    /// all of it for the app's launch, none for a still frame or a test, which must reach no
    /// further than they are sent. See [`Background`].
    pub background: Background,
    /// Parts a test stands the engine up with instead of the ones it builds.
    /// Where a damaged database was moved before this launch, when one was: said at launch,
    /// so the viewer knows why their history starts over and where the old file is. See
    /// `Store::open_or_set_aside`.
    pub set_aside: Option<std::path::PathBuf>,
    #[cfg(feature = "testing")]
    pub parts: Option<Parts>,
    /// What keep next takes to be free on the device, in bytes, in place of asking the disk:
    /// for a test, whose answer should not depend on the machine it runs on.
    #[cfg(feature = "testing")]
    pub free_space: Option<u64>,
}

impl EngineOptions {
    /// The options for the app's own launch: the engine builds its source chain and does its
    /// own work beside the commands.
    pub fn new(
        paths: Paths,
        config: Config,
        http: HttpClient,
        store: Store,
        player_theme: PlayerTheme,
    ) -> Self {
        Self {
            paths,
            config,
            saved: None,
            http,
            store,
            player_theme,
            hosted_player: !cfg!(feature = "desktop"),
            every_play_a_party: false,
            app_clients: AppClients::default(),
            device_label: anistream_model::stores::COMPUTER.to_owned(),
            sources: None,
            background: Background::default(),
            set_aside: None,
            #[cfg(feature = "testing")]
            parts: None,
            #[cfg(feature = "testing")]
            free_space: None,
        }
    }
}

/// The OAuth clients a native app registered for itself and ships with: AniList's for its
/// implicit grant, which needs no secret, and MAL's for PKCE, each redirecting to where its client
/// registered, the app's own URL scheme unless it says otherwise; and Simkl's and Trakt's for
/// their device flows. Unlike `trackers.*.client_id`, which a user registers for the loopback
/// sign-in, these are the app's, so they are never written to the config.
///
/// A tracker with a client here is turned on for the trackers alone. AniList and MyAnimeList sign
/// in through `Event::Authorize` and the app's sheet ([`crate::tracking::native`]), Simkl and
/// Trakt with a code to type on another screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppClients {
    /// The app's AniList client id.
    pub anilist: Option<String>,
    /// Where the app's AniList client redirects, as it was registered: an Android App Link
    /// (`https://…/auth/anilist`), which no other app can claim, since the implicit grant puts the
    /// token in the redirect. `None` for the app's scheme, `anistream://auth/anilist`.
    pub anilist_redirect: Option<String>,
    /// The app's MyAnimeList client id.
    pub mal: Option<String>,
    /// Where the app's MyAnimeList client redirects, `None` for `anistream://auth/mal`.
    pub mal_redirect: Option<String>,
    /// The app's Simkl client id.
    pub simkl: Option<String>,
    /// The app's Trakt client id.
    pub trakt: Option<String>,
    /// The app's Trakt client secret, for a client that has one. Polling for the device flow's
    /// token goes without it when it is `None`.
    pub trakt_secret: Option<String>,
}

/// The work the engine does on its own beside the commands, each piece on its own switch.
///
/// [`Default`] is the desktop app's launch, with everything on. A still frame and a test turn
/// it all off with [`Self::none`]. A phone keeps what it has a use for: the system there may
/// run some of it on its own schedule, or not let it run at all while the app is in the
/// background.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Background {
    /// Sweep old logs, stale covers and the player's leftovers at launch, and the torrent cache
    /// when torrents are off, since nothing else would.
    pub prune: bool,
    /// Ask once a day whether there is a new release, and say so when there is.
    pub update_check: bool,
    /// Say what aired since the last launch for the shows being watched: a notice, and on the
    /// desktop too when the settings ask for it.
    pub digest: bool,
    /// Keep the mapping datasets current, which the trackers match titles by.
    pub datasets: bool,
    /// Push what is queued to the trackers, and pull their lists, on a timer.
    pub sync_loops: bool,
    /// Ask each joined plugin store for its list once at launch, and say what can be updated.
    pub store_refresh: bool,
    /// Check that mpv is there to play, and new enough to wear anistream's interface, and say
    /// so at launch. Only with the `desktop` feature: without it there is no player to check.
    pub player_check: bool,
    /// Keep next: queue the next episodes of each show being watched, at launch and as the
    /// viewer moves on, as many as the show or Settings keeps. See
    /// [`crate::downloads::ahead`].
    pub keep_next: bool,
}

impl Background {
    /// None of it: a still frame's, and a test's.
    pub const fn none() -> Self {
        Self {
            prune: false,
            update_check: false,
            digest: false,
            datasets: false,
            sync_loops: false,
            store_refresh: false,
            player_check: false,
            keep_next: false,
        }
    }
}

impl Default for Background {
    /// All of it, as the app launches.
    fn default() -> Self {
        Self {
            prune: true,
            update_check: true,
            digest: true,
            datasets: true,
            sync_loops: true,
            store_refresh: true,
            player_check: true,
            keep_next: true,
        }
    }
}

/// What a test stands the engine up with, in place of what the engine would build: a metadata
/// source pointed at a fake, and trackers and plugin stores that keep their tokens in a
/// temporary directory, the trackers' clients pointed at the fake too.
///
/// With these, nothing the engine does reaches the network or the user's keychain. An engine
/// started with them also says in its event stream what it did with each message it left
/// itself, and when it has nothing left to do: see
/// [`EventStream::next_traced`](crate::engine::EventStream::next_traced).
#[cfg(feature = "testing")]
pub struct Parts {
    /// The metadata source.
    pub meta: anistream_meta::Meta,
    /// The trackers, built with the test's token store.
    pub sync: crate::tracking::Sync,
    /// The plugin stores, built with the test's token store.
    pub stores: crate::plugin_stores::Stores,
    /// The clock the new-episode check, the widgets' record and the notifications' week go by,
    /// which the test can move on, and the zone they read days in, which it can choose.
    pub clock: crate::clock::Clock,
    /// The exit network the connection check answers with, so that it makes no lookup and brings
    /// no torrent session up: fixtures, never a real one.
    pub fixture_exit: Option<anistream_providers::vpn::Egress>,
    /// Counted into as each player window opens, when given.
    pub windows: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_apps_launch_and_none_is_nothing_at_all() {
        // The terminal app starts on the default, so a switch defaulting to off would quietly
        // stop something it has always done at launch.
        let on = Background::default();
        let all = [
            on.prune,
            on.update_check,
            on.digest,
            on.datasets,
            on.sync_loops,
            on.store_refresh,
            on.player_check,
            on.keep_next,
        ];
        assert!(all.iter().all(|on| *on), "{on:?}");
        assert_eq!(
            Background::none(),
            Background {
                prune: false,
                update_check: false,
                digest: false,
                datasets: false,
                sync_loops: false,
                store_refresh: false,
                player_check: false,
                keep_next: false,
            }
        );
    }
}
