//! A title's page, its episode table, and the questions of which source and which match.
//!
//! The title's pinned source is said before the title itself, so the picker is right from the
//! first frame. An episode table that cannot be filled still arrives, empty, before the reason
//! does, so the screen leaves its loading state whatever happened.

use anistream_core::{media::SearchHit, traits::SourceCandidate};
use anistream_model::{command::Command, event::Event};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_store::ResolutionRung;
use anistream_testkit::{
    fixtures::{FINISHED, FIXTURE_NOW, MISSING},
    seed, sources,
};

use super::{ends, toasts};
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
    assert_eq!(names(&updates), ["Toast", "ProviderPreference", "Detail", "JobDone"]);
    assert_eq!(toasts(&updates), ["info: alt will be used for this title"]);
    assert!(matches!(&updates[1], Event::ProviderPreference(Some(p)) if p == "alt"));
    ends(&updates, job);
    assert_eq!(h.store().provider_preference(FINISHED).unwrap().as_deref(), Some("alt"));

    h.send(Command::SetProviderPreference { id: FINISHED, provider: None });
    let updates = h.settle().await;
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
    assert_eq!(names(&updates), ["Toast", "Episodes", "JobDone"]);
    assert_eq!(numbers(&updates[1]), ["1", "2", "3"], "alt's numbering, which Enter plays");
    ends(&updates, job);

    h.send(Command::SetProviderPreference { id: FINISHED, provider: None });
    h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "Episodes", "JobDone"]);
    assert_eq!(numbers(&updates[1]).len(), 12, "the first source in the chain again");
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
