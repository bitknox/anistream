//! What the seek bar draws beside the playhead: the openings and endings it marks, and the
//! stretches of the file the player holds. The marks are said whenever they change; what the
//! player holds changes many times a second while it fetches, so it is paced here.

/// The least time between two reports of what the player holds: twice a second at most.
const EVERY: f64 = 0.5;

/// How far a stretch has to move before it is said again, in seconds of the file. A bar a few
/// hundred pixels wide for a twenty-four minute episode moves a pixel every few seconds.
const MOVED: f64 = 1.0;

/// Paces what the player holds to the seek bar: at most twice a second, only once a stretch has
/// moved a second or more, and nothing while paused, when what was held last waits for play
/// to carry on and is said then.
#[derive(Debug, Default)]
pub(super) struct Buffered {
    /// What was said last, and when, in seconds of the session's clock.
    said: Option<(Vec<(f64, f64)>, f64)>,
    /// What came since that is not said yet.
    held: Option<Vec<(f64, f64)>>,
}

impl Buffered {
    /// The player now holds `ranges`, at `now` on the session's clock: what to say, if anything.
    pub(super) fn offer(
        &mut self,
        ranges: Vec<(f64, f64)>,
        now: f64,
        paused: bool,
    ) -> Option<Vec<(f64, f64)>> {
        self.held = Some(ranges);
        self.due(now, paused)
    }

    /// What is held back and now due, at `now`, once the pace allows.
    pub(super) fn due(&mut self, now: f64, paused: bool) -> Option<Vec<(f64, f64)>> {
        if paused { None } else { self.take(now, true) }
    }

    /// Play carried on at `now`: what the player came to hold while paused, said at once, since
    /// the bar has waited on it for as long as the pause lasted.
    pub(super) fn resumed(&mut self, now: f64) -> Option<Vec<(f64, f64)>> {
        self.take(now, false)
    }

    fn take(&mut self, now: f64, paced: bool) -> Option<Vec<(f64, f64)>> {
        let ranges = self.held.as_ref()?;
        match &self.said {
            Some((said, _)) if !moved(said, ranges) => {
                self.held = None;
                None
            }
            Some((_, at)) if paced && now - at < EVERY => None,
            _ => {
                let ranges = self.held.take()?;
                self.said = Some((ranges.clone(), now));
                Some(ranges)
            }
        }
    }
}

/// Whether `now` differs from `said` by enough to draw: a stretch more or fewer, or one whose
/// start or end moved a second or more.
fn moved(said: &[(f64, f64)], now: &[(f64, f64)]) -> bool {
    said.len() != now.len()
        || said
            .iter()
            .zip(now)
            .any(|(a, b)| (a.0 - b.0).abs() >= MOVED || (a.1 - b.1).abs() >= MOVED)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_report_is_said_and_a_small_move_is_not() {
        let mut gate = Buffered::default();
        assert_eq!(gate.offer(vec![(0.0, 30.0)], 0.0, false), Some(vec![(0.0, 30.0)]));
        assert_eq!(gate.offer(vec![(0.0, 30.4)], 2.0, false), None, "under a second");
        assert_eq!(gate.offer(vec![(0.0, 31.0)], 3.0, false), Some(vec![(0.0, 31.0)]));
    }

    #[test]
    fn reports_are_said_twice_a_second_at_most_and_the_last_is_not_lost() {
        let mut gate = Buffered::default();
        gate.offer(vec![(0.0, 10.0)], 0.0, false);
        assert_eq!(gate.offer(vec![(0.0, 20.0)], 0.2, false), None, "too soon");
        assert_eq!(gate.offer(vec![(0.0, 25.0)], 0.3, false), None);
        assert_eq!(gate.due(0.4, false), None);
        assert_eq!(gate.due(0.5, false), Some(vec![(0.0, 25.0)]), "the latest, once due");
        assert_eq!(gate.due(1.5, false), None, "said once");
    }

    #[test]
    fn nothing_is_said_while_paused_and_the_latest_once_play_carries_on() {
        let mut gate = Buffered::default();
        gate.offer(vec![(0.0, 10.0)], 0.0, false);
        assert_eq!(gate.offer(vec![(0.0, 40.0)], 5.0, true), None);
        assert_eq!(gate.offer(vec![(0.0, 90.0)], 9.0, true), None);
        assert_eq!(gate.due(12.0, true), None);
        assert_eq!(gate.resumed(12.1), Some(vec![(0.0, 90.0)]));
        // Even a pause too short for the pace.
        assert_eq!(gate.offer(vec![(0.0, 99.0)], 12.2, true), None);
        assert_eq!(gate.resumed(12.3), Some(vec![(0.0, 99.0)]));
        assert_eq!(gate.resumed(12.4), None, "nothing new");
    }

    #[test]
    fn a_seek_past_what_is_held_is_a_new_stretch_said_at_once() {
        let mut gate = Buffered::default();
        gate.offer(vec![(0.0, 60.0)], 0.0, false);
        let jumped = vec![(0.0, 60.0), (600.0, 600.5)];
        assert_eq!(gate.offer(jumped.clone(), 1.0, false), Some(jumped));
        // A seek that dropped what was held, the same.
        assert_eq!(gate.offer(Vec::new(), 2.0, false), Some(Vec::new()));
    }
}
