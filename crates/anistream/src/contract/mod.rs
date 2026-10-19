//! Dispatch contract tests: what each task sends back, and in what order.
//!
//! Each test sends tasks through [`crate::harness`] and reads what settled, never `dispatch`
//! itself, so the same tests can hold the engine that replaces it to the same answers. They
//! assert the order first, as variant names: those stayed when the enum was renamed, and stay
//! while what the variants carry changes. Then the parts that matter, through the helpers here where
//! more than one test reads them. Nothing compares `Debug` output, which would churn at every
//! change of a field's type.
//!
//! Grouped by feature: the lists and loads, a title's page and its sources, settings, watching
//! and the trackers, downloads, plugin stores, and the rules every job keeps. Playback is not
//! here: it needs mpv, and the engine's `tests/binge.rs` covers it.

mod downloads;
mod jobs;
mod lists;
mod preview;
mod settings;
mod stores;
mod title;
mod watch;

use anistream_core::ids::AnilistId;
use anistream_model::{catalog::Content, event::Event, jobs::JobId, notice::NoticeKind};

use crate::harness::names;

/// The default config with Tenrai off, so a failing AniList fails the load rather than being
/// answered by the fallback's empty lists.
pub const NO_FALLBACK: &str = "[meta]\nfallback = false\n";

/// AniList as a tracker beside Kitsu, which is on by default. AniList is refused at load
/// without a client of its own, so it gets a made-up one.
pub const WITH_ANILIST: &str = "[trackers]\n\
    enabled = [\"anilist\", \"kitsu\"]\n\
    \n\
    [trackers.anilist]\n\
    client_id = \"harness\"\n\
    client_secret = \"harness\"\n";

/// The toast an update is, as `info: …` or `alert: …`.
pub fn toast(update: &Event) -> Option<String> {
    let Event::Toast(toast) = update else { return None };
    let kind = match toast.kind {
        NoticeKind::Info => "info",
        NoticeKind::Alert => "alert",
    };
    Some(format!("{kind}: {}", toast.text))
}

/// Every toast, in order.
pub fn toasts(updates: &[Event]) -> Vec<String> {
    updates.iter().filter_map(toast).collect()
}

/// What the one `Content` among `updates` carries.
#[track_caller]
pub fn content(updates: &[Event]) -> &Content {
    let mut lists = updates.iter().filter_map(|update| match update {
        Event::Content { content, .. } => Some(content),
        _ => None,
    });
    let first = lists.next().expect("a list was sent");
    assert!(lists.next().is_none(), "one list, not several: {:?}", names(updates));
    first
}

/// The titles a list holds, in its order.
#[track_caller]
pub fn ids(content: &Content) -> Vec<AnilistId> {
    match content {
        Content::Entries(entries) => entries.iter().map(|entry| entry.id).collect(),
        other => panic!("not a list of titles: {other:?}"),
    }
}

/// The reason a list failed with.
#[track_caller]
pub fn failure(content: &Content) -> &str {
    match content {
        Content::Failed(reason) => reason,
        other => panic!("not a failure: {other:?}"),
    }
}

/// That `job` was started, and ended exactly once, after everything its work sent.
#[track_caller]
pub fn ends(updates: &[Event], job: Option<JobId>) {
    let job = job.expect("work someone waits on starts a job");
    let done: Vec<usize> = updates
        .iter()
        .enumerate()
        .filter(|(_, update)| matches!(update, Event::JobDone(id) if *id == job))
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        done,
        [updates.len() - 1],
        "the job ends once, after the rest: {:?}",
        names(updates)
    );
}
