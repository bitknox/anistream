//! Jobs: work the user started and is waiting on, numbered so that whatever ends one can say
//! which.
//!
//! Only the id crosses. How a job is shown, when it earns a place on screen and how its wait is
//! counted down are the front end's.

/// Identifies one job, something the user started and is waiting on, so the work that finishes
/// it can say which.
///
/// The terminal lists each one as a `Job` in its activity row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct JobId(u64);

impl JobId {
    /// The job numbered `n`. Whoever starts jobs numbers them, once each per run: the side that
    /// runs the work, which announces each with `Event::JobStarted`.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// The job's number, for a front end that keys its jobs by a plain integer: one on the far
    /// side of an FFI.
    pub const fn get(self) -> u64 {
        self.0
    }
}
