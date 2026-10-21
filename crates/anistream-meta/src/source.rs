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
//!
//! A fresh source that keeps failing is left alone for a while. Each rung waits out its
//! own timeout before the next is tried, so with the network hanging every screen spent
//! a timeout on AniList and another on Tenrai before reaching the cache — forty seconds a
//! view. After a few failures in a row a [`Breaker`] skips the source for a cooldown, then
//! lets one request through to see whether it is back.
//!
//! One title asked for again within minutes is not asked of AniList again. Opening a title
//! asks for it three times over — its page, its episodes, then the play — and each was a
//! request against thirty a minute for an answer a moment old. See [`Recent`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anistream_core::ids::AnilistId;
use anistream_store::Store;

use crate::{
    anilist::{
        AiringEntry, AniList, AniListError, BrowseFilter, LastAired, Media, Page, Season,
    },
    tenrai::{Tenrai, TenraiError},
};

#[derive(Debug, Clone, thiserror::Error)]
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
    /// While the source is being skipped, when the next request goes through as a probe.
    ///
    /// A deadline rather than a countdown: the screen is redrawn on change, not every second,
    /// so "next try in 39s" formatted here read 39s for as long as the screen sat idle.
    pub retry_at: Option<SystemTime>,
}

/// The last error each fresh source produced, cleared on its next success.
#[derive(Default)]
struct Weather {
    anilist: Option<String>,
    tenrai: Option<String>,
}

/// Failures in a row that take a fresh source out of the ladder for a cooldown.
const BREAKER_THRESHOLD: u32 = 3;

/// The first cooldown. Each probe that fails doubles it, up to [`BREAKER_MAX`].
const BREAKER_COOLDOWN: Duration = Duration::from_secs(30);

/// The longest a source is left alone between probes. Short enough that a recovered
/// AniList is back on the ground floor within minutes without anyone pressing a key.
const BREAKER_MAX: Duration = Duration::from_secs(300);

/// How long AniList's answer for one title answers every ask for it again.
///
/// Long enough to cover a title being opened, its episodes listed and an episode played,
/// short enough that what a title page shows is never older than a few minutes. An episode
/// airing in the meantime ends it early: see [`Answered::serves`].
pub const REUSE_FOR: Duration = Duration::from_secs(5 * 60);

/// One ask for a title: under way, or answered.
///
/// Shared by everyone who asks for the title while it is under way, so a page and its
/// episode table opened together cost one request, and an outage one walk down the ladder
/// rather than one each.
type Flight = Arc<tokio::sync::OnceCell<Answered>>;

/// What an ask for a title came back with.
#[derive(Debug, Clone)]
struct Answered {
    at: Instant,
    outcome: Result<Media>,
    /// Whether AniList itself answered. Only its answers are asked for again from here: an
    /// answer from the fallback or the cache is for an outage, and the next ask should find
    /// out whether it is over.
    fresh: bool,
}

impl Answered {
    /// Whether this answer still answers an ask made `now`.
    ///
    /// Not once an episode has aired since: the next one and its countdown have moved, and
    /// the episode count with them.
    fn serves(&self, now: Instant, reuse_for: Duration) -> bool {
        let age = now.saturating_duration_since(self.at);
        let Ok(media) = &self.outcome else { return false };
        let aired = media.next_airing_episode.as_ref().is_some_and(|next| {
            next.time_until_airing <= i64::try_from(age.as_secs()).unwrap_or(i64::MAX)
        });
        self.fresh && age < reuse_for && !aired
    }

    /// The answer as of `now`, its countdown run on by the time since.
    fn as_of(&self, now: Instant) -> Result<Media> {
        let age =
            i64::try_from(now.saturating_duration_since(self.at).as_secs()).unwrap_or(i64::MAX);
        self.outcome.clone().map(|mut media| {
            if let Some(next) = &mut media.next_airing_episode {
                next.time_until_airing = next.time_until_airing.saturating_sub(age);
            }
            media
        })
    }
}

/// The asks for single titles under way, and AniList's answers from the last
/// [`REUSE_FOR`].
///
/// Kept beside the cache, not read from it: the cache holds whichever fetch touched a title
/// last, and a list fetch writes a title without its relations, recommendations, trailer or
/// episode names. A title page drawn from that row would be missing half of itself.
#[derive(Default)]
struct Recent {
    titles: HashMap<AnilistId, Flight>,
}

/// A circuit breaker for one fresh source.
///
/// Closed, every request goes through. After [`BREAKER_THRESHOLD`] failures in a row it
/// opens and requests are skipped, so the ladder goes straight to the next rung. When the
/// cooldown has run, one request goes through as a probe while the rest keep skipping: a
/// success closes the breaker, a failure opens it again for twice as long.
#[derive(Debug, Default)]
struct Breaker {
    failures: u32,
    /// Times it has opened since the source last answered, which sets the cooldown.
    trips: u32,
    open_until: Option<Instant>,
}

impl Breaker {
    /// Whether a request may go to the source now. Past the cooldown this lets one probe
    /// through and holds everyone else for another cooldown while it is out.
    fn allows(&mut self, now: Instant) -> bool {
        match self.open_until {
            None => true,
            Some(until) if now >= until => {
                self.open_until = Some(now + self.cooldown());
                true
            }
            Some(_) => false,
        }
    }

    /// The source answered — with data, or with an answer such as "not found".
    fn answered(&mut self) {
        *self = Self::default();
    }

    /// The source failed: unreachable, erroring, or answering in a shape we cannot read.
    fn failed(&mut self, now: Instant) {
        self.failures = self.failures.saturating_add(1);
        if self.failures >= BREAKER_THRESHOLD {
            self.trips = self.trips.saturating_add(1);
            self.open_until = Some(now + self.cooldown());
        }
    }

    fn cooldown(&self) -> Duration {
        let doublings = self.trips.saturating_sub(1).min(8);
        (BREAKER_COOLDOWN * 2_u32.pow(doublings)).min(BREAKER_MAX)
    }

    /// How long until the next probe, while the source is being skipped.
    fn resting_for(&self, now: Instant) -> Option<Duration> {
        self.open_until.filter(|until| *until > now).map(|until| until - now)
    }
}

/// One breaker per fresh source.
#[derive(Debug, Default)]
struct Breakers {
    anilist: Breaker,
    tenrai: Breaker,
}

/// What a skipped request says instead of asking.
fn skipped(source: &str, wait: Duration) -> String {
    format!("{source} skipped after repeated failures; next try in {}s", wait.as_secs().max(1))
}

/// The metadata source the application talks to, in place of any one service.
#[derive(Clone)]
pub struct Meta {
    /// Which form of each title is shown, shared with every clone.
    titles: crate::titles::Titles,
    anilist: AniList,
    tenrai: Option<std::sync::Arc<Tenrai>>,
    store: Store,
    rung: std::sync::Arc<AtomicU8>,
    notice: std::sync::Arc<Mutex<Option<String>>>,
    weather: std::sync::Arc<Mutex<Weather>>,
    breakers: std::sync::Arc<Mutex<Breakers>>,
    recent: Arc<Mutex<Recent>>,
    /// How long one of AniList's answers for a title is used again: [`REUSE_FOR`].
    reuse_for: Duration,
}

impl Meta {
    pub fn new(anilist: AniList, tenrai: Option<Tenrai>, store: Store) -> Self {
        Self {
            titles: crate::titles::Titles::default(),
            anilist,
            tenrai: tenrai.map(std::sync::Arc::new),
            store,
            rung: std::sync::Arc::new(AtomicU8::new(FRESH)),
            notice: std::sync::Arc::new(Mutex::new(None)),
            weather: std::sync::Arc::new(Mutex::new(Weather::default())),
            breakers: std::sync::Arc::new(Mutex::new(Breakers::default())),
            recent: Arc::default(),
            reuse_for: REUSE_FOR,
        }
    }

    /// Which form of each title is shown: the setting, and the signed-in viewer's own choice
    /// when the setting follows AniList.
    pub fn titles(&self) -> &crate::titles::Titles {
        &self.titles
    }

    /// This source, using AniList's answer for a title again for `reuse_for` rather than
    /// [`REUSE_FOR`]. Zero asks every time, for a test of the ladder beneath; asks made
    /// together are still asked once.
    #[must_use]
    pub fn reusing_for(self, reuse_for: Duration) -> Self {
        Self { reuse_for, ..self }
    }

    /// Every rung's condition, for the Providers screen: is the ladder standing on the
    /// ground floor, and if not, why not. "Has it died?" should never need a log file.
    pub fn health(&self) -> Vec<SourceHealth> {
        let rung = self.rung.load(Ordering::Relaxed);
        let weather = self.weather.lock().map(|w| (w.anilist.clone(), w.tenrai.clone()));
        let (anilist_err, tenrai_err) = weather.unwrap_or((None, None));

        let now = Instant::now();
        let (anilist_rest, tenrai_rest) = self
            .breakers
            .lock()
            .map(|b| (b.anilist.resting_for(now), b.tenrai.resting_for(now)))
            .unwrap_or((None, None));

        let mut rows = Vec::with_capacity(3);
        rows.push(SourceHealth {
            source: "anilist",
            state: match (anilist_rest, &anilist_err) {
                (Some(_), _) => "backing off",
                (None, None) => "live",
                (None, Some(_)) => "unreachable",
            },
            healthy: anilist_err.is_none() && anilist_rest.is_none(),
            active: rung == FRESH,
            // The cause, beside the deadline: the screen says that requests are being skipped
            // and until when, so a screen loading from the fallback does not read as AniList
            // being ignored.
            detail: match anilist_rest {
                Some(_) => Some(anilist_err.unwrap_or_else(|| "failing".into())),
                None => anilist_err,
            },
            retry_at: anilist_rest.map(|wait| SystemTime::now() + wait),
        });
        rows.push(match &self.tenrai {
            None => SourceHealth {
                source: "tenrai",
                state: "off",
                healthy: true,
                active: false,
                detail: Some("meta.fallback = false".into()),
                retry_at: None,
            },
            Some(_) => SourceHealth {
                source: "tenrai",
                state: match (rung == TENRAI, tenrai_err.is_none(), tenrai_rest.is_some()) {
                    (true, _, _) => "serving",
                    (false, _, true) => "backing off",
                    (false, true, false) => "standby",
                    (false, false, false) => "failing",
                },
                healthy: tenrai_err.is_none() && tenrai_rest.is_none(),
                active: rung == TENRAI,
                detail: match tenrai_rest {
                    Some(_) => Some(tenrai_err.unwrap_or_else(|| "failing".into())),
                    None => tenrai_err,
                },
                retry_at: tenrai_rest.map(|wait| SystemTime::now() + wait),
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
            retry_at: None,
        });
        rows
    }

    fn note_anilist(&self, error: Option<String>) {
        if let Ok(mut weather) = self.weather.lock() {
            weather.anilist = error;
        }
    }

    /// Tenrai's weather is noted by the callers, which see skips as errors too. While it is
    /// resting the error that put it there is kept, so a skip cannot stand in for the cause.
    fn note_tenrai(&self, error: Option<String>) {
        if error.is_some() && self.resting("tenrai") {
            return;
        }
        if let Ok(mut weather) = self.weather.lock() {
            weather.tenrai = error;
        }
    }

    fn resting(&self, source: &str) -> bool {
        self.breakers
            .lock()
            .map(|mut b| self.breaker(&mut b, source).resting_for(Instant::now()).is_some())
            .unwrap_or(false)
    }

    /// Ask AniList, unless it has failed often enough lately to be skipped.
    ///
    /// The call is a future, so a skipped one is never polled and never sends anything.
    /// Its outcome is noted here — the breaker, and the weather the Providers screen reads —
    /// so a skip cannot overwrite the error that caused it.
    async fn ask_anilist<T>(
        &self,
        call: impl std::future::Future<Output = std::result::Result<T, AniListError>>,
    ) -> std::result::Result<T, AniListError> {
        if let Some(wait) = self.skip("anilist") {
            return Err(AniListError::Network(skipped("anilist", wait)));
        }
        let result = call.await;
        match &result {
            Err(e) if Self::descends(e) => {
                self.note_anilist(Some(e.to_string()));
                self.trip("anilist");
            }
            Ok(_) => {
                self.note_anilist(None);
                self.reset("anilist");
            }
            // An answer about the title, which says the service is up.
            Err(_) => self.reset("anilist"),
        }
        result
    }

    /// Ask Tenrai, on the same terms as [`Self::ask_anilist`]. Its weather is noted by the
    /// callers, which aggregate it across titles.
    async fn ask_tenrai<T>(
        &self,
        call: impl std::future::Future<Output = std::result::Result<T, TenraiError>>,
    ) -> std::result::Result<T, TenraiError> {
        if let Some(wait) = self.skip("tenrai") {
            return Err(TenraiError::Network(skipped("tenrai", wait)));
        }
        let result = call.await;
        match &result {
            Err(TenraiError::Network(_) | TenraiError::Api(_)) => self.trip("tenrai"),
            _ => self.reset("tenrai"),
        }
        result
    }

    /// How long a source still has to rest, or `None` to go ahead (claiming the probe).
    fn skip(&self, source: &str) -> Option<Duration> {
        let now = Instant::now();
        let mut breakers = self.breakers.lock().ok()?;
        let breaker = self.breaker(&mut breakers, source);
        if breaker.allows(now) { None } else { breaker.resting_for(now) }
    }

    fn trip(&self, source: &str) {
        if let Ok(mut breakers) = self.breakers.lock() {
            let breaker = self.breaker(&mut breakers, source);
            let was_open = breaker.resting_for(Instant::now()).is_some();
            breaker.failed(Instant::now());
            if !was_open && breaker.resting_for(Instant::now()).is_some() {
                tracing::warn!(source, cooldown = ?breaker.cooldown(), "failing repeatedly; skipping it for a while");
            }
        }
    }

    fn reset(&self, source: &str) {
        if let Ok(mut breakers) = self.breakers.lock() {
            self.breaker(&mut breakers, source).answered();
        }
    }

    fn breaker<'a>(&self, breakers: &'a mut Breakers, source: &str) -> &'a mut Breaker {
        if source == "tenrai" { &mut breakers.tenrai } else { &mut breakers.anilist }
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
    ///
    /// From AniList's answer of the last few minutes when there is one, and otherwise asked,
    /// once for everyone asking at the same time: see [`Recent`].
    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        let flight = self.flight(id);
        let answered = flight
            .get_or_init(|| async {
                let (outcome, fresh) = self.ask_media(id).await;
                Answered { at: Instant::now(), outcome, fresh }
            })
            .await;
        answered.as_of(Instant::now())
    }

    /// The ask for `id` to join: one under way, or an answer that still serves, or else a
    /// new one. Answers that serve no longer are let go on the way.
    fn flight(&self, id: AnilistId) -> Flight {
        let now = Instant::now();
        let Ok(mut recent) = self.recent.lock() else { return Flight::default() };
        recent.titles.retain(|_, flight| {
            flight.get().is_none_or(|answered| answered.serves(now, self.reuse_for))
        });
        Arc::clone(recent.titles.entry(id).or_default())
    }

    /// The ladder, for one title. Returns whether AniList itself answered.
    async fn ask_media(&self, id: AnilistId) -> (Result<Media>, bool) {
        let primary = match self.ask_anilist(self.anilist.media(id)).await {
            Ok(media) => {
                self.went(FRESH);
                self.remember(&media);
                return (Ok(media), true);
            }
            Err(e) if !Self::descends(&e) => return (Err(e.into()), false),
            Err(e) => e,
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match self.ask_tenrai(tenrai.media(id)).await {
                Ok(media) => {
                    self.note_tenrai(None);
                    self.went(TENRAI);
                    self.remember(&media);
                    return (Ok(media), false);
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
            return (Ok(media), false);
        }
        (Err(self.exhausted(primary, tenrai_error)), false)
    }

    /// Several titles, in the order asked. Fallback assembles per title — the cache
    /// first, because these are almost always titles from the caller's own history,
    /// then Tenrai for the gaps. A title nothing can describe is omitted, not invented.
    pub async fn media_many(&self, ids: &[AnilistId]) -> Result<Vec<Media>> {
        let primary = match self.ask_anilist(self.anilist.media_many(ids)).await {
            Ok(list) => {
                self.went(FRESH);
                for media in &list {
                    self.remember(media);
                }
                return Ok(list);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => e,
        };

        let mut assembled = Vec::with_capacity(ids.len());
        let mut tenrai_error = None;
        let mut rung = CACHE;
        for id in ids {
            if let Some(media) = self.recall(*id) {
                assembled.push(media);
            } else if let Some(tenrai) = &self.tenrai {
                match self.ask_tenrai(tenrai.media(*id)).await {
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
        let primary = match self
            .ask_anilist(self.anilist.search_filtered(term, filter, page, per_page))
            .await
        {
            Ok(found) => {
                self.went(FRESH);
                self.remember_page(&found);
                return Ok(found);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => e,
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match self.ask_tenrai(tenrai.search(term, filter, page, per_page)).await {
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
        let primary = match self
            .ask_anilist(self.anilist.seasonal(season, year, filter, page, per_page))
            .await
        {
            Ok(found) => {
                self.went(FRESH);
                self.remember_page(&found);
                return Ok(found);
            }
            Err(e) if !Self::descends(&e) => return Err(e.into()),
            Err(e) => e,
        };

        let mut tenrai_error = None;
        if let Some(tenrai) = &self.tenrai {
            match self.ask_tenrai(tenrai.seasonal(season, year, page, per_page)).await {
                Ok(mut found) => {
                    found.items.retain(|media| {
                        filter.genres.iter().all(|g| media.genres.contains(g))
                            // By AniList's name for the format, which the filter carries:
                            // the enum's name upper-cased is `TVSHORT`, not `TV_SHORT`.
                            && filter.format.as_deref().is_none_or(|f| {
                                media.format.and_then(|m| serde_json::to_value(m).ok())
                                    == Some(serde_json::Value::from(f))
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
        self.weathered(self.anilist.airing_between(from, to, page, per_page)).await
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
        self.weathered(self.anilist.airing_between_sorted(
            from,
            to,
            page,
            per_page,
            newest_first,
        ))
        .await
    }

    /// When each title last aired. AniList only — nothing else knows; absent beats guessed.
    pub async fn last_aired(&self, ids: &[AnilistId]) -> Result<Vec<LastAired>> {
        self.weathered(self.anilist.last_aired(ids)).await
    }

    /// What aired for the titles followed, after `from` and before `to`: the new-episode check.
    /// AniList only, as the calendar is: a notification for an episode that has not aired is
    /// worse than one a check later.
    pub async fn aired_since(
        &self,
        ids: &[AnilistId],
        from: i64,
        to: i64,
    ) -> Result<Page<AiringEntry>> {
        self.weathered(self.anilist.aired_since(ids, from, to)).await
    }

    /// A passthrough still reports the weather — a calendar failure is the same outage the
    /// Providers screen should be able to explain — and still skips a source that is
    /// resting, so the calendar and the broadcast lines fail at once rather than after a
    /// timeout each.
    async fn weathered<T>(
        &self,
        call: impl std::future::Future<Output = std::result::Result<T, AniListError>>,
    ) -> Result<T> {
        self.ask_anilist(call).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fallback's format filter compares against the filter's value, which is AniList's
    /// name for the format. The enum's own name upper-cased is `TVSHORT`, which matched nothing.
    #[test]
    fn a_format_compares_by_the_name_the_filter_uses() {
        let wire = serde_json::to_value(anistream_core::media::MediaFormat::TvShort).ok();
        assert_eq!(wire, Some(serde_json::Value::from("TV_SHORT")));
    }

    #[test]
    fn health_starts_on_the_ground_floor_with_everything_in_its_place() {
        let store = Store::open_in_memory().unwrap();
        let http =
            anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
                .unwrap();
        let anilist = AniList::new(http.clone(), 30);
        let meta =
            Meta::new(anilist, Some(crate::tenrai::Tenrai::new(http, store.clone())), store);

        let rows = meta.health();
        assert_eq!(rows.len(), 3);
        assert_eq!((rows[0].source, rows[0].state, rows[0].active), ("anilist", "live", true));
        assert_eq!(
            (rows[1].source, rows[1].state, rows[1].active),
            ("tenrai", "standby", false)
        );
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

    fn meta() -> Meta {
        let store = Store::open_in_memory().unwrap();
        let http =
            anistream_net::HttpClient::new(&anistream_core::config::NetworkConfig::default())
                .unwrap();
        Meta::new(
            AniList::new(http.clone(), 30),
            Some(crate::tenrai::Tenrai::new(http, store.clone())),
            store,
        )
    }

    #[test]
    fn a_breaker_opens_after_repeated_failures_and_probes_after_the_cooldown() {
        let start = Instant::now();
        let mut breaker = Breaker::default();
        for _ in 0..BREAKER_THRESHOLD - 1 {
            breaker.failed(start);
            assert!(breaker.allows(start), "a failure or two is not an outage");
        }
        breaker.failed(start);
        assert!(!breaker.allows(start), "skipped once it keeps failing");
        assert_eq!(breaker.resting_for(start), Some(BREAKER_COOLDOWN));

        // After the cooldown one probe goes through, and the rest keep skipping meanwhile.
        let later = start + BREAKER_COOLDOWN;
        assert!(breaker.allows(later));
        assert!(!breaker.allows(later), "only one probe at a time");

        // A failed probe rests twice as long; an answer closes it.
        breaker.failed(later);
        assert_eq!(breaker.resting_for(later), Some(BREAKER_COOLDOWN * 2));
        breaker.answered();
        assert!(breaker.allows(later));
        assert_eq!(breaker.resting_for(later), None);
    }

    /// A title whose next episode airs `in_secs` from when it was asked for.
    fn airing(in_secs: i64) -> Media {
        serde_json::from_value(serde_json::json!({
            "id": 154_587,
            "nextAiringEpisode": { "episode": 6, "airingAt": 1_000 + in_secs, "timeUntilAiring": in_secs },
        }))
        .unwrap()
    }

    #[test]
    fn an_answer_serves_for_its_minutes_and_not_past_a_broadcast() {
        let at = Instant::now();
        let answered = |media: Media, fresh| Answered { at, outcome: Ok(media), fresh };
        let minute = Duration::from_secs(60);

        let hour_away = answered(airing(3_600), true);
        assert!(hour_away.serves(at + minute, REUSE_FOR));
        assert!(!hour_away.serves(at + REUSE_FOR, REUSE_FOR), "only for so long");
        assert!(!hour_away.serves(at, Duration::ZERO), "and not at all with no time to serve");
        // The countdown runs on from the answer.
        let later = hour_away.as_of(at + minute).unwrap();
        assert_eq!(later.next_airing_episode.unwrap().time_until_airing, 3_540);

        // An episode that airs in the meantime ends it: the next one is another answer.
        let soon = answered(airing(90), true);
        assert!(soon.serves(at + minute, REUSE_FOR));
        assert!(!soon.serves(at + Duration::from_secs(90), REUSE_FOR));

        // The fallback's answers, and failures, are asked again.
        assert!(!answered(airing(3_600), false).serves(at, REUSE_FOR));
        let failed = Answered {
            at,
            outcome: Err(MetaError::Anilist(AniListError::NotFound)),
            fresh: true,
        };
        assert!(!failed.serves(at, REUSE_FOR));
    }

    #[test]
    fn a_breaker_rests_no_longer_than_its_ceiling() {
        let now = Instant::now();
        let mut breaker = Breaker::default();
        for _ in 0..40 {
            breaker.failed(now);
        }
        assert_eq!(breaker.cooldown(), BREAKER_MAX);
    }

    #[tokio::test]
    async fn a_resting_anilist_is_not_asked_and_says_why_on_the_providers_screen() {
        // With the network hanging every view waited out AniList's timeout before the
        // fallback was even tried. A resting source is skipped without a request: the call
        // below would panic if it were polled.
        let meta = meta();
        meta.note_anilist(Some("network: timed out".into()));
        for _ in 0..BREAKER_THRESHOLD {
            meta.trip("anilist");
        }

        let answer: std::result::Result<(), _> =
            meta.ask_anilist(async { panic!("a resting source was asked") }).await;
        let Err(AniListError::Network(message)) = answer else { panic!("not skipped") };
        assert!(message.contains("next try in"), "{message}");

        let row = &meta.health()[0];
        assert_eq!(row.state, "backing off");
        assert!(!row.healthy);
        let detail = row.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("timed out"), "the cause survives the skip: {detail}");
        // A deadline, not a countdown: one formatted here froze on an idle screen.
        let retry = row.retry_at.expect("a resting source says when it is tried again");
        let ahead = retry.duration_since(std::time::SystemTime::now()).unwrap_or_default();
        assert!(ahead > Duration::ZERO && ahead <= BREAKER_COOLDOWN, "{ahead:?}");

        // The calendar and the broadcast lines skip too, rather than each waiting it out.
        assert!(meta.last_aired(&[AnilistId::new(1)]).await.is_err());
    }

    #[tokio::test]
    async fn an_answer_closes_the_breaker_and_clears_the_weather() {
        let meta = meta();
        meta.trip("anilist");
        meta.trip("anilist");
        let found: std::result::Result<(), _> =
            meta.ask_anilist(async { Err(AniListError::NotFound) }).await;
        assert!(found.is_err());
        // Not-found is an answer, so the count starts over.
        meta.trip("anilist");
        assert!(meta.skip("anilist").is_none());

        let ok = meta.ask_anilist(async { Ok::<_, AniListError>(1) }).await;
        assert_eq!(ok.unwrap(), 1);
        assert_eq!(meta.health()[0].state, "live");
    }

    #[tokio::test]
    async fn a_resting_tenrai_is_skipped_too() {
        let meta = meta();
        for _ in 0..BREAKER_THRESHOLD {
            meta.trip("tenrai");
        }
        let answer: std::result::Result<(), _> =
            meta.ask_tenrai(async { panic!("a resting source was asked") }).await;
        assert!(matches!(answer, Err(TenraiError::Network(_))));
        assert_eq!(meta.health()[1].state, "backing off");

        // The skip, noted by a caller, does not replace the cause.
        meta.note_tenrai(Some("network: tenrai skipped".into()));
        let row = &meta.health()[1];
        assert_eq!(row.detail.as_deref(), Some("failing"));
        assert!(row.retry_at.is_some());
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
