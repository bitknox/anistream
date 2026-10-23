//! A title's franchise: the seasons along its prequel and sequel chain, the same list whichever
//! of them it is asked from.
//!
//! A title knows its own relations, and its page asks one level further ([`query::by_id`]), so
//! a page sees two steps either way along the chain. Numbering only what one page saw made the
//! third season "Season 1" on its own page and moved every label as the viewer went on to the
//! next. So the walk goes on from there, asking after every title at the edge of what is known
//! in one request ([`query::relations`]), two steps further each time, until the chain ends both
//! ways, [`MAX_HOPS`] steps out at most. What it learns is kept, so every other season's page,
//! and the same page opened again, is known without asking.
//!
//! A season is a TV, TV short or ONA title ([`is_season`]), as the picker has always had it;
//! films, OVAs and specials are among the page's related titles instead. The chain follows a
//! prequel or a sequel that is a season and goes no further through one that is not, as the
//! picker never did. A film's page has the seasons it hangs off, none of them itself. The order
//! is the chain's, prequels first, with when each premiered settling what the links leave open:
//! two sequels of one season, or a loop.
//!
//! [`query::by_id`]: crate::anilist::query::by_id
//! [`query::relations`]: crate::anilist::query::relations

use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    future::Future,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use anistream_core::{ids::AnilistId, media::MediaFormat};

use crate::anilist::{
    Media, Season,
    model::{CoverImage, RelationConnection, RelationNode},
};

/// The farthest a season can be from the title opened, in steps along the chain either way,
/// and be among its seasons.
pub const MAX_HOPS: usize = 30;

/// The most requests one walk makes. Each takes it two steps on, after the page's own two, so
/// this many reach [`MAX_HOPS`] and no more are needed.
pub const MAX_REQUESTS: usize = MAX_HOPS / 2;

/// The most titles one request asks after: AniList's page.
const PER_REQUEST: usize = 50;

/// How long what was learned of a title's relations is believed. A season announced since is a
/// new sequel on the last one, which a walk past this asks about again; the title's own page
/// learns it at once.
const KEEP_FOR: Duration = Duration::from_secs(6 * 60 * 60);

/// How many titles are kept. Past it, those learned of longest ago are let go.
const MAX_TITLES: usize = 4096;

/// How long a page waits for the walk before it is shown without its seasons, which follow:
/// see [`seasons`].
pub const HOLD: Duration = Duration::from_secs(1);

/// Whether a title of `format` is a season: TV, TV short or ONA.
pub fn is_season(format: Option<MediaFormat>) -> bool {
    matches!(format, Some(MediaFormat::Tv | MediaFormat::TvShort | MediaFormat::Ona))
}

/// One of a title's prequels or sequels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Link {
    id: AnilistId,
    /// It comes after the title: a sequel. A prequel otherwise.
    sequel: bool,
}

/// What is known of one title.
#[derive(Debug, Clone)]
struct Known {
    /// What the picker shows of it, once something that names it has come: its page, a walk's
    /// answer, or a relation on either.
    card: Option<RelationNode>,
    format: Option<MediaFormat>,
    /// Its prequels and sequels, once its relations are known, and when they were learned.
    links: Option<(Vec<Link>, Instant)>,
    /// When it was asked after and did not come: a link that leads nowhere.
    gone: Option<Instant>,
    /// When anything was last learned of it, which says who is let go first.
    touched: Instant,
}

impl Known {
    fn new(now: Instant) -> Self {
        Self { card: None, format: None, links: None, gone: None, touched: now }
    }

    /// Its prequels and sequels, while what was learned of them is still believed.
    fn links(&self, now: Instant) -> Option<&[Link]> {
        self.links
            .as_ref()
            .filter(|(_, at)| now.saturating_duration_since(*at) < KEEP_FOR)
            .map(|(links, _)| links.as_slice())
    }

    fn gone(&self, now: Instant) -> bool {
        self.gone.is_some_and(|at| now.saturating_duration_since(at) < KEEP_FOR)
    }

    /// When it premiered, as far as the card says, for the order the links leave open.
    fn premiered(&self) -> (u16, u8) {
        let card = self.card.as_ref();
        let year = card.and_then(|card| card.season_year).unwrap_or(u16::MAX);
        let season = match card.and_then(|card| card.season) {
            Some(Season::Winter) => 0,
            Some(Season::Spring) => 1,
            Some(Season::Summer) => 2,
            Some(Season::Fall) => 3,
            None => 4,
        };
        (year, season)
    }
}

/// The relations learned so far: every title met along a prequel or a sequel, with its card
/// and, once asked, its own prequels and sequels. Shared by every page through the metadata
/// source, so a franchise walked once is known from all of its seasons.
#[derive(Debug, Clone, Default)]
pub struct Relations {
    titles: HashMap<AnilistId, Known>,
}

/// Where a walk stands from one title.
#[derive(Debug, Default, PartialEq, Eq)]
struct Chain {
    /// The seasons reached, in the order reached.
    members: Vec<AnilistId>,
    /// Those reached whose own prequels and sequels are not known yet: what to ask next.
    unknown: Vec<AnilistId>,
}

impl Relations {
    fn known(&mut self, id: AnilistId, now: Instant) -> &mut Known {
        let known = self.titles.entry(id).or_insert_with(|| Known::new(now));
        known.touched = now;
        known
    }

    /// What a title's page said: the title, its prequels and sequels, and theirs.
    pub fn learn_page(&mut self, media: &Media, now: Instant) {
        let page = RelationNode {
            id: media.id,
            title: media.title.clone(),
            format: media.format,
            cover_image: CoverImage {
                extra_large: None,
                large: media
                    .cover_image
                    .large
                    .clone()
                    .or_else(|| media.cover_image.extra_large.clone()),
                color: media.cover_image.color.clone(),
            },
            season: media.season,
            season_year: media.season_year,
            relations: None,
        };
        self.learn(&page, Some(&media.relations), now);
        self.let_go();
    }

    /// What a walk's answer said of one title asked after. It was asked with its relations, so
    /// none at all means it has none.
    fn learn_answer(&mut self, node: &RelationNode, now: Instant) {
        let none = RelationConnection::default();
        self.learn(node, Some(node.relations.as_ref().unwrap_or(&none)), now);
    }

    /// Take in one title and, where `relations` says, its prequels and sequels: each of those
    /// with its card when it carries one, and its own prequels and sequels when they came too.
    fn learn(
        &mut self,
        node: &RelationNode,
        relations: Option<&RelationConnection>,
        now: Instant,
    ) {
        let known = self.known(node.id, now);
        if node.format.is_some() {
            known.format = node.format;
        }
        // A relation two steps out is named by id and format alone, and that is no card.
        if !node.title.all().is_empty() {
            known.card = Some(RelationNode { relations: None, ..node.clone() });
        }
        let Some(relations) = relations else { return };
        known.gone = None;
        let mut links = Vec::new();
        for edge in &relations.edges {
            let sequel = match edge.relation_type.as_deref() {
                Some("SEQUEL") => true,
                Some("PREQUEL") => false,
                _ => continue,
            };
            if edge.node.id == node.id {
                continue;
            }
            links.push(Link { id: edge.node.id, sequel });
            self.learn(&edge.node, edge.node.relations.as_ref(), now);
        }
        self.known(node.id, now).links = Some((links, now));
    }

    /// Asked after and not answered: AniList has no anime there, or none this build can read.
    fn learn_gone(&mut self, id: AnilistId, now: Instant) {
        let known = self.known(id, now);
        known.gone = Some(now);
        known.links = Some((Vec::new(), now));
    }

    /// Keep [`MAX_TITLES`] at most, letting go of those learned of longest ago.
    fn let_go(&mut self) {
        if self.titles.len() <= MAX_TITLES {
            return;
        }
        let mut ages: Vec<Instant> = self.titles.values().map(|known| known.touched).collect();
        ages.sort_unstable();
        let keep_from = ages[ages.len() - MAX_TITLES * 3 / 4];
        self.titles.retain(|_, known| known.touched >= keep_from);
    }

    fn is_season(&self, id: AnilistId, now: Instant) -> bool {
        self.titles.get(&id).is_some_and(|known| is_season(known.format) && !known.gone(now))
    }

    /// The seasons reached from `start` along prequels and sequels that are seasons, and which
    /// of them still have to be asked after. A link either side states counts, so a title whose
    /// own relations leave a season out is still reached from that season's.
    fn chain(&self, start: AnilistId, now: Instant) -> Chain {
        let mut next: HashMap<AnilistId, Vec<AnilistId>> = HashMap::new();
        for (id, known) in &self.titles {
            for link in known.links(now).into_iter().flatten() {
                next.entry(*id).or_default().push(link.id);
                next.entry(link.id).or_default().push(*id);
            }
        }
        let start_is_season = self.is_season(start, now);
        let mut chain = Chain::default();
        let mut seen = HashSet::from([start]);
        let mut queue = VecDeque::from([(start, 0)]);
        while let Some((id, hops)) = queue.pop_front() {
            if id != start || start_is_season {
                chain.members.push(id);
            }
            // Asked after even at the farthest step, for its card: the walk goes no further.
            if self.titles.get(&id).and_then(|known| known.links(now)).is_none() {
                chain.unknown.push(id);
            }
            if hops == MAX_HOPS {
                continue;
            }
            let mut neighbours = next.get(&id).cloned().unwrap_or_default();
            // In id order, so what is reached first, and asked first, never rests on a map's.
            neighbours.sort_unstable();
            for other in neighbours {
                if self.is_season(other, now) && seen.insert(other) {
                    queue.push_back((other, hops + 1));
                }
            }
        }
        chain
    }

    /// The seasons of `start`'s franchise as far as they are known, in watch order: the cards
    /// the picker shows, `start`'s own among them when it is a season. None when there is no
    /// other season to go to.
    pub fn seasons(&self, start: AnilistId, now: Instant) -> Vec<RelationNode> {
        let members = self.chain(start, now).members;
        let order = self.order(start, &members, now);
        let cards: Vec<RelationNode> = order
            .into_iter()
            .filter_map(|id| self.titles.get(&id).and_then(|known| known.card.clone()))
            .collect();
        if cards.iter().all(|card| card.id == start) { Vec::new() } else { cards }
    }

    /// `members` in watch order: each after its prequels and before its sequels, and where the
    /// links leave a choice, or loop, the one that premiered first. A film's page also puts the
    /// seasons before it ahead of those after it, which only it may link.
    fn order(&self, start: AnilistId, members: &[AnilistId], now: Instant) -> Vec<AnilistId> {
        let set: HashSet<AnilistId> = members.iter().copied().collect();
        let mut after: HashMap<AnilistId, BTreeSet<AnilistId>> = HashMap::new();
        let mut before: HashMap<AnilistId, usize> = HashMap::new();
        let mut link = |from: AnilistId, to: AnilistId| {
            if from != to
                && set.contains(&from)
                && set.contains(&to)
                && after.entry(from).or_default().insert(to)
            {
                *before.entry(to).or_default() += 1;
            }
        };
        let links = |id: AnilistId| {
            self.titles.get(&id).and_then(|known| known.links(now)).unwrap_or_default()
        };
        for &id in members {
            for l in links(id) {
                if l.sequel { link(id, l.id) } else { link(l.id, id) }
            }
        }
        if !set.contains(&start) {
            let around = links(start);
            for prequel in around.iter().filter(|l| !l.sequel) {
                for sequel in around.iter().filter(|l| l.sequel) {
                    link(prequel.id, sequel.id);
                }
            }
        }

        let key = |id: AnilistId| {
            let (year, season) = self.titles.get(&id).map_or((u16::MAX, 4), Known::premiered);
            (year, season, id.get())
        };
        let mut left: BTreeSet<(u16, u8, u32)> = members.iter().map(|&id| key(id)).collect();
        let mut ready: BTreeSet<(u16, u8, u32)> = members
            .iter()
            .filter(|id| before.get(id).copied().unwrap_or(0) == 0)
            .map(|&id| key(id))
            .collect();
        let mut out = Vec::with_capacity(members.len());
        // A loop leaves nothing ready while titles are left: the first of those to premiere
        // goes next, which breaks it the same way from wherever it is entered.
        while let Some(next) = ready.pop_first().or_else(|| left.first().copied()) {
            left.remove(&next);
            let id = AnilistId::new(next.2);
            out.push(id);
            for &to in after.get(&id).into_iter().flatten() {
                let count = before.entry(to).or_default();
                *count = count.saturating_sub(1);
                if *count == 0 && left.contains(&key(to)) {
                    ready.insert(key(to));
                }
            }
        }
        out
    }
}

fn lock(relations: &Mutex<Relations>) -> MutexGuard<'_, Relations> {
    relations.lock().unwrap_or_else(|e| e.into_inner())
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Walked {
    /// Every season reached is known, both ways.
    Whole,
    /// [`MAX_REQUESTS`] spent with seasons still to ask after.
    Cut,
    /// AniList did not answer.
    Failed,
}

/// Ask after every season at the edge of what is known from `start` until none is left,
/// [`MAX_REQUESTS`] times at most: those reached by a link and not yet asked after, all at once
/// (fifty to a request), each answer taking the walk two steps further.
async fn walk<F, Fut, E>(relations: &Mutex<Relations>, start: AnilistId, mut ask: F) -> Walked
where
    F: FnMut(Vec<AnilistId>) -> Fut,
    Fut: Future<Output = Result<Vec<RelationNode>, E>>,
    E: std::fmt::Display,
{
    let mut asked = 0;
    loop {
        let unknown = lock(relations).chain(start, Instant::now()).unknown;
        if unknown.is_empty() {
            return Walked::Whole;
        }
        for batch in unknown.chunks(PER_REQUEST) {
            if asked == MAX_REQUESTS {
                return Walked::Cut;
            }
            asked += 1;
            let answer = match ask(batch.to_vec()).await {
                Ok(answer) => answer,
                Err(e) => {
                    tracing::debug!(error = %e, "the season walk stopped short");
                    return Walked::Failed;
                }
            };
            let now = Instant::now();
            let mut relations = lock(relations);
            for node in &answer {
                relations.learn_answer(node, now);
            }
            // Asked for and not answered: never asked for again, so a broken link ends there.
            for id in batch {
                if !answer.iter().any(|node| node.id == *id) {
                    relations.learn_gone(*id, now);
                }
            }
            relations.let_go();
        }
    }
}

/// The seasons of `media`'s franchise, said through `say` as a title page should show them: at
/// once when they are known, as they are for every franchise walked before. Otherwise after the
/// walk, if it takes no longer than `hold`; and if it does, `None` first, for the page to show
/// without its picker until the seasons follow. Never a list that a later one renumbers.
///
/// `fresh` says whether AniList itself sent `media`. An answer from the fallback or the cache,
/// in an outage, may be a list's row, which has no relations: it is read beside what is known
/// rather than into it, and nothing is asked.
///
/// `ask` is one request of the walk ([`crate::Meta::relations`]). A walk that fails, or is cut
/// short, says what it reached.
pub(crate) async fn seasons<F, Fut, E>(
    relations: &Mutex<Relations>,
    media: &Media,
    fresh: bool,
    hold: Duration,
    ask: F,
    mut say: impl FnMut(Option<Vec<RelationNode>>),
) where
    F: FnMut(Vec<AnilistId>) -> Fut,
    Fut: Future<Output = Result<Vec<RelationNode>, E>>,
    E: std::fmt::Display,
{
    let now = Instant::now();
    if !fresh {
        let mut beside = lock(relations).clone();
        beside.learn_page(media, now);
        say(Some(beside.seasons(media.id, now)));
        return;
    }
    let whole = {
        let mut relations = lock(relations);
        relations.learn_page(media, now);
        relations.chain(media.id, now).unknown.is_empty()
    };
    if !whole {
        let walk = walk(relations, media.id, ask);
        tokio::pin!(walk);
        if tokio::time::timeout(hold, &mut walk).await.is_err() {
            say(None);
            walk.await;
        }
    }
    say(Some(lock(relations).seasons(media.id, Instant::now())));
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::BTreeMap,
        rc::Rc,
    };

    use serde_json::{Value, json};

    use super::*;

    /// One made-up title: its format, the year it premiered, and its relations by kind.
    type Made = (&'static str, u16, Vec<(&'static str, u32)>);

    /// A made-up catalogue, answered in AniList's wire shape.
    #[derive(Default, Clone)]
    struct Catalogue {
        titles: BTreeMap<u32, Made>,
    }

    impl Catalogue {
        fn with(mut self, id: u32, format: &'static str, year: u16) -> Self {
            self.titles.insert(id, (format, year, Vec::new()));
            self
        }

        /// `from` says `to` is its `relation`, and `to` says the converse, as AniList pairs them.
        fn link(mut self, from: u32, relation: &'static str, to: u32) -> Self {
            self = self.one_way(from, relation, to);
            let back = match relation {
                "SEQUEL" => "PREQUEL",
                "PREQUEL" => "SEQUEL",
                "SIDE_STORY" => "PARENT",
                "PARENT" => "SIDE_STORY",
                other => other,
            };
            self.one_way(to, back, from)
        }

        fn one_way(mut self, from: u32, relation: &'static str, to: u32) -> Self {
            self.titles
                .get_mut(&from)
                .expect("a title in the catalogue")
                .2
                .push((relation, to));
            self
        }

        fn card(&self, id: u32) -> Value {
            let (format, year, _) = &self.titles[&id];
            json!({
                "id": id,
                "title": { "romaji": format!("Title {id}"), "english": null, "native": null },
                "format": format,
                "coverImage": { "large": format!("https://img.example/{id}.jpg"), "color": null },
                "season": "SPRING",
                "seasonYear": year,
            })
        }

        /// `id`'s relations, each related title with its card and its own, by id and format.
        fn relations(&self, id: u32) -> Value {
            let edges: Vec<Value> = self.titles[&id]
                .2
                .iter()
                .map(|(relation, to)| {
                    let mut node = self.card(*to);
                    node["relations"] = self.neighbours(*to);
                    json!({ "relationType": relation, "node": node })
                })
                .collect();
            json!({ "edges": edges })
        }

        fn neighbours(&self, id: u32) -> Value {
            let edges: Vec<Value> = self.titles[&id]
                .2
                .iter()
                .map(|(relation, to)| {
                    json!({ "relationType": relation,
                            "node": { "id": to, "format": self.titles[to].0 } })
                })
                .collect();
            json!({ "edges": edges })
        }

        /// The title page's answer for `id`.
        fn page(&self, id: u32) -> Media {
            let mut media = self.card(id);
            media["relations"] = self.relations(id);
            serde_json::from_value(media).expect("a page decodes")
        }

        /// One request of the walk: the titles asked that the catalogue has.
        fn answer(&self, ids: &[AnilistId]) -> Vec<RelationNode> {
            ids.iter()
                .filter(|id| self.titles.contains_key(&id.get()))
                .map(|id| {
                    let mut node = self.card(id.get());
                    node["relations"] = self.relations(id.get());
                    serde_json::from_value(node).expect("an answer decodes")
                })
                .collect()
        }
    }

    /// Five seasons in a row, a film between the second and the third, an OVA off the first,
    /// a side story that is an ONA, and an OVA after the last: only the five are seasons.
    fn franchise() -> Catalogue {
        let mut c = Catalogue::default();
        for (n, year) in (1..=5).zip(2020..) {
            c = c.with(n, "TV", year);
        }
        c = c
            .with(11, "MOVIE", 2021)
            .with(12, "OVA", 2020)
            .with(13, "ONA", 2022)
            .with(14, "OVA", 2025);
        for n in 1..5 {
            c = c.link(n, "SEQUEL", n + 1);
        }
        c.link(2, "SEQUEL", 11)
            .link(11, "SEQUEL", 3)
            .link(1, "SIDE_STORY", 12)
            .link(3, "SIDE_STORY", 13)
            .link(5, "SEQUEL", 14)
    }

    fn ids(cards: &[RelationNode]) -> Vec<u32> {
        cards.iter().map(|card| card.id.get()).collect()
    }

    /// Open `id`'s page over `relations`, as the metadata source does: what was said, and how
    /// many requests the walk made.
    async fn open(
        catalogue: &Catalogue,
        relations: &Mutex<Relations>,
        id: u32,
    ) -> (Vec<Option<Vec<u32>>>, usize) {
        let asked = Cell::new(0);
        let said = RefCell::new(Vec::new());
        seasons(
            relations,
            &catalogue.page(id),
            true,
            Duration::from_secs(5),
            |ids: Vec<AnilistId>| {
                asked.set(asked.get() + 1);
                let answer = catalogue.answer(&ids);
                async move { Ok::<_, String>(answer) }
            },
            |seasons: Option<Vec<RelationNode>>| {
                said.borrow_mut().push(seasons.map(|s| ids(&s)))
            },
        )
        .await;
        (said.into_inner(), asked.get())
    }

    #[tokio::test]
    async fn every_season_has_the_same_seasons_in_the_same_order() {
        let catalogue = franchise();
        let five = Some(vec![1, 2, 3, 4, 5]);
        for start in [1, 3, 5] {
            // Each from nothing known, as on a first launch.
            let relations = Mutex::new(Relations::default());
            let (said, _) = open(&catalogue, &relations, start).await;
            assert_eq!(said, std::slice::from_ref(&five), "opened at season {start}");
        }
    }

    #[tokio::test]
    async fn the_walk_takes_two_steps_a_request_and_the_franchise_is_kept() {
        let catalogue = franchise();
        // The page sees two steps either way; each request two more.
        for (start, requests) in [(1, 2), (3, 1), (5, 2), (2, 1), (4, 1)] {
            let relations = Mutex::new(Relations::default());
            let (_, asked) = open(&catalogue, &relations, start).await;
            assert_eq!(asked, requests, "opened at season {start}");
        }
        // Walked once, every season's page, and the film's, is known without asking.
        let relations = Mutex::new(Relations::default());
        open(&catalogue, &relations, 1).await;
        for start in [2, 3, 4, 5, 11, 1] {
            let (said, asked) = open(&catalogue, &relations, start).await;
            assert_eq!((said, asked), (vec![Some(vec![1, 2, 3, 4, 5])], 0), "page {start}");
        }
    }

    #[tokio::test]
    async fn films_ovas_and_side_stories_are_no_seasons_and_a_films_page_has_its_franchises() {
        let catalogue = franchise();
        let relations = Mutex::new(Relations::default());
        // The film between the second and third seasons: the five, none of them itself.
        let (said, _) = open(&catalogue, &relations, 11).await;
        assert_eq!(said, [Some(vec![1, 2, 3, 4, 5])]);
        // An OVA that is a side story hangs off no prequel or sequel: no picker.
        let (said, _) = open(&catalogue, &relations, 12).await;
        assert_eq!(said, [Some(vec![])]);
        // The OVA after the last season is that season's sequel, so it has the five.
        let (said, _) = open(&catalogue, &relations, 14).await;
        assert_eq!(said, [Some(vec![1, 2, 3, 4, 5])]);
    }

    #[tokio::test]
    async fn a_title_that_is_its_only_season_has_none_to_pick() {
        let catalogue = Catalogue::default()
            .with(1, "TV", 2020)
            .with(2, "MOVIE", 2021)
            .link(1, "SEQUEL", 2);
        let relations = Mutex::new(Relations::default());
        assert_eq!(open(&catalogue, &relations, 1).await, (vec![Some(vec![])], 0));
    }

    #[tokio::test]
    async fn a_loop_is_walked_once_and_broken_where_it_began_to_air() {
        // The third season's sequel is the first: a loop AniList's data can hold.
        let catalogue = Catalogue::default()
            .with(1, "TV", 2020)
            .with(2, "TV", 2021)
            .with(3, "TV", 2022)
            .with(4, "TV", 2023)
            .link(1, "SEQUEL", 2)
            .link(2, "SEQUEL", 3)
            .link(3, "SEQUEL", 4)
            .link(4, "SEQUEL", 1);
        for start in 1..=4 {
            let relations = Mutex::new(Relations::default());
            let (said, asked) = open(&catalogue, &relations, start).await;
            assert_eq!(said, [Some(vec![1, 2, 3, 4])], "opened at {start}");
            assert!(asked <= 1, "opened at {start}: {asked} requests");
        }
    }

    #[tokio::test]
    async fn a_link_that_leads_nowhere_is_asked_once_and_left_out() {
        // The second season's sequel is a title AniList does not answer for.
        let mut catalogue = Catalogue::default()
            .with(1, "TV", 2020)
            .with(2, "TV", 2021)
            .with(3, "TV", 2022)
            .link(1, "SEQUEL", 2);
        catalogue = catalogue.one_way(2, "SEQUEL", 99).one_way(3, "PREQUEL", 99);
        catalogue.titles.insert(99, ("TV", 2022, Vec::new()));
        let mut asked_for = catalogue.clone();
        asked_for.titles.remove(&99);
        let relations = Mutex::new(Relations::default());
        let asked = Cell::new(0);
        let said = RefCell::new(Vec::new());
        seasons(
            &relations,
            &catalogue.page(1),
            true,
            Duration::from_secs(5),
            |ids: Vec<AnilistId>| {
                asked.set(asked.get() + 1);
                let answer = asked_for.answer(&ids);
                async move { Ok::<_, String>(answer) }
            },
            |seasons: Option<Vec<RelationNode>>| {
                said.borrow_mut().push(seasons.map(|s| ids(&s)))
            },
        )
        .await;
        assert_eq!(said.into_inner(), [Some(vec![1, 2])]);
        assert_eq!(asked.get(), 1, "asked once, and never again");
    }

    #[tokio::test]
    async fn a_long_franchise_is_walked_so_far_and_no_further() {
        let mut catalogue = Catalogue::default();
        for n in 1..=100 {
            catalogue = catalogue.with(n, "TV", 1990 + n as u16);
        }
        for n in 1..100 {
            catalogue = catalogue.link(n, "SEQUEL", n + 1);
        }
        let relations = Mutex::new(Relations::default());
        let (said, asked) = open(&catalogue, &relations, 1).await;
        assert!(asked <= MAX_REQUESTS, "{asked} requests");
        let reached = said.last().cloned().flatten().expect("the seasons reached");
        assert_eq!(reached, (1..=31).collect::<Vec<_>>(), "thirty steps on, and no further");
    }

    #[tokio::test]
    async fn an_answer_without_relations_ends_the_walk_there() {
        let catalogue = franchise();
        let relations = Mutex::new(Relations::default());
        let asked = Cell::new(0);
        seasons(
            &relations,
            &catalogue.page(1),
            true,
            Duration::from_secs(5),
            |ids: Vec<AnilistId>| {
                asked.set(asked.get() + 1);
                let mut answer = catalogue.answer(&ids);
                answer.iter_mut().for_each(|node| node.relations = None);
                async move { Ok::<_, String>(answer) }
            },
            |_| {},
        )
        .await;
        assert_eq!(asked.get(), 1);
    }

    #[tokio::test]
    async fn a_walk_still_under_way_says_nothing_it_would_renumber() {
        // Opened at the third of five seasons, the page alone knows the second to the fourth:
        // numbered from that, the third would be "Season 2" until the rest arrived.
        let catalogue = franchise();
        let relations = Mutex::new(Relations::default());
        let gate = Rc::new(tokio::sync::Notify::new());
        let said = RefCell::new(Vec::new());
        let page = catalogue.page(3);
        let opened = seasons(
            &relations,
            &page,
            true,
            Duration::from_millis(20),
            |ids: Vec<AnilistId>| {
                let (gate, answer) = (Rc::clone(&gate), catalogue.answer(&ids));
                async move {
                    gate.notified().await;
                    Ok::<_, String>(answer)
                }
            },
            |seasons: Option<Vec<RelationNode>>| {
                said.borrow_mut().push(seasons.map(|s| ids(&s)))
            },
        );
        let release = async {
            tokio::time::sleep(Duration::from_millis(60)).await;
            assert_eq!(*said.borrow(), [None], "held, with nothing numbered");
            gate.notify_one();
        };
        tokio::join!(opened, release);
        assert_eq!(said.into_inner(), [None, Some(vec![1, 2, 3, 4, 5])]);
    }

    #[tokio::test]
    async fn a_failed_walk_says_what_it_reached() {
        let catalogue = franchise();
        let relations = Mutex::new(Relations::default());
        let said = RefCell::new(Vec::new());
        seasons(
            &relations,
            &catalogue.page(1),
            true,
            Duration::from_secs(5),
            |_: Vec<AnilistId>| async { Err::<Vec<RelationNode>, _>("offline") },
            |seasons: Option<Vec<RelationNode>>| {
                said.borrow_mut().push(seasons.map(|s| ids(&s)))
            },
        )
        .await;
        // What the page itself knows, said once: the third season is only a link two steps out,
        // with nothing to show it by. The next opening walks again.
        assert_eq!(said.into_inner(), [Some(vec![1, 2])]);
        assert!(!lock(&relations).chain(AnilistId::new(1), Instant::now()).unknown.is_empty());
    }

    #[tokio::test]
    async fn a_page_from_the_fallback_is_read_beside_what_is_known_and_not_into_it() {
        let catalogue = franchise();
        let relations = Mutex::new(Relations::default());
        open(&catalogue, &relations, 3).await;
        // A cached list row for the first season: no relations at all.
        let mut row = catalogue.page(1);
        row.relations = RelationConnection::default();
        let said = RefCell::new(Vec::new());
        seasons(
            &relations,
            &row,
            false,
            Duration::from_secs(5),
            |_: Vec<AnilistId>| async { Err::<Vec<RelationNode>, _>("not asked") },
            |seasons: Option<Vec<RelationNode>>| {
                said.borrow_mut().push(seasons.map(|s| ids(&s)))
            },
        )
        .await;
        assert_eq!(said.into_inner(), [Some(vec![1, 2, 3, 4, 5])], "the chain still links it");
        let (said, asked) = open(&catalogue, &relations, 2).await;
        assert_eq!((said, asked), (vec![Some(vec![1, 2, 3, 4, 5])], 0), "and is as it was");
    }
}
