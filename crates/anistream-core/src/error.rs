//! Error taxonomy.
//!
//! The distinctions here are load-bearing, not cosmetic. [`ProviderError`] is split the
//! way it is because the registry makes a *decision* from the variant: `Blocked` and
//! `Parse` mean this source is broken and we should fail over to the next one, while
//! `NotFound` means the source works fine and the title genuinely isn't there — failing
//! over on that would mask a correct answer and waste every remaining provider.

use std::fmt;

/// Top-level error type for anistream.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("configuration: {0}")]
    Config(String),

    #[error("provider {provider}: {source}")]
    Provider {
        provider: String,
        #[source]
        source: ProviderError,
    },

    #[error("no provider could resolve {title:?} episode {episode}")]
    AllProvidersFailed {
        title: String,
        episode: String,
        /// Per-provider reasons, preserved so the UI can say *which* source failed and
        /// why instead of showing an empty list.
        failures: Vec<(String, ProviderError)>,
    },

    #[error("player: {0}")]
    Player(String),

    #[error("tracker {tracker}: {message}")]
    Tracker { tracker: String, message: String },

    /// Credentials are absent, expired or rejected.
    ///
    /// Separate from [`Self::Tracker`] because the outbox treats the two completely
    /// differently: a network failure is worth retrying with backoff, while a rejected token
    /// will fail identically forever. Retrying it would burn attempts and hide the one thing
    /// the user actually has to do.
    #[error("authentication: {0}")]
    Auth(String),

    #[error("storage: {0}")]
    Store(String),

    #[error("network: {0}")]
    Network(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow_like::BoxError),
}

/// Minimal boxed-error shim so this crate does not need to depend on `anyhow`.
pub mod anyhow_like {
    pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a provider call failed.
///
/// Mirrors the `provider-error` variant in `wit/anistream-provider.wit` one-for-one, so
/// a WASM plugin and a native provider report failures in exactly the same vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    /// The provider works but has no such title or episode. **Not** a failover trigger.
    #[error("not found")]
    NotFound,

    /// Bot protection, rate limiting, geo-blocking, or an outright ban. The signature
    /// failure of a web source, and the main reason failover exists.
    #[error("blocked: {0}")]
    Blocked(String),

    /// The response arrived but didn't look like what we expected — almost always means
    /// the site changed its markup or API shape and the site adapter needs updating.
    #[error("could not parse response: {0}")]
    Parse(String),

    /// Transport-level failure: DNS, TLS, timeout, connection reset.
    #[error("transport: {0}")]
    Transport(String),

    /// The provider is configured but deliberately unavailable right now — for example
    /// the torrent provider while the VPN guard is failing. Distinct from `Blocked`
    /// because the cause is local policy, not the remote end.
    #[error("unavailable: {0}")]
    Unavailable(String),

    #[error("{0}")]
    Other(String),
}

impl ProviderError {
    /// Whether the registry should try the next provider.
    ///
    /// `NotFound` deliberately returns `false`: the source answered correctly.
    pub const fn should_failover(&self) -> bool {
        matches!(self, Self::Blocked(_) | Self::Parse(_) | Self::Transport(_) | Self::Other(_))
    }

    /// Whether this should count against the provider's health score.
    ///
    /// `Unavailable` is excluded — a provider held back by our own VPN policy is not
    /// unhealthy, and marking it down would produce a misleading Providers screen.
    pub const fn counts_against_health(&self) -> bool {
        matches!(self, Self::Blocked(_) | Self::Parse(_) | Self::Transport(_))
    }

    /// Short label for the Providers screen `STATE` column.
    pub const fn state_label(&self) -> &'static str {
        match self {
            Self::NotFound => "no match",
            Self::Blocked(_) => "blocked",
            Self::Parse(_) => "parse error",
            Self::Transport(_) => "unreachable",
            Self::Unavailable(_) => "held back",
            Self::Other(_) => "error",
        }
    }
}

/// Health of a single provider, as shown on the Providers screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Health {
    #[default]
    Unknown,
    Ready,
    Degraded,
    Down,
}

impl Health {
    /// The glyph used in the `STATE` column. Deliberately not emoji — a nerd-font or
    /// emoji dependency is both a portability hazard and a templated-TUI tell.
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::Unknown => "·",
            Self::Ready => "●",
            Self::Degraded => "▲",
            Self::Down => "✕",
        }
    }
}

impl fmt::Display for Health {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Unknown => "unknown",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::Down => "down",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_does_not_trigger_failover() {
        // The important asymmetry: a working provider that simply lacks the title must
        // not burn every other provider in the chain.
        assert!(!ProviderError::NotFound.should_failover());
        assert!(ProviderError::Blocked("cf challenge".into()).should_failover());
        assert!(ProviderError::Parse("no sourceUrl".into()).should_failover());
        assert!(ProviderError::Transport("timeout".into()).should_failover());
    }

    #[test]
    fn locally_held_back_providers_are_not_marked_unhealthy() {
        let vpn_down = ProviderError::Unavailable("vpn guard failing".into());
        assert!(!vpn_down.counts_against_health());
        assert!(!vpn_down.should_failover());
        assert!(ProviderError::Blocked("403".into()).counts_against_health());
    }

    #[test]
    fn health_glyphs_avoid_emoji() {
        for h in [Health::Unknown, Health::Ready, Health::Degraded, Health::Down] {
            let g = h.glyph();
            assert_eq!(g.chars().count(), 1);
            assert!(!g.is_ascii(), "glyph should be a box/geometric char");
        }
    }
}
