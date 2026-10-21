//! What the log may say about an address: its scheme, host and port, never the path or query,
//! which can carry a signed token.

/// `https://cdn.example`, from `https://cdn.example/a/b.m3u8?token=…`.
pub fn address(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else {
        return if url.starts_with('/') { "a local file".into() } else { "an address".into() };
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Credentials in the authority are not the host's.
    let host = host.rsplit('@').next().unwrap_or(host);
    format!("{scheme}://{host}")
}

#[cfg(test)]
mod tests {
    use super::address;

    #[test]
    fn an_address_keeps_its_host_and_drops_what_could_be_a_token() {
        assert_eq!(address("https://cdn.example/a/b.m3u8?token=secret"), "https://cdn.example");
        assert_eq!(address("http://127.0.0.1:7878/mend/abc"), "http://127.0.0.1:7878");
        assert_eq!(address("https://user:pass@cdn.example/x"), "https://cdn.example");
        assert_eq!(address("magnet:?xt=urn:btih:abc"), "an address");
        assert_eq!(address("/Users/someone/episode.mkv"), "a local file");
    }
}
