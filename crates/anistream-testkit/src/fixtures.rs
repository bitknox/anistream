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
//! Each title also carries what its page shows beside that: a season, dates, what it was adapted
//! from, tags, every broadcast the calendar agrees with, rankings, and a cast and crew. All of it
//! invented, and the people are named for their part ("Ren's voice", "Director of Stardust
//! Wayfarer") so none can be taken for someone real.
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

/// Where every fixture image is: covers, banners and episode stills. Nothing resolves the host, so
/// an image fails unless the fake is serving art; see [`crate::FakeAniList::serve_art`].
pub const ART_HOST: &str = "https://img.example/";

/// The signed-in user the fake's library belongs to.
pub const VIEWER_ID: u32 = 5150;

/// Fields AniList only sends with the single-title query (`MEDIA_DETAIL_FIELDS` plus
/// relations and recommendations). A list answer leaves them out, as the real one does, so a
/// screen that wrongly relies on them in a list fails here too.
///
/// `aired` is not a field of AniList's `Media`: the single-title query asks for it beside the
/// title, and the fake moves it there. The fixtures keep it on the title because that is where
/// the app's `Media` holds it once read.
pub(crate) const DETAIL_ONLY: [&str; 10] = [
    "externalLinks",
    "streamingEpisodes",
    "relations",
    "recommendations",
    "tags",
    "upcoming",
    "aired",
    "rankings",
    "characters",
    "staff",
];

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
    Some(listed_form(wire(id)?))
}

/// `media` as a list query answers it: the detail-only fields left out.
pub(crate) fn listed_form(media: &Value) -> Value {
    let mut media = media.clone();
    // The relations by kind and id alone, as the list fields ask for them.
    let prequels: Vec<Value> = media["relations"]["edges"]
        .as_array()
        .map(|edges| {
            edges
                .iter()
                .map(|e| serde_json::json!({ "relationType": e["relationType"], "node": { "id": e["node"]["id"] } }))
                .collect()
        })
        .unwrap_or_default();
    if let Some(fields) = media.as_object_mut() {
        fields.insert("prequels".into(), serde_json::json!({ "edges": prequels }));
        for key in DETAIL_ONLY {
            fields.remove(key);
        }
    }
    media
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

/// The opt-in catalogue's ids, in the order [`catalogue_titles`] lists them: see there.
pub const CATALOGUE: [AnilistId; 8] = [
    AnilistId::new(900_101),
    AnilistId::new(900_102),
    AnilistId::new(900_103),
    AnilistId::new(900_104),
    AnilistId::new(900_105),
    AnilistId::new(900_106),
    AnilistId::new(900_107),
    AnilistId::new(900_108),
];

/// Eight more invented titles for a full browse grid, in no list or search until
/// [`crate::FakeAniList::add_catalogue`] is told to add them: the six alone make one row on a
/// wide window. Each has a poster and a wide drawing (see [`crate::art`]) and no episodes,
/// broadcasts or library entries. Four are Fantasy, so with the three Fantasy fixtures the filter
/// finds seven of fourteen.
///
/// | id | title (romaji / english) | shape |
/// |---|---|---|
/// | 900101 | Shiranami no Tomoshibi / The Lighthouse Keeper's Lamp | TV 2024, finished; Fantasy, Drama |
/// | 900102 | Koi no Ike no Yoru / Night at the Koi Pond | ONA 2022, finished; Fantasy, Romance |
/// | 900103 | Chochin Matsuri / The Paper Lantern Festival | OVA 2021, finished; Comedy, Romance |
/// | 900104 | Yamakage Kisha / The Mountain Shadow Express | TV 2026, releasing; Adventure, Mystery |
/// | 900105 | Kitsune no Omote / The Fox Mask | movie 2024; Fantasy, Action |
/// | 900106 | Denpatou no Yoru / Radio Tower Nights | ONA 2025, releasing; Sci-Fi, Mystery |
/// | 900107 | Onshitsu no Yonin / Four Seasons in the Greenhouse | TV 2020, finished; Fantasy, Slice of Life |
/// | 900108 | Asagiri no Bouto / Morning Mist Rowing Club | TV 2023, finished; Sports, Drama |
pub fn catalogue_titles() -> Vec<Value> {
    [
        include_str!("../fixtures/anilist/media/900101.json"),
        include_str!("../fixtures/anilist/media/900102.json"),
        include_str!("../fixtures/anilist/media/900103.json"),
        include_str!("../fixtures/anilist/media/900104.json"),
        include_str!("../fixtures/anilist/media/900105.json"),
        include_str!("../fixtures/anilist/media/900106.json"),
        include_str!("../fixtures/anilist/media/900107.json"),
        include_str!("../fixtures/anilist/media/900108.json"),
    ]
    .iter()
    .map(|raw| serde_json::from_str(raw).expect("a catalogue fixture is valid JSON"))
    .collect()
}

/// A made-up franchise beside the six, for the season picker, in no list, search or schedule:
/// [`crate::FakeAniList::add_titles`] answers its pages and the season walk.
///
/// | id | title (romaji / english) | shape |
/// |---|---|---|
/// | [`FRANCHISE_SEASONS`] 910001–910005 | Tomoshibi no Mori / Lantern Wood, then Season 2 to 5 | TV, one a year from 2019, each the sequel of the one before |
/// | [`FRANCHISE_FILM`] 910006 | Lantern Wood: The Night Market | film, the second season's sequel and the third's prequel |
/// | [`FRANCHISE_OVA`] 910007 | Lantern Wood: Before the First Light | OVA, a side story of the first season |
/// | [`FRANCHISE_SIDE_STORY`] 910008 | Lantern Wood: Moth Letters | ONA, a side story of the third season |
pub const FRANCHISE_SEASONS: [AnilistId; 5] = [
    AnilistId::new(910_001),
    AnilistId::new(910_002),
    AnilistId::new(910_003),
    AnilistId::new(910_004),
    AnilistId::new(910_005),
];
/// [`FRANCHISE_SEASONS`]' film, between the second and the third.
pub const FRANCHISE_FILM: AnilistId = AnilistId::new(910_006);
/// [`FRANCHISE_SEASONS`]' OVA, a side story of the first.
pub const FRANCHISE_OVA: AnilistId = AnilistId::new(910_007);
/// [`FRANCHISE_SEASONS`]' web series, a side story of the third: an ONA that is no season.
pub const FRANCHISE_SIDE_STORY: AnilistId = AnilistId::new(910_008);

/// The franchise's titles, as the single-title query answers each: see [`FRANCHISE_SEASONS`].
pub fn franchise() -> Vec<Value> {
    use serde_json::json;
    let [one, two, three, four, five] = FRANCHISE_SEASONS.map(|id| id.get());
    let (film, ova, side) =
        (FRANCHISE_FILM.get(), FRANCHISE_OVA.get(), FRANCHISE_SIDE_STORY.get());
    let named = |id: u32| -> (String, String, &'static str, u16) {
        let n = id - 910_000;
        match id {
            _ if id == film => (
                "Tomoshibi no Mori: Yoichi".into(),
                "Lantern Wood: The Night Market".into(),
                "MOVIE",
                2021,
            ),
            _ if id == ova => (
                "Tomoshibi no Mori: Hajimari".into(),
                "Lantern Wood: Before the First Light".into(),
                "OVA",
                2019,
            ),
            _ if id == side => (
                "Tomoshibi no Mori: Ga no Tegami".into(),
                "Lantern Wood: Moth Letters".into(),
                "ONA",
                2021,
            ),
            _ if n == 1 => ("Tomoshibi no Mori".into(), "Lantern Wood".into(), "TV", 2019),
            _ => (
                format!("Tomoshibi no Mori {n}"),
                format!("Lantern Wood Season {n}"),
                "TV",
                2018 + n as u16,
            ),
        }
    };
    let card = |id: u32| {
        let (romaji, english, format, year) = named(id);
        json!({
            "id": id,
            "title": { "romaji": romaji, "english": english, "native": null },
            "format": format,
            "coverImage": { "large": format!("{ART_HOST}cover/{id}.jpg"), "color": "#6a8f5c" },
            "season": "SPRING",
            "seasonYear": year,
        })
    };
    let relations: [(u32, Vec<(&str, u32)>); 8] = [
        (one, vec![("SEQUEL", two), ("SIDE_STORY", ova)]),
        (two, vec![("PREQUEL", one), ("SEQUEL", three), ("SEQUEL", film)]),
        (
            three,
            vec![("PREQUEL", two), ("PREQUEL", film), ("SEQUEL", four), ("SIDE_STORY", side)],
        ),
        (four, vec![("PREQUEL", three), ("SEQUEL", five)]),
        (five, vec![("PREQUEL", four)]),
        (film, vec![("PREQUEL", two), ("SEQUEL", three)]),
        (ova, vec![("PARENT", one)]),
        (side, vec![("PARENT", three)]),
    ];
    relations
        .into_iter()
        .map(|(id, edges)| {
            let (_, _, format, year) = named(id);
            let mut title = card(id);
            title["idMal"] = Value::Null;
            title["status"] = json!("FINISHED");
            title["episodes"] = json!(if format == "TV" { 12 } else { 1 });
            title["duration"] = json!(if format == "MOVIE" { 95 } else { 24 });
            title["startDate"] = json!({ "year": year, "month": 4, "day": 6 });
            title["coverImage"]["extraLarge"] = title["coverImage"]["large"].clone();
            let edges: Vec<Value> = edges
                .into_iter()
                .map(|(relation, to)| json!({ "relationType": relation, "node": card(to) }))
                .collect();
            title["relations"] = json!({ "edges": edges });
            title
        })
        .collect()
}

/// A long made-up franchise, for a season picker with more seasons than fit across a screen. The
/// scenes have it beside the six, in no list, search or schedule, as [`franchise`] is: a page
/// opens on it by id.
///
/// | id | title (romaji / english) | shape |
/// |---|---|---|
/// | [`LONG_FRANCHISE_SEASONS`] 920001–920012 | Kumokiri Hikousen / Cloudcutter Airship, then Season 2 to 12 | TV, one a year from 2014, each the sequel of the one before |
/// | [`LONG_FRANCHISE_FILM`] 920013 | Cloudcutter Airship: Harbour of Stars | film, a side story of the sixth season |
/// | [`LONG_FRANCHISE_OVA`] 920014 | Cloudcutter Airship: First Flight | OVA, a side story of the first season |
pub const LONG_FRANCHISE_SEASONS: [AnilistId; 12] = [
    AnilistId::new(920_001),
    AnilistId::new(920_002),
    AnilistId::new(920_003),
    AnilistId::new(920_004),
    AnilistId::new(920_005),
    AnilistId::new(920_006),
    AnilistId::new(920_007),
    AnilistId::new(920_008),
    AnilistId::new(920_009),
    AnilistId::new(920_010),
    AnilistId::new(920_011),
    AnilistId::new(920_012),
];
/// [`LONG_FRANCHISE_SEASONS`]' film, a side story of the sixth.
pub const LONG_FRANCHISE_FILM: AnilistId = AnilistId::new(920_013);
/// [`LONG_FRANCHISE_SEASONS`]' OVA, a side story of the first.
pub const LONG_FRANCHISE_OVA: AnilistId = AnilistId::new(920_014);

/// The long franchise's titles, as the single-title query answers each: see
/// [`LONG_FRANCHISE_SEASONS`].
pub fn long_franchise() -> Vec<Value> {
    use serde_json::json;
    let seasons = LONG_FRANCHISE_SEASONS.map(|id| id.get());
    let (film, ova) = (LONG_FRANCHISE_FILM.get(), LONG_FRANCHISE_OVA.get());
    // Each season's own muted cover colour, so the plates tell them apart.
    const COLOURS: [&str; 12] = [
        "#5f8fb0", "#6f8f6a", "#b08a5f", "#8a6fa8", "#5f9e96", "#a86f6f", "#7a86b8", "#9a9a5c",
        "#b07a96", "#5c8a7a", "#a8875c", "#6a7f9e",
    ];
    struct Named {
        romaji: String,
        english: String,
        format: &'static str,
        year: u16,
        season: &'static str,
        episodes: u32,
        colour: &'static str,
    }
    let named = |id: u32| -> Named {
        match id {
            _ if id == film => Named {
                romaji: "Kumokiri Hikousen: Hoshi no Minato".into(),
                english: "Cloudcutter Airship: Harbour of Stars".into(),
                format: "MOVIE",
                year: 2020,
                season: "SUMMER",
                episodes: 1,
                colour: "#7f7fa8",
            },
            _ if id == ova => Named {
                romaji: "Kumokiri Hikousen: Hatsuhikou".into(),
                english: "Cloudcutter Airship: First Flight".into(),
                format: "OVA",
                year: 2015,
                season: "WINTER",
                episodes: 2,
                colour: "#8fa0b0",
            },
            _ => {
                let n = id - 920_000;
                let (romaji, english) = if n == 1 {
                    ("Kumokiri Hikousen".to_owned(), "Cloudcutter Airship".to_owned())
                } else {
                    (
                        format!("Kumokiri Hikousen {n}"),
                        format!("Cloudcutter Airship Season {n}"),
                    )
                };
                Named {
                    romaji,
                    english,
                    format: "TV",
                    year: 2013 + n as u16,
                    season: if n % 2 == 1 { "SPRING" } else { "FALL" },
                    episodes: if n.is_multiple_of(3) { 13 } else { 12 },
                    colour: COLOURS[n as usize - 1],
                }
            }
        }
    };
    let card = |id: u32| {
        let title = named(id);
        json!({
            "id": id,
            "title": { "romaji": title.romaji, "english": title.english, "native": null },
            "format": title.format,
            "coverImage": { "large": format!("{ART_HOST}cover/{id}.jpg"), "color": title.colour },
            "season": title.season,
            "seasonYear": title.year,
        })
    };
    let mut relations: Vec<(u32, Vec<(&str, u32)>)> = seasons
        .iter()
        .enumerate()
        .map(|(at, &id)| {
            let mut edges = Vec::new();
            if at > 0 {
                edges.push(("PREQUEL", seasons[at - 1]));
            }
            if let Some(&next) = seasons.get(at + 1) {
                edges.push(("SEQUEL", next));
            }
            (id, edges)
        })
        .collect();
    relations[0].1.push(("SIDE_STORY", ova));
    relations[5].1.push(("SIDE_STORY", film));
    relations.push((film, vec![("PARENT", seasons[5])]));
    relations.push((ova, vec![("PARENT", seasons[0])]));
    relations
        .into_iter()
        .map(|(id, edges)| {
            let Named { english, format, year, episodes, .. } = named(id);
            let mut title = card(id);
            title["idMal"] = Value::Null;
            title["status"] = json!("FINISHED");
            title["description"] = json!(format!(
                "The crew of a patched-up airship carry letters and strangers between islands \
                 that float above an endless sea of cloud. {english}."
            ));
            title["genres"] = json!(["Adventure", "Slice of Life"]);
            title["episodes"] = json!(episodes);
            title["duration"] = json!(if format == "MOVIE" { 104 } else { 24 });
            title["startDate"] = json!({ "year": year, "month": 4, "day": 9 });
            // Weekly from 9 April: a film or an OVA ends the day it starts.
            let (month, day) = match episodes {
                1 | 2 => (4, 9),
                12 => (6, 25),
                _ => (7, 2),
            };
            title["endDate"] = json!({ "year": year, "month": month, "day": day });
            title["coverImage"]["extraLarge"] = title["coverImage"]["large"].clone();
            let edges: Vec<Value> = edges
                .into_iter()
                .map(|(relation, to)| json!({ "relationType": relation, "node": card(to) }))
                .collect();
            title["relations"] = json!({ "edges": edges });
            title
        })
        .collect()
}

/// [`long_franchise`]'s titles, decoded.
pub fn long_franchise_media() -> Vec<Media> {
    long_franchise().into_iter().map(decode).collect()
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
    fn every_schedule_agrees_with_the_calendar_and_the_countdown() {
        // Three places say when an episode airs: the title's own schedule, its countdown and the
        // calendar. A disagreement would mark an episode unaired on one screen and out on another.
        for id in ALL {
            let full = media(id);
            assert!(full.aired.iter().all(|b| b.airing_at <= FIXTURE_NOW), "{id} aired ahead");
            assert!(full.upcoming.nodes.iter().all(|b| b.airing_at > FIXTURE_NOW), "{id}");
            assert!(full.aired.windows(2).all(|w| w[0].episode > w[1].episode), "newest first");
            if let Some(next) = full.next_airing_episode {
                let first =
                    full.upcoming.nodes.first().expect("an airing title has more to come");
                assert_eq!((first.episode, first.airing_at), (next.episode, next.airing_at));
            }
            let times = full.air_times();
            for row in schedule().into_iter().filter(|row| row.media.id == id) {
                assert_eq!(
                    times.get(&row.episode),
                    Some(&row.airing_at),
                    "{id} ep {}",
                    row.episode
                );
            }
        }
    }

    #[test]
    fn the_title_page_fields_stay_out_of_list_answers() {
        for id in ALL {
            let list = listed(id);
            assert!(list.tags.is_empty() && list.characters.edges.is_empty(), "{id}");
            assert!(list.aired.is_empty() && list.upcoming.nodes.is_empty(), "{id}");
            assert!(list.rankings.is_empty() && list.staff.edges.is_empty(), "{id}");
            // What every list carries stays.
            assert_eq!(list.season, media(id).season, "{id}");
            assert_eq!(list.start_date, media(id).start_date, "{id}");
        }
        assert!(
            !media(FINISHED).tags.is_empty() && !media(FINISHED).characters.edges.is_empty()
        );
    }

    #[test]
    fn the_franchises_decode_and_say_each_link_from_both_sides() {
        for (titles, count) in [(franchise(), 8), (long_franchise(), 14)] {
            let titles: Vec<Media> = titles.into_iter().map(decode).collect();
            assert_eq!(titles.len(), count);
            let links = |id: AnilistId| -> Vec<(String, AnilistId)> {
                let title = titles.iter().find(|t| t.id == id).expect("a franchise title");
                title
                    .relations
                    .edges
                    .iter()
                    .map(|e| (e.relation_type.clone().unwrap_or_default(), e.node.id))
                    .collect()
            };
            for title in &titles {
                for (relation, to) in links(title.id) {
                    let back = match relation.as_str() {
                        "SEQUEL" => "PREQUEL",
                        "PREQUEL" => "SEQUEL",
                        "PARENT" => "SIDE_STORY",
                        _ => "PARENT",
                    };
                    assert!(
                        links(to).contains(&(back.to_owned(), title.id)),
                        "{} {relation} {to}",
                        title.id
                    );
                }
            }
            assert!(
                titles.iter().all(|t| !ALL.contains(&t.id)),
                "beside the six, not among them"
            );
        }
        let short: Vec<u64> = franchise().iter().filter_map(|t| t["id"].as_u64()).collect();
        assert!(
            long_franchise()
                .iter()
                .all(|t| !short.contains(&t["id"].as_u64().unwrap_or_default())),
            "the two franchises share no title"
        );
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
