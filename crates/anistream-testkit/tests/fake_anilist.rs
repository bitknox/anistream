//! The fake AniList, driven through the app's real clients: `Meta` for what screens load, and
//! `AniList` directly for the signed-in calls and the failure shapes.

use anistream_meta::{AniListError, BrowseFilter, Season, source::MetaError};
use anistream_testkit::{
    Failure, FakeAniList, MetaKit, Operation,
    fixtures::{
        self, AIRING, AIRING_NO_MAL, FINISHED, FIXTURE_NOW, MISSING, MOVIE, ONA, SEQUEL,
    },
};

use std::time::Duration;

const DAY: i64 = 86_400;

fn ids(page: &[anistream_meta::Media]) -> Vec<u32> {
    page.iter().map(|m| m.id.get()).collect()
}

#[tokio::test]
async fn a_search_is_answered_from_the_fixtures_and_recorded() {
    let kit = MetaKit::start().await;
    let found = kit.meta.search("hoshikuzu", &BrowseFilter::default(), 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [FINISHED.get(), SEQUEL.get()]);
    assert!(!found.has_next);
    // A list answer has no detail fields, as AniList's has none.
    assert!(found.items[0].streaming_episodes.is_empty());

    let asked = kit.server.requests_for(Operation::Search);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].variables["r0_search"], "hoshikuzu");
    assert_eq!(asked[0].authorization, None);
    assert_eq!(kit.server.requests().len(), 1, "one round trip, nothing else");

    // By its English title too, in any case.
    let found =
        kit.meta.search("STEEL GARDENER", &BrowseFilter::default(), 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [ONA.get()]);
    let none = kit.meta.search("no such show", &BrowseFilter::default(), 1, 20).await.unwrap();
    assert!(none.items.is_empty());
}

#[tokio::test]
async fn a_browse_lists_everything_in_pages_and_honours_its_filters() {
    let kit = MetaKit::start().await;
    let first = kit.meta.search("", &BrowseFilter::default(), 1, 4).await.unwrap();
    assert_eq!(
        ids(&first.items),
        fixtures::ALL[..4].iter().map(|id| id.get()).collect::<Vec<_>>()
    );
    assert!(first.has_next);
    let second = kit.meta.search("", &BrowseFilter::default(), 2, 4).await.unwrap();
    assert_eq!(second.items.len(), 2);
    assert!(!second.has_next);

    let movies = BrowseFilter { formats: vec!["MOVIE".into()], ..Default::default() };
    let found = kit.meta.search("", &movies, 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [MOVIE.get()]);
    let sports = BrowseFilter { genres: vec!["Sports".into()], ..Default::default() };
    let found = kit.meta.search("", &sports, 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [AIRING_NO_MAL.get()]);
}

#[tokio::test]
async fn the_catalogue_fills_a_browse_and_the_fantasy_filter_halves_it() {
    let kit = MetaKit::start().await;
    kit.server.add_catalogue(fixtures::catalogue_titles());
    let all = kit.meta.search("", &BrowseFilter::default(), 1, 50).await.unwrap();
    assert_eq!(all.items.len(), 14);
    assert!(!all.has_next);
    // Mixed, not the new ones after the six.
    let first_six = &ids(&all.items)[..6];
    assert!(first_six.iter().any(|id| *id < 900_100), "{first_six:?}");
    assert!(first_six.iter().any(|id| *id > 900_100), "{first_six:?}");

    let fantasy = BrowseFilter { genres: vec!["Fantasy".into()], ..Default::default() };
    let found = kit.meta.search("", &fantasy, 1, 50).await.unwrap();
    assert_eq!(found.items.len(), 7);

    // The new titles are found by name and open by id, and the seasonal browse is as it was.
    let found = kit.meta.search("fox mask", &BrowseFilter::default(), 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [900_105]);
    assert!(kit.meta.media(fixtures::CATALOGUE[0]).await.is_ok());
}

#[tokio::test]
async fn every_filter_is_applied_as_anilist_applies_it() {
    let kit = MetaKit::start().await;
    let find = async |filter: BrowseFilter| {
        ids(&kit.meta.search("", &filter, 1, 20).await.unwrap().items)
    };
    let not_drama =
        BrowseFilter { genres_excluded: vec!["Drama".into()], ..Default::default() };
    assert_eq!(find(not_drama).await, [AIRING.get(), ONA.get(), AIRING_NO_MAL.get()]);
    // The tag at its rank, and not below it.
    let travel = BrowseFilter { tags: vec!["Travel".into()], ..Default::default() };
    assert_eq!(find(travel.clone()).await, [FINISHED.get()]);
    let ranked = BrowseFilter { min_tag_rank: Some(95), ..travel };
    assert!(find(ranked).await.is_empty());
    let short = BrowseFilter { episodes_lesser: Some(13), ..Default::default() };
    assert_eq!(
        find(short).await,
        [FINISHED.get(), SEQUEL.get(), MOVIE.get(), ONA.get(), AIRING_NO_MAL.get()]
    );
    let between = BrowseFilter {
        started_after: Some(2024_9999),
        started_before: Some(2026_0000),
        ..Default::default()
    };
    assert_eq!(find(between).await, [SEQUEL.get(), MOVIE.get()]);
    let manga = BrowseFilter { sources: vec!["MANGA".into()], ..Default::default() };
    assert_eq!(find(manga).await, [FINISHED.get(), SEQUEL.get()]);
    let streaming =
        BrowseFilter { licensed_by: vec!["Lantern Stream".into()], ..Default::default() };
    assert_eq!(find(streaming).await, [FINISHED.get(), AIRING.get(), SEQUEL.get()]);
    let korean = BrowseFilter { countries: vec!["KR".into()], ..Default::default() };
    assert!(find(korean).await.is_empty(), "the fixtures are all made in Japan");

    // Sorted by score, best first, and the title with none last.
    let best = BrowseFilter { sort: vec!["SCORE_DESC".into()], ..Default::default() };
    assert_eq!(
        find(best).await,
        [
            MOVIE.get(),
            SEQUEL.get(),
            FINISHED.get(),
            AIRING.get(),
            ONA.get(),
            AIRING_NO_MAL.get()
        ]
    );
}

#[tokio::test]
async fn the_rails_come_in_one_request_and_the_facets_in_another() {
    let kit = MetaKit::start().await;
    let trending = BrowseFilter { sort: vec!["TRENDING_DESC".into()], ..Default::default() };
    let movies = BrowseFilter { formats: vec!["MOVIE".into()], ..Default::default() };
    let rails = kit.meta.browse_many(&[("", &trending), ("", &movies)], 3).await.unwrap();
    assert_eq!(rails.len(), 2);
    assert_eq!(rails[0].items.len(), 3);
    assert!(rails[0].has_next);
    assert_eq!(ids(&rails[1].items), [MOVIE.get()]);
    assert_eq!(kit.server.requests_for(Operation::Browse).len(), 1);

    let facets = kit.meta.facets().await.unwrap();
    assert!(facets.genres.contains(&"Mecha".to_owned()));
    assert!(facets.tags.iter().any(|t| t.name == "Nudity" && t.adult));
    assert_eq!(
        facets.streaming,
        ["Lantern Stream", "Hikari Stream"],
        "once each, none disabled"
    );
    assert_eq!(kit.server.requests().len(), 2);
}

#[tokio::test]
async fn a_detail_lookup_carries_the_detail_fields() {
    let kit = MetaKit::start().await;
    let media = kit.meta.media(FINISHED).await.unwrap();
    assert_eq!(media.title.display(), "Stardust Wayfarer");
    assert_eq!(media.title.secondary(), Some("Hoshikuzu no Tabibito"));
    assert_eq!(media.streaming_episodes.len(), 12);
    assert_eq!(media.watch_order()[0].id, SEQUEL);
    assert_eq!(media.recommended().len(), 2);
    assert_eq!(media, fixtures::media(FINISHED), "what the server sent is the fixture");
    // The richer page: the broadcasts asked for beside the title came back onto it, newest
    // first, with the tags, the cast and crew, and covers on what it relates to.
    assert_eq!(media.aired.len(), 12);
    assert_eq!(media.aired.first().map(|b| b.episode), Some(12));
    assert_eq!(media.season, Some(Season::Spring));
    assert!(!media.tags.is_empty() && !media.characters.edges.is_empty());
    assert!(!media.staff.edges.is_empty() && !media.rankings.is_empty());
    assert!(media.relations.edges[0].node.cover_image.best().is_some());

    let asked = kit.server.requests_for(Operation::Detail);
    assert_eq!(asked[0].variables["id"], FINISHED.get());
    // The fetch was written through to the cache, as every fresh one is.
    assert_eq!(kit.store.cached_media_count().unwrap(), 1);
}

#[tokio::test]
async fn a_title_asked_for_again_within_minutes_is_not_asked_of_anilist_again() {
    let kit = MetaKit::start().await;
    let first = kit.meta.media(FINISHED).await.unwrap();
    // Its episodes, and then the play, a moment after its page.
    let again = kit.meta.media(FINISHED).await.unwrap();
    assert_eq!(again, first);
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 1);

    // Another title is its own ask.
    kit.meta.media(AIRING).await.unwrap();
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 2);
}

#[tokio::test]
async fn asks_for_one_title_made_together_are_asked_once() {
    let kit = MetaKit::start().await;
    // A title's page and its episode table, opened together.
    let (page, table) = tokio::join!(kit.meta.media(FINISHED), kit.meta.media(FINISHED));
    assert_eq!(page.unwrap(), table.unwrap());
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 1);

    // So is a failure: one walk down the ladder, which both are told of.
    kit.server.fail_always(Failure::Status(503));
    let (page, table) = tokio::join!(kit.meta.media(MOVIE), kit.meta.media(MOVIE));
    assert!(page.is_err() && table.is_err(), "{page:?} {table:?}");
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 2);
}

#[tokio::test]
async fn a_failed_ask_is_asked_again() {
    let kit = MetaKit::start().await;
    kit.server.fail_next(Failure::Status(500));
    assert!(kit.meta.media(FINISHED).await.is_err());
    // Nothing was cached for it either, so the next ask is AniList's again.
    assert_eq!(kit.meta.media(FINISHED).await.unwrap(), fixtures::media(FINISHED));
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 2);
}

#[tokio::test]
async fn a_title_from_a_list_is_still_asked_for_its_page() {
    let kit = MetaKit::start().await;
    // The Continue rail's batch has none of a title page's detail.
    kit.meta.media_many(&[FINISHED, MOVIE]).await.unwrap();
    let page = kit.meta.media(FINISHED).await.unwrap();
    assert_eq!(page.streaming_episodes.len(), 12);
    assert_eq!(page.recommended().len(), 2);
    assert_eq!(kit.server.requests_for(Operation::Detail).len(), 1);
}

#[tokio::test]
async fn an_unknown_title_is_refused_as_anilist_refuses_one() {
    let kit = MetaKit::start().await;
    let error = kit.meta.media(MISSING).await.unwrap_err();
    assert!(error.to_string().contains("Not Found."), "{error}");
}

#[tokio::test]
async fn countdowns_follow_the_fakes_clock() {
    let kit = MetaKit::start().await;
    // Asked of the fake every time, for what it answers.
    let meta = kit.meta.clone().reusing_for(Duration::ZERO);
    let next = meta.media(AIRING).await.unwrap().next_airing_episode.unwrap();
    assert_eq!((next.episode, next.time_until_airing), (6, 2 * DAY + 3 * 3_600 + 15 * 60));
    assert_eq!(next.airing_at, FIXTURE_NOW + next.time_until_airing);

    // Against another clock the schedule moves with it, the same distance away.
    let before = meta.media(AIRING).await.unwrap().air_times();
    let now = FIXTURE_NOW + 40 * DAY + 17;
    kit.server.set_now(now);
    let moved_media = meta.media(AIRING).await.unwrap();
    let moved = moved_media.next_airing_episode.unwrap();
    assert_eq!(moved.airing_at, now + next.time_until_airing);
    assert_eq!(moved.time_until_airing, next.time_until_airing);
    // Every broadcast with it, those out and those to come.
    let after = moved_media.air_times();
    assert_eq!(after.len(), before.len());
    assert!(before.iter().all(|(episode, at)| after[episode] == at + (now - FIXTURE_NOW)));
}

#[tokio::test]
async fn the_calendar_window_is_honoured_in_both_directions() {
    let kit = MetaKit::start().await;
    let week = 7 * DAY;
    let past = kit.meta.airing_between_sorted(FIXTURE_NOW - week, FIXTURE_NOW, 1, 50, true);
    let past = past.await.unwrap();
    let rows: Vec<_> = past.items.iter().map(|a| (a.media.id, a.episode)).collect();
    assert_eq!(rows, [(AIRING_NO_MAL, 8), (AIRING, 5)], "newest first");

    let ahead = kit.meta.airing_between(FIXTURE_NOW, FIXTURE_NOW + week, 1, 50).await.unwrap();
    let rows: Vec<_> = ahead.items.iter().map(|a| (a.media.id, a.episode)).collect();
    assert_eq!(rows, [(AIRING, 6), (AIRING_NO_MAL, 9)], "oldest first");
    assert!(ahead.items.iter().all(|a| a.airing_at > FIXTURE_NOW));

    let paged = kit.meta.airing_between(0, i64::from(u32::MAX), 1, 3).await.unwrap();
    assert_eq!(paged.items.len(), 3);
    assert!(paged.has_next);
}

#[tokio::test]
async fn the_rails_queries_answer_too() {
    let kit = MetaKit::start().await;
    let fall = BrowseFilter {
        season: Some(Season::Fall.as_str().into()),
        year: Some(2026),
        ..Default::default()
    };
    let season = kit.meta.search("", &fall, 1, 20).await;
    assert_eq!(ids(&season.unwrap().items), [AIRING.get(), AIRING_NO_MAL.get()]);

    // In the order asked for, though the fake answers in its own.
    let rail = kit.meta.media_many(&[MOVIE, FINISHED, MISSING]).await.unwrap();
    assert_eq!(ids(&rail), [MOVIE.get(), FINISHED.get()]);

    let aired = kit.meta.last_aired(&[AIRING, AIRING_NO_MAL, FINISHED]).await.unwrap();
    let rows: Vec<_> = aired.iter().map(|r| (r.media_id, r.episode)).collect();
    assert_eq!(rows, [(AIRING, 5), (AIRING_NO_MAL, 8)]);

    let operations: Vec<Operation> =
        kit.server.requests().into_iter().map(|r| r.operation).collect();
    assert_eq!(operations, [Operation::Seasonal, Operation::Many, Operation::LastAired]);
}

#[tokio::test]
async fn signed_in_calls_send_the_token_and_read_the_library() {
    let server = FakeAniList::start().await;
    let anilist = server.anilist(&anistream_testkit::http()).with_token(Some("tok".into()));
    assert_eq!(anilist.viewer_id().await.unwrap(), fixtures::VIEWER_ID);
    let viewer = anilist.viewer().await.unwrap();
    // AniList's stylised English, read as English.
    assert_eq!(viewer.title_language, Some(anistream_meta::anilist::TitleLanguage::English));
    server.set_viewer_title_language("NATIVE");
    let reading = anilist.viewer().await.unwrap().title_language;
    assert_eq!(
        reading,
        Some(anistream_meta::anilist::TitleLanguage::Native),
        "as the test sets it"
    );

    let library = anilist.library(fixtures::VIEWER_ID).await.unwrap();
    let rows: Vec<_> =
        library.iter().map(|e| (e.media.id, e.status.as_str(), e.progress, e.score)).collect();
    assert_eq!(
        rows,
        [
            (FINISHED, "COMPLETED", 12, Some(8.5)),
            (AIRING, "CURRENT", 3, None),
            (MOVIE, "PLANNING", 0, None)
        ]
    );

    let entry = anilist.save_entry(AIRING, Some(4), Some("CURRENT"), None).await.unwrap();
    assert_eq!(entry, Some(70_002));
    assert!(anilist.delete_entry(70_003).await.unwrap());

    let saves = server.requests_for(Operation::SaveEntry);
    assert_eq!(saves[0].variables["progress"], 4);
    assert!(saves[0].variables.get("scoreRaw").is_none(), "an absent score is not sent");
    let tokens: Vec<_> = server.requests().into_iter().map(|r| r.authorization).collect();
    assert!(tokens.iter().all(|t| t.as_deref() == Some("Bearer tok")), "{tokens:?}");
}

#[tokio::test]
async fn an_error_answers_one_request_and_then_the_fixtures_return() {
    let server = FakeAniList::start().await;
    let anilist = server.anilist(&anistream_testkit::http());
    server.fail_next(Failure::GraphQl("boom".into()));
    match anilist.search("hoshikuzu", 1, 10).await {
        Err(AniListError::Api(message)) => assert_eq!(message, "boom"),
        other => panic!("expected the API error, got {other:?}"),
    }
    assert_eq!(anilist.search("hoshikuzu", 1, 10).await.unwrap().items.len(), 2);
    assert_eq!(server.requests_for(Operation::Search).len(), 2, "the failure was recorded too");
}

#[tokio::test]
async fn a_rate_limit_is_reported_and_held_to() {
    let server = FakeAniList::start().await;
    let anilist = server.anilist(&anistream_testkit::http());
    server.fail_next(Failure::RateLimited { retry_after_secs: 30 });
    match anilist.media(FINISHED).await {
        Err(AniListError::Api(message)) => {
            assert!(message.contains("rate limited"), "{message}")
        }
        other => panic!("expected a rate-limit error, got {other:?}"),
    }
    let wait = anilist.rate_limit_wait().await.expect("the client holds off after a 429");
    assert!(wait.as_secs() > 20, "{wait:?}");
}

#[tokio::test]
async fn a_status_and_a_hang_up_fail_as_their_own_kinds() {
    let server = FakeAniList::start().await;
    let anilist = server.anilist(&anistream_testkit::http());
    server.fail_next(Failure::Status(500));
    server.fail_next(Failure::Disconnect);
    let status = anilist.media(FINISHED).await;
    assert!(matches!(status, Err(AniListError::Api(ref m)) if m.contains("500")), "{status:?}");
    let hang_up = anilist.media(FINISHED).await;
    assert!(matches!(hang_up, Err(AniListError::Network(_))), "{hang_up:?}");
    assert!(anilist.media(FINISHED).await.is_ok(), "both were one request each");
}

#[tokio::test]
async fn an_outage_walks_the_ladder_down_to_the_cache() {
    let kit = MetaKit::start_with_tenrai().await;
    anistream_testkit::seed::mappings(&kit.store);
    kit.meta.media(FINISHED).await.unwrap();

    kit.server.fail_always(Failure::Status(503));
    // Tenrai is empty: it finds no such title, so the cached copy answers. Asked for again at
    // once, AniList's answer of a moment ago would, so this asks as if minutes had gone by.
    let later = kit.meta.clone().reusing_for(Duration::ZERO);
    let cached = later.media(FINISHED).await.unwrap();
    assert_eq!(cached.title.display(), "Stardust Wayfarer");
    let notice = kit.meta.take_notice().unwrap_or_default();
    assert!(notice.contains("cached"), "{notice}");
    let tenrai = kit.server.requests_for(Operation::Tenrai);
    assert_eq!(tenrai.len(), 1);
    assert_eq!(tenrai[0].path, "/tenrai/v1/anime/800001/full");

    // A search has no cache rung, and the empty Tenrai answers it with nothing.
    let found = kit.meta.search("hoshikuzu", &BrowseFilter::default(), 1, 20).await.unwrap();
    assert!(found.items.is_empty());

    kit.server.heal();
    kit.meta.media(FINISHED).await.unwrap();
    assert_eq!(kit.meta.take_notice().as_deref(), Some("anilist is back"));
}

#[tokio::test]
async fn with_the_fallback_off_an_outage_names_every_rung() {
    let kit = MetaKit::start().await;
    kit.server.fail_always(Failure::Status(503));
    let error =
        kit.meta.search("hoshikuzu", &BrowseFilter::default(), 1, 20).await.unwrap_err();
    assert!(matches!(error, MetaError::Exhausted { .. }), "{error}");
    assert!(error.to_string().contains("tenrai is off"), "{error}");
    assert_eq!(kit.meta.health()[0].state, "unreachable");
    assert!(kit.server.requests_for(Operation::Tenrai).is_empty());
}

/// The bytes at `url`, when it answers with a success.
async fn fetch(url: &str) -> Option<Vec<u8>> {
    let response = anistream_testkit::http().plain().get(url).send().await.ok()?;
    response.status().is_success().then_some(())?;
    Some(response.bytes().await.ok()?.to_vec())
}

#[tokio::test]
async fn art_is_served_only_when_asked_and_then_every_image_points_at_it() {
    let plain = MetaKit::start().await;
    let media = plain.meta.media(FINISHED).await.unwrap();
    assert_eq!(
        media.cover_image.extra_large.as_deref(),
        Some("https://img.example/cover/900001-xl.jpg"),
        "the fixtures' own URLs, until the fake is asked to serve art"
    );
    let art = format!("{}cover/900001-xl.jpg", plain.server.art_base());
    assert!(fetch(&art).await.is_none(), "no art until asked");

    let kit = MetaKit::start().await;
    kit.server.serve_art();
    let media = kit.meta.media(FINISHED).await.unwrap();
    let base = kit.server.art_base();
    let cover = media.cover_image.extra_large.clone().unwrap();
    let banner = media.banner_image.clone().unwrap();
    let still = media.streaming_episodes[0].thumbnail.clone().unwrap();
    for url in [&cover, &banner, &still] {
        assert!(url.starts_with(&base), "{url} is under {base}");
        let bytes = fetch(url).await.unwrap_or_else(|| panic!("{url} answers"));
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "{url} is a PNG");
    }
    assert_ne!(fetch(&cover).await, fetch(&banner).await, "a poster and a banner differ");
    assert!(fetch(&format!("{base}cover/900999.jpg")).await.is_none());
    // Art is not AniList's: nothing about it is recorded as a request.
    assert!(kit.server.requests().iter().all(|r| !r.path.starts_with("/art/")));
}

#[tokio::test]
async fn art_served_badly_fails_every_second_image_a_few_times_first() {
    let kit = MetaKit::start().await;
    kit.server.serve_art();
    kit.server.trouble_art(2, std::time::Duration::ZERO);
    let base = kit.server.art_base();
    let (first, second) =
        (format!("{base}cover/900001-xl.jpg"), format!("{base}banner/900001.jpg"));
    assert!(fetch(&first).await.is_none(), "the first image fails");
    assert!(fetch(&second).await.is_some(), "the second does not");
    assert!(fetch(&first).await.is_none(), "twice");
    assert!(fetch(&first).await.is_some(), "and then it is served");
}
