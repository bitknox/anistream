//! A title's episodes when no source lists it: from AniList's own data, with none set up and
//! when the sources there cannot find the show. The table comes as it always does, its rows
//! marked as AniList's alone, and only a source that was asked says why; what a source would
//! do with the rows, a download or keeping the next ones, is not asked of it. Once a source has
//! the show, its table is the source's.

use anistream_model::{command::Command, episodes::EpisodeRow, event::Event};
use anistream_providers::{MockProvider, ProviderRegistry};
use anistream_testkit::{
    Operation,
    fixtures::{AIRING, FINISHED, FIXTURE_NOW},
    seed, sources,
};

use super::{ends, toasts};
use crate::harness::{Harness, Options, names};

async fn harness(registry: ProviderRegistry) -> Harness {
    Harness::with(Options { registry: Some(registry), ..Options::default() }).await
}

/// No source set up at all.
fn none() -> ProviderRegistry {
    ProviderRegistry::new(Vec::new())
}

/// A source that has none of the fixture titles.
fn unaware() -> ProviderRegistry {
    ProviderRegistry::new(vec![MockProvider::new("fixture").arc()])
}

/// The rows of the one table among `updates`.
#[track_caller]
fn table(updates: &[Event]) -> &[EpisodeRow] {
    let mut tables = updates.iter().filter_map(|update| match update {
        Event::Episodes { rows, .. } => Some(rows.as_slice()),
        _ => None,
    });
    let rows = tables.next().expect("a table was sent");
    assert!(tables.next().is_none(), "one table: {:?}", names(updates));
    rows
}

fn numbers(rows: &[EpisodeRow]) -> Vec<&str> {
    rows.iter().map(|row| row.number.as_str()).collect()
}

#[tokio::test]
async fn with_no_sources_a_finished_shows_episodes_are_anilists() {
    let mut h = harness(none()).await;
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    // The table and nothing else: no source was asked, so there is nothing to say yet.
    assert_eq!(names(&updates), ["Episodes", "JobDone"]);
    let rows = table(&updates);
    let expected: Vec<String> = (1..=12).map(|n| n.to_string()).collect();
    assert_eq!(numbers(rows), expected, "as many as AniList counts");
    assert!(rows.iter().all(|row| row.metadata_only));
    // Named, given stills and dated by AniList's listings and broadcasts.
    assert_eq!(rows[0].title.as_deref(), Some("The Lantern Road"));
    assert_eq!(rows[0].thumbnail.as_deref(), Some("https://img.example/ep/900001/1.jpg"));
    assert_eq!(rows[0].duration_secs, Some(24 * 60));
    assert!(rows.iter().all(|row| row.airs_at.is_some_and(|at| at <= FIXTURE_NOW)));
    ends(&updates, job);
}

#[tokio::test]
async fn with_no_sources_an_airing_show_has_what_aired_and_the_next_to_come() {
    let mut h = harness(none()).await;
    h.send(Command::LoadEpisodes(AIRING));
    let updates = h.settle().await;
    let rows = table(&updates);
    assert_eq!(numbers(rows), ["1", "2", "3", "4", "5", "6"]);
    assert!(rows[..5].iter().all(|row| row.airs_at.is_some_and(|at| at <= FIXTURE_NOW)));
    assert!(rows[5].airs_at.is_some_and(|at| at > FIXTURE_NOW), "the sixth is still to air");
    assert!(toasts(&updates).is_empty());
}

#[tokio::test]
async fn a_source_that_cannot_find_the_show_leaves_anilists_episodes_and_says_so_quietly() {
    let mut h = harness(unaware()).await;
    let job = h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;

    assert_eq!(names(&updates), ["Episodes", "Toast", "JobDone"]);
    let rows = table(&updates);
    assert_eq!(rows.len(), 12);
    assert!(rows.iter().all(|row| row.metadata_only));
    assert_eq!(
        toasts(&updates),
        ["info: no source could list this show: fixture found no matches"]
    );
    ends(&updates, job);
}

#[tokio::test]
async fn once_a_source_has_the_show_its_table_is_the_sources_with_the_marks_kept() {
    let mut h = harness(none()).await;
    h.send(Command::LoadEpisodes(FINISHED));
    h.settle().await;
    // Marked on AniList's rows, by number.
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["3".into()], watched: true });
    h.settle().await;

    // A source is added, and the title is opened again.
    h.registry().put(sources::provider().arc());
    h.send(Command::LoadEpisodes(FINISHED));
    let updates = h.settle().await;
    let rows = table(&updates);
    assert!(rows.iter().all(|row| !row.metadata_only), "the source's own table");
    assert_eq!(rows.len(), sources::episodes(FINISHED).len());
    let done: Vec<_> = rows.iter().filter(|row| row.completed).map(|row| &row.number).collect();
    assert_eq!(done, ["3"], "the mark made before the source came");
}

#[tokio::test]
async fn a_download_of_a_show_no_source_lists_is_refused_without_asking_one() {
    // None set up.
    let mut h = harness(none()).await;
    let job = h.send(Command::DownloadEpisode { id: FINISHED, episode: "1".into() });
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Toast", "JobDone"]);
    assert_eq!(toasts(&updates), ["alert: no sources configured: see the Providers screen"]);
    ends(&updates, job);
    assert!(h.store().downloads().unwrap().is_empty());

    // One that could not list it: the table said so, and the source is not asked again.
    let provider = MockProvider::new("fixture");
    let calls = provider.call_count();
    let mut h = harness(ProviderRegistry::new(vec![provider.arc()])).await;
    h.send(Command::LoadEpisodes(FINISHED));
    h.settle().await;
    let asked = calls.load(std::sync::atomic::Ordering::SeqCst);
    for task in [
        Command::DownloadEpisode { id: FINISHED, episode: "1".into() },
        Command::DownloadMany { id: FINISHED, episodes: vec!["1".into(), "2".into()] },
    ] {
        h.send(task);
        let updates = h.settle().await;
        assert_eq!(
            toasts(&updates),
            ["alert: no source could list this show, so it can't be downloaded"]
        );
    }
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), asked, "not asked again");
    assert!(h.store().downloads().unwrap().is_empty());
}

#[tokio::test]
async fn keep_next_queues_nothing_with_no_source_to_download_from() {
    let mut h = Harness::with(Options {
        keep_next: true,
        registry: Some(none()),
        config: Some("[downloads]\nkeep_next = 2\n".into()),
        before_start: Some(|store| {
            seed::watched(store, FINISHED, 4, anistream_store::now() - 600)
        }),
        ..Options::default()
    })
    .await;
    // As on start, after a mark, and after an episode is played to its end.
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["5".into()], watched: true });
    h.settle().await;
    assert!(h.store().downloads().unwrap().is_empty(), "nothing queued");
    assert!(
        h.server().requests_for(Operation::Detail).is_empty(),
        "nor a show looked up for it"
    );
}
