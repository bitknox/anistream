//! A title's page, its episode table, and the questions of which source and which match.
//!
//! The title's pinned source is said before the title itself, so the picker is right from the
//! first frame. An episode table that cannot be filled still arrives, empty, before the reason
//! does, so the screen leaves its loading state whatever happened.

use anistream_core::ids::AnilistId;
use anistream_core::{media::SearchHit, traits::SourceCandidate};
use anistream_model::{
    catalog::{
        CastRole, FuzzyDate, RankingKind, RelationKind, SearchFilters, Season, SourceMaterial,
        TitleLanguage,
    },
    command::Command,
    event::Event,
    settings::SettingKey,
};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_store::ResolutionRung;
use anistream_testkit::{
    Operation,
    fixtures::{
        self, AIRING, FINISHED, FIXTURE_NOW, FRANCHISE_FILM, FRANCHISE_OVA, FRANCHISE_SEASONS,
        FRANCHISE_SIDE_STORY, MISSING, SEQUEL,
    },
    seed, sources,
};

use super::{WITH_ANILIST, ends, toasts};
use crate::harness::{Harness, Options, names};

/// The seasons each page said, in the order the pages came: `None` for one sent while the walk
/// along its franchise was still under way.
fn seasons_said(updates: &[Event]) -> Vec<Option<Vec<AnilistId>>> {
    let said = |update: &Event| match update {
        Event::Detail(page) => {
            let seasons = page.detail.as_ref().and_then(|detail| detail.seasons.as_ref());
            Some(seasons.map(|seasons| seasons.iter().map(|season| season.id).collect()))
        }
        _ => None,
    };
    updates.iter().filter_map(said).collect()
}

/// The app on the default config, with the made-up franchise beside the fixtures.
async fn with_franchise(options: Options) -> Harness {
    let h = Harness::with(options).await;
    h.server().add_titles(fixtures::franchise());
    h
}

/// The fixture provider beside a second one, `alt`, that has only three of
/// [`FINISHED`]'s episodes up.
fn with_alt() -> ProviderRegistry {
    let three = sources::episodes(FINISHED).into_iter().take(3).collect();
    let alt = MockProvider::new("alt")
        .with_hits(sources::hits())
        .with_episodes_for(sources::key(FINISHED), three);
    ProviderRegistry::new(vec![sources::provider().arc(), alt.arc()])
}

/// A source whose only hit for [`FINISHED`] is too vague to choose on its own.
fn unsure() -> ProviderRegistry {
    let vague = SearchHit::new(sources::key(FINISHED), "Tabibito");
    let provider = MockProvider::new(sources::PROVIDER_ID)
        .with_hits(vec![vague])
        .with_episodes_for(sources::key(FINISHED), sources::episodes(FINISHED));
    ProviderRegistry::new(vec![provider.arc()])
}

async fn harness(registry: ProviderRegistry) -> Harness {
    Harness::with(Options { registry: Some(registry), ..Options::default() }).await
}

/// How many shows keep a source of their own, as settings say it under the source order.
fn kept_sources(update: &Event) -> u32 {
    let Event::Settings(view) = update else { panic!("not the settings: {update:?}") };
    let order = view.overridden.iter().find(|o| o.key == SettingKey::SourceOrder);
    order.map_or(0, |o| o.shows)
}

/// The episode numbers a table holds.
fn numbers(update: &Event) -> Vec<String> {
    match update {
        Event::Episodes { rows, .. } => rows.iter().map(|row| row.number.clone()).collect(),
        other => panic!("not an episode table: {other:?}"),
    }
}

#[tokio::test]
async fn a_title_says_its_pinned_source_before_it_arrives() {
    let mut h = Harness::new().await;
    let job = h.send(Command::LoadDetail(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["ProviderPreference", "Detail", "JobDone"]);
    assert!(matches!(&updates[0], Event::ProviderPreference(None)));
    let Event::Detail(entry) = &updates[1] else { unreachable!() };
    assert_eq!((entry.id, entry.title.as_str()), (FINISHED, "Stardust Wayfarer"));
    assert!(!entry.related.is_empty(), "the detail-only fields came with it");
    ends(&updates, job);
}

#[tokio::test]
async fn a_franchises_seasons_are_the_same_from_every_one_of_them() {
    let [first, _, third, _, last] = FRANCHISE_SEASONS;
    // Each the first page opened, as on a launch: the page waits for the walk along the
    // franchise and comes once, with all five, two steps further for every request.
    for (start, walked) in [(first, 2), (third, 1), (last, 2)] {
        let mut h = with_franchise(Options::default()).await;
        let job = h.send(Command::LoadDetail(start));
        let updates = h.settle().await;
        assert_eq!(names(&updates), ["ProviderPreference", "Detail", "JobDone"], "{start}");
        assert_eq!(seasons_said(&updates), [Some(FRANCHISE_SEASONS.to_vec())], "{start}");
        assert_eq!(h.server().requests_for(Operation::Relations).len(), walked, "{start}");
        ends(&updates, job);
    }
}

#[tokio::test]
async fn a_franchise_walked_once_is_known_on_every_page_without_asking() {
    let mut h = with_franchise(Options::default()).await;
    h.send(Command::LoadDetail(FRANCHISE_SEASONS[0]));
    h.settle().await;
    let walked = h.server().requests_for(Operation::Relations).len();
    // Every season, and the film between the second and the third, which is none of them.
    for id in FRANCHISE_SEASONS.into_iter().chain([FRANCHISE_FILM]) {
        h.send(Command::LoadDetail(id));
        let updates = h.settle().await;
        assert_eq!(seasons_said(&updates), [Some(FRANCHISE_SEASONS.to_vec())], "{id}");
    }
    assert_eq!(h.server().requests_for(Operation::Relations).len(), walked, "nothing asked");
    // The OVA and the side story are no prequel or sequel of a season: none to pick.
    for id in [FRANCHISE_OVA, FRANCHISE_SIDE_STORY] {
        h.send(Command::LoadDetail(id));
        let updates = h.settle().await;
        assert_eq!(seasons_said(&updates), [Some(Vec::new())], "{id}");
    }
}

#[tokio::test]
async fn a_page_whose_seasons_take_long_comes_without_them_and_never_renumbers() {
    let mut h = with_franchise(Options {
        season_hold: Some(std::time::Duration::from_millis(50)),
        ..Options::default()
    })
    .await;
    h.server().answer_slowly(Operation::Relations, std::time::Duration::from_millis(400));
    let job = h.send(Command::LoadDetail(FRANCHISE_SEASONS[2]));
    let updates = h.settle().await;
    // The page first, without seasons, then with all five. Never what the third season's page
    // alone knows, the second to the fourth, which would have numbered it Season 2.
    assert_eq!(names(&updates), ["ProviderPreference", "Detail", "Detail", "JobDone"]);
    assert_eq!(seasons_said(&updates), [None, Some(FRANCHISE_SEASONS.to_vec())]);
    ends(&updates, job);
}

#[tokio::test]
async fn a_title_and_its_sequel_have_both_seasons_from_their_own_pages() {
    let mut h = Harness::new().await;
    for id in [FINISHED, SEQUEL] {
        h.send(Command::LoadDetail(id));
        let updates = h.settle().await;
        assert_eq!(seasons_said(&updates), [Some(vec![FINISHED, SEQUEL])], "{id}");
    }
    assert!(
        h.server().requests_for(Operation::Relations).is_empty(),
        "a page sees two steps along the chain itself"
    );
    // A title with no prequel or sequel has no seasons to pick.
    h.send(Command::LoadDetail(AIRING));
    assert_eq!(seasons_said(&h.settle().await), [Some(Vec::new())]);
}

#[tokio::test]
async fn a_pin_written_inline_is_what_the_next_task_reads() {
    let mut h = harness(with_alt()).await;
    // Sent together, as two keys pressed in a row: the write lands before the load runs.
    let pinned =
        h.send(Command::SetProviderPreference { id: FINISHED, provider: Some("alt".into()) });
    let job = h.send(Command::LoadDetail(FINISHED));
    let updates = h.settle().await;

    assert!(pinned.is_none(), "a write nobody waits on");
    // The settings first, with the shows that keep a source of their own counted again.
    assert_eq!(
        names(&updates),
        ["Settings", "Toast", "ProviderPreference", "Detail", "JobDone"]
    );
    assert_eq!(kept_sources(&updates[0]), 1);
    assert_eq!(toasts(&updates), ["info: alt will be used for this title"]);
    assert!(matches!(&updates[2], Event::ProviderPreference(Some(p)) if p == "alt"));
    ends(&updates, job);
    assert_eq!(h.store().provider_preference(FINISHED).unwrap().as_deref(), Some("alt"));

    h.send(Command::SetProviderPreference { id: FINISHED, provider: None });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Settings", "Toast"]);
    assert_eq!(kept_sources(&updates[0]), 0);
    assert_eq!(toasts(&updates), ["info: back to automatic source selection"]);
    assert_eq!(h.store().provider_preference(FINISHED).unwrap(), None);
}

#[tokio::test]
async fn a_title_that_cannot_be_loaded_is_a_toast_after_the_preference() {
    let mut h = Harness::new().await;
    // An id AniList has no title for, which it answers as an error rather than "not found".
    let job = h.send(Command::LoadDetail(MISSING));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["ProviderPreference", "Toast", "JobDone"]);
    let said = toasts(&updates);
    assert!(said[0].starts_with("alert: could not load title: "), "{said:?}");
    ends(&updates, job);
}

#[tokio::test]
async fn a_matched_titles_episodes_are_named_and_carry_its_history() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 600);
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Episodes", "JobDone"]);
    let Event::Episodes { id, rows } = &updates[0] else { unreachable!() };
    assert_eq!((*id, rows.len()), (FINISHED, 12));
    assert_eq!(rows[0].title.as_deref(), Some("The Lantern Road"));
    let done: Vec<&str> =
        rows.iter().filter(|r| r.completed).map(|r| r.number.as_str()).collect();
    assert_eq!(done, ["1", "2"]);
    ends(&updates, job);
}

#[tokio::test]
async fn how_far_a_dub_goes_is_the_sources_own_listing_in_the_dub() {
    let mut h = Harness::new().await;
    h.send(Command::LoadDubbedTo(FINISHED));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["DubbedTo"]);
    let Event::DubbedTo { id, episode } = &updates[0] else { unreachable!() };
    let last = sources::episodes(FINISHED).last().unwrap().number.as_str().to_owned();
    assert_eq!((*id, episode.as_deref()), (FINISHED, Some(last.as_str())));

    // A title no source has: nothing to say of its dub.
    h.send(Command::LoadDubbedTo(MISSING));
    let updates = h.settle().await;
    assert!(matches!(&updates[..], [Event::DubbedTo { id: MISSING, episode: None }]));
}

#[tokio::test]
async fn an_episode_the_schedule_moved_later_says_when_it_was_to_air() {
    let mut h = Harness::new().await;
    h.send(Command::LoadEpisodes(AIRING));
    let updates = h.settle().await;
    let Event::Episodes { rows, .. } = &updates[0] else {
        unreachable!("{:?}", names(&updates))
    };
    let row = rows.iter().find(|row| row.airs_at.is_some()).expect("a dated episode");
    assert!(rows.iter().all(|row| row.delayed_from.is_none()), "nothing seen move yet");
    let (number, airs_at) = (row.number.clone(), row.airs_at.unwrap());

    // As if this device had seen it an hour earlier on the schedule, before AniList moved it.
    let episode = number.parse().unwrap();
    h.store().remember_airings(&[(AIRING, episode, airs_at - 3_600)], FIXTURE_NOW).unwrap();
    h.send(Command::LoadEpisodes(AIRING));
    let updates = h.settle().await;
    let Event::Episodes { rows, .. } = &updates[0] else { unreachable!() };
    let moved: Vec<_> =
        rows.iter().filter_map(|row| Some((row.number.as_str(), row.delayed_from?))).collect();
    assert_eq!(moved, [(number.as_str(), airs_at - 3_600)]);
}

#[tokio::test]
async fn an_unsure_match_is_asked_about_and_the_answer_sticks_until_cleared() {
    let mut h = harness(unsure()).await;
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["MatchChoices", "JobDone"]);
    let Event::MatchChoices { id, provider_id, candidates } = &updates[0] else {
        unreachable!()
    };
    assert_eq!((*id, provider_id.as_str()), (FINISHED, sources::PROVIDER_ID));
    let offered: Vec<_> = candidates.iter().map(|c| (c.title.as_str(), &c.key)).collect();
    assert_eq!(offered, [("Tabibito", &sources::key(FINISHED))]);
    ends(&updates, job);

    // Picked: remembered as an override, so the table comes and the question does not return.
    let job = h.send(Command::FixMatch {
        id: FINISHED,
        provider_id: sources::PROVIDER_ID.into(),
        key: sources::key(FINISHED),
    });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Episodes", "JobDone"]);
    assert_eq!(numbers(&updates[0]).len(), 12);
    ends(&updates, job);
    let pinned = h.store().lookup_mapping(FINISHED, sources::PROVIDER_ID).unwrap().unwrap();
    assert_eq!(pinned.rung, ResolutionRung::Override);
    h.send(Command::LoadEpisodes(FINISHED));
    assert_eq!(names(&h.settle().await), ["Episodes", "JobDone"]);

    // Cleared: forgotten, and asked again.
    let job = h.send(Command::ClearMatch { id: FINISHED });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["MatchChoices", "JobDone"]);
    ends(&updates, job);
    assert!(h.store().lookup_mapping(FINISHED, sources::PROVIDER_ID).unwrap().is_none());
}

#[tokio::test]
async fn a_title_no_source_has_gets_an_empty_table_and_then_the_reason() {
    let mut h = harness(ProviderRegistry::new(vec![MockProvider::new("fixture").arc()])).await;
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Episodes", "Toast", "JobDone"]);
    assert!(numbers(&updates[0]).is_empty());
    assert_eq!(toasts(&updates), ["alert: could not list episodes: fixture found no matches"]);
    ends(&updates, job);
}

#[tokio::test]
async fn with_no_sources_at_all_the_table_is_refused_before_asking_anyone() {
    let mut h = harness(ProviderRegistry::new(Vec::new())).await;
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Episodes", "Toast", "JobDone"]);
    assert!(numbers(&updates[0]).is_empty());
    assert_eq!(toasts(&updates), ["alert: no sources configured"]);
    ends(&updates, job);
    assert!(h.server().requests().is_empty());
}

#[tokio::test]
async fn a_pinned_source_lists_its_own_episodes() {
    let mut h = harness(with_alt()).await;
    h.send(Command::SetProviderPreference { id: FINISHED, provider: Some("alt".into()) });
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Settings", "Toast", "Episodes", "JobDone"]);
    assert_eq!(numbers(&updates[2]), ["1", "2", "3"], "alt's numbering, which Enter plays");
    ends(&updates, job);

    h.send(Command::SetProviderPreference { id: FINISHED, provider: None });
    h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Settings", "Toast", "Episodes", "JobDone"]);
    assert_eq!(numbers(&updates[2]).len(), 12, "the first source in the chain again");
}

#[tokio::test]
async fn an_episodes_releases_are_listed_for_the_picker() {
    let release = SourceCandidate {
        id: "release-1".into(),
        provider_id: sources::PROVIDER_ID.into(),
        title: "Stardust Wayfarer - 01 (1080p)".into(),
        quality: Some(1080),
        seeders: None,
        size: None,
        dual_audio: false,
        dubbed: false,
        auto_pick: true,
    };
    let provider = sources::provider().with_sources(vec![release]);
    let mut h = harness(ProviderRegistry::new(vec![provider.arc()])).await;
    let job = h.send(Command::LoadSources { id: FINISHED, episode: "1".into() });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Sources", "JobDone"]);
    let Event::Sources { id, episode, candidates } = &updates[0] else { unreachable!() };
    assert_eq!((*id, episode.as_str()), (FINISHED, "1"));
    let listed: Vec<_> = candidates.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(listed, ["release-1"]);
    ends(&updates, job);
}

#[tokio::test]
async fn releases_that_cannot_be_listed_are_a_toast() {
    let mut h = harness(ProviderRegistry::new(Vec::new())).await;
    let job = h.send(Command::LoadSources { id: FINISHED, episode: "1".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: no sources configured — see the Providers screen"]);
    ends(&updates, job);

    let mut h = harness(unsure()).await;
    let job = h.send(Command::LoadSources { id: FINISHED, episode: "1".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert!(toasts(&updates)[0].starts_with("alert: could not match this title: "));
    ends(&updates, job);
}

#[tokio::test]
async fn a_search_in_your_own_words_offers_ranked_choices() {
    let mut h = Harness::new().await;
    let job = h.send(Command::ManualSearch { id: FINISHED, query: "tabibito".into() });
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["MatchChoices", "JobDone"]);
    let Event::MatchChoices { id, provider_id, candidates } = &updates[0] else {
        unreachable!()
    };
    assert_eq!((*id, provider_id.as_str()), (FINISHED, sources::PROVIDER_ID));
    assert_eq!(candidates[0].key, sources::key(FINISHED), "ranked against the real title");
    assert!(candidates.windows(2).all(|pair| pair[0].similarity >= pair[1].similarity));
    ends(&updates, job);

    let mut h = harness(ProviderRegistry::new(vec![MockProvider::new("fixture").arc()])).await;
    let job = h.send(Command::ManualSearch { id: FINISHED, query: "tabibito".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: nothing found for \"tabibito\""]);
    ends(&updates, job);
}

#[tokio::test]
async fn a_titles_page_carries_what_only_it_fetches_and_a_list_does_not() {
    let mut h = Harness::new().await;
    h.send(Command::LoadDetail(FINISHED));
    let updates = h.settle().await;
    let Event::Detail(entry) = &updates[1] else { panic!("the page: {updates:?}") };

    // What every list carries too.
    assert_eq!(entry.season, Some(Season::Spring));
    assert_eq!(entry.start_date, Some(FuzzyDate { year: 2024, month: Some(4), day: Some(6) }));
    assert_eq!(entry.end_date, Some(FuzzyDate { year: 2024, month: Some(6), day: Some(22) }));
    assert_eq!(entry.source, Some(SourceMaterial::Manga));
    assert_eq!(entry.title_native.as_deref(), Some("星屑の旅人"));
    assert_eq!(entry.title_romaji.as_deref(), Some("Hoshikuzu no Tabibito"));
    assert!(!entry.adult);

    // And what only the page does.
    let detail = entry.detail.as_ref().expect("the page's own fields");
    assert_eq!(detail.schedule.len(), 12, "every episode's broadcast");
    let weeks = detail.schedule.windows(2).all(|w| w[1].airs_at - w[0].airs_at == 7 * 86_400);
    assert!(weeks && detail.schedule[0].episode == 1, "{:?}", detail.schedule);
    assert!(detail.tags.iter().any(|tag| tag.media_spoiler), "the spoiler is marked");
    assert_eq!(detail.characters[0].role, CastRole::Main);
    assert_eq!(detail.characters[0].voice_actors[0].name, "Ren's voice");
    assert_eq!(detail.relations[0].relation, Some(RelationKind::Sequel));
    assert_eq!(detail.relations[0].season, Some(Season::Winter));
    assert!(detail.relations[0].cover_url.is_some(), "a card has its cover");
    assert_eq!(detail.recommendations.len(), entry.recommended.len());
    assert_eq!(detail.rankings[0].kind, RankingKind::Rated);
    assert_eq!(detail.staff[0].role, "Director");

    h.send(Command::Search { query: "hoshikuzu".into(), filter: SearchFilters::default() });
    let updates = h.settle().await;
    let listed = updates
        .iter()
        .find_map(|update| match update {
            Event::Content { content, .. } => {
                content.entries().iter().find(|e| e.id == FINISHED)
            }
            _ => None,
        })
        .expect("the search lists it");
    assert_eq!(listed.season, Some(Season::Spring));
    assert!(listed.detail.is_none(), "a list fetches none of the page");
}

#[tokio::test]
async fn each_episode_row_says_when_it_aired() {
    let mut h = Harness::new().await;
    h.send(Command::LoadEpisodes(AIRING));
    let updates = h.settle().await;
    let rows = updates
        .iter()
        .find_map(|update| match update {
            Event::Episodes { rows, .. } => Some(rows.clone()),
            _ => None,
        })
        .expect("the table");
    let times = fixtures::media(AIRING).air_times();
    assert!(!rows.is_empty());
    for row in &rows {
        let number: u32 = row.number.parse().expect("numbered");
        assert_eq!(row.airs_at, times.get(&number).copied(), "ep {number}");
        assert!(row.airs_at.is_some_and(|at| at <= FIXTURE_NOW), "listed, so out");
    }
}

#[tokio::test]
async fn the_signed_in_viewers_title_language_is_said_when_asked() {
    // Nobody signed in to AniList: there is no language to say, and nothing is asked.
    let said = |updates: &[Event]| {
        updates.iter().find_map(|update| match update {
            Event::ViewerTitleLanguage(language) => Some(*language),
            _ => None,
        })
    };
    let mut h = Harness::new().await;
    h.send(Command::LoadTitleLanguage);
    let updates = h.settle().await;
    assert_eq!(said(&updates), Some(None), "{updates:?}");
    assert!(h.server().requests_for(anistream_testkit::Operation::Viewer).is_empty());

    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    for _ in 0..2 {
        h.send(Command::LoadTitleLanguage);
        let updates = h.settle().await;
        assert_eq!(said(&updates), Some(Some(TitleLanguage::English)), "{updates:?}");
    }
    let asked = h.server().requests_for(anistream_testkit::Operation::Viewer);
    assert_eq!(asked.len(), 1, "the viewer is looked up once and kept");
}

#[tokio::test]
async fn a_title_says_where_it_is_in_its_run() {
    use anistream_model::catalog::AiringStatus;
    let mut h = Harness::new().await;
    for (id, status) in [(AIRING, AiringStatus::Releasing), (FINISHED, AiringStatus::Finished)]
    {
        h.send(Command::LoadDetail(id));
        let updates = h.settle().await;
        let entry = updates.iter().find_map(|update| match update {
            Event::Detail(entry) => Some(entry.clone()),
            _ => None,
        });
        assert_eq!(entry.and_then(|e| e.status), Some(status), "{id}");
    }
}

#[tokio::test]
async fn related_cards_say_where_each_title_is_on_the_library_and_carry_its_colour() {
    use anistream_model::catalog::LibrarySegment;
    use anistream_testkit::fixtures::SEQUEL;
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        ..Options::default()
    })
    .await;
    let page = |updates: Vec<Event>| {
        updates
            .into_iter()
            .find_map(|update| match update {
                Event::Detail(entry) => entry.detail,
                _ => None,
            })
            .expect("the page")
    };

    // Before anything is pulled, nothing is known of the library.
    h.send(Command::LoadDetail(SEQUEL));
    let detail = page(h.settle().await);
    assert_eq!(detail.relations[0].list_status, None);
    assert_eq!(detail.relations[0].cover_color.as_deref(), Some("#e4a15d"), "its own colour");

    // A sync pulls the library, and its statuses are kept: the prequel was watched.
    h.send(Command::SyncNow);
    h.settle().await;
    h.send(Command::LoadDetail(SEQUEL));
    let detail = page(h.settle().await);
    assert_eq!(detail.relations[0].list_status, Some(LibrarySegment::Completed));

    // And a Library load keeps them too, every segment's: a recommendation is planned.
    h.send(Command::LoadLibrary(LibrarySegment::Watching));
    h.settle().await;
    h.send(Command::LoadDetail(FINISHED));
    let detail = page(h.settle().await);
    let cards: Vec<_> =
        detail.recommendations.iter().map(|card| (card.id, card.list_status)).collect();
    assert_eq!(
        cards,
        [
            (anistream_testkit::fixtures::ONA, None),
            (anistream_testkit::fixtures::MOVIE, Some(LibrarySegment::Planning)),
        ]
    );
}
