//! Token-bucket rate limiting.
//!
//! AniList advertises its limit in response headers (`x-ratelimit-limit: 30`,
//! `x-ratelimit-remaining`, `x-ratelimit-reset`), and the spine of the whole app sits
//! behind it — search, seasonal, calendar, library. Getting throttled there does not
//! degrade one provider, it degrades everything.
//!
//! So the limiter is *reactive* as well as proactive: it paces requests to the configured
//! budget, and when the server reports its own view it defers to that. The server is the
//! authority on its own limit; our local count is only an estimate.

use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

/// A refilling token bucket.
#[derive(Debug)]
pub struct RateLimiter {
    state: Mutex<State>,
    capacity: f64,
    /// Tokens added per second.
    refill_rate: f64,
}

#[derive(Debug)]
struct State {
    tokens: f64,
    last_refill: Instant,
    /// Set when the server tells us to stop until a given moment.
    hold_until: Option<Instant>,
}

impl RateLimiter {
    /// Build a limiter for `per_minute` requests.
    pub fn per_minute(per_minute: u32) -> Self {
        let capacity = f64::from(per_minute.max(1));
        Self {
            state: Mutex::new(State {
                tokens: capacity,
                last_refill: Instant::now(),
                hold_until: None,
            }),
            capacity,
            refill_rate: capacity / 60.0,
        }
    }

    /// Wait until a request may proceed.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut state = self.state.lock().await;
                let now = Instant::now();

                // A server-imposed hold outranks our own accounting.
                if let Some(until) = state.hold_until {
                    if until > now {
                        Some(until - now)
                    } else {
                        state.hold_until = None;
                        None
                    }
                } else {
                    None
                }
                .or_else(|| {
                    let elapsed = now.duration_since(state.last_refill).as_secs_f64();
                    state.tokens =
                        (state.tokens + elapsed * self.refill_rate).min(self.capacity);
                    state.last_refill = now;

                    if state.tokens >= 1.0 {
                        state.tokens -= 1.0;
                        None
                    } else {
                        let deficit = 1.0 - state.tokens;
                        Some(Duration::from_secs_f64(deficit / self.refill_rate))
                    }
                })
            };

            match wait {
                Some(d) => tokio::time::sleep(d).await,
                None => return,
            }
        }
    }

    /// Adopt the server's own view of the limit.
    ///
    /// `remaining` is what the server says is left; when it hits zero we hold until
    /// `reset` rather than continuing to trust the local bucket, which can drift out of
    /// step after retries or a restart.
    /// How long the next [`Self::acquire`] would block, without consuming a token.
    ///
    /// For telling the user *why* something is slow. A budget of thirty requests a minute is small
    /// enough that a burst of navigation can drain it, and an indefinite loading indicator with no
    /// explanation is the worst possible way to communicate "waiting for a rate limit".
    pub async fn would_wait(&self) -> Option<Duration> {
        let state = self.state.lock().await;
        let now = Instant::now();
        if let Some(until) = state.hold_until
            && until > now
        {
            return Some(until - now);
        }
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let tokens = (state.tokens + elapsed * self.refill_rate).min(self.capacity);
        if tokens >= 1.0 {
            return None;
        }
        Some(Duration::from_secs_f64((1.0 - tokens) / self.refill_rate))
    }

    pub async fn observe(&self, remaining: Option<u32>, reset_in: Option<Duration>) {
        let mut state = self.state.lock().await;
        if let Some(remaining) = remaining {
            // Never revise upwards: the server may be counting requests we do not know
            // about, so its number is a ceiling, not a refill.
            state.tokens = state.tokens.min(f64::from(remaining));
            if remaining == 0 {
                let hold = reset_in.unwrap_or(Duration::from_secs(60));
                state.hold_until = Some(Instant::now() + hold);
                tracing::warn!(?hold, "rate limit exhausted, holding");
            }
        }
    }

    /// Explicitly back off, for a `429` or `Retry-After`.
    pub async fn back_off(&self, duration: Duration) {
        let mut state = self.state.lock().await;
        state.tokens = 0.0;
        state.hold_until = Some(Instant::now() + duration);
    }
}

/// Pull rate-limit signals out of response headers.
///
/// Returns `(remaining, reset_in)`. Handles both the absolute-epoch and
/// seconds-from-now forms of `x-ratelimit-reset`, because services disagree about which
/// they mean and guessing wrong turns a 1-second wait into a 56-year one.
pub fn parse_rate_headers(
    get: impl Fn(&str) -> Option<String>,
    now_epoch: i64,
) -> (Option<u32>, Option<Duration>) {
    let remaining = get("x-ratelimit-remaining").and_then(|v| v.trim().parse::<u32>().ok());

    let reset = get("x-ratelimit-reset")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|reset| {
            // Values far in the future are epoch timestamps; small ones are durations.
            let secs = if reset > now_epoch.saturating_sub(86_400) && reset > 100_000 {
                (reset - now_epoch).max(0)
            } else {
                reset.max(0)
            };
            Duration::from_secs(secs.min(3_600) as u64)
        })
        .or_else(|| {
            get("retry-after")
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(Duration::from_secs)
        });

    (remaining, reset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let owned: Vec<(String, String)> =
            pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
        move |name| {
            owned.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
        }
    }

    #[tokio::test(start_paused = true)]
    async fn requests_within_budget_do_not_block() {
        let limiter = RateLimiter::per_minute(30);
        let start = Instant::now();
        for _ in 0..30 {
            limiter.acquire().await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO, "full bucket should be free");
    }

    #[tokio::test(start_paused = true)]
    async fn exceeding_the_budget_paces_rather_than_failing() {
        let limiter = RateLimiter::per_minute(30);
        for _ in 0..30 {
            limiter.acquire().await;
        }
        let start = Instant::now();
        limiter.acquire().await;
        // 30/min means one token every 2s.
        assert!(
            start.elapsed() >= Duration::from_secs(2),
            "should have waited for a refill, waited {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn the_server_view_can_only_tighten_the_budget() {
        let limiter = RateLimiter::per_minute(30);
        // Server says only 1 request left despite our bucket being full.
        limiter.observe(Some(1), None).await;
        limiter.acquire().await;

        let start = Instant::now();
        limiter.acquire().await;
        assert!(
            start.elapsed() >= Duration::from_secs(2),
            "server's lower count must be respected over our own optimism"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exhaustion_holds_until_the_reset_moment() {
        let limiter = RateLimiter::per_minute(30);
        limiter.observe(Some(0), Some(Duration::from_secs(45))).await;
        let start = Instant::now();
        limiter.acquire().await;
        assert!(start.elapsed() >= Duration::from_secs(45));
    }

    #[tokio::test(start_paused = true)]
    async fn back_off_blocks_for_the_requested_duration() {
        let limiter = RateLimiter::per_minute(60);
        limiter.back_off(Duration::from_secs(10)).await;
        let start = Instant::now();
        limiter.acquire().await;
        assert!(start.elapsed() >= Duration::from_secs(10));
    }

    #[test]
    fn reset_header_as_seconds_from_now_is_read_literally() {
        let (remaining, reset) = parse_rate_headers(
            headers(&[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "12")]),
            1_785_000_000,
        );
        assert_eq!(remaining, Some(0));
        assert_eq!(reset, Some(Duration::from_secs(12)));
    }

    #[test]
    fn reset_header_as_an_epoch_timestamp_is_converted_to_a_delay() {
        // The failure this guards against: treating an epoch value as a duration would
        // schedule a wait decades long and hang the app.
        let now = 1_785_000_000_i64;
        let (_, reset) =
            parse_rate_headers(headers(&[("x-ratelimit-reset", "1785000030")]), now);
        assert_eq!(reset, Some(Duration::from_secs(30)));
    }

    #[test]
    fn reset_delays_are_capped_to_an_hour() {
        let (_, reset) = parse_rate_headers(headers(&[("x-ratelimit-reset", "99999999")]), 0);
        assert_eq!(reset, Some(Duration::from_secs(3_600)));
    }

    #[test]
    fn retry_after_is_used_when_no_reset_header_exists() {
        let (_, reset) = parse_rate_headers(headers(&[("retry-after", "5")]), 0);
        assert_eq!(reset, Some(Duration::from_secs(5)));
    }

    #[test]
    fn missing_headers_yield_no_signal() {
        let (remaining, reset) = parse_rate_headers(headers(&[]), 0);
        assert_eq!(remaining, None);
        assert_eq!(reset, None);
    }

    #[test]
    fn anilist_headers_parse_as_observed_in_the_wild() {
        // Exactly what graphql.anilist.co returned during planning.
        let (remaining, _) = parse_rate_headers(
            headers(&[("x-ratelimit-limit", "30"), ("x-ratelimit-remaining", "29")]),
            1_785_000_000,
        );
        assert_eq!(remaining, Some(29));
    }
}
