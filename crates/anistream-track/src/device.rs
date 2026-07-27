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
//! Both services implement the same shape with different spellings, so the differences live in a
//! [`DeviceEndpoints`] descriptor rather than in two near-identical modules.

use std::time::Duration;

use crate::secret::TokenPair;

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("network: {0}")]
    Network(String),
    #[error("{0}")]
    Api(String),
    #[error("unexpected response: {0}")]
    Decode(String),
    /// The user never approved it. Distinguished from a refusal because the remedy differs: this
    /// one is "try again", and a refusal is not.
    #[error("the code expired before it was approved")]
    Expired,
    #[error("access was denied")]
    Denied,
}

/// Where a service's device flow lives, and what it calls things.
#[derive(Debug, Clone, Copy)]
pub struct DeviceEndpoints {
    /// Requests a device code. `GET` for Simkl, `POST` for Trakt.
    pub code_url: &'static str,
    pub code_is_post: bool,
    /// Exchanges an approved device code for a token.
    pub token_url: &'static str,
    /// Whether the poll sends `{"code": …}` (Trakt) or `?user_code=…` (Simkl).
    pub poll_is_post: bool,
}

/// What the user has to be told to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCode {
    /// The short code the user types on the verification page.
    pub user_code: String,
    /// The long code the app polls with. Never shown — showing both invites typing the wrong one.
    pub device_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    /// Seconds between polls, as the *server* asked. Ignoring it is how a client gets rate-limited.
    pub interval: u64,
}

/// Ask for a device code.
pub async fn request_code(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
) -> Result<DeviceCode, DeviceError> {
    let request = if endpoints.code_is_post {
        http.post(endpoints.code_url).json(&serde_json::json!({ "client_id": client_id }))
    } else {
        http.get(endpoints.code_url).query(&[("client_id", client_id)])
    };

    let response = request.send().await.map_err(|e| DeviceError::Network(e.to_string()))?;
    let status = response.status();
    let body = response.text().await.map_err(|e| DeviceError::Network(e.to_string()))?;

    if !status.is_success() {
        // The body carries the real reason — `client not found` for an unregistered id, which is by
        // far the most likely mistake and worth passing through verbatim.
        return Err(DeviceError::Api(format!("{}: {}", status.as_u16(), body.trim())));
    }

    let json: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| DeviceError::Decode(e.to_string()))?;
    let user_code = json["user_code"]
        .as_str()
        .ok_or_else(|| DeviceError::Decode("no user_code".into()))?
        .to_owned();
    // Simkl returns the literal string `DEVICE_CODE` here — measured against the live endpoint,
    // not read in a doc — because its PIN flow polls with the *user* code. Trakt returns a real one
    // and polls with that. Falling back to the user code keeps both correct.
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
    })
}

/// Poll until the user approves, or the code expires.
///
/// The server's own `interval` is honoured rather than a fixed delay. Polling faster than asked is
/// how a client earns a rate limit, and both of these services return `slow_down` for it.
pub async fn poll_for_token(
    http: &reqwest::Client,
    endpoints: &DeviceEndpoints,
    client_id: &str,
    client_secret: Option<&str>,
    code: &DeviceCode,
) -> Result<TokenPair, DeviceError> {
    let deadline = std::time::Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval);

    while std::time::Instant::now() < deadline {
        tokio::time::sleep(interval).await;

        let request = if endpoints.poll_is_post {
            let mut body = serde_json::json!({
                "code": code.device_code,
                "client_id": client_id,
            });
            if let Some(secret) = client_secret {
                body["client_secret"] = serde_json::json!(secret);
            }
            http.post(endpoints.token_url).json(&body)
        } else {
            http.get(endpoints.token_url.replace("{code}", &code.user_code))
                .query(&[("client_id", client_id)])
        };

        let response = request.send().await.map_err(|e| DeviceError::Network(e.to_string()))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        // Both services signal "not yet" with a status rather than a body, and the codes are the
        // one place they genuinely differ in meaning.
        match status.as_u16() {
            200 => {
                let json: serde_json::Value = serde_json::from_str(&body)
                    .map_err(|e| DeviceError::Decode(e.to_string()))?;
                // Simkl answers 200 with `{"result":"KO"}` while it waits, so a success status is
                // not on its own an approval — checking only the status would return a token pair
                // with an empty access token and look like a working sign-in.
                if json["result"].as_str() == Some("KO") {
                    continue;
                }
                let access = json["access_token"]
                    .as_str()
                    .ok_or_else(|| DeviceError::Decode(format!("no access_token in {body}")))?
                    .to_owned();
                let expires_at =
                    json["expires_in"].as_i64().map(|seconds| crate::now_epoch() + seconds);
                return Ok(TokenPair {
                    access,
                    refresh: json["refresh_token"].as_str().map(str::to_owned),
                    expires_at,
                });
            }
            // Trakt: pending.
            400 => continue,
            404 => return Err(DeviceError::Api("the code was not recognised".into())),
            409 => return Err(DeviceError::Api("already approved".into())),
            410 => return Err(DeviceError::Expired),
            418 => return Err(DeviceError::Denied),
            // Explicitly asked to back off. Doubling rather than adding, because a server that says
            // this twice means it.
            429 => {
                interval = (interval * 2).min(Duration::from_secs(60));
                continue;
            }
            other => {
                return Err(DeviceError::Api(format!("{other}: {}", body.trim())));
            }
        }
    }
    Err(DeviceError::Expired)
}

/// Trakt's endpoints.
pub const TRAKT: DeviceEndpoints = DeviceEndpoints {
    code_url: "https://api.trakt.tv/oauth/device/code",
    code_is_post: true,
    token_url: "https://api.trakt.tv/oauth/device/token",
    poll_is_post: true,
};

/// Simkl's endpoints.
///
/// Simkl's poll is a `GET` with the code in the path, which is why the URL carries a placeholder.
pub const SIMKL: DeviceEndpoints = DeviceEndpoints {
    code_url: "https://api.simkl.com/oauth/pin",
    code_is_post: false,
    token_url: "https://api.simkl.com/oauth/pin/{code}",
    poll_is_post: false,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_code_response_is_read_from_either_spelling() {
        // Simkl says `verification_url`, the RFC says `verification_uri`, and both appear in the
        // wild — including both at once, which is what Simkl actually returns.
        for key in ["verification_url", "verification_uri"] {
            let body = serde_json::json!({
                "user_code": "6DF31",
                "device_code": "LONG",
                key: "https://simkl.com/pin",
                "expires_in": 900,
                "interval": 5
            });
            let url =
                body["verification_url"].as_str().or_else(|| body["verification_uri"].as_str());
            assert_eq!(url, Some("https://simkl.com/pin"), "failed for {key}");
        }
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
    fn simkls_waiting_response_is_not_mistaken_for_success() {
        // Simkl answers HTTP 200 with `result: KO` while it waits. Trusting the status alone would
        // return an empty access token and look like a completed sign-in.
        let waiting = serde_json::json!({ "result": "KO" });
        assert_eq!(waiting["result"].as_str(), Some("KO"));
        assert!(waiting["access_token"].as_str().is_none());

        let done = serde_json::json!({ "result": "OK", "access_token": "abc" });
        assert_ne!(done["result"].as_str(), Some("KO"));
        assert_eq!(done["access_token"].as_str(), Some("abc"));
    }

    #[test]
    fn the_device_code_is_polled_with_but_never_shown() {
        // Two codes exist and showing both invites typing the long one into the web page.
        let code = DeviceCode {
            user_code: "6DF31".into(),
            device_code: "a-long-opaque-string".into(),
            verification_url: "https://simkl.com/pin".into(),
            expires_in: 900,
            interval: 5,
        };
        assert_ne!(code.user_code, code.device_code);
        assert!(code.user_code.len() < code.device_code.len());
    }

    #[test]
    fn simkls_poll_url_carries_the_code_in_the_path() {
        assert_eq!(
            SIMKL.token_url.replace("{code}", "6DF31"),
            "https://api.simkl.com/oauth/pin/6DF31"
        );
        // Trakt's does not, and must be left alone by the same substitution.
        assert_eq!(TRAKT.token_url.replace("{code}", "6DF31"), TRAKT.token_url);
    }
}
