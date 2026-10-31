//! Identifier newtypes.
//!
//! [`AnilistId`] is the canonical primary key for the whole application. Every other
//! identifier here exists because some *external* service insists on its own numbering:
//! aniskip wants a MAL id, tvdb/tmdb-keyed sources want theirs. The mapping layer's job
//! is to translate between them, so nothing outside it should ever hold two ids for the
//! same title and have to reconcile them by hand.
//!
//! These are deliberately distinct types rather than bare `u32`s. Mixing up an AniList id
//! and a MAL id produces a plausible-looking wrong answer rather than an error, which is
//! exactly the class of bug that is miserable to find later.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! numeric_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn new(raw: u32) -> Self {
                Self(raw)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<u32> for $name {
            fn from(raw: u32) -> Self {
                Self(raw)
            }
        }

        // The number itself on the far side of an FFI, under the type's name: Swift and Kotlin
        // see a type alias for the integer, so the bindings still say which id a value is.
        #[cfg(feature = "ffi")]
        uniffi::custom_newtype!($name, u32);
    };
}

numeric_id! {
    /// AniList media id — the canonical key for a title throughout anistream.
    AnilistId
}

numeric_id! {
    /// MyAnimeList id. Needed by aniskip and by MAL-based trackers.
    MalId
}

numeric_id! {
    /// Kitsu id.
    KitsuId
}

numeric_id! {
    /// TheTVDB id. Season/absolute episode numbering here needs `episode_offset`.
    TvdbId
}

numeric_id! {
    /// AniDB id.
    AnidbId
}

numeric_id! {
    /// The Movie Database id.
    TmdbId
}

impl AnilistId {
    /// Where ids anistream made up for titles AniList does not have begin. AniList's own ids are
    /// far below this, so one of ours can never collide with a real title.
    pub const OWN_BASE: u32 = 2_000_000_000;

    /// An id for a title with its own metadata, such as files kept unmatched. `n` counts from zero.
    pub const fn own(n: u32) -> Self {
        Self(Self::OWN_BASE + n)
    }

    /// Whether this id belongs to a title with its own metadata. Such a title is never written to
    /// a tracker, because no tracker knows it.
    pub const fn is_own(self) -> bool {
        self.0 >= Self::OWN_BASE
    }
}

/// The ids other services know a title by. A source that keeps its own index answers a title's
/// episodes from these, with no fuzzy matching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct ExternalIds {
    pub anilist: Option<AnilistId>,
    pub mal: Option<MalId>,
    pub anidb: Option<AnidbId>,
    pub tvdb: Option<TvdbId>,
    pub tmdb: Option<TmdbId>,
}

impl ExternalIds {
    /// Whether no id is set.
    pub const fn is_empty(&self) -> bool {
        self.anilist.is_none()
            && self.mal.is_none()
            && self.anidb.is_none()
            && self.tvdb.is_none()
            && self.tmdb.is_none()
    }

    /// Just an AniList id.
    pub const fn anilist(id: AnilistId) -> Self {
        Self { anilist: Some(id), mal: None, anidb: None, tvdb: None, tmdb: None }
    }
}

/// A provider's own opaque identifier for a title.
///
/// Deliberately an unparsed string: site catalogues use opaque hashes, torrent sources use search
/// terms, a remote HTTP provider may use anything. The core makes no assumptions and
/// never tries to interpret one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProviderKey(pub String);

impl ProviderKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for ProviderKey {
    fn from(key: &str) -> Self {
        Self(key.to_owned())
    }
}

impl From<String> for ProviderKey {
    fn from(key: String) -> Self {
        Self(key)
    }
}

// The string itself on the far side of an FFI, under the type's name, as the numeric ids are.
#[cfg(feature = "ffi")]
uniffi::custom_newtype!(ProviderKey, String);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_not_interchangeable() {
        // The point of the newtypes: this is a compile-time distinction. If these
        // were bare u32s, passing a MAL id where an AniList id belongs would silently
        // fetch the wrong anime.
        let anilist = AnilistId::new(154_587);
        let mal = MalId::new(52_991);
        assert_eq!(anilist.get(), 154_587);
        assert_eq!(mal.get(), 52_991);
        assert_eq!(anilist.to_string(), "154587");
    }

    #[test]
    fn own_ids_sit_above_every_anilist_id() {
        assert!(!AnilistId::new(190_000).is_own());
        assert!(AnilistId::own(0).is_own());
        assert_eq!(AnilistId::own(7).get(), AnilistId::OWN_BASE + 7);
    }

    #[test]
    fn external_ids_know_when_they_are_empty() {
        assert!(ExternalIds::default().is_empty());
        assert!(!ExternalIds::anilist(AnilistId::new(1)).is_empty());
    }

    #[test]
    fn provider_key_round_trips_through_json() {
        let key = ProviderKey::new("ReooPAxPMsHM4KPMY");
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"ReooPAxPMsHM4KPMY\"");
        assert_eq!(serde_json::from_str::<ProviderKey>(&json).unwrap(), key);
    }
}
