//! A stand-in for Kitsu under `/kitsu/`, beside the fake AniList: its password grant and the
//! little of its API the tracker reads, so a fixture build signs in to Kitsu without a password
//! ever leaving the machine.
//!
//! It knows one made-up account, [`USER`] with [`PASSWORD`], whose library is empty. Any other
//! user name or password is refused as Kitsu refuses one, `400` with `invalid_grant`.

use serde_json::json;

use super::Reply;

/// The fixture account's user name. Its email, [`EMAIL`], is taken too, as Kitsu takes either.
pub const USER: &str = "wayfarer";
pub const EMAIL: &str = "wayfarer@kitsu.example";
pub const PASSWORD: &str = "stardust";

/// The fixture account's Kitsu id.
const USER_ID: &str = "900";

/// What the access token the fake hands out reads, which every API call must carry.
const TOKEN: &str = "fixture-kitsu-token";

/// The answer for `path`, the part after `/kitsu`.
pub(super) fn answer(
    method: &str,
    path: &str,
    authorization: Option<&str>,
    body: &[u8],
) -> Reply {
    let route = path.split('?').next().unwrap_or_default();
    if route == "/api/oauth/token" && method == "POST" {
        return token(&form(body));
    }
    if !route.starts_with("/api/edge/") {
        return Reply::status(404, json!({ "errors": [{ "title": "Not Found" }] }));
    }
    if authorization != Some(&format!("Bearer {TOKEN}")) {
        return Reply::status(401, json!({ "errors": [{ "title": "Invalid token" }] }));
    }
    match (method, route) {
        ("GET", "/api/edge/users") => Reply::status(
            200,
            json!({ "data": [{ "id": USER_ID, "type": "users", "attributes": { "name": USER } }] }),
        ),
        // An empty library: no page to follow, no entry to update.
        ("GET", "/api/edge/library-entries") => {
            Reply::status(200, json!({ "data": [], "links": {} }))
        }
        // A push creates or updates an entry, which the fake keeps nowhere.
        ("POST" | "PATCH", route) if route.starts_with("/api/edge/library-entries") => {
            Reply::status(200, json!({ "data": { "id": "1", "type": "libraryEntries" } }))
        }
        _ => Reply::status(404, json!({ "errors": [{ "title": "Not Found" }] })),
    }
}

/// The password grant and the refresh grant, for the fixture account only.
fn token(form: &[(String, String)]) -> Reply {
    let field = |name: &str| form.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str());
    let granted = match field("grant_type") {
        Some("password") => {
            matches!(field("username"), Some(USER | EMAIL))
                && field("password") == Some(PASSWORD)
        }
        Some("refresh_token") => field("refresh_token") == Some("fixture-kitsu-refresh"),
        _ => false,
    };
    if !granted {
        return Reply::status(
            400,
            json!({
                "error": "invalid_grant",
                "error_description": "The provided authorization grant is invalid, expired, \
                    revoked, does not match the redirection URI used in the authorization \
                    request, or was issued to another client.",
            }),
        );
    }
    Reply::status(
        200,
        json!({
            "access_token": TOKEN,
            "refresh_token": "fixture-kitsu-refresh",
            "token_type": "Bearer",
            "expires_in": 2_592_000,
            "scope": "public",
        }),
    )
}

/// An `application/x-www-form-urlencoded` body's fields.
fn form(body: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

/// `+` as a space and `%XX` as its byte.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
                match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                    Some(byte) => {
                        out.push(byte);
                        i += 2;
                    }
                    None => out.push(b'%'),
                }
            }
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn a_form_decodes_as_a_browser_encodes_it() {
        let fields =
            form(b"grant_type=password&username=wayfarer%40kitsu.example&password=a+b%25");
        assert_eq!(fields[1], ("username".to_owned(), EMAIL.to_owned()));
        assert_eq!(fields[2], ("password".to_owned(), "a b%".to_owned()));
    }

    #[test]
    fn only_the_fixture_account_is_granted() {
        let pair = |user: &str, password: &str| {
            vec![
                ("grant_type".to_owned(), "password".to_owned()),
                ("username".to_owned(), user.to_owned()),
                ("password".to_owned(), password.to_owned()),
            ]
        };
        let status = |reply: Reply| match reply {
            Reply::Answer { status, body, .. } => (status, body),
            _ => panic!("an answer"),
        };
        assert_eq!(status(token(&pair(USER, PASSWORD))).0, 200);
        assert_eq!(status(token(&pair(EMAIL, PASSWORD))).0, 200);
        let (code, body) = status(token(&pair(USER, "nope")));
        assert_eq!((code, body["error"].clone()), (400, Value::from("invalid_grant")));
    }
}
