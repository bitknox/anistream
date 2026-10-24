//! The lists: Continue, Seasonal, the Calendar, Search, Library, and the Providers check.
//!
//! A list arrives as one `Content`, tagged with the view that asked for it. What rides with it
//! is the contract: a caption sent before it, the calendar's window before it, the broadcast
//! dates after it, and a metadata notice just ahead of whichever update it arrived with.

use anistream_model::{
    catalog::{ContentFor, ContentList, LibrarySegment},
    command::Command,
    event::Event,
    search::SearchQuery,
};
use anistream_testkit::{
    Failure, Operation,
    fixtures::{AIRING, AIRING_NO_MAL, FINISHED, FIXTURE_NOW, MOVIE, SEASONAL, SEQUEL},
    seed,
};

use super::{NO_FALLBACK, WITH_ANILIST, content, ends, failure, ids, toasts};
use crate::harness::{Harness, Options, names};

fn no_fallback() -> Options {
    Options { config: Some(NO_FALLBACK.into()), ..Options::default() }
}

fn search(query: &str) -> Command {
    Command::Search { query: SearchQuery::text(query) }
}

#[tokio::test]
async fn continue_with_no_history_is_empty_rather_than_an_empty_list() {
    let mut h = Harness::new().await;
    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert!(matches!(
        &updates[0],
        Event::Content { view: ContentFor::Continue, content: ContentList::Empty }
    ));
    ends(&updates, job);
    assert!(h.server().requests().is_empty(), "nothing to look up");
}

#[tokio::test]
async fn continue_with_everything_hidden_is_an_empty_list() {
    // History, all of it dismissed with `x`: not "nothing watched yet", which `Empty` says.
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 3_600);
    h.store().hide_from_continue(FINISHED).unwrap();
    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert_eq!(content(&updates), &ContentList::Entries(Vec::new()));
    ends(&updates, job);
    assert!(h.server().requests().is_empty(), "nothing to look up");
}

#[tokio::test]
async fn continue_lists_the_latest_first_then_dates_what_is_airing() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 3, FIXTURE_NOW - 7_200);
    seed::watched(h.store(), AIRING, 2, FIXTURE_NOW - 600);
    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "LastAired", "JobDone"]);
    assert!(matches!(&updates[0], Event::Content { view: ContentFor::Continue, .. }));
    assert_eq!(ids(content(&updates)), [AIRING, FINISHED], "most recent first");
    // Asked about the airing title only; the age is the real clock's, so only its sign is read.
    let Event::LastAired(rows) = &updates[1] else { unreachable!() };
    let aired: Vec<_> = rows.iter().map(|row| (row.id, row.aired.episode)).collect();
    assert_eq!(aired, [(AIRING, 5)]);
    assert!(rows.iter().all(|row| row.aired.ago >= 0));
    ends(&updates, job);
    assert_eq!(h.server().requests_for(Operation::Many).len(), 1);
    assert_eq!(h.server().requests_for(Operation::LastAired).len(), 1);
}

#[tokio::test]
async fn continue_that_cannot_be_fetched_fails_in_place() {
    let mut h = Harness::with(no_fallback()).await;
    seed::watched(h.store(), FINISHED, 1, FIXTURE_NOW - 60);
    h.server().fail_always(Failure::Status(500));
    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert!(failure(content(&updates)).contains("answered 500"), "{updates:?}");
    ends(&updates, job);
}

#[tokio::test]
async fn seasonal_names_its_season_before_the_list_and_dates_it_after() {
    let mut h = Harness::new().await;
    let job = h.send(Command::LoadSeasonal);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Caption", "Content", "LastAired", "JobDone"]);
    // The season is the real clock's, as the app asks for the one it is in.
    let (season, year) = anistream_engine::catalog::lists::current_season();
    let Event::Caption { view: ContentFor::Seasonal, text } = &updates[0] else {
        panic!("{:?}", updates[0])
    };
    assert_eq!(
        text,
        &format!("{} {year}", season.as_str().to_lowercase()),
        "one page held all"
    );
    assert_eq!(ids(content(&updates)), SEASONAL);
    let Event::LastAired(rows) = &updates[2] else { unreachable!() };
    let aired: Vec<_> = rows.iter().map(|row| (row.id, row.aired.episode)).collect();
    assert_eq!(aired, [(AIRING, 5), (AIRING_NO_MAL, 8)]);
    ends(&updates, job);
}

#[tokio::test]
async fn seasonal_that_cannot_be_fetched_fails_without_a_caption() {
    let mut h = Harness::with(no_fallback()).await;
    h.server().fail_always(Failure::Status(500));
    let job = h.send(Command::LoadSeasonal);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert!(matches!(&updates[0], Event::Content { view: ContentFor::Seasonal, .. }));
    assert!(failure(content(&updates)).contains("answered 500"));
    ends(&updates, job);
}

#[tokio::test]
async fn a_metadata_notice_comes_just_before_the_update_it_arrived_with() {
    // The loop says the ladder moved when the next update arrives: the status rows, then the
    // toast, then that update. Once on the way down to Tenrai, and once on the way back.
    let mut h = Harness::new().await;
    h.server().fail_next(Failure::Status(500));
    let job = h.send(Command::LoadSeasonal);
    let down = h.settle().await;

    assert_eq!(names(&down), ["MetaStatus", "Toast", "Caption", "Content", "JobDone"]);
    assert_eq!(toasts(&down), ["info: anilist is unreachable — fresh data via tenrai for now"]);
    let Event::MetaStatus(rows) = &down[0] else { unreachable!() };
    let serving: Vec<_> = rows.iter().filter(|r| r.active).map(|r| r.source.as_str()).collect();
    assert_eq!(serving, ["tenrai"]);
    assert_eq!(
        content(&down),
        &ContentList::Entries(Vec::new()),
        "the empty fallback answered"
    );
    ends(&down, job);

    let job = h.send(Command::LoadSeasonal);
    let back = h.settle().await;
    assert_eq!(
        names(&back),
        ["MetaStatus", "Toast", "Caption", "Content", "LastAired", "JobDone"]
    );
    assert_eq!(toasts(&back), ["info: anilist is back"]);
    assert_eq!(ids(content(&back)), SEASONAL);
    ends(&back, job);
}

#[tokio::test]
async fn the_calendar_sends_its_window_before_its_entries() {
    let mut h = Harness::new().await;
    // The window is taken from the real clock, so the fake's broadcasts are moved to it.
    let before = anistream_store::now();
    h.server().set_now(before);
    let job = h.send(Command::LoadCalendar);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["CalendarWindow", "Content", "JobDone"]);
    let Event::CalendarWindow { anchor, pages, more_past } = &updates[0] else {
        unreachable!()
    };
    assert!((before..=anistream_store::now()).contains(anchor));
    assert_eq!((*pages, *more_past), (1, false), "one page held the past");
    let ContentList::Entries(entries) = content(&updates) else { panic!("{updates:?}") };
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| e.airing_episode.is_some()), "every row is a broadcast");
    let times: Vec<i64> = entries.iter().filter_map(|e| e.airing_in).collect();
    assert!(times.is_sorted(), "one timeline, oldest first: {times:?}");
    assert!(times.first() < Some(&0) && times.last() > Some(&0), "aired and to come");
    ends(&updates, job);
}

#[tokio::test]
async fn a_calendar_that_cannot_be_fetched_fails_in_place() {
    let mut h = Harness::with(no_fallback()).await;
    h.server().fail_always(Failure::Status(500));
    let job = h.send(Command::LoadCalendar);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert!(matches!(&updates[0], Event::Content { view: ContentFor::Calendar, .. }));
    assert!(failure(content(&updates)).contains("answered 500"));
    ends(&updates, job);
}

#[tokio::test]
async fn earlier_broadcasts_come_as_a_backfill_and_a_failed_one_says_nothing() {
    let mut h = Harness::with(no_fallback()).await;
    let anchor = anistream_store::now();
    h.server().set_now(anchor);
    let job = h.send(Command::LoadCalendarPast { anchor, page: 2 });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["CalendarBackfill", "JobDone"]);
    let Event::CalendarBackfill { page, entries, more_past } = &updates[0] else {
        unreachable!()
    };
    assert_eq!((*page, entries.len(), *more_past), (Some(2), 0, false), "one page held it all");
    ends(&updates, job);

    // Not a toast: nobody asked for the page by name, and the next scroll asks again.
    h.server().fail_always(Failure::Status(500));
    let job = h.send(Command::LoadCalendarPast { anchor, page: 2 });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["CalendarBackfill", "JobDone"]);
    let Event::CalendarBackfill { page, entries, more_past } = &updates[0] else {
        unreachable!()
    };
    assert_eq!((*page, entries.len(), *more_past), (None, 0, true), "tried again next time");
    ends(&updates, job);
}

#[tokio::test]
async fn a_search_is_captioned_every_time_then_listed() {
    let mut h = Harness::new().await;
    let job = h.send(search("hoshikuzu"));
    let updates = h.settle().await;

    // Kept among the recent searches, then whether there is more, the caption and the list.
    // Finished titles only, so there is nothing to date.
    assert_eq!(
        names(&updates),
        ["RecentSearches", "SearchPaging", "Caption", "Content", "JobDone"]
    );
    let view = ContentFor::Search { query: SearchQuery::text("hoshikuzu") };
    let Event::SearchPaging { has_next, .. } = &updates[1] else { unreachable!() };
    assert!(!has_next);
    let Event::Caption { view: captioned, text } = &updates[2] else { unreachable!() };
    assert_eq!((captioned, text.as_str()), (&view, ""), "said even when it says nothing");
    assert!(matches!(&updates[3], Event::Content { view: v, .. } if *v == view));
    assert_eq!(ids(content(&updates)), [FINISHED, SEQUEL]);
    let asked = h.server().requests_for(Operation::Search);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].variables["r0_search"], "hoshikuzu");
    ends(&updates, job);
}

#[tokio::test]
async fn browsing_with_no_words_lists_everything_and_dates_what_airs() {
    let mut h = Harness::new().await;
    let job = h.send(search(""));
    let updates = h.settle().await;

    // Nothing to keep among the recent searches.
    assert_eq!(names(&updates), ["SearchPaging", "Caption", "Content", "LastAired", "JobDone"]);
    assert_eq!(ids(content(&updates)).len(), 6);
    ends(&updates, job);
}

#[tokio::test]
async fn a_browse_with_filters_and_no_words_is_most_popular_first_and_hides_adult_titles() {
    let mut h = Harness::new().await;
    let query = SearchQuery { genres: vec!["Drama".into()], ..SearchQuery::default() };
    h.send(Command::Search { query });
    let updates = h.settle().await;
    assert_eq!(ids(content(&updates)), [FINISHED, SEQUEL, MOVIE]);
    let asked = h.server().requests_for(Operation::Search);
    assert_eq!(asked[0].variables["r0_sort"], serde_json::json!(["POPULARITY_DESC", "ID"]));
    assert_eq!(asked[0].variables["r0_genre_in"], serde_json::json!(["Drama"]));
    assert_eq!(asked[0].variables["r0_isAdult"], false);
}

#[tokio::test]
async fn live_search_waits_for_the_typing_to_stop_and_asks_once() {
    let mut h = Harness::new().await;
    for text in ["h", "ho", "hos", "hoshi", "hoshikuzu"] {
        h.send(Command::SearchLive { query: SearchQuery::text(text) });
    }
    let updates = h.settle().await;
    let asked = h.server().requests_for(Operation::Search);
    assert_eq!(asked.len(), 1, "one search for the text as it ended up");
    assert_eq!(asked[0].variables["r0_search"], "hoshikuzu");
    assert_eq!(names(&updates), ["SearchPaging", "Caption", "Content"]);
    assert_eq!(ids(content(&updates)), [FINISHED, SEQUEL]);
    // Live results are not kept among the recent searches until one is opened or submitted.
    assert!(!names(&updates).contains(&"RecentSearches"));

    // The same search again is answered from what was kept, at once.
    h.send(Command::Search { query: SearchQuery::text("hoshikuzu ") });
    let again = h.settle().await;
    assert_eq!(ids(content(&again)), [FINISHED, SEQUEL]);
    assert_eq!(h.server().requests_for(Operation::Search).len(), 1, "nothing asked again");
}

#[tokio::test]
async fn one_letter_is_not_searched_yet_but_one_kanji_is() {
    let mut h = Harness::new().await;
    h.send(Command::SearchLive { query: SearchQuery::text("h") });
    assert!(h.settle().await.is_empty());
    h.send(Command::SearchLive { query: SearchQuery::text("星") });
    let updates = h.settle().await;
    assert_eq!(ids(content(&updates)), [FINISHED, SEQUEL]);
    assert_eq!(h.server().requests_for(Operation::Search).len(), 1);
}

#[tokio::test]
async fn a_further_page_follows_the_first() {
    let mut h = Harness::new().await;
    let query = SearchQuery::default();
    h.send(Command::SearchMore { query: query.clone(), page: 2 });
    let updates = h.settle().await;
    let Some(Event::SearchMore { query: answered, page, entries, has_next }) =
        updates.iter().find(|u| matches!(u, Event::SearchMore { .. }))
    else {
        panic!("no further page in {:?}", names(&updates))
    };
    assert_eq!((answered, *page, entries.len(), *has_next), (&query, 2, 0, false));
    assert_eq!(h.server().requests_for(Operation::Search)[0].variables["r0_page"], 2);
}

#[tokio::test]
async fn recent_searches_keep_their_filters_newest_first_and_once_each() {
    let mut h = Harness::new().await;
    let mecha = SearchQuery { genres: vec!["Mecha".into()], ..SearchQuery::text("kaze") };
    for query in [mecha.clone(), SearchQuery::text("tsuki"), mecha.clone()] {
        h.send(Command::Search { query });
        h.settle().await;
    }
    h.send(Command::LoadRecentSearches);
    let updates = h.settle().await;
    let Some(Event::RecentSearches(recent)) = updates.first() else { unreachable!() };
    assert_eq!(recent, &[mecha.clone(), SearchQuery::text("tsuki")]);

    h.send(Command::ForgetRecentSearch(mecha));
    let updates = h.settle().await;
    let Some(Event::RecentSearches(recent)) = updates.first() else { unreachable!() };
    assert_eq!(recent, &[SearchQuery::text("tsuki")]);

    h.send(Command::RememberSearch(SearchQuery::text("hagane")));
    h.settle().await;
    h.send(Command::ClearRecentSearches);
    let updates = h.settle().await;
    assert!(matches!(updates.first(), Some(Event::RecentSearches(r)) if r.is_empty()));
}

#[tokio::test]
async fn searchs_rails_come_in_one_request_and_are_kept() {
    let mut h = Harness::new().await;
    h.send(Command::LoadBrowse);
    let updates = h.settle().await;
    let Some(Event::Browse(rails)) = updates.iter().find(|u| matches!(u, Event::Browse(_)))
    else {
        panic!("no rails in {:?}", names(&updates))
    };
    let titles: Vec<_> = rails.iter().map(|r| r.title.as_str()).collect();
    // The next season has nothing in the fixtures, so its rail is left out.
    assert_eq!(
        titles,
        ["Trending now", "Popular this season", "All-time popular", "Top rated"]
    );
    let ids_of = |rail: &anistream_model::search::BrowseRail| {
        rail.entries.iter().map(|e| e.id).collect::<Vec<_>>()
    };
    assert_eq!(ids_of(&rails[1]), SEASONAL);
    assert_eq!(ids_of(&rails[3])[0], MOVIE, "the best rated first");
    assert_eq!(h.server().requests_for(Operation::Browse).len(), 1);

    // Asked again, from what was kept: no request.
    h.send(Command::LoadBrowse);
    let again = h.settle().await;
    assert!(again.iter().any(|u| matches!(u, Event::Browse(r) if r.len() == 4)));
    assert_eq!(h.server().requests().len(), 1);
}

#[tokio::test]
async fn the_facets_leave_adult_tags_out_and_are_asked_once() {
    let mut h = Harness::new().await;
    h.send(Command::LoadSearchFacets);
    let updates = h.settle().await;
    let Some(Event::Facets(facets)) = updates.first() else { unreachable!() };
    assert!(!facets.genres.contains(&"Hentai".to_owned()));
    let categories: Vec<_> = facets.tag_groups.iter().map(|g| g.category.as_str()).collect();
    assert_eq!(categories, ["Theme", "Setting"]);
    assert_eq!(facets.streaming_sites, ["Lantern Stream", "Hikari Stream"]);
    assert_eq!(facets.tags_matching("gar", 5), ["Gardening"]);

    h.send(Command::LoadSearchFacets);
    h.settle().await;
    assert_eq!(h.server().requests_for(Operation::Facets).len(), 1);
}

#[tokio::test]
async fn a_search_that_cannot_be_answered_fails_without_a_caption() {
    let mut h = Harness::with(no_fallback()).await;
    h.server().fail_always(Failure::Status(500));
    let job = h.send(search("hoshikuzu"));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["RecentSearches", "Content", "JobDone"]);
    assert!(failure(content(&updates)).contains("answered 500"));
    ends(&updates, job);
}

#[tokio::test]
async fn the_library_signed_out_is_an_empty_list_for_every_segment() {
    let mut h = Harness::new().await;
    for segment in LibrarySegment::ALL {
        let job = h.send(Command::LoadLibrary(segment));
        let updates = h.settle().await;
        assert_eq!(names(&updates), ["Content", "JobDone"], "{segment:?}");
        assert!(matches!(
            &updates[0],
            Event::Content { view: ContentFor::Library(s), content: ContentList::Entries(rows) }
                if *s == segment && rows.is_empty()
        ));
        ends(&updates, job);
    }
    assert!(h.server().requests().is_empty(), "no account, nothing to ask");
}

#[tokio::test]
async fn the_library_signed_in_lists_each_segment_from_the_tracker() {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    for (segment, expected) in [
        (LibrarySegment::Watching, vec![AIRING]),
        (LibrarySegment::Planning, vec![MOVIE]),
        (LibrarySegment::Completed, vec![FINISHED]),
        (LibrarySegment::Paused, vec![]),
        (LibrarySegment::Dropped, vec![]),
    ] {
        let job = h.send(Command::LoadLibrary(segment));
        let updates = h.settle().await;
        assert_eq!(names(&updates), ["Content", "JobDone"], "{segment:?}");
        let Event::Content { view, .. } = &updates[0] else { unreachable!() };
        assert_eq!(view, &ContentFor::Library(segment));
        assert_eq!(ids(content(&updates)), expected, "{segment:?}");
        let ContentList::Entries(rows) = content(&updates) else { unreachable!() };
        assert!(rows.iter().all(|row| row.list_status == Some(segment)));
        ends(&updates, job);
    }
    let token = h.server().requests_for(Operation::Library)[0].authorization.clone();
    assert_eq!(token.as_deref(), Some("Bearer harness-anilist-token"));
}

#[tokio::test]
async fn the_library_opened_asks_for_the_list_once_for_all_its_segments() {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    // As the app opens the screen: every segment at once.
    for segment in LibrarySegment::ALL {
        h.send(Command::LoadLibrary(segment));
    }
    let updates = h.settle().await;
    let mut shown: Vec<_> = updates
        .iter()
        .filter_map(|update| match update {
            Event::Content { view: ContentFor::Library(segment), .. } => Some(*segment),
            _ => None,
        })
        .collect();
    shown.sort_by_key(|segment| LibrarySegment::ALL.iter().position(|s| s == segment));
    assert_eq!(shown, LibrarySegment::ALL);
    assert_eq!(h.server().requests_for(Operation::Library).len(), 1);

    // And again a moment later, for a tab looked at twice: the same answer.
    let job = h.send(Command::LoadLibrary(LibrarySegment::Planning));
    let updates = h.settle().await;
    assert_eq!(ids(content(&updates)), vec![MOVIE]);
    ends(&updates, job);
    assert_eq!(h.server().requests_for(Operation::Library).len(), 1);
}

#[tokio::test]
async fn a_change_written_to_the_list_has_the_library_asked_for_again() {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    h.send(Command::LoadLibrary(LibrarySegment::Watching));
    h.settle().await;
    let library = |h: &Harness| h.server().requests_for(Operation::Library).len();
    assert_eq!(library(&h), 1);

    // A status set and sent: the list as fetched a moment ago is out of date, so the pull
    // after the send asks for it again.
    h.send(Command::SetStatus { id: MOVIE, status: LibrarySegment::Completed });
    h.settle().await;
    h.send(Command::SyncNow);
    h.settle().await;
    assert_eq!(h.server().requests_for(Operation::SaveEntry).len(), 1, "the change was sent");
    assert_eq!(library(&h), 2);
    // Which answers the screen after it.
    h.send(Command::LoadLibrary(LibrarySegment::Completed));
    h.settle().await;
    assert_eq!(library(&h), 2);
}

#[tokio::test]
async fn a_library_that_cannot_be_fetched_fails_in_place() {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    h.server().fail_always(Failure::Status(500));
    let job = h.send(Command::LoadLibrary(LibrarySegment::Watching));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Content", "JobDone"]);
    assert!(failure(content(&updates)).contains("500"), "{updates:?}");
    ends(&updates, job);
}

#[tokio::test]
async fn checking_providers_reports_the_ladder_then_the_sources() {
    let mut h = Harness::new().await;
    let job = h.send(Command::CheckProviders);
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["MetaStatus", "Providers", "JobDone"]);
    let Event::MetaStatus(ladder) = &updates[0] else { unreachable!() };
    let rungs: Vec<_> = ladder.iter().map(|row| row.source.as_str()).collect();
    assert_eq!(rungs, ["anilist", "tenrai", "cache"]);
    let Event::Providers(rows) = &updates[1] else { unreachable!() };
    let checked: Vec<_> =
        rows.iter().map(|row| (row.id.as_str(), row.state.as_str())).collect();
    assert_eq!(checked, [("fixture", "ready")]);
    ends(&updates, job);
}
