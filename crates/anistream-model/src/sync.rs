//! Tracker sync: each tracker's state, what a library pull found in dispute, and the code a
//! device sign-in waits on.

use anistream_core::ids::AnilistId;

/// A device code the user must enter elsewhere to finish signing in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct DeviceCodePrompt {
    pub tracker: String,
    /// The short code to type. The only thing on screen that has to be transcribed by hand, so it
    /// gets the brightest treatment the palette has rather than the dimmest.
    pub code: String,
    pub url: String,
}

/// Sync state for one tracker, as the Accounts overlay and the status badge show it.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SyncState {
    pub tracker: String,
    pub connected: bool,
    /// Which account, once known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub user: Option<String>,
    /// Where the token lives, and whether that is the degraded option.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub storage: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub storage_degraded: bool,
    /// Queued operations waiting to go out.
    pub outbox: u32,
    /// How many of those wait for an id mapping: titles this tracker cannot be told about
    /// until the mapping datasets name them.
    pub deferred: u32,
    /// The tracker rejected our credentials; nothing but re-authorising helps.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub needs_reauth: bool,
    /// Most recent sync outcome, in words.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last: Option<String>,
    /// Why the last library pull failed, and when. Cleared by the next pull that succeeds, so a
    /// row never goes on saying "pulled" over an outage.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pull_error: Option<PullError>,
}

/// Why a tracker's library pull failed, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PullError {
    /// What went wrong, in words.
    pub reason: String,
    /// When, in Unix seconds.
    pub at: i64,
}

impl SyncState {
    /// A tracker that is configured but signed out.
    pub fn new(tracker: impl Into<String>) -> Self {
        Self {
            tracker: tracker.into(),
            connected: false,
            user: None,
            storage: None,
            storage_degraded: false,
            outbox: 0,
            deferred: 0,
            needs_reauth: false,
            last: None,
            pull_error: None,
        }
    }

    /// Whether the badge should use the alert role.
    pub fn is_alerting(&self) -> bool {
        self.needs_reauth
    }
}

/// One unresolved divergence, for the Conflicts overlay.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ConflictRow {
    pub anilist_id: AnilistId,
    pub title: String,
    pub field: String,
    pub local: String,
    pub remote: String,
}
