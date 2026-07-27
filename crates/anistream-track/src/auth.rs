//! Getting a token out of AniList and into the keychain.
//!
//! ## What AniList actually supports, measured rather than assumed
//!
//! The plan called for "OAuth2 loopback + PKCE". Probed directly against
//! `anilist.co/api/v2/oauth/token`:
//!
//! | Request | Result |
//! |---|---|
//! | `response_type=token` (implicit grant) | **`unsupported_grant_type`** |
//! | `authorization_code` + `code_verifier`, no secret | `invalid_client` |
//! | `authorization_code` + `client_id` + `client_secret` | `invalid_client` only for a *wrong* secret |
//!
//! So: **no implicit grant, no PKCE.** AniList supports exactly one flow — the authorization
//! code grant with a client secret. There is no public client to borrow either, so the user
//! registers their own client and both halves live in their config. That is not the same as
//! shipping a secret: it is *their* client, on *their* machine, and anistream never embeds one.
//!
//! The upside of being wrong about the implicit grant is that this is simpler. An authorization
//! code comes back in the **query string**, which the loopback listener sees directly — no
//! in-browser relay page is needed to fish a fragment out of `location.hash`.
//!
//! ```text
//!   authorize?response_type=code  ──►  browser  ──►  127.0.0.1:PORT/callback?code=…
//!                                                             │
//!                                          POST /oauth/token   ▼   (id + secret + code)
//!                                                    access_token, ~1 year
//! ```
//!
//! [`Flow::Paste`] exists for machines a browser cannot reach: AniList's own PIN page displays
//! the code, and the user pastes it for the same exchange.

use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const AUTHORIZE: &str = "https://anilist.co/api/v2/oauth/authorize";
const TOKEN: &str = "https://anilist.co/api/v2/oauth/token";

/// AniList's own copy-paste page, for [`Flow::Paste`].
pub const PIN_REDIRECT: &str = "https://anilist.co/api/v2/oauth/pin";

/// Default loopback port.
///
/// Fixed, not ephemeral: AniList matches the redirect URI against the registered one exactly,
/// so a random port would fail every time. Chosen high and unmemorable to reduce the chance of
/// colliding with something the user already runs.
pub const DEFAULT_PORT: u16 = 45_617;

/// How long to wait for the browser round trip before giving up.
///
/// Long enough to log in and read a consent screen, short enough that a misconfigured redirect
/// URL — by far the most likely failure — is diagnosed in a minute rather than five.
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("no client id configured — register an API client on AniList and set `client_id`")]
    NoClientId,
    #[error(
        "no client secret configured — AniList only supports the authorization code grant, \
         so `client_secret` is required too"
    )]
    NoClientSecret,
    #[error("could not listen on 127.0.0.1:{port}: {source}")]
    Listen {
        port: u16,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "timed out after {0:?} — nothing reached 127.0.0.1. The usual cause is the Redirect URL \
         registered on AniList not matching exactly; check it against `anistream --login-url`"
    )]
    TimedOut(Duration),
    #[error("authorization failed: {0}")]
    Denied(String),
    #[error("token exchange failed: {0}")]
    Exchange(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// How the authorization code gets back to us.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flow {
    /// Catch it on a local listener. Needs the port registered with AniList.
    #[default]
    Loopback,
    /// Show AniList's PIN page and let the user paste the code.
    Paste,
}

/// The URL to open in a browser.
pub fn authorize_url(client_id: &str, flow: Flow, port: u16) -> Result<String, AuthError> {
    if client_id.trim().is_empty() {
        return Err(AuthError::NoClientId);
    }
    let redirect = match flow {
        Flow::Loopback => redirect_uri(port),
        Flow::Paste => PIN_REDIRECT.to_owned(),
    };
    // `response_type=code`, because `token` returns `unsupported_grant_type` — see the module
    // docs. This is the only flow AniList has.
    Ok(format!(
        "{AUTHORIZE}?client_id={}&redirect_uri={}&response_type=code",
        urlencode(client_id.trim()),
        urlencode(&redirect)
    ))
}

/// The redirect URI for a given port. This exact string is what the user registers.
pub fn redirect_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/callback")
}

/// Wait for the browser to arrive at the callback, and return the authorization code.
///
/// One request, read straight off the request line: the code is a query parameter, so unlike a
/// fragment it reaches the server directly.
pub async fn wait_for_code(port: u16) -> Result<String, AuthError> {
    wait_for_code_from(port, "your tracker").await
}

/// Wait for the callback, naming the service on the page it serves.
///
/// The service name is worth threading through: with more than one tracker configured, a page that
/// says only "Signed in" leaves you wondering which one, and the browser tab is the last thing you
/// see before closing it.
pub async fn wait_for_code_from(port: u16, service: &str) -> Result<String, AuthError> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|source| AuthError::Listen { port, source })?;

    let deadline = tokio::time::Instant::now() + CALLBACK_TIMEOUT;
    loop {
        let Ok(accepted) = tokio::time::timeout_at(deadline, listener.accept()).await else {
            return Err(AuthError::TimedOut(CALLBACK_TIMEOUT));
        };
        let (mut socket, _) = accepted?;

        let mut reader = BufReader::new(&mut socket);
        let mut request_line = String::new();
        reader.read_line(&mut request_line).await?;

        // A browser also asks for /favicon.ico; answering that as if it were the callback would
        // end the flow with nothing.
        let query = request_line
            .split_whitespace()
            .nth(1)
            .and_then(|target| target.split_once('?').map(|(_, q)| q.to_owned()));

        let Some(query) = query else {
            respond(&mut socket, WAITING_PAGE).await?;
            continue;
        };

        if let Some(code) = extract_field(&query, "code") {
            respond(&mut socket, &done_page(service)).await?;
            return Ok(code);
        }
        if let Some(error) = extract_field(&query, "error") {
            respond(&mut socket, DENIED_PAGE).await?;
            return Err(AuthError::Denied(error));
        }
        respond(&mut socket, WAITING_PAGE).await?;
    }
}

/// Exchange an authorization code for an access token.
///
/// Takes the HTTP client rather than building one, so the exchange goes through the same
/// configured client — including its proxy settings — as everything else.
pub async fn exchange_code(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<String, AuthError> {
    if client_id.trim().is_empty() {
        return Err(AuthError::NoClientId);
    }
    if client_secret.trim().is_empty() {
        return Err(AuthError::NoClientSecret);
    }

    let response = http
        .post(TOKEN)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&serde_json::json!({
            "grant_type": "authorization_code",
            "client_id": client_id.trim(),
            "client_secret": client_secret.trim(),
            "redirect_uri": redirect_uri,
            "code": code.trim(),
        }))
        .send()
        .await
        .map_err(|e| AuthError::Exchange(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();

    if let Some(token) = parsed["access_token"].as_str().filter(|t| !t.is_empty()) {
        return Ok(token.to_owned());
    }

    // AniList's errors are actually informative, so they are passed through rather than
    // flattened into "sign-in failed".
    let message = parsed["message"]
        .as_str()
        .or_else(|| parsed["error"].as_str())
        .unwrap_or(&body)
        .to_owned();
    Err(AuthError::Exchange(format!("{status}: {message}")))
}

/// When an access token expires, read from the token itself.
///
/// AniList issues JWTs, so the expiry is already in hand — no request needed. The signature is
/// deliberately *not* verified: this is for telling the user "signed in until March", not for
/// deciding whether to trust the token. AniList decides that.
///
/// Returns `None` for anything that is not a JWT with an `exp` claim, which is the right answer
/// for "unknown" rather than a guess.
pub fn token_expiry(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let decoded = base64url_decode(payload)?;
    let claims: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    claims["exp"].as_i64()
}

/// Minimal unpadded base64url decoder.
///
/// Hand-written rather than pulling a dependency in for twenty lines used in exactly one place.
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    const fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
            b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
            b'-' => Some(62),
            b'_' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer = 0_u32;
    let mut bits = 0_u32;
    for byte in input.bytes() {
        // JWTs are unpadded, but tolerate padding rather than failing on it.
        if byte == b'=' {
            break;
        }
        buffer = (buffer << 6) | value(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/// Pull an authorization code out of whatever the user pasted.
///
/// Tolerant on purpose: the PIN page shows a bare code, but pasting the whole redirect URL out
/// of the address bar is an obvious thing to try and should also work.
pub fn extract_code(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    // A bare code: AniList's are long, and a paste of one has no delimiters at all.
    if !trimmed.contains('=') && !trimmed.contains('&') && !trimmed.contains('/') {
        return (trimmed.len() > 20).then(|| trimmed.to_owned());
    }

    let payload = trimmed
        .rsplit_once('?')
        .map(|(_, q)| q)
        .or_else(|| trimmed.rsplit_once('#').map(|(_, f)| f))
        .unwrap_or(trimmed);
    extract_field(payload, "code")
}

/// Read one `key=value` out of a URL-encoded pair list.
fn extract_field(payload: &str, key: &str) -> Option<String> {
    payload
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| urldecode(v))
        .filter(|v| !v.is_empty())
}

async fn respond(socket: &mut tokio::net::TcpStream, body: &str) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: text/html; charset=utf-8\r\n\
         content-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(response.as_bytes()).await?;
    socket.flush().await
}

///
/// `pub(crate)` because the MAL flow needs it too, and two copies of an encoder is exactly how one
/// of them ends up subtly different.
pub(crate) fn urlencode(input: &str) -> String {
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

fn urldecode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match u8::from_str_radix(&input[i + 1..i + 3], 16) {
                Ok(byte) => {
                    out.push(byte);
                    i += 3;
                }
                Err(_) => {
                    out.push(bytes[i]);
                    i += 1;
                }
            },
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

// The three pages the loopback listener serves.
//
// Self-contained by necessity: a sign-in page that fetched a stylesheet would look broken behind
// the very network problem someone might be signing in to fix. The obi bar appears here too —
// this is the one part of anistream that renders outside a terminal.

/// The page shown once a code has been captured.
///
/// Built rather than a constant so it can name the service — see `wait_for_code_from`.
fn done_page(service: &str) -> String {
    format!(
        concat!(
            "<!doctype html><meta charset=utf-8><title>anistream</title>",
            "<style>:root{{color-scheme:light dark}}",
            "body{{font:15px/1.6 ui-monospace,monospace;margin:12vh auto;max-width:30rem;",
            "padding:0 1.5rem}}p{{color:#8B90AD}}",
            ".obi{{display:inline-block;width:.35rem;height:1.1em;background:#F2A64B;",
            "vertical-align:-.15em;margin-right:.6rem}}</style>",
            "<h1><span class=obi></span>anistream</h1>",
            "<p>Signed in to {}. You can close this tab.</p>"
        ),
        // Escaped: the service name comes from a caller, and even in-crate that should not be able
        // to inject markup into a page served to a browser.
        service.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    )
}

const DENIED_PAGE: &str = concat!(
    "<!doctype html><meta charset=utf-8><title>anistream</title>",
    "<style>:root{color-scheme:light dark}",
    "body{font:15px/1.6 ui-monospace,monospace;margin:12vh auto;max-width:30rem;",
    "padding:0 1.5rem}p{color:#8B90AD}",
    ".obi{display:inline-block;width:.35rem;height:1.1em;background:#F2A64B;",
    "vertical-align:-.15em;margin-right:.6rem}</style>",
    "<h1><span class=obi></span>anistream</h1>",
    "<p>Authorisation was declined. Nothing was changed.</p>"
);

const WAITING_PAGE: &str = concat!(
    "<!doctype html><meta charset=utf-8><title>anistream</title>",
    "<style>:root{color-scheme:light dark}",
    "body{font:15px/1.6 ui-monospace,monospace;margin:12vh auto;max-width:30rem;",
    "padding:0 1.5rem}p{color:#8B90AD}",
    ".obi{display:inline-block;width:.35rem;height:1.1em;background:#F2A64B;",
    "vertical-align:-.15em;margin-right:.6rem}</style>",
    "<h1><span class=obi></span>anistream</h1>",
    "<p>Waiting for authorisation&hellip;</p>"
);

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT: &str = "47071";

    #[test]
    fn the_authorize_url_asks_for_a_code_not_a_token() {
        // Measured: `response_type=token` returns `unsupported_grant_type`. AniList has exactly
        // one flow, and a regression here would break sign-in entirely.
        let url = authorize_url(CLIENT, Flow::Loopback, DEFAULT_PORT).unwrap();
        assert!(url.contains("response_type=code"), "{url}");
        assert!(!url.contains("response_type=token"), "{url}");
        assert!(url.contains("client_id=47071"), "{url}");
    }

    #[test]
    fn the_redirect_uri_is_percent_encoded() {
        // Unencoded, the `://` and `/` truncate the parameter and AniList rejects the request.
        let url = authorize_url(CLIENT, Flow::Loopback, 45_617).unwrap();
        assert!(
            url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A45617%2Fcallback"),
            "{url}"
        );
    }

    #[test]
    fn the_redirect_uri_is_exactly_what_the_user_must_register() {
        // AniList matches this string character-for-character, so it is part of the setup
        // instructions and cannot drift.
        assert_eq!(redirect_uri(45_617), "http://127.0.0.1:45617/callback");
    }

    #[test]
    fn the_paste_flow_uses_anilists_own_pin_page() {
        let url = authorize_url(CLIENT, Flow::Paste, DEFAULT_PORT).unwrap();
        assert!(url.contains("oauth%2Fpin"), "{url}");
        assert!(!url.contains("127.0.0.1"), "the paste flow must not need a local port");
    }

    #[test]
    fn missing_credentials_fail_before_any_request() {
        // Registering a client is unavoidable, so saying so beats a browser tab showing an
        // AniList error page.
        assert!(matches!(authorize_url("", Flow::Loopback, 1), Err(AuthError::NoClientId)));
        assert!(matches!(authorize_url("  ", Flow::Loopback, 1), Err(AuthError::NoClientId)));
    }

    #[tokio::test]
    async fn an_exchange_without_a_secret_fails_locally() {
        // AniList only supports the code grant, so a missing secret is a configuration error
        // rather than something to discover from a 401.
        let http = reqwest::Client::new();
        let result =
            exchange_code(&http, CLIENT, "", "http://127.0.0.1:1/callback", "abc").await;
        assert!(matches!(result, Err(AuthError::NoClientSecret)));
    }

    #[test]
    fn a_code_is_found_in_a_callback_query() {
        assert_eq!(extract_field("code=abc123&state=x", "code").as_deref(), Some("abc123"));
    }

    #[test]
    fn a_whole_pasted_redirect_url_works() {
        let url = "http://127.0.0.1:45617/callback?code=def-456_ghi";
        assert_eq!(extract_code(url).as_deref(), Some("def-456_ghi"));
    }

    #[test]
    fn a_bare_pasted_code_works() {
        // What AniList's PIN page gives you is the code on its own.
        let code = "a".repeat(400);
        assert_eq!(extract_code(&code).as_deref(), Some(code.as_str()));
    }

    #[test]
    fn a_short_stray_paste_is_not_mistaken_for_a_code() {
        // "yes" or an accidental word must not be exchanged and then stored — the failure
        // would only surface later as a 401.
        assert_eq!(extract_code("yes"), None);
        assert_eq!(extract_code(""), None);
        assert_eq!(extract_code("   \n"), None);
        assert_eq!(extract_code("http://127.0.0.1:45617/callback?state=x"), None);
    }

    #[test]
    fn a_percent_encoded_code_is_decoded() {
        assert_eq!(extract_code("?code=a%2Bb%2Fc").as_deref(), Some("a+b/c"));
    }

    #[test]
    fn a_tokens_expiry_is_read_from_the_token_itself() {
        // AniList issues JWTs, so "signed in until when" needs no request. Payload here is
        // `{"exp":1893456000}` base64url-encoded, unpadded, as a real JWT would be.
        let token = "header.eyJleHAiOjE4OTM0NTYwMDB9.signature";
        assert_eq!(token_expiry(token), Some(1_893_456_000));
    }

    #[test]
    fn an_opaque_token_has_an_unknown_expiry_rather_than_a_guessed_one() {
        // Not every tracker issues JWTs, and inventing a date would be worse than saying
        // nothing.
        assert_eq!(token_expiry("just-an-opaque-string"), None);
        assert_eq!(token_expiry(""), None);
        assert_eq!(token_expiry("a.!!!not-base64!!!.c"), None);
        // Well-formed base64 that is not JSON.
        assert_eq!(token_expiry("a.aGVsbG8.c"), None);
    }

    #[test]
    fn the_base64_decoder_handles_every_padding_remainder() {
        // Unpadded base64url leaves 0, 2 or 3 characters over; getting the tail wrong would
        // truncate the JSON and silently lose the claim.
        assert_eq!(base64url_decode("aGVsbG8").as_deref(), Some(&b"hello"[..]));
        assert_eq!(base64url_decode("aGVsbG8h").as_deref(), Some(&b"hello!"[..]));
        assert_eq!(base64url_decode("aGVsbA").as_deref(), Some(&b"hell"[..]));
        // Padding is tolerated rather than rejected.
        assert_eq!(base64url_decode("aGVsbG8=").as_deref(), Some(&b"hello"[..]));
        // The URL-safe alphabet, which standard base64 would reject.
        assert!(base64url_decode("-_-_").is_some());
    }

    #[test]
    fn the_served_pages_are_self_contained() {
        for body in [done_page("AniList").as_str(), DENIED_PAGE, WAITING_PAGE] {
            assert!(!body.contains("http://"), "external reference in a served page");
            assert!(!body.contains("https://"), "external reference in a served page");
        }
    }

    #[tokio::test]
    async fn the_loopback_flow_captures_a_code_from_the_query_string() {
        // Drives the real listener over a real socket. Unlike a fragment, the code reaches the
        // server directly — which is why no in-browser relay is needed.
        let port = 45_941;
        let waiting = tokio::spawn(wait_for_code(port));
        tokio::time::sleep(Duration::from_millis(150)).await;

        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        socket
            .write_all(b"GET /callback?code=live-code-abc HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut page = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut socket, &mut page).await.unwrap();
        assert!(page.contains("Signed in"), "the done page was not served");

        let code = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("flow did not finish")
            .expect("task panicked")
            .expect("no code");
        assert_eq!(code, "live-code-abc");
    }

    #[tokio::test]
    async fn a_favicon_request_does_not_end_the_flow() {
        // Browsers ask for /favicon.ico unprompted. Treating that as the callback would end
        // sign-in with nothing and look like a timeout.
        let port = 45_942;
        let waiting = tokio::spawn(wait_for_code(port));
        tokio::time::sleep(Duration::from_millis(150)).await;

        let mut noise = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        noise.write_all(b"GET /favicon.ico HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n").await.unwrap();
        drop(noise);

        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        socket
            .write_all(b"GET /callback?code=after-noise HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();

        let code = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("flow did not finish")
            .expect("task panicked")
            .expect("no code");
        assert_eq!(code, "after-noise", "a favicon request consumed the callback");
    }

    #[tokio::test]
    async fn a_declined_authorisation_is_reported_as_a_decision() {
        // Pressing "Deny" must read as a choice, not as a broken flow.
        let port = 45_943;
        let waiting = tokio::spawn(wait_for_code(port));
        tokio::time::sleep(Duration::from_millis(150)).await;

        let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        socket
            .write_all(b"GET /callback?error=access_denied HTTP/1.1\r\nhost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();

        let result = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("flow did not finish")
            .expect("task panicked");
        assert!(matches!(result, Err(AuthError::Denied(ref e)) if e == "access_denied"));
    }

    #[tokio::test]
    async fn a_port_already_in_use_fails_clearly() {
        // The port is fixed because AniList matches the redirect exactly, which makes a
        // collision with another program a real possibility worth naming.
        let port = 45_944;
        let _held = tokio::net::TcpListener::bind(("127.0.0.1", port)).await.unwrap();
        assert!(matches!(wait_for_code(port).await, Err(AuthError::Listen { .. })));
    }
}
