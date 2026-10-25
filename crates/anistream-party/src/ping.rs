//! How long a message takes to arrive: the round trip measured on every state the server sends,
//! smoothed, and the one-way delay a position from the room is moved on by.
//!
//! Every server state carries back the timestamp this client sent with its last reply, so the
//! round trip is the time since then. It is smoothed with a moving average, as one slow packet
//! should not move everyone's playhead. The delay a message spent on its way here is taken as
//! half the smoothed round trip, plus whatever this round trip ran longer than the server's own,
//! since the extra time was spent on this side.

/// How much of the average each new round trip replaces.
const WEIGHT: f64 = 0.85;

#[derive(Debug, Clone, Default)]
pub struct Latency {
    average: Option<f64>,
    last: f64,
    forward: f64,
}

impl Latency {
    /// Take in a server state that echoed `sent_at`, this client's clock when it sent its last
    /// reply, with the server's own round trip, at `now` by the same clock.
    pub fn receive(&mut self, sent_at: f64, server_rtt: f64, now: f64) {
        let rtt = now - sent_at;
        // A clock that moved backwards, or an echo of nothing.
        if !rtt.is_finite() || rtt < 0.0 || !server_rtt.is_finite() || server_rtt < 0.0 {
            return;
        }
        let average = match self.average {
            Some(average) => average * WEIGHT + rtt * (1.0 - WEIGHT),
            None => rtt,
        };
        self.average = Some(average);
        self.last = rtt;
        self.forward =
            if server_rtt < rtt { average / 2.0 + (rtt - server_rtt) } else { average / 2.0 };
    }

    /// The last round trip, in seconds, which each reply tells the server.
    pub fn rtt(&self) -> f64 {
        self.last
    }

    /// How long a message from the server spends on its way, in seconds.
    pub fn forward_delay(&self) -> f64 {
        self.forward
    }

    /// The smoothed round trip in whole milliseconds, for a front end to show; `None` before the
    /// first.
    pub fn millis(&self) -> Option<u32> {
        self.average.map(|a| (a * 1000.0).round().clamp(0.0, u32::MAX as f64) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_round_trip_is_taken_as_it_is() {
        let mut latency = Latency::default();
        assert_eq!(latency.millis(), None);
        latency.receive(10.0, 0.1, 10.1);
        assert!((latency.rtt() - 0.1).abs() < 1e-9);
        assert!((latency.forward_delay() - 0.05).abs() < 1e-9);
        assert_eq!(latency.millis(), Some(100));
    }

    #[test]
    fn one_slow_round_trip_moves_the_average_only_a_little() {
        let mut latency = Latency::default();
        latency.receive(0.0, 0.1, 0.1);
        latency.receive(1.0, 0.1, 2.1);
        // 0.1 * 0.85 + 1.1 * 0.15
        assert_eq!(latency.millis(), Some(250));
        assert!((latency.rtt() - 1.1).abs() < 1e-9);
    }

    #[test]
    fn time_spent_on_this_side_adds_to_the_delay() {
        let mut latency = Latency::default();
        latency.receive(0.0, 0.1, 0.3);
        // Half the average, plus the 0.2 seconds this trip ran longer than the server's.
        assert!((latency.forward_delay() - (0.15 + 0.2)).abs() < 1e-9);
    }

    #[test]
    fn a_clock_that_ran_backwards_is_ignored() {
        let mut latency = Latency::default();
        latency.receive(5.0, 0.1, 4.0);
        assert_eq!(latency.millis(), None);
        latency.receive(5.0, -1.0, 6.0);
        assert_eq!(latency.millis(), None);
    }
}
