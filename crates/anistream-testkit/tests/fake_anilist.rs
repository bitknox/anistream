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
    assert_eq!(asked[0].variables["search"], "hoshikuzu");
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

    let movies = BrowseFilter { format: Some("MOVIE".into()), ..Default::default() };
    let found = kit.meta.search("", &movies, 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [MOVIE.get()]);
    let sports = BrowseFilter { genres: vec!["Sports".into()], ..Default::default() };
    let found = kit.meta.search("", &sports, 1, 20).await.unwrap();
    assert_eq!(ids(&found.items), [AIRING_NO_MAL.get()]);
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
    let now = FIXTURE_NOW + 40 * DAY + 17;
    kit.server.set_now(now);
    let moved = meta.media(AIRING).await.unwrap().next_airing_episode.unwrap();
    assert_eq!(moved.airing_at, now + next.time_until_airing);
    assert_eq!(moved.time_until_airing, next.time_until_airing);
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
    let season = kit.meta.seasonal(Season::Fall, 2026, &BrowseFilter::default(), 1, 20).await;
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
