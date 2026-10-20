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

        let plain = trusted_roots(reqwest::Client::builder())
            .timeout(timeout)
            .user_agent(concat!("anistream/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| NetError::Build(format!("plain client: {e}")))?;

        Ok(Self { emulated, plain, profile })
    }

    /// A client that can reach no host by name, for tests that stand the app up whole.
    ///
    /// The app's own two clients, built from `config` as [`Self::new`] builds them, with two
    /// changes. Every name resolves to an error naming this client, at once, so a request for
    /// a real host fails before a packet — a DNS query included — leaves the machine. And no
    /// proxy is used, because a proxy is reached by its own address and resolves the name at
    /// its end, which would let a request past the resolver. An address written as an IP
    /// literal is never resolved, so a server on `127.0.0.1` answers as usual.
    ///
    /// The point is the requests a test did not think of: a tracker built with the real
    /// endpoint, a cover, a skip-time lookup. Each fails loudly in its test instead of
    /// reaching a real service with a made-up token. Hidden because nothing outside a test has
    /// a use for a client that cannot connect.
    #[doc(hidden)]
    pub fn offline(config: &NetworkConfig) -> Result<Self, NetError> {
        let profile = Profile::parse(&config.emulation)?;
        let timeout = Duration::from_secs(config.timeout_secs.max(1));

        let emulated = wreq::Client::builder()
            .emulation(profile.emulation())
            .timeout(timeout)
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(Unresolved))
            .build()
            .map_err(|e| NetError::Build(format!("emulated client: {e}")))?;

        let plain = trusted_roots(reqwest::Client::builder())
            .timeout(timeout)
            .user_agent(concat!("anistream/", env!("CARGO_PKG_VERSION")))
            .no_proxy()
            .dns_resolver(std::sync::Arc::new(Unresolved))
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

/// `builder`, trusting what a plain client trusts on this platform. Every plain client is built
/// through this: [`HttpClient`]'s, and the VPN guard's.
///
/// Everywhere but Android, reqwest's own choice, the platform's verifier: the system's trust
/// store, with whatever the user or their organisation installed in it. Desktops and iOS are
/// left exactly as they were.
///
/// On Android that verifier reaches the system's store through JNI, and the app has to hand it
/// the JVM and a context before the first request, or every request fails. So there a plain
/// client trusts the Mozilla roots built into the binary instead, [`bundled_roots`], which need
/// nothing from the app. **The cost: a CA the user installed on the device is not trusted.** A
/// request through a proxy that inspects TLS, or to a server behind a private CA, fails on
/// Android where it works elsewhere. The emulated client has always trusted those same bundled
/// roots, through BoringSSL, on every platform.
pub fn trusted_roots(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    if cfg!(target_os = "android") { builder.tls_certs_only(bundled_roots()) } else { builder }
}

/// The Mozilla roots built into the binary, as reqwest takes them: what Android's plain clients
/// trust, see [`trusted_roots`].
fn bundled_roots() -> Vec<reqwest::Certificate> {
    webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|der| reqwest::Certificate::from_der(der).ok())
        .collect()
}

/// The resolver behind [`HttpClient::offline`]: every name is refused, without asking anyone.
#[derive(Debug)]
struct Unresolved;

impl Unresolved {
    /// What a refused name fails with. Says which client refused it, so a test that trips over
    /// it learns that the request was for a real host rather than that the network is down.
    fn refusal(name: &str) -> Box<dyn std::error::Error + Send + Sync> {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("{name} was not resolved: an offline client reaches IP addresses only"),
        ))
    }
}

impl reqwest::dns::Resolve for Unresolved {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(std::future::ready(Err(Self::refusal(name.as_str()))))
    }
}

impl wreq::dns::Resolve for Unresolved {
    fn resolve(&self, name: wreq::dns::Name) -> wreq::dns::Resolving {
        Box::pin(std::future::ready(Err(Self::refusal(name.as_str()))))
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
    fn every_bundled_root_is_one_a_client_will_trust() {
        // What Android's plain clients trust, checked where the tests run: one root dropped as
        // unreadable is a host that fails there and nowhere else, and a root rustls refuses
        // fails the client's build, which is the app not starting.
        let roots = bundled_roots();
        assert_eq!(roots.len(), webpki_root_certs::TLS_SERVER_ROOT_CERTS.len());
        assert!(roots.len() > 100, "{} roots", roots.len());
        reqwest::Client::builder().tls_certs_only(roots).build().unwrap();
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

    /// Every message in an error's chain, outermost first: the resolver's refusal is several
    /// layers down in both clients.
    fn chain(error: &dyn std::error::Error) -> String {
        let mut said = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            said.push_str(" / ");
            said.push_str(&cause.to_string());
            source = cause.source();
        }
        said
    }

    #[tokio::test]
    async fn an_offline_client_reaches_loopback_and_no_named_host() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let client =
            HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..Default::default() })
                .unwrap();
        let refused = "guard.example was not resolved";

        // A name, through either client, is refused by the resolver: the request never gets as
        // far as a connection, and the error says why rather than looking like an outage.
        let plain = client.plain().get("http://guard.example/").send().await.unwrap_err();
        assert!(plain.is_connect(), "{plain:?}");
        assert!(chain(&plain).contains(refused), "{}", chain(&plain));
        let emulated =
            client.emulated().get("https://guard.example/").send().await.unwrap_err();
        assert!(chain(&emulated).contains(refused), "{}", chain(&emulated));

        // An address is not a name, so a server on loopback answers both clients.
        let listener =
            tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut head = Vec::new();
                let mut byte = [0_u8; 1];
                while !head.ends_with(b"\r\n\r\n") && socket.read(&mut byte).await.unwrap() == 1
                {
                    head.push(byte[0]);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 204 No Content\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .await
                    .unwrap();
            }
        });
        let url = format!("http://{addr}/");
        assert_eq!(client.plain().get(&url).send().await.unwrap().status().as_u16(), 204);
        assert_eq!(client.emulated().get(&url).send().await.unwrap().status().as_u16(), 204);
        server.await.unwrap();
    }
}
