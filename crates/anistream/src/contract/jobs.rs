//! The rules every job keeps, whatever the task.
//!
//! A job is started for work someone waits on, and ends — once, and after everything its work
//! sent — however that work ends: with an answer, with a failure, or refused before it began.
//! Work nobody waits on starts none. [`Harness::settle`] also fails any test that leaves a job
//! open, so every test here and in the other groups holds the second half of the rule.

use anistream_core::settings::SettingValue;
use anistream_model::{
    catalog::{ContentFor, LibrarySegment, SearchFilters},
    command::{Command, PlayerCommand},
    event::Event,
    settings::SettingKey,
};
use anistream_providers::ProviderRegistry;
use anistream_store::DownloadState;
use anistream_testkit::{
    Failure,
    fixtures::{FINISHED, FIXTURE_NOW, MISSING},
    seed, sources,
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

fn search(query: &str) -> Command {
    Command::Search { query: query.into(), filter: SearchFilters::default() }
}

#[tokio::test]
async fn a_job_ends_last_on_every_way_out_of_its_work() {
    use Setup::*;
    let cases: Vec<(&str, Setup, Command)> = vec![
        ("continue, nothing watched", Plain, Command::LoadContinue),
        ("continue, listed", History, Command::LoadContinue),
        ("continue, failed", Failing, Command::LoadContinue),
        ("seasonal", Plain, Command::LoadSeasonal),
        ("seasonal, failed", Failing, Command::LoadSeasonal),
        ("calendar, failed", Failing, Command::LoadCalendar),
        ("calendar past", Plain, Command::LoadCalendarPast { anchor: FIXTURE_NOW, page: 2 }),
        (
            "calendar past, failed",
            Failing,
            Command::LoadCalendarPast { anchor: FIXTURE_NOW, page: 2 },
        ),
        ("search", Plain, search("hoshikuzu")),
        ("browse", Plain, search("")),
        ("search, failed", Failing, search("hoshikuzu")),
        ("detail", Plain, Command::LoadDetail(FINISHED)),
        ("detail, unknown", Plain, Command::LoadDetail(MISSING)),
        ("episodes", Plain, Command::LoadEpisodes(FINISHED)),
        ("episodes, failed", Failing, Command::LoadEpisodes(FINISHED)),
        ("episodes, not found", NothingFound, Command::LoadEpisodes(FINISHED)),
        ("episodes, refused", NoSources, Command::LoadEpisodes(FINISHED)),
        ("clear match", Plain, Command::ClearMatch { id: FINISHED }),
        (
            "fix match",
            Plain,
            Command::FixMatch {
                id: FINISHED,
                provider_id: sources::PROVIDER_ID.into(),
                key: sources::key(FINISHED),
            },
        ),
        ("sources", Plain, Command::LoadSources { id: FINISHED, episode: "1".into() }),
        (
            "sources, refused",
            NoSources,
            Command::LoadSources { id: FINISHED, episode: "1".into() },
        ),
        (
            "manual search",
            Plain,
            Command::ManualSearch { id: FINISHED, query: "tabibito".into() },
        ),
        (
            "manual search, nothing",
            NothingFound,
            Command::ManualSearch { id: FINISHED, query: "tabibito".into() },
        ),
        ("providers", Plain, Command::CheckProviders),
        ("library, signed out", Plain, Command::LoadLibrary(LibrarySegment::Watching)),
        ("sync, signed out", Plain, Command::SyncNow),
        (
            "download, refused",
            Plain,
            Command::DownloadEpisode { id: FINISHED, episode: "1".into() },
        ),
        (
            "download range, refused",
            Plain,
            Command::DownloadMany { id: FINISHED, episodes: vec!["1".into()] },
        ),
        ("stores", Plain, Command::LoadStores),
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
        Command::SetWatched { id: FINISHED, episodes: vec!["1".into()], watched: true },
        Command::HideFromContinue { id: FINISHED },
        Command::SetStatus { id: FINISHED, status: LibrarySegment::Watching },
        Command::SetScore { id: FINISHED, score: 7.0 },
        Command::ResolveConflict { id: FINISHED, keep_local: true },
        Command::SetProviderPreference { id: FINISHED, provider: Some("fixture".into()) },
        Command::SetSetting { key: SettingKey::AutoNext, value: SettingValue::Bool(false) },
        Command::LoadDownloads,
        Command::DownloadPause { id: queued.id },
        Command::DownloadCancel { id: failed.id },
        Command::DownloadDelete { id: done.id },
        Command::DownloadClearCompleted,
        // Nothing is playing, so the command goes nowhere.
        Command::Player(PlayerCommand::TogglePause),
        // Found ahead quietly, for a Play of the same episode to pick up.
        Command::Prefetch { id: FINISHED, episode: "2".into() },
        Command::Disconnect { tracker: "kitsu".into() },
        Command::RemovePlugin { plugin: "absent".into() },
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
    h.send(Command::LoadContinue);
    h.settle().await;

    let job = h.send(Command::LoadContinue);
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["JobNote", "Content", "JobDone"]);
    let Event::JobNote { job: noted, note, wait } = &updates[0] else { unreachable!() };
    assert_eq!((Some(*noted), note.as_str()), (job, "AniList limit"));
    assert!(wait.is_some(), "it counts down to when the wait is over");
    ends(&updates, job);
}

#[tokio::test]
async fn loads_sent_together_each_end_after_their_own_answer() {
    let mut h = Harness::new().await;
    seed::watched(h.store(), FINISHED, 2, FIXTURE_NOW - 600);
    let sent = [
        (h.send(Command::LoadContinue), ContentFor::Continue),
        (h.send(Command::LoadSeasonal), ContentFor::Seasonal),
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
            .position(|u| matches!(u, Event::Content { view: v, .. } if v == view))
            .unwrap_or_else(|| panic!("{view:?} answered: {:?}", names(&updates)));
        let ended = updates
            .iter()
            .position(|u| matches!(u, Event::JobDone(id) if Some(*id) == *job))
            .expect("the job ended");
        assert!(answered < ended, "{view:?}: {:?}", names(&updates));
    }
    let ended = names(&updates).iter().filter(|name| **name == "JobDone").count();
    assert_eq!(ended, 3);
}
