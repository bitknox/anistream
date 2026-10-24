//! What a start takes beside the app's host, and how it fails: the only types that cross which
//! are this crate's own rather than the model's.

use anistream_engine::options::{AppClients, Background};
use anistream_model::playback::PlayerTheme;

/// Where the engine keeps its files: directories the app chooses, such as its Application
/// Support and Caches directories on macOS, or its container's on iOS.
///
/// Paths as strings, which every foreign language has. Each is created when it is missing.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiDirs {
    /// Where `config.toml` is, and the plugins.
    pub config: String,
    /// Where the database is.
    pub data: String,
    /// Where what can be fetched again is kept: covers, logs, the player's leftovers.
    pub cache: String,
}

/// The work the engine does on its own beside the commands, each piece on its own switch: the
/// engine's [`Background`], as an app hands it over.
///
/// Every field is on unless the app says otherwise, as the terminal app launches, so a front end
/// names only what it turns off: the sync loops, say, where the system runs the app's background
/// work on a schedule of its own. Without the desktop feature there is no player to check, and
/// `player_check` does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct FfiBackground {
    /// Sweep old logs, stale covers and the player's leftovers at launch, and the torrent cache
    /// when torrents are off.
    #[uniffi(default = true)]
    pub prune: bool,
    /// Ask once a day whether there is a new release, and say so when there is.
    #[uniffi(default = true)]
    pub update_check: bool,
    /// Say what aired since the last launch for the shows being watched.
    #[uniffi(default = true)]
    pub digest: bool,
    /// Keep the mapping datasets current, which the trackers match titles by.
    #[uniffi(default = true)]
    pub datasets: bool,
    /// Push what is queued to the trackers, and pull their lists, on a timer.
    #[uniffi(default = true)]
    pub sync_loops: bool,
    /// Ask each joined plugin store for its list once at launch.
    #[uniffi(default = true)]
    pub store_refresh: bool,
    /// Check that mpv is there to play, and say so at launch.
    #[uniffi(default = true)]
    pub player_check: bool,
    /// Keep next: queue the next episodes of each show being watched, at launch and as the
    /// viewer moves on. Off for a background task's engine, which has seconds to do its work.
    #[uniffi(default = true)]
    pub keep_next: bool,
}

impl Default for FfiBackground {
    /// Everything on, as the terminal app launches.
    fn default() -> Self {
        Background::default().into()
    }
}

impl From<Background> for FfiBackground {
    fn from(background: Background) -> Self {
        let Background {
            prune,
            update_check,
            digest,
            datasets,
            sync_loops,
            store_refresh,
            player_check,
            keep_next,
        } = background;
        Self {
            prune,
            update_check,
            digest,
            datasets,
            sync_loops,
            store_refresh,
            player_check,
            keep_next,
        }
    }
}

impl From<FfiBackground> for Background {
    fn from(background: FfiBackground) -> Self {
        let FfiBackground {
            prune,
            update_check,
            digest,
            datasets,
            sync_loops,
            store_refresh,
            player_check,
            keep_next,
        } = background;
        Self {
            prune,
            update_check,
            digest,
            datasets,
            sync_loops,
            store_refresh,
            player_check,
            keep_next,
        }
    }
}

/// What the app says of itself as it starts the engine: how it dresses the player, what it runs
/// on, the sign-in clients it ships with, and where tokens are kept.
///
/// Every field has a default, and [`FfiStart::default`], `FfiStart()` in Swift, is the engine as
/// it started before there was a choice: the terminal app's palette and the app's keychain. The
/// two enums are optional only because UniFFI cannot give an enum field a default; `None` is
/// the default named on each.
#[derive(Debug, Clone, Default, PartialEq, Eq, uniffi::Record)]
pub struct FfiStart {
    /// How the player's own interface is dressed: the app's palette. `None` for the terminal
    /// app's immersive palette.
    #[uniffi(default = None)]
    pub theme: Option<PlayerTheme>,
    /// What the app runs on, which a plugin store calls this device when it joins with an
    /// invite, unless the person joining names it otherwise: [`FfiPlatform::device_label`].
    /// `None` for "Computer".
    #[uniffi(default = None)]
    pub platform: Option<FfiPlatform>,
    /// The AniList client the app registered and ships with, for signing in through its own
    /// URL scheme. Not yet used: native sign-in (E6) reads it.
    #[uniffi(default = None)]
    pub anilist_client_id: Option<String>,
    /// Where the AniList client redirects, as it was registered: an Android App Link such as
    /// `https://anistream.tv/auth/anilist`, which no other app can claim, where the implicit
    /// grant puts the token in the redirect. `None` for the app's scheme,
    /// `anistream://auth/anilist`. `Event::Authorize`'s callback scheme follows it.
    #[uniffi(default = None)]
    pub anilist_redirect_uri: Option<String>,
    /// The MyAnimeList client the app registered and ships with, as `anilist_client_id`.
    #[uniffi(default = None)]
    pub mal_client_id: Option<String>,
    /// Where the MyAnimeList client redirects, `None` for `anistream://auth/mal`.
    #[uniffi(default = None)]
    pub mal_redirect_uri: Option<String>,
    /// The Simkl client the app registered and ships with, for its device sign-in.
    #[uniffi(default = None)]
    pub simkl_client_id: Option<String>,
    /// The Trakt client the app registered and ships with, for its device sign-in.
    #[uniffi(default = None)]
    pub trakt_client_id: Option<String>,
    /// The Trakt client's secret, for a client that has one; `None` signs in without.
    #[uniffi(default = None)]
    pub trakt_client_secret: Option<String>,
    /// Where the trackers' and the plugin stores' tokens are kept. `None` for
    /// [`SecretsMode::Host`].
    #[uniffi(default = None)]
    pub secrets: Option<SecretsMode>,
}

impl FfiStart {
    /// Where tokens are kept, the default filled in.
    pub fn secrets_mode(&self) -> SecretsMode {
        self.secrets.unwrap_or_default()
    }

    /// The sign-in clients, as the engine is handed them.
    pub fn app_clients(&self) -> AppClients {
        AppClients {
            anilist: self.anilist_client_id.clone(),
            anilist_redirect: self.anilist_redirect_uri.clone(),
            mal: self.mal_client_id.clone(),
            mal_redirect: self.mal_redirect_uri.clone(),
            simkl: self.simkl_client_id.clone(),
            trakt: self.trakt_client_id.clone(),
            trakt_secret: self.trakt_client_secret.clone(),
        }
    }
}

/// The system an app runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiPlatform {
    MacOs,
    Ios,
    IpadOs,
    /// An Android phone.
    Android,
    /// An Android tablet: a screen at least 600 dp wide the short way.
    AndroidTablet,
    /// Android TV or Google TV: a television driven by a remote, whatever its screen.
    AndroidTv,
    Windows,
}

impl FfiPlatform {
    /// What a plugin store calls a device of this kind that joins with an invite, unless the
    /// person joining names it otherwise. The kind of device, never its own name, which is
    /// often its owner's.
    pub const fn device_label(self) -> &'static str {
        match self {
            Self::MacOs => "Mac",
            Self::Ios => "iPhone",
            Self::IpadOs => "iPad",
            Self::Android => "Android phone",
            Self::AndroidTablet => "Android tablet",
            Self::AndroidTv => "Android TV",
            Self::Windows => "Windows PC",
        }
    }
}

/// Where the engine keeps the trackers' and the plugin stores' tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, uniffi::Enum)]
pub enum SecretsMode {
    /// Always the app's keychain, through its host: [`FfiHost::secret_get`] and the rest. An iOS
    /// app's, whose keychain is its own.
    ///
    /// [`FfiHost::secret_get`]: crate::FfiHost::secret_get
    #[default]
    Host,
    /// Where the terminal app keeps them, for a Mac app that shares its data: as
    /// `trackers.token_storage` says, with `ANISTREAM_TOKEN_STORAGE` over it as the terminal
    /// has it. `"file"` is `0600` files in `tokens` under the data directory, the terminal's own;
    /// `"keychain"` is the app's keychain through its host, which keeps the items under service
    /// `anistream` and the token's id, the ones the terminal keeps, with those files as the
    /// fallback. Signing in through one app signs in the other.
    FollowConfig,
}

/// Why [`FfiEngine::start`](crate::FfiEngine::start) could not stand the engine up, by what
/// failed, each with words a front end can show as they are.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, uniffi::Error)]
pub enum FfiError {
    /// One of the directories could not be created.
    #[error("{reason}")]
    Dirs { reason: String },
    /// `config.toml` could not be read, or says something unsafe or self-contradictory. Refused
    /// rather than replaced by the defaults, which could mean ignoring a VPN setting.
    #[error("{reason}")]
    Config { reason: String },
    /// The database could not be opened.
    #[error("{reason}")]
    Store { reason: String },
    /// The HTTP client could not be built from the network settings.
    #[error("{reason}")]
    Client { reason: String },
    /// The runtime the engine runs on could not be started.
    #[error("{reason}")]
    Runtime { reason: String },
    /// The engine itself did not start.
    #[error("{reason}")]
    Engine { reason: String },
    /// A TV's remote could not start listening, or could not forget a phone.
    #[error("{reason}")]
    Remote { reason: String },
    /// The hosted player's libmpv core did not start. Only in a build with the `mpv-host`
    /// feature, so the phones' bindings never have it.
    #[cfg(feature = "mpv-host")]
    #[error("{reason}")]
    Player { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_an_app_names_nothing_of_is_the_engine_as_it_started_before() {
        let start = FfiStart::default();
        assert_eq!(start.theme, None);
        assert_eq!(start.platform, None);
        assert_eq!(start.secrets_mode(), SecretsMode::Host);
        assert_eq!(start.app_clients(), AppClients::default());
        let mac = FfiStart {
            secrets: Some(SecretsMode::FollowConfig),
            anilist_client_id: Some("4242".into()),
            ..FfiStart::default()
        };
        assert_eq!(mac.secrets_mode(), SecretsMode::FollowConfig);
        assert_eq!(
            mac.app_clients(),
            AppClients { anilist: Some("4242".into()), ..AppClients::default() }
        );
    }

    #[test]
    fn a_store_calls_each_device_its_kind() {
        let kinds = [
            (FfiPlatform::MacOs, "Mac"),
            (FfiPlatform::Ios, "iPhone"),
            (FfiPlatform::IpadOs, "iPad"),
            (FfiPlatform::Android, "Android phone"),
            (FfiPlatform::Windows, "Windows PC"),
            (FfiPlatform::AndroidTablet, "Android tablet"),
            (FfiPlatform::AndroidTv, "Android TV"),
        ];
        for (platform, label) in kinds {
            assert_eq!(platform.device_label(), label);
        }
    }

    #[test]
    fn an_apps_redirects_and_device_clients_reach_the_engine_as_given() {
        let android = FfiStart {
            anilist_client_id: Some("4242".into()),
            anilist_redirect_uri: Some("https://anistream.tv/auth/anilist".into()),
            mal_client_id: Some("a-mal-client".into()),
            simkl_client_id: Some("a-simkl-client".into()),
            trakt_client_id: Some("a-trakt-client".into()),
            ..FfiStart::default()
        };
        let clients = android.app_clients();
        assert_eq!(
            clients.anilist_redirect.as_deref(),
            Some("https://anistream.tv/auth/anilist")
        );
        assert_eq!(clients.callback_scheme("anilist"), "https");
        assert_eq!(
            clients.redirect("mal"),
            "anistream://auth/mal",
            "MyAnimeList keeps its scheme"
        );
        assert_eq!(clients.simkl.as_deref(), Some("a-simkl-client"));
        assert_eq!(clients.trakt.as_deref(), Some("a-trakt-client"));
        assert_eq!(clients.trakt_secret, None);
    }

    #[test]
    fn the_background_an_app_names_nothing_of_is_the_terminal_apps_launch() {
        // Every switch defaults to on, here and in the bindings, so an app turns off only what it
        // names, and a switch added later starts on for an app that does not know of it yet.
        assert_eq!(Background::from(FfiBackground::default()), Background::default());
        let quiet = FfiBackground { sync_loops: false, ..FfiBackground::default() };
        assert_eq!(
            Background::from(quiet),
            Background { sync_loops: false, ..Background::default() }
        );
        assert_eq!(
            FfiBackground::from(Background::none()),
            FfiBackground {
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
