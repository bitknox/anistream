//! Trakt.
//!
//! The awkward one, and worth being honest about why rather than pretending otherwise: **Trakt is
//! not an anime tracker.** It tracks television, and anime appears in it as ordinary shows keyed on
//! TVDB and TMDB ids. That has two consequences this module cannot engineer away:
//!
//! - **Identity comes from `tvdb_id`.** Measured against the real 22,374-title mapping table:
//!   20,625 titles carry a `mal_id` (which is what AniList, MAL and Simkl key on) and only **7,225
//!   carry a TVDB id**. Two thirds of the corpus cannot reach Trakt at all.
//! - **Episode numbering is per-season, and usually unknowable.** Trakt wants `S2E03`; anistream
//!   counts absolutely, because that is how fansubs number and how AniList splits cours. The
//!   datasets' `episode_offset` bridges it — but of the 4,196 TVDB-mapped entries that *share* a
//!   series with another entry, only 590 have one. So for **3,606 titles** there is no way to say
//!   which season an episode belongs to.
//!
//! That second number is why this module refuses rather than assuming season one. Writing absolute
//! episode numbers into a first season would mark unrelated episodes as watched on somebody's real
//! account, and silently: 26 episodes into a third season would land on the first season's finale.
//! A skipped push is recoverable and visible in the log; wrong data is neither.
//!
//! Net effect: Trakt works correctly for roughly **3,600 titles** — the single-season ones — and
//! declines the rest. It is worth having if you already live in Trakt. It is not a replacement for
//! AniList, MAL or Simkl, all three of which cover 92% of the same corpus with no ambiguity.

use std::sync::Arc;

use anistream_core::{
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, Tracker, WatchStatus},
};

use crate::secret::TokenPair;

const API: &str = "https://api.trakt.tv";
/// Trakt requires this header and rejects requests without it.
const API_VERSION: &str = "2";

/// What Trakt needs to identify an episode: a show, and a season-relative number.
pub trait SeasonMapping: Send + Sync {
    /// The TVDB id for a title, if the mapping layer knows one.
    fn tvdb_id(&self, anilist_id: AnilistId) -> Option<u32>;

    /// Turn an absolute episode number into `(season, episode)`, or `None` if it cannot be placed.
    ///
    /// **`None` is not an edge case here, it is half the corpus.** Measured against the real mapping
    /// table: 7,225 titles carry a TVDB id, 4,196 of them share that id with another AniList entry —
    /// because TVDB keeps a sequel in the same series where AniList splits it — and only 590 of those
    /// have the `episode_offset` needed to say which season they are. That leaves 3,606 titles where
    /// assuming season 1 would write absolute episode numbers into the first season: watching S3E02
    /// would mark something entirely different as seen.
    ///
    /// So an ambiguous title is refused rather than guessed. A skipped push is recoverable; wrong
    /// data written to somebody's account is not.
    fn season_episode(&self, anilist_id: AnilistId, absolute: u32) -> Option<(u32, u32)>;

    /// The reverse, for reading a library back.
    fn absolute_episode(&self, anilist_id: AnilistId, season: u32, episode: u32) -> u32;

    fn anilist_for_tvdb(&self, tvdb_id: u32) -> Option<AnilistId>;
}

pub struct TraktTracker {
    client_id: String,
    http: reqwest::Client,
    mapping: Arc<dyn SeasonMapping>,
    token: tokio::sync::RwLock<Option<TokenPair>>,
}

impl TraktTracker {
    pub fn new(
        client_id: impl Into<String>,
        http: reqwest::Client,
        mapping: Arc<dyn SeasonMapping>,
        token: Option<TokenPair>,
    ) -> Self {
        Self {
            client_id: client_id.into(),
            http,
            mapping,
            token: tokio::sync::RwLock::new(token),
        }
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
    ) -> Result<reqwest::RequestBuilder, anistream_core::Error> {
        let token = self
            .token
            .read()
            .await
            .as_ref()
            .map(|pair| pair.access.clone())
            .ok_or_else(|| anistream_core::Error::Auth("trakt: not signed in".into()))?;
        Ok(self
            .http
            .request(method, format!("{API}{path}"))
            .header("trakt-api-key", &self.client_id)
            .header("trakt-api-version", API_VERSION)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json"))
    }
}

#[async_trait::async_trait]
impl Tracker for TraktTracker {
    fn id(&self) -> &str {
        "trakt"
    }

    fn is_authenticated(&self) -> bool {
        self.token.try_read().map_or(true, |held| held.is_some())
    }

    async fn accept_credentials(
        &self,
        access: &str,
        refresh: Option<&str>,
        expires_at: Option<i64>,
    ) {
        *self.token.write().await = Some(TokenPair {
            access: access.to_owned(),
            refresh: refresh.map(str::to_owned),
            expires_at,
        });
    }

    async fn forget_credentials(&self) {
        *self.token.write().await = None;
    }

    async fn pull_library(&self) -> Result<Vec<TrackedEntry>, anistream_core::Error> {
        let response = self
            .request(reqwest::Method::GET, "/sync/watched/shows")
            .await?
            .send()
            .await
            .map_err(|e| anistream_core::Error::Tracker {
                tracker: "trakt".into(),
                message: e.to_string(),
            })?;

        if response.status().as_u16() == 401 {
            return Err(anistream_core::Error::Auth("trakt rejected the token".into()));
        }
        let body: serde_json::Value = response.json().await.map_err(|e| {
            anistream_core::Error::Tracker { tracker: "trakt".into(), message: e.to_string() }
        })?;

        let mut entries = Vec::new();
        for item in body.as_array().into_iter().flatten() {
            let Some(tvdb_id) = item["show"]["ids"]["tvdb"].as_u64() else { continue };
            let Some(anilist_id) = self.mapping.anilist_for_tvdb(tvdb_id as u32) else {
                tracing::debug!(tvdb_id, "trakt show has no anilist mapping; skipped");
                continue;
            };

            // Trakt reports per-season episode lists. The highest watched episode across every
            // season, converted back to absolute numbering, is the progress figure — taking the
            // count instead would be wrong for a show with gaps.
            let mut progress = 0;
            for season in item["seasons"].as_array().into_iter().flatten() {
                let number = season["number"].as_u64().unwrap_or(1) as u32;
                for episode in season["episodes"].as_array().into_iter().flatten() {
                    let watched = episode["number"].as_u64().unwrap_or(0) as u32;
                    progress = progress
                        .max(self.mapping.absolute_episode(anilist_id, number, watched));
                }
            }

            entries.push(TrackedEntry {
                anilist_id,
                progress,
                // `/sync/watched` says what has been seen, not which list it is on. Reporting
                // `Current` is the honest reading; guessing `Completed` from an episode count would
                // need a total Trakt does not give here.
                status: WatchStatus::Current,
                score: None,
            });
        }
        tracing::info!(count = entries.len(), "trakt library pulled");
        Ok(entries)
    }

    async fn push(&self, ops: &[TrackOp]) -> Result<(), anistream_core::Error> {
        if !self.is_authenticated() {
            return Err(anistream_core::Error::Auth("signed out".into()));
        }

        let mut shows = Vec::new();
        for op in ops {
            // Progress only. Trakt's list semantics do not line up with a watch status — a show is
            // in your watchlist or it is not — and inventing a mapping would write something the
            // user did not ask for.
            let TrackOp::SetProgress { anilist_id, episode } = op else { continue };
            let Some(tvdb_id) = self.mapping.tvdb_id(*anilist_id) else {
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "no tvdb id, so trakt cannot be told about this title"
                );
                continue;
            };
            let Some((season, number)) = self.mapping.season_episode(*anilist_id, *episode)
            else {
                // Refused, not guessed. This title shares a TVDB series with other seasons and the
                // datasets carry no offset, so there is no way to say which season the episode is
                // in — and assuming the first would mark an unrelated episode as watched.
                tracing::warn!(
                    anilist_id = anilist_id.get(),
                    "trakt: cannot place this episode in a season, so it was not pushed"
                );
                continue;
            };
            shows.push(serde_json::json!({
                "ids": { "tvdb": tvdb_id },
                "seasons": [{
                    "number": season,
                    // Every episode up to this point, like Simkl: Trakt records a history of
                    // episodes rather than a progress counter.
                    "episodes": (1..=number)
                        .map(|n| serde_json::json!({ "number": n }))
                        .collect::<Vec<_>>(),
                }],
            }));
        }

        if shows.is_empty() {
            return Ok(());
        }
        let response = self
            .request(reqwest::Method::POST, "/sync/history")
            .await?
            .json(&serde_json::json!({ "shows": shows }))
            .send()
            .await
            .map_err(|e| anistream_core::Error::Tracker {
                tracker: "trakt".into(),
                message: e.to_string(),
            })?;

        let status = response.status();
        if status.as_u16() == 401 {
            return Err(anistream_core::Error::Auth("trakt rejected the token".into()));
        }
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anistream_core::Error::Tracker {
                tracker: "trakt".into(),
                message: format!("{} {}", status.as_u16(), body.trim()),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed {
        tvdb: Option<u32>,
        /// Episodes in seasons before this one, as the datasets' `episode_offset` gives it.
        offset: u32,
    }

    impl SeasonMapping for Fixed {
        fn tvdb_id(&self, _: AnilistId) -> Option<u32> {
            self.tvdb
        }
        fn season_episode(&self, _: AnilistId, absolute: u32) -> Option<(u32, u32)> {
            if self.offset == 0 {
                Some((1, absolute))
            } else {
                Some((2, absolute.saturating_sub(self.offset)))
            }
        }
        fn absolute_episode(&self, _: AnilistId, season: u32, episode: u32) -> u32 {
            if season <= 1 { episode } else { episode + self.offset }
        }
        fn anilist_for_tvdb(&self, _: u32) -> Option<AnilistId> {
            Some(AnilistId::new(1))
        }
    }

    #[test]
    fn an_absolute_episode_becomes_season_relative() {
        // The problem this whole trait exists for: anistream counts absolutely because fansubs do,
        // and Trakt wants S2E03. Sending 15 as season 1 episode 15 marks the wrong episode.
        let mapping = Fixed { tvdb: Some(1234), offset: 12 };
        assert_eq!(mapping.season_episode(AnilistId::new(1), 15), Some((2, 3)));
        assert_eq!(mapping.absolute_episode(AnilistId::new(1), 2, 3), 15);
    }

    #[test]
    fn with_no_offset_everything_is_season_one() {
        // The documented assumption. Stated in a test so it is a known limitation rather than a
        // surprise: a split-cour show with no dataset offset syncs to season 1.
        let mapping = Fixed { tvdb: Some(1), offset: 0 };
        assert_eq!(mapping.season_episode(AnilistId::new(1), 15), Some((1, 15)));
    }

    #[tokio::test]
    async fn a_title_with_no_tvdb_id_is_skipped_rather_than_guessed() {
        // TVDB coverage is much thinner than MAL's, so this is the common case rather than an edge
        // one — and inventing an id would write progress onto somebody else's show.
        let tracker = TraktTracker::new(
            "id",
            reqwest::Client::new(),
            Arc::new(Fixed { tvdb: None, offset: 0 }),
            Some(TokenPair { access: "t".into(), refresh: None, expires_at: None }),
        );
        // No network happens: with nothing mappable there is nothing to send.
        let result = tracker
            .push(&[TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 3 }])
            .await;
        assert!(result.is_ok(), "an unmappable title is not an error, got {result:?}");
    }

    #[tokio::test]
    async fn a_signed_out_tracker_refuses_before_touching_the_network() {
        let tracker = TraktTracker::new(
            "id",
            reqwest::Client::new(),
            Arc::new(Fixed { tvdb: Some(1), offset: 0 }),
            None,
        );
        let error = tracker
            .push(&[TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 1 }])
            .await
            .unwrap_err();
        assert!(matches!(error, anistream_core::Error::Auth(_)), "got {error:?}");
    }
}
