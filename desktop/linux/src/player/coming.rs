//! The Up next card's timing. The engine hands over the card and its countdown; the app decides
//! when it shows and sends `PlayNext` at zero. The card shows from where the ending starts, or
//! near the end of the file when that is not known, and the countdown runs on the playhead.

/// What the engine said about the next episode, and what has happened to the card since.
#[derive(Debug, Clone, PartialEq)]
pub struct Coming {
    pub auto: bool,
    pub ending_at: Option<f64>,
    pub countdown: u32,
    pub cancelled: bool,
    pub played: bool,
}

/// What the card shows for a playhead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Card {
    Away,
    /// Up next, waiting for Play now or Cancel.
    Waiting,
    /// Up next with seconds still to run, and the fraction of the countdown left.
    Counting { left: f64, fraction: f64 },
}

impl Coming {
    pub fn new(auto: bool, ending_at: Option<f64>, countdown: u32) -> Self {
        Self { auto, ending_at, countdown, cancelled: false, played: false }
    }

    /// Where in the file the card appears, once the file has a length.
    pub fn shows_at(&self, duration: f64) -> Option<f64> {
        if let Some(at) = self.ending_at {
            return Some(at);
        }
        if duration <= 0.0 {
            return None;
        }
        Some(if duration > 120.0 {
            duration - 30.0
        } else {
            let lead = if self.countdown > 0 { f64::from(self.countdown) } else { 10.0 };
            (duration - lead).max(0.0)
        })
    }

    /// The card for the playhead at `position` in a file `duration` long.
    pub fn card(&self, position: f64, duration: f64) -> Card {
        if self.cancelled {
            return Card::Away;
        }
        let Some(start) = self.shows_at(duration) else { return Card::Away };
        if position < start {
            return Card::Away;
        }
        if !self.auto || self.countdown == 0 {
            return Card::Waiting;
        }
        let total = f64::from(self.countdown);
        let left = (total - (position - start)).clamp(0.0, total);
        Card::Counting { left, fraction: left / total }
    }

    /// Whether the app should send `PlayNext` now: the countdown has run out, once.
    pub fn due(&self, position: f64, duration: f64) -> bool {
        !self.played
            && matches!(self.card(position, duration), Card::Counting { left, .. } if left <= 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_waits_for_the_ending_not_the_first_seconds() {
        let coming = Coming::new(true, Some(1300.0), 8);
        assert_eq!(coming.card(7.0, 1420.0), Card::Away);
        assert_eq!(coming.card(1299.0, 1420.0), Card::Away);
        assert!(matches!(coming.card(1300.0, 1420.0), Card::Counting { .. }));
    }

    #[test]
    fn without_an_ending_it_waits_for_the_last_half_minute_of_a_long_file() {
        let coming = Coming::new(true, None, 8);
        assert_eq!(coming.shows_at(1420.0), Some(1390.0));
        assert_eq!(coming.shows_at(60.0), Some(52.0));
        assert_eq!(coming.shows_at(0.0), None);
        assert_eq!(coming.card(22.0, 60.0), Card::Away);
    }

    #[test]
    fn the_countdown_runs_on_the_playhead_and_ends_once() {
        let mut coming = Coming::new(true, Some(50.0), 8);
        assert_eq!(coming.card(54.0, 60.0), Card::Counting { left: 4.0, fraction: 0.5 });
        assert!(!coming.due(57.0, 60.0));
        assert!(coming.due(58.0, 60.0));
        coming.played = true;
        assert!(!coming.due(59.0, 60.0));
    }

    #[test]
    fn cancel_puts_the_card_away_and_no_countdown_only_waits() {
        let mut coming = Coming::new(true, Some(50.0), 8);
        coming.cancelled = true;
        assert_eq!(coming.card(59.0, 60.0), Card::Away);
        assert!(!coming.due(60.0, 60.0));
        let waits = Coming::new(false, Some(50.0), 8);
        assert_eq!(waits.card(59.0, 60.0), Card::Waiting);
        assert!(!waits.due(60.0, 60.0));
    }
}
