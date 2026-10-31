//! The parts of Jellyfin's answers anistream reads. Jellyfin writes PascalCase.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Ticks are Jellyfin's time unit: 100 nanoseconds.
pub const TICKS_PER_SECOND: i64 = 10_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ServerInfo {
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub product_name: String,
    #[serde(default)]
    pub id: String,
}

/// A signed-in session: the token to keep, and who it is for. The password is never kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub user_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct AuthenticationResult {
    pub access_token: String,
    pub user: AuthUser,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct AuthUser {
    pub id: String,
    pub name: String,
}

/// A Quick Connect attempt: the code to show, and the secret to poll with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct QuickConnect {
    pub code: String,
    pub secret: String,
    #[serde(default)]
    pub authenticated: bool,
}

/// A library, as `/Users/{id}/Views` lists it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Library {
    pub id: String,
    pub name: String,
    /// `tvshows`, `movies`, `music`, `homevideos` and so on. Absent for a mixed library.
    #[serde(default)]
    pub collection_type: Option<String>,
}

/// A series in a library.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Series {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub original_title: Option<String>,
    #[serde(default)]
    pub production_year: Option<u16>,
    /// The ids other services know it by: `AniList`, `AniDB`, `Tvdb`, `Tmdb`, `Imdb`. Keys differ
    /// in case between metadata plugins, so look them up with [`Series::provider_id`].
    #[serde(default)]
    pub provider_ids: BTreeMap<String, String>,
    #[serde(default)]
    pub child_count: Option<u32>,
}

impl Series {
    /// A provider id by name, without regard to case.
    pub fn provider_id(&self, name: &str) -> Option<u32> {
        self.provider_ids
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .and_then(|(_, value)| value.trim().parse().ok())
    }
}

/// One episode of a series.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EpisodeItem {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// The season the server files it under; `0` is specials.
    #[serde(default, rename = "ParentIndexNumber")]
    pub season: Option<u32>,
    #[serde(default, rename = "IndexNumber")]
    pub number: Option<u32>,
    #[serde(default)]
    pub run_time_ticks: Option<i64>,
    #[serde(default)]
    pub user_data: Option<UserData>,
}

/// What the server remembers of this user's viewing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct UserData {
    #[serde(default)]
    pub playback_position_ticks: i64,
    #[serde(default)]
    pub played: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Items<T> {
    #[serde(default = "Vec::new")]
    pub items: Vec<T>,
}

/// How to play an item.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct PlaybackInfo {
    #[serde(default)]
    pub media_sources: Vec<MediaSource>,
    #[serde(default)]
    pub play_session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaSource {
    pub id: String,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub supports_direct_play: bool,
    #[serde(default)]
    pub media_streams: Vec<MediaStream>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct MediaStream {
    pub index: i32,
    /// `Video`, `Audio` or `Subtitle`.
    #[serde(default, rename = "Type")]
    pub kind: String,
    #[serde(default)]
    pub is_external: bool,
    #[serde(default)]
    pub codec: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    /// For an external subtitle, where the server serves it, relative to the server.
    #[serde(default)]
    pub delivery_url: Option<String>,
}
