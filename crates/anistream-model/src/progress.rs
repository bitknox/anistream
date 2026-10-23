//! A title's progress: the rules a watch, a mark and the next episode are worked out by.
//!
//! Pure, over the rows and entries they read, so that whoever holds a title's episode table
//! and its entry applies them alike.

use crate::{
    catalog::{Entry, ResumePoint},
    episodes::EpisodeRow,
};

/// How far through a title the viewer is: how many episodes are done, and the one Continue
/// offers next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TitleProgress {
    pub done: u32,
    pub next: u32,
}

/// How far through one episode the viewer is, as its row in the episode table shows it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EpisodeProgress {
    pub number: String,
    /// The meter: how far through it, from nought to one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched: f64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub completed: bool,
}

/// A playback's word on one episode: how far it got, of how long, and whether it counts as
/// watched. Every reflection of a watch is worked out from this, and a mark is one too: a
/// finished watch with no position.
#[derive(Debug, Clone, PartialEq)]
pub struct Watch {
    pub episode: String,
    pub position: f64,
    pub duration: Option<f64>,
    pub completed: bool,
}

impl Watch {
    /// How far the episode's meter goes.
    ///
    /// `completed` is authoritative, and the position is not: mpv's last reported `time-pos`
    /// before end-of-file is routinely a second or two short of the runtime, and a meter stuck
    /// at 98% on a finished episode is exactly the "did that register?" doubt this fixes.
    pub fn fraction(&self) -> f64 {
        if self.completed {
            1.0
        } else {
            self.duration
                .filter(|d| *d > 0.0)
                .map(|d| (self.position / d).clamp(0.0, 1.0))
                .unwrap_or(0.0)
        }
    }
}

/// A watch on the episode's row in a list of a title's rows, when the list has it.
///
/// Monotonic: a re-watch that was quit early must not erase that it was once finished.
pub fn watch_row(rows: &mut [EpisodeRow], watch: &Watch) {
    if let Some(row) = rows.iter_mut().find(|r| r.number == watch.episode) {
        row.watched = row.watched.max(watch.fraction());
        row.completed = row.completed || watch.completed;
    }
}

/// A watch on a title's entry: where Continue carries on, and how many episodes are done.
///
/// `rows` is the title's episode table, read before the watch lands on it, or `None` when the
/// table on hand is another title's or there is none. Continue is worked out as the store works
/// it out (see the engine's `data::entry_from`): the last episode touched when it is
/// unfinished, and otherwise the first one after it not yet watched.
pub fn watch_entry(entry: &mut Entry, rows: Option<&[EpisodeRow]>, watch: &Watch) {
    let episode = watch.episode.as_str();
    let (rows_apply, rows) = (rows.is_some(), rows.unwrap_or_default());
    let row = |number: &str| rows.iter().find(|r| r.number == number);
    // A rewatch, which must not move the count: the store counts distinct episodes.
    let was_done = row(episode).is_some_and(|r| r.completed);
    // An episode the table knows and has never seen finished, which Continue moves to.
    let unfinished = row(episode).is_some_and(|r| !r.completed);
    let after = episode.trim().parse::<u32>().ok().map(|n| {
        let mut next = n.saturating_add(1);
        // Bounded: an all-watched tail must still terminate.
        while next - n < 1_000 && row(&next.to_string()).is_some_and(|r| r.completed) {
            next += 1;
        }
        // Past the table's last episode, the first one in it skipped.
        let numbers =
            || rows.iter().filter_map(|r| Some((r.number.trim().parse::<u32>().ok()?, r)));
        if numbers().map(|(n, _)| n).max().is_some_and(|last| next > last)
            && let Some((skipped, _)) = numbers().find(|(_, r)| !r.completed)
        {
            next = skipped;
        }
        next
    });

    let next_up = entry.progress.map_or(1, |p| p.next).to_string();
    if !watch.completed {
        // A part-watched episode moves the resume point, so the Continue rail is current the
        // moment mpv exits rather than at the next reload from the store — and it is where
        // Continue now carries on, unless it is one already watched.
        if watch.position > 0.0
            && (episode == next_up || unfinished)
            && let Ok(number) = episode.trim().parse::<u32>()
        {
            entry.progress = Some(TitleProgress {
                done: entry.progress.map_or(0, |p| p.done),
                next: number,
            });
            entry.resume = Some(ResumePoint {
                position: watch.position,
                fraction: watch
                    .duration
                    .filter(|d| *d > 0.0)
                    .map(|d| (watch.position / d).clamp(0.0, 1.0)),
            });
        }
        return;
    }

    // Progress counts *completed* episodes, mirroring the store's distinct count: a first
    // watch moves it — including the very first episode of a show with no history, and an
    // out-of-order catch-up the table can vouch for — where a rewatch leaves it alone. Continue
    // moves past the episode just finished either way.
    let first = !was_done && (entry.progress.is_none() || rows_apply || episode == next_up);
    let done = entry.progress.map_or(0, |p| p.done) + u32::from(first);
    let next = after.or(entry.progress.map(|p| p.next)).unwrap_or(done + 1);
    entry.progress = Some(TitleProgress { done, next });
    // Where the previous Continue episode was left is no longer what Continue offers.
    if next.to_string() != next_up {
        entry.resume = None;
    }
}

/// One episode marked watched or not on its row, with the meter full or empty to match.
pub fn mark_row(rows: &mut [EpisodeRow], number: &str, watched: bool) {
    if let Some(row) = rows.iter_mut().find(|r| r.number == number) {
        row.completed = watched;
        row.watched = if watched { 1.0 } else { 0.0 };
    }
}

/// An episode that was watched, marked not watched, on a title's entry: the count goes down
/// one, and Continue goes back to it when it was the episode Continue had just moved past.
///
/// As the store re-derives Continue from what survives the unmark (`data::continue_from`):
/// past the table's end it falls back to the first episode not watched. `rows` is the title's
/// table with the episode already unmarked.
pub fn unmark_entry(entry: &mut Entry, rows: &[EpisodeRow], number: &str) {
    let Some(TitleProgress { done, next }) = entry.progress else { return };
    let unmarked = number.trim().parse::<u32>().ok();
    let numbered =
        || rows.iter().filter_map(|r| Some((r.number.trim().parse::<u32>().ok()?, r)));
    let last = numbered().map(|(n, _)| n).max();
    let gap = numbered().find(|(_, r)| !r.completed).map(|(n, _)| n);
    let moved = match unmarked {
        Some(n) if n.saturating_add(1) == next => n,
        _ if last.is_some_and(|last| next > last) => gap.unwrap_or(next),
        _ => next,
    };
    if moved != next {
        entry.resume = None;
    }
    entry.progress = Some(TitleProgress { done: done.saturating_sub(1), next: moved });
}

/// Where auto-next actually lands from `next`, honouring `skip_filler`.
///
/// Steps over episodes marked pure filler (`mixed` is never skipped — it carries story). Only
/// rows present in the title's episode table can be judged; an unknown episode is played
/// rather than guessed about, and another show's table is never asked.
pub fn after_filler(rows: &[EpisodeRow], mut next: String, skip_filler: bool) -> String {
    if !skip_filler {
        return next;
    }
    let mut steps = 0;
    while let Some(row) = rows.iter().find(|r| r.number == next) {
        if !row.skippable {
            break;
        }
        let Ok(n) = next.trim().parse::<i64>() else { break };
        next = (n + 1).to_string();
        // An all-filler tail must still terminate.
        steps += 1;
        if steps > 100 {
            break;
        }
    }
    next
}

/// The episode that follows `episode` in a title's episode table, honouring the filler
/// setting, when the table can vouch that it exists.
///
/// `None` when the table has no such row: the player then shows no Up Next card and closes at
/// the end, which is right for a finale and harmless otherwise.
pub fn next_up<'a>(
    rows: &'a [EpisodeRow],
    episode: &str,
    skip_filler: bool,
) -> Option<&'a EpisodeRow> {
    let next = episode.trim().parse::<u32>().ok()?.checked_add(1)?.to_string();
    let next = after_filler(rows, next, skip_filler);
    rows.iter().find(|r| r.number == next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(last: u32) -> Vec<EpisodeRow> {
        (1..=last)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: Some(format!("Episode {n}")),
                thumbnail: None,
                description: None,
                duration_secs: Some(1435),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
            })
            .collect()
    }

    /// Moved from the App, whose table it asked: "OVA" and "Special" are real labels with no
    /// successor, and guessing one would start playing something unrelated.
    #[test]
    fn a_non_numeric_episode_has_no_next() {
        let rows = rows(12);
        assert!(next_up(&rows, "OVA", false).is_none());
        assert_eq!(next_up(&rows, "11", false).map(|r| r.number.as_str()), Some("12"));
        assert!(next_up(&rows, "12", false).is_none(), "the table ends at 12");
    }
}
