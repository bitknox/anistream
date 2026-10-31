//! A title's episodes across every source that lists some of them.
//!
//! One source rarely has a whole season. A source that names torrents lists the releases it
//! finds, and a web source may lag behind the broadcast. The table a title shows is
//! the union: each episode comes from the highest-ranked source that has it, and an episode
//! AniList says is out that no source lists stays in the table, marked, so the season is whole.

use std::collections::{BTreeMap, BTreeSet};

use anistream_core::{
    error::ProviderError,
    ids::{AnilistId, ProviderKey},
    media::{Episode, MediaStatus, Translation},
};
use anistream_meta::anilist::Media;
use anistream_providers::{Attempt, Contribution, ProviderRegistry};
use anistream_store::Store;

use crate::data::{self, Numbering};

/// Whether AniList says title `media` has not started: announced, and nothing of it out by its
/// count or its schedule. There is nothing of it for a source to list, and what a search for it
/// finds is another title, most often the season before, whose name a sequel's begins with.
pub fn unstarted(media: &Media, now: i64) -> bool {
    media.status == Some(MediaStatus::NotYetReleased)
        && data::metadata_episodes(media, now).next().is_none()
}

/// How many episodes past a finished title's count a source can list and still be listing it.
/// Sites often number a bonus episode, a recap or an OVA on from the last episode, so a season
/// of 12 is listed as 13 or 14; a listing further past the count is a longer work. The same
/// slack the ladder gives a search hit's episode count before calling it another title.
const FINISHED_EXTRAS: u32 = 2;

/// How far a source can list title `media`, in AniList's numbers: a finished title's count and
/// the extras sites number on from it ([`FINISHED_EXTRAS`]), or on a show not yet finished what
/// is out and the episode airing next, which a source can carry a little before AniList's
/// schedule says it is out. `None` when AniList cannot say: a show on the air with no count or
/// schedule to go by.
fn listable(media: &Media, now: i64) -> Option<u32> {
    let out = *data::metadata_episodes(media, now).end();
    if out == 0 && !unstarted(media, now) {
        return None;
    }
    let done = matches!(media.status, Some(MediaStatus::Finished | MediaStatus::Cancelled))
        && media.next_airing_episode.is_none();
    Some(if done { out + FINISHED_EXTRAS } else { out + 1 })
}

/// Whether `listing`, a source's for `media`, runs past what has aired: it has an episode, in
/// AniList's numbers, beyond what [`listable`] allows. Such a listing is another title's that
/// the source's search took for this one, as the season before a sequel is, and none of it is
/// this title's. A listing numbered in a way that does not line up with AniList's is not judged.
pub fn runs_ahead(media: &Media, listing: &[Episode], now: i64) -> bool {
    let Some(bound) = listable(media, now) else { return false };
    let numbering = Numbering::of(listing, media, now);
    listing
        .iter()
        .filter_map(|episode| numbering.anilist(episode.number.as_str()))
        .any(|number| number > bound)
}

/// What `source` lists of title `media` (title `id`) under `key`, when it is this title's. A
/// listing that [`runs_ahead`] is the source not having the title: it does not reach a table,
/// fill another source's gaps or say how far a dub goes, and the sources below are asked
/// instead.
///
/// Unless the user chose the source's page for the title themselves: their correction stands,
/// as it does on the ladder. A season released on a site all at once while AniList's schedule
/// still goes by the weekly broadcast looks, from the listing and AniList alone, just like the
/// season before matched by its name, and a correction is how such a title is set right.
pub async fn listing(
    source: ProviderRegistry,
    key: ProviderKey,
    translation: Translation,
    (store, id): (&Store, AnilistId),
    media: &Media,
    now: i64,
) -> Attempt<Vec<Episode>> {
    let corrected = || {
        source.ids().first().is_some_and(|provider| {
            store.lookup_mapping(id, provider).ok().flatten().is_some_and(|mapping| {
                mapping.rung == anistream_store::ResolutionRung::Override
                    && mapping.provider_key == key
            })
        })
    };
    let mut attempt = source.episodes(&key, translation, now).await;
    if let Some(episodes) = &attempt.value
        && runs_ahead(media, episodes, now)
        && !corrected()
    {
        let provider = attempt.provider.take().unwrap_or_default();
        tracing::info!(
            %provider, listed = episodes.len(), "lists more than has aired; another title's"
        );
        attempt.value = None;
        attempt.failures.push((provider, ProviderError::NotFound));
    }
    attempt
}

/// The sources' listings of one title, merged: see [`merge`].
#[derive(Debug, Clone, PartialEq)]
pub struct Merged {
    /// Every episode, in order, each from the highest-ranked source that has it. Those no
    /// source lists are bare placeholders, numbered as the table is.
    pub episodes: Vec<Episode>,
    /// The source the table is from: the highest-ranked one that answered.
    pub main: String,
    /// The episodes a source other than `main` supplies, by number, with that source's id.
    pub filled: BTreeMap<String, String>,
    /// The episodes AniList says are out that no source lists.
    pub missing: BTreeSet<String>,
}

/// What marks a table's rows after they are made from its episodes: the source a gap was filled
/// from (by name) and the episodes no source lists. Carried with the table, so every path that
/// remakes its rows (the naming pass too) marks them the same.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    /// Canonical episode number to the name of the source that supplies it.
    pub filled: BTreeMap<String, String>,
    pub missing: BTreeSet<String>,
}

impl Marks {
    pub fn apply(&self, rows: &mut [anistream_model::episodes::EpisodeRow]) {
        for row in rows {
            let number = key(&row.number);
            if let Some(name) = self.filled.get(&number) {
                row.source = Some(name.clone());
            }
            row.unsourced = self.missing.contains(&number);
        }
    }
}

/// `answers`, in rank order, as one table for `media`.
///
/// Sources number their episodes their own way, and an episode taken from a source that counts
/// differently would be a different episode under the same number. So only sources that number
/// the title as the first does are merged in; the rest are left out of the table, though a play
/// still falls over to them one episode at a time.
///
/// `answers` must not be empty.
pub fn merge(media: &Media, answers: &[Contribution<Vec<Episode>>], now: i64) -> Merged {
    let first = &answers[0];
    let numbering = Numbering::of(&first.value, media, now);
    let mut episodes = first.value.clone();
    let mut have: BTreeSet<String> = episodes.iter().map(label).collect();
    let mut filled = BTreeMap::new();
    if numbering != Numbering::Unknown {
        for answer in &answers[1..] {
            if Numbering::of(&answer.value, media, now) != numbering {
                tracing::info!(provider = %answer.provider_id, "numbered differently, not merged");
                continue;
            }
            for episode in &answer.value {
                if have.insert(label(episode)) {
                    filled.insert(label(episode), answer.provider_id.clone());
                    episodes.push(episode.clone());
                }
            }
        }
    }
    // What AniList says is out, in the table's numbers.
    let mut missing = BTreeSet::new();
    let out: Vec<u32> = data::metadata_episodes(media, now).collect();
    for n in out {
        let number = match numbering {
            Numbering::Same => Some(n),
            Numbering::After(offset) => n.checked_add(offset),
            Numbering::Unknown => None,
        };
        if let Some(number) = number.map(|n| n.to_string())
            && have.insert(number.clone())
        {
            episodes.push(Episode::new(number.as_str()));
            missing.insert(number);
        }
    }
    // Numeric episodes in order, then the specials in the order they came, as a source lists.
    episodes.sort_by(|a, b| a.number.cmp(&b.number));
    Merged { episodes, main: first.provider_id.clone(), filled, missing }
}

/// Whether `answers` leave nothing AniList says is out unlisted. When there is no way to say
/// (the numbering cannot be lined up with AniList's, or AniList gives no count for a show still
/// airing), nothing is known to be whole, so the sources below are still asked.
pub fn is_whole(media: &Media, answers: &[Contribution<Vec<Episode>>], now: i64) -> bool {
    if answers.is_empty() || Numbering::of(&answers[0].value, media, now) == Numbering::Unknown
    {
        return false;
    }
    data::metadata_episodes(media, now).next().is_some()
        && merge(media, answers, now).missing.is_empty()
}

/// The canonical form of an episode number: a number when it parses ("01", "1" and "1.0" are
/// all "1"; "7.5" stays "7.5"), else the trimmed text ("OVA"). Every table here is keyed by it.
pub fn key(number: &str) -> String {
    let number = number.trim();
    match number.parse::<f64>() {
        Ok(n) if n.is_finite() => n.to_string(),
        _ => number.to_owned(),
    }
}

fn label(episode: &Episode) -> String {
    key(episode.number.as_str())
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::ProviderKey;

    use super::*;

    fn media() -> Media {
        serde_json::from_value(serde_json::json!({
            "id": 1, "status": "FINISHED", "episodes": 13, "duration": 24,
        }))
        .unwrap()
    }

    fn answer(id: &str, numbers: std::ops::RangeInclusive<u32>) -> Contribution<Vec<Episode>> {
        Contribution {
            provider_id: id.into(),
            key: ProviderKey::new(format!("{id}-key")),
            value: numbers.map(Episode::new).collect(),
        }
    }

    fn numbers(merged: &Merged) -> Vec<String> {
        merged.episodes.iter().map(label).collect()
    }

    #[test]
    fn the_highest_ranked_source_serves_each_episode_and_the_next_fills_the_gaps() {
        let merged = merge(&media(), &[answer("batch", 1..=6), answer("shelf", 1..=13)], 0);
        assert_eq!(numbers(&merged).len(), 13);
        assert_eq!(merged.main, "batch");
        let from: Vec<_> = merged.filled.keys().cloned().collect();
        assert_eq!(from, ["10", "11", "12", "13", "7", "8", "9"]);
        assert!(merged.filled.values().all(|p| p == "shelf"));
        assert!(merged.missing.is_empty());
        assert_eq!(numbers(&merged)[9], "10", "in episode order, not text order");
    }

    #[test]
    fn numbers_are_compared_by_value_not_by_spelling() {
        assert_eq!(key("01"), "1");
        assert_eq!(key(" 7.0 "), "7");
        assert_eq!(key("7.5"), "7.5");
        assert_eq!(key("OVA"), "OVA");
        let zeros = Contribution {
            provider_id: "batch".into(),
            key: ProviderKey::new("k"),
            value: ["01", "02", "03"].into_iter().map(Episode::new).collect(),
        };
        let merged = merge(&media(), &[zeros, answer("shelf", 1..=13)], 0);
        assert_eq!(merged.episodes.len(), 13, "01 and 1 are the one episode");
        assert!(!merged.filled.contains_key("2"));
        assert_eq!(merged.filled.get("4").map(String::as_str), Some("shelf"));
    }

    #[test]
    fn nothing_is_whole_when_the_season_cannot_be_counted() {
        // Airing with no count and nothing aired yet: there is no total to hold a source to.
        let unknown: Media = serde_json::from_value(serde_json::json!({
            "id": 1, "status": "RELEASING", "duration": 24,
        }))
        .unwrap();
        assert!(!is_whole(&unknown, &[answer("batch", 1..=6)], 0));
    }

    #[test]
    fn marks_survive_a_table_made_again() {
        let merged = merge(&media(), &[answer("batch", 1..=6)], 0);
        let marks = Marks { filled: BTreeMap::new(), missing: merged.missing.clone() };
        let episodes: Vec<Episode> =
            (1..=13).map(|n| Episode::new(format!("{n:02}").as_str())).collect();
        let store = anistream_store::Store::open_in_memory().unwrap();
        let mut rows =
            data::episode_rows(&episodes, &store, anistream_core::ids::AnilistId::new(1));
        marks.apply(&mut rows);
        assert!(!rows[0].unsourced);
        assert!(rows[12].unsourced, "13 is found under 13, however it is spelled");
    }

    #[test]
    fn what_no_source_lists_stays_in_the_table_as_missing() {
        let merged = merge(&media(), &[answer("batch", 1..=6), answer("shelf", 3..=9)], 0);
        assert_eq!(numbers(&merged).len(), 13);
        let missing: Vec<_> = merged.missing.iter().cloned().collect();
        assert_eq!(missing, ["10", "11", "12", "13"]);
        assert!(!is_whole(&media(), &[answer("batch", 1..=6)], 0));
        assert!(is_whole(&media(), &[answer("batch", 1..=13)], 0));
    }

    #[test]
    fn a_listing_past_what_has_aired_is_another_titles() {
        let listed = |numbers: std::ops::RangeInclusive<u32>| -> Vec<Episode> {
            numbers.map(Episode::new).collect()
        };
        let title = |fields: serde_json::Value| -> Media {
            let mut wire = serde_json::json!({ "id": 1, "duration": 24 });
            wire.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
            serde_json::from_value(wire).unwrap()
        };
        // Three of twelve out and the fourth next.
        let airing = title(serde_json::json!({
            "status": "RELEASING", "episodes": 12,
            "aired": [
                { "episode": 1, "airingAt": 100 }, { "episode": 2, "airingAt": 200 },
                { "episode": 3, "airingAt": 300 },
            ],
            "nextAiringEpisode": { "episode": 4, "airingAt": 400, "timeUntilAiring": 50 },
        }));
        assert!(!runs_ahead(&airing, &listed(1..=4), 0), "the next can be out a little early");
        assert!(runs_ahead(&airing, &listed(1..=12), 0), "the season before, by its name");
        assert!(!runs_ahead(&airing, &listed(13..=15), 0), "counted on from the season before");
        // Nothing to go by.
        let unknown = title(serde_json::json!({ "status": "RELEASING" }));
        assert!(!runs_ahead(&unknown, &listed(1..=40), 0));
        // A finished season of 12, listed with the extras sites number on from it, and a
        // listing twice as long, which is a longer work.
        let finished = title(serde_json::json!({ "status": "FINISHED", "episodes": 12 }));
        assert!(!runs_ahead(&finished, &listed(1..=12), 0));
        assert!(!runs_ahead(&finished, &listed(1..=13), 0), "a bonus numbered 13");
        assert!(!runs_ahead(&finished, &listed(1..=14), 0), "a recap and an OVA after it");
        assert!(runs_ahead(&finished, &listed(1..=24), 0));
        // Not started: nothing is out, and only the first episode is ever close to it.
        let announced = title(serde_json::json!({ "status": "NOT_YET_RELEASED" }));
        assert!(unstarted(&announced, 0));
        assert!(runs_ahead(&announced, &listed(1..=12), 0));
        assert!(!runs_ahead(&announced, &[], 0));
        assert!(!unstarted(&airing, 0));
    }

    #[test]
    fn a_source_that_numbers_the_season_differently_is_not_merged() {
        let airing: Media = serde_json::from_value(serde_json::json!({
            "id": 1, "status": "RELEASING", "episodes": 12, "duration": 24,
            "nextAiringEpisode": { "episode": 6, "airingAt": 600, "timeUntilAiring": 50 },
        }))
        .unwrap();
        // The first counts on from the season before (13 to 17); the second has 1 to 5.
        let merged = merge(&airing, &[answer("a", 13..=17), answer("b", 1..=5)], 0);
        assert!(merged.filled.is_empty());
        assert_eq!(numbers(&merged), ["13", "14", "15", "16", "17"]);
    }
}
