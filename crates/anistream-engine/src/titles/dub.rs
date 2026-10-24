//! How far a title's dub goes, for the title page's "Dub to episode 6": [`Command::LoadDub`].
//!
//! The page asks when it opens. The answer is one listing of the title in the dub, from the
//! source a dubbed play would use, so it costs that source a request, and on a TV, where plugins
//! run interpreted, seconds of its processor. Everything here keeps it out of the way:
//!
//! - It waits for the title's episode table when one is loading, so the two never ask the source
//!   at once, and the table, which the page cannot do without, comes first.
//! - One runs at a time. A page asking for another title stops the one before, and so does a
//!   play, which the source's time is for. Stopped, it says nothing, and is asked again when
//!   the page is.
//! - It gives up after [`DUB_WITHIN`], whatever the source does.
//! - What it found is kept for the title and the source it was asked of, so a page opened again
//!   asks nothing: see [`kept_for`]. A show pinned to another source since asks that one.
//! - While offline it asks nothing, and says what was kept, or nothing.
//!
//! [`Command::LoadDub`]: anistream_model::command::Command::LoadDub

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use anistream_core::{
    ids::AnilistId,
    media::{Episode, MediaStatus},
};
use anistream_meta::{Meta, anilist::Media};
use anistream_model::{episodes::DubExtent, event::Event};
use anistream_providers::{Answer, ProviderRegistry};
use anistream_store::Store;
use tokio::sync::Notify;

use crate::{clock::Clock, data, outbox::Outbox, services::Services, titles::episodes};

/// How long a lookup has, from asking the source to its answer: a TV's interpreted plugins take
/// seconds for a listing, and a lookup nobody waits on can let them.
pub const DUB_WITHIN: Duration = Duration::from_secs(30);

/// How long a lookup waits on its title's episode table before it asks all the same: a table
/// that takes longer has met trouble of its own, which waiting longer will not mend.
const TABLE_WITHIN: Duration = Duration::from_secs(60);

/// How long an answer is kept for a show still airing: its dub can move on with each week.
pub const AIRING_FOR: i64 = 6 * 3_600;
/// How long an answer is kept for any other show, whose dub rarely moves.
pub const SETTLED_FOR: i64 = 24 * 3_600;
/// How long a source that could not say is left before it is asked again.
pub const UNSAID_FOR: i64 = 15 * 60;
/// How many answers are kept before the stale ones are let go.
const KEPT: usize = 512;

/// What a lookup came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Looked {
    /// The source listed the title in the dub: how far that goes, `None` for no episode, and
    /// whether the show is still airing, which decides how long the answer keeps.
    Said { dub: Option<DubExtent>, airing: bool },
    /// No source could say: none matched it in the dub, none answered, or there is none.
    Unsaid,
}

/// How long `looked` keeps.
pub fn kept_for(looked: Looked) -> i64 {
    match looked {
        Looked::Said { airing: true, .. } => AIRING_FOR,
        Looked::Said { airing: false, .. } => SETTLED_FOR,
        Looked::Unsaid => UNSAID_FOR,
    }
}

/// The engine's lookups: what each found, kept, the one running, and the episode tables loading,
/// which a lookup waits on. Cloned into each lookup, which keeps what it finds here itself.
#[derive(Clone, Default)]
pub struct Dubs {
    inner: Arc<Mutex<Inner>>,
    /// Woken each time an episode table finishes loading.
    loaded: Arc<Notify>,
}

#[derive(Default)]
struct Inner {
    /// By title and the sources asked: see [`route`].
    kept: HashMap<(AnilistId, String), Kept>,
    running: Option<Running>,
    /// How many loads of each title's episode table are under way.
    tables: HashMap<AnilistId, usize>,
    /// Counts lookups started, so one that ends knows whether it is still the one running.
    serial: u64,
    /// A test's time in place of [`DUB_WITHIN`].
    #[cfg(test)]
    within: Option<Duration>,
}

struct Kept {
    dub: Option<DubExtent>,
    until: i64,
}

struct Running {
    id: AnilistId,
    serial: u64,
    handle: tokio::task::AbortHandle,
}

/// What a lookup asks with: the engine's services it needs, cloned.
pub(crate) struct Asking {
    pub meta: Meta,
    pub store: Store,
    pub registry: ProviderRegistry,
    pub clock: Clock,
}

/// Held while a load of title `id`'s episode table runs: see [`Dubs::table`].
pub struct TableLoading {
    dubs: Dubs,
    id: AnilistId,
}

impl Drop for TableLoading {
    fn drop(&mut self) {
        let mut inner = self.dubs.lock();
        if let Some(count) = inner.tables.get_mut(&self.id) {
            *count -= 1;
            if *count == 0 {
                inner.tables.remove(&self.id);
            }
        }
        drop(inner);
        self.dubs.loaded.notify_waiters();
    }
}

/// The sources a lookup of `id` asks, as the key its answer is kept under: the one the show is
/// pinned to, or the order of them all. A show pinned to another source since, or a source added
/// or taken away, is a key with nothing kept, so its page asks again.
pub fn route(store: &Store, registry: &ProviderRegistry, id: AnilistId) -> String {
    let pinned = store.provider_preference(id).ok().flatten();
    match pinned.filter(|provider| registry.pinned(provider).is_some()) {
        Some(provider) => format!("pinned:{provider}"),
        None => registry.ids().join(","),
    }
}

impl Dubs {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Note that a load of title `id`'s episode table has begun, until the guard is dropped: a
    /// lookup of the title's dub waits for it.
    pub fn table(&self, id: AnilistId) -> TableLoading {
        *self.lock().tables.entry(id).or_default() += 1;
        TableLoading { dubs: self.clone(), id }
    }

    fn table_loading(&self, id: AnilistId) -> bool {
        self.lock().tables.contains_key(&id)
    }

    /// Until no load of title `id`'s episode table is under way.
    async fn table_loaded(&self, id: AnilistId) {
        loop {
            let notified = self.loaded.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if !self.table_loading(id) {
                return;
            }
            notified.await;
        }
    }

    /// What is kept of title `id` asked of `route`, and whether it is still fresh at `now`.
    fn kept(&self, id: AnilistId, route: &str, now: i64) -> Option<(Option<DubExtent>, bool)> {
        let inner = self.lock();
        let kept = inner.kept.get(&(id, route.to_owned()))?;
        Some((kept.dub, kept.until > now))
    }

    fn keep(
        &self,
        id: AnilistId,
        route: String,
        looked: Looked,
        now: i64,
    ) -> Option<DubExtent> {
        let dub = match looked {
            Looked::Said { dub, .. } => dub,
            Looked::Unsaid => None,
        };
        let until = now + kept_for(looked);
        let mut inner = self.lock();
        // A session opens a few hundred pages at most; past that, what has gone stale goes.
        if inner.kept.len() >= KEPT {
            inner.kept.retain(|_, kept| kept.until > now);
        }
        inner.kept.insert((id, route), Kept { dub, until });
        dub
    }

    /// [`Command::LoadDub`] for title `id`: answered from what is kept when it is fresh, or
    /// while `offline`; otherwise looked up, unless a lookup of it is running already.
    ///
    /// [`Command::LoadDub`]: anistream_model::command::Command::LoadDub
    pub fn ask(&self, id: AnilistId, offline: bool, services: &Services, tx: &Outbox) {
        let asking = Asking {
            meta: services.meta.clone(),
            store: services.store.clone(),
            registry: services.registry.clone(),
            clock: services.clock.clone(),
        };
        self.ask_with(id, offline, asking, tx);
    }

    /// [`Self::ask`], of what `asking` holds.
    pub(crate) fn ask_with(&self, id: AnilistId, offline: bool, asking: Asking, tx: &Outbox) {
        let now = asking.clock.now();
        let route = route(&asking.store, &asking.registry, id);
        match self.kept(id, &route, now) {
            Some((dub, true)) => {
                let _ = tx.send(Event::TitleDub { id, dub });
                return;
            }
            // Offline, what was kept is better than nothing, however old.
            Some((dub, false)) if offline => {
                let _ = tx.send(Event::TitleDub { id, dub });
                return;
            }
            _ if offline => {
                let _ = tx.send(Event::TitleDub { id, dub: None });
                return;
            }
            _ => {}
        }

        let mut inner = self.lock();
        if inner.running.as_ref().is_some_and(|running| running.id == id) {
            return;
        }
        if let Some(running) = inner.running.take() {
            running.handle.abort();
        }
        inner.serial += 1;
        let serial = inner.serial;
        #[cfg(test)]
        let within = inner.within.unwrap_or(DUB_WITHIN);
        #[cfg(not(test))]
        let within = DUB_WITHIN;
        let dubs = self.clone();
        let Asking { meta, store, registry, clock } = asking;
        let tx = tx.clone();
        let lookup = async move {
            if tokio::time::timeout(TABLE_WITHIN, dubs.table_loaded(id)).await.is_err() {
                tracing::info!(%id, "the episode table is still loading; listing the dub anyway");
            }
            let looked =
                tokio::time::timeout(within, look_up(&meta, &store, &registry, id, &clock))
                    .await
                    .unwrap_or_else(|_| {
                        tracing::info!(%id, ?within, "the dub listing gave up waiting");
                        Looked::Unsaid
                    });
            // Kept under the sources as they stand now that any on their way have arrived.
            let route = self::route(&store, &registry, id);
            let dub = dubs.keep(id, route, looked, clock.now());
            {
                let mut inner = dubs.lock();
                if inner.running.as_ref().is_some_and(|running| running.serial == serial) {
                    inner.running = None;
                }
            }
            let _ = tx.send(Event::TitleDub { id, dub });
        };
        let handle = tokio::spawn(lookup).abort_handle();
        inner.running = Some(Running { id, serial, handle });
    }

    /// Stop the lookup running, if one is: a play wants the source's time. It says nothing, and
    /// keeps nothing, so the page asks again when it is next shown.
    pub fn stop(&self) {
        if let Some(running) = self.lock().running.take() {
            tracing::debug!(id = %running.id, "the dub listing stopped for a play");
            running.handle.abort();
        }
    }

    /// Forget what is kept of title `id`, and stop its lookup: its match to a source changed,
    /// so what was listed may have been another title's.
    pub fn forget(&self, id: AnilistId) {
        let mut inner = self.lock();
        inner.kept.retain(|(kept, _), _| *kept != id);
        if inner.running.as_ref().is_some_and(|running| running.id == id)
            && let Some(running) = inner.running.take()
        {
            running.handle.abort();
        }
    }
}

/// Title `id` listed in the dub from the source a dubbed play would use, and how far that goes.
pub async fn look_up(
    meta: &Meta,
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    clock: &Clock,
) -> Looked {
    if registry.is_empty() && !registry.is_arriving() {
        return Looked::Unsaid;
    }
    // The page has just loaded the title, so the cache has it, as a rule.
    let cached = store.cached_media(id).ok().flatten();
    let media =
        match cached.and_then(|(payload, _)| serde_json::from_str::<Media>(&payload).ok()) {
            Some(media) => media,
            None => match meta.media(id).await {
                Ok(media) => media,
                Err(_) => return Looked::Unsaid,
            },
        };
    let now = clock.now();
    let airing =
        media.status == Some(MediaStatus::Releasing) || media.next_airing_episode.is_some();
    let listed = episodes::dub_answer(&media, store, registry, id, now).await;
    match listed.answer {
        Answer::Answered { value, .. } => {
            Looked::Said { dub: extent(&value, &media, now), airing }
        }
        // Each source has the title and no dub of it: as sure an answer as a listing.
        Answer::Unanswered { .. } if listed.none => Looked::Said { dub: None, airing },
        Answer::Ambiguous { .. } | Answer::Unanswered { .. } => Looked::Unsaid,
    }
}

/// How far `listed`, a source's episodes of `media` in the dub, goes as of `now`: its highest
/// whole episode, and whether that is every one aired, counted in the source's own numbers.
/// `None` when it lists no whole episode.
pub fn extent(listed: &[Episode], media: &Media, now: i64) -> Option<DubExtent> {
    let through = reach(listed)?;
    let out = *data::metadata_episodes(media, now).end();
    let aired = match data::Numbering::of(listed, media, now) {
        data::Numbering::Same => Some(out),
        data::Numbering::After(offset) => Some(out + offset),
        // Numbered so that AniList's count cannot be lined up with it: how far it goes is still
        // so, but not whether that is all.
        data::Numbering::Unknown => None,
    };
    let complete = aired.is_some_and(|aired| aired > 0 && through >= aired);
    Some(DubExtent { through, complete })
}

/// The highest whole episode a listing reaches: 6 for one that ends at "6" or "6.5", `None` for
/// one with no episode numbered.
pub fn reach(episodes: &[Episode]) -> Option<u32> {
    episodes
        .iter()
        .filter_map(|episode| episode.number.as_number())
        .filter(|number| *number >= 1.0)
        .map(|number| number.floor() as u32)
        .max()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anistream_core::{
        config::NetworkConfig,
        error::ProviderError,
        ids::ProviderKey,
        media::{MediaFormat, SearchHit},
    };
    use anistream_meta::AniList;
    use anistream_providers::MockProvider;
    use tokio::sync::mpsc;

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_101);
    const DAY: i64 = 86_400;

    /// The show, as the metadata cache keeps it: twelve episodes, finished, or airing with
    /// eight out and the ninth three days off.
    fn show(store: &Store, now: i64, airing: bool) {
        let mut media = serde_json::json!({
            "id": SHOW.get(),
            "title": { "romaji": "Tsukiyo no Watashimori", "english": "The Moonlit Ferry" },
            "format": "TV",
            "status": if airing { "RELEASING" } else { "FINISHED" },
            "episodes": 12,
            "seasonYear": 2026,
        });
        if airing {
            media["nextAiringEpisode"] = serde_json::json!({ "episode": 9, "airingAt": now + 3 * DAY, "timeUntilAiring": 3 * DAY });
        }
        store.cache_media(SHOW, &media.to_string(), now).unwrap();
    }

    /// A source called `id` with the show's twelve episodes in the sub, and in the dub those up
    /// to `dubbed`, or none at all.
    fn source(id: &str, dubbed: Option<u32>) -> MockProvider {
        let key = ProviderKey::new("moonlit-ferry");
        let hit = SearchHit {
            key: key.clone(),
            title: "Tsukiyo no Watashimori".into(),
            synonyms: vec!["The Moonlit Ferry".into()],
            episode_count: Some(12),
            year: Some(2026),
            format: Some(MediaFormat::Tv),
        };
        let source = MockProvider::new(id)
            .with_hits(vec![hit])
            .with_episodes_for(key.clone(), (1..=12).map(Episode::new).collect());
        match dubbed {
            Some(dubbed) => source.with_dub_for(key, (1..=dubbed).map(Episode::new).collect()),
            None => source.without_dub(),
        }
    }

    fn registry(sources: Vec<MockProvider>) -> ProviderRegistry {
        ProviderRegistry::new(sources.into_iter().map(MockProvider::arc).collect())
    }

    fn asking(store: &Store, registry: &ProviderRegistry, clock: &Clock) -> Asking {
        let http = anistream_net::HttpClient::offline(&NetworkConfig::default()).unwrap();
        Asking {
            meta: Meta::new(AniList::new(http, 30), None, store.clone()),
            store: store.clone(),
            registry: registry.clone(),
            clock: clock.clone(),
        }
    }

    struct Page {
        dubs: Dubs,
        store: Store,
        registry: ProviderRegistry,
        clock: Clock,
        tx: Outbox,
        rx: mpsc::UnboundedReceiver<Event>,
    }

    impl Page {
        fn new(registry: ProviderRegistry, airing: bool) -> Self {
            let store = Store::open_in_memory().unwrap();
            let clock = Clock::default();
            show(&store, clock.now(), airing);
            let (tx, rx) = mpsc::unbounded_channel();
            Self { dubs: Dubs::default(), store, registry, clock, tx: Outbox::from(tx), rx }
        }

        fn ask(&self, offline: bool) {
            let asking = asking(&self.store, &self.registry, &self.clock);
            self.dubs.ask_with(SHOW, offline, asking, &self.tx);
        }

        /// What the page is told of the show's dub next.
        async fn told(&mut self) -> Option<DubExtent> {
            let event = tokio::time::timeout(Duration::from_secs(60), self.rx.recv())
                .await
                .expect("an answer")
                .expect("the channel open");
            match event {
                Event::TitleDub { id: SHOW, dub } => dub,
                other => panic!("only the dub is said: {other:?}"),
            }
        }

        /// The page opened, and what it is told.
        async fn open(&mut self) -> Option<DubExtent> {
            self.ask(false);
            self.told().await
        }

        fn quiet(&mut self) -> bool {
            self.rx.try_recv().is_err()
        }
    }

    fn calls(counter: &Arc<AtomicUsize>) -> usize {
        counter.load(Ordering::SeqCst)
    }

    #[tokio::test]
    async fn a_dub_with_every_episode_is_complete_and_reads_dubbed() {
        let mut page = Page::new(registry(vec![source("first", Some(12))]), false);
        let dub = page.open().await;
        assert_eq!(dub, Some(DubExtent { through: 12, complete: true }));
        assert_eq!(anistream_model::present::dub_line(dub.as_ref()).as_deref(), Some("Dubbed"));

        // An airing show's dub caught up with its eighth broadcast is whole too.
        let mut page = Page::new(registry(vec![source("first", Some(8))]), true);
        assert_eq!(page.open().await, Some(DubExtent { through: 8, complete: true }));
    }

    #[tokio::test]
    async fn a_dub_that_stops_short_says_how_far_it_goes() {
        let mut page = Page::new(registry(vec![source("first", Some(6))]), false);
        let dub = page.open().await;
        assert_eq!(dub, Some(DubExtent { through: 6, complete: false }));
        assert_eq!(
            anistream_model::present::dub_line(dub.as_ref()).as_deref(),
            Some("Dub to episode 6")
        );
        // Behind an airing show's broadcast, the same.
        let mut page = Page::new(registry(vec![source("first", Some(5))]), true);
        assert_eq!(page.open().await, Some(DubExtent { through: 5, complete: false }));
    }

    #[tokio::test]
    async fn no_dub_is_no_extent_and_kept_as_an_answer() {
        let none = source("first", None);
        let counter = none.call_count();
        let mut page = Page::new(registry(vec![none]), false);
        assert_eq!(page.open().await, None);

        // A source that lists the show in the dub with nothing in it, the same.
        let empty = source("first", Some(0));
        let mut listed = Page::new(registry(vec![empty]), false);
        assert_eq!(listed.open().await, None);

        // Kept as long as a dub would be, not as a source that could not say.
        let asked = calls(&counter);
        page.clock.advance(UNSAID_FOR + 1);
        assert_eq!(page.open().await, None);
        assert_eq!(calls(&counter), asked, "not asked again");
    }

    #[tokio::test]
    async fn a_source_that_cannot_say_is_no_extent_and_asked_again_sooner() {
        let failing =
            source("first", Some(6)).failing(ProviderError::Transport("reset".into()));
        let counter = failing.call_count();
        let mut page = Page::new(registry(vec![failing]), false);
        assert_eq!(page.open().await, None);
        let asked = calls(&counter);
        assert!(asked > 0);

        // Kept a while, so a page opened again does not ask at once...
        assert_eq!(page.open().await, None);
        assert_eq!(calls(&counter), asked);
        // ...but only a while.
        page.clock.advance(UNSAID_FOR + 1);
        assert_eq!(page.open().await, None);
        assert!(calls(&counter) > asked);
    }

    #[tokio::test]
    async fn offline_nothing_is_asked_and_what_was_kept_is_said() {
        let dubbed = source("first", Some(6));
        let counter = dubbed.call_count();
        let mut page = Page::new(registry(vec![dubbed]), true);
        page.ask(true);
        assert_eq!(page.told().await, None, "nothing kept yet");
        assert_eq!(calls(&counter), 0, "and nothing asked");

        let six = Some(DubExtent { through: 6, complete: false });
        assert_eq!(page.open().await, six);
        let asked = calls(&counter);
        // Long after it went stale, offline still says it rather than nothing.
        page.clock.advance(SETTLED_FOR + 1);
        page.ask(true);
        assert_eq!(page.told().await, six);
        assert_eq!(calls(&counter), asked);
    }

    #[tokio::test]
    async fn a_page_opened_again_is_answered_from_what_was_kept_until_it_goes_stale() {
        let dubbed = source("first", Some(6));
        let counter = dubbed.call_count();
        let mut page = Page::new(registry(vec![dubbed]), true);
        let six = Some(DubExtent { through: 6, complete: false });
        assert_eq!(page.open().await, six);
        let asked = calls(&counter);

        page.clock.advance(AIRING_FOR - 60);
        assert_eq!(page.open().await, six);
        assert_eq!(calls(&counter), asked, "kept: the source is not asked");

        // An airing show's answer keeps six hours; then the page asks again.
        page.clock.advance(120);
        assert_eq!(page.open().await, six);
        assert!(calls(&counter) > asked);
        assert_eq!(kept_for(Looked::Said { dub: six, airing: false }), SETTLED_FOR);
    }

    #[tokio::test]
    async fn the_extent_is_the_source_a_play_would_use_and_follows_the_shows_own_pick() {
        let (first, second) = (source("first", Some(6)), source("second", Some(10)));
        let second_calls = second.call_count();
        let mut page = Page::new(registry(vec![first, second]), false);
        // The first in the order, which a dubbed play would use; the second is not asked.
        assert_eq!(page.open().await, Some(DubExtent { through: 6, complete: false }));
        assert_eq!(calls(&second_calls), 0);

        // Pinned to the second, the show's dub is the second's, asked afresh.
        page.store.set_provider_preference(SHOW, "second", page.clock.now()).unwrap();
        assert_eq!(page.open().await, Some(DubExtent { through: 10, complete: false }));
        assert!(calls(&second_calls) > 0);
    }

    #[tokio::test]
    async fn a_lookup_waits_for_the_titles_episode_table() {
        let dubbed = source("first", Some(6));
        let counter = dubbed.call_count();
        let mut page = Page::new(registry(vec![dubbed]), false);
        let table = page.dubs.table(SHOW);
        page.ask(false);
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(page.quiet());
        assert_eq!(calls(&counter), 0, "the table has the source first");

        drop(table);
        assert_eq!(page.told().await, Some(DubExtent { through: 6, complete: false }));
    }

    #[tokio::test]
    async fn a_play_stops_the_lookup_and_nothing_is_said_or_kept() {
        let slow = source("first", Some(6)).with_latency(Duration::from_secs(5));
        let counter = slow.call_count();
        let mut page = Page::new(registry(vec![slow]), false);
        page.ask(false);
        // Asked again while it runs: still the one lookup.
        page.ask(false);
        tokio::time::sleep(Duration::from_millis(50)).await;
        page.dubs.stop();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(page.quiet());
        assert_eq!(calls(&counter), 0, "stopped before the source answered");
        assert!(page.dubs.lock().kept.is_empty());
        assert!(page.dubs.lock().running.is_none(), "so the page's next opening asks again");
    }

    #[tokio::test]
    async fn a_source_slower_than_its_time_is_given_up_on() {
        let stuck = source("first", Some(6)).with_latency(Duration::from_secs(30));
        let mut page = Page::new(registry(vec![stuck]), false);
        page.dubs.lock().within = Some(Duration::from_millis(200));
        page.ask(false);
        assert_eq!(page.told().await, None);
        let kept = page.dubs.lock().kept.values().map(|kept| kept.until).collect::<Vec<_>>();
        assert_eq!(kept.len(), 1, "kept as a source that could not say");
    }

    #[test]
    fn a_listing_reaches_its_highest_whole_episode() {
        let listed = |numbers: &[&str]| -> Vec<Episode> {
            numbers.iter().map(|n| Episode::new(*n)).collect()
        };
        assert_eq!(reach(&listed(&["1", "2", "12", "3"])), Some(12));
        assert_eq!(reach(&listed(&["5", "5.5"])), Some(5));
        assert_eq!(reach(&listed(&["OVA", "Special"])), None);
        assert_eq!(reach(&[]), None);
    }

    #[test]
    fn a_sequel_numbered_on_from_its_first_season_is_lined_up_before_it_is_called_whole() {
        let media: Media = serde_json::from_value(serde_json::json!({
            "id": SHOW.get(),
            "title": { "romaji": "Tsukiyo no Watashimori 2" },
            "format": "TV",
            "status": "FINISHED",
            "episodes": 12,
        }))
        .unwrap();
        let listed = |range: std::ops::RangeInclusive<u32>| -> Vec<Episode> {
            range.map(Episode::new).collect()
        };
        // Counted on from the first season's twelve: 13 to 24 is all of it.
        assert_eq!(
            extent(&listed(13..=24), &media, 0),
            Some(DubExtent { through: 24, complete: true })
        );
        // Partway, it says how far in the source's numbers, and cannot call it whole.
        assert_eq!(
            extent(&listed(13..=18), &media, 0),
            Some(DubExtent { through: 18, complete: false })
        );
    }
}
