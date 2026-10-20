//! What a start takes beside the app's host, and how it fails: the only types that cross which
//! are this crate's own rather than the model's.

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
