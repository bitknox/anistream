//! The Jellyfin HTTP API, as far as anistream uses it.

use anistream_net::HttpClient;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::{Serialize, de::DeserializeOwned};

use crate::{
    address::Address,
    error::{JellyfinError, Result},
    model::{
        AuthenticationResult, EpisodeItem, Items, Library, PlaybackInfo, QuickConnect, Series,
        ServerInfo, Session, TICKS_PER_SECOND,
    },
};

/// The oldest server this works with. Quick Connect and the current auth header need it.
pub const MINIMUM_VERSION: (u32, u32) = (10, 8);

/// Who is asking, as Jellyfin's dashboard lists the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientInfo {
    pub device: String,
    pub device_id: String,
    pub version: String,
}

/// A server, and optionally who is signed in to it.
#[derive(Clone)]
pub struct Jellyfin {
    http: reqwest::Client,
    address: Address,
    info: ClientInfo,
    session: Option<Session>,
}

impl std::fmt::Debug for Jellyfin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the token.
        f.debug_struct("Jellyfin").field("address", &self.address.display()).finish_non_exhaustive()
    }
}

/// Whether `version` (`10.9.11`) is at least [`MINIMUM_VERSION`].
pub fn version_supported(version: &str) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u32>().ok());
    let (Some(Some(major)), Some(Some(minor))) = (parts.next(), parts.next()) else {
        return false;
    };
    (major, minor) >= MINIMUM_VERSION
}

impl Jellyfin {
    pub fn new(http: &HttpClient, address: Address, info: ClientInfo) -> Self {
        Self { http: http.plain().clone(), address, info, session: None }
    }

    /// The same server, signed in with a token that was kept.
    pub fn with_session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    pub fn address(&self) -> &Address {
        &self.address
    }

    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// The `Authorization` header: who is asking, and the token once there is one.
    fn authorization(&self) -> String {
        let mut value = format!(
            "MediaBrowser Client=\"anistream\", Device=\"{}\", DeviceId=\"{}\", Version=\"{}\"",
            clean(&self.info.device),
            clean(&self.info.device_id),
            clean(&self.info.version),
        );
        if let Some(session) = &self.session {
            value.push_str(&format!(", Token=\"{}\"", clean(&session.token)));
        }
        value
    }

    /// The header pair a player sends to read from the server.
    pub fn player_headers(&self) -> Vec<(String, String)> {
        vec![("Authorization".into(), self.authorization())]
    }

    fn request(&self, method: Method, path: &str) -> RequestBuilder {
        self.http
            .request(method, self.address.url(path))
            .header("Authorization", self.authorization())
            .header("Accept", "application/json")
    }

    async fn send(&self, request: RequestBuilder) -> Result<Response> {
        let response = request.send().await.map_err(|e| self.transport(&e))?;
        match response.status() {
            status if status.is_success() => Ok(response),
            StatusCode::UNAUTHORIZED if self.session.is_some() => Err(JellyfinError::SignedOut),
            status => Err(JellyfinError::Status { status: status.as_u16() }),
        }
    }

    fn transport(&self, error: &reqwest::Error) -> JellyfinError {
        // The certificate is the one failure with its own step.
        let mut source: Option<&dyn std::error::Error> = Some(error);
        while let Some(e) = source {
            let text = e.to_string().to_lowercase();
            if text.contains("certificate") || text.contains("unknownissuer") {
                return JellyfinError::UntrustedCertificate { host: self.address.host().to_owned() };
            }
            source = e.source();
        }
        JellyfinError::Unreachable { address: self.address.display().to_owned() }
    }

    async fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        let response = self.send(request).await?;
        response.json::<T>().await.map_err(|e| JellyfinError::Decode(e.to_string()))
    }

    /// Ask who answers at the address: its name and version. Fails with
    /// [`JellyfinError::NotJellyfin`] for anything else, and [`JellyfinError::TooOld`] for a server
    /// older than 10.8.
    pub async fn public_info(&self) -> Result<ServerInfo> {
        let response = self.send(self.request(Method::GET, "/System/Info/Public")).await;
        let response = match response {
            Err(JellyfinError::Status { .. }) => return Err(JellyfinError::NotJellyfin),
            other => other?,
        };
        let info: ServerInfo =
            response.json().await.map_err(|_| JellyfinError::NotJellyfin)?;
        if info.id.is_empty() || info.version.is_empty() {
            return Err(JellyfinError::NotJellyfin);
        }
        if !version_supported(&info.version) {
            return Err(JellyfinError::TooOld { version: info.version });
        }
        Ok(info)
    }

    /// Sign in with a user name and password. The password is not kept.
    pub async fn sign_in(&self, user: &str, password: &str) -> Result<Session> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Body<'a> {
            username: &'a str,
            pw: &'a str,
        }
        let request = self
            .request(Method::POST, "/Users/AuthenticateByName")
            .json(&Body { username: user, pw: password });
        let response = request.send().await.map_err(|e| self.transport(&e))?;
        authenticated(response).await
    }

    /// Whether the server lets a device sign in with Quick Connect.
    pub async fn quick_connect_enabled(&self) -> Result<bool> {
        self.json::<bool>(self.request(Method::GET, "/QuickConnect/Enabled")).await
    }

    /// Start a Quick Connect: show `code` to the person, and poll with `secret`.
    pub async fn quick_connect_start(&self) -> Result<QuickConnect> {
        let response = self
            .request(Method::POST, "/QuickConnect/Initiate")
            .send()
            .await
            .map_err(|e| self.transport(&e))?;
        match response.status() {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(JellyfinError::QuickConnectOff),
            status if status.is_success() => {
                response.json().await.map_err(|e| JellyfinError::Decode(e.to_string()))
            }
            status => Err(JellyfinError::Status { status: status.as_u16() }),
        }
    }

    /// Whether the code has been approved on a signed-in device. A code that has run out is
    /// [`JellyfinError::QuickConnectExpired`].
    pub async fn quick_connect_approved(&self, secret: &str) -> Result<bool> {
        let request = self.request(Method::GET, "/QuickConnect/Connect").query(&[("secret", secret)]);
        let response = request.send().await.map_err(|e| self.transport(&e))?;
        match response.status() {
            StatusCode::NOT_FOUND | StatusCode::UNAUTHORIZED => {
                Err(JellyfinError::QuickConnectExpired)
            }
            status if status.is_success() => {
                let state: QuickConnect =
                    response.json().await.map_err(|e| JellyfinError::Decode(e.to_string()))?;
                Ok(state.authenticated)
            }
            status => Err(JellyfinError::Status { status: status.as_u16() }),
        }
    }

    /// Sign in with an approved Quick Connect.
    pub async fn sign_in_with_quick_connect(&self, secret: &str) -> Result<Session> {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct Body<'a> {
            secret: &'a str,
        }
        let request = self
            .request(Method::POST, "/Users/AuthenticateWithQuickConnect")
            .json(&Body { secret });
        let response = request.send().await.map_err(|e| self.transport(&e))?;
        authenticated(response).await
    }

    fn user(&self) -> Result<&Session> {
        self.session.as_ref().ok_or(JellyfinError::SignedOut)
    }

    /// The signed-in user's libraries.
    pub async fn libraries(&self) -> Result<Vec<Library>> {
        let user = self.user()?;
        let path = format!("/Users/{}/Views", user.user_id);
        Ok(self.json::<Items<Library>>(self.request(Method::GET, &path)).await?.items)
    }

    /// Every series in a library, with the ids other services know them by.
    pub async fn series(&self, library_id: &str) -> Result<Vec<Series>> {
        let user = self.user()?;
        let request = self.request(Method::GET, "/Items").query(&[
            ("userId", user.user_id.as_str()),
            ("parentId", library_id),
            ("includeItemTypes", "Series"),
            ("recursive", "true"),
            ("fields", "ProviderIds,ProductionYear,OriginalTitle,ChildCount"),
            ("sortBy", "SortName"),
        ]);
        Ok(self.json::<Items<Series>>(request).await?.items)
    }

    /// A series' episodes with this user's resume points and watched marks.
    pub async fn episodes(&self, series_id: &str) -> Result<Vec<EpisodeItem>> {
        let user = self.user()?;
        let path = format!("/Shows/{series_id}/Episodes");
        let request = self
            .request(Method::GET, &path)
            .query(&[("userId", user.user_id.as_str()), ("fields", "RunTimeTicks")]);
        Ok(self.json::<Items<EpisodeItem>>(request).await?.items)
    }

    /// The address of an item's primary image, `width` pixels wide at most. The player and the
    /// image loader send [`Self::player_headers`] with it.
    pub fn image_url(&self, item_id: &str, width: u32) -> String {
        self.address.url(&format!("/Items/{item_id}/Images/Primary?maxWidth={width}"))
    }

    /// How an item plays: its media sources and subtitle streams.
    pub async fn playback_info(&self, item_id: &str) -> Result<PlaybackInfo> {
        let user = self.user()?;
        let path = format!("/Items/{item_id}/PlaybackInfo");
        let request = self
            .request(Method::POST, &path)
            .query(&[("userId", user.user_id.as_str())])
            .json(&serde_json::json!({}));
        self.json(request).await
    }

    /// The address that plays a source as it is stored: no transcoding. mpv plays every codec
    /// Jellyfin keeps, so anistream never asks the server to convert.
    pub fn direct_stream_url(&self, item_id: &str, media_source_id: &str) -> String {
        self.address.url(&format!(
            "/Videos/{item_id}/stream?static=true&mediaSourceId={media_source_id}"
        ))
    }

    /// The address of an external subtitle stream, from its `DeliveryUrl`.
    pub fn subtitle_url(&self, delivery_url: &str) -> String {
        if delivery_url.starts_with("http") {
            delivery_url.to_owned()
        } else {
            self.address.url(delivery_url)
        }
    }

    /// Tell the server playback began.
    pub async fn report_started(&self, item_id: &str, media_source_id: &str, position_secs: f64) -> Result<()> {
        self.report("/Sessions/Playing", item_id, media_source_id, position_secs, false).await
    }

    /// Tell the server where playback is. Sent every ten seconds or so.
    pub async fn report_progress(
        &self,
        item_id: &str,
        media_source_id: &str,
        position_secs: f64,
        paused: bool,
    ) -> Result<()> {
        self.report("/Sessions/Playing/Progress", item_id, media_source_id, position_secs, paused).await
    }

    /// Tell the server playback ended. The server marks the item played near its end.
    pub async fn report_stopped(&self, item_id: &str, media_source_id: &str, position_secs: f64) -> Result<()> {
        self.report("/Sessions/Playing/Stopped", item_id, media_source_id, position_secs, false).await
    }

    async fn report(
        &self,
        path: &str,
        item_id: &str,
        media_source_id: &str,
        position_secs: f64,
        paused: bool,
    ) -> Result<()> {
        let body = serde_json::json!({
            "ItemId": item_id,
            "MediaSourceId": media_source_id,
            "PositionTicks": (position_secs.max(0.0) * TICKS_PER_SECOND as f64) as i64,
            "IsPaused": paused,
            "CanSeek": true,
            "PlayMethod": "DirectPlay",
        });
        self.send(self.request(Method::POST, path).json(&body)).await.map(|_| ())
    }

    /// Mark an item played or not played for this user.
    pub async fn set_played(&self, item_id: &str, played: bool) -> Result<()> {
        let user = self.user()?;
        let path = format!("/Users/{}/PlayedItems/{item_id}", user.user_id);
        let method = if played { Method::POST } else { Method::DELETE };
        self.send(self.request(method, &path)).await.map(|_| ())
    }
}

async fn authenticated(response: Response) -> Result<Session> {
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::BAD_REQUEST => {
            Err(JellyfinError::BadCredentials)
        }
        status if status.is_success() => {
            let result: AuthenticationResult =
                response.json().await.map_err(|e| JellyfinError::Decode(e.to_string()))?;
            Ok(Session {
                token: result.access_token,
                user_id: result.user.id,
                user_name: result.user.name,
            })
        }
        status => Err(JellyfinError::Status { status: status.as_u16() }),
    }
}

/// A header value without the characters that would end it.
fn clean(value: &str) -> String {
    value.chars().filter(|c| !c.is_control() && *c != '"').collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_before_10_8_are_too_old() {
        for old in ["10.7.7", "9.0.1", "", "next"] {
            assert!(!version_supported(old), "{old}");
        }
        for fine in ["10.8.0", "10.9.11", "11.0.0"] {
            assert!(version_supported(fine), "{fine}");
        }
    }
}
