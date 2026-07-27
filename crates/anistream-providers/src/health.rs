//! Per-provider health accounting.
//!
//! Feeds the Providers screen, which exists because sources die and the user needs to see
//! *which* one and why rather than an unexplained empty list.
//!
//! One distinction runs through this module: being **held back** is not being **unhealthy**.
//! The torrent provider with a failing VPN guard is working perfectly; local policy is
//! keeping it out. Marking it down would produce a Providers screen that lies.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use anistream_core::error::{Health, ProviderError};

/// Consecutive failures before a provider is considered down rather than degraded.
///
/// More than one, because a single timeout is usually the network rather than the source.
const DOWN_AFTER: u32 = 3;

/// What the Providers screen shows for one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderHealth {
    pub id: String,
    pub health: Health,
    /// Consecutive failures. Reset by any success.
    pub consecutive_failures: u32,
    pub last_error: Option<String>,
    /// Round-trip of the last successful call.
    pub last_latency: Option<Duration>,
    pub last_checked: Option<i64>,
    /// Set when local policy is withholding the provider — a failing VPN guard, say.
    pub held_back: Option<String>,
}

impl ProviderHealth {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            health: Health::Unknown,
            consecutive_failures: 0,
            last_error: None,
            last_latency: None,
            last_checked: None,
            held_back: None,
        }
    }

    /// Whether resolution should even try this provider.
    pub fn is_usable(&self) -> bool {
        self.held_back.is_none() && self.health != Health::Down
    }

    /// Short state text for the Providers screen.
    pub fn state_label(&self) -> String {
        match (&self.held_back, self.health) {
            (Some(_), _) => "held back".into(),
            (None, Health::Ready) => "ready".into(),
            (None, Health::Degraded) => "degraded".into(),
            (None, Health::Down) => "down".into(),
            (None, Health::Unknown) => "unchecked".into(),
        }
    }
}

/// Shared, mutable health for every provider.
#[derive(Debug, Clone, Default)]
pub struct HealthTracker {
    inner: Arc<Mutex<Vec<ProviderHealth>>>,
}

impl HealthTracker {
    pub fn new(ids: impl IntoIterator<Item = String>) -> Self {
        Self { inner: Arc::new(Mutex::new(ids.into_iter().map(ProviderHealth::new).collect())) }
    }

    /// Start tracking sources that appeared after startup.
    ///
    /// Ignores ids already present, so a re-registration cannot wipe a provider's failure history
    /// and hand a known-bad source a clean slate.
    pub fn register(&self, ids: impl IntoIterator<Item = String>) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            if !inner.iter().any(|h| h.id == id) {
                inner.push(ProviderHealth::new(id));
            }
        }
    }

    fn with<T>(&self, f: impl FnOnce(&mut Vec<ProviderHealth>) -> T) -> T {
        // A panic elsewhere must not make health reporting unusable.
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        f(&mut guard)
    }

    fn entry<'a>(list: &'a mut Vec<ProviderHealth>, id: &str) -> &'a mut ProviderHealth {
        if let Some(index) = list.iter().position(|h| h.id == id) {
            return &mut list[index];
        }
        list.push(ProviderHealth::new(id));
        list.last_mut().expect("just pushed")
    }

    pub fn record_success(&self, id: &str, latency: Duration, now: i64) {
        self.with(|list| {
            let entry = Self::entry(list, id);
            entry.health = Health::Ready;
            entry.consecutive_failures = 0;
            entry.last_error = None;
            entry.last_latency = Some(latency);
            entry.last_checked = Some(now);
        });
    }

    pub fn record_failure(&self, id: &str, error: &ProviderError, now: i64) {
        self.with(|list| {
            let entry = Self::entry(list, id);
            entry.last_checked = Some(now);

            // A provider that answered correctly and simply has no such title is healthy.
            // Counting that as a failure would take working sources out of rotation.
            if !error.counts_against_health() {
                if let ProviderError::Unavailable(reason) = error {
                    entry.held_back = Some(reason.clone());
                }
                return;
            }

            entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            entry.last_error = Some(error.to_string());
            entry.health = if entry.consecutive_failures >= DOWN_AFTER {
                Health::Down
            } else {
                Health::Degraded
            };
        });
    }

    /// Record that local policy is withholding a provider.
    pub fn hold_back(&self, id: &str, reason: impl Into<String>) {
        self.with(|list| Self::entry(list, id).held_back = Some(reason.into()));
    }

    pub fn release(&self, id: &str) {
        self.with(|list| Self::entry(list, id).held_back = None);
    }

    pub fn get(&self, id: &str) -> Option<ProviderHealth> {
        self.with(|list| list.iter().find(|h| h.id == id).cloned())
    }

    pub fn all(&self) -> Vec<ProviderHealth> {
        self.with(|list| list.clone())
    }

    /// Providers worth attempting, in the order given.
    pub fn usable(&self, order: &[String]) -> Vec<String> {
        self.with(|list| {
            order
                .iter()
                .filter(|id| {
                    list.iter().find(|h| &&h.id == id).is_none_or(ProviderHealth::is_usable)
                })
                .cloned()
                .collect()
        })
    }

    /// Clear the down state so a recovered provider gets another chance.
    pub fn reset(&self, id: &str) {
        self.with(|list| {
            let entry = Self::entry(list, id);
            entry.health = Health::Unknown;
            entry.consecutive_failures = 0;
            entry.last_error = None;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker() -> HealthTracker {
        HealthTracker::new(["torrent".to_string(), "remote".to_string()])
    }

    #[test]
    fn a_fresh_provider_is_unchecked_but_usable() {
        let t = tracker();
        let h = t.get("torrent").unwrap();
        assert_eq!(h.health, Health::Unknown);
        assert!(h.is_usable(), "never having been tried is not a reason to skip it");
        assert_eq!(h.state_label(), "unchecked");
    }

    #[test]
    fn repeated_failures_degrade_then_take_a_provider_down() {
        let t = tracker();
        let err = ProviderError::Blocked("cloudflare".into());

        t.record_failure("torrent", &err, 0);
        assert_eq!(t.get("torrent").unwrap().health, Health::Degraded);
        assert!(t.get("torrent").unwrap().is_usable(), "one failure is usually the network");

        t.record_failure("torrent", &err, 1);
        t.record_failure("torrent", &err, 2);
        let h = t.get("torrent").unwrap();
        assert_eq!(h.health, Health::Down);
        assert_eq!(h.consecutive_failures, 3);
        assert!(!h.is_usable());
        assert_eq!(h.last_error.as_deref(), Some("blocked: cloudflare"));
    }

    #[test]
    fn any_success_clears_the_failure_streak() {
        let t = tracker();
        let err = ProviderError::Transport("timeout".into());
        t.record_failure("torrent", &err, 0);
        t.record_failure("torrent", &err, 1);

        t.record_success("torrent", Duration::from_millis(310), 2);
        let h = t.get("torrent").unwrap();
        assert_eq!(h.health, Health::Ready);
        assert_eq!(h.consecutive_failures, 0);
        assert!(h.last_error.is_none(), "a stale error would misreport a working source");
        assert_eq!(h.last_latency, Some(Duration::from_millis(310)));
    }

    #[test]
    fn a_missing_title_never_counts_against_health() {
        // The asymmetry that matters: a provider answering "I do not have that" is working
        // perfectly, and marking it down would remove a good source from rotation.
        let t = tracker();
        for i in 0..10 {
            t.record_failure("torrent", &ProviderError::NotFound, i);
        }
        let h = t.get("torrent").unwrap();
        assert_eq!(h.health, Health::Unknown);
        assert_eq!(h.consecutive_failures, 0);
        assert!(h.is_usable());
    }

    #[test]
    fn being_held_back_is_not_being_unhealthy() {
        // A torrent provider behind a failing VPN guard is fine; local policy is what is
        // withholding it. Reporting it as "down" would be a lie.
        let t = tracker();
        t.record_failure("torrent", &ProviderError::Unavailable("vpn guard failing".into()), 0);

        let h = t.get("torrent").unwrap();
        assert_eq!(h.health, Health::Unknown, "not a health problem");
        assert_eq!(h.consecutive_failures, 0);
        assert!(!h.is_usable(), "but it must still be skipped");
        assert_eq!(h.state_label(), "held back");
        assert_eq!(h.held_back.as_deref(), Some("vpn guard failing"));
    }

    #[test]
    fn a_held_back_provider_can_be_released() {
        let t = tracker();
        t.hold_back("torrent", "vpn down");
        assert!(!t.get("torrent").unwrap().is_usable());
        t.release("torrent");
        assert!(t.get("torrent").unwrap().is_usable());
    }

    #[test]
    fn usable_preserves_configured_order_and_drops_the_unusable() {
        let t = tracker();
        let order = vec!["torrent".to_string(), "remote".to_string()];
        assert_eq!(t.usable(&order), order, "order is the user's preference");

        t.hold_back("torrent", "vpn down");
        assert_eq!(t.usable(&order), vec!["remote".to_string()]);
    }

    #[test]
    fn an_unknown_provider_id_is_assumed_usable() {
        // A provider that has never been recorded should be tried, not skipped.
        let t = HealthTracker::default();
        assert_eq!(t.usable(&["brand-new".to_string()]), vec!["brand-new".to_string()]);
    }

    #[test]
    fn resetting_gives_a_recovered_provider_another_chance() {
        let t = tracker();
        let err = ProviderError::Blocked("403".into());
        for i in 0..3 {
            t.record_failure("torrent", &err, i);
        }
        assert!(!t.get("torrent").unwrap().is_usable());

        t.reset("torrent");
        assert!(t.get("torrent").unwrap().is_usable());
        assert!(t.get("torrent").unwrap().last_error.is_none());
    }

    #[test]
    fn a_reset_does_not_release_a_policy_hold() {
        // Otherwise re-checking a provider would quietly bypass the VPN guard.
        let t = tracker();
        t.hold_back("torrent", "vpn down");
        t.reset("torrent");
        assert!(!t.get("torrent").unwrap().is_usable(), "the hold must survive a reset");
    }

    #[test]
    fn health_is_shared_across_clones() {
        // The registry and the Providers screen hold separate handles to the same state.
        let a = tracker();
        let b = a.clone();
        a.record_success("torrent", Duration::from_millis(5), 0);
        assert_eq!(b.get("torrent").unwrap().health, Health::Ready);
    }
}
