//! A provider that carries the fixture titles: search hits, episodes and a stream for each.
//!
//! Built on `anistream_providers::MockProvider`, which answers by key, so the app's own
//! matching runs against it unchanged: a search lists every title, the matcher picks the one
//! the AniList title describes, and that key's episodes come back.
//!
//! The stream is a `lavfi` test pattern, which mpv renders itself — a binge test can play it
//! with no file and no network.

use std::time::Duration;

use anistream_core::{
    ids::{AnilistId, ProviderKey},
    media::{Episode, SearchHit},
    stream::{Stream, StreamKind},
};
use anistream_providers::{MockProvider, ProviderRegistry};

use crate::fixtures::{self, AIRING, AIRING_NO_MAL, FINISHED, MOVIE, ONA, SEQUEL};

/// The fixture provider's id.
pub const PROVIDER_ID: &str = "fixture";

/// The episode of [`SEQUEL`] the source claims is filler, for the filler-skipping paths.
pub const FILLER_EPISODE: u32 = 7;

/// [`FINISHED`]'s episode titles, the only fixture source that names its episodes.
const FINISHED_TITLES: [&str; 12] = [
    "The Lantern Road",
    "A Town Without Night",
    "The Ferryman's Ledger",
    "Embers in the Snow",
    "What the Beacons Remember",
    "The Cartographer's Daughter",
    "Rain over Kiriya Pass",
    "A Letter Never Sent",
    "The Last Lamplighter",
    "Where the Roads Cross",
    "The Sky Archive",
    "Home, by Starlight",
];

/// The provider's own key for a fixture title.
pub fn key(id: AnilistId) -> ProviderKey {
    ProviderKey::new(format!("fixture-{}", id.get()))
}

/// How many episodes the source has up for a title: all of a finished one, and only what has
/// aired of one still airing.
pub fn episode_count(id: AnilistId) -> u32 {
    match id {
        AIRING => 5,
        AIRING_NO_MAL => 8,
        _ => fixtures::media(id).episodes.unwrap_or(1),
    }
}

/// A title's episodes as the source lists them.
///
/// [`FINISHED`]'s are the full set: titles, still frames, air dates and a synopsis for the
/// first. [`ONA`]'s have stills and nothing else, so a title with no banner still has wide art
/// a front end can show. [`SEQUEL`] claims episode [`FILLER_EPISODE`] is filler. The rest are
/// bare numbers and a runtime, as most sources list them.
pub fn episodes(id: AnilistId) -> Vec<Episode> {
    let runtime = Duration::from_secs_f64(fixtures::runtime_secs(id));
    (1..=episode_count(id))
        .map(|n| {
            let bare = Episode { duration: Some(runtime), ..Episode::new(n) };
            match id {
                FINISHED => Episode {
                    title: Some(FINISHED_TITLES[n as usize - 1].to_owned()),
                    thumbnail: Some(still(id, n)),
                    // Weekly from Saturday 2024-04-06.
                    air_date: Some(air_date(n)),
                    description: (n == 1).then(|| {
                        "A lamplighter wakes on the star road with a lantern and no name."
                            .into()
                    }),
                    ..bare
                },
                ONA => Episode { thumbnail: Some(still(id, n)), ..bare },
                MOVIE => Episode { title: Some(fixtures::title(MOVIE)), ..bare },
                SEQUEL => Episode { filler: (n == FILLER_EPISODE).then_some(true), ..bare },
                _ => bare,
            }
        })
        .collect()
}

/// Where an episode's still is, under [`fixtures::ART_HOST`].
fn still(id: AnilistId, n: u32) -> String {
    format!("{}ep/{}/{n}.jpg", fixtures::ART_HOST, id.get())
}

fn air_date(n: u32) -> String {
    // April has 30 days, May 31; twelve weeks from the 6th end on 22 June.
    let day = 6 + (n - 1) * 7;
    match day {
        ..=30 => format!("2024-04-{day:02}"),
        31..=61 => format!("2024-05-{:02}", day - 30),
        _ => format!("2024-06-{:02}", day - 61),
    }
}

/// What a search on the source lists for every fixture title.
///
/// The romaji title with the English one as a synonym, the episode count the source has up,
/// the year and the format: the fields the matcher's gates read, so each AniList title matches
/// its own entry confidently and no other.
pub fn hits() -> Vec<SearchHit> {
    fixtures::ALL
        .iter()
        .map(|id| {
            let media = fixtures::listed(*id);
            SearchHit {
                key: key(*id),
                title: media.title.romaji.clone().unwrap_or_default(),
                synonyms: media.title.english.iter().cloned().collect(),
                episode_count: Some(episode_count(*id)),
                year: media.season_year,
                format: media.format,
            }
        })
        .collect()
}

/// A test pattern mpv draws itself, attributed to the fixture provider: an episode's 24 minutes,
/// so a device run stays on the episode it started rather than ending inside Up Next's window
/// and going on to the next.
pub fn clip() -> Stream {
    Stream {
        provider_id: PROVIDER_ID.into(),
        ..Stream::new("av://lavfi:testsrc=duration=1440:size=320x240:rate=30", StreamKind::Mp4)
    }
}

/// The fixture provider, before it is boxed, so a test can still make it slow, failing or
/// withheld, or take its call counter.
pub fn provider() -> MockProvider {
    provider_with_art(fixtures::ART_HOST)
}

/// The fixture provider with its episode stills under `art` rather than
/// [`fixtures::ART_HOST`]: under the fake's own address, when it serves art.
pub fn provider_with_art(art: &str) -> MockProvider {
    let mut provider = MockProvider::new(PROVIDER_ID).with_hits(hits());
    for id in fixtures::ALL {
        let episodes = episodes(id)
            .into_iter()
            .map(|e| Episode {
                thumbnail: e.thumbnail.map(|t| t.replacen(fixtures::ART_HOST, art, 1)),
                ..e
            })
            .collect();
        provider = provider
            .with_episodes_for(key(id), episodes)
            .with_streams_for(key(id), vec![clip()]);
    }
    provider
}

/// `provider` with the test kit's long franchise beside the six ([`fixtures::long_franchise`]):
/// a hit for each of its titles, named as the fixtures' are, and bare episodes, so its pages have
/// episode tables where a scene's fake AniList has the franchise.
pub fn with_long_franchise(provider: MockProvider) -> MockProvider {
    let titles = fixtures::long_franchise_media();
    let mut every = hits();
    every.extend(titles.iter().map(|media| SearchHit {
        key: key(media.id),
        title: media.title.romaji.clone().unwrap_or_default(),
        synonyms: media.title.english.iter().cloned().collect(),
        episode_count: media.episodes,
        year: media.season_year,
        format: media.format,
    }));
    let mut provider = provider.with_hits(every);
    for media in &titles {
        let runtime = Duration::from_secs(u64::from(media.duration.unwrap_or(24)) * 60);
        let episodes = (1..=media.episodes.unwrap_or(1))
            .map(|n| Episode { duration: Some(runtime), ..Episode::new(n) })
            .collect();
        provider = provider
            .with_episodes_for(key(media.id), episodes)
            .with_streams_for(key(media.id), vec![clip()]);
    }
    provider
}

/// A registry holding only the fixture provider.
pub fn registry() -> ProviderRegistry {
    ProviderRegistry::new(vec![provider().arc()])
}

/// A registry holding only the fixture provider, its stills under `art`.
pub fn registry_with_art(art: &str) -> ProviderRegistry {
    ProviderRegistry::new(vec![provider_with_art(art).arc()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::media::Translation;
    use anistream_providers::{Answer, first_answer};

    #[test]
    fn air_dates_run_weekly_across_the_month_ends() {
        let dates: Vec<String> = (1..=12).map(air_date).collect();
        assert_eq!(dates[0], "2024-04-06");
        assert_eq!(dates[3], "2024-04-27");
        assert_eq!(dates[4], "2024-05-04");
        assert_eq!(dates[8], "2024-06-01");
        assert_eq!(dates[11], "2024-06-22");
    }

    #[tokio::test]
    async fn every_title_matches_its_own_entry_and_gets_its_own_episodes() {
        let store = crate::store();
        let registry = registry();
        for id in fixtures::ALL {
            let target = fixtures::media(id).match_target();
            let answer = first_answer(
                &store,
                &registry,
                id,
                &target,
                Translation::Sub,
                fixtures::FIXTURE_NOW,
                |source, key| async move {
                    source.episodes(&key, Translation::Sub, fixtures::FIXTURE_NOW).await
                },
            )
            .await;
            let Answer::Answered { provider_id, key: matched, value } = answer else {
                panic!("{id} was not answered: {}", answer.explain());
            };
            assert_eq!((provider_id.as_str(), &matched), (PROVIDER_ID, &key(id)), "{id}");
            assert_eq!(value.len() as u32, episode_count(id), "{id}");
        }
    }

    #[tokio::test]
    async fn the_long_franchise_beside_them_matches_each_season_to_its_own_entry() {
        let store = crate::store();
        let registry = ProviderRegistry::new(vec![with_long_franchise(provider()).arc()]);
        let titles = fixtures::long_franchise_media();
        for media in titles.iter().chain([fixtures::media(FINISHED)].iter()) {
            let answer = first_answer(
                &store,
                &registry,
                media.id,
                &media.match_target(),
                Translation::Sub,
                fixtures::FIXTURE_NOW,
                |source, key| async move {
                    source.episodes(&key, Translation::Sub, fixtures::FIXTURE_NOW).await
                },
            )
            .await;
            let Answer::Answered { key: matched, value, .. } = answer else {
                panic!("{} was not answered: {}", media.id, answer.explain());
            };
            assert_eq!(matched, key(media.id), "{}", media.id);
            assert_eq!(Some(value.len() as u32), media.episodes, "{}", media.id);
        }
    }

    #[tokio::test]
    async fn each_title_has_a_stream_and_an_unknown_key_has_none() {
        let registry = registry();
        let streams = registry.resolve(&key(MOVIE), "1", Translation::Sub, 0).await;
        assert_eq!(streams.value.expect("a stream")[0].url, clip().url);
        let unknown =
            registry.resolve(&ProviderKey::new("nope"), "1", Translation::Sub, 0).await;
        assert!(unknown.value.is_none());
    }

    #[tokio::test]
    async fn the_stills_follow_the_art_base() {
        use anistream_core::traits::Provider;
        let base = "http://127.0.0.1:9/art/";
        let episodes = provider_with_art(base)
            .episodes(&key(FINISHED), Translation::Sub)
            .await
            .expect("the finished title's episodes");
        assert_eq!(
            episodes[2].thumbnail.as_deref(),
            Some("http://127.0.0.1:9/art/ep/900001/3.jpg")
        );
        let plain = provider()
            .episodes(&key(FINISHED), Translation::Sub)
            .await
            .expect("the finished title's episodes");
        assert_eq!(plain[2].thumbnail.as_deref(), Some("https://img.example/ep/900001/3.jpg"));
    }

    /// The two titles with no banner: one has stills to show in its place, the other nothing.
    #[test]
    fn of_the_bannerless_titles_one_has_stills_and_one_has_none() {
        let ona = episodes(ONA);
        assert_eq!(ona[0].thumbnail.as_deref(), Some("https://img.example/ep/900005/1.jpg"));
        assert!(ona.iter().all(|e| e.thumbnail.is_some()));
        assert!(episodes(AIRING_NO_MAL).iter().all(|e| e.thumbnail.is_none()));
    }

    #[test]
    fn the_sequel_marks_one_filler_episode() {
        let filler: Vec<_> =
            episodes(SEQUEL).into_iter().filter(|e| e.filler == Some(true)).collect();
        assert_eq!(filler.len(), 1);
        assert_eq!(filler[0].number.as_str(), FILLER_EPISODE.to_string());
    }
}
