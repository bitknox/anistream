//! Reading the address a person types, and the rule that keeps a password off the open internet.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

use crate::error::{JellyfinError, Result};

/// Jellyfin's default HTTP port, used when a plain address names none.
pub const DEFAULT_PORT: u16 = 8096;

/// An address read from what was typed: always with a scheme, never with a trailing slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    base: String,
    host: String,
}

impl Address {
    /// `jellyfin.home.example`, `192.168.1.20:8096`, `http://nas.local:8096/` or
    /// `https://media.mika.example/jellyfin`. Without a scheme, an address on the local network
    /// is tried over HTTP and the default port, and any other over HTTPS.
    ///
    /// An `http://` address that is not on the local network is refused: the password would
    /// travel unprotected.
    pub fn parse(typed: &str) -> Result<Self> {
        let typed = typed.trim();
        if typed.is_empty() {
            return Err(JellyfinError::BadAddress);
        }
        let with_scheme = if typed.contains("://") {
            typed.to_owned()
        } else {
            let host = typed.split(['/', ':']).next().unwrap_or(typed);
            let scheme = if is_private_host(host) { "http" } else { "https" };
            let port = (scheme == "http" && !typed.contains(':')).then_some(DEFAULT_PORT);
            match port {
                Some(port) => {
                    let (host, path) =
                        typed.split_once('/').map_or((typed, ""), |(h, p)| (h, p));
                    if path.is_empty() {
                        format!("{scheme}://{host}:{port}")
                    } else {
                        format!("{scheme}://{host}:{port}/{path}")
                    }
                }
                None => format!("{scheme}://{typed}"),
            }
        };
        let url = Url::parse(&with_scheme).map_err(|_| JellyfinError::BadAddress)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(JellyfinError::BadAddress);
        }
        let host = url.host_str().ok_or(JellyfinError::BadAddress)?.to_owned();
        if url.scheme() == "http" && !is_private_host(&host) {
            return Err(JellyfinError::PlainHttp);
        }
        let base = url.as_str().trim_end_matches('/').to_owned();
        Ok(Self { base, host })
    }

    /// The address as a base for paths: `http://192.168.1.20:8096`.
    pub fn base(&self) -> &str {
        &self.base
    }

    /// Just the host, such as `192.168.1.20`.
    pub fn host(&self) -> &str {
        &self.host
    }

    /// What the person is shown: the address without its scheme.
    pub fn display(&self) -> &str {
        self.base.split_once("://").map_or(&self.base, |(_, rest)| rest)
    }

    /// A path on the server: `address.url("/System/Info/Public")`.
    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
}

/// Whether a host is on a network the person runs: loopback, private and link-local addresses,
/// Tailscale's range (100.64.0.0/10), and names that only resolve locally.
pub fn is_private_host(host: &str) -> bool {
    let host = host.trim_matches(['[', ']']);
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(ip) => private_v4(ip),
            IpAddr::V6(ip) => private_v6(ip),
        };
    }
    let host = host.to_ascii_lowercase();
    // A name with no dot is the network's own (`nas`); these suffixes are never public DNS.
    !host.contains('.')
        || host.ends_with(".local")
        || host.ends_with(".lan")
        || host.ends_with(".home.arpa")
        || host.ends_with(".internal")
        || host == "localhost"
}

fn private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || (a == 100 && (64..=127).contains(&b))
}

fn private_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback() || (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_address_without_a_scheme_is_http_on_the_default_port() {
        let a = Address::parse(" 192.168.1.20 ").unwrap();
        assert_eq!(a.base(), "http://192.168.1.20:8096");
        assert_eq!(a.display(), "192.168.1.20:8096");
        assert_eq!(
            Address::parse("192.168.1.20:8920").unwrap().base(),
            "http://192.168.1.20:8920"
        );
        assert_eq!(Address::parse("nas.local/jf").unwrap().base(), "http://nas.local:8096/jf");
    }

    #[test]
    fn any_other_address_without_a_scheme_is_https() {
        let a = Address::parse("jellyfin.mika.example").unwrap();
        assert_eq!(a.base(), "https://jellyfin.mika.example");
        assert_eq!(a.host(), "jellyfin.mika.example");
        assert_eq!(
            a.url("/System/Info/Public"),
            "https://jellyfin.mika.example/System/Info/Public"
        );
    }

    #[test]
    fn plain_http_is_refused_off_the_local_network() {
        assert_eq!(
            Address::parse("http://jellyfin.mika.example"),
            Err(JellyfinError::PlainHttp)
        );
        assert_eq!(Address::parse("http://8.8.8.8:8096"), Err(JellyfinError::PlainHttp));
        assert!(Address::parse("http://100.101.102.103:8096").is_ok(), "Tailscale is private");
        assert!(Address::parse("http://[fd00::5]:8096").is_ok());
        assert!(Address::parse("https://jellyfin.mika.example/").is_ok());
    }

    #[test]
    fn nonsense_is_not_an_address() {
        for typed in ["", "   ", "ftp://nas.local", "http://"] {
            assert_eq!(Address::parse(typed), Err(JellyfinError::BadAddress), "{typed:?}");
        }
    }

    #[test]
    fn the_trailing_slash_goes() {
        assert_eq!(
            Address::parse("https://media.mika.example/").unwrap().base(),
            "https://media.mika.example"
        );
    }
}
