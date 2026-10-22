//! The time the new-episode check and the widgets' record go by.
//!
//! The system's clock, which a test can move on with `Clock::advance` (with the `testing`
//! feature) to see what a check finds once an episode has aired, without waiting for it. The
//! rest of the engine reads [`crate::now_epoch`] directly.

use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

/// Now, as the work that is dated by it reads it. Cheap to clone; every clone moves together.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    /// How far ahead of the system's clock this one runs: always nought outside a test.
    ahead: Arc<AtomicI64>,
}

impl Clock {
    /// Now, in seconds since the Unix epoch.
    pub fn now(&self) -> i64 {
        crate::now_epoch().saturating_add(self.ahead.load(Ordering::Relaxed))
    }

    /// Move this clock, and every clone of it, `secs` further on.
    #[cfg(feature = "testing")]
    pub fn advance(&self, secs: i64) {
        self.ahead.fetch_add(secs, Ordering::Relaxed);
    }
}
