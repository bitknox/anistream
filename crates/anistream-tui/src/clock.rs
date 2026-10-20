//! The time the interface reads, as a value it holds rather than a call it makes.
//!
//! Every date, clock time and running time on screen comes from here: the calendar's day
//! rulings, a pull that failed "at 14:32", a source "skipped until" a time, an invite that
//! "expires tomorrow", how long a job has been running. Production reads the system on every
//! ask, exactly as those call sites once did themselves.
//!
//! Tests stop the clock and pick the zone. The zone is the reason this is a type and not just
//! an epoch passed around: it is the process's `TZ`, and with edition 2024 and `unsafe_code`
//! forbidden a test cannot set it. A stopped clock in a fixed zone is what lets a rendered
//! screen be compared byte for byte, whenever and wherever the test runs.

use std::time::{Duration, Instant};

use chrono::{DateTime, FixedOffset, Local, NaiveDate, TimeZone, Utc};

/// The zone dates and clock times are read in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// The system's, as [`chrono::Local`] reads it: what the app runs in.
    Local,
    /// A fixed offset from UTC, with no daylight saving to move it: a test's zone.
    Fixed(FixedOffset),
}

impl Zone {
    /// UTC itself.
    pub fn utc() -> Self {
        Self::Fixed(FixedOffset::east_opt(0).expect("a zero offset is in range"))
    }

    /// `hours` east of UTC, negative for west. `None` past a day either way.
    pub fn hours_east(hours: i32) -> Option<Self> {
        FixedOffset::east_opt(hours.checked_mul(3_600)?).map(Self::Fixed)
    }
}

/// Now, as wall-clock seconds and as a monotonic instant, and the zone to read dates in.
///
/// `Copy` and cheap: the renderer reads it many times a frame, and a test that needs a later
/// moment moves its own copy with [`Clock::advance`].
#[derive(Debug, Clone, Copy)]
pub struct Clock {
    now: Now,
    zone: Zone,
}

#[derive(Debug, Clone, Copy)]
enum Now {
    /// Read from the system on every ask.
    System,
    /// One moment that holds until it is moved. Both halves move together, so wall-clock text
    /// and running times never disagree about how much time has passed.
    Fixed { at: DateTime<Utc>, instant: Instant },
}

impl Clock {
    /// The system's time in the system's zone: what the app runs on.
    pub fn system() -> Self {
        Self { now: Now::System, zone: Zone::Local }
    }

    /// A clock stopped at `epoch` seconds, reading dates in `zone`.
    ///
    /// The instant standing for that moment is taken now. An `Instant` has no origin that can
    /// be named, and only spans between instants ever reach the screen, so a base fixed once is
    /// all a deterministic frame needs.
    ///
    /// # Panics
    ///
    /// On an epoch outside what a date can hold, roughly ±262,000 years: a test's mistake.
    pub fn fixed(epoch: i64, zone: Zone) -> Self {
        let at =
            DateTime::from_timestamp(epoch, 0).expect("a fixed clock's epoch is a real date");
        Self { now: Now::Fixed { at, instant: Instant::now() }, zone }
    }

    /// Move a stopped clock on by `by`, its epoch and its instant together. A system clock
    /// moves by itself and is left as it is.
    pub fn advance(&mut self, by: Duration) {
        if let Now::Fixed { at, instant } = &mut self.now {
            *at += by;
            *instant += by;
        }
    }

    /// Now, in seconds since the Unix epoch.
    pub fn now_epoch(&self) -> i64 {
        match self.now {
            Now::System => Utc::now().timestamp(),
            Now::Fixed { at, .. } => at.timestamp(),
        }
    }

    /// Now, as an instant to measure running times against.
    pub fn now_instant(&self) -> Instant {
        match self.now {
            Now::System => Instant::now(),
            Now::Fixed { instant, .. } => instant,
        }
    }

    /// The zone this clock reads dates in.
    pub fn zone(&self) -> Zone {
        self.zone
    }

    /// The moment `epoch` names, in this clock's zone: for formatting a clock time or a date.
    /// `None` only for an epoch outside what a date can hold.
    pub fn local(&self, epoch: i64) -> Option<DateTime<FixedOffset>> {
        match self.zone {
            Zone::Local => Local.timestamp_opt(epoch, 0).single().map(|t| t.fixed_offset()),
            Zone::Fixed(offset) => offset.timestamp_opt(epoch, 0).single(),
        }
    }

    /// The calendar date `epoch` falls on in this clock's zone.
    pub fn date_of(&self, epoch: i64) -> Option<NaiveDate> {
        self.local(epoch).map(|t| t.date_naive())
    }

    /// Today's date in this clock's zone.
    pub fn today(&self) -> NaiveDate {
        self.date_of(self.now_epoch()).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-19 23:30 UTC: late enough in the day that an hour east is already tomorrow.
    const LATE_MONDAY: i64 = 1_792_452_600;

    #[test]
    fn a_fixed_clock_holds_still_until_it_is_moved() {
        let mut clock = Clock::fixed(LATE_MONDAY, Zone::utc());
        let started = clock.now_instant();
        assert_eq!(clock.now_epoch(), LATE_MONDAY);
        assert_eq!(clock.now_instant(), started);

        clock.advance(Duration::from_millis(90_500));
        assert_eq!(clock.now_epoch(), LATE_MONDAY + 90, "whole seconds on the wall clock");
        assert_eq!(clock.now_instant() - started, Duration::from_millis(90_500));
    }

    #[test]
    fn the_zone_decides_the_date_and_the_clock_time() {
        let utc = Clock::fixed(LATE_MONDAY, Zone::utc());
        let east = Clock::fixed(LATE_MONDAY, Zone::hours_east(1).unwrap());
        let west = Clock::fixed(LATE_MONDAY, Zone::hours_east(-8).unwrap());

        assert_eq!(utc.today(), NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        assert_eq!(east.today(), NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
        assert_eq!(west.today(), NaiveDate::from_ymd_opt(2026, 10, 19).unwrap());
        assert_eq!(utc.local(LATE_MONDAY).unwrap().format("%H:%M").to_string(), "23:30");
        assert_eq!(east.local(LATE_MONDAY).unwrap().format("%H:%M").to_string(), "00:30");
        assert_eq!(west.local(LATE_MONDAY).unwrap().format("%H:%M").to_string(), "15:30");
    }

    /// The system clock is what the call sites read before there was a clock to hand them:
    /// the system's time, and dates in the system's zone.
    #[test]
    fn the_system_clock_reads_the_system() {
        let clock = Clock::system();
        let before = Utc::now().timestamp();
        let now = clock.now_epoch();
        assert!((before..=Utc::now().timestamp()).contains(&now));
        assert_eq!(clock.zone(), Zone::Local);
        assert_eq!(
            clock.local(LATE_MONDAY).unwrap().naive_local(),
            Local.timestamp_opt(LATE_MONDAY, 0).unwrap().naive_local()
        );
    }

    #[test]
    fn an_offset_past_a_day_is_not_a_zone() {
        assert!(Zone::hours_east(24).is_none());
        assert!(Zone::hours_east(i32::MAX).is_none());
        assert!(Zone::hours_east(-23).is_some());
    }
}
