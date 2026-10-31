//! Home's world: 36 invented titles around the six, dated by the clock a scene starts on, so
//! that every shelf of Home fills as the design board draws it.
//!
//! The six fixtures are dated by [`FIXTURE_NOW`] and moved together, which suits a calendar but
//! not a season: This season needs titles of whatever season the scene starts in. These are made
//! for that moment ([`HomeClock`]): fourteen air in the current season, one starts in the next,
//! and twenty-one finished in seasons before. Each airing title has a weekly slot at a local time
//! on a weekday counted from today, so some air today whatever the date, and the episode out
//! never runs back past the season's start by more than two weeks.
//!
//! | ids | what they are | on Home |
//! |---|---|---|
//! | 950001–950008 | airing, untouched | This season, the most popular first; the first is the popular pick |
//! | 950009–950011 | airing, caught up on the list and here | This week |
//! | 950012–950013 | airing second seasons of 950018 and 950019 | Up next for you, New season |
//! | 950014–950015 | on the Planning list, one airing and one starting next season | Up next for you, Planning |
//! | 950016–950019 | finished last season and before, watched here and Completed | what the rest is found from |
//! | 950020–950031 | what 950016 to 950019 recommend | Up next for you, then Because you finished |
//! | 950032–950036 | older Slice of Life, the most finished genre | More slice of life |
//!
//! Every title, person and drawing is made up. The art is in [`crate::art`].

use anistream_core::ids::AnilistId;
use anistream_meta::Media;
use anistream_store::Store;
use serde_json::{Value, json};

use crate::fixtures::{ART_HOST, FIXTURE_NOW};

const DAY: i64 = 86_400;
const WEEK: i64 = 7 * DAY;
const MINUTE: i64 = 60;
const HOUR: i64 = 3_600;

/// The moment a scene starts and the local day it is in: what the world is dated by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HomeClock {
    /// Now, in Unix seconds.
    pub now: i64,
    /// The start of today where the viewer is, in Unix seconds.
    pub midnight: i64,
    /// Today's date where the viewer is: year, month, day.
    pub today: (i32, u8, u8),
}

impl HomeClock {
    /// `now`, with today read in `zone`, as the engine's clock reads it.
    pub fn in_zone(now: i64, zone: &jiff::tz::TimeZone) -> Self {
        let zoned = jiff::Timestamp::from_second(now)
            .expect("a time in jiff's range")
            .to_zoned(zone.clone());
        let midnight = zoned.start_of_day().map_or(now, |start| start.timestamp().as_second());
        let today = (i32::from(zoned.year()), zoned.month() as u8, zoned.day() as u8);
        Self { now, midnight, today }
    }
}

/// How a title airs, or aired.
#[derive(Clone, Copy)]
enum Run {
    /// Weekly in a slot `day` days after today (0 to 6) at `at` (hours, minutes) local time,
    /// with episode `episode` in this week's slot. Never earlier in the season than two weeks
    /// before it started.
    Airing { day: i64, at: (i64, i64), episode: u32 },
    /// Starts in the next season, `day` days after it begins, at `at` local time.
    Next { day: i64, at: (i64, i64) },
    /// Finished `back` seasons before this one.
    Finished { back: u32 },
}

/// What the viewer did with a title.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Did {
    Nothing,
    /// Watched every episode here, the last `ago` seconds before now, and Completed on the list.
    Finished {
        ago: i64,
    },
    /// Watched every episode out here, the last `ago` seconds before now, and Watching on the
    /// list at that episode.
    CaughtUp {
        ago: i64,
    },
    /// On the Planning list.
    Planning,
}

struct Spec {
    n: u32,
    romaji: &'static str,
    english: &'static str,
    format: &'static str,
    episodes: u32,
    minutes: u32,
    genres: &'static [&'static str],
    score: Option<u32>,
    popularity: u32,
    colour: &'static str,
    run: Run,
    did: Did,
    /// The title's prequel, by its number here.
    after: Option<u32>,
    /// What people who liked it rated highest, by number here.
    recommends: &'static [u32],
    synopsis: &'static str,
}

const fn airing(day: i64, h: i64, m: i64, episode: u32) -> Run {
    Run::Airing { day, at: (h, m), episode }
}

const fn back(back: u32) -> Run {
    Run::Finished { back }
}

#[rustfmt::skip]
const SPECS: [Spec; 36] = [
    // This season, untouched: This season's row, most popular first.
    Spec { n: 1, romaji: "Hokori to Hanabi", english: "Dust and Fireworks", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Slice of Life"], score: Some(83), popularity: 48_210, colour: "#24294a",
        run: airing(0, 0, 30, 5), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A fireworks maker's apprentice has one summer festival to prove that the family workshop still knows how to fill a sky." },
    Spec { n: 2, romaji: "Hotaru no Kawa", english: "Firefly River", format: "TV", episodes: 12, minutes: 24,
        genres: &["Romance", "Slice of Life"], score: Some(79), popularity: 39_120, colour: "#5f9db0",
        run: airing(0, 20, 0, 5), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "Two cousins spend a summer reopening their grandmother's river ferry, and the fireflies along its banks keep count of what neither of them says." },
    Spec { n: 3, romaji: "Kamome Shashinkan", english: "The Seagull Photo Studio", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Slice of Life"], score: Some(76), popularity: 33_870, colour: "#e08a5b",
        run: airing(0, 21, 30, 4), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A seaside photo studio takes one portrait a day. The owner's granddaughter has to decide who gets the last roll of film." },
    Spec { n: 4, romaji: "Sora Tobu Shoten", english: "The Flying Bookshop", format: "TV", episodes: 13, minutes: 24,
        genres: &["Adventure", "Fantasy"], score: Some(80), popularity: 30_550, colour: "#a9c7d6",
        run: airing(2, 22, 0, 6), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A bookshop tied to a hot-air balloon drifts from town to town, lending each one the book it needs and taking one back." },
    Spec { n: 5, romaji: "Niji no Hashira", english: "The Rainbow Pillar", format: "TV", episodes: 12, minutes: 24,
        genres: &["Fantasy", "Mystery"], score: Some(74), popularity: 26_400, colour: "#c98f8f",
        run: airing(3, 23, 0, 4), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A column of coloured light stands over a mountain village every evening. The girl who measures it notices it has started to lean." },
    Spec { n: 6, romaji: "Akari Ya", english: "The Lamp Shop", format: "TV", episodes: 12, minutes: 24,
        genres: &["Fantasy", "Slice of Life"], score: Some(78), popularity: 22_980, colour: "#262c57",
        run: airing(4, 23, 30, 6), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "Every lamp in the shop was left by someone who could not carry it any further. The keeper lights one a night and listens." },
    Spec { n: 7, romaji: "Yoake no Minato", english: "Harbour at Dawn", format: "TV", episodes: 24, minutes: 24,
        genres: &["Adventure", "Drama"], score: Some(72), popularity: 19_730, colour: "#e08a5b",
        run: airing(5, 18, 30, 3), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "The last ferry crew of a shrinking harbour town take on one more route, to an island that only shows itself at first light." },
    Spec { n: 8, romaji: "Shiroi Kujaku", english: "The White Peacock", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Mystery"], score: Some(75), popularity: 16_840, colour: "#3f7a64",
        run: airing(6, 21, 0, 6), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A white peacock walks into a hillside school every morning and leaves at noon. The student council decides to follow it home." },
    // Caught up: This week.
    Spec { n: 9, romaji: "Kaze Machi Nikki", english: "Wind Town Diary", format: "TV", episodes: 12, minutes: 24,
        genres: &["Slice of Life", "Comedy"], score: Some(81), popularity: 35_500, colour: "#9fb3c8",
        run: airing(1, 22, 30, 8), did: Did::CaughtUp { ago: 3 * DAY }, after: None, recommends: &[],
        synopsis: "A town where the wind never stops keeps a shared diary nailed to the station wall. A new arrival starts writing back." },
    Spec { n: 10, romaji: "Tsubame Hiko Kurabu", english: "The Swallow Flying Club", format: "TV", episodes: 12, minutes: 24,
        genres: &["Sports", "Comedy"], score: Some(77), popularity: 24_100, colour: "#c98f8f",
        run: airing(2, 23, 0, 6), did: Did::CaughtUp { ago: 4 * DAY }, after: None, recommends: &[],
        synopsis: "Three students rebuild a school glider club from one cracked wing and a handbook with half its pages missing." },
    Spec { n: 11, romaji: "Ginga Yubinkyoku", english: "The Galaxy Post Office", format: "TV", episodes: 12, minutes: 24,
        genres: &["Sci-Fi", "Slice of Life"], score: Some(79), popularity: 28_700, colour: "#262c57",
        run: airing(5, 17, 0, 7), did: Did::CaughtUp { ago: 5 * DAY }, after: None, recommends: &[],
        synopsis: "A post office at the edge of a space station sorts letters to people who left on ships decades ago, and some of them write back." },
    // Second seasons of what was finished: New season.
    Spec { n: 12, romaji: "Koori no Kishi Season 2", english: "The Ice Knight Season 2", format: "TV", episodes: 12, minutes: 24,
        genres: &["Action", "Drama"], score: Some(82), popularity: 41_200, colour: "#1f3048",
        run: airing(2, 20, 0, 5), did: Did::Nothing, after: Some(18), recommends: &[],
        synopsis: "The knight who guarded the frozen pass for a winter is sent south, where nobody has seen snow and nobody believes her." },
    Spec { n: 13, romaji: "Kaminari Shokudo Season 2", english: "Thunder Diner Season 2", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Slice of Life"], score: Some(77), popularity: 21_300, colour: "#3c2f57",
        run: airing(4, 18, 0, 5), did: Did::Nothing, after: Some(19), recommends: &[],
        synopsis: "The diner that only opens during storms gets a second branch, and a rival cook who can make it rain." },
    // On Planning.
    Spec { n: 14, romaji: "Kumo Kujira", english: "The Cloud Whale", format: "TV", episodes: 12, minutes: 24,
        genres: &["Adventure", "Fantasy"], score: Some(73), popularity: 18_200, colour: "#5f9db0",
        run: airing(6, 19, 30, 4), did: Did::Planning, after: None, recommends: &[],
        synopsis: "A whale made of cloud drifts over the plains once a generation. A cartographer's son climbs aboard to map it." },
    Spec { n: 15, romaji: "Amagasa Shokai", english: "The Umbrella Office", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Mystery"], score: None, popularity: 9_100, colour: "#c98f8f",
        run: Run::Next { day: 7, at: (22, 0) }, did: Did::Planning, after: None, recommends: &[],
        synopsis: "An office that returns lost umbrellas to their owners takes a case with no owner, no rain and forty umbrellas." },
    // Finished: what Home's suggestions come from.
    Spec { n: 16, romaji: "Mori no Yubinya", english: "The Forest Courier", format: "TV", episodes: 12, minutes: 24,
        genres: &["Slice of Life", "Fantasy", "Adventure"], score: Some(84), popularity: 52_800, colour: "#3f7a64",
        run: back(1), did: Did::Finished { ago: 40 * MINUTE }, after: None,
        recommends: &[20, 24, 25, 26, 27, 28, 29, 30, 31],
        synopsis: "A courier delivers parcels between villages deep in a forest that rearranges its paths each night." },
    Spec { n: 17, romaji: "Kotonoha Hanaya", english: "The Word Flower Shop", format: "TV", episodes: 12, minutes: 24,
        genres: &["Slice of Life", "Romance"], score: Some(80), popularity: 31_800, colour: "#7f9a6a",
        run: back(2), did: Did::Finished { ago: DAY + 6 * HOUR }, after: None, recommends: &[21],
        synopsis: "A florist arranges bouquets that say what their buyers cannot. One customer keeps ordering the same one." },
    Spec { n: 18, romaji: "Koori no Kishi", english: "The Ice Knight", format: "TV", episodes: 12, minutes: 24,
        genres: &["Action", "Drama"], score: Some(81), popularity: 58_900, colour: "#1f3048",
        run: back(4), did: Did::Finished { ago: 2 * DAY + 6 * HOUR }, after: None, recommends: &[22],
        synopsis: "A young knight is posted alone to a frozen mountain pass for one winter, with orders nobody will explain." },
    Spec { n: 19, romaji: "Kaminari Shokudo", english: "Thunder Diner", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Slice of Life"], score: Some(76), popularity: 27_400, colour: "#3c2f57",
        run: back(6), did: Did::Finished { ago: 4 * DAY + 2 * HOUR }, after: None, recommends: &[23],
        synopsis: "A diner on a hilltop only opens when a storm is overhead, and its cook only ever serves what the thunder orders." },
    // What they recommend.
    Spec { n: 20, romaji: "Hoshifuru Ryokan", english: "The Inn of Falling Stars", format: "TV", episodes: 12, minutes: 24,
        genres: &["Fantasy", "Slice of Life"], score: Some(79), popularity: 33_300, colour: "#262c57",
        run: back(3), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A mountain inn catches the stars that fall into its hot spring and keeps them in jars until their owners come asking." },
    Spec { n: 21, romaji: "Garasu no Niwashi", english: "The Glass Gardener", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Fantasy"], score: Some(78), popularity: 25_600, colour: "#e3e9e4",
        run: back(2), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A glassblower grows a greenhouse at the edge of a frozen sea, tending the one flower that tells the ferry when it is safe to sail." },
    Spec { n: 22, romaji: "Akane Iro no Tegami", english: "Letters in Madder Red", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Mystery"], score: Some(83), popularity: 22_100, colour: "#5a2c2c",
        run: back(1), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A calligrapher restores letters damaged in a flood, one family at a time. Each episode is one letter and one house." },
    Spec { n: 23, romaji: "Yukiguni Densha", english: "The Snow Country Train", format: "TV", episodes: 12, minutes: 24,
        genres: &["Adventure", "Drama"], score: Some(75), popularity: 19_900, colour: "#1f3048",
        run: back(5), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "The last train of the winter line runs one more season after the line is closed, because its driver has not been told." },
    Spec { n: 24, romaji: "Kumo Hakobi", english: "The Cloud Carriers", format: "TV", episodes: 12, minutes: 24,
        genres: &["Adventure", "Slice of Life"], score: Some(77), popularity: 24_800, colour: "#7f9a6a",
        run: back(4), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "Two sisters move clouds from where they are not wanted to where they are, one cart at a time." },
    Spec { n: 25, romaji: "Hoshikaze Gakuen", english: "Star Wind Academy", format: "TV", episodes: 24, minutes: 24,
        genres: &["Sci-Fi", "Comedy"], score: Some(74), popularity: 30_100, colour: "#262c57",
        run: back(8), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A boarding school for solar sailors loses its only ship on the first day of term and has to build another." },
    Spec { n: 26, romaji: "Sora no Fune", english: "The Sky Ship", format: "MOVIE", episodes: 1, minutes: 112,
        genres: &["Adventure", "Fantasy"], score: Some(85), popularity: 20_400, colour: "#e08a5b",
        run: back(5), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A stowaway on a cargo airship learns the captain is flying it home for the last time." },
    Spec { n: 27, romaji: "Yume Ressha", english: "Dream Express", format: "TV", episodes: 12, minutes: 24,
        genres: &["Fantasy", "Mystery"], score: Some(72), popularity: 15_800, colour: "#3c2f57",
        run: back(2), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A sleeper train carries passengers through each other's dreams. The conductor is looking for one that is not his." },
    Spec { n: 28, romaji: "Neko Machi no Chocho", english: "The Mayor of Cat Town", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Slice of Life"], score: Some(76), popularity: 34_700, colour: "#5f9db0",
        run: back(12), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A cat is elected mayor of a fishing town by mistake, and nobody wants to be the one to tell it." },
    Spec { n: 29, romaji: "Mizuiro Yubin", english: "Pale Blue Post", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Slice of Life"], score: Some(79), popularity: 18_300, colour: "#5f9db0",
        run: back(4), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A postwoman on a chain of small islands delivers the mail by rowing boat and the news by heart." },
    Spec { n: 30, romaji: "Haru Machi Densetsu", english: "Spring Town Legend", format: "TV", episodes: 12, minutes: 24,
        genres: &["Fantasy", "Romance"], score: Some(73), popularity: 14_200, colour: "#3f7a64",
        run: back(6), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A town where spring arrives a week late every year asks the girl who brings it to come on time, just once." },
    Spec { n: 31, romaji: "Yoruiro Gakudan", english: "The Night Colour Band", format: "MOVIE", episodes: 1, minutes: 98,
        genres: &["Drama", "Music"], score: Some(81), popularity: 17_600, colour: "#1f3048",
        run: back(1), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A band that only plays after midnight has one concert left before its hall is torn down." },
    // Older Slice of Life: the genre rail.
    Spec { n: 32, romaji: "Ameagari Shokudo", english: "After the Rain Diner", format: "TV", episodes: 12, minutes: 24,
        genres: &["Slice of Life"], score: Some(82), popularity: 61_200, colour: "#2c3b63",
        run: back(4), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A diner opens for the hour after each rain, and serves whoever was caught in it." },
    Spec { n: 33, romaji: "Mizutama Kissa", english: "Polka Dot Cafe", format: "TV", episodes: 12, minutes: 24,
        genres: &["Comedy", "Slice of Life"], score: Some(78), popularity: 52_100, colour: "#a9c7d6",
        run: back(8), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "Four friends run a cafe in a converted bus and move it whenever the town council finds them." },
    Spec { n: 34, romaji: "Kame no Ongakukai", english: "The Tortoise Concert", format: "TV", episodes: 12, minutes: 24,
        genres: &["Music", "Slice of Life"], score: Some(75), popularity: 44_900, colour: "#a9c7d6",
        run: back(9), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A village orchestra rehearses a single piece for a hundred years, one player handing it to the next." },
    Spec { n: 35, romaji: "Nami Kiri Kurabu", english: "The Wave Cutting Club", format: "TV", episodes: 12, minutes: 24,
        genres: &["Slice of Life", "Sports"], score: Some(77), popularity: 38_300, colour: "#e08a5b",
        run: back(3), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "A school surf club with one board and five members takes turns, one wave each, all summer." },
    Spec { n: 36, romaji: "Tanpopo Yuyake", english: "Dandelion Sunset", format: "TV", episodes: 12, minutes: 24,
        genres: &["Drama", "Slice of Life"], score: Some(74), popularity: 29_600, colour: "#e08a5b",
        run: back(1), did: Did::Nothing, after: None, recommends: &[],
        synopsis: "Two old friends walk the length of a river from its mouth to its source the summer before one of them moves away." },
];

/// The first id of the world; title `n` is `FIRST + n - 1`.
pub(crate) const FIRST: u32 = 950_001;

fn id_of(n: u32) -> u32 {
    FIRST + n - 1
}

/// Every id of the world, in its own order.
pub fn ids() -> Vec<AnilistId> {
    SPECS.iter().map(|s| AnilistId::new(id_of(s.n))).collect()
}

/// The world's titles at `clock`: what the fake answers for each, and what the viewer did.
pub struct HomeWorld {
    /// Each title as AniList's single-title query answers it, broadcast times at
    /// [`FIXTURE_NOW`]'s clock, as the fixtures' are, so the fake moves them to the scene's.
    pub titles: Vec<Value>,
    /// The titles of the season the clock is in, which the fake's seasonal browse answers.
    pub this_season: Vec<AnilistId>,
    /// Every broadcast, `(title, episode, airing_at)` at [`FIXTURE_NOW`]'s clock.
    pub broadcasts: Vec<(AnilistId, u32, i64)>,
    /// The viewer's list entries for the world, in the fake's library shape.
    pub entries: Vec<Value>,
    history: Vec<(AnilistId, u32, i64)>,
}

impl HomeWorld {
    /// The world at `clock`.
    pub fn at(clock: HomeClock) -> Self {
        let shift = clock.now - FIXTURE_NOW;
        let (season, year) = season_of(clock.today.0, clock.today.1);
        let mut world = Self {
            titles: Vec::new(),
            this_season: Vec::new(),
            broadcasts: Vec::new(),
            entries: Vec::new(),
            history: Vec::new(),
        };
        for spec in &SPECS {
            let id = id_of(spec.n);
            let mut title = card(spec, clock);
            title["idMal"] = Value::Null;
            title["description"] = json!(spec.synopsis);
            title["episodes"] = json!(spec.episodes);
            title["duration"] = json!(spec.minutes);
            title["genres"] = json!(spec.genres);
            title["averageScore"] = json!(spec.score);
            title["popularity"] = json!(spec.popularity);
            title["isAdult"] = json!(false);
            title["source"] = json!("ORIGINAL");
            title["synonyms"] = json!([]);
            title["coverImage"]["extraLarge"] = json!(format!("{ART_HOST}cover/{id}-xl.jpg"));
            title["bannerImage"] = json!(format!("{ART_HOST}banner/{id}.jpg"));
            title["studios"] = json!({ "nodes": [] });
            for empty in ["externalLinks", "streamingEpisodes", "tags", "rankings"] {
                title[empty] = json!([]);
            }
            title["characters"] = json!({ "edges": [] });
            title["staff"] = json!({ "edges": [] });
            title["trailer"] = Value::Null;

            // The broadcasts, real times first.
            let slots = slots(spec, clock);
            let (first, last) = (slots.first().copied(), slots.last().copied());
            let out: Vec<(u32, i64)> = slots
                .iter()
                .copied()
                .zip(1..)
                .map(|(at, e)| (e, at))
                .filter(|(_, at)| *at <= clock.now)
                .collect();
            let coming: Vec<(u32, i64)> = slots
                .iter()
                .copied()
                .zip(1..)
                .map(|(at, e)| (e, at))
                .filter(|(_, at)| *at > clock.now)
                .collect();
            let wire = |at: i64| at - shift;
            match spec.run {
                Run::Finished { back } => {
                    let (s, y) = seasons_back(season, year, back);
                    title["season"] = json!(s);
                    title["seasonYear"] = json!(y);
                    let start = days_from_civil(
                        i64::from(y),
                        season_month(s),
                        6 + i64::from(spec.n % 9),
                    );
                    let end = start + i64::from(spec.episodes.saturating_sub(1)) * 7;
                    title["status"] = json!("FINISHED");
                    title["startDate"] = date(start);
                    title["endDate"] = date(end);
                    title["nextAiringEpisode"] = Value::Null;
                    title["upcoming"] = json!({ "nodes": [] });
                    title["aired"] = json!([]);
                }
                Run::Airing { .. } | Run::Next { .. } => {
                    let today = days_from_civil(
                        i64::from(clock.today.0),
                        i64::from(clock.today.1),
                        i64::from(clock.today.2),
                    );
                    let local_day = |at: i64| today + (at - clock.midnight).div_euclid(DAY);
                    let starts = first.map(local_day).unwrap_or(today);
                    let (s, y) = match spec.run {
                        Run::Next { .. } => next_season(season, year),
                        _ => (season, year),
                    };
                    title["season"] = json!(s);
                    title["seasonYear"] = json!(y);
                    title["startDate"] = date(starts);
                    let done = out.len() as u32 == spec.episodes;
                    title["status"] = json!(match (out.is_empty(), done) {
                        (true, _) => "NOT_YET_RELEASED",
                        (false, true) => "FINISHED",
                        (false, false) => "RELEASING",
                    });
                    title["endDate"] = if done {
                        date(last.map(local_day).unwrap_or(today))
                    } else {
                        json!({ "year": null, "month": null, "day": null })
                    };
                    title["nextAiringEpisode"] = coming.first().map_or(Value::Null, |(e, at)| {
                        json!({ "episode": e, "airingAt": wire(*at), "timeUntilAiring": at - clock.now })
                    });
                    let nodes: Vec<Value> = coming
                        .iter()
                        .take(3)
                        .map(|(e, at)| json!({ "episode": e, "airingAt": wire(*at) }))
                        .collect();
                    title["upcoming"] = json!({ "nodes": nodes });
                    let aired: Vec<Value> = out
                        .iter()
                        .rev()
                        .map(|(e, at)| json!({ "episode": e, "airingAt": wire(*at) }))
                        .collect();
                    title["aired"] = json!(aired);
                    for (e, at) in out.iter().chain(&coming) {
                        world.broadcasts.push((AnilistId::new(id), *e, wire(*at)));
                    }
                    if matches!(spec.run, Run::Airing { .. }) {
                        world.this_season.push(AnilistId::new(id));
                    }
                }
            }

            // Relations both ways, and recommendations.
            let mut edges: Vec<Value> = Vec::new();
            if let Some(before) = spec.after {
                edges.push(
                    json!({ "relationType": "PREQUEL", "node": card(spec_of(before), clock) }),
                );
            }
            for later in SPECS.iter().filter(|s| s.after == Some(spec.n)) {
                edges.push(json!({ "relationType": "SEQUEL", "node": card(later, clock) }));
            }
            title["relations"] = json!({ "edges": edges });
            let nodes: Vec<Value> = spec
                .recommends
                .iter()
                .map(|n| json!({ "mediaRecommendation": card(spec_of(*n), clock) }))
                .collect();
            title["recommendations"] = json!({ "nodes": nodes });

            // What the viewer did.
            let entry = |status: &str, progress: u32, ago: i64| {
                json!({
                    "id": 81_000 + spec.n,
                    "mediaId": id,
                    "status": status,
                    "progress": progress,
                    "score": 0,
                    "updatedAt": clock.now - ago,
                })
            };
            match spec.did {
                Did::Nothing => {}
                Did::Finished { ago } => {
                    world.entries.push(entry("COMPLETED", spec.episodes, ago));
                    world.history.push((AnilistId::new(id), spec.episodes, ago));
                }
                Did::CaughtUp { ago } => {
                    let watched = out.len() as u32;
                    world.entries.push(entry("CURRENT", watched, ago));
                    world.history.push((AnilistId::new(id), watched, ago));
                }
                Did::Planning => world.entries.push(entry("PLANNING", 0, 7 * DAY)),
            }
            world.titles.push(title);
        }
        world
    }

    /// The world's list entries as statuses, as [`fixtures::library_statuses`] has the
    /// fixture library's.
    ///
    /// [`fixtures::library_statuses`]: crate::fixtures::library_statuses
    pub fn library_statuses(&self) -> Vec<(AnilistId, &str)> {
        self.entries
            .iter()
            .filter_map(|e| {
                Some((AnilistId::new(e["mediaId"].as_u64()? as u32), e["status"].as_str()?))
            })
            .collect()
    }

    /// The titles decoded, each with as many episodes as are out: what a source has up.
    pub fn provided(&self) -> Vec<(Media, u32)> {
        self.titles
            .iter()
            .map(|t| {
                let out = t["aired"].as_array().map_or(0, Vec::len) as u32;
                let episodes = if t["status"] == "FINISHED" {
                    t["episodes"].as_u64().unwrap_or(1) as u32
                } else {
                    out
                };
                (crate::fixtures::decode(crate::fixtures::listed_form(t)), episodes)
            })
            .collect()
    }

    /// The viewer's history of the world in `store`, as of `now`: the finished titles watched
    /// to the end, The Forest Courier last, and the caught-up ones to their latest episode. Each
    /// is out of Continue, as a viewer leaves a finished or caught-up show, so Continue is what
    /// it was; the history is what Home suggests from.
    ///
    /// The finished titles' records are in the metadata cache too, as of when they were last
    /// watched, as the app keeps a title it has played: what Home's taste rail reads, so it is
    /// there from the first load. They have no broadcasts to go stale.
    pub fn seed_history(&self, store: &Store, now: i64) {
        for spec in SPECS.iter() {
            let Did::Finished { ago } = spec.did else { continue };
            let id = id_of(spec.n);
            let title =
                self.titles.iter().find(|t| t["id"] == id).expect("a title of the world");
            let media = crate::fixtures::decode(title.clone());
            let payload = serde_json::to_string(&media).expect("a title serialises");
            store.cache_media(media.id, &payload, now - ago).expect("caching a seeded title");
        }
        // Oldest first, since each watch moves its title to the top.
        let mut history = self.history.clone();
        history.sort_by_key(|(_, _, ago)| std::cmp::Reverse(*ago));
        for (id, episodes, ago) in history {
            if episodes == 0 {
                continue;
            }
            crate::seed::watched(store, id, episodes, now - ago);
            store.hide_from_continue(id).expect("hiding a seeded title from Continue");
        }
    }
}

fn spec_of(n: u32) -> &'static Spec {
    SPECS.iter().find(|s| s.n == n).expect("a title of the world")
}

/// A title as a relation or a recommendation carries it.
fn card(spec: &Spec, clock: HomeClock) -> Value {
    let id = id_of(spec.n);
    let (season, year) = season_of(clock.today.0, clock.today.1);
    let (s, y) = match spec.run {
        Run::Finished { back } => seasons_back(season, year, back),
        Run::Next { .. } => next_season(season, year),
        Run::Airing { .. } => (season, year),
    };
    json!({
        "id": id,
        "title": { "romaji": spec.romaji, "english": spec.english, "native": null },
        "format": spec.format,
        "coverImage": { "large": format!("{ART_HOST}cover/{id}.jpg"), "color": spec.colour },
        "season": s,
        "seasonYear": y,
    })
}

/// Each episode's broadcast in real seconds, first to last. None for a finished title.
fn slots(spec: &Spec, clock: HomeClock) -> Vec<i64> {
    let (slot, episode) = match spec.run {
        Run::Finished { .. } => return Vec::new(),
        Run::Airing { day, at: (h, m), episode } => {
            // Never more than two weeks before the season began.
            let (season, year) = season_of(clock.today.0, clock.today.1);
            let today = days_from_civil(
                i64::from(clock.today.0),
                i64::from(clock.today.1),
                i64::from(clock.today.2),
            );
            let began = days_from_civil(i64::from(year), season_month(season), 1);
            let weeks_in = (today + day - began).div_euclid(7);
            let most = u32::try_from(weeks_in + 3).unwrap_or(1).max(1);
            (
                clock.midnight + day * DAY + h * HOUR + m * MINUTE,
                episode.min(most).min(spec.episodes),
            )
        }
        Run::Next { day, at: (h, m) } => {
            let (season, year) = season_of(clock.today.0, clock.today.1);
            let (next, next_year) = next_season(season, year);
            let today = days_from_civil(
                i64::from(clock.today.0),
                i64::from(clock.today.1),
                i64::from(clock.today.2),
            );
            let starts = days_from_civil(i64::from(next_year), season_month(next), 1) + day;
            (clock.midnight + (starts - today) * DAY + h * HOUR + m * MINUTE, 1)
        }
    };
    (1..=spec.episodes).map(|e| slot + (i64::from(e) - i64::from(episode)) * WEEK).collect()
}

/// AniList's season of a month, and its year.
fn season_of(year: i32, month: u8) -> (&'static str, u16) {
    let season = match month {
        1..=3 => "WINTER",
        4..=6 => "SPRING",
        7..=9 => "SUMMER",
        _ => "FALL",
    };
    (season, year.max(0) as u16)
}

const SEASONS: [&str; 4] = ["WINTER", "SPRING", "SUMMER", "FALL"];

fn season_index(season: &str) -> usize {
    SEASONS.iter().position(|s| *s == season).unwrap_or(0)
}

fn season_month(season: &str) -> i64 {
    season_index(season) as i64 * 3 + 1
}

fn next_season(season: &'static str, year: u16) -> (&'static str, u16) {
    seasons_forward(season, year, 1)
}

fn seasons_back(season: &'static str, year: u16, back: u32) -> (&'static str, u16) {
    let at = i64::from(year) * 4 + season_index(season) as i64 - i64::from(back);
    (SEASONS[at.rem_euclid(4) as usize], at.div_euclid(4) as u16)
}

fn seasons_forward(season: &'static str, year: u16, ahead: u32) -> (&'static str, u16) {
    let at = i64::from(year) * 4 + season_index(season) as i64 + i64::from(ahead);
    (SEASONS[at.rem_euclid(4) as usize], at.div_euclid(4) as u16)
}

/// Days since 1970-01-01 of a civil date, the standard algorithm. A day past a month's end
/// rolls into the next.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count, as AniList's fuzzy date.
fn date(days: i64) -> Value {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    json!({ "year": year, "month": month, "day": day })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Saturday 31 October 2026, 14:20 UTC, read in UTC.
    const CLOCK: HomeClock =
        HomeClock { now: 1_793_456_400, midnight: 1_793_404_800, today: (2026, 10, 31) };

    #[test]
    fn the_civil_dates_round_trip() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(
            date(days_from_civil(2026, 10, 31)),
            json!({ "year": 2026, "month": 10, "day": 31 })
        );
        assert_eq!(
            date(days_from_civil(2024, 2, 29)),
            json!({ "year": 2024, "month": 2, "day": 29 })
        );
        assert_eq!(CLOCK.midnight, days_from_civil(2026, 10, 31) * DAY);
        assert_eq!(seasons_back("WINTER", 2027, 1), ("FALL", 2026));
        assert_eq!(next_season("FALL", 2026), ("WINTER", 2027));
    }

    #[test]
    fn every_title_decodes_and_its_broadcasts_agree() {
        let world = HomeWorld::at(CLOCK);
        assert_eq!(world.titles.len(), 36);
        let shift = CLOCK.now - FIXTURE_NOW;
        for (media, _) in world.provided() {
            let title = world.titles.iter().find(|t| t["id"] == media.id.get()).unwrap();
            let full = crate::fixtures::decode(title.clone());
            assert!(
                full.aired.iter().all(|b| b.airing_at + shift <= CLOCK.now),
                "{}",
                media.id
            );
            assert!(full.upcoming.nodes.iter().all(|b| b.airing_at + shift > CLOCK.now));
            if let Some(next) = full.next_airing_episode {
                assert_eq!(
                    next.airing_at - next.time_until_airing,
                    FIXTURE_NOW,
                    "{}",
                    media.id
                );
            }
            for row in world.broadcasts.iter().filter(|b| b.0 == media.id) {
                assert_eq!(full.air_times().get(&row.1).copied().or(Some(row.2)), Some(row.2));
            }
        }
    }

    #[test]
    fn this_season_is_the_clocks_and_something_of_it_airs_today() {
        let world = HomeWorld::at(CLOCK);
        assert_eq!(world.this_season.len(), 14);
        for id in &world.this_season {
            let title = world.titles.iter().find(|t| t["id"] == id.get()).unwrap();
            assert_eq!(
                (title["season"].as_str(), title["seasonYear"].as_u64()),
                (Some("FALL"), Some(2026))
            );
            assert_eq!(title["status"], "RELEASING", "{id}");
        }
        let today = |at: i64| at + CLOCK.now - FIXTURE_NOW - CLOCK.midnight;
        let airs_today =
            world.broadcasts.iter().filter(|b| (0..DAY).contains(&today(b.2))).count();
        assert!(airs_today >= 3, "{airs_today}");
    }

    #[test]
    fn on_a_seasons_first_day_nothing_started_more_than_two_weeks_before() {
        let clock = HomeClock {
            now: days_from_civil(2027, 1, 1) * DAY + 12 * HOUR,
            midnight: days_from_civil(2027, 1, 1) * DAY,
            today: (2027, 1, 1),
        };
        let world = HomeWorld::at(clock);
        for id in &world.this_season {
            let title = world.titles.iter().find(|t| t["id"] == id.get()).unwrap();
            let d = &title["startDate"];
            let started = days_from_civil(
                d["year"].as_i64().unwrap(),
                d["month"].as_i64().unwrap(),
                d["day"].as_i64().unwrap(),
            );
            assert!(started >= days_from_civil(2026, 12, 18), "{id} {d}");
            assert_eq!(title["season"], "WINTER");
            assert!(
                !title["aired"].as_array().unwrap().is_empty()
                    || title["status"] == "NOT_YET_RELEASED"
            );
        }
    }

    #[test]
    fn relations_and_recommendations_stay_in_the_world() {
        let world = HomeWorld::at(CLOCK);
        let ids: Vec<u64> = world.titles.iter().filter_map(|t| t["id"].as_u64()).collect();
        for title in &world.titles {
            for edge in title["relations"]["edges"].as_array().unwrap() {
                let to = edge["node"]["id"].as_u64().unwrap();
                let back = if edge["relationType"] == "SEQUEL" { "PREQUEL" } else { "SEQUEL" };
                let other = world.titles.iter().find(|t| t["id"] == to).unwrap();
                assert!(
                    other["relations"]["edges"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|e| e["relationType"] == back && e["node"]["id"] == title["id"]),
                    "{} {}",
                    title["id"],
                    to
                );
            }
            for node in title["recommendations"]["nodes"].as_array().unwrap() {
                assert!(ids.contains(&node["mediaRecommendation"]["id"].as_u64().unwrap()));
            }
        }
    }

    #[test]
    fn the_history_finishes_three_slice_of_life_titles_and_the_courier_last() {
        let store = crate::store();
        let world = HomeWorld::at(CLOCK);
        world.seed_history(&store, CLOCK.now);
        let watched = store.watched_titles(50).unwrap();
        assert_eq!(watched.first().map(|p| p.anilist_id), Some(AnilistId::new(id_of(16))));
        assert!(store.continue_list(50).unwrap().is_empty(), "out of Continue");
        assert_eq!(store.cached_media_count().unwrap(), 4, "the finished titles are cached");
        let finished: Vec<&Spec> =
            SPECS.iter().filter(|s| matches!(s.did, Did::Finished { .. })).collect();
        let slice = finished.iter().filter(|s| s.genres.contains(&"Slice of Life")).count();
        assert_eq!(slice, 3);
    }
}
