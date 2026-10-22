//! Talking to a store.
//!
//! Every response that carries plugins is checked here before anyone else sees it: the index
//! against the key from the invite and the highest sequence already accepted, a download against
//! the size and digest the signed index gave for it. Callers get verified data or an error, never
//! something to check themselves.

use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    crypto::{self, PublicKey},
    invite::{self, Invite},
    protocol::{
        AdminLink, ErrorBody, Index, IndexEntry, InviteCreated, InviteRequest,
        MAX_PLUGIN_BYTES, Me, MemberList, PART_SIZE, PROTOCOL, PreviewRequest, PublishRequest,
        RedeemRequest, RedeemResponse, SignedIndex, StoreInfo, UploadRequest, UploadStarted,
        is_valid_id, is_valid_version,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ClientError {
    #[error(transparent)]
    Invite(#[from] invite::InviteError),
    #[error("could not reach the store: {0}")]
    Network(String),
    /// The token was refused: removed by the owner, or revoked from another device.
    #[error("the store no longer accepts this device")]
    Unauthorized,
    #[error("{message}")]
    Server { status: u16, code: String, message: String },
    #[error("the store's key does not match the one in the invite — do not join it")]
    KeyMismatch,
    #[error("the store's plugin list is not signed by its key")]
    BadSignature,
    #[error("the store sent an older plugin list than one already seen ({got} after {seen})")]
    Rollback { seen: u64, got: u64 },
    #[error("the store speaks protocol {0}, and this anistream speaks {PROTOCOL}")]
    Protocol(u32),
    #[error("the store sent something unexpected: {0}")]
    Malformed(String),
    #[error("the download does not match the store's list: {0}")]
    Integrity(String),
}

impl ClientError {
    /// The server's error code, when there was one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Server { code, .. } => Some(code),
            _ => None,
        }
    }
}

/// A connection to any number of stores.
#[derive(Debug, Clone)]
pub struct StoreClient {
    http: reqwest::Client,
}

impl StoreClient {
    /// A client that never follows redirects.
    ///
    /// A redirect would carry the bearer token somewhere the invite did not name. reqwest strips
    /// the header on a cross-origin hop, but a store has no reason to redirect at all, so the
    /// simpler rule is to not follow one.
    pub fn new(timeout: Duration) -> Result<Self, ClientError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("anistream/", env!("CARGO_PKG_VERSION")));
        // On Android the Mozilla roots built in, rather than the platform's verifier, which
        // fails every request until the app hands it the JVM. A CA the user installed on the
        // device is not trusted there. As `anistream_net::trusted_roots` decides for the app's
        // other plain clients; this crate stands apart from the network layer, so it says so
        // itself.
        #[cfg(target_os = "android")]
        let http = http.tls_certs_only(
            webpki_root_certs::TLS_SERVER_ROOT_CERTS
                .iter()
                .filter_map(|der| reqwest::Certificate::from_der(der).ok()),
        );
        let http = http.build().map_err(|e| ClientError::Network(e.to_string()))?;
        Ok(Self { http })
    }

    /// The store's public description, checked against the invite.
    ///
    /// The first request a client makes to a store, before it spends the invite: a store that
    /// answers with a different key is not the store the owner sent, and joining it would hand
    /// the invite to whoever that is.
    pub async fn preview(&self, invite: &Invite) -> Result<StoreInfo, ClientError> {
        let info = self.info(&invite.base).await?;
        if PublicKey::parse(&info.key).ok() != Some(invite.key) {
            return Err(ClientError::KeyMismatch);
        }
        Ok(info)
    }

    pub async fn info(&self, base: &str) -> Result<StoreInfo, ClientError> {
        let info: StoreInfo = self.get_json(base, "v1/store", None).await?;
        if info.protocol != PROTOCOL {
            return Err(ClientError::Protocol(info.protocol));
        }
        Ok(info)
    }

    /// The plugin list an invite leads to, verified against the key the invite carries, before
    /// it is spent: who runs the store and what it offers, for the confirmation. Spends nothing.
    ///
    /// `None` from a store that predates the preview, which answers that it has no such
    /// endpoint: the confirmation goes without. Any sequence is taken, since nothing is kept.
    pub async fn preview_index(&self, invite: &Invite) -> Result<Option<Index>, ClientError> {
        let body = PreviewRequest { invite: invite.code.clone() };
        let answer: Result<SignedIndex, _> = self
            .send_json(reqwest::Method::POST, &invite.base, "v1/preview", None, Some(&body))
            .await;
        match answer {
            Ok(signed) => verify_index(&signed, &invite.key, 0).map(Some),
            // Without the endpoint. Not an invite that would not join, which is a 404 too.
            Err(ClientError::Server { status: 404, code, .. }) if code != "invite_unusable" => {
                Ok(None)
            }
            Err(e) => Err(e),
        }
    }

    /// Spend an invite for a token.
    pub async fn redeem(
        &self,
        invite: &Invite,
        device: &str,
    ) -> Result<RedeemResponse, ClientError> {
        let body = RedeemRequest { invite: invite.code.clone(), device: device.to_owned() };
        self.send_json(reqwest::Method::POST, &invite.base, "v1/redeem", None, Some(&body))
            .await
    }

    pub async fn me(&self, base: &str, token: &str) -> Result<Me, ClientError> {
        self.get_json(base, "v1/me", Some(token)).await
    }

    /// Revoke this device's token. Used when leaving a store, so the token stops working even if
    /// a copy of it survives somewhere.
    pub async fn leave(&self, base: &str, token: &str) -> Result<(), ClientError> {
        self.send_empty(reqwest::Method::DELETE, base, "v1/me", Some(token)).await
    }

    /// The plugin list, verified.
    ///
    /// `seen` is the highest sequence already accepted from this store. An older list is refused
    /// rather than shown, because it is exactly what a store would send to keep someone on a
    /// version with a known problem.
    pub async fn index(
        &self,
        base: &str,
        token: &str,
        key: &PublicKey,
        seen: u64,
    ) -> Result<Index, ClientError> {
        let signed: SignedIndex = self.get_json(base, "v1/index", Some(token)).await?;
        verify_index(&signed, key, seen)
    }

    /// One plugin's bytes, verified against its entry in a signed index.
    pub async fn download(
        &self,
        base: &str,
        token: &str,
        entry: &IndexEntry,
    ) -> Result<Vec<u8>, ClientError> {
        if !is_valid_id(&entry.id) || !is_valid_version(&entry.version) {
            return Err(ClientError::Malformed(format!("unusable plugin id {:?}", entry.id)));
        }
        if entry.size > MAX_PLUGIN_BYTES {
            return Err(ClientError::Integrity(format!(
                "{} is {} bytes, over the {} byte limit",
                entry.id, entry.size, MAX_PLUGIN_BYTES
            )));
        }
        let path = format!("v1/plugins/{}/{}", entry.id, entry.version);

        // In ranges of one part each, because some platforms a store runs on cannot answer with
        // more than about 4 MB at once. A store that ignores ranges answers the first request
        // with the whole file, which is taken as it is.
        let mut bytes = Vec::with_capacity(entry.size.min(MAX_PLUGIN_BYTES) as usize);
        while (bytes.len() as u64) < entry.size {
            let start = bytes.len() as u64;
            let end = (start + PART_SIZE).min(entry.size) - 1;
            let response = self
                .request_with(reqwest::Method::GET, base, &path, Some(token), |r| {
                    r.header(reqwest::header::RANGE, format!("bytes={start}-{end}"))
                })
                .await?;
            let response = checked(response).await?;
            let whole = response.status() == reqwest::StatusCode::OK;
            if whole && start > 0 {
                return Err(ClientError::Malformed(
                    "the store stopped honouring ranges".into(),
                ));
            }
            if !whole
                && !response
                    .headers()
                    .get(reqwest::header::CONTENT_RANGE)
                    .and_then(|v| v.to_str().ok())
                    .is_some_and(|range| range.starts_with(&format!("bytes {start}-")))
            {
                return Err(ClientError::Malformed("the store sent the wrong part".into()));
            }
            // Read no more than the index promised, plus one byte to notice a longer body. A
            // store cannot fill the disk or memory with a "plugin" by sending more than it listed.
            let limit = (entry.size - start).saturating_add(1) as usize;
            let part = read_limited(response, limit).await.map_err(|e| match e {
                ClientError::Malformed(_) => ClientError::Integrity(format!(
                    "{} is longer than the store's list says",
                    entry.id
                )),
                e => e,
            })?;
            if part.is_empty() {
                break;
            }
            bytes.extend_from_slice(&part);
            if whole {
                break;
            }
        }
        verify_download(&bytes, entry)?;
        Ok(bytes)
    }

    pub async fn create_invite(
        &self,
        base: &str,
        token: &str,
        request: &InviteRequest,
    ) -> Result<InviteCreated, ClientError> {
        self.send_json(reqwest::Method::POST, base, "v1/invites", Some(token), Some(request))
            .await
    }

    /// A one-time link that signs a browser in to the store's admin page. Owners only.
    pub async fn admin_link(&self, base: &str, token: &str) -> Result<AdminLink, ClientError> {
        self.send_json::<(), AdminLink>(
            reqwest::Method::POST,
            base,
            "v1/admin/links",
            Some(token),
            None,
        )
        .await
    }

    pub async fn members(&self, base: &str, token: &str) -> Result<MemberList, ClientError> {
        self.get_json(base, "v1/members", Some(token)).await
    }

    pub async fn remove_member(
        &self,
        base: &str,
        token: &str,
        member: &str,
    ) -> Result<(), ClientError> {
        let path = format!("v1/members/{}", segment(member)?);
        self.send_empty(reqwest::Method::DELETE, base, &path, Some(token)).await
    }

    pub async fn cancel_invite(
        &self,
        base: &str,
        token: &str,
        invite: &str,
    ) -> Result<(), ClientError> {
        let path = format!("v1/invites/{}", segment(invite)?);
        self.send_empty(reqwest::Method::DELETE, base, &path, Some(token)).await
    }

    /// Publish a plugin: sent in parts, then put together and published by the store, which
    /// reads its id and version from the plugin itself.
    pub async fn publish(
        &self,
        base: &str,
        token: &str,
        wasm: Vec<u8>,
    ) -> Result<IndexEntry, ClientError> {
        let started: UploadStarted = self
            .send_json(
                reqwest::Method::POST,
                base,
                "v1/uploads",
                Some(token),
                Some(&UploadRequest { size: wasm.len() as u64 }),
            )
            .await?;
        let id = segment(&started.id)?.to_owned();
        let part_size = started.part_size.clamp(1, PART_SIZE) as usize;
        for (index, part) in wasm.chunks(part_size).enumerate() {
            let path = format!("v1/uploads/{id}/{index}");
            let part = part.to_vec();
            let response = self
                .request_with(reqwest::Method::PUT, base, &path, Some(token), |r| {
                    r.header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                        .body(part)
                })
                .await?;
            checked(response).await?;
        }
        self.send_json(
            reqwest::Method::POST,
            base,
            "v1/plugins",
            Some(token),
            Some(&PublishRequest { upload: id, sha256: Some(crypto::sha256_hex(&wasm)) }),
        )
        .await
    }

    pub async fn unpublish(
        &self,
        base: &str,
        token: &str,
        id: &str,
    ) -> Result<(), ClientError> {
        let path = format!("v1/plugins/{}", segment(id)?);
        self.send_empty(reqwest::Method::DELETE, base, &path, Some(token)).await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        base: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<T, ClientError> {
        self.send_json::<(), T>(reqwest::Method::GET, base, path, token, None).await
    }

    async fn send_json<B: Serialize, T: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        token: Option<&str>,
        body: Option<&B>,
    ) -> Result<T, ClientError> {
        let response = self
            .request_with(method, base, path, token, |request| match body {
                Some(body) => request.json(body),
                None => request,
            })
            .await?;
        decode(checked(response).await?).await
    }

    async fn send_empty(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<(), ClientError> {
        let response = self.request(method, base, path, token).await?;
        checked(response).await.map(drop)
    }

    async fn request(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        token: Option<&str>,
    ) -> Result<reqwest::Response, ClientError> {
        self.request_with(method, base, path, token, |r| r).await
    }

    async fn request_with(
        &self,
        method: reqwest::Method,
        base: &str,
        path: &str,
        token: Option<&str>,
        build: impl FnOnce(reqwest::RequestBuilder) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, ClientError> {
        let url = url::Url::parse(&format!("{}/{path}", base.trim_end_matches('/')))
            .map_err(|_| ClientError::Invite(invite::InviteError::NotALink))?;
        // Checked on every request, not only when joining: a base can come from a state file
        // somebody edited.
        invite::require_secure(&url)?;
        let mut request = self.http.request(method, url);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        build(request).send().await.map_err(|e| ClientError::Network(describe(&e)))
    }
}

/// Check a signed index, then read it.
pub fn verify_index(
    signed: &SignedIndex,
    key: &PublicKey,
    seen: u64,
) -> Result<Index, ClientError> {
    key.verify_index(&signed.index, &signed.signature)
        .map_err(|_| ClientError::BadSignature)?;
    let index: Index = serde_json::from_str(&signed.index)
        .map_err(|e| ClientError::Malformed(e.to_string()))?;
    if index.protocol != PROTOCOL {
        return Err(ClientError::Protocol(index.protocol));
    }
    // The key inside must be the key that signed it. Not a second check of the signature, but of
    // a store that signs with one key and claims another.
    if PublicKey::parse(&index.key).ok() != Some(*key) {
        return Err(ClientError::KeyMismatch);
    }
    if index.sequence < seen {
        return Err(ClientError::Rollback { seen, got: index.sequence });
    }
    if let Some(bad) = index.plugins.iter().find(|p| !is_valid_id(&p.id)) {
        return Err(ClientError::Malformed(format!("unusable plugin id {:?}", bad.id)));
    }
    Ok(index)
}

/// Check downloaded bytes against their index entry.
pub fn verify_download(bytes: &[u8], entry: &IndexEntry) -> Result<(), ClientError> {
    if bytes.len() as u64 != entry.size {
        return Err(ClientError::Integrity(format!(
            "{} should be {} bytes and is {}",
            entry.id,
            entry.size,
            bytes.len()
        )));
    }
    if crypto::sha256_hex(bytes) != entry.sha256.to_ascii_lowercase() {
        return Err(ClientError::Integrity(format!("{} has the wrong SHA-256", entry.id)));
    }
    Ok(())
}

/// The response if it succeeded, or the store's own account of why not.
async fn checked(response: reqwest::Response) -> Result<reqwest::Response, ClientError> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(ClientError::Unauthorized);
    }
    if status.is_redirection() {
        return Err(ClientError::Malformed(format!(
            "the store answered with a redirect (HTTP {}), which is never followed",
            status.as_u16()
        )));
    }
    // A store's error body says what went wrong in words meant for the user. Anything that is not
    // one — a reverse proxy's HTML error page, say — falls back to the status.
    let body = read_limited(response, MAX_ERROR_BYTES).await.unwrap_or_default();
    let (code, message) = match serde_json::from_slice::<ErrorBody>(&body) {
        Ok(error) => (error.error, error.message),
        Err(_) => (
            format!("http_{}", status.as_u16()),
            format!("the store answered HTTP {}", status.as_u16()),
        ),
    };
    Err(ClientError::Server { status: status.as_u16(), code, message })
}

/// The largest JSON answer read from a store. An index of a thousand plugins is a few hundred
/// kilobytes; past this it is not an index.
const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;

/// An error body is a sentence. Anything longer is not read.
const MAX_ERROR_BYTES: usize = 64 * 1024;

async fn decode<T: DeserializeOwned>(response: reqwest::Response) -> Result<T, ClientError> {
    let body = read_limited(response, MAX_JSON_BYTES).await?;
    serde_json::from_slice(&body).map_err(|e| ClientError::Malformed(e.to_string()))
}

/// Read a body, stopping at `limit`. The first request to a store is made before anyone has
/// joined it — a pasted link is enough — so nothing it sends can be allowed to grow without end.
async fn read_limited(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, ClientError> {
    let too_large =
        || ClientError::Malformed(format!("the store's answer is over {} KiB", limit / 1024));
    if response.content_length().is_some_and(|length| length > limit as u64) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) =
        response.chunk().await.map_err(|e| ClientError::Network(e.to_string()))?
    {
        if body.len() + chunk.len() > limit {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn segment(id: &str) -> Result<&str, ClientError> {
    if id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') && !id.is_empty()
    {
        Ok(id)
    } else {
        Err(ClientError::Malformed(format!("unusable id {id:?}")))
    }
}

fn describe(error: &reqwest::Error) -> String {
    if error.is_timeout() {
        "timed out".into()
    } else if error.is_connect() {
        "could not connect".into()
    } else {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::SigningKey;

    fn signed(key: &SigningKey, sequence: u64, ids: &[&str]) -> SignedIndex {
        let index = Index {
            protocol: PROTOCOL,
            store: "Test".into(),
            key: key.public().encode(),
            sequence,
            issued_at: 0,
            plugins: ids
                .iter()
                .map(|id| IndexEntry {
                    id: (*id).into(),
                    name: (*id).into(),
                    version: "1.0.0".into(),
                    allowed_hosts: Vec::new(),
                    translations: Vec::new(),
                    sha256: String::new(),
                    size: 0,
                    published_at: 0,
                })
                .collect(),
            owners: Vec::new(),
        };
        let index = serde_json::to_string(&index).unwrap();
        SignedIndex { signature: key.sign_index(&index), index }
    }

    #[test]
    fn a_good_index_verifies() {
        let key = SigningKey::generate().unwrap();
        let index = verify_index(&signed(&key, 3, &["example"]), &key.public(), 2).unwrap();
        assert_eq!(index.sequence, 3);
        assert_eq!(index.plugins[0].id, "example");
    }

    #[test]
    fn the_same_sequence_again_is_accepted() {
        // Fetching an unchanged list twice is normal; only going backwards is suspicious.
        let key = SigningKey::generate().unwrap();
        assert!(verify_index(&signed(&key, 3, &[]), &key.public(), 3).is_ok());
    }

    #[test]
    fn an_older_index_is_refused() {
        let key = SigningKey::generate().unwrap();
        assert_eq!(
            verify_index(&signed(&key, 2, &[]), &key.public(), 5),
            Err(ClientError::Rollback { seen: 5, got: 2 })
        );
    }

    #[test]
    fn an_index_signed_by_another_key_is_refused() {
        let key = SigningKey::generate().unwrap();
        let other = SigningKey::generate().unwrap();
        assert_eq!(
            verify_index(&signed(&other, 1, &[]), &key.public(), 0),
            Err(ClientError::BadSignature)
        );
    }

    #[test]
    fn an_edited_index_is_refused() {
        let key = SigningKey::generate().unwrap();
        let mut index = signed(&key, 1, &["example"]);
        index.index = index.index.replace("example", "exampl3");
        assert_eq!(verify_index(&index, &key.public(), 0), Err(ClientError::BadSignature));
    }

    #[test]
    fn a_signed_index_naming_a_traversal_is_still_refused() {
        // Signed by the store's own key, so the signature passes: the id check is what stands
        // between a hostile store and a file outside the plugin directory.
        let key = SigningKey::generate().unwrap();
        let index = signed(&key, 1, &["../../.ssh/authorized_keys"]);
        assert!(matches!(
            verify_index(&index, &key.public(), 0),
            Err(ClientError::Malformed(_))
        ));
    }

    #[test]
    fn a_download_must_match_size_and_digest() {
        let bytes = b"plugin bytes".to_vec();
        let key = SigningKey::generate().unwrap();
        let index = verify_index(&signed(&key, 1, &["example"]), &key.public(), 0).unwrap();
        let entry = IndexEntry {
            size: bytes.len() as u64,
            sha256: crypto::sha256_hex(&bytes),
            ..index.plugins[0].clone()
        };
        verify_download(&bytes, &entry).unwrap();

        let mut longer = bytes.clone();
        longer.push(b'!');
        assert!(matches!(verify_download(&longer, &entry), Err(ClientError::Integrity(_))));

        let mut changed = bytes.clone();
        changed[0] = b'P';
        assert!(matches!(verify_download(&changed, &entry), Err(ClientError::Integrity(_))));
    }
}
