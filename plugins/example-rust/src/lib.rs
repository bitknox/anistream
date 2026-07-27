//! A reference provider plugin.
//!
//! This exists to be read. It exercises every part of the ABI — the manifest, the lent `fetch`,
//! the lent regex, the error vocabulary — so the shape of a real provider is visible without
//! needing a volatile live source to be reachable.
//!
//! It talks to `httpbin.org`, which is a stable public echo service rather than an anime source.
//! That is deliberate: a reference implementation whose tests fail because someone else's site
//! changed its markup teaches nothing.
//!
//! **The thing to notice:** there is no HTTP client here, no TLS, no async runtime. The whole
//! plugin is a parser. `host::fetch` does the networking, which is why this compiles to a few
//! kilobytes and inherits anistream's browser fingerprint for free.

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "../../wit/anistream-provider.wit",
        world: "plugin",
    });
}

use bindings::{
    anistream::provider::host,
    exports::anistream::provider::provider::{
        Episode, Guest, Manifest, MediaStream, ProviderError, SearchHit, Subtitle,
    },
};

struct Component;

/// The one host this plugin may reach. Declared here and enforced by the host, so asking for more
/// than it needs would be visible in `anistream plugin inspect`.
const ALLOWED_HOST: &str = "httpbin.org";

impl Guest for Component {
    fn describe() -> Manifest {
        Manifest {
            id: "example-rust".into(),
            display_name: "Example (Rust)".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            allowed_hosts: vec![ALLOWED_HOST.into()],
            translation_types: vec!["sub".into(), "dub".into()],
        }
    }

    fn search(query: String, translation: String) -> Result<Vec<SearchHit>, ProviderError> {
        host::log("debug", &format!("search {query:?} ({translation})"));

        // A real provider would request the site's search endpoint. Here the query is echoed
        // back, which still exercises the whole round trip: request construction, the host's
        // allowlist check, the response body arriving as bytes.
        let body = get(&format!("https://{ALLOWED_HOST}/get?q={}", encode(&query)))?;

        // The lent regex, rather than a bundled one. A parser that had to carry its own regex
        // engine would be an order of magnitude larger than this whole component.
        let echoed = host::regex_captures(r#""q":\s*"([^"]*)""#, &body)
            .into_iter()
            .next()
            .and_then(|groups| groups.into_iter().nth(1))
            .ok_or_else(|| ProviderError::Parse("no query echoed back".into()))?;

        Ok(vec![SearchHit {
            id: format!("example:{echoed}"),
            title: format!("Echo of {echoed}"),
            episode_count: Some(3),
            year: Some(2026),
        }])
    }

    fn list_episodes(id: String, _translation: String) -> Result<Vec<Episode>, ProviderError> {
        // `not-found` rather than an empty list: the distinction is load-bearing in the host,
        // where `not-found` deliberately does *not* trigger failover to the next provider.
        let slug = id.strip_prefix("example:").ok_or(ProviderError::NotFound)?;
        host::log("debug", &format!("episodes for {slug}"));

        Ok((1..=3)
            .map(|n| Episode {
                number: n.to_string(),
                title: Some(format!("{slug} — part {n}")),
                duration_secs: Some(1_440),
            })
            .collect())
    }

    fn resolve(
        id: String,
        episode: String,
        translation: String,
    ) -> Result<Vec<MediaStream>, ProviderError> {
        let slug = id.strip_prefix("example:").ok_or(ProviderError::NotFound)?;

        // Demonstrates the lent AES: several real sources wrap their stream URL in AES-128-CBC
        // with a key found elsewhere on the page. Here a known vector stands in for that, which
        // proves the capability works end to end rather than merely being linked.
        let key = b"anistream-demo!!";
        let iv = b"0123456789abcdef";
        let sealed = SEALED_URL;
        let decrypted = host::aes_decrypt(key, iv, sealed)
            .map_err(|e| ProviderError::Parse(format!("aes: {e}")))?;
        let url = String::from_utf8(decrypted)
            .map_err(|_| ProviderError::Parse("decrypted payload was not utf-8".into()))?;

        host::log("info", &format!("resolved {slug} ep {episode} ({translation})"));

        Ok(vec![MediaStream {
            url,
            kind: "hls".into(),
            quality: Some(1080),
            // Referer-locked CDNs return 403 without this, which is why headers travel with the
            // stream rather than being the player's guess.
            headers: vec![("referer".into(), format!("https://{ALLOWED_HOST}/"))],
            subtitles: vec![Subtitle {
                language: "eng".into(),
                url: format!("https://{ALLOWED_HOST}/anything/{slug}.vtt"),
                hard: false,
            }],
        }])
    }
}

/// `https://cdn.example.test/master.m3u8` under AES-128-CBC with the key and iv in `resolve`.
///
/// Baked in as bytes so the demonstration needs no network: the point is that the *host* holds
/// the crypto, not that this particular payload is interesting.
const SEALED_URL: &[u8] = &[
    0x98, 0xe2, 0x0c, 0xda, 0xa0, 0x6e, 0xce, 0x05, 0xbb, 0x5f, 0x6f, 0x31, 0x13, 0x71, 0xa4,
    0xe2, 0x91, 0xfd, 0xbc, 0xc7, 0xbb, 0xc5, 0xf0, 0xa6, 0xfb, 0x55, 0x0d, 0xfc, 0x58, 0xff,
    0x26, 0x9e, 0x26, 0x74, 0x61, 0x04, 0xfb, 0x4b, 0x14, 0xb6, 0x68, 0xcc, 0xb0, 0x28, 0x5c,
    0x24, 0x15, 0x0a,
];

/// A GET through the host, with the host's failures translated into ours.
fn get(url: &str) -> Result<String, ProviderError> {
    let response = host::fetch(&host::HttpRequest {
        method: "GET".into(),
        url: url.into(),
        headers: vec![("accept".into(), "application/json".into())],
        body: None,
    })
    .map_err(|error| match error {
        // A denial means the manifest and the code disagree — a bug in the plugin, not a site
        // being down, so it is reported as such rather than as `blocked`.
        host::HostError::Denied(m) => ProviderError::Other(format!("denied by the host: {m}")),
        host::HostError::Timeout => ProviderError::Blocked("timed out".into()),
        host::HostError::Transport(m) => ProviderError::Blocked(m),
    })?;

    match response.status {
        200..=299 => String::from_utf8(response.body)
            .map_err(|_| ProviderError::Parse("response was not utf-8".into())),
        404 => Err(ProviderError::NotFound),
        // The signature source failure, and the one the registry fails over on.
        403 | 429 => Err(ProviderError::Blocked(format!("status {}", response.status))),
        status => Err(ProviderError::Other(format!("unexpected status {status}"))),
    }
}

/// Percent-encode a query value.
///
/// Hand-rolled because pulling a URL crate into a component to escape one parameter would be
/// most of the binary.
fn encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

bindings::export!(Component with_types_in bindings);
