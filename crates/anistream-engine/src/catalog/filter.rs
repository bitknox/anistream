//! What may be suggested to the viewer, on Up next for you and on every discovery shelf of Home:
//! never a title already in Continue or started, never one finished, never one dismissed with
//! Not interested, never a sequel whose prequel is unfinished, and adult titles only when the
//! settings allow them.

use std::collections::HashSet;

use anistream_core::ids::AnilistId;
use anistream_meta::Meta;
use anistream_model::catalog::{AiringStatus, Entry};
use anistream_store::Store;
use serde::{Deserialize, Serialize};

/// Where the dismissals are kept, for Up next for you and Home alike.
pub(crate) const DISMISSED_KEY: &str = "up-next:dismissed";

/// A dismissal. `waiting` is a title that had not started airing when it was dismissed: it
/// stays hidden until it does. Any other is hidden for good, as it has no start left to wait
/// for.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Dismissed {
    pub id: u32,
    pub waiting: bool,
}

/// The dismissals, oldest first.
pub(crate) fn dismissed(store: &Store) -> Vec<Dismissed> {
    store
        .get_meta_text(DISMISSED_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// What the viewer has done, as the rules read it.
pub(crate) struct Viewer<'a> {
    store: &'a Store,
    /// A tracker is signed in, so its list counts.
    pub listed: bool,
    /// Adult titles may be suggested.
    pub adult: bool,
    /// The titles in Continue.
    pub queue: HashSet<AnilistId>,
    /// The titles finished: every episode watched here, or Completed on the list.
    pub finished: HashSet<AnilistId>,
    pub dismissed: Vec<Dismissed>,
}

impl<'a> Viewer<'a> {
    pub fn new(
        store: &'a Store,
        listed: bool,
        adult: bool,
        queue: impl IntoIterator<Item = AnilistId>,
        finished: HashSet<AnilistId>,
        dismissed: Vec<Dismissed>,
    ) -> Self {
        Self { store, listed, adult, queue: queue.into_iter().collect(), finished, dismissed }
    }

    /// The watch history the rules read.
    pub fn store(&self) -> &'a Store {
        self.store
    }

    /// Where the title is on the tracker's list, when signed in.
    pub fn status_of(&self, id: AnilistId) -> Option<String> {
        self.listed.then(|| self.store.library_status(id).ok().flatten()).flatten()
    }

    /// Watched here at all, or on the list as anything but Planning.
    pub fn started(&self, id: AnilistId) -> bool {
        self.store.progress(id).ok().flatten().is_some()
            || self.status_of(id).is_some_and(|s| s != "PLANNING")
    }

    /// Dismissed, and not back: a title dismissed before it aired returns once it airs.
    pub fn hidden(&self, entry: &Entry) -> bool {
        self.dismissed.iter().any(|d| {
            d.id == entry.id.get()
                && (!d.waiting || entry.status == Some(AiringStatus::NotYetReleased))
        })
    }

    /// A title's earlier parts are all finished, whichever way. Unknown counts as unfinished.
    pub fn after_finished(&self, entry: &Entry) -> bool {
        entry.prequels.iter().all(|prequel| self.finished.contains(&prequel.id))
    }

    /// Whether `entry` may be suggested: the rules every shelf shares.
    pub fn eligible(&self, entry: &Entry) -> bool {
        !self.queue.contains(&entry.id)
            && !self.started(entry.id)
            && !self.finished.contains(&entry.id)
            && !self.hidden(entry)
            && (self.adult || !entry.adult)
            && entry.format.as_deref() != Some("MUSIC")
            && entry.status != Some(AiringStatus::Cancelled)
            && self.after_finished(entry)
    }
}

/// The titles finished as far as this device knows without asking anyone: Completed on the
/// list when signed in, and every episode watched here of a title whose length is cached.
pub(crate) fn finished_here(meta: &Meta, store: &Store, listed: bool) -> HashSet<AnilistId> {
    let mut finished: HashSet<AnilistId> = if listed {
        store.library_with_status("COMPLETED").unwrap_or_default().into_iter().collect()
    } else {
        HashSet::new()
    };
    let watched = store.watched_titles(200).unwrap_or_default();
    let ids: Vec<AnilistId> = watched.iter().map(|p| p.anilist_id).collect();
    for media in meta.cached(&ids) {
        let done =
            watched.iter().find(|p| p.anilist_id == media.id).map_or(0, |p| p.episodes_done);
        if media.episodes.is_some_and(|total| total > 0 && done >= total) {
            finished.insert(media.id);
        }
    }
    finished
}
