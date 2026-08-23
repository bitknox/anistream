//! The metadata failover: AniList, then Tenrai, then what was cached.
//!
//! Freshness first, always. This application exists for what is airing *now*, so a
//! fallback that quietly served stale data while a fresh source could answer would be
//! worse than the outage it papers over. The ladder only descends on failure: AniList
//! is the canonical source, Tenrai is a different well of the same fresh water — keyed
//! by `mal_id` and translated home through the mapping table — and the cache is the
//! last resort that exists so an AniList outage can never stop an episode you have
//! watched thirty times from playing.
//!
//! Two calls never descend, deliberately. The airing calendar needs exact timestamps
//! and Tenrai publishes only broadcast weekdays — a wrong countdown is worse than a
//! missing one. And `last_aired` has no equivalent anywhere else; absent beats guessed.
//!
//! Every successful fetch, from either fresh source, rewrites the cache. The cache is
//! therefore exactly as stale as the last outage was long — nothing ages in it while
//! the network works.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU8, Ordering};

use anistream_core::ids::AnilistId;
use anistream_store::Store;

use crate::{
    anilist::{AiringEntry, AniList, AniListError, BrowseFilter, LastAired, Media, Page, Season},
    tenrai::Tenrai,
};

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("{0}")]
    Anilist(#[from] AniListError),
    /// AniList failed, and so did every rung below it. The primary error leads, because
    /// it is the one whose outage this ladder exists to survive.
    #[error("anilist: {anilist}; fallback: {fallback}")]
    Exhausted { anilist: AniListError, fallback: String },
}

type Result<T> = std::result::Result<T, MetaError>;

/// Which rung answered most recently. Only transitions are worth a toast.
const FRESH: u8 = 0;
const TENRAI: u8 = 1;
const CACHE: u8 = 2;

/// One rung's condition, for the Providers screen.
#[derive(Debug, Clone)]
pub struct SourceHealth {
    pub source: &'static str,
    /// One word the screen can print: `live`, `serving`, `standby`, `failing`, …
    pub state: &'static str,
    pub healthy: bool,
    /// Whether this rung answered the most recent request.
    pub active: bool,
    /// The last error, or the cache's size — whatever explains the state.
    pub detail: Option<String>,
}

/// The last error each fresh source produced, cleared on its next success.
#[derive(Default)]
struct Weather {
    anilist: Option<String>,
    tenrai: Option<String>,
}

/// The metadata source the application talks to, in place of any one service.
#[derive(Clone)]
pub struct Meta {
    anilist: AniList,
    tenrai: Option<std::sync::Arc<Tenrai>>,
    store: Store,
    rung: std::sync::Arc<AtomicU8>,
    notice: std::sync::Arc<Mutex<Option<String>>>,
    weather: std::sync::Arc<Mutex<Weather>>,
}

impl Meta {
    pub fn new(anilist: AniList, tenrai: Option<Tenrai>, store: Store) -> Self {
        Self {
            anilist,
            tenrai: tenrai.map(std::sync::Arc::new),
            store,
            rung: std::sync::Arc::new(AtomicU8::new(FRESH)),
            notice: std::sync::Arc::new(Mutex::new(None)),
            weather: std::sync::Arc::new(Mutex::new(Weather::default())),
        }
    }

    /// Every rung's condition, for the Providers screen: is the ladder standing on the
    /// ground floor, and if not, why not. "Has it died?" should never need a log file.
    pub fn health(&self) -> Vec<SourceHealth> {
        let rung = self.rung.load(Ordering::Relaxed);
        let weather = self.weather.lock().map(|w| (w.anilist.clone(), w.tenrai.clone()));
        let (anilist_err, tenrai_err) = weather.unwrap_or((None, None));

        let mut rows = Vec::with_capacity(3);
        rows.push(SourceHealth {
            source: "anilist",
            state: if anilist_err.is_none() { "live" } else { "unreachable" },
            healthy: anilist_err.is_none(),
            active: rung == FRESH,
            detail: anilist_err,
        });
        rows.push(match &self.tenrai {
            None => SourceHealth {
                source: "tenrai",
                state: "off",
                healthy: true,
                active: false,
                detail: Some("meta.fallback = false".into()),
            },
            Some(_) => SourceHealth {
                source: "tenrai",
                state: match (rung == TENRAI, tenrai_err.is_none()) {
                    (true, _) => "serving",
                    (false, true) => "standby",
                    (false, false) => "failing",
                },
                healthy: tenrai_err.is_none(),
                active: rung == TENRAI,
                detail: tenrai_err,
            },
        });
        let cached = self.store.cached_media_count().unwrap_or(0);
        rows.push(SourceHealth {
            source: "cache",
            state: if rung == CACHE { "serving" } else { "ready" },
            // An empty cache is only a problem when it is what everything depends on.
            healthy: cached > 0 || rung != CACHE,
            active: rung == CACHE,
            detail: Some(match cached {
                1 => "1 title".into(),
                n => format!("{n} titles"),
            }),
        });
        rows
    }

    fn note_anilist(&self, error: Option<String>) {
        if let Ok(mut weather) = self.weather.lock() {
            weather.anilist = error;
        }
    }

    fn note_tenrai(&self, error: Option<String>) {
        if let Ok(mut weather) = self.weather.lock() {
            weather.tenrai = error;
        }
    }

    /// The lower rungs' collective failure, spelled out. A full-ladder failure that only
    /// mentioned AniList read exactly like the fallback not existing.
    fn exhausted(&self, anilist: AniListError, tenrai_error: Option<String>) -> MetaError {
        let fallback = match (self.tenrai.is_some(), tenrai_error) {
            (false, _) => "tenrai is off; nothing useful cached".to_owned(),
            (true, Some(e)) => format!("tenrai: {e}; nothing useful cached"),
            (true, None) => "nothing useful cached".to_owned(),
        };
        MetaError::Exhausted { anilist, fallback }
    }

    /// The AniList client itself, for the calls that have no fallback semantics.
    pub fn anilist(&self) -> &AniList {
        &self.anilist
    }

    /// How long the primary's rate limiter would stall a request right now.
    ///
    /// The task scheduler uses this to defer non-urgent work; it deliberately reads the
    /// primary's budget, because that is the source the work will hit first.
    pub async fn rate_limit_wait(&self) -> Option<std::time::Duration> {
        self.anilist.rate_limit_wait().await
    }

    /// The one-line degradation notice, at most once per transition.
    ///
    /// Taken, not peeked: the caller toasts it exactly once. Transitions in both
    /// directions speak — silently recovering would leave the last thing said untrue.
    pub fn take_notice(&self) -> Option<String> {
        self.notice.lock().ok()?.take()
    }

    fn went(&self, rung: u8) {
        let was = self.rung.swap(rung, Ordering::Relaxed);
        if was == rung {
            return;
        }
        let message = match rung {
            TENRAI => "anilist is unreachable — fresh data via tenrai for now",
            CACHE => "no metadata source answered — showing what was last cached",
            _ => "anilist is back",
        };
        if let Ok(mut slot) = self.notice.lock() {
            *slot = Some(message.to_owned());
        }
    }

    /// Whether this failure is the ladder's business, or an answer to respect.
    ///
    /// Not-found is a fact about the title, not the service; falling back on it would
    /// re-ask a question that was already answered. Unauthenticated cannot happen on
    /// these unauthenticated reads, but if it ever does, another source is not the fix.
    fn descends(error: &AniListError) -> bool {
        matches!(
            error,
            AniListError::Network(_) | AniListError::Api(_) | AniListError::Decode(_)
        )
    }

    /// Write-through: every successful fetch leaves the cache exactly this fresh.
    fn remember(&self, media: &Media) {
        let Ok(payload) = serde_json::to_string(media) else { return };
        if let Err(e) = self.store.cache_media(media.id, &payload, anistream_store::now()) {
            tracing::warn!(error = %e, "could not cache metadata");
        }
    }

    fn remember_page(&self, page: &Page<Media>) {
        for media in &page.items {
            self.remember(media);
        }
    }

    /// A cached row, with its clock made honest again.
    fn recall(&self, id: AnilistId) -> Option<Media> {
        let (payload, _) = self.store.cached_media(id).ok().flatten()?;
        let mut media: Media = serde_json::from_str(&payload).ok()?;
        // The stored countdown froze at fetch time; the air date did not move.
        if let Some(next) = &mut media.next_airing_episode {
            next.time_until_airing = next.airing_at - anistream_store::now();
        }
        Some(media)
    }

    /// One title, with the full detail-screen surface.
    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        let primary = match self.anilist.media(id).await {
            Ok(media) => {
                self.note_anilist(None);
                self.went(FRESH);
                self.remember(&media);
                return Ok(media);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => {
                self.note_anilist(Some(e.to_string()));
                e
            }
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match tenrai.media(id).await {
                Ok(media) => {
                    self.note_tenrai(None);
                    self.went(TENRAI);
                    self.remember(&media);
                    return Ok(media);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "tenrai could not answer either");
                    tenrai_error = Some(e.to_string());
                    self.note_tenrai(tenrai_error.clone());
                }
            }
        }
        if let Some(media) = self.recall(id) {
            self.went(CACHE);
            return Ok(media);
        }
        Err(self.exhausted(primary, tenrai_error))
    }

    /// Several titles, in the order asked. Fallback assembles per title — the cache
    /// first, because these are almost always titles from the caller's own history,
    /// then Tenrai for the gaps. A title nothing can describe is omitted, not invented.
    pub async fn media_many(&self, ids: &[AnilistId]) -> Result<Vec<Media>> {
        let primary = match self.anilist.media_many(ids).await {
            Ok(list) => {
                self.note_anilist(None);
                self.went(FRESH);
                for media in &list {
                    self.remember(media);
                }
                return Ok(list);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => {
                self.note_anilist(Some(e.to_string()));
                e
            }
        };

        let mut assembled = Vec::with_capacity(ids.len());
        let mut tenrai_error = None;
        let mut rung = CACHE;
        for id in ids {
            if let Some(media) = self.recall(*id) {
                assembled.push(media);
            } else if let Some(tenrai) = &self.tenrai {
                match tenrai.media(*id).await {
                    Ok(media) => {
                        self.remember(&media);
                        rung = TENRAI;
                        assembled.push(media);
                    }
                    Err(e) => tenrai_error = Some(e.to_string()),
                }
            }
        }
        if tenrai_error.is_none() && rung == TENRAI {
            self.note_tenrai(None);
        } else if tenrai_error.is_some() {
            self.note_tenrai(tenrai_error.clone());
        }
        if assembled.is_empty() && !ids.is_empty() {
            return Err(self.exhausted(primary, tenrai_error));
        }
        self.went(rung);
        Ok(assembled)
    }

    /// Filtered search. Tenrai applies what MAL can express; the rest is said, not faked.
    pub async fn search(
        &self,
        term: &str,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let primary = match self.anilist.search_filtered(term, filter, page, per_page).await {
            Ok(found) => {
                self.note_anilist(None);
                self.went(FRESH);
                self.remember_page(&found);
                return Ok(found);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => {
                self.note_anilist(Some(e.to_string()));
                e
            }
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match tenrai.search(term, filter, page, per_page).await {
                Ok(found) => {
                    self.note_tenrai(None);
                    self.went(TENRAI);
                    self.remember_page(&found);
                    return Ok(found);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "tenrai search failed too");
                    tenrai_error = Some(e.to_string());
                    self.note_tenrai(tenrai_error.clone());
                }
            }
        }
        // No cache rung for discovery: a stale search result claims titles exist that
        // may not match the query today, and this screen is where people expect fresh.
        Err(self.exhausted(primary, tenrai_error))
    }

    /// Seasonal browse. Tenrai's season endpoint takes no filters, so ours are applied
    /// to its answer client-side — fewer rows, never wrong rows.
    pub async fn seasonal(
        &self,
        season: Season,
        year: u16,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let primary = match self.anilist.seasonal(season, year, filter, page, per_page).await {
            Ok(found) => {
                self.note_anilist(None);
                self.went(FRESH);
                self.remember_page(&found);
                return Ok(found);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => {
                self.note_anilist(Some(e.to_string()));
                e
            }
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match tenrai.seasonal(season, year, page, per_page).await {
                Ok(mut found) => {
                    found.items.retain(|media| {
                        filter.genres.iter().all(|g| media.genres.contains(g))
                            && filter.format.as_deref().is_none_or(|f| {
                                media.format.map(|m| format!("{m:?}").to_uppercase())
                                    == Some(f.to_owned())
                            })
                    });
                    self.note_tenrai(None);
                    self.went(TENRAI);
                    self.remember_page(&found);
                    return Ok(found);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "tenrai seasonal failed too");
                    tenrai_error = Some(e.to_string());
                    self.note_tenrai(tenrai_error.clone());
                }
            }
        }
        Err(self.exhausted(primary, tenrai_error))
    }

    /// The calendar. AniList only: Tenrai publishes broadcast weekdays, not timestamps,
    /// and a wrong countdown is worse than a missing calendar.
    pub async fn airing_between(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Page<AiringEntry>> {
        self.weathered(self.anilist.airing_between(from, to, page, per_page).await)
    }

    /// The same window, with the sort direction chosen by the caller.
    pub async fn airing_between_sorted(
        &self,
        from: i64,
        to: i64,
        page: u32,
        per_page: u32,
        newest_first: bool,
    ) -> Result<Page<AiringEntry>> {
        self.weathered(
            self.anilist.airing_between_sorted(from, to, page, per_page, newest_first).await,
        )
    }

    /// When each title last aired. AniList only — nothing else knows; absent beats guessed.
    pub async fn last_aired(&self, ids: &[AnilistId]) -> Result<Vec<LastAired>> {
        self.weathered(self.anilist.last_aired(ids).await)
    }

    /// A passthrough still reports the weather: a calendar failure is the same outage
    /// the Providers screen should be able to explain.
    fn weathered<T>(&self, result: std::result::Result<T, AniListError>) -> Result<T> {
        match result {
            Ok(value) => {
                self.note_anilist(None);
                Ok(value)
            }
            Err(e) => {
                if Self::descends(&e) {
                    self.note_anilist(Some(e.to_string()));
                }
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_starts_on_the_ground_floor_with_everything_in_its_place() {
        let store = Store::open_in_memory().unwrap();
        let http =
            anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
                .unwrap();
        let anilist = AniList::new(http.clone(), 30);
        let meta = Meta::new(
            anilist,
            Some(crate::tenrai::Tenrai::new(http, store.clone())),
            store,
        );

        let rows = meta.health();
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].source, rows[0].state, rows[0].active), ("anilist", "live", true));
        assert_eq!((rows[1].source, rows[1].state, rows[1].active), ("tenrai", "standby", false));
        assert_eq!((rows[2].source, rows[2].state, rows[2].active), ("cache", "ready", false));
        assert_eq!(rows[2].detail.as_deref(), Some("0 titles"));
        assert!(rows.iter().all(|r| r.healthy));
    }

    #[test]
    fn a_disabled_fallback_says_so_instead_of_looking_broken() {
        let store = Store::open_in_memory().unwrap();
        let http =
            anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
                .unwrap();
        let meta = Meta::new(AniList::new(http, 30), None, store);
        let tenrai = &meta.health()[1];
        assert_eq!(tenrai.state, "off");
        assert!(tenrai.healthy, "off by choice is not unhealthy");
        assert_eq!(tenrai.detail.as_deref(), Some("meta.fallback = false"));
    }

    #[test]
    fn a_full_ladder_failure_names_every_rung() {
        // The regression this encodes: the error mentioned only AniList, which read
        // exactly like the fallback not existing.
        let store = Store::open_in_memory().unwrap();
        let http =
            anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
                .unwrap();
        let meta = Meta::new(
            AniList::new(http.clone(), 30),
            Some(crate::tenrai::Tenrai::new(http, store.clone())),
            store,
        );
        let error = meta.exhausted(
            AniListError::Network("timed out".into()),
            Some("504 gateway timeout".into()),
        );
        let text = error.to_string();
        assert!(text.contains("timed out"), "got {text:?}");
        assert!(text.contains("tenrai"), "got {text:?}");
        assert!(text.contains("504"), "got {text:?}");
        assert!(text.contains("cached"), "got {text:?}");
    }

    #[test]
    fn only_service_failures_descend_the_ladder() {
        // Not-found is an answer about the title; re-asking Tenrai would second-guess it.
        assert!(Meta::descends(&AniListError::Network("timed out".into())));
        assert!(Meta::descends(&AniListError::Api("internal error".into())));
        assert!(Meta::descends(&AniListError::Decode("shape changed".into())));
        assert!(!Meta::descends(&AniListError::NotFound));
        assert!(!Meta::descends(&AniListError::Unauthenticated));
    }
}
