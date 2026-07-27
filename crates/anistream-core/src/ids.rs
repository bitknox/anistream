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
    fn provider_key_round_trips_through_json() {
        let key = ProviderKey::new("ReooPAxPMsHM4KPMY");
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"ReooPAxPMsHM4KPMY\"");
        assert_eq!(serde_json::from_str::<ProviderKey>(&json).unwrap(), key);
    }
}
