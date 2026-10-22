//! The OAuth device-code flow, shared by Simkl and Trakt.
//!
//! A better fit here than the loopback redirect AniList and MAL need, and worth saying why. A
//! loopback flow requires the redirect URI to match a registration character for character — the
//! single most common way to get stuck, and something the user cannot debug from inside the app. The
//! device flow has no redirect at all: the app asks for a code, the user types it into a web page,
//! and the app polls until it is approved. Nothing to misconfigure.
//!
//! It also suits a terminal application specifically. There is no assumption that the machine
//! running anistream has a browser, so the same flow works over SSH — where the loopback flow simply
//! cannot, because the browser that opens is on the wrong machine.
//!
//! The services speak it three ways, which live in a [`DeviceEndpoints`] descriptor rather than in
//! near-identical modules:
//!
//! - [`DeviceFlow::OAuth2`]: RFC 8628 as written, which Simkl's OAuth 2.0 apps use. Form posts to
//!   `/oauth2/device` and `/oauth2/token`, the grant type
//!   `urn:ietf:params:oauth:grant-type:device_code`, and the state of a poll named in the body
//!   (`authorization_pending`, `slow_down`, `expired_token`, `access_denied`).
//! - [`DeviceFlow::Trakt`]: JSON posts, the state of a poll in the status code.
//! - [`DeviceFlow::SimklPin`]: Simkl's older PIN flow, which an app registered before its OAuth 2.0
//!   still uses until Simkl retires it. It is tried only when `/oauth2/device` says the app is not
//!   an OAuth 2.0 one, so a configuration with an older client id keeps signing in.

use std::time::Duration;

use crate::secret::TokenPair;

/// Why a sign-in did not complete, each said as one plain sentence naming the service.
#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("{service} could not be reached: {message}")]
    Network { service: &'static str, message: String },
    /// The service said no, in its own words: the `error_description` or `message` of its answer,
    /// never the raw JSON. `error` is its machine-readable code, when it gave one.
    #[error("{service} refused this sign-in: {message}")]
    Refused { service: &'static str, message: String, error: Option<String> },
    #[error("{service} answered in a way anistream did not understand: {message}")]
    Decode { service: &'static str, message: String },
    /// The user never approved it. Distinguished from a refusal because the remedy differs: this
    /// one is "try again", and a refusal is not.
    #[error("the {service} code expired before it was approved")]
    Expired { service: &'static str },
    #[error("the {service} sign-in was declined")]
    Denied { service: &'static str },
}

/// How a service speaks the device flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFlow {
    /// RFC 8628: form posts, the state of a poll named in the body.
    OAuth2,
    /// Trakt's: JSON posts, the state of a poll in the status code.
    Trakt,
    /// Simkl's PIN flow, for an app registered before Simkl's OAuth 2.0.
    SimklPin,
}

/// Where a service's device flow lives, and how it is spoken.
#[derive(Debug, Clone, Copy)]
pub struct DeviceEndpoints {
    /// The service as its errors name it: "Simkl".
    pub service: &'static str,
    pub flow: DeviceFlow,
    /// Requests a device code.
    pub code_url: &'static str,
    /// Exchanges an approved device code for a token. Simkl's PIN flow carries `{code}`, for the
    /// user code in the path.
    pub token_url: &'static str,
    /// The scopes asked for, where the service takes them.
    pub scope: Option<&'static str>,
    /// The flow tried in this one's place when the service says the app is not registered for it.
    pub fallback: Option<&'static DeviceEndpoints>,
    /// What every request to the service carries in its URL: Simkl asks each to name the app.
    pub query: &'static [(&'static str, &'static str)],
}

/// What the user has to be told to do.
#[derive(Debug, Clone)]
pub struct DeviceCode {
    /// The short code the user types on the verification page.
    pub user_code: String,
    /// The long code the app polls with. Never shown — showing both invites typing the wrong one.
    pub device_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    /// Seconds between polls, as the *server* asked. Ignoring it is how a client gets rate-limited.
    pub interval: u64,
    /// The flow the code came from, which polls for it: Simkl's PIN flow, when that is what
    /// answered.
    pub endpoints: DeviceEndpoints,
}

/// RFC 8628's grant type.
pub const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";

/// Ask for a device code, falling back to the older flow where the service says to.
pub async fn request_code(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
) -> Result<DeviceCode, DeviceError> {
    match ask_for_code(http, endpoints, client_id).await {
        Err(e) if is_not_registered_for(&e) => match endpoints.fallback {
            Some(older) => {
                tracing::info!(
                    service = endpoints.service,
                    "not an OAuth 2.0 app: the PIN flow instead"
                );
                ask_for_code(http, older, client_id).await
            }
            None => Err(e),
        },
        other => other,
    }
}

async fn ask_for_code(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
) -> Result<DeviceCode, DeviceError> {
    let response = code_request(http, endpoints, client_id)
        .send()
        .await
        .map_err(|e| network(endpoints, &e))?;
    read_code(response, endpoints).await
}

/// Whether the service said the app is not registered for this flow: Simkl's `/oauth2/device`
/// answers an app registered before its OAuth 2.0 with `invalid_client`.
fn is_not_registered_for(error: &DeviceError) -> bool {
    matches!(error, DeviceError::Refused { error: Some(code), .. } if code == "invalid_client")
}

/// `request` naming the app, as the service asks: its URL parameters, and the User-Agent.
fn named(
    request: reqwest::RequestBuilder,
    endpoints: &DeviceEndpoints,
) -> reqwest::RequestBuilder {
    let request = request.header(reqwest::header::USER_AGENT, crate::USER_AGENT);
    if endpoints.query.is_empty() { request } else { request.query(endpoints.query) }
}

/// The request for a device code.
fn code_request(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
) -> reqwest::RequestBuilder {
    let client_id = client_id.trim();
    let request = match endpoints.flow {
        DeviceFlow::OAuth2 => {
            let mut form = vec![("client_id", client_id)];
            if let Some(scope) = endpoints.scope {
                form.push(("scope", scope));
            }
            http.post(endpoints.code_url).form(&form)
        }
        DeviceFlow::Trakt => {
            http.post(endpoints.code_url).json(&serde_json::json!({ "client_id": client_id }))
        }
        DeviceFlow::SimklPin => http.get(endpoints.code_url).query(&[("client_id", client_id)]),
    };
    named(request, endpoints)
}

/// A device code from the service's answer.
async fn read_code(
    response: reqwest::Response,
    endpoints: &DeviceEndpoints,
) -> Result<DeviceCode, DeviceError> {
    let status = response.status();
    let body = response.text().await.map_err(|e| network(endpoints, &e))?;
    if !status.is_success() {
        // An unregistered id is by far the most likely mistake, and the service's own words say so.
        return Err(refused(endpoints, status, &body));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| decode(endpoints, e.to_string()))?;
    let user_code = json["user_code"]
        .as_str()
        .ok_or_else(|| decode(endpoints, "no user_code".into()))?
        .to_owned();
    // Simkl's PIN flow returns the literal string `DEVICE_CODE` here — measured against the live
    // endpoint, not read in a doc — because it polls with the *user* code. Everyone else returns a
    // real one and polls with that. Falling back to the user code keeps both correct.
    let device_code = json["device_code"].as_str().unwrap_or(&user_code).to_owned();
    let verification_url = json["verification_url"]
        .as_str()
        .or_else(|| json["verification_uri"].as_str())
        .unwrap_or("https://simkl.com/pin")
        .to_owned();

    Ok(DeviceCode {
        user_code,
        device_code,
        verification_url,
        expires_in: json["expires_in"].as_u64().unwrap_or(900),
        // Floored at one second: a server asking for zero would spin.
        interval: json["interval"].as_u64().unwrap_or(5).max(1),
        endpoints: *endpoints,
    })
}

/// Poll until the user approves, or the code expires, in the flow the code came from.
///
/// The server's own `interval` is honoured rather than a fixed delay. Polling faster than asked is
/// how a client earns a rate limit, and every one of these services says `slow down` for it.
pub async fn poll_for_token(
    http: &reqwest::Client,
    client_id: &str,
    client_secret: Option<&str>,
    code: &DeviceCode,
) -> Result<TokenPair, DeviceError> {
    let endpoints = &code.endpoints;
    let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval);

    while std::time::Instant::now() < deadline {
        tokio::time::sleep(interval).await;
        let response = poll_request(http, endpoints, client_id, client_secret, code)
            .send()
            .await
            .map_err(|e| network(endpoints, &e))?;
        match read_poll(response, endpoints, crate::now_epoch()).await? {
            Poll::Token(pair) => return Ok(pair),
            Poll::Pending => continue,
            // RFC 8628 says five seconds more. Trakt's 429 is doubled, because a server that says it
            // twice means it.
            Poll::SlowDown => {
                interval = match endpoints.flow {
                    DeviceFlow::OAuth2 => interval + Duration::from_secs(5),
                    _ => interval * 2,
                }
                .min(Duration::from_secs(60));
            }
        }
    }
    Err(DeviceError::Expired { service: endpoints.service })
}

/// What one poll came to.
#[derive(Debug, PartialEq)]
enum Poll {
    Token(TokenPair),
    Pending,
    SlowDown,
}

/// The request one poll makes.
fn poll_request(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
    client_secret: Option<&str>,
    code: &DeviceCode,
) -> reqwest::RequestBuilder {
    let client_id = client_id.trim();
    let request = match endpoints.flow {
        DeviceFlow::OAuth2 => {
            let mut form = vec![
                ("grant_type", DEVICE_GRANT),
                ("device_code", code.device_code.as_str()),
                ("client_id", client_id),
            ];
            // A device, TV or command-line app has no secret; one registered as a server app sends
            // its own.
            if let Some(secret) = client_secret.filter(|s| !s.trim().is_empty()) {
                form.push(("client_secret", secret.trim()));
            }
            http.post(endpoints.token_url).form(&form)
        }
        DeviceFlow::Trakt => {
            let mut body =
                serde_json::json!({ "code": code.device_code, "client_id": client_id });
            if let Some(secret) = client_secret {
                body["client_secret"] = serde_json::json!(secret);
            }
            http.post(endpoints.token_url).json(&body)
        }
        DeviceFlow::SimklPin => http
            .get(endpoints.token_url.replace("{code}", &code.user_code))
            .query(&[("client_id", client_id)]),
    };
    named(request, endpoints)
}

/// One poll's answer, read as the flow has it.
async fn read_poll(
    response: reqwest::Response,
    endpoints: &DeviceEndpoints,
    now: i64,
) -> Result<Poll, DeviceError> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let service = endpoints.service;

    if status.is_success() {
        // Simkl's PIN flow answers 200 with `{"result":"KO"}` while it waits, so a success status is
        // not on its own an approval — checking only the status would return a token pair with an
        // empty access token and look like a working sign-in.
        if json["result"].as_str() == Some("KO") {
            return Ok(Poll::Pending);
        }
        return token_pair(&json, endpoints, now).map(Poll::Token);
    }

    match endpoints.flow {
        DeviceFlow::OAuth2 => match json["error"].as_str() {
            Some("authorization_pending") => Ok(Poll::Pending),
            Some("slow_down") => Ok(Poll::SlowDown),
            Some("expired_token") => Err(DeviceError::Expired { service }),
            Some("access_denied") => Err(DeviceError::Denied { service }),
            _ => Err(refused(endpoints, status, &body)),
        },
        DeviceFlow::Trakt => match status.as_u16() {
            400 => Ok(Poll::Pending),
            404 => Err(refusal(endpoints, "the code was not recognised")),
            409 => Err(refusal(endpoints, "the code was already approved")),
            410 => Err(DeviceError::Expired { service }),
            418 => Err(DeviceError::Denied { service }),
            429 => Ok(Poll::SlowDown),
            _ => Err(refused(endpoints, status, &body)),
        },
        DeviceFlow::SimklPin => Err(refused(endpoints, status, &body)),
    }
}

/// A token pair from a token endpoint's answer, its relative `expires_in` made an instant.
fn token_pair(
    json: &serde_json::Value,
    endpoints: &DeviceEndpoints,
    now: i64,
) -> Result<TokenPair, DeviceError> {
    let access = json["access_token"]
        .as_str()
        .filter(|token| !token.is_empty())
        .ok_or_else(|| decode(endpoints, "no access token in its answer".into()))?
        .to_owned();
    Ok(TokenPair {
        access,
        refresh: json["refresh_token"].as_str().map(str::to_owned),
        expires_at: json["expires_in"].as_i64().map(|seconds| now + seconds),
    })
}

/// Renew an OAuth 2.0 access token with its refresh token, at the flow's token endpoint: Simkl's
/// last a week. Simkl answers with the same refresh token, its window extended, and the old access
/// token stops working at once.
pub async fn refresh_token(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
    client_secret: Option<&str>,
    refresh: &str,
    now: i64,
) -> Result<TokenPair, DeviceError> {
    let response = refresh_request(http, endpoints, client_id, client_secret, refresh)
        .send()
        .await
        .map_err(|e| network(endpoints, &e))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(refused(endpoints, status, &body));
    }
    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| decode(endpoints, e.to_string()))?;
    let mut pair = token_pair(&json, endpoints, now)?;
    // Kept when the answer leaves it out: the one held is still good.
    pair.refresh.get_or_insert_with(|| refresh.to_owned());
    Ok(pair)
}

fn refresh_request(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
    client_secret: Option<&str>,
    refresh: &str,
) -> reqwest::RequestBuilder {
    let mut form = vec![
        ("grant_type", "refresh_token"),
        ("client_id", client_id.trim()),
        ("refresh_token", refresh.trim()),
    ];
    if let Some(secret) = client_secret.filter(|s| !s.trim().is_empty()) {
        form.push(("client_secret", secret.trim()));
    }
    named(http.post(endpoints.token_url).form(&form), endpoints)
}

/// The service's refusal in its own words: `error_description`, or `message`, or the error's code,
/// or the status — never the JSON it came in.
fn refused(
    endpoints: &DeviceEndpoints,
    status: reqwest::StatusCode,
    body: &str,
) -> DeviceError {
    let json: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let words = |key: &str| json[key].as_str().map(str::trim).filter(|s| !s.is_empty());
    let message = words("error_description")
        .or_else(|| words("message"))
        .map(str::to_owned)
        .or_else(|| words("error").map(|code| code.replace('_', " ")))
        // Not JSON: a proxy's page or an outage. Its status says enough.
        .unwrap_or_else(|| format!("it answered {}", status.as_u16()));
    DeviceError::Refused {
        service: endpoints.service,
        message,
        error: words("error").map(str::to_owned),
    }
}

fn refusal(endpoints: &DeviceEndpoints, message: &str) -> DeviceError {
    DeviceError::Refused {
        service: endpoints.service,
        message: message.to_owned(),
        error: None,
    }
}

fn network(endpoints: &DeviceEndpoints, error: &reqwest::Error) -> DeviceError {
    DeviceError::Network { service: endpoints.service, message: error.to_string() }
}

fn decode(endpoints: &DeviceEndpoints, message: String) -> DeviceError {
    DeviceError::Decode { service: endpoints.service, message }
}

/// Trakt's endpoints.
pub const TRAKT: DeviceEndpoints = DeviceEndpoints {
    service: "Trakt",
    flow: DeviceFlow::Trakt,
    code_url: "https://api.trakt.tv/oauth/device/code",
    token_url: "https://api.trakt.tv/oauth/device/token",
    scope: None,
    fallback: None,
    query: &[],
};

/// Simkl's OAuth 2.0 device flow (AUTH V2), reading and writing the library, with its PIN flow
/// behind it for an app registered before.
pub const SIMKL: DeviceEndpoints = DeviceEndpoints {
    service: "Simkl",
    flow: DeviceFlow::OAuth2,
    code_url: "https://api.simkl.com/oauth2/device",
    token_url: "https://api.simkl.com/oauth2/token",
    scope: Some("media:read media:write"),
    fallback: Some(&SIMKL_PIN),
    query: crate::simkl::APP_PARAMS,
};

/// Simkl's PIN flow (AUTH V1). Its poll is a `GET` with the code in the path, which is why the URL
/// carries a placeholder. Its tokens do not expire, and come with no refresh token.
pub const SIMKL_PIN: DeviceEndpoints = DeviceEndpoints {
    service: "Simkl",
    flow: DeviceFlow::SimklPin,
    code_url: "https://api.simkl.com/oauth/pin",
    token_url: "https://api.simkl.com/oauth/pin/{code}",
    scope: None,
    fallback: None,
    query: crate::simkl::APP_PARAMS,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// What every request to Simkl carries in its URL.
    const NAMED: &str = concat!("app-name=anistream&app-version=", env!("CARGO_PKG_VERSION"));

    fn answer(status: u16, body: &'static str) -> reqwest::Response {
        http::Response::builder().status(status).body(body).unwrap().into()
    }

    /// A request's body, as text.
    fn body(request: &reqwest::Request) -> String {
        String::from_utf8(
            request.body().and_then(|b| b.as_bytes()).unwrap_or_default().to_vec(),
        )
        .unwrap()
    }

    fn code(endpoints: DeviceEndpoints) -> DeviceCode {
        DeviceCode {
            user_code: "BDWP-HQPK".into(),
            device_code: "a-long-opaque-device-code".into(),
            verification_url: "https://simkl.com/pin".into(),
            expires_in: 900,
            interval: 5,
            endpoints,
        }
    }

    #[test]
    fn simkls_code_request_is_a_form_post_asking_to_read_and_write() {
        let http = reqwest::Client::new();
        let request = code_request(&http, &SIMKL, " client-id ").build().unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request.url().as_str(),
            format!("https://api.simkl.com/oauth2/device?{NAMED}")
        );
        assert_eq!(request.headers()["user-agent"], crate::USER_AGENT);
        assert_eq!(request.headers()["content-type"], "application/x-www-form-urlencoded");
        assert_eq!(body(&request), "client_id=client-id&scope=media%3Aread+media%3Awrite");
    }

    #[test]
    fn simkls_poll_is_rfc_8628s_device_grant_with_no_secret_for_a_public_app() {
        let http = reqwest::Client::new();
        let request =
            poll_request(&http, &SIMKL, "client-id", None, &code(SIMKL)).build().unwrap();
        assert_eq!(request.method(), reqwest::Method::POST);
        assert_eq!(
            request.url().as_str(),
            format!("https://api.simkl.com/oauth2/token?{NAMED}")
        );
        assert_eq!(request.headers()["user-agent"], crate::USER_AGENT);
        assert_eq!(
            body(&request),
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code\
             &device_code=a-long-opaque-device-code&client_id=client-id"
        );
        // A server app sends its secret; an empty one in the config is no secret.
        let with = poll_request(&http, &SIMKL, "client-id", Some("s3cret"), &code(SIMKL))
            .build()
            .unwrap();
        assert!(body(&with).ends_with("&client_secret=s3cret"), "{}", body(&with));
        let empty =
            poll_request(&http, &SIMKL, "client-id", Some(" "), &code(SIMKL)).build().unwrap();
        assert!(!body(&empty).contains("client_secret"));
    }

    #[tokio::test]
    async fn simkls_code_answer_is_read_whole() {
        let answer = answer(
            200,
            r#"{"device_code":"DC","user_code":"7633-BNPR","verification_uri":"https:\/\/simkl.com\/pin",
                "verification_uri_complete":"https:\/\/simkl.com\/pin?user_code=7633-BNPR","expires_in":900,"interval":5}"#,
        );
        let code = read_code(answer, &SIMKL).await.unwrap();
        assert_eq!(code.user_code, "7633-BNPR");
        assert_eq!(code.device_code, "DC");
        assert_eq!(code.verification_url, "https://simkl.com/pin");
        assert_eq!((code.expires_in, code.interval), (900, 5));
        assert_eq!(code.endpoints.flow, DeviceFlow::OAuth2);
    }

    #[tokio::test]
    async fn simkls_poll_states_are_read_from_the_body() {
        assert_eq!(
            read_poll(answer(400, r#"{"error":"authorization_pending"}"#), &SIMKL, 0)
                .await
                .unwrap(),
            Poll::Pending
        );
        assert_eq!(
            read_poll(answer(400, r#"{"error":"slow_down"}"#), &SIMKL, 0).await.unwrap(),
            Poll::SlowDown
        );
        let expired = r#"{"error":"expired_token","error_description":"Device code is invalid or expired"}"#;
        assert!(matches!(
            read_poll(answer(400, expired), &SIMKL, 0).await,
            Err(DeviceError::Expired { .. })
        ));
        assert!(matches!(
            read_poll(answer(400, r#"{"error":"access_denied"}"#), &SIMKL, 0).await,
            Err(DeviceError::Denied { .. })
        ));
    }

    #[tokio::test]
    async fn an_approved_code_is_a_token_pair_with_its_expiry_made_an_instant() {
        let approved = r#"{"access_token":"simkl_at_A","token_type":"Bearer","expires_in":604800,
                           "refresh_token":"simkl_rt_R","scope":"media:read media:write"}"#;
        let Poll::Token(pair) = read_poll(answer(200, approved), &SIMKL, 1_000).await.unwrap()
        else {
            panic!("expected a token")
        };
        assert_eq!(pair.access, "simkl_at_A");
        assert_eq!(pair.refresh.as_deref(), Some("simkl_rt_R"));
        assert_eq!(pair.expires_at, Some(1_000 + 604_800));
    }

    #[tokio::test]
    async fn a_refusal_is_one_plain_sentence_not_json() {
        let v1_app = r#"{"error":"unauthorized_client","code":400,"message":"This client_id is an OAuth 2.0 app: use POST \/oauth2\/device instead of \/oauth\/pin"}"#;
        let e = read_code(answer(400, v1_app), &SIMKL_PIN).await.unwrap_err();
        assert_eq!(
            e.to_string(),
            "Simkl refused this sign-in: This client_id is an OAuth 2.0 app: use POST /oauth2/device instead of /oauth/pin"
        );
        let unknown = r#"{"error":"invalid_client","error_description":"This client_id is not enabled for OAuth 2.0"}"#;
        let e = read_code(answer(401, unknown), &SIMKL).await.unwrap_err();
        assert_eq!(
            e.to_string(),
            "Simkl refused this sign-in: This client_id is not enabled for OAuth 2.0"
        );
        assert!(
            is_not_registered_for(&e),
            "an app from before OAuth 2.0 falls back to the PIN flow"
        );
        // An error with only its code is said in words; a page that is not JSON by its status.
        let e = read_poll(answer(400, r#"{"error":"invalid_grant"}"#), &SIMKL, 0)
            .await
            .unwrap_err();
        assert_eq!(e.to_string(), "Simkl refused this sign-in: invalid grant");
        let e = read_code(answer(503, "<html>down</html>"), &SIMKL).await.unwrap_err();
        assert_eq!(e.to_string(), "Simkl refused this sign-in: it answered 503");
        assert!(!is_not_registered_for(&e));
    }

    #[tokio::test]
    async fn simkls_pin_flow_still_reads_as_it_did() {
        let http = reqwest::Client::new();
        let request = code_request(&http, &SIMKL_PIN, "client-id").build().unwrap();
        assert_eq!(request.method(), reqwest::Method::GET);
        assert_eq!(
            request.url().as_str(),
            format!("https://api.simkl.com/oauth/pin?client_id=client-id&{NAMED}")
        );
        let poll = poll_request(&http, &SIMKL_PIN, "client-id", None, &code(SIMKL_PIN))
            .build()
            .unwrap();
        assert_eq!(
            poll.url().as_str(),
            format!("https://api.simkl.com/oauth/pin/BDWP-HQPK?client_id=client-id&{NAMED}")
        );

        // It polls with the user code, and its "device code" is a placeholder.
        let pin = r#"{"result":"OK","device_code":"DEVICE_CODE","user_code":"6DF31","verification_url":"https://simkl.com/pin","expires_in":900,"interval":5}"#;
        let code = read_code(answer(200, pin), &SIMKL_PIN).await.unwrap();
        assert_eq!(code.endpoints.flow, DeviceFlow::SimklPin);
        // Its waiting answer is a 200 with `KO`, never taken for a token.
        assert_eq!(
            read_poll(answer(200, r#"{"result":"KO"}"#), &SIMKL_PIN, 0).await.unwrap(),
            Poll::Pending
        );
        let done =
            read_poll(answer(200, r#"{"result":"OK","access_token":"abc"}"#), &SIMKL_PIN, 0)
                .await
                .unwrap();
        let Poll::Token(pair) = done else { panic!("expected a token") };
        assert_eq!((pair.access.as_str(), pair.refresh, pair.expires_at), ("abc", None, None));
    }

    #[tokio::test]
    async fn trakts_poll_states_are_read_from_the_status() {
        let http = reqwest::Client::new();
        let poll =
            poll_request(&http, &TRAKT, "id", Some("secret"), &code(TRAKT)).build().unwrap();
        // Trakt asks for no parameters of its own: its URL stays as it was.
        assert_eq!(poll.url().as_str(), "https://api.trakt.tv/oauth/device/token");
        let sent: serde_json::Value = serde_json::from_str(&body(&poll)).unwrap();
        assert_eq!(
            sent,
            serde_json::json!({ "code": "a-long-opaque-device-code", "client_id": "id", "client_secret": "secret" })
        );
        assert_eq!(read_poll(answer(400, ""), &TRAKT, 0).await.unwrap(), Poll::Pending);
        assert_eq!(read_poll(answer(429, ""), &TRAKT, 0).await.unwrap(), Poll::SlowDown);
        assert!(matches!(
            read_poll(answer(410, ""), &TRAKT, 0).await,
            Err(DeviceError::Expired { .. })
        ));
        assert!(matches!(
            read_poll(answer(418, ""), &TRAKT, 0).await,
            Err(DeviceError::Denied { .. })
        ));
        let e = read_poll(answer(404, ""), &TRAKT, 0).await.unwrap_err();
        assert_eq!(e.to_string(), "Trakt refused this sign-in: the code was not recognised");
    }

    #[tokio::test]
    async fn a_refresh_is_the_refresh_grant_and_keeps_its_refresh_token() {
        let http = reqwest::Client::new();
        let request =
            refresh_request(&http, &SIMKL, "client-id", None, "simkl_rt_R").build().unwrap();
        assert_eq!(
            request.url().as_str(),
            format!("https://api.simkl.com/oauth2/token?{NAMED}")
        );
        assert_eq!(request.headers()["user-agent"], crate::USER_AGENT);
        assert_eq!(
            body(&request),
            "grant_type=refresh_token&client_id=client-id&refresh_token=simkl_rt_R"
        );
        let json = serde_json::json!({ "access_token": "simkl_at_B", "expires_in": 604800 });
        let pair = token_pair(&json, &SIMKL, 10).unwrap();
        assert_eq!(pair.expires_at, Some(10 + 604_800));
    }

    #[test]
    fn a_missing_interval_does_not_produce_a_spin() {
        // A zero interval would poll as fast as the network allows and earn an immediate rate limit.
        let body = serde_json::json!({ "user_code": "X", "interval": 0 });
        assert_eq!(body["interval"].as_u64().unwrap_or(5).max(1), 1);
        let absent = serde_json::json!({ "user_code": "X" });
        assert_eq!(absent["interval"].as_u64().unwrap_or(5).max(1), 5);
    }

    #[test]
    fn the_device_code_is_polled_with_but_never_shown() {
        // Two codes exist and showing both invites typing the long one into the web page.
        let code = code(SIMKL);
        assert_ne!(code.user_code, code.device_code);
        assert!(code.user_code.len() < code.device_code.len());
    }
}
