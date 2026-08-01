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
    let Some(parsed) = parse(url) else { return false };
    if is_local(&parsed) {
        return false;
    }
    let Some(host) = hostname(&parsed) else { return false };
    allowed.iter().any(|pattern| host_matches(&host, pattern))
}

/// Parse a URL **with the parser the request itself will use**.
///
/// This is the whole boundary. An allowlist that reads a URL its own way is not a check on where
/// the request goes, it is a check on a second opinion about the string — and the two disagree in
/// ways that are not obvious: WHATWG ends the authority at a backslash as well as a slash, so
/// `https://evil.test\.example.com/` is host `evil.test` with a path, while a hand-rolled split
/// sees a subdomain of `example.com`. It also normalises `2130706433`, `0x7f.0.0.1` and `127.1`
/// to `127.0.0.1`, which a strict dotted-quad reader does not recognise as an address at all.
/// Both readings pointed somewhere the plugin never declared.
///
/// Credentials are refused rather than parsed: the only reason to write `user@host` here is to
/// make the host look like something else.
fn parse(url: &str) -> Option<url::Url> {
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    parsed.host()?;
    Some(parsed)
}

/// The hostname to match against the allowlist: lowercase, unbracketed, trailing dot removed.
///
/// The parser renders an IPv6 host in its URL form, `[::1]`, but a manifest declares an address
/// the way a person writes one, so the brackets come off before matching.
fn hostname(parsed: &url::Url) -> Option<String> {
    let host = parsed.host_str()?;
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    // A trailing dot is a legal FQDN form that would defeat an exact comparison.
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// Extract a lowercase hostname from an absolute http(s) URL.
///
/// Returns `None` for anything that is not plainly one, including URLs carrying credentials.
pub fn host_of(url: &str) -> Option<String> {
    parse(url).as_ref().and_then(hostname)
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

/// Whether a URL points at this machine or a private network.
///
/// Takes the *parsed* host, so every spelling of an address has already been normalised to the
/// one the socket will use — `2130706433` and `0x7f.0.0.1` arrive here as `127.0.0.1`. Reading
/// the literal out of the string instead let all of those through, and a plugin declares its own
/// allowlist, so reaching loopback was one manifest line away.
///
/// Names that merely *resolve* to a local address are still not caught: that would mean a DNS
/// lookup before the check, and the allowlist is meant to be cheap and total. This is one layer.
fn is_local(parsed: &url::Url) -> bool {
    match parsed.host() {
        Some(url::Host::Domain(name)) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            name == "localhost" || name.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(v4)) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
        }
        Some(url::Host::Ipv6(v6)) => {
            // `is_unique_local` and `is_unicast_link_local` are still unstable, so the prefixes
            // are checked directly: fc00::/7 and fe80::/10. An IPv4-mapped address is the same
            // machine wearing a different hat, so it is unwrapped rather than trusted.
            let segments = v6.segments();
            v6.is_loopback()
                || v6.is_unspecified()
                || (segments[0] & 0xfe00) == 0xfc00
                || (segments[0] & 0xffc0) == 0xfe80
                || v6.to_ipv4_mapped().is_some_and(|v4| {
                    v4.is_loopback()
                        || v4.is_private()
                        || v4.is_link_local()
                        || v4.is_unspecified()
                        || v4.is_broadcast()
                })
        }
        None => false,
    }
}

#[cfg(test)]
mod escape_tests {
    use super::*;

    /// What the HTTP client will actually connect to, as opposed to what the allowlist read.
    ///
    /// The allowlist is only a boundary if those two agree, so the check is written against the
    /// same parser the request goes through rather than against our own reading of the string.
    fn real_host(url: &str) -> Option<String> {
        url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase))
    }

    #[test]
    fn the_host_checked_is_the_host_contacted() {
        // A URL the allowlist and the client read differently is an escape by construction. The
        // backslash is the one that matters: WHATWG treats it as an authority terminator for
        // special schemes, so `evil.test\.example.com` is `evil.test` with a path — while a
        // split on `/?#` alone sees a subdomain of `example.com` and waves it through.
        for url in [
            "https://evil.test\\.example.com/",
            "https://evil.test\\x.example.com/",
            "https://a\\.example.com:8080/x",
            "https://example.com/",
            "https://cdn.example.com/a?b#c",
        ] {
            let (Some(checked), Some(contacted)) = (host_of(url), real_host(url)) else {
                continue;
            };
            assert_eq!(
                checked, contacted,
                "{url}: allowlist inspected {checked:?} but the request goes to {contacted:?}"
            );
        }
    }

    #[test]
    fn every_spelling_of_a_local_address_is_refused() {
        // `is_local` parses with Rust's strict dotted-quad reader; the client parses with the
        // WHATWG one, which also accepts decimal, octal, hex and short forms. Each of these is
        // 127.0.0.1 to the socket, so each must be refused however it is written — a plugin
        // declares its own allowlist, so reaching loopback is one manifest line away otherwise.
        let declared =
            ["2130706433".to_string(), "0x7f.0.0.1".to_string(), "127.1".to_string()];
        for url in [
            "http://2130706433/s/probe",
            "http://0x7f.0.0.1/s/probe",
            "http://127.1/s/probe",
            "http://017700000001/s/probe",
            "http://[::ffff:127.0.0.1]/s/probe",
        ] {
            assert!(!is_allowed(url, &declared), "{url} reaches this machine and was allowed");
        }
    }
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
