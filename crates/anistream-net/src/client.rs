//! The two-client HTTP surface.

use std::time::Duration;

use anistream_core::config::NetworkConfig;

#[derive(Debug, thiserror::Error)]
pub enum NetError {
    #[error("building http client: {0}")]
    Build(String),

    #[error("request to {url} failed: {message}")]
    Request { url: String, message: String },

    #[error("{url} returned HTTP {status}")]
    Status { url: String, status: u16 },

    #[error("unknown emulation profile {0:?}")]
    UnknownProfile(String),
}

/// Which browser handshake to reproduce.
///
/// Exposed in config rather than hardcoded so it can be changed without a rebuild.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Chrome,
    Firefox,
    Safari,
    Edge,
}

impl Profile {
    pub fn parse(name: &str) -> Result<Self, NetError> {
        match name.trim().to_ascii_lowercase().as_str() {
            "chrome" | "chromium" => Ok(Self::Chrome),
            "firefox" => Ok(Self::Firefox),
            "safari" => Ok(Self::Safari),
            "edge" => Ok(Self::Edge),
            other => Err(NetError::UnknownProfile(other.to_owned())),
        }
    }

    /// The newest profile `wreq-util` ships for this browser.
    ///
    /// The bundled versions trail current releases — 2.2.6's newest Chrome profile is 137 while
    /// shipping Chrome is past 141 — which is one reason the choice is a config value.
    fn emulation(self) -> wreq_util::Emulation {
        match self {
            Self::Chrome => wreq_util::Emulation::Chrome137,
            Self::Firefox => wreq_util::Emulation::Firefox139,
            Self::Safari => wreq_util::Emulation::Safari18,
            Self::Edge => wreq_util::Emulation::Edge134,
        }
    }
}

/// Shared HTTP access.
///
/// Cheap to clone — both inner clients are reference-counted and pool connections, so a
/// single instance should be created at startup and handed around.
#[derive(Clone)]
pub struct HttpClient {
    emulated: wreq::Client,
    plain: reqwest::Client,
    profile: Profile,
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient").field("profile", &self.profile).finish_non_exhaustive()
    }
}

impl HttpClient {
    pub fn new(config: &NetworkConfig) -> Result<Self, NetError> {
        let profile = Profile::parse(&config.emulation)?;
        let timeout = Duration::from_secs(config.timeout_secs.max(1));

        let emulated = wreq::Client::builder()
            .emulation(profile.emulation())
            .timeout(timeout)
            .build()
            .map_err(|e| NetError::Build(format!("emulated client: {e}")))?;

        let plain = reqwest::Client::builder()
            .timeout(timeout)
            .user_agent(concat!("anistream/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| NetError::Build(format!("plain client: {e}")))?;

        Ok(Self { emulated, plain, profile })
    }

    /// Client with a browser TLS/HTTP2 fingerprint. Use for provider hosts behind bot
    /// protection.
    pub fn emulated(&self) -> &wreq::Client {
        &self.emulated
    }

    /// Plain client. Use for hosts with no bot protection — it is faster and its
    /// behaviour is easier to reason about.
    pub fn plain(&self) -> &reqwest::Client {
        &self.plain
    }

    pub fn profile(&self) -> Profile {
        self.profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_parse_case_insensitively_and_reject_nonsense() {
        assert_eq!(Profile::parse("Chrome").unwrap(), Profile::Chrome);
        assert_eq!(Profile::parse("  firefox ").unwrap(), Profile::Firefox);
        assert_eq!(Profile::parse("chromium").unwrap(), Profile::Chrome);
        assert!(Profile::parse("netscape").is_err());
    }

    #[test]
    fn client_builds_from_default_config() {
        let client = HttpClient::new(&NetworkConfig::default()).unwrap();
        assert_eq!(client.profile(), Profile::Chrome);
    }

    #[test]
    fn an_unknown_profile_fails_loudly_rather_than_silently_defaulting() {
        // Silently falling back would mean a typo in the emulation profile turns into
        // mysterious 403s from every provider.
        let cfg = NetworkConfig {
            emulation: "definitely-not-a-browser".into(),
            ..Default::default()
        };
        assert!(matches!(HttpClient::new(&cfg), Err(NetError::UnknownProfile(_))));
    }
}
