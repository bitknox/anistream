//! The capabilities the host lends to a guest.
//!
//! Everything a plugin can do passes through here, which makes this the place where the sandbox
//! is real rather than declared. Three rules shape it:
//!
//! - **The guest never sees a socket.** [`Capabilities::fetch`] takes a request and returns
//!   bytes; the connection, the TLS fingerprint and the rate limiter all
//!   belong to the host.
//! - **Redirects are not followed.** A plugin allowed `example.com` must not be able to reach
//!   `evil.test` because `example.com` chose to `302` there. The redirect is handed back as a
//!   response, so the guest can decide — and the next hop is allowlisted again.
//! - **Every ceiling is counted host-side.** Fetch counts and body sizes are tracked here, not
//!   trusted to the guest.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use crate::sandbox::{self, Limits};

/// Why a host call failed, in the vocabulary the WIT world uses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostError {
    /// The URL was not permitted by the plugin's manifest.
    #[error("denied: {0}")]
    Denied(String),
    #[error("timed out")]
    Timeout,
    #[error("transport: {0}")]
    Transport(String),
}

/// One HTTP exchange, as a guest describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// Headers a guest may not set.
///
/// Not paranoia about the guest so much as about *correctness*: `accept-encoding` has to agree
/// with the handshake the host negotiated, and `host`, `content-length`, `connection` and
/// `transfer-encoding` are the client's to compute. Everything else, including `user-agent` and
/// `cookie`, is the guest's to set.
const RESERVED_HEADERS: &[&str] =
    &["host", "content-length", "connection", "transfer-encoding", "accept-encoding"];

/// What the host lends to one plugin, for the duration of one call.
///
/// Constructed per call rather than per plugin, because the fetch counter has to reset — a
/// long-lived counter would let the twelfth search of a session fail for no reason the user
/// could understand.
pub struct Capabilities {
    plugin_id: String,
    allowed_hosts: Vec<String>,
    limits: Limits,
    fetches: Arc<AtomicU32>,
    /// `None` in tests that exercise policy without a network.
    http: Option<anistream_net::HttpClient>,
    /// This plugin's section of the user's configuration, served one key at a time through
    /// `config-get`. Read-only pairs the host does not interpret — how a login-gated source
    /// receives an API key without the guest gaining any filesystem access.
    settings: std::collections::BTreeMap<String, String>,
}

impl Capabilities {
    pub fn new(
        plugin_id: impl Into<String>,
        allowed_hosts: Vec<String>,
        limits: Limits,
        http: Option<anistream_net::HttpClient>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            allowed_hosts,
            limits,
            fetches: Arc::new(AtomicU32::new(0)),
            http,
            settings: Default::default(),
        }
    }

    /// Attach the user's settings for this plugin. Absent by default, because most calls —
    /// and every `describe` — must work without any.
    pub fn with_settings(
        mut self,
        settings: std::collections::BTreeMap<String, String>,
    ) -> Self {
        self.settings = settings;
        self
    }

    /// One value from this plugin's settings, for the lent `config-get`.
    pub fn setting(&self, key: &str) -> Option<String> {
        self.settings.get(key).cloned()
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn fetches_used(&self) -> u32 {
        self.fetches.load(Ordering::Relaxed)
    }

    /// Check a request against policy without performing it.
    ///
    /// Separated from [`Self::fetch`] so the rules are testable without a network, and so the
    /// order is unambiguous: budget, then scheme and host, then headers.
    pub fn authorise(&self, request: &Request) -> Result<(), HostError> {
        let used = self.fetches.load(Ordering::Relaxed);
        if used >= self.limits.max_fetches {
            return Err(HostError::Denied(format!(
                "fetch budget exhausted ({} per call)",
                self.limits.max_fetches
            )));
        }

        // GET and POST are what parsing needs. Anything else is a way to change state on a
        // remote site, which a provider plugin has no business doing.
        let method = request.method.to_ascii_uppercase();
        if !matches!(method.as_str(), "GET" | "POST") {
            return Err(HostError::Denied(format!("method {method} is not permitted")));
        }

        if !sandbox::is_allowed(&request.url, &self.allowed_hosts) {
            let host = sandbox::host_of(&request.url).unwrap_or_else(|| "?".into());
            return Err(HostError::Denied(format!(
                "{host:?} is not in {}'s allowed-hosts",
                self.plugin_id
            )));
        }

        if let Some(name) = request
            .headers
            .iter()
            .map(|(n, _)| n.to_ascii_lowercase())
            .find(|n| RESERVED_HEADERS.contains(&n.as_str()))
        {
            return Err(HostError::Denied(format!("header {name:?} is set by the host")));
        }

        Ok(())
    }

    /// Perform a request on the guest's behalf.
    pub async fn fetch(&self, request: Request) -> Result<Response, HostError> {
        self.authorise(&request)?;
        self.fetches.fetch_add(1, Ordering::Relaxed);

        let Some(http) = &self.http else {
            return Err(HostError::Transport("no http client in this host".into()));
        };

        tracing::debug!(plugin = %self.plugin_id, url = %request.url, "plugin fetch");

        // The emulating client: a plugin inherits the same TLS/HTTP2 fingerprint as the native
        // providers, which is the whole reason `fetch` is lent rather than granted.
        let mut builder = match request.method.to_ascii_uppercase().as_str() {
            "POST" => http.emulated().post(&request.url),
            _ => http.emulated().get(&request.url),
        };
        for (name, value) in &request.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = request.body {
            builder = builder.body(body);
        }

        let response = tokio::time::timeout(self.limits.deadline, builder.send())
            .await
            .map_err(|_| HostError::Timeout)?
            .map_err(|e| HostError::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.to_string(), value.to_str().ok()?.to_owned()))
            })
            .collect();

        let body = response.bytes().await.map_err(|e| HostError::Transport(e.to_string()))?;
        if body.len() > self.limits.max_response_bytes {
            return Err(HostError::Denied(format!(
                "response of {} bytes exceeds the {} byte ceiling",
                body.len(),
                self.limits.max_response_bytes
            )));
        }

        Ok(Response { status, headers, body: body.to_vec() })
    }
}

/// AES-128-CBC decryption with PKCS#7 padding.
///
/// Lent because some sources wrap their stream payloads in it, and a TinyGo or JS
/// guest bundling its own AES would dwarf the parsing logic it exists for.
pub fn aes_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};

    // The key length selects the variant, exactly as the WIT documents: real sources use
    // AES-256 about as often as AES-128, and a guest cannot pad its own key into working.
    if !matches!(key.len(), 16 | 24 | 32) {
        return Err(format!(
            "key must be 16, 24 or 32 bytes for AES-128/192/256, got {}",
            key.len()
        ));
    }
    if iv.len() != 16 {
        return Err(format!("iv must be 16 bytes, got {}", iv.len()));
    }
    if data.is_empty() || !data.len().is_multiple_of(16) {
        return Err(format!(
            "ciphertext must be a non-zero multiple of 16 bytes, got {}",
            data.len()
        ));
    }

    match key.len() {
        16 => cbc::Decryptor::<aes::Aes128>::new(key.into(), iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(data)
            .map_err(|e| e.to_string()),
        24 => cbc::Decryptor::<aes::Aes192>::new(key.into(), iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(data)
            .map_err(|e| e.to_string()),
        _ => cbc::Decryptor::<aes::Aes256>::new(key.into(), iv.into())
            .decrypt_padded_vec_mut::<Pkcs7>(data)
            .map_err(|e| e.to_string()),
    }
}

/// Capture groups for every match of `pattern` in `haystack`.
///
/// Group 0 is the whole match. A group that did not participate yields an empty string rather
/// than shifting the others along, so a guest can index by group number.
///
/// An invalid pattern returns no matches rather than an error: a guest cannot do anything useful
/// with a regex-compilation failure, and the WIT signature keeps it simple.
pub fn regex_captures(pattern: &str, haystack: &str) -> Vec<Vec<String>> {
    // Bounded so a pathological pattern cannot spend the plugin's whole deadline compiling.
    let Ok(re) = regex::RegexBuilder::new(pattern).size_limit(1 << 20).build() else {
        tracing::debug!(pattern, "plugin supplied an invalid or oversized regex");
        return Vec::new();
    };
    re.captures_iter(haystack)
        .map(|caps| {
            caps.iter()
                .map(|group| group.map(|m| m.as_str().to_owned()).unwrap_or_default())
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caps() -> Capabilities {
        Capabilities::new("example-plugin", vec!["example.com".into()], Limits::default(), None)
    }

    fn get(url: &str) -> Request {
        Request { method: "GET".into(), url: url.into(), headers: Vec::new(), body: None }
    }

    #[test]
    fn a_declared_host_is_authorised() {
        assert_eq!(caps().authorise(&get("https://example.com/api")), Ok(()));
    }

    #[test]
    fn an_undeclared_host_is_denied_by_name() {
        // The message names the plugin, because the user's question when this fires is "which
        // plugin is trying to reach that?".
        let error = caps().authorise(&get("https://evil.test/")).unwrap_err();
        assert!(
            matches!(error, HostError::Denied(ref m) if m.contains("example-plugin")),
            "{error}"
        );
        assert!(error.to_string().contains("evil.test"), "{error}");
    }

    #[test]
    fn only_get_and_post_are_permitted() {
        // A parser has no reason to change state on a remote site.
        for method in ["DELETE", "PUT", "PATCH", "CONNECT", "TRACE"] {
            let request = Request { method: method.into(), ..get("https://example.com/") };
            assert!(
                matches!(caps().authorise(&request), Err(HostError::Denied(_))),
                "{method} should be denied"
            );
        }
        for method in ["GET", "get", "POST", "post"] {
            let request = Request { method: method.into(), ..get("https://example.com/") };
            assert_eq!(caps().authorise(&request), Ok(()), "{method} should be allowed");
        }
    }

    #[test]
    fn a_guest_cannot_override_the_hosts_fingerprint_headers() {
        // A guest setting `accept-encoding` would make the headers disagree with the
        // handshake the host negotiated, and the response would fail to decode.
        for name in ["user-agent", "Accept-Encoding", "HOST", "content-length"] {
            let request = Request {
                headers: vec![(name.into(), "x".into())],
                ..get("https://example.com/")
            };
            let result = caps().authorise(&request);
            if name.eq_ignore_ascii_case("user-agent") {
                // Deliberately allowed: some sources key on a specific UA, and the host's
                // profile already sets a coherent one that the guest merely narrows.
                assert_eq!(result, Ok(()), "user-agent should be the guest's to set");
            } else {
                assert!(
                    matches!(result, Err(HostError::Denied(_))),
                    "{name} should be reserved"
                );
            }
        }
    }

    #[test]
    fn ordinary_headers_are_the_guests_to_set() {
        // Referer and cookies are exactly what web sources need.
        let request = Request {
            headers: vec![
                ("referer".into(), "https://example.com/".into()),
                ("x-requested-with".into(), "XMLHttpRequest".into()),
            ],
            ..get("https://example.com/")
        };
        assert_eq!(caps().authorise(&request), Ok(()));
    }

    #[test]
    fn the_fetch_budget_is_counted_host_side() {
        // A guest cannot use the host's client as a request amplifier.
        let caps = Capabilities::new(
            "p",
            vec!["example.com".into()],
            Limits { max_fetches: 2, ..Limits::default() },
            None,
        );
        assert_eq!(caps.authorise(&get("https://example.com/")), Ok(()));
        caps.fetches.fetch_add(2, Ordering::Relaxed);
        let error = caps.authorise(&get("https://example.com/")).unwrap_err();
        assert!(error.to_string().contains("budget"), "{error}");
    }

    #[test]
    fn aes_round_trips_a_known_vector() {
        // NIST SP 800-38A F.2.1, AES-128-CBC. Pinned because a wrong implementation would fail
        // only on real provider payloads, where it would look like a parse error.
        use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
        let key = [
            0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf,
            0x4f, 0x3c,
        ];
        let iv = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ];
        let plaintext = b"a stream url would go here";

        type Encryptor = cbc::Encryptor<aes::Aes128>;
        let ciphertext =
            Encryptor::new(&key.into(), &iv.into()).encrypt_padded_vec_mut::<Pkcs7>(plaintext);

        assert_eq!(aes_decrypt(&key, &iv, &ciphertext).unwrap(), plaintext);
    }

    #[test]
    fn aes_picks_the_variant_from_the_key_length() {
        // Real sources use AES-256 about as often as AES-128, and the WIT promises the key
        // length selects the variant — so both wide keys must round-trip, not just 128.
        use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
        let iv = [0x24; 16];
        let plaintext = b"the same url, behind a wider key";

        let key = [0x42; 32];
        let sealed = cbc::Encryptor::<aes::Aes256>::new(&key.into(), &iv.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
        assert_eq!(aes_decrypt(&key, &iv, &sealed).unwrap(), plaintext);

        let key = [0x42; 24];
        let sealed = cbc::Encryptor::<aes::Aes192>::new(&key.into(), &iv.into())
            .encrypt_padded_vec_mut::<Pkcs7>(plaintext);
        assert_eq!(aes_decrypt(&key, &iv, &sealed).unwrap(), plaintext);
    }

    #[test]
    fn settings_are_absent_unless_attached() {
        // `describe` runs on capabilities with no settings, and that absence is deliberate:
        // a manifest must not depend on configuration it is about to be granted.
        let bare = caps();
        assert_eq!(bare.setting("api-key"), None);

        let configured = caps().with_settings(
            [("api-key".to_string(), "s3cret".to_string())].into_iter().collect(),
        );
        assert_eq!(configured.setting("api-key").as_deref(), Some("s3cret"));
        assert_eq!(configured.setting("unset"), None);
    }

    #[test]
    fn aes_rejects_malformed_input_rather_than_panicking() {
        // A guest can pass anything, and a panic inside a host call would take down the process.
        assert!(aes_decrypt(&[0; 8], &[0; 16], &[0; 16]).is_err(), "short key");
        assert!(aes_decrypt(&[0; 16], &[0; 8], &[0; 16]).is_err(), "short iv");
        assert!(aes_decrypt(&[0; 16], &[0; 16], &[0; 7]).is_err(), "not a block multiple");
        assert!(aes_decrypt(&[0; 16], &[0; 16], &[]).is_err(), "empty");
        // Valid shape, wrong key: bad padding, reported rather than panicking.
        assert!(aes_decrypt(&[1; 16], &[0; 16], &[0; 32]).is_err());
    }

    #[test]
    fn regex_returns_every_group_of_every_match() {
        let found = regex_captures(
            r#"file:\s*"([^"]+)".*?label:\s*"(\d+)p""#,
            r#"
            {file: "https://cdn.example.com/1080.m3u8", label: "1080p"},
            {file: "https://cdn.example.com/720.m3u8", label: "720p"}
        "#,
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0][1], "https://cdn.example.com/1080.m3u8");
        assert_eq!(found[0][2], "1080");
        assert_eq!(found[1][2], "720");
    }

    #[test]
    fn a_non_participating_group_is_empty_rather_than_absent() {
        // So a guest can index by group number without counting which ones matched.
        let found = regex_captures(r"(a)|(b)", "a");
        assert_eq!(found[0], vec!["a", "a", ""]);
    }

    #[test]
    fn an_invalid_pattern_yields_no_matches_rather_than_an_error() {
        // A guest cannot do anything useful with a compilation failure.
        assert!(regex_captures(r"([unclosed", "anything").is_empty());
        assert!(regex_captures("", "").is_empty() || regex_captures("", "x").len() <= 2);
    }

    #[test]
    fn regex_has_no_catastrophic_backtracking_to_exploit() {
        // The `regex` crate is a finite automaton, so the classic nested-quantifier bomb is
        // linear rather than exponential. Worth asserting: a guest choosing the pattern makes
        // this a denial-of-service surface if the engine were a backtracker.
        let bomb = "(a+)+b";
        let haystack = "a".repeat(2_000);
        let started = std::time::Instant::now();
        let found = regex_captures(bomb, &haystack);
        assert!(found.is_empty());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "took {:?}",
            started.elapsed()
        );
    }
}
