//! What a start takes beside the app's host, and how it fails: the only types that cross which
//! are this crate's own rather than the model's.

use anistream_engine::options::Background;

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
        } = background;
        Self { prune, update_check, digest, datasets, sync_loops, store_refresh, player_check }
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
        } = background;
        Self { prune, update_check, digest, datasets, sync_loops, store_refresh, player_check }
    }
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
            }
        );
    }
}
