//! The device-code sign-ins running, one per tracker: asking for a new code replaces the one
//! running, and closing the sign-in stops it.
//!
//! A code lasts as long as its service says, fifteen minutes as a rule. A TV counts down to
//! [`DeviceCodePrompt::expires_at`] and, once it passes, sends [`Command::Connect`] for the
//! tracker again; so does "Get a new code". Each would otherwise leave the flow before it
//! polling for a code nobody can still type until it timed out, and then say it expired over
//! the new one.
//!
//! [`DeviceCodePrompt::expires_at`]: anistream_model::sync::DeviceCodePrompt::expires_at
//! [`Command::Connect`]: anistream_model::command::Command::Connect

use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
};

use anistream_model::sync::DeviceCodePrompt;
use tokio::sync::Notify;

/// The flows running, by tracker.
#[derive(Clone, Default)]
pub struct DeviceFlows {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    running: HashMap<String, (u64, Arc<Notify>)>,
    /// Counts flows begun, so one that ends knows whether it is still the tracker's.
    serial: u64,
}

/// One flow, from [`DeviceFlows::begin`]: what it waits on stops when another begins for the
/// tracker, or the sign-in is closed. Dropped, it is no longer running.
pub struct DeviceFlow {
    flows: DeviceFlows,
    tracker: String,
    serial: u64,
    stop: Arc<Notify>,
}

impl DeviceFlows {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Begin a flow for `tracker`, stopping the one running for it.
    pub fn begin(&self, tracker: &str) -> DeviceFlow {
        let stop = Arc::new(Notify::new());
        let mut inner = self.lock();
        inner.serial += 1;
        let serial = inner.serial;
        if let Some((_, before)) =
            inner.running.insert(tracker.to_owned(), (serial, Arc::clone(&stop)))
        {
            tracing::info!(tracker, "a new device code replaces the one before");
            before.notify_one();
        }
        DeviceFlow { flows: self.clone(), tracker: tracker.to_owned(), serial, stop }
    }

    /// Stop the flow running for `tracker`: its sign-in was closed. Whether one was running.
    pub fn stop(&self, tracker: &str) -> bool {
        match self.lock().running.remove(tracker) {
            Some((_, stop)) => {
                stop.notify_one();
                true
            }
            None => false,
        }
    }

    /// Whether a flow is running for `tracker`.
    pub fn running(&self, tracker: &str) -> bool {
        self.lock().running.contains_key(tracker)
    }
}

impl DeviceFlow {
    /// `work`, unless the flow is stopped first: then `None`, and the flow says nothing more,
    /// since what stopped it says what follows.
    pub async fn run<F: Future>(&self, work: F) -> Option<F::Output> {
        tokio::select! {
            done = work => Some(done),
            () = self.stop.notified() => None,
        }
    }

    /// Whether this is still the tracker's flow: none has replaced it, and it was not closed.
    pub fn current(&self) -> bool {
        self.flows
            .lock()
            .running
            .get(&self.tracker)
            .is_some_and(|(serial, _)| *serial == self.serial)
    }
}

impl Drop for DeviceFlow {
    fn drop(&mut self) {
        let mut inner = self.flows.lock();
        if inner.running.get(&self.tracker).is_some_and(|(serial, _)| *serial == self.serial) {
            inner.running.remove(&self.tracker);
        }
    }
}

/// The prompt for `user_code`, typed at `url`, which the service said lasts `expires_in`
/// seconds from `now`.
pub fn prompt(
    tracker: &str,
    user_code: &str,
    url: &str,
    expires_in: u64,
    now: i64,
) -> DeviceCodePrompt {
    DeviceCodePrompt {
        tracker: tracker.to_owned(),
        code: user_code.to_owned(),
        url: url.to_owned(),
        expires_at: i64::try_from(expires_in).ok().map(|secs| now.saturating_add(secs)),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn a_prompt_says_when_its_code_stops_working() {
        let prompt = prompt("simkl", "AB12C", "https://pin.example/", 900, 1_800_000_000);
        assert_eq!(prompt.expires_at, Some(1_800_000_900));
        assert_eq!(prompt.code, "AB12C");
    }

    #[tokio::test]
    async fn a_new_code_stops_the_flow_before_it_and_only_that_trackers() {
        let flows = DeviceFlows::default();
        let first = flows.begin("simkl");
        let other = flows.begin("trakt");
        let waiting = tokio::spawn(async move {
            let polled = first.run(std::future::pending::<()>()).await;
            (polled, first.current())
        });
        tokio::task::yield_now().await;
        let second = flows.begin("simkl");
        let (polled, current) =
            tokio::time::timeout(Duration::from_secs(5), waiting).await.unwrap().unwrap();
        assert_eq!(polled, None, "stopped, saying nothing");
        assert!(!current, "replaced");
        assert!(second.current() && other.current());
        assert_eq!(second.run(async { 7 }).await, Some(7));
    }

    #[tokio::test]
    async fn closing_the_sign_in_stops_its_flow_even_before_it_waits() {
        let flows = DeviceFlows::default();
        let flow = flows.begin("trakt");
        assert!(flows.stop("trakt"));
        assert!(!flows.stop("trakt"), "nothing left to stop");
        let polled = tokio::time::timeout(
            Duration::from_secs(5),
            flow.run(std::future::pending::<()>()),
        )
        .await
        .unwrap();
        assert_eq!(polled, None);
        assert!(!flow.current());
    }

    #[test]
    fn a_flow_that_ends_is_no_longer_running_unless_replaced() {
        let flows = DeviceFlows::default();
        drop(flows.begin("simkl"));
        assert!(!flows.running("simkl"));
        let first = flows.begin("simkl");
        let second = flows.begin("simkl");
        drop(first);
        assert!(flows.running("simkl"), "the newer one still runs");
        drop(second);
        assert!(!flows.running("simkl"));
    }
}
