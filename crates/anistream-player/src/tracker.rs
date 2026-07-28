//! Playback policy: when to record, when to commit, when to offer a skip.
//!
//! Deliberately pure. It takes events and returns [`Action`]s for the caller to perform,
//! touching no database and no socket, so every rule here is exhaustively testable — and these
//! rules decide what gets written to your history and pushed to a tracker, which is not
//! somewhere to be guessing.
//!
//! The throttling matters more than it looks: mpv reports position about thirty times a second
//! (measured against a live stream), so writing a history row per report would mean tens of
//! thousands of rows per episode.

use crate::{
    mpv::PlaybackEvent,
    skip::{SkipInterval, SkipKind, active},
};

/// How far the playhead must move before a new history row is worth writing.
///
/// Ten seconds bounds the loss from a crash to something nobody would notice while keeping
/// roughly a hundred rows per episode instead of fifteen hundred.
pub const RECORD_INTERVAL_SECS: f64 = 10.0;

/// Something the caller should do.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Append a history row.
    Record { position: f64, duration: Option<f64>, completed: bool },
    /// Offer to skip a segment. The caller shows this on mpv's OSD.
    OfferSkip { kind: SkipKind, to: f64 },
    /// Withdraw the offer — the segment has passed.
    ClearSkip,
    /// The episode is finished. Push progress and consider the next one.
    Finished { watched: bool },
    /// Remember the chosen speed for the next episode.
    RememberSpeed(f64),
    /// Remember the chosen volume for the next session.
    RememberVolume(f64),
}

/// Tracks one episode's playback.
#[derive(Debug, Clone)]
pub struct PlaybackTracker {
    /// Fraction of runtime after which the episode counts as watched.
    threshold: f64,
    skips: Vec<SkipInterval>,
    auto_skip: bool,

    position: f64,
    duration: Option<f64>,
    last_recorded: f64,
    /// The `completed` flag of the last row written, so a transition always records even when
    /// the playhead has not moved.
    last_recorded_completed: bool,
    /// Seconds of actual playback, as distinct from position — a single seek to the end
    /// should not look like having watched the episode.
    watched_secs: f64,
    paused: bool,
    speed: f64,
    volume: Option<f64>,
    committed: bool,
    offering: Option<SkipKind>,
    ended: bool,
}

impl PlaybackTracker {
    pub fn new(threshold: f64, skips: Vec<SkipInterval>, auto_skip: bool) -> Self {
        Self {
            threshold: threshold.clamp(0.05, 1.0),
            skips,
            auto_skip,
            position: 0.0,
            duration: None,
            last_recorded: 0.0,
            last_recorded_completed: false,
            watched_secs: 0.0,
            paused: false,
            speed: 1.0,
            volume: None,
            committed: false,
            offering: None,
            ended: false,
        }
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn duration(&self) -> Option<f64> {
        self.duration
    }

    pub fn watched_secs(&self) -> f64 {
        self.watched_secs
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Fraction of the episode reached.
    pub fn fraction(&self) -> f64 {
        match self.duration {
            Some(d) if d > 0.0 => (self.position / d).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// Whether the episode has passed the commit threshold.
    pub fn is_watched(&self) -> bool {
        self.duration.is_some_and(|d| d > 0.0) && self.fraction() >= self.threshold
    }

    /// The segment currently offering a skip, for the UI.
    pub fn offered_skip(&self) -> Option<&SkipInterval> {
        active(&self.skips, self.position)
    }

    /// Feed an event, and get back what to do.
    pub fn observe(&mut self, event: &PlaybackEvent) -> Vec<Action> {
        let mut actions = Vec::new();

        match event {
            PlaybackEvent::Progress { position, duration } => {
                if let Some(d) = duration {
                    self.duration = Some(*d);
                }

                // Accumulate real watch time only for forward movement of a plausible size.
                // A seek must not count, or skipping to the end would look like viewing.
                let delta = position - self.position;
                if delta > 0.0 && delta < 5.0 && !self.paused {
                    self.watched_secs += delta;
                }
                self.position = *position;

                // Crossing the threshold is recorded immediately rather than waiting for the
                // next throttle window — this is the row that marks the episode watched.
                if !self.committed && self.is_watched() {
                    self.committed = true;
                    actions.extend(self.record());
                } else if (self.position - self.last_recorded).abs() >= RECORD_INTERVAL_SECS {
                    actions.extend(self.record());
                }

                // Skip prompt, driven off position.
                match active(&self.skips, self.position) {
                    Some(interval) => {
                        // Emitted once per entry into the interval, whether or not auto-skip is
                        // on: the action is the same either way, and *acting* on it — seeking
                        // versus prompting — is the caller's decision.
                        if self.offering != Some(interval.kind) {
                            self.offering = Some(interval.kind);
                            actions.push(Action::OfferSkip {
                                kind: interval.kind,
                                to: interval.end,
                            });
                        }
                    }
                    None => {
                        if self.offering.take().is_some() {
                            actions.push(Action::ClearSkip);
                        }
                    }
                }
            }

            PlaybackEvent::Paused(paused) => {
                self.paused = *paused;
                // Pausing is a natural point to persist: it is often followed by quitting.
                if *paused {
                    actions.extend(self.record());
                }
            }

            PlaybackEvent::Speed(speed) => {
                if (self.speed - *speed).abs() > f64::EPSILON {
                    self.speed = *speed;
                    actions.push(Action::RememberSpeed(*speed));
                }
            }

            // Remote controls are the session's business, not the tracker's.
            PlaybackEvent::Remote(_) => {}

            PlaybackEvent::Chapters(chapters) => {
                // The file's own chapters outrank aniskip: they were authored against this
                // exact encode, where community times were taken against someone else's.
                let from_file = crate::skip::from_chapters(chapters);
                if !from_file.is_empty() {
                    self.skips = from_file;
                }
            }

            PlaybackEvent::Volume(volume) => {
                // The first report is mpv telling us the volume we started it at, not a
                // choice — remembering it would just re-save the config's own value.
                let changed = self.volume.is_some_and(|v| (v - *volume).abs() > f64::EPSILON);
                if changed {
                    actions.push(Action::RememberVolume(*volume));
                }
                self.volume = Some(*volume);
            }

            PlaybackEvent::Ended { complete } => {
                if self.ended {
                    // mpv can report an ending twice — an `eof-reached` property change and
                    // then `end-file`. Acting twice would double-write history.
                    return actions;
                }
                self.ended = true;

                // Reaching the end of the file counts as watched regardless of where the
                // position last landed: mpv may stop reporting a little short.
                let watched = *complete || self.is_watched();
                self.committed = watched;
                actions.extend(self.record());
                actions.push(Action::Finished { watched });
            }
        }

        actions
    }

    /// Emit a history row, unless there is nothing new to say.
    ///
    /// Every record goes through here, because the natural persistence points overlap: pausing
    /// then stopping at the same position used to write three identical rows. Duplicates are not
    /// merely wasteful — `watched_secs` is cumulative for the session, so three rows each
    /// reporting 7 seconds make a naive `SUM(watched_secs)` claim 21.
    fn record(&mut self) -> Option<Action> {
        let unchanged = (self.position - self.last_recorded).abs() < f64::EPSILON
            && self.committed == self.last_recorded_completed;
        if unchanged {
            return None;
        }
        self.last_recorded = self.position;
        self.last_recorded_completed = self.committed;
        Some(Action::Record {
            position: self.position,
            duration: self.duration,
            completed: self.committed,
        })
    }

    /// Whether auto-skip is on, so the caller can seek rather than merely prompt.
    pub fn auto_skips(&self) -> bool {
        self.auto_skip
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skips() -> Vec<SkipInterval> {
        vec![
            SkipInterval { kind: SkipKind::Opening, start: 3.0, end: 93.0 },
            SkipInterval { kind: SkipKind::Ending, start: 1400.0, end: 1490.0 },
        ]
    }

    fn tracker() -> PlaybackTracker {
        PlaybackTracker::new(0.85, skips(), false)
    }

    fn progress(position: f64) -> PlaybackEvent {
        PlaybackEvent::Progress { position, duration: Some(1500.0) }
    }

    fn records(actions: &[Action]) -> Vec<&Action> {
        actions.iter().filter(|a| matches!(a, Action::Record { .. })).collect()
    }

    #[test]
    fn position_updates_are_throttled_rather_than_written_every_second() {
        // mpv reports about once a second; writing each one would be ~1500 rows an episode.
        let mut t = tracker();
        let mut written = 0;
        for second in 1..=60 {
            written += records(&t.observe(&progress(f64::from(second)))).len();
        }
        assert!(written <= 7, "expected ~6 rows for 60s, got {written}");
        assert!(written >= 5, "should still be recording periodically, got {written}");
    }

    #[test]
    fn crossing_the_threshold_is_recorded_immediately() {
        // This is the row that marks the episode watched; waiting for the throttle window
        // could lose it if the viewer quits straight after.
        let mut t = tracker();
        t.observe(&progress(1000.0));
        assert!(!t.is_watched());

        // 85% of 1500 is 1275.
        let actions = t.observe(&progress(1275.0));
        let recorded = records(&actions);
        assert_eq!(recorded.len(), 1);
        assert!(matches!(recorded[0], Action::Record { completed: true, .. }));
        assert!(t.is_watched());
    }

    #[test]
    fn the_threshold_is_not_crossed_early() {
        let mut t = tracker();
        t.observe(&progress(1274.0));
        assert!(!t.is_watched(), "84.9% must not count as watched");
    }

    #[test]
    fn a_seek_to_the_end_does_not_count_as_having_watched() {
        // Watch time accumulates only from plausible forward movement; otherwise skipping to
        // the credits would look like viewing the episode.
        let mut t = tracker();
        t.observe(&progress(1.0));
        t.observe(&progress(1450.0));
        assert!(t.watched_secs() < 5.0, "a jump counted as watch time: {}", t.watched_secs());

        // Real playback does accumulate.
        let mut real = tracker();
        for second in 1..=30 {
            real.observe(&progress(f64::from(second)));
        }
        assert!(real.watched_secs() >= 25.0);
    }

    #[test]
    fn a_paused_period_does_not_accumulate_watch_time() {
        let mut t = tracker();
        t.observe(&PlaybackEvent::Paused(true));
        let before = t.watched_secs();
        for second in 1..=10 {
            t.observe(&progress(f64::from(second)));
        }
        assert_eq!(t.watched_secs(), before, "paused time was counted as watched");
    }

    #[test]
    fn pausing_persists_immediately_because_quitting_often_follows() {
        let mut t = tracker();
        t.observe(&progress(600.0));
        // Inside the throttle window, so this position is not in history yet — which is
        // exactly what makes pausing worth persisting.
        t.observe(&progress(604.0));
        let actions = t.observe(&PlaybackEvent::Paused(true));
        assert_eq!(records(&actions).len(), 1);
        assert!(matches!(records(&actions)[0], Action::Record { position: 604.0, .. }));
        assert!(t.is_paused());
    }

    #[test]
    fn the_same_position_is_never_written_twice() {
        // Found by the live probe: pausing and then stopping at the same place wrote three
        // identical rows. `watched_secs` is cumulative for the session, so duplicates make a
        // naive `SUM(watched_secs)` over-report by a factor of however many there were.
        let mut t = tracker();
        t.observe(&progress(37.0));

        assert!(records(&t.observe(&PlaybackEvent::Paused(true))).is_empty());
        assert!(records(&t.observe(&PlaybackEvent::Paused(false))).is_empty());
        let ending = t.observe(&PlaybackEvent::Ended { complete: false });
        assert!(records(&ending).is_empty(), "stopping rewrote an identical row");
        assert!(
            ending.contains(&Action::Finished { watched: false }),
            "suppressing the row must not suppress the ending itself"
        );
    }

    #[test]
    fn crossing_the_threshold_records_even_without_moving() {
        // The `completed` flag is the one thing worth writing at an unchanged position: it is
        // what marks the episode watched, and a tracker push depends on it.
        let mut t = tracker();
        t.observe(&progress(1200.0));
        assert!(!t.is_watched());

        // `complete` from mpv commits at the same position the last row already holds.
        let actions = t.observe(&PlaybackEvent::Ended { complete: true });
        assert_eq!(records(&actions).len(), 1, "the completion row was suppressed");
        assert!(matches!(records(&actions)[0], Action::Record { completed: true, .. }));
    }

    #[test]
    fn reaching_the_end_of_the_file_counts_as_watched_even_if_position_fell_short() {
        // mpv sometimes stops reporting a little before the true end.
        let mut t = tracker();
        t.observe(&progress(1200.0));
        assert!(!t.is_watched());

        let actions = t.observe(&PlaybackEvent::Ended { complete: true });
        assert!(actions.contains(&Action::Finished { watched: true }));
        assert!(matches!(records(&actions)[0], Action::Record { completed: true, .. }));
    }

    #[test]
    fn quitting_early_is_not_recorded_as_watched() {
        let mut t = tracker();
        t.observe(&progress(120.0));
        // Past the last recorded row, so quitting here has a position worth keeping.
        t.observe(&progress(124.0));
        let actions = t.observe(&PlaybackEvent::Ended { complete: false });
        assert!(actions.contains(&Action::Finished { watched: false }));
        assert!(matches!(
            records(&actions)[0],
            Action::Record { position: 124.0, completed: false, .. }
        ));
    }

    #[test]
    fn quitting_after_the_threshold_still_counts_as_watched() {
        // You watched it; closing the window before the credits should not undo that.
        let mut t = tracker();
        t.observe(&progress(1400.0));
        let actions = t.observe(&PlaybackEvent::Ended { complete: false });
        assert!(actions.contains(&Action::Finished { watched: true }));
    }

    #[test]
    fn a_duplicate_ending_does_not_double_write_history() {
        // mpv can report both an `eof-reached` property change and an `end-file` event.
        let mut t = tracker();
        t.observe(&progress(1490.0));
        let first = t.observe(&PlaybackEvent::Ended { complete: true });
        let second = t.observe(&PlaybackEvent::Ended { complete: true });
        assert!(!first.is_empty());
        assert!(second.is_empty(), "second ending produced {second:?}");
    }

    #[test]
    fn the_skip_prompt_appears_once_per_segment_and_is_withdrawn_after() {
        let mut t = tracker();

        // Before the opening: nothing.
        assert!(
            t.observe(&progress(1.0)).iter().all(|a| !matches!(a, Action::OfferSkip { .. }))
        );

        // Entering it offers once.
        let entering = t.observe(&progress(10.0));
        assert!(entering.contains(&Action::OfferSkip { kind: SkipKind::Opening, to: 93.0 }));

        // Still inside: no repeat, or the OSD would flash every second.
        let inside = t.observe(&progress(20.0));
        assert!(!inside.iter().any(|a| matches!(a, Action::OfferSkip { .. })));

        // Leaving withdraws it.
        assert!(t.observe(&progress(100.0)).contains(&Action::ClearSkip));
    }

    #[test]
    fn the_ending_segment_offers_separately_from_the_opening() {
        let mut t = tracker();
        t.observe(&progress(10.0));
        t.observe(&progress(200.0));
        let actions = t.observe(&progress(1450.0));
        assert!(actions.contains(&Action::OfferSkip { kind: SkipKind::Ending, to: 1490.0 }));
    }

    #[test]
    fn a_title_with_no_skip_data_never_offers() {
        let mut t = PlaybackTracker::new(0.85, Vec::new(), false);
        for position in [5.0, 50.0, 1450.0] {
            let actions = t.observe(&progress(position));
            assert!(!actions.iter().any(|a| matches!(a, Action::OfferSkip { .. })));
        }
    }

    #[test]
    fn a_speed_change_is_remembered_once() {
        let mut t = tracker();
        assert!(t.observe(&PlaybackEvent::Speed(1.5)).contains(&Action::RememberSpeed(1.5)));
        // The same speed again is not a change.
        assert!(t.observe(&PlaybackEvent::Speed(1.5)).is_empty());
        assert_eq!(t.speed(), 1.5);
    }

    #[test]
    fn an_unknown_duration_never_marks_anything_watched() {
        // Without a runtime there is no way to judge, and a wrongly-completed episode gets
        // pushed to a tracker and cannot be taken back.
        let mut t = PlaybackTracker::new(0.85, Vec::new(), false);
        t.observe(&PlaybackEvent::Progress { position: 99_999.0, duration: None });
        assert!(!t.is_watched());
        assert_eq!(t.fraction(), 0.0);
    }

    #[test]
    fn the_threshold_is_clamped_to_something_sane() {
        // A zero threshold would mark every episode watched on open.
        let zero = PlaybackTracker::new(0.0, Vec::new(), false);
        assert!(zero.threshold >= 0.05);
        let over = PlaybackTracker::new(5.0, Vec::new(), false);
        assert!(over.threshold <= 1.0);
    }

    #[test]
    fn fraction_is_bounded_even_past_the_reported_duration() {
        let mut t = tracker();
        t.observe(&PlaybackEvent::Progress { position: 9_999.0, duration: Some(1500.0) });
        assert_eq!(t.fraction(), 1.0);
    }
}
