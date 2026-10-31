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
    http: Option<crate::HttpClient>,
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
        http: Option<crate::HttpClient>,
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
        self.send(http, request).await
    }

    /// Without `fetch` there is no client, so there is nothing to send with.
    #[cfg(not(feature = "fetch"))]
    async fn send(
        &self,
        http: &crate::HttpClient,
        _request: Request,
    ) -> Result<Response, HostError> {
        match *http {}
    }

    #[cfg(feature = "fetch")]
    async fn send(
        &self,
        http: &crate::HttpClient,
        request: Request,
    ) -> Result<Response, HostError> {
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

        let body = read_capped(response, self.limits.max_response_bytes).await?;
        Ok(Response { status, headers, body })
    }
}

#[cfg(feature = "fetch")]
/// A response's body, refused as soon as it is known to pass `ceiling` bytes: from its
/// Content-Length when it says one, else as it arrives. Read whole first, a body could fill the
/// host's memory with as much as the client's timeout let through, none of it counted against
/// the plugin's own memory ceiling.
async fn read_capped(response: wreq::Response, ceiling: usize) -> Result<Vec<u8>, HostError> {
    use futures::StreamExt;

    let too_large = |size: u64| {
        HostError::Denied(format!(
            "response of {size} bytes exceeds the {ceiling} byte ceiling"
        ))
    };
    let declared = response.content_length();
    if let Some(declared) = declared
        && declared > ceiling as u64
    {
        return Err(too_large(declared));
    }
    let mut body = Vec::with_capacity(declared.map_or(0, |n| n as usize));
    let mut chunks = response.bytes_stream();
    while let Some(chunk) = chunks.next().await {
        let chunk = chunk.map_err(|e| HostError::Transport(e.to_string()))?;
        if body.len() + chunk.len() > ceiling {
            return Err(too_large((body.len() + chunk.len()) as u64));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// Capture groups for every match of `pattern` in `haystack`.
///
/// Group 0 is the whole match. A group that did not participate yields an empty string rather
/// than shifting the others along, so a guest can index by group number.
///
/// An invalid pattern returns no matches rather than an error: a guest cannot do anything useful
/// with a regex-compilation failure, and the WIT signature keeps it simple.
/// Matches one `regex-captures` call may return.
///
/// A parser pulling stream URLs out of a page wants tens. Anything past this is a guest using
/// the host's allocator rather than parsing, and the answer is no longer useful either way.
const MAX_REGEX_MATCHES: usize = 10_000;

pub fn regex_captures(pattern: &str, haystack: &str) -> Vec<Vec<String>> {
    // Bounded so a pathological pattern cannot spend the plugin's whole deadline compiling.
    let Ok(re) = regex::RegexBuilder::new(pattern).size_limit(1 << 20).build() else {
        tracing::debug!(pattern, "plugin supplied an invalid or oversized regex");
        return Vec::new();
    };
    // Bounded output as well as bounded compilation. The pattern, the haystack and therefore
    // the number of matches are all the guest's to choose, and every match allocates on the
    // *host* side — where neither containment mechanism reaches. The memory limiter governs
    // guest linear memory, not this; and epoch interruption cannot fire while the guest is not
    // executing, which it is not for the duration of a host call. `a?` over a full 64 MiB of
    // guest memory matched at every byte and cost gigabytes of host heap.
    let mut out = Vec::new();
    for caps in re.captures_iter(haystack) {
        if out.len() >= MAX_REGEX_MATCHES {
            tracing::warn!(
                pattern,
                limit = MAX_REGEX_MATCHES,
                "truncating a plugin's regex results"
            );
            break;
        }
        out.push(
            caps.iter()
                .map(|group| group.map(|m| m.as_str().to_owned()).unwrap_or_default())
                .collect(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body with no length that never ends is refused once it passes the ceiling, rather than
    /// read until the client's timeout.
    #[cfg(feature = "fetch")]
    #[tokio::test]
    async fn a_body_past_the_ceiling_is_refused_as_it_arrives() {
        let endless = futures::stream::repeat_with(|| Ok::<_, std::io::Error>(vec![0u8; 1024]));
        let response: wreq::Response =
            http::Response::new(wreq::Body::wrap_stream(endless)).into();
        let error = read_capped(response, 4 * 1024).await.unwrap_err();
        assert!(
            matches!(error, HostError::Denied(ref m) if m.contains("4096 byte")),
            "{error}"
        );

        let small: wreq::Response = http::Response::new(wreq::Body::from(vec![1u8; 10])).into();
        assert_eq!(read_capped(small, 4 * 1024).await.unwrap(), vec![1u8; 10]);
    }

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
    fn a_pattern_that_matches_everywhere_does_not_allocate_without_bound() {
        // The pattern and the haystack are both the guest's, and every match allocates on the
        // *host* side — where neither containment mechanism reaches. The memory limiter
        // governs guest linear memory, and epoch interruption cannot fire while the guest is
        // not executing, which it is not for the whole of a host call. `a?` over a guest's
        // full 64 MiB matched at every byte and cost gigabytes of host heap.
        let haystack = "a".repeat(MAX_REGEX_MATCHES * 2);
        let found = regex_captures("a?", &haystack);
        assert_eq!(found.len(), MAX_REGEX_MATCHES, "output must be bounded");
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
