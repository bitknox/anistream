//! A title's page, its episode table, and the questions of which source and which match.
//!
//! The title's pinned source is said before the title itself, so the picker is right from the
//! first frame. An episode table that cannot be filled still arrives, empty, before the reason
//! does, so the screen leaves its loading state whatever happened.

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
    fixtures::{self, AIRING, FINISHED, FIXTURE_NOW, MISSING},
    seed, sources,
};

use super::{WITH_ANILIST, ends, toasts};
use crate::harness::{Harness, Options, names};

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
        assert_eq!(said(&updates), Some(Some(TitleLanguage::Romaji)), "{updates:?}");
    }
    let asked = h.server().requests_for(anistream_testkit::Operation::Viewer);
    assert_eq!(asked.len(), 1, "the viewer is looked up once and kept");
}
