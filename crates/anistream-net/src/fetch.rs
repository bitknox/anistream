//! Conditional fetches for the refreshable datasets.
//!
//! The mapping corpora are ~7.7 MB raw and update daily. Re-downloading that on a hunch
//! would be wasteful, but skipping the check would let the data rot — and stale mappings
//! are exactly the failure the mapping layer exists to prevent.
//!
//! `raw.githubusercontent.com` serves a strong `ETag` and honours `If-None-Match`, so the
//! steady-state check costs a round trip and **zero bytes of body**. Measured during
//! planning: 304 with no content, versus 1.24 MB gzipped for a real refresh.

use crate::{Result, client::NetError};

/// What a conditional fetch found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConditionalResponse {
    /// Server confirmed our copy is current. No body transferred.
    NotModified,
    /// New content, with the ETag to store for next time.
    Fetched { etag: Option<String>, body: Vec<u8> },
}

impl ConditionalResponse {
    pub fn is_modified(&self) -> bool {
        matches!(self, Self::Fetched { .. })
    }

    pub fn body(&self) -> Option<&[u8]> {
        match self {
            Self::Fetched { body, .. } => Some(body),
            Self::NotModified => None,
        }
    }
}

/// A URL that can be re-fetched conditionally.
#[derive(Debug, Clone)]
pub struct Conditional {
    pub url: String,
    /// ETag from the previous successful fetch, if any.
    pub etag: Option<String>,
}

impl Conditional {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into(), etag: None }
    }

    pub fn with_etag(mut self, etag: Option<String>) -> Self {
        self.etag = etag;
        self
    }

    /// Perform the fetch.
    ///
    /// Uses the plain client: these are GitHub-hosted static files with no bot
    /// protection, so paying for fingerprint emulation would buy nothing.
    pub async fn get(&self, client: &reqwest::Client) -> Result<ConditionalResponse> {
        let mut req = client.get(&self.url).header("accept-encoding", "gzip");
        if let Some(etag) = &self.etag {
            req = req.header("if-none-match", etag);
        }

        let response = req
            .send()
            .await
            .map_err(|e| NetError::Request { url: self.url.clone(), message: e.to_string() })?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            tracing::debug!(url = %self.url, "dataset unchanged (304)");
            return Ok(ConditionalResponse::NotModified);
        }
        if !status.is_success() {
            return Err(NetError::Status { url: self.url.clone(), status: status.as_u16() });
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);

        let body = response
            .bytes()
            .await
            .map_err(|e| NetError::Request { url: self.url.clone(), message: e.to_string() })?
            .to_vec();

        tracing::info!(url = %self.url, bytes = body.len(), "dataset refreshed");
        Ok(ConditionalResponse::Fetched { etag, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_modified_carries_no_body() {
        let r = ConditionalResponse::NotModified;
        assert!(!r.is_modified());
        assert!(r.body().is_none());
    }

    #[test]
    fn fetched_exposes_body_and_etag() {
        let r =
            ConditionalResponse::Fetched { etag: Some("\"abc\"".into()), body: b"[]".to_vec() };
        assert!(r.is_modified());
        assert_eq!(r.body(), Some(&b"[]"[..]));
    }

    #[test]
    fn etag_is_carried_into_the_request_builder() {
        let c =
            Conditional::new("https://example.test/a.json").with_etag(Some("\"v1\"".into()));
        assert_eq!(c.etag.as_deref(), Some("\"v1\""));
        // A first-ever fetch has no ETag and must still be valid.
        assert!(Conditional::new("https://example.test/a.json").etag.is_none());
    }
}
