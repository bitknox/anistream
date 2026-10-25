//! The time the new-episode check, the widgets' record and the notifications' week go by, and
//! the zone they read days in.
//!
//! The system's clock in the system's zone. With the `testing` feature a test can move it on
//! (`Clock::advance`), to see what a check finds once an episode has aired without waiting for
//! it, and read its days in a zone of its own (`Clock::in_zone`), so that what is today and what
//! airs this week do not depend on where, or at what hour, the test runs. It cannot be stopped at
//! a moment of its own: what it dates is compared with what the store and the metadata cache
//! date by the system's clock, which the rest of the engine reads directly
//! ([`crate::now_epoch`]).

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use jiff::tz::TimeZone;

/// Now, as the work that is dated by it reads it, and the zone its days are in. Cheap to clone;
/// every clone moves together.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    /// How far ahead of the system's clock this one runs: always nought outside a test.
    ahead: Arc<AtomicI64>,
    /// The zone a test reads days in, in place of the system's.
    zone: Option<TimeZone>,
}

impl Clock {
    /// Now, in seconds since the Unix epoch.
    pub fn now(&self) -> i64 {
        crate::now_epoch().saturating_add(self.ahead.load(Ordering::Relaxed))
    }

    /// The zone days are read in: the system's, as the device has it, or a test's.
    pub fn zone(&self) -> TimeZone {
        self.zone.clone().unwrap_or_else(TimeZone::system)
    }

    /// The system's clock, reading days in `zone` rather than the system's zone.
    #[cfg(feature = "testing")]
    pub fn in_zone(zone: TimeZone) -> Self {
        Self { ahead: Arc::default(), zone: Some(zone) }
    }

    /// Move this clock, and every clone of it, `secs` further on.
    #[cfg(any(test, feature = "testing"))]
    pub fn advance(&self, secs: i64) {
        self.ahead.fetch_add(secs, Ordering::Relaxed);
    }
}
