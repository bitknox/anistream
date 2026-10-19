//! The rules every job keeps, whatever the task.
//!
//! A job is started for work someone waits on, and ends — once, and after everything its work
//! sent — however that work ends: with an answer, with a failure, or refused before it began.
//! Work nobody waits on starts none. [`Harness::settle`] also fails any test that leaves a job
//! open, so every test here and in the other groups holds the second half of the rule.

use anistream_core::settings::SettingValue;
use anistream_providers::ProviderRegistry;
use anistream_store::DownloadState;
use anistream_testkit::{
    Failure,
    fixtures::{FINISHED, FIXTURE_NOW, MISSING},
    seed, sources,
};
use anistream_ui::{
    LibrarySegment, PlayerCommand,
    app::{ContentFor, SearchFilters, Task, Update},
};

use super::{NO_FALLBACK, ends};
use crate::harness::{Harness, Options, names};

/// How a case's app is set up before its task is sent.
#[derive(Clone, Copy)]
enum Setup {
    /// The defaults.
    Plain,
    /// Some history, so Continue has something to look up.
    History,
    /// AniList failing, with no fallback to answer instead.
    Failing,
    /// No sources at all.
    NoSources,
    /// A source that has heard of nothing.
    NothingFound,
}

async fn harness(setup: Setup) -> Harness {
    let mut options = Options::default();
    match setup {
        Setup::Failing => options.config = Some(NO_FALLBACK.into()),
        Setup::NoSources => options.registry = Some(ProviderRegistry::new(Vec::new())),
        Setup::NothingFound => {
            let bare = anistream_providers::MockProvider::new(sources::PROVIDER_ID);
            options.registry = Some(ProviderRegistry::new(vec![bare.arc()]));
        }
        Setup::Plain | Setup::History => {}
    }
    let h = Harness::with(options).await;
    match setup {
        Setup::History => seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 600),
        Setup::Failing => h.server().fail_always(Failure::Status(500)),
        _ => {}
    }
    h
}

fn search(query: &str) -> Task {
    Task::Search { query: query.into(), filter: SearchFilters::default() }
}

#[tokio::test]
async fn a_job_ends_last_on_every_way_out_of_its_work() {
    use Setup::*;
    let cases: Vec<(&str, Setup, Task)> = vec![
        ("continue, nothing watched", Plain, Task::LoadContinue),
        ("continue, listed", History, Task::LoadContinue),
        ("continue, failed", Failing, Task::LoadContinue),
        ("seasonal", Plain, Task::LoadSeasonal),
        ("seasonal, failed", Failing, Task::LoadSeasonal),
        ("calendar, failed", Failing, Task::LoadCalendar),
        ("calendar past", Plain, Task::LoadCalendarPast { anchor: FIXTURE_NOW, page: 2 }),
        (
            "calendar past, failed",
            Failing,
            Task::LoadCalendarPast { anchor: FIXTURE_NOW, page: 2 },
        ),
        ("search", Plain, search("hoshikuzu")),
        ("browse", Plain, search("")),
        ("search, failed", Failing, search("hoshikuzu")),
        ("detail", Plain, Task::LoadDetail(FINISHED)),
        ("detail, unknown", Plain, Task::LoadDetail(MISSING)),
        ("episodes", Plain, Task::LoadEpisodes(FINISHED)),
        ("episodes, failed", Failing, Task::LoadEpisodes(FINISHED)),
        ("episodes, not found", NothingFound, Task::LoadEpisodes(FINISHED)),
        ("episodes, refused", NoSources, Task::LoadEpisodes(FINISHED)),
        ("clear match", Plain, Task::ClearMatch { id: FINISHED }),
        (
            "fix match",
            Plain,
            Task::FixMatch {
                id: FINISHED,
                provider_id: sources::PROVIDER_ID.into(),
                key: sources::key(FINISHED),
            },
        ),
        ("sources", Plain, Task::LoadSources { id: FINISHED, episode: "1".into() }),
        (
            "sources, refused",
            NoSources,
            Task::LoadSources { id: FINISHED, episode: "1".into() },
        ),
        ("manual search", Plain, Task::ManualSearch { id: FINISHED, query: "tabibito".into() }),
        (
            "manual search, nothing",
            NothingFound,
            Task::ManualSearch { id: FINISHED, query: "tabibito".into() },
        ),
        ("providers", Plain, Task::CheckProviders),
        ("library, signed out", Plain, Task::LoadLibrary(LibrarySegment::Watching)),
        ("sync, signed out", Plain, Task::SyncNow),
        (
            "download, refused",
            Plain,
            Task::DownloadEpisode { id: FINISHED, episode: "1".into() },
        ),
        (
            "download range, refused",
            Plain,
            Task::DownloadMany { id: FINISHED, episodes: vec!["1".into()] },
        ),
        ("stores", Plain, Task::LoadStores),
    ];
    for (case, setup, task) in cases {
        let mut h = harness(setup).await;
        let job = h.send(task);
        let updates = h.settle().await;
        assert!(job.is_some(), "{case}: a job");
        assert!(updates.len() > 1, "{case}: an answer before the end: {:?}", names(&updates));
        ends(&updates, job);
    }
}

#[tokio::test]
async fn work_nobody_waits_on_starts_no_job() {
    let mut h = Harness::new().await;
    let queued = seed::download(h.store(), FINISHED, "1", DownloadState::Queued);
    let done = seed::download(h.store(), FINISHED, "2", DownloadState::Done);
    let failed = seed::download(h.store(), FINISHED, "3", DownloadState::Failed);
    let tasks = [
        Task::SetWatched { id: FINISHED, episodes: vec!["1".into()], watched: true },
        Task::HideFromContinue { id: FINISHED },
        Task::SetStatus { id: FINISHED, status: LibrarySegment::Watching },
        Task::SetScore { id: FINISHED, score: 7.0 },
        Task::ResolveConflict { id: FINISHED, keep_local: true },
        Task::SetProviderPreference { id: FINISHED, provider: Some("fixture".into()) },
        Task::SaveSetting {
            table: &["playback"],
            key: "auto_next",
            value: SettingValue::Bool(false),
        },
        Task::LoadDownloads,
        Task::DownloadPause { id: queued.id },
        Task::DownloadCancel { id: failed.id },
        Task::DownloadDelete { id: done.id },
        Task::DownloadClearCompleted,
        // Nothing is playing, so the command goes nowhere.
        Task::Player(PlayerCommand::TogglePause),
        // Found ahead quietly, for a Play of the same episode to pick up.
        Task::Prefetch { id: FINISHED, episode: "2".into() },
        Task::Disconnect { tracker: "kitsu".into() },
        Task::RemovePlugin { plugin: "absent".into() },
    ];
    for task in tasks {
        let said = format!("{task:?}");
        assert!(h.send(task).is_none(), "{said} started a job");
        let updates = h.settle().await;
        assert!(!names(&updates).contains(&"JobDone"), "{said}: {:?}", names(&updates));
    }
}

#[tokio::test]
async fn a_load_held_up_by_the_rate_limit_says_so_on_its_job_first() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 1, FIXTURE_NOW - 60);
    // A spent budget, held for a second: long enough to be worth a note, short for a test.
    h.server().fail_next(Failure::RateLimited { retry_after_secs: 1 });
    h.send(Task::LoadContinue);
    h.settle().await;

    let job = h.send(Task::LoadContinue);
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["JobNote", "Content", "JobDone"]);
    let Update::JobNote { job: noted, note, until } = &updates[0] else { unreachable!() };
    assert_eq!((Some(*noted), note.as_str()), (job, "AniList limit"));
    assert!(until.is_some(), "it counts down to when the wait is over");
    ends(&updates, job);
}

#[tokio::test]
async fn loads_sent_together_each_end_after_their_own_answer() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 600);
    let sent = [
        (h.send(Task::LoadContinue), ContentFor::Continue),
        (h.send(Task::LoadSeasonal), ContentFor::Seasonal),
        (
            h.send(search("hoshikuzu")),
            ContentFor::Search { query: "hoshikuzu".into(), filter: SearchFilters::default() },
        ),
    ];
    let updates = h.settle().await;

    let jobs: Vec<_> = sent.iter().map(|(job, _)| job.expect("a job")).collect();
    assert!(jobs.windows(2).all(|pair| pair[0] != pair[1]), "one job each");
    for (job, view) in &sent {
        let answered = updates
            .iter()
            .position(|u| matches!(u, Update::Content { view: v, .. } if v == view))
            .unwrap_or_else(|| panic!("{view:?} answered: {:?}", names(&updates)));
        let ended = updates
            .iter()
            .position(|u| matches!(u, Update::JobDone(id) if Some(*id) == *job))
            .expect("the job ended");
        assert!(answered < ended, "{view:?}: {:?}", names(&updates));
    }
    let ended = names(&updates).iter().filter(|name| **name == "JobDone").count();
    assert_eq!(ended, 3);
}
