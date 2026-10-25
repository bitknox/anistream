//! A newer release of anistream: what it is, and the files each app installs it from.
//!
//! The engine looks for one once a day, and whenever a front end asks with
//! [`Command::CheckForUpdates`](crate::command::Command::CheckForUpdates), and says what it found
//! with [`Event::Update`](crate::event::Event::Update). Nothing here downloads or installs: an
//! app does that with the file named for its platform, once the viewer chooses to update.

/// A release newer than the build running, as GitHub publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Release {
    /// The version, without the tag's `v`: `2.0.3`.
    pub version: String,
    /// When it was published, in seconds since the Unix epoch.
    pub published_at: i64,
    /// Its page on GitHub, with the full notes and every file, for a platform that cannot
    /// install from here.
    pub page_url: String,
    /// What changed, in short: the first section of the release's notes as plain text, at most
    /// a few hundred characters, cut at the end of a line. Empty when the release says nothing.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub notes: String,
    /// The Mac app, `anistream-<version>-macos.dmg`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub macos: Option<ReleaseAsset>,
    /// The iPhone and iPad app for SideStore and AltStore, `anistream-<version>-ios-unsigned.ipa`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ios: Option<ReleaseAsset>,
    /// The Android app, `anistream-<version>-android-arm64.apk`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub android: Option<ReleaseAsset>,
    /// The Android TV app, `anistream-<version>-android-tv.apk`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub android_tv: Option<ReleaseAsset>,
}

/// One file of a release, with what it has to match once downloaded.
///
/// A platform's file is only named when the release has it and its `.sha256` beside it, and that
/// checksum file reads: a release without one, or with one that does not say a SHA-256, gives
/// the platform no file, and the app sends the viewer to [`Release::page_url`] instead.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ReleaseAsset {
    /// The file's name: `anistream-2.0.3-android-arm64.apk`.
    pub name: String,
    /// Where it downloads from: GitHub's address for it, which redirects.
    pub url: String,
    /// Its SHA-256 as the release publishes it in `<name>.sha256`: 64 lowercase hex digits.
    pub sha256: String,
    /// Its size in bytes, as GitHub lists it.
    pub size: u64,
}

/// Where the update check stands, as [`Event::Update`](crate::event::Event::Update) says it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum UpdateStatus {
    /// Asking GitHub now, for a [`Command::CheckForUpdates`](crate::command::Command::CheckForUpdates).
    Checking,
    /// This build is the newest release.
    UpToDate,
    /// A newer release. Settings offers it for as long as it is the newest.
    ///
    /// `notify` is whether to tell of it now, with the app's own notice: a release is told the
    /// first time it is found, then a newer patch at most once a week, and a new minor or major
    /// at once. False for a release found by a check the viewer asked for, which Settings shows
    /// where they asked.
    Available { release: Box<Release>, notify: bool },
    /// The check the viewer asked for got no answer: offline, or GitHub did not say. The daily
    /// check says nothing when it fails.
    Failed { reason: String },
}
