//! Invite links, `{base}/join/{code}#{key}`, and open links, `{base}/join/{code}?open#{key}`.
//!
//! The link is the whole trust anchor. It names the store, carries the code that buys a token,
//! and carries the store's public key, so the first contact with a store is already checked
//! against the key its owner sent rather than trusted on first use.
//!
//! An invite's code works once. An open link's works for anyone, as often as they like, while the
//! store lets anyone join. The two differ only by `?open`, so a client from before open links reads
//! one as an invite and asks the store, which answers in words that it needs a newer anistream.

use url::Url;

use crate::crypto::{self, PublicKey};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InviteError {
    #[error("that is not a link")]
    NotALink,
    #[error("that is not an invite link — it should contain /join/")]
    NotAnInvite,
    #[error("the invite code in that link is damaged")]
    BadCode,
    #[error(
        "the link is missing the store's key — copy the whole link, including what follows #"
    )]
    MissingKey,
    #[error("the store's key in that link is damaged")]
    BadKey,
    #[error("{0} is not HTTPS — stores are only reached over HTTPS, apart from this machine")]
    Insecure(String),
    /// `https://plugins.example@elsewhere/` reads as one host and goes to the other.
    #[error("that link has a user name in its address, which hides where it really goes")]
    Userinfo,
}

/// A parsed invite, or an open link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    /// The store's base URL, with no trailing slash.
    pub base: String,
    pub code: String,
    pub key: PublicKey,
    /// An open link, which anyone may join with, rather than a one-time invite.
    pub open: bool,
}

impl Invite {
    /// Read a pasted link.
    ///
    /// Forgiving about what comes with a paste — whitespace, a trailing newline, the angle brackets
    /// some mail clients wrap links in — and strict about everything that matters.
    pub fn parse(link: &str) -> Result<Self, InviteError> {
        let link = link.trim().trim_start_matches('<').trim_end_matches('>').trim();
        let url = Url::parse(link).map_err(|_| InviteError::NotALink)?;
        require_secure(&url)?;

        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|segment| !segment.is_empty()).collect())
            .unwrap_or_default();
        // The last `join` segment, so a store hosted under a path that itself contains "join"
        // still parses.
        let at = segments.iter().rposition(|s| *s == "join").ok_or(InviteError::NotAnInvite)?;
        let code = segments.get(at + 1).ok_or(InviteError::NotAnInvite)?;
        if at + 2 != segments.len() {
            return Err(InviteError::NotAnInvite);
        }
        if !crypto::is_code(code) {
            return Err(InviteError::BadCode);
        }

        let fragment =
            url.fragment().filter(|f| !f.is_empty()).ok_or(InviteError::MissingKey)?;
        let key = PublicKey::parse(fragment).map_err(|_| InviteError::BadKey)?;

        // `?open`, alone or among other parameters a chat app may have added.
        let open = url.query_pairs().any(|(name, _)| name == OPEN_MARK);

        let mut base = url.clone();
        base.set_fragment(None);
        base.set_query(None);
        base.set_path(&segments[..at].join("/"));
        Ok(Self { base: normalise_base(&base), code: (*code).to_owned(), key, open })
    }

    /// The link for this invite, or this open link.
    pub fn link(&self) -> String {
        let mark = if self.open { format!("?{OPEN_MARK}") } else { String::new() };
        format!("{}/join/{}{mark}#{}", self.base, self.code, self.key.encode())
    }
}

/// What marks an open link: the query parameter after its code.
pub const OPEN_MARK: &str = "open";

/// Parse a store's base URL as an owner gives it to the server.
pub fn parse_base(text: &str) -> Result<String, InviteError> {
    let url = Url::parse(text.trim()).map_err(|_| InviteError::NotALink)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(InviteError::NotALink);
    }
    Ok(normalise_base(&url))
}

/// Whether a client would agree to talk to a store at `base`.
pub fn check_base(base: &str) -> Result<(), InviteError> {
    let url = Url::parse(base.trim()).map_err(|_| InviteError::NotALink)?;
    require_secure(&url)
}

/// `scheme://host[:port][/path]`, with no trailing slash, query or fragment.
fn normalise_base(url: &Url) -> String {
    let mut base = url.clone();
    base.set_fragment(None);
    base.set_query(None);
    base.as_str().trim_end_matches('/').to_owned()
}

/// Refuse anything but HTTPS, apart from this machine.
///
/// A bearer token sent over plain HTTP is readable by everyone on the path, so the client never
/// does it. Loopback is the exception because nothing leaves the machine — which is also what
/// makes a store testable without certificates.
pub fn require_secure(url: &Url) -> Result<(), InviteError> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err(InviteError::Userinfo);
    }
    match url.scheme() {
        "https" => Ok(()),
        "http" if is_loopback(url) => Ok(()),
        "http" => {
            Err(InviteError::Insecure(url.host_str().unwrap_or("that address").to_owned()))
        }
        _ => Err(InviteError::NotALink),
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SigningKey;

    fn key() -> PublicKey {
        SigningKey::generate().unwrap().public()
    }

    fn link(base: &str, key: &PublicKey) -> (String, String) {
        let code = crypto::new_code().unwrap();
        (format!("{base}/join/{code}#{}", key.encode()), code)
    }

    #[test]
    fn a_link_parses_into_its_parts_and_back() {
        let key = key();
        let (text, code) = link("https://plugins.example", &key);
        let invite = Invite::parse(&text).unwrap();
        assert_eq!(invite.base, "https://plugins.example");
        assert_eq!(invite.code, code);
        assert_eq!(invite.key, key);
        assert_eq!(invite.link(), text);
    }

    #[test]
    fn an_open_link_is_an_invite_link_marked_open() {
        let key = key();
        let (text, code) = link("https://plugins.example", &key);
        let open = text.replace('#', "?open#");
        let parsed = Invite::parse(&open).unwrap();
        assert!(parsed.open);
        assert_eq!(parsed.code, code);
        assert_eq!(parsed.base, "https://plugins.example");
        assert_eq!(parsed.link(), open);
        assert!(!Invite::parse(&text).unwrap().open);
        // A parameter a chat app added on the way does not hide the mark, or count as one.
        assert!(Invite::parse(&text.replace('#', "?utm=x&open#")).unwrap().open);
        assert!(!Invite::parse(&text.replace('#', "?opened=1#")).unwrap().open);
    }

    #[test]
    fn a_store_under_a_path_keeps_its_path() {
        let key = key();
        let (text, _) = link("https://example.com/friends/anistream", &key);
        let invite = Invite::parse(&text).unwrap();
        assert_eq!(invite.base, "https://example.com/friends/anistream");
        assert_eq!(invite.link(), text);
    }

    #[test]
    fn a_port_is_kept() {
        let key = key();
        let (text, _) = link("https://plugins.example:8443", &key);
        assert_eq!(Invite::parse(&text).unwrap().base, "https://plugins.example:8443");
    }

    #[test]
    fn what_a_paste_brings_along_is_trimmed() {
        let key = key();
        let (text, _) = link("https://plugins.example", &key);
        assert!(Invite::parse(&format!("  {text}\n")).is_ok());
        assert!(Invite::parse(&format!("<{text}>")).is_ok());
    }

    #[test]
    fn plain_http_is_refused_except_on_this_machine() {
        let key = key();
        let (remote, _) = link("http://plugins.example", &key);
        assert_eq!(
            Invite::parse(&remote),
            Err(InviteError::Insecure("plugins.example".into()))
        );
        // A LAN address is not this machine.
        let (lan, _) = link("http://192.168.1.20:7878", &key);
        assert!(matches!(Invite::parse(&lan), Err(InviteError::Insecure(_))));

        for local in ["http://127.0.0.1:7878", "http://localhost:7878", "http://[::1]:7878"] {
            let (text, _) = link(local, &key);
            assert!(Invite::parse(&text).is_ok(), "{local} should be allowed");
        }
    }

    #[test]
    fn a_link_that_disguises_its_host_is_refused() {
        let key = key();
        let (text, _) = link("https://plugins.example@elsewhere.example", &key);
        assert_eq!(Invite::parse(&text), Err(InviteError::Userinfo));
        let (text, _) = link("https://user:pass@plugins.example", &key);
        assert_eq!(Invite::parse(&text), Err(InviteError::Userinfo));
        assert_eq!(check_base("https://a@plugins.example"), Err(InviteError::Userinfo));
    }

    #[test]
    fn a_link_without_the_key_is_refused_with_a_hint() {
        let key = key();
        let (text, _) = link("https://plugins.example", &key);
        let without = text.split('#').next().unwrap();
        assert_eq!(Invite::parse(without), Err(InviteError::MissingKey));
        assert_eq!(Invite::parse(&format!("{without}#")), Err(InviteError::MissingKey));
    }

    #[test]
    fn damaged_parts_are_named() {
        let key = key();
        let (text, code) = link("https://plugins.example", &key);
        assert_eq!(Invite::parse(&text.replace(&code, "abc")), Err(InviteError::BadCode));
        let cut = &text[..text.len() - 3];
        assert_eq!(Invite::parse(cut), Err(InviteError::BadKey));
        assert_eq!(Invite::parse("https://plugins.example/"), Err(InviteError::NotAnInvite));
        assert_eq!(
            Invite::parse(&text.replace("/join/", "/join/x/")),
            Err(InviteError::NotAnInvite)
        );
        assert_eq!(Invite::parse("plugins.example/join/x"), Err(InviteError::NotALink));
        assert_eq!(Invite::parse("ftp://plugins.example/join/x#y"), Err(InviteError::NotALink));
    }

    #[test]
    fn a_base_is_checked_the_way_a_client_checks_it() {
        assert!(check_base("https://plugins.example").is_ok());
        assert!(check_base("http://127.0.0.1:7878").is_ok());
        assert!(matches!(check_base("http://plugins.example"), Err(InviteError::Insecure(_))));
    }

    #[test]
    fn a_base_is_normalised() {
        assert_eq!(parse_base("https://plugins.example/").unwrap(), "https://plugins.example");
        assert_eq!(parse_base("https://example.com/a/b/").unwrap(), "https://example.com/a/b");
        assert_eq!(parse_base("https://example.com/a?x=1#y").unwrap(), "https://example.com/a");
        assert!(parse_base("not a url").is_err());
    }
}
