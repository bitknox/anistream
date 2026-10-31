//! Fixing a source match: what the match picker shows. The question a source could not answer
//! for a title, what a search of the source by hand found, the pick on its way, and what the
//! title is matched to now. Kept from the engine's events, and the same rules as the Mac's and
//! the phones' match sheets.

use std::collections::{HashMap, HashSet};

use anistream_core::ids::{AnilistId, ProviderKey};
use anistream_i18n::t;
use anistream_model::{
    event::Event,
    search::MediaFormat,
    sources::{CurrentMatch, MatchCandidate, MatchQuestion},
};

/// The titles a source offered for a show when it could not choose between them.
#[derive(Debug, Clone, PartialEq)]
pub struct Asked {
    pub provider_id: String,
    /// The source's name, as a person reads it.
    pub source: String,
    pub candidates: Vec<MatchCandidate>,
}

impl Asked {
    /// The candidates the ladder did not rule out, which the page's line counts.
    pub fn possible(&self) -> usize {
        self.candidates.iter().filter(|c| c.rejected.is_none()).count()
    }
}

/// A search of a source by hand, from the sheet's field.
#[derive(Debug, Clone, PartialEq)]
pub enum Search {
    Searching {
        query: String,
    },
    /// What the source found. `source` is its name, `provider_id` the one to pin.
    Found {
        query: String,
        provider_id: String,
        source: String,
        candidates: Vec<MatchCandidate>,
    },
    /// The source answered with nothing.
    Empty {
        query: String,
        source: String,
    },
    /// The source could not be searched; `reason` is the engine's, worded.
    Failed {
        query: String,
        reason: String,
    },
}

impl Search {
    pub fn query(&self) -> &str {
        match self {
            Self::Searching { query }
            | Self::Found { query, .. }
            | Self::Empty { query, .. }
            | Self::Failed { query, .. } => query,
        }
    }
}

/// A pick that has settled, for the notice that says it and the sheet that closes on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settled {
    pub id: AnilistId,
    pub title: String,
    pub source: String,
    /// Tells two of the same picks apart.
    pub serial: u32,
}

/// The match sheet's state per title.
#[derive(Debug, Default)]
pub struct MatchBook {
    asked: HashMap<u32, Asked>,
    /// What each title is matched to now. A title in `known` but not here is matched to nothing.
    current: HashMap<u32, CurrentMatch>,
    known: HashSet<u32>,
    searches: HashMap<u32, Search>,
    picking: HashMap<u32, ProviderKey>,
    pick_failed: HashMap<u32, String>,
    picked_title: HashMap<u32, (String, String)>,
    settled: Option<Settled>,
    serial: u32,
}

impl MatchBook {
    /// Whether the page for `id` asks for its match in place of Play.
    pub fn asks(&self, id: AnilistId) -> bool {
        self.asked.contains_key(&id.get())
    }

    pub fn asked(&self, id: AnilistId) -> Option<&Asked> {
        self.asked.get(&id.get())
    }

    pub fn current(&self, id: AnilistId) -> Option<&CurrentMatch> {
        self.current.get(&id.get())
    }

    /// Whether the engine has said what `id` is matched to.
    #[cfg(any(feature = "fixtures", test))]
    pub fn is_known(&self, id: AnilistId) -> bool {
        self.known.contains(&id.get())
    }

    pub fn search(&self, id: AnilistId) -> Option<&Search> {
        self.searches.get(&id.get())
    }

    pub fn picking(&self, id: AnilistId) -> Option<&ProviderKey> {
        self.picking.get(&id.get())
    }

    pub fn pick_failed(&self, id: AnilistId) -> Option<&str> {
        self.pick_failed.get(&id.get()).map(String::as_str)
    }

    pub fn settled(&self) -> Option<&Settled> {
        self.settled.as_ref()
    }

    /// The source the sheet for `id` searches: the one that asked, else the one matched now,
    /// else the one a search found on, as `(provider id, name)`.
    pub fn source_for(&self, id: AnilistId) -> Option<(String, String)> {
        if let Some(asked) = self.asked(id) {
            return Some((asked.provider_id.clone(), asked.source.clone()));
        }
        if let Some(now) = self.current(id) {
            return Some((now.provider_id.clone(), now.source.clone()));
        }
        match self.search(id) {
            Some(Search::Found { provider_id, source, .. }) => {
                Some((provider_id.clone(), source.clone()))
            }
            _ => None,
        }
    }

    /// Fold in one of the engine's events.
    pub fn take(&mut self, event: &Event) {
        match event {
            Event::MatchChoices { id, provider_id, candidates, answers, source } => {
                match answers {
                    MatchQuestion::Ladder => {
                        self.asked.insert(
                            id.get(),
                            Asked {
                                provider_id: provider_id.clone(),
                                source: source.clone(),
                                candidates: candidates.clone(),
                            },
                        );
                    }
                    MatchQuestion::Search { query } => {
                        // An answer to a search the sheet has moved on from is not shown.
                        if self.searches.get(&id.get()).map(Search::query)
                            != Some(query.as_str())
                        {
                            return;
                        }
                        let search = if candidates.is_empty() {
                            Search::Empty { query: query.clone(), source: source.clone() }
                        } else {
                            Search::Found {
                                query: query.clone(),
                                provider_id: provider_id.clone(),
                                source: source.clone(),
                                candidates: candidates.clone(),
                            }
                        };
                        self.searches.insert(id.get(), search);
                    }
                }
            }
            Event::MatchSearchFailed { id, query, reason } => {
                if self.searches.get(&id.get()).map(Search::query) == Some(query.as_str()) {
                    self.searches.insert(
                        id.get(),
                        Search::Failed {
                            query: query.clone(),
                            reason: crate::say::msg(reason),
                        },
                    );
                }
            }
            Event::MatchFixFailed { id, reason } => {
                self.picking.remove(&id.get());
                self.picked_title.remove(&id.get());
                self.pick_failed.insert(id.get(), crate::say::msg(reason));
            }
            Event::TitleMatch { id, current } => self.matched(*id, current.clone()),
            _ => {}
        }
    }

    fn matched(&mut self, id: AnilistId, now: Option<CurrentMatch>) {
        let key = id.get();
        self.known.insert(key);
        let Some(now) = now else {
            self.current.remove(&key);
            return;
        };
        self.current.insert(key, now.clone());
        // Matched: the question is answered.
        self.asked.remove(&key);
        if self.picking.remove(&key).is_some() {
            let said = self.picked_title.remove(&key);
            self.serial += 1;
            self.settled = Some(Settled {
                id,
                title: now.title.clone().or(said.map(|s| s.0)).unwrap_or_default(),
                source: now.source,
                serial: self.serial,
            });
            self.searches.remove(&key);
        }
    }

    /// The sheet's field searched `query`.
    pub fn searching(&mut self, id: AnilistId, query: &str) {
        self.searches.insert(id.get(), Search::Searching { query: query.to_owned() });
        self.pick_failed.remove(&id.get());
    }

    /// `candidate` picked on `source`, until the engine says the title is matched.
    pub fn pick(&mut self, id: AnilistId, candidate: &MatchCandidate, source: &str) {
        self.picking.insert(id.get(), candidate.key.clone());
        self.pick_failed.remove(&id.get());
        self.picked_title.insert(id.get(), (candidate.title.clone(), source.to_owned()));
    }

    /// The person let the engine match `id` again: what they chose is forgotten at once.
    pub fn cleared(&mut self, id: AnilistId) {
        let key = id.get();
        self.current.remove(&key);
        self.searches.remove(&key);
        self.picking.remove(&key);
        self.pick_failed.remove(&key);
    }

    /// The sheet for `id` closed: a search or pick it showed is not kept for the next one.
    pub fn closed(&mut self, id: AnilistId) {
        self.searches.remove(&id.get());
        self.pick_failed.remove(&id.get());
    }
}

/// "TV · 2024 · 12 episodes", from what the source lists.
pub fn facts(candidate: &MatchCandidate) -> String {
    [
        candidate.format.map(format),
        candidate.year.map(|y| y.to_string()),
        candidate.episodes.map(|n| t::episodes_count(i64::from(n))),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

/// How close a candidate is, "71% match", or why it was ruled out.
pub fn closeness(candidate: &MatchCandidate) -> String {
    match &candidate.rejected {
        Some(reason) => crate::say::msg(reason),
        None => t::match_similarity(&crate::say::percent(candidate.similarity * 100.0)),
    }
}

/// What AniList says of the show: "On AniList: TV · 2024 · 12 episodes". Empty with no facts.
pub fn anilist(entry: &anistream_model::catalog::Entry) -> String {
    let facts = [
        entry.format.as_deref().map(crate::say::format),
        entry.year.map(|y| y.to_string()),
        entry.episodes.filter(|n| *n > 0).map(|n| t::episodes_count(i64::from(n))),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    if facts.is_empty() { String::new() } else { t::match_anilist_facts(&facts) }
}

fn format(format: MediaFormat) -> String {
    match format {
        MediaFormat::Tv => t::format_tv(),
        MediaFormat::TvShort => t::format_tv_short(),
        MediaFormat::Movie => t::format_movie(),
        MediaFormat::Special => t::format_special(),
        MediaFormat::Ova => t::format_ova(),
        MediaFormat::Ona => t::format_ona(),
        MediaFormat::Music => t::format_music(),
    }
}

/// The page's line while the match is open: how many titles could be it.
pub fn uncertain(asked: &Asked) -> String {
    match asked.possible() {
        0 => t::match_uncertain_none(&asked.source),
        n => t::match_uncertain(n as i64, &asked.source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::Msg;

    fn id() -> AnilistId {
        AnilistId::new(900_001)
    }

    fn candidate(key: &str, title: &str, rejected: bool) -> MatchCandidate {
        MatchCandidate {
            title: title.into(),
            key: ProviderKey::new(key),
            similarity: 0.71,
            rejected: rejected.then_some(Msg::MatchRuledOutYear),
            year: Some(2024),
            episodes: Some(12),
            format: Some(MediaFormat::Tv),
        }
    }

    fn ladder(candidates: Vec<MatchCandidate>) -> Event {
        Event::MatchChoices {
            id: id(),
            provider_id: "a".into(),
            candidates,
            answers: MatchQuestion::Ladder,
            source: "Source A".into(),
        }
    }

    fn found(query: &str, candidates: Vec<MatchCandidate>) -> Event {
        Event::MatchChoices {
            id: id(),
            provider_id: "a".into(),
            candidates,
            answers: MatchQuestion::Search { query: query.into() },
            source: "Source A".into(),
        }
    }

    fn matched(by_you: bool) -> Event {
        Event::TitleMatch {
            id: id(),
            current: Some(CurrentMatch {
                provider_id: "a".into(),
                source: "Source A".into(),
                title: Some("Made Up Show".into()),
                chosen_by_you: by_you,
            }),
        }
    }

    #[test]
    fn the_ladder_asks_and_counts_only_the_titles_not_ruled_out() {
        let mut book = MatchBook::default();
        book.take(&ladder(vec![
            candidate("a:1", "One", false),
            candidate("a:2", "Two", false),
            candidate("a:3", "Three", true),
        ]));
        assert!(book.asks(id()));
        assert_eq!(book.asked(id()).map(Asked::possible), Some(2));
        assert_eq!(book.source_for(id()), Some(("a".into(), "Source A".into())));
    }

    #[test]
    fn a_match_answers_the_question() {
        let mut book = MatchBook::default();
        book.take(&ladder(vec![candidate("a:1", "One", false)]));
        book.take(&matched(false));
        assert!(!book.asks(id()));
        assert!(book.is_known(id()));
        assert!(book.settled().is_none(), "no pick was made, so nothing to say");
    }

    #[test]
    fn no_match_keeps_the_question_and_is_known() {
        let mut book = MatchBook::default();
        book.take(&ladder(vec![candidate("a:1", "One", false)]));
        book.take(&Event::TitleMatch { id: id(), current: None });
        assert!(book.asks(id()));
        assert!(book.is_known(id()));
        assert!(book.current(id()).is_none());
    }

    #[test]
    fn a_search_answer_for_an_old_query_is_not_shown() {
        let mut book = MatchBook::default();
        book.searching(id(), "second");
        book.take(&found("first", vec![candidate("a:1", "One", false)]));
        assert_eq!(book.search(id()), Some(&Search::Searching { query: "second".into() }));
        book.take(&found("second", vec![candidate("a:1", "One", false)]));
        assert!(matches!(book.search(id()), Some(Search::Found { .. })));
        book.searching(id(), "third");
        book.take(&found("third", vec![]));
        assert!(matches!(book.search(id()), Some(Search::Empty { .. })));
    }

    #[test]
    fn a_pick_settles_when_the_title_is_matched() {
        let mut book = MatchBook::default();
        let pick = candidate("a:1", "One", false);
        book.searching(id(), "one");
        book.pick(id(), &pick, "Source A");
        assert_eq!(book.picking(id()), Some(&pick.key));
        book.take(&matched(true));
        assert!(book.picking(id()).is_none());
        let settled = book.settled().expect("it settled");
        assert_eq!(
            (settled.title.as_str(), settled.source.as_str()),
            ("Made Up Show", "Source A")
        );
        assert!(book.search(id()).is_none(), "the search is done with");
        let first = settled.serial;
        book.pick(id(), &pick, "Source A");
        book.take(&matched(true));
        assert_ne!(
            book.settled().map(|s| s.serial),
            Some(first),
            "the same pick twice differs"
        );
    }

    #[test]
    fn a_pick_that_failed_says_why_and_a_new_search_forgets_it() {
        let mut book = MatchBook::default();
        book.pick(id(), &candidate("a:1", "One", false), "Source A");
        book.take(&Event::MatchFixFailed { id: id(), reason: Msg::NoticeMatchDidNotStick });
        assert!(book.picking(id()).is_none());
        assert!(book.pick_failed(id()).is_some());
        book.searching(id(), "again");
        assert!(book.pick_failed(id()).is_none());
    }

    #[test]
    fn matching_automatically_forgets_the_choice_at_once() {
        let mut book = MatchBook::default();
        book.take(&matched(true));
        book.cleared(id());
        assert!(book.current(id()).is_none());
        assert!(book.is_known(id()), "what the engine said stays said");
    }

    #[test]
    fn closing_the_sheet_forgets_its_search_and_failure() {
        let mut book = MatchBook::default();
        book.searching(id(), "x");
        book.closed(id());
        assert!(book.search(id()).is_none());
    }

    #[test]
    fn a_row_says_its_facts_and_how_close_it_is_or_why_not() {
        let near = candidate("a:1", "One", false);
        assert_eq!(facts(&near), "TV · 2024 · 12 episodes");
        assert_eq!(closeness(&near), "71% match");
        let out = candidate("a:2", "Two", true);
        assert_eq!(closeness(&out), t::match_ruled_out_year());
        let bare = MatchCandidate { year: None, episodes: None, format: None, ..near };
        assert_eq!(facts(&bare), "");
    }

    #[test]
    fn the_page_line_counts_what_could_be_it() {
        let asked =
            Asked { provider_id: "a".into(), source: "Source A".into(), candidates: vec![] };
        assert_eq!(uncertain(&asked), t::match_uncertain_none("Source A"));
        let one = Asked { candidates: vec![candidate("a:1", "One", false)], ..asked };
        assert_eq!(uncertain(&one), t::match_uncertain(1, "Source A"));
    }
}
