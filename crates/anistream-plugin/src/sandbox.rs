//! What a plugin is allowed to do.
//!
//! A plugin registry is a supply-chain surface: a `.wasm` file dropped into a config directory
//! runs code the user did not write, to parse HTML from sites that change without notice. So the
//! limits here are not configuration, they are the contract — and every one of them is enforced
//! **host-side**, because a limit a guest could check for itself is a limit a compromised guest
//! ignores.
//!
//! Four things are bounded:
//!
//! | Limit | Why |
//! |---|---|
//! | [`is_allowed`] — hostnames | A parser has no business reaching anything but the site it parses. |
//! | [`Limits::memory_bytes`] | A guest that allocates without bound would take the process down. |
//! | [`Limits::deadline`] | A guest that loops forever must not wedge the UI. |
//! | no filesystem, no sockets | Not in the WIT world, and not in the linker — see [`crate::engine`] for what the empty WASI floor does and does not include. |
//!
//! The allowlist is the part most likely to be got wrong, so it is a pure function over strings
//! with the evasions written down as tests rather than as comments.

use std::time::Duration;

/// Resource ceilings for one plugin instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Linear-memory ceiling. Exceeding it fails the allocation inside the guest rather than
    /// growing the host's footprint.
    pub memory_bytes: usize,
    /// Wall-clock budget for a single call.
    ///
    /// Enforced by wasmtime's epoch interruption, which can stop a guest mid-loop — unlike a
    /// timeout on the future, which a spinning guest would never yield to.
    pub deadline: Duration,
    /// Ceiling on `fetch` calls per plugin call, so a guest cannot use the host's client as a
    /// request amplifier.
    pub max_fetches: u32,
    /// Ceiling on a single response body handed to a guest.
    pub max_response_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            // Generous for a parser, small enough that a runaway allocation is contained.
            memory_bytes: 64 * 1024 * 1024,
            // Long enough for a slow remote site, short enough that a hung plugin is noticed
            // rather than endured.
            deadline: Duration::from_secs(20),
            max_fetches: 12,
            max_response_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Whether a plugin declaring `allowed` may fetch `url`.
///
/// Deliberately strict and deliberately dull:
///
/// - **`http`/`https` only.** `file:`, `data:` and friends are not transport, they are ways to
///   read things a parser should not see.
/// - **Exact host match, or a subdomain of a declared host.** `cdn.example.com` is reachable
///   from `example.com`; `example.com.evil.test` is not, which is the trick a naive `ends_with`
///   would fall for.
/// - **No credentials in the URL.** Userinfo before an `@` is the classic way to make a URL look
///   like it points somewhere it does not, and a plugin has no reason to use it.
/// - **No loopback or link-local literals.** Defence in depth: the declared hosts are
///   user-visible, but a plugin should not be able to reach a service on the user's own machine
///   even if they approved a hostname that happens to resolve there.
pub fn is_allowed(url: &str, allowed: &[String]) -> bool {
    let Some(host) = host_of(url) else { return false };
    if is_local(&host) {
        return false;
    }
    allowed.iter().any(|pattern| host_matches(&host, pattern))
}

/// Extract a lowercase hostname from an absolute http(s) URL.
///
/// Returns `None` for anything that is not plainly one, including URLs carrying credentials —
/// rejecting is the right answer for input we do not fully understand.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .or_else(|| url.strip_prefix("HTTPS://"))
        .or_else(|| url.strip_prefix("HTTP://"))?;

    // Authority ends at the first `/`, `?` or `#`.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    // `user:pass@host` — refused rather than parsed, because the only reason to write one here is
    // to make the host look like something else.
    if authority.contains('@') {
        return None;
    }

    // Strip the port. An IPv6 literal is bracketed, so the last colon outside brackets is the
    // port separator.
    let host = match authority.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => authority.split(':').next().unwrap_or(""),
    };
    if host.is_empty() {
        return None;
    }

    // A trailing dot is a legal FQDN form that would defeat an exact comparison.
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    // Anything with a path separator or whitespace left in it is not a hostname.
    if host.is_empty() || host.contains(|c: char| c.is_whitespace()) {
        return None;
    }
    Some(host)
}

/// Whether `host` is the declared `pattern` or a subdomain of it.
fn host_matches(host: &str, pattern: &str) -> bool {
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if pattern.is_empty() {
        return false;
    }
    if host == pattern {
        return true;
    }
    // The dot is what makes this a subdomain check rather than a suffix check.
    host.strip_suffix(&pattern).is_some_and(|prefix| prefix.ends_with('.'))
}

/// Whether a hostname literal points at this machine or a private network.
///
/// Only catches literals, not names that resolve there — resolving would mean a DNS lookup
/// before the allowlist check, and the allowlist is meant to be cheap and total. This is the
/// second layer, not the only one.
fn is_local(host: &str) -> bool {
    if host == "localhost" || host.ends_with(".localhost") {
        return true;
    }
    if let Ok(v4) = host.parse::<std::net::Ipv4Addr>() {
        return v4.is_loopback()
            || v4.is_private()
            || v4.is_link_local()
            || v4.is_unspecified()
            || v4.is_broadcast();
    }
    if let Ok(v6) = host.parse::<std::net::Ipv6Addr>() {
        // `is_unique_local` and `is_unicast_link_local` are still unstable, so the prefixes are
        // checked directly: fc00::/7 and fe80::/10.
        let segments = v6.segments();
        return v6.is_loopback()
            || v6.is_unspecified()
            || (segments[0] & 0xfe00) == 0xfc00
            || (segments[0] & 0xffc0) == 0xfe80;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed() -> Vec<String> {
        vec!["example.com".into(), "cdn.other.test".into()]
    }

    #[test]
    fn a_declared_host_is_reachable() {
        assert!(is_allowed("https://example.com/api", &allowed()));
        assert!(is_allowed("http://example.com", &allowed()));
        assert!(is_allowed("https://example.com:8443/x?y=1", &allowed()));
    }

    #[test]
    fn a_subdomain_of_a_declared_host_is_reachable() {
        // Provider CDNs live on subdomains, and forcing every one to be declared would make
        // manifests wrong the first time a site adds an edge node.
        assert!(is_allowed("https://cdn.example.com/v.m3u8", &allowed()));
        assert!(is_allowed("https://a.b.example.com/", &allowed()));
    }

    #[test]
    fn a_suffix_that_is_not_a_subdomain_is_refused() {
        // The evasion a naive `ends_with` falls for, and the reason `host_matches` insists on
        // the dot.
        assert!(!is_allowed("https://example.com.evil.test/", &allowed()));
        assert!(!is_allowed("https://notexample.com/", &allowed()));
        assert!(!is_allowed("https://myexample.com/", &allowed()));
    }

    #[test]
    fn an_undeclared_host_is_refused() {
        assert!(!is_allowed("https://evil.test/", &allowed()));
        assert!(
            !is_allowed("https://other.test/", &allowed()),
            "only cdn.other.test was declared"
        );
    }

    #[test]
    fn the_allowed_host_appearing_in_the_path_does_not_help() {
        assert!(!is_allowed("https://evil.test/example.com/x", &allowed()));
        assert!(!is_allowed("https://evil.test/?u=https://example.com", &allowed()));
        assert!(!is_allowed("https://evil.test/#example.com", &allowed()));
    }

    #[test]
    fn credentials_in_the_url_are_refused_outright() {
        // `https://example.com@evil.test/` points at evil.test. Rather than parse that
        // correctly and hope, userinfo is rejected — a parser has no use for it.
        assert!(!is_allowed("https://example.com@evil.test/", &allowed()));
        assert!(!is_allowed("https://user:pass@example.com/", &allowed()));
    }

    #[test]
    fn case_and_trailing_dots_do_not_evade_the_check() {
        assert!(is_allowed("https://EXAMPLE.COM/x", &allowed()));
        assert!(is_allowed("HTTPS://Example.Com/x", &allowed()));
        assert!(
            is_allowed("https://example.com./x", &allowed()),
            "a trailing dot is a legal FQDN"
        );
        assert!(!is_allowed("https://example.com.evil.test./", &allowed()));
    }

    #[test]
    fn non_http_schemes_are_refused() {
        // `file:` and `data:` are not transport, they are ways to read things a parser should
        // never see.
        for url in [
            "file:///etc/passwd",
            "data:text/html,<script>",
            "ftp://example.com/",
            "ws://example.com/",
            "//example.com/",
            "example.com",
            "",
        ] {
            assert!(!is_allowed(url, &allowed()), "{url:?} should be refused");
        }
    }

    #[test]
    fn loopback_and_private_addresses_are_refused_even_if_declared() {
        // Defence in depth: a plugin must not be able to reach a service on the user's own
        // machine, including the app's own torrent stream server.
        let permissive = vec![
            "127.0.0.1".to_string(),
            "localhost".into(),
            "10.0.0.5".into(),
            "192.168.1.1".into(),
            "169.254.169.254".into(),
            "[::1]".into(),
        ];
        for url in [
            "http://127.0.0.1:62600/s/probe",
            "http://localhost:8080/",
            "http://10.0.0.5/",
            "http://192.168.1.1/",
            // The cloud metadata endpoint, the canonical SSRF target.
            "http://169.254.169.254/latest/meta-data/",
            "http://[::1]:80/",
            "http://[fe80::1]/",
            "http://[fc00::1]/",
            "http://0.0.0.0/",
            "http://sub.localhost/",
        ] {
            assert!(!is_allowed(url, &permissive), "{url:?} must be refused");
        }
    }

    #[test]
    fn an_empty_allowlist_reaches_nothing() {
        // A manifest that declares no hosts gets no network, rather than defaulting open.
        assert!(!is_allowed("https://example.com/", &[]));
    }

    #[test]
    fn a_blank_pattern_does_not_match_everything() {
        // An empty string in a manifest would otherwise be a wildcard.
        let sloppy = vec![String::new(), "   ".into()];
        assert!(!is_allowed("https://example.com/", &sloppy));
        assert!(!is_allowed("https://evil.test/", &sloppy));
    }

    #[test]
    fn host_extraction_handles_ports_and_ipv6() {
        assert_eq!(host_of("https://example.com:443/x").as_deref(), Some("example.com"));
        assert_eq!(
            host_of("http://[2606:4700::1111]:8080/").as_deref(),
            Some("2606:4700::1111")
        );
        assert_eq!(host_of("https://example.com").as_deref(), Some("example.com"));
        assert_eq!(host_of("https://"), None);
        assert_eq!(host_of("https://:8080/"), None);
    }

    #[test]
    fn a_public_ipv6_literal_is_allowed_when_declared() {
        // Rejecting all literals would be simpler but wrong: some CDNs are addressed directly.
        let declared = vec!["2606:4700::1111".to_string()];
        assert!(is_allowed("http://[2606:4700::1111]/x", &declared));
    }

    #[test]
    fn the_default_limits_are_bounded_on_every_axis() {
        // An unbounded axis is the one that gets exploited, so this asserts each is set at all.
        let limits = Limits::default();
        assert!(limits.memory_bytes > 0 && limits.memory_bytes <= 256 * 1024 * 1024);
        assert!(limits.deadline > Duration::ZERO && limits.deadline <= Duration::from_secs(60));
        assert!(limits.max_fetches > 0 && limits.max_fetches <= 64);
        assert!(limits.max_response_bytes > 0);
    }
}
