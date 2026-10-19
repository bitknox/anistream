//! The fixture catalogue: six invented titles, and the clock their broadcasts are set by.
//!
//! | id | title (romaji / english) | shape |
//! |---|---|---|
//! | [`FINISHED`] 900001 | Hoshikuzu no Tabibito / Stardust Wayfarer | TV, finished, 12 episodes; a sequel, recommendations, per-episode streaming links |
//! | [`AIRING`] 900002 | Kumo no Ue no Toshokan / The Library Above the Clouds | TV, releasing, 5 of 24 aired; episode 6 airs in 2d 3h 15m |
//! | [`MOVIE`] 900003 | Tsukiakari no Teishajo / Moonlit Platform | movie, 108 minutes |
//! | [`SEQUEL`] 900004 | Hoshikuzu no Tabibito 2nd Season / Stardust Wayfarer Season 2 | TV, finished, 12 episodes; prequel is 900001 |
//! | [`ONA`] 900005 | Hagane no Niwashi / The Steel Gardener | ONA, finished, 10 episodes; no banner |
//! | [`AIRING_NO_MAL`] 900006 | Natsukaze Relay / Summer Wind Relay | TV, releasing, 8 of 12 aired; no MAL id, no score, no studio |
//!
//! The JSON under `fixtures/anilist/` is AniList's own wire shape, so the fake server can hand
//! it out unchanged and what a test decodes went through the same `serde` path as production.
//! It is compiled in rather than read at run time, so a crate using the kit needs no path to
//! it.
//!
//! Broadcast times are absolute and consistent with [`FIXTURE_NOW`]; the fake moves them all
//! together when told the time is something else (see [`crate::FakeAniList::set_now`]).

use std::sync::LazyLock;

use anistream_core::ids::AnilistId;
use anistream_meta::{Media, anilist::AiringEntry};
use serde_json::Value;

/// The moment the fixtures describe: Wednesday 2026-10-14, 12:00:30 UTC.
///
/// Mid-minute on purpose, so a countdown or a clock rendered from it cannot land on a boundary
/// and round differently on another machine.
pub const FIXTURE_NOW: i64 = 1_791_979_230;

/// A finished TV series with a sequel: the ordinary case.
pub const FINISHED: AnilistId = AnilistId::new(900_001);
/// A series still airing, with a next episode on the way.
pub const AIRING: AnilistId = AnilistId::new(900_002);
/// A film: one episode, a long runtime.
pub const MOVIE: AnilistId = AnilistId::new(900_003);
/// [`FINISHED`]'s second season, so a search can match more than one title.
pub const SEQUEL: AnilistId = AnilistId::new(900_004);
/// A web series, with a shorter runtime and no banner.
pub const ONA: AnilistId = AnilistId::new(900_005);
/// A second airing series that has no MAL id, no score and no studio: the sparse case.
pub const AIRING_NO_MAL: AnilistId = AnilistId::new(900_006);
/// An id the catalogue does not have. The fake answers it as AniList does an unknown title.
pub const MISSING: AnilistId = AnilistId::new(900_999);

/// Every title, in the order a browse lists them (the fixtures' notion of popularity).
pub const ALL: [AnilistId; 6] = [FINISHED, AIRING, SEQUEL, MOVIE, ONA, AIRING_NO_MAL];

/// The titles the seasonal browse answers with, whatever season is asked for.
///
/// Any season, because the app asks for the one the real clock is in, and a test should not
/// start failing when the calendar turns over.
pub const SEASONAL: [AnilistId; 2] = [AIRING, AIRING_NO_MAL];

/// The signed-in user the fake's library belongs to.
pub const VIEWER_ID: u32 = 5150;

/// Fields AniList only sends with the single-title query (`MEDIA_DETAIL_FIELDS` plus
/// relations and recommendations). A list answer leaves them out, as the real one does, so a
/// screen that wrongly relies on them in a list fails here too.
pub(crate) const DETAIL_ONLY: [&str; 5] =
    ["externalLinks", "streamingEpisodes", "trailer", "relations", "recommendations"];

static WIRE: LazyLock<Vec<Value>> = LazyLock::new(|| {
    [
        include_str!("../fixtures/anilist/media/900001.json"),
        include_str!("../fixtures/anilist/media/900002.json"),
        include_str!("../fixtures/anilist/media/900003.json"),
        include_str!("../fixtures/anilist/media/900004.json"),
        include_str!("../fixtures/anilist/media/900005.json"),
        include_str!("../fixtures/anilist/media/900006.json"),
    ]
    .iter()
    .map(|raw| serde_json::from_str(raw).expect("a media fixture is valid JSON"))
    .collect()
});

static SCHEDULE: LazyLock<Vec<Value>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../fixtures/anilist/airing.json"))
        .expect("the airing fixture is valid JSON")
});

static LIBRARY: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../fixtures/anilist/library.json"))
        .expect("the library fixture is valid JSON")
});

/// One title as AniList's single-title query answers it, or `None` for an id not in the set.
pub(crate) fn wire(id: AnilistId) -> Option<&'static Value> {
    WIRE.iter().find(|m| m["id"].as_u64() == Some(u64::from(id.get())))
}

/// One title as a list query answers it: the detail-only fields left out.
pub(crate) fn listed_wire(id: AnilistId) -> Option<Value> {
    let mut media = wire(id)?.clone();
    if let Some(fields) = media.as_object_mut() {
        for key in DETAIL_ONLY {
            fields.remove(key);
        }
    }
    Some(media)
}

/// Every broadcast row: `mediaId`, `episode`, `airingAt`, oldest first.
pub(crate) fn schedule_wire() -> &'static [Value] {
    &SCHEDULE
}

/// The viewer and their list entries: `id`, `mediaId`, `status`, `progress`, `score`,
/// `updatedAt`.
pub(crate) fn library_wire() -> &'static Value {
    &LIBRARY
}

/// One title with everything the Title screen shows. Panics for an id not in the set.
pub fn media(id: AnilistId) -> Media {
    decode(wire(id).unwrap_or_else(|| panic!("{id} is not a fixture title")).clone())
}

/// One title as a search, browse or rail carries it, without the detail-only fields.
pub fn listed(id: AnilistId) -> Media {
    decode(listed_wire(id).unwrap_or_else(|| panic!("{id} is not a fixture title")))
}

/// Every title in list shape, in [`ALL`]'s order.
pub fn catalogue() -> Vec<Media> {
    ALL.iter().map(|id| listed(*id)).collect()
}

/// The title as the UI shows it: English when there is one.
pub fn title(id: AnilistId) -> String {
    media(id).title.display().to_owned()
}

/// Every broadcast in the schedule at [`FIXTURE_NOW`], oldest first, each with its title in
/// list shape — what the Calendar is built from.
pub fn schedule() -> Vec<AiringEntry> {
    schedule_wire()
        .iter()
        .map(|row| AiringEntry {
            episode: row["episode"].as_u64().expect("a broadcast has an episode") as u32,
            airing_at: row["airingAt"].as_i64().expect("a broadcast has a time"),
            media: listed(AnilistId::new(
                row["mediaId"].as_u64().expect("a broadcast names its title") as u32,
            )),
        })
        .collect()
}

/// A title's runtime per episode, in seconds: AniList's minutes, or a TV half-hour without
/// them.
pub fn runtime_secs(id: AnilistId) -> f64 {
    wire(id).and_then(|m| m["duration"].as_f64()).unwrap_or(24.0) * 60.0
}

fn decode(value: Value) -> Media {
    serde_json::from_value(value).expect("a fixture decodes as the app's Media")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_title_decodes_in_both_shapes() {
        for id in ALL {
            let full = media(id);
            assert_eq!(full.id, id);
            let list = listed(id);
            assert!(list.relations.edges.is_empty() && list.streaming_episodes.is_empty());
            assert_eq!(list.title, full.title);
        }
        assert!(wire(MISSING).is_none());
    }

    #[test]
    fn the_countdowns_agree_with_the_fixture_clock_and_the_schedule() {
        // A countdown that disagrees with its own air time, or a next episode the calendar
        // has never heard of, would make two screens contradict each other.
        for id in [AIRING, AIRING_NO_MAL] {
            let next =
                media(id).next_airing_episode.expect("an airing title has a next episode");
            assert_eq!(next.airing_at - next.time_until_airing, FIXTURE_NOW, "{id}");
            assert!(
                schedule().iter().any(|row| row.media.id == id
                    && row.episode == next.episode
                    && row.airing_at == next.airing_at),
                "{id}'s next episode is in the schedule"
            );
        }
    }

    #[test]
    fn relations_point_at_titles_that_exist() {
        for id in ALL {
            let full = media(id);
            for related in full.watch_order() {
                assert!(ALL.contains(&related.id), "{id} relates to {}", related.id);
            }
            for recommended in full.recommended() {
                assert!(ALL.contains(&recommended.id), "{id} recommends {}", recommended.id);
            }
        }
    }
}
