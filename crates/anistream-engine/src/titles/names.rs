//! Episode titles and stills from another source, for a table whose own source has none:
//! [`Names`].
//!
//! A torrent source lists a title's episodes from the names of its releases, so it knows episode
//! 7 is out but never what it is called or what it looks like. AniList's streaming listings fill
//! that in first, but they exist mostly for shows on the official services. Another installed
//! source with a catalogue usually knows both. The video still comes from the table's own
//! source; only the titles and stills come from the other.
//!
//! - The table is said at once, as ever. What the other source knows arrives in another
//!   [`Event::Episodes`] for the title: the same rows in the same order, more of them named. It is
//!   only said when it names something the first did not.
//! - The other source is one: usable, not a torrent, not the table's own, and the first of those
//!   with a match of the title kept already, one the user chose before one the ladder made, or
//!   else the first in the order. At most [`TRIED`] are searched for the title, and only the
//!   first matched is asked for its episodes. One the ladder cannot match confidently is not
//!   asked: which title is meant is the table's question, not this one's.
//! - Its episodes are put on the table's by AniList's numbers, through [`Numbering`] on both
//!   sides. Where either cannot be lined up with AniList, nothing is put on: a wrong title is
//!   worse than none. What the table's source said, and what AniList filled in, always stands.
//! - One lookup runs at a time. A play stops it, as does another title's table or page, and a
//!   change to the title's match. It gives up after [`NAMES_WITHIN`], whatever the source does.
//! - What it found is kept for the title and the sources it asked, as long as a dub's extent is
//!   ([`dub::AIRING_FOR`], [`dub::SETTLED_FOR`], [`dub::UNSAID_FOR`]). A table loaded again
//!   within that is named at once and asks nothing; later, it is named from what was kept and
//!   asked again.
//! - While offline it asks nothing, and names from what was kept, however old.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::Duration,
};

use anistream_core::{
    ids::AnilistId,
    media::{Episode, MediaStatus, Translation},
    traits::ProviderKind,
};
use anistream_meta::anilist::Media;
use anistream_model::{episodes::EpisodeRow, event::Event};
use anistream_providers::{Answer, ProviderRegistry};
use anistream_store::{ResolutionRung, Store};

use crate::{
    clock::Clock,
    data::{self, Numbering},
    outbox::Outbox,
    titles::{dub, episodes},
};

/// How long a lookup has, from its first search to the episodes' answer: a TV's interpreted
/// plugins take seconds for each, and a lookup nobody waits on can let them.
pub const NAMES_WITHIN: Duration = Duration::from_secs(30);

/// How many sources are searched for the title before the lookup gives up on finding one.
pub const TRIED: usize = 2;

/// How many answers are kept before the stale ones are let go.
const KEPT: usize = 512;

/// What another source calls a title's episodes, and their stills, by AniList's numbers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Borrowed {
    titles: BTreeMap<u32, String>,
    stills: BTreeMap<u32, String>,
}

impl Borrowed {
    /// What `listed`, another source's episodes of `media`, says of them as of `now`, by
    /// AniList's numbers: nothing when its numbers cannot be lined up with AniList's.
    pub fn from_listing(listed: &[Episode], media: &Media, now: i64) -> Self {
        let numbering = Numbering::of(listed, media, now);
        let mut borrowed = Self::default();
        for episode in listed {
            let Some(number) = numbering.anilist(episode.number.as_str()) else { continue };
            let title =
                episode.title.as_deref().and_then(anistream_meta::anilist::model::unnumbered);
            if let Some(title) = title {
                borrowed.titles.entry(number).or_insert_with(|| title.to_owned());
            }
            // Only a web address: the apps fetch a still by it, with nothing else.
            let still = episode
                .thumbnail
                .as_deref()
                .map(str::trim)
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
            if let Some(still) = still {
                borrowed.stills.entry(number).or_insert_with(|| still.to_owned());
            }
        }
        borrowed
    }

    fn is_empty(&self) -> bool {
        self.titles.is_empty() && self.stills.is_empty()
    }
}

/// A table that left episodes without a title or a still: what naming them takes.
pub(crate) struct Unnamed {
    media: Media,
    /// The table's episodes, as its source listed them and AniList filled them in.
    episodes: Vec<Episode>,
    /// How the table's source numbers them against AniList.
    numbering: Numbering,
    /// The source that listed them.
    provider_id: String,
    /// When the table was loaded.
    now: i64,
}

impl Unnamed {
    /// What naming `episodes` takes, the table's from `provider_id` for `media` numbered as
    /// `numbering` has them: `None` when every whole episode has a title and a still, and when
    /// their numbers cannot be lined up with AniList's, so nothing could be put on them.
    pub fn of(
        media: &Media,
        episodes: &[Episode],
        numbering: Numbering,
        provider_id: &str,
        now: i64,
    ) -> Option<Self> {
        if numbering == Numbering::Unknown {
            return None;
        }
        let missing = episodes.iter().any(|episode| {
            numbering.anilist(episode.number.as_str()).is_some()
                && (episode.title.is_none() || episode.thumbnail.is_none())
        });
        missing.then(|| Self {
            media: media.clone(),
            episodes: episodes.to_vec(),
            numbering,
            provider_id: provider_id.to_owned(),
            now,
        })
    }

    /// The table's episodes, with what `borrowed` has put on those that lacked it.
    fn named(&self, borrowed: &Borrowed) -> Vec<Episode> {
        let mut episodes = self.episodes.clone();
        data::name_episodes(
            &mut episodes,
            &self.numbering.rekey(&borrowed.titles),
            &self.numbering.rekey(&borrowed.stills),
        );
        episodes
    }

    /// `episodes`' rows, as a load makes them: watched as the store has it now, and dated.
    fn rows(&self, episodes: &[Episode], store: &Store, id: AnilistId) -> Vec<EpisodeRow> {
        let rows = data::episode_rows(episodes, store, id);
        episodes::dated(rows, &self.media, self.numbering, store, id, self.now)
    }

    fn airing(&self) -> bool {
        self.media.status == Some(MediaStatus::Releasing)
            || self.media.next_airing_episode.is_some()
    }
}

/// What a lookup came to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Looked {
    /// A source listed the title, and this is what it says, which may be nothing: see
    /// [`Borrowed::from_listing`]. With whether the show is still airing, which decides how
    /// long it keeps.
    Said { borrowed: Borrowed, airing: bool },
    /// No source could say: none matched it, none answered, or there is none.
    Unsaid,
}

/// How long `looked` keeps: as long as a dub's extent would.
fn kept_for(looked: &Looked) -> i64 {
    match looked {
        Looked::Said { airing: true, .. } => dub::AIRING_FOR,
        Looked::Said { airing: false, .. } => dub::SETTLED_FOR,
        Looked::Unsaid => dub::UNSAID_FOR,
    }
}

/// The engine's lookups: what each found, kept, and the one running. Cloned into each lookup,
/// which keeps what it finds here itself.
#[derive(Clone, Default)]
pub struct Names {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    /// By title and the sources asked: see [`route`].
    kept: HashMap<(AnilistId, String), Kept>,
    running: Option<Running>,
    /// Counts lookups started, so one that ends knows whether it is still the one running.
    serial: u64,
    /// A test's time in place of [`NAMES_WITHIN`].
    #[cfg(test)]
    within: Option<Duration>,
}

struct Kept {
    borrowed: Borrowed,
    until: i64,
}

struct Running {
    id: AnilistId,
    serial: u64,
    handle: tokio::task::AbortHandle,
}

/// What a load of a title's table names it with: taken when the load is asked for.
pub(crate) struct Naming {
    names: Names,
    offline: bool,
    clock: Clock,
}

/// The sources that could name title `id`'s episodes for a table from `playing`, in the order
/// they are tried: usable, not a torrent, not `playing`; those with a match of the title the
/// user chose first, then those the ladder matched it on, then the rest, each in the order.
fn others(
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    playing: &str,
) -> Vec<String> {
    let usable = registry.usable_ids();
    let mut others: Vec<(u8, String)> = registry
        .all()
        .iter()
        .map(|source| source.manifest())
        .filter(|manifest| manifest.kind != ProviderKind::Torrent && manifest.id != playing)
        .filter(|manifest| usable.contains(&manifest.id))
        .map(|manifest| {
            let rank = match store.lookup_mapping(id, &manifest.id).ok().flatten() {
                Some(kept)
                    if matches!(
                        kept.rung,
                        ResolutionRung::Override | ResolutionRung::Disambiguated
                    ) =>
                {
                    0
                }
                Some(_) => 1,
                None => 2,
            };
            (rank, manifest.id.clone())
        })
        .collect();
    // Stable, so each rank keeps the user's order.
    others.sort_by_key(|(rank, _)| *rank);
    others.into_iter().map(|(_, id)| id).collect()
}

/// The key an answer is kept under: the table's source and the others as they are tried. A
/// source added, taken away or matched since is a key with nothing kept, so the title asks again.
fn route(playing: &str, others: &[String]) -> String {
    format!("{playing}>{}", others.join(","))
}

impl Names {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// For a load of title `id`'s table, while `offline` or not: a lookup for another title
    /// stops, since the viewer has moved on from it.
    pub(crate) fn naming(&self, id: AnilistId, offline: bool, clock: &Clock) -> Naming {
        self.leave_for(id);
        Naming { names: self.clone(), offline, clock: clock.clone() }
    }

    /// Stop the lookup running when it is for a title other than `id`, which is the one on
    /// screen now. It says nothing, and keeps nothing.
    pub fn leave_for(&self, id: AnilistId) {
        let mut inner = self.lock();
        if inner.running.as_ref().is_some_and(|running| running.id != id)
            && let Some(running) = inner.running.take()
        {
            tracing::debug!(id = %running.id, "the episode names lookup stopped for another title");
            running.handle.abort();
        }
    }

    /// Stop the lookup running, if one is: a play wants the sources' time. It says nothing, and
    /// keeps nothing, so the table's next load asks again.
    pub fn stop(&self) {
        if let Some(running) = self.lock().running.take() {
            tracing::debug!(id = %running.id, "the episode names lookup stopped for a play");
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

    /// What is kept of title `id` asked of `route`, and whether it is still fresh at `now`.
    fn kept(&self, id: AnilistId, route: &str, now: i64) -> Option<(Borrowed, bool)> {
        let inner = self.lock();
        let kept = inner.kept.get(&(id, route.to_owned()))?;
        Some((kept.borrowed.clone(), kept.until > now))
    }

    fn keep(&self, id: AnilistId, route: String, looked: Looked, now: i64) -> Borrowed {
        let until = now + kept_for(&looked);
        let borrowed = match looked {
            Looked::Said { borrowed, .. } => borrowed,
            Looked::Unsaid => Borrowed::default(),
        };
        let mut inner = self.lock();
        // A session opens a few hundred pages at most; past that, what has gone stale goes.
        if inner.kept.len() >= KEPT {
            inner.kept.retain(|_, kept| kept.until > now);
        }
        inner.kept.insert((id, route), Kept { borrowed: borrowed.clone(), until });
        borrowed
    }
}

impl Naming {
    /// Say title `id`'s table, `unnamed`, named from what was kept, and look up what it lacks
    /// when that is due: see the module's notes.
    pub(crate) fn say(
        self,
        id: AnilistId,
        unnamed: Unnamed,
        store: &Store,
        registry: &ProviderRegistry,
        tx: &Outbox,
    ) {
        let Self { names, offline, clock } = self;
        let others = others(store, registry, id, &unnamed.provider_id);
        let route = route(&unnamed.provider_id, &others);
        let kept = names.kept(id, &route, clock.now());
        let sent = match &kept {
            Some((borrowed, _)) => unnamed.named(borrowed),
            None => unnamed.episodes.clone(),
        };
        let _ = tx.send(Event::Episodes { id, rows: unnamed.rows(&sent, store, id) });
        let fresh = kept.is_some_and(|(_, fresh)| fresh);
        if offline || fresh || others.is_empty() {
            return;
        }

        let mut inner = names.lock();
        if inner.running.as_ref().is_some_and(|running| running.id == id) {
            return;
        }
        if let Some(running) = inner.running.take() {
            running.handle.abort();
        }
        inner.serial += 1;
        let serial = inner.serial;
        #[cfg(test)]
        let within = inner.within.unwrap_or(NAMES_WITHIN);
        #[cfg(not(test))]
        let within = NAMES_WITHIN;
        let (store, registry, tx, names) =
            (store.clone(), registry.clone(), tx.clone(), names.clone());
        let lookup = async move {
            let looked = tokio::time::timeout(
                within,
                look_up(&store, &registry, id, &others, &unnamed),
            )
            .await
            .unwrap_or_else(|_| {
                tracing::info!(%id, ?within, "the episode names lookup gave up waiting");
                Looked::Unsaid
            });
            let borrowed = names.keep(id, route, looked, clock.now());
            {
                let mut inner = names.lock();
                if inner.running.as_ref().is_some_and(|running| running.serial == serial) {
                    inner.running = None;
                }
            }
            let named = unnamed.named(&borrowed);
            let shown = |episodes: &[Episode]| -> Vec<(Option<String>, Option<String>)> {
                episodes.iter().map(|e| (e.title.clone(), e.thumbnail.clone())).collect()
            };
            // Said only when it names more than the table already showed.
            if shown(&named) != shown(&sent) {
                let _ = tx.send(Event::Episodes { id, rows: unnamed.rows(&named, &store, id) });
            }
        };
        let handle = tokio::spawn(lookup).abort_handle();
        inner.running = Some(Running { id, serial, handle });
    }
}

/// What the first of `others` to match title `id` lists of its episodes, for the table
/// `unnamed`: see the module's notes.
async fn look_up(
    store: &Store,
    registry: &ProviderRegistry,
    id: AnilistId,
    others: &[String],
    unnamed: &Unnamed,
) -> Looked {
    let (media, now) = (&unnamed.media, unnamed.now);
    let target = media.match_target();
    for provider_id in others.iter().take(TRIED) {
        let Some(source) = registry.pinned(provider_id) else { continue };
        let answer = anistream_providers::first_answer(
            store,
            &source,
            id,
            &target,
            Translation::Sub,
            now,
            |source, key| async move { source.episodes(&key, Translation::Sub, now).await },
        )
        .await;
        match answer {
            Answer::Answered { value, provider_id, .. } => {
                let borrowed = Borrowed::from_listing(&value, media, now);
                if borrowed.is_empty() {
                    tracing::info!(%id, provider = %provider_id, "no episode names to borrow");
                }
                return Looked::Said { borrowed, airing: unnamed.airing() };
            }
            // Matched, and could not list it: the one listing this lookup asks for was asked.
            Answer::Unanswered { matched: true, .. } => return Looked::Unsaid,
            // Not matched, or not confidently: the next may have it.
            Answer::Unanswered { matched: false, .. } | Answer::Ambiguous { .. } => {}
        }
    }
    Looked::Unsaid
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anistream_core::{
        error::ProviderError,
        ids::ProviderKey,
        media::{MediaFormat, SearchHit},
    };
    use anistream_providers::MockProvider;
    use tokio::sync::mpsc;

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_211);

    /// The show: twelve episodes, finished, or airing with eight out.
    fn media(airing: bool) -> Media {
        let mut wire = serde_json::json!({
            "id": SHOW.get(),
            "title": { "romaji": "Kazamidori no Uta", "english": "The Weathervane Song" },
            "format": "TV",
            "status": if airing { "RELEASING" } else { "FINISHED" },
            "episodes": 12,
            "seasonYear": 2026,
        });
        if airing {
            wire["nextAiringEpisode"] = serde_json::json!({ "episode": 9, "airingAt": 2_000_000, "timeUntilAiring": 1 });
        }
        serde_json::from_value(wire).unwrap()
    }

    fn hit(key: &ProviderKey) -> SearchHit {
        SearchHit {
            key: key.clone(),
            title: "Kazamidori no Uta".into(),
            synonyms: vec!["The Weathervane Song".into()],
            episode_count: Some(12),
            year: Some(2026),
            format: Some(MediaFormat::Tv),
        }
    }

    /// Episodes `numbers`, each named and with a still, from a catalogue at `host`.
    fn catalogued(numbers: impl IntoIterator<Item = u32>, host: &str) -> Vec<Episode> {
        numbers
            .into_iter()
            .map(|n| Episode {
                title: Some(format!("Episode {n} - Verse {n}")),
                thumbnail: Some(format!("https://{host}/still/{n}.jpg")),
                ..Episode::new(n)
            })
            .collect()
    }

    /// A catalogue source called `id` listing `episodes` for the show.
    fn catalogue(id: &str, episodes: Vec<Episode>) -> MockProvider {
        let key = ProviderKey::new("weathervane-song");
        MockProvider::new(id).with_hits(vec![hit(&key)]).with_episodes_for(key, episodes)
    }

    /// The torrent source, which lists the show's episodes by number alone.
    fn torrent() -> MockProvider {
        let key = ProviderKey::new("weathervane-song-releases");
        MockProvider::new("torrent")
            .kind(ProviderKind::Torrent)
            .with_hits(vec![hit(&key)])
            .with_episodes_for(key, (1..=12).map(Episode::new).collect())
    }

    fn registry(sources: Vec<MockProvider>) -> ProviderRegistry {
        ProviderRegistry::new(sources.into_iter().map(MockProvider::arc).collect())
    }

    fn calls(counter: &Arc<AtomicUsize>) -> usize {
        counter.load(Ordering::SeqCst)
    }

    struct Page {
        names: Names,
        store: Store,
        registry: ProviderRegistry,
        clock: Clock,
        media: Media,
        tx: Outbox,
        rx: mpsc::UnboundedReceiver<Event>,
    }

    impl Page {
        fn new(registry: ProviderRegistry, airing: bool) -> Self {
            let (tx, rx) = mpsc::unbounded_channel();
            Self {
                names: Names::default(),
                store: Store::open_in_memory().unwrap(),
                registry,
                clock: Clock::default(),
                media: media(airing),
                tx: Outbox::from(tx),
                rx,
            }
        }

        /// The torrent's table of `episodes`, filled in as a load fills it, said.
        fn load(&self, episodes: Vec<Episode>, offline: bool) {
            let mut episodes = episodes;
            let numbering = data::enrich_episodes(&mut episodes, &self.media, 0);
            let naming = self.names.naming(SHOW, offline, &self.clock);
            match Unnamed::of(&self.media, &episodes, numbering, "torrent", 0) {
                Some(unnamed) => {
                    naming.say(SHOW, unnamed, &self.store, &self.registry, &self.tx)
                }
                None => {
                    let rows = data::episode_rows(&episodes, &self.store, SHOW);
                    let _ = self.tx.send(Event::Episodes { id: SHOW, rows });
                }
            }
        }

        /// The next table said.
        async fn table(&mut self) -> Vec<EpisodeRow> {
            let event = tokio::time::timeout(Duration::from_secs(10), self.rx.recv())
                .await
                .expect("a table")
                .expect("the channel open");
            match event {
                Event::Episodes { id: SHOW, rows } => rows,
                other => panic!("only tables are said: {other:?}"),
            }
        }

        /// No more is said within a while.
        async fn quiet(&mut self) -> bool {
            tokio::time::timeout(Duration::from_millis(300), self.rx.recv()).await.is_err()
        }
    }

    fn titles(rows: &[EpisodeRow]) -> Vec<Option<&str>> {
        rows.iter().map(|row| row.title.as_deref()).collect()
    }

    fn stills(rows: &[EpisodeRow]) -> Vec<Option<&str>> {
        rows.iter().map(|row| row.thumbnail.as_deref()).collect()
    }

    fn bare(numbers: std::ops::RangeInclusive<u32>) -> Vec<Episode> {
        numbers.map(Episode::new).collect()
    }

    #[tokio::test]
    async fn a_torrents_bare_table_is_named_by_a_catalogue_afterwards_in_the_same_rows() {
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"));
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), false);

        let first = page.table().await;
        assert!(first.iter().all(|row| row.title.is_none() && row.thumbnail.is_none()));
        let named = page.table().await;
        let numbers =
            |rows: &[EpisodeRow]| rows.iter().map(|r| r.number.clone()).collect::<Vec<_>>();
        assert_eq!(numbers(&named), numbers(&first), "the same rows, in the same order");
        assert_eq!(named[2].title.as_deref(), Some("Verse 3"), "without the numbering");
        assert_eq!(
            named[11].thumbnail.as_deref(),
            Some("https://img.shelf.example/still/12.jpg")
        );
        assert!(page.quiet().await);
    }

    #[tokio::test]
    async fn a_catalogue_with_some_of_it_names_only_those() {
        // Titles for one to four, a still for one alone: the rest stay as they were.
        let mut listed = catalogued(1..=4, "img.shelf.example");
        for episode in &mut listed[1..] {
            episode.thumbnail = None;
        }
        let mut page = Page::new(registry(vec![torrent(), catalogue("shelf", listed)]), false);
        page.load(bare(1..=6), false);
        page.table().await;
        let named = page.table().await;
        assert_eq!(
            titles(&named),
            [Some("Verse 1"), Some("Verse 2"), Some("Verse 3"), Some("Verse 4"), None, None]
        );
        assert_eq!(
            stills(&named),
            [Some("https://img.shelf.example/still/1.jpg"), None, None, None, None, None]
        );
    }

    #[tokio::test]
    async fn a_catalogue_numbered_so_it_cannot_be_lined_up_names_nothing() {
        // Twenty past a season of twelve: neither the season's own numbers nor all of it
        // counted on from one before.
        let shelf = catalogue("shelf", catalogued(13..=32, "img.shelf.example"));
        let counter = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), false);
        let first = page.table().await;
        assert!(page.quiet().await, "nothing more is said");
        assert!(calls(&counter) > 0, "it was asked");
        assert!(titles(&first).iter().all(Option::is_none));

        // Nor is a table whose own numbers cannot be lined up asked about at all: four to
        // eleven of a season eight episodes in reads as its own numbers or as one to eight.
        let mut episodes = bare(4..=11);
        let airing = media(true);
        let numbering = data::enrich_episodes(&mut episodes, &airing, 0);
        assert_eq!(numbering, Numbering::Unknown);
        assert!(Unnamed::of(&airing, &episodes, numbering, "torrent", 0).is_none());
    }

    #[tokio::test]
    async fn a_catalogue_numbered_on_from_the_season_before_is_lined_up() {
        let shelf = catalogue("shelf", catalogued(13..=24, "img.shelf.example"));
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), false);
        page.table().await;
        let named = page.table().await;
        assert_eq!(named[0].title.as_deref(), Some("Verse 13"), "its 13 is the season's 1");
        assert_eq!(named[11].title.as_deref(), Some("Verse 24"));
    }

    #[tokio::test]
    async fn what_the_tables_own_source_said_stands() {
        let shelf = catalogue("shelf", catalogued(1..=3, "img.shelf.example"));
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        let mut own = bare(1..=3);
        own[0].title = Some("Director's Cut".into());
        own[1].thumbnail = Some("https://img.own.example/2.jpg".into());
        page.load(own, false);
        page.table().await;
        let named = page.table().await;
        assert_eq!(titles(&named), [Some("Director's Cut"), Some("Verse 2"), Some("Verse 3")]);
        assert_eq!(named[1].thumbnail.as_deref(), Some("https://img.own.example/2.jpg"));
        assert_eq!(
            named[0].thumbnail.as_deref(),
            Some("https://img.shelf.example/still/1.jpg")
        );
    }

    #[tokio::test]
    async fn a_table_with_nothing_missing_asks_nothing() {
        let shelf = catalogue("shelf", catalogued(1..=3, "img.shelf.example"));
        let counter = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(catalogued(1..=3, "img.own.example"), false);
        let rows = page.table().await;
        assert_eq!(rows[0].title.as_deref(), Some("Verse 1"));
        assert!(page.quiet().await);
        assert_eq!(calls(&counter), 0);
    }

    #[tokio::test]
    async fn offline_nothing_is_asked_and_what_was_kept_names_the_table() {
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"));
        let counter = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), true);
        assert!(titles(&page.table().await).iter().all(Option::is_none));
        assert!(page.quiet().await);
        assert_eq!(calls(&counter), 0, "nothing asked offline");

        page.load(bare(1..=12), false);
        page.table().await;
        page.table().await;
        let asked = calls(&counter);
        // Long after it went stale, offline names the table from it in the first saying.
        page.clock.advance(dub::SETTLED_FOR + 1);
        page.load(bare(1..=12), true);
        assert_eq!(page.table().await[0].title.as_deref(), Some("Verse 1"));
        assert!(page.quiet().await);
        assert_eq!(calls(&counter), asked);
    }

    #[tokio::test]
    async fn a_table_loaded_again_is_named_at_once_from_what_was_kept_until_it_goes_stale() {
        let shelf = catalogue("shelf", catalogued(1..=8, "img.shelf.example"));
        let counter = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), shelf]), true);
        page.load(bare(1..=8), false);
        page.table().await;
        page.table().await;
        let asked = calls(&counter);

        page.clock.advance(dub::AIRING_FOR - 60);
        page.load(bare(1..=8), false);
        assert_eq!(page.table().await[0].title.as_deref(), Some("Verse 1"), "named at once");
        assert!(page.quiet().await, "and said once");
        assert_eq!(calls(&counter), asked, "kept: the source is not asked");

        // An airing show's answer keeps six hours; then it is asked again, and the table is
        // named from what was kept meanwhile, with nothing more to say when that still stands.
        page.clock.advance(120);
        page.load(bare(1..=8), false);
        assert_eq!(page.table().await[0].title.as_deref(), Some("Verse 1"));
        assert!(page.quiet().await);
        assert!(calls(&counter) > asked);
    }

    #[tokio::test]
    async fn a_play_stops_the_lookup_and_nothing_is_said_or_kept() {
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"))
            .with_latency(Duration::from_secs(5));
        let counter = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), false);
        page.table().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        page.names.stop();
        assert!(page.quiet().await);
        assert_eq!(calls(&counter), 0, "stopped before the source answered");
        assert!(page.names.lock().kept.is_empty());
        assert!(page.names.lock().running.is_none(), "so the next load asks again");
    }

    #[tokio::test]
    async fn another_titles_table_stops_the_lookup() {
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"))
            .with_latency(Duration::from_secs(5));
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.load(bare(1..=12), false);
        page.table().await;
        // Its own title asking again keeps it.
        page.names.leave_for(SHOW);
        assert!(page.names.lock().running.is_some());
        let _ = page.names.naming(AnilistId::new(900_212), false, &page.clock);
        assert!(page.names.lock().running.is_none());
        assert!(page.quiet().await);
    }

    #[tokio::test]
    async fn a_source_slower_than_its_time_is_given_up_on() {
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"))
            .with_latency(Duration::from_secs(30));
        let mut page = Page::new(registry(vec![torrent(), shelf]), false);
        page.names.lock().within = Some(Duration::from_millis(200));
        page.load(bare(1..=12), false);
        page.table().await;
        assert!(page.quiet().await);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let inner = page.names.lock();
        let kept: Vec<_> = inner.kept.values().collect();
        assert_eq!(kept.len(), 1, "kept as a source that could not say");
        assert!(kept[0].borrowed.is_empty());
        let left = kept[0].until - page.clock.now();
        assert!(left <= dub::UNSAID_FOR && left > dub::UNSAID_FOR - 60, "{left}");
        assert!(inner.running.is_none());
    }

    #[tokio::test]
    async fn the_source_asked_is_one_the_title_was_matched_on_and_never_the_torrent() {
        let (first, second) = (
            catalogue("first", catalogued(1..=12, "img.first.example")),
            catalogue("second", catalogued(1..=12, "img.second.example")),
        );
        let (first_calls, second_calls) = (first.call_count(), second.call_count());
        let page = Page::new(registry(vec![torrent(), first, second]), false);
        let others = |page: &Page| others(&page.store, &page.registry, SHOW, "torrent");
        assert_eq!(others(&page), ["first", "second"], "in the order, without the torrent");

        // The second matched the title already, as a dub lookup or a play leaves it.
        let key = ProviderKey::new("weathervane-song");
        page.store
            .cache_resolution(SHOW, "second", &key, 0.9, ResolutionRung::ProviderSearch, 0)
            .unwrap();
        assert_eq!(others(&page), ["second", "first"]);
        // One the user chose comes before one the ladder made.
        page.store.set_override(SHOW, "first", &key, 0).unwrap();
        assert_eq!(others(&page), ["first", "second"]);

        let mut page = page;
        page.load(bare(1..=12), false);
        page.table().await;
        let named = page.table().await;
        assert_eq!(
            named[0].thumbnail.as_deref(),
            Some("https://img.first.example/still/1.jpg")
        );
        assert!(calls(&first_calls) > 0);
        assert_eq!(calls(&second_calls), 0, "one source is asked");
    }

    #[tokio::test]
    async fn a_source_that_cannot_match_the_title_hands_on_to_the_next() {
        let stranger = MockProvider::new("stranger").with_hits(Vec::new());
        let failing =
            catalogue("failing", Vec::new()).failing(ProviderError::Transport("reset".into()));
        let shelf = catalogue("shelf", catalogued(1..=12, "img.shelf.example"));
        let shelf_calls = shelf.call_count();
        let mut page = Page::new(registry(vec![torrent(), stranger, shelf]), false);
        page.load(bare(1..=12), false);
        page.table().await;
        assert_eq!(page.table().await[0].title.as_deref(), Some("Verse 1"));
        assert!(calls(&shelf_calls) > 0);

        // Past the number tried, it gives up rather than search every source.
        let stranger = || MockProvider::new("stranger").with_hits(Vec::new());
        let mut page = Page::new(
            registry(vec![
                torrent(),
                stranger(),
                failing,
                catalogue("shelf", catalogued(1..=12, "img.shelf.example")),
            ]),
            false,
        );
        page.load(bare(1..=12), false);
        page.table().await;
        assert!(page.quiet().await);
    }

    #[test]
    fn only_a_web_address_is_taken_for_a_still() {
        let mut listed = catalogued(1..=3, "img.shelf.example");
        listed[1].thumbnail = Some("  ".into());
        listed[2].thumbnail = Some("data:image/png;base64,AAAA".into());
        let borrowed = Borrowed::from_listing(&listed, &media(false), 0);
        assert_eq!(borrowed.stills.keys().collect::<Vec<_>>(), [&1]);
        assert_eq!(borrowed.titles.len(), 3);
    }
}
