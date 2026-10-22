//! A tracker's token pair as it is held while the tracker runs: renewed before it runs out, and
//! kept in step with the other processes signed in with it.
//!
//! MyAnimeList, Kitsu and Simkl each issue a pair that runs out (in a month, a month, a week),
//! and each is renewed with its refresh token a day early. The terminal and the Mac app keep one
//! sign-in in one [`TokenStore`], and a renewal by either ends the access token the other holds.
//! Simkl ends it at once; MyAnimeList and Kitsu replace the refresh token too, so the other's
//! renewal would fail as well. Two rules keep the processes in step:
//!
//! - **Read the store again before renewing, and when a token is rejected.** It is read past this
//!   process's cache. A pair another process stored is adopted instead of renewed a second time,
//!   and a request it rejected is sent once more with that pair. Only when the store holds the same
//!   pair is it renewed here, and only when there is nothing to renew it with does the call fail.
//! - **Store a renewed pair at once.** The token it replaces no longer works, so another process
//!   reading the store before the next drain would find only a dead one.

use std::future::Future;

use tokio::sync::RwLock;

use crate::secret::{SecretError, TokenPair, TokenStore};

/// How early a pair is renewed: a day before it runs out.
pub(crate) const MARGIN_SECS: i64 = 24 * 3_600;

/// How a tracker renews its pair, at its own token endpoint.
pub(crate) trait Renews: Sync {
    fn renew(
        &self,
        refresh: &str,
        now: i64,
    ) -> impl Future<Output = Result<TokenPair, String>> + Send;
}

/// One tracker's pair.
pub(crate) struct Held {
    /// The tracker's id, its key in the store: `mal`.
    id: &'static str,
    /// The tracker as the person reads it: `myanimelist`.
    name: &'static str,
    pair: RwLock<Option<TokenPair>>,
    /// A renewed pair the store did not take, for the caller to try again.
    pending_save: RwLock<Option<TokenPair>>,
    /// The store this pair is kept in, shared with the other processes signed in with it.
    store: Option<TokenStore>,
}

/// What the store holds now, against the pair held here.
enum Stored {
    /// The same pair, or no answer: the one held stands.
    Unchanged,
    /// Another process renewed it, or signed in again.
    Renewed(TokenPair),
    /// Another process signed out.
    SignedOut,
}

impl Held {
    pub(crate) fn new(id: &'static str, name: &'static str, pair: Option<TokenPair>) -> Self {
        Self { id, name, pair: RwLock::new(pair), pending_save: RwLock::new(None), store: None }
    }

    /// Kept in step with the other processes signed in through `store`.
    pub(crate) fn share(&mut self, store: TokenStore) {
        self.store = Some(store);
    }

    /// A renewed pair the caller should store, if the store did not take it when it was renewed.
    pub(crate) async fn take_renewed(&self) -> Option<TokenPair> {
        self.pending_save.write().await.take()
    }

    /// Whether a pair is held. `try_read`, so a screen asking is never blocked by a renewal in
    /// flight: a renewal under way means it is signed in.
    pub(crate) fn is_held(&self) -> bool {
        self.pair.try_read().map_or(true, |held| held.is_some())
    }

    pub(crate) async fn accept(&self, pair: TokenPair) {
        *self.pair.write().await = Some(pair);
    }

    pub(crate) async fn forget(&self) {
        // The pair held is what every call reads, so clearing the store alone would leave the
        // tracker pushing with a revoked credential.
        *self.pair.write().await = None;
        *self.pending_save.write().await = None;
    }

    /// A usable access token, renewed first when it is close to running out.
    pub(crate) async fn access(
        &self,
        renews: &impl Renews,
        now: i64,
    ) -> Result<String, anistream_core::Error> {
        {
            let held = self.pair.read().await;
            let pair = held.as_ref().ok_or_else(|| self.not_signed_in())?;
            if !pair.needs_refresh(now, MARGIN_SECS) && !pair.is_expired(now) {
                return Ok(pair.access.clone());
            }
        }
        self.renew(renews, now, None).await
    }

    /// Send the request `build` makes with `token`. If the service rejects it, `token` becomes
    /// the pair another process stored, or one renewed now, and the request is sent once more.
    /// `token` is left as the one the answer was for, for the requests after it.
    pub(crate) async fn send(
        &self,
        renews: &impl Renews,
        token: &mut String,
        build: impl Fn(&str) -> reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, anistream_core::Error> {
        let failed = |e: reqwest::Error| anistream_core::Error::Tracker {
            tracker: self.id.into(),
            message: e.to_string(),
        };
        let response = build(token).send().await.map_err(failed)?;
        if !is_rejection(&response) {
            return Ok(response);
        }
        *token = self.renew(renews, crate::now_epoch(), Some(token.as_str())).await?;
        build(token).send().await.map_err(failed)
    }

    /// Renew under the write lock, re-checked: two calls at once make one renewal, and each
    /// renewal ends the token before it. `rejected` is a token the service refused.
    async fn renew(
        &self,
        renews: &impl Renews,
        now: i64,
        rejected: Option<&str>,
    ) -> Result<String, anistream_core::Error> {
        let mut held = self.pair.write().await;
        let pair = held.as_ref().ok_or_else(|| self.not_signed_in())?;
        // Another call here has moved on from the rejected token already.
        if let Some(rejected) = rejected
            && pair.access != rejected
        {
            return Ok(pair.access.clone());
        }

        match self.stored(pair) {
            Stored::Unchanged => {}
            Stored::Renewed(stored) => {
                tracing::info!(tracker = self.id, "taking the pair another process stored");
                *held = Some(stored);
            }
            // Renewing would write the pair back into the store, and sign the other back in.
            Stored::SignedOut => {
                tracing::info!(tracker = self.id, "signed out elsewhere");
                *held = None;
                return Err(self.not_signed_in());
            }
        }

        let pair = held.as_ref().ok_or_else(|| self.not_signed_in())?;
        let rejected = rejected.is_some_and(|rejected| pair.access == rejected);
        if !rejected && !pair.needs_refresh(now, MARGIN_SECS) {
            // `needs_refresh` is false for a pair with nothing to renew it with.
            return if pair.is_expired(now) {
                Err(self.ran_out())
            } else {
                Ok(pair.access.clone())
            };
        }
        let Some(refresh) = pair.refresh.clone() else {
            return Err(if rejected { self.refused() } else { self.ran_out() });
        };

        tracing::info!("refreshing the {} token", self.name);
        let renewed = renews.renew(&refresh, now).await.map_err(anistream_core::Error::Auth)?;
        let access = renewed.access.clone();
        self.save(&renewed).await;
        *held = Some(renewed);
        Ok(access)
    }

    /// The store's pair now, read past this process's cache.
    fn stored(&self, held: &TokenPair) -> Stored {
        let Some(store) = &self.store else {
            return Stored::Unchanged;
        };
        match store.reread_pair(self.id) {
            Ok(stored) if !stored.access.is_empty() && stored.access != held.access => {
                Stored::Renewed(stored)
            }
            Ok(_) => Stored::Unchanged,
            Err(SecretError::Missing(_)) => Stored::SignedOut,
            // A keychain that did not answer says nothing about the pair.
            Err(e) => {
                tracing::debug!(tracker = self.id, error = %e, "could not read the store again");
                Stored::Unchanged
            }
        }
    }

    /// Store a renewed pair at once where the store is shared, or leave it for the caller.
    async fn save(&self, renewed: &TokenPair) {
        if let Some(store) = &self.store {
            match store.set_pair(
                self.id,
                &renewed.access,
                renewed.refresh.as_deref(),
                renewed.expires_at,
            ) {
                Ok(_) => return,
                Err(e) => tracing::warn!(
                    tracker = self.id,
                    error = %e,
                    "could not store the renewed pair; the next drain tries again"
                ),
            }
        }
        *self.pending_save.write().await = Some(renewed.clone());
    }

    fn not_signed_in(&self) -> anistream_core::Error {
        anistream_core::Error::Auth(format!("not signed in to {}", self.name))
    }

    fn ran_out(&self) -> anistream_core::Error {
        anistream_core::Error::Auth(format!(
            "{} token expired and there is no refresh token",
            self.name
        ))
    }

    fn refused(&self) -> anistream_core::Error {
        anistream_core::Error::Auth(format!("{} rejected the token", self.name))
    }
}

/// Whether the service rejected the token: a 401, or RFC 6750's `invalid_token` in its challenge.
pub(crate) fn is_rejection(response: &reqwest::Response) -> bool {
    response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response
            .headers()
            .get(reqwest::header::WWW_AUTHENTICATE)
            .and_then(|challenge| challenge.to_str().ok())
            .is_some_and(|challenge| challenge.contains("invalid_token"))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::secret::Backend;

    /// A token endpoint that counts its renewals and numbers what it issues.
    #[derive(Default)]
    struct Endpoint {
        renewals: AtomicUsize,
    }

    impl Renews for Endpoint {
        async fn renew(&self, refresh: &str, now: i64) -> Result<TokenPair, String> {
            let n = self.renewals.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(TokenPair {
                access: format!("renewed-{n}"),
                refresh: Some(refresh.to_owned()),
                expires_at: Some(now + 7 * 24 * 3_600),
            })
        }
    }

    const NOW: i64 = 1_000_000;

    fn pair(access: &str, expires_at: i64) -> TokenPair {
        TokenPair {
            access: access.into(),
            refresh: Some("r".into()),
            expires_at: Some(expires_at),
        }
    }

    /// One process: its own store over the shared folder, so its own cache.
    fn process(dir: &std::path::Path, held: TokenPair) -> Held {
        let mut tracker = Held::new("simkl", "simkl", Some(held));
        tracker.share(TokenStore::with_backend(dir, Backend::File));
        tracker
    }

    fn store(dir: &std::path::Path, pair: &TokenPair) {
        TokenStore::with_backend(dir, Backend::File)
            .set_pair("simkl", &pair.access, pair.refresh.as_deref(), pair.expires_at)
            .unwrap();
    }

    #[tokio::test]
    async fn a_pair_another_process_renewed_is_taken_instead_of_renewed_again() {
        let dir = tempfile::tempdir().unwrap();
        let first = pair("first", NOW + 3_600);
        store(dir.path(), &first);
        let (app, terminal) = (process(dir.path(), first.clone()), process(dir.path(), first));
        let endpoint = Endpoint::default();

        // Inside the margin: the app renews, and stores it at once.
        assert_eq!(app.access(&endpoint, NOW).await.unwrap(), "renewed-1");
        assert!(
            app.take_renewed().await.is_none(),
            "stored already, nothing left to the drain"
        );
        // The terminal finds it in the store, rather than renewing and ending the app's.
        assert_eq!(terminal.access(&endpoint, NOW).await.unwrap(), "renewed-1");
        assert_eq!(endpoint.renewals.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_rejected_token_is_retried_with_the_stored_pair_before_renewing() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = process(dir.path(), pair("first", NOW + 5 * 24 * 3_600));
        store(dir.path(), &pair("second", NOW + 7 * 24 * 3_600));
        let endpoint = Endpoint::default();

        assert_eq!(terminal.renew(&endpoint, NOW, Some("first")).await.unwrap(), "second");
        assert_eq!(endpoint.renewals.load(Ordering::SeqCst), 0);
        // A call that still had the old token moves on to the one held, without a renewal.
        assert_eq!(terminal.renew(&endpoint, NOW, Some("first")).await.unwrap(), "second");
        assert_eq!(endpoint.renewals.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_rejected_token_the_store_still_holds_is_renewed_and_stored() {
        let dir = tempfile::tempdir().unwrap();
        let first = pair("first", NOW + 5 * 24 * 3_600);
        store(dir.path(), &first);
        let terminal = process(dir.path(), first);
        let endpoint = Endpoint::default();

        assert_eq!(terminal.renew(&endpoint, NOW, Some("first")).await.unwrap(), "renewed-1");
        let stored = TokenStore::with_backend(dir.path(), Backend::File).get_pair("simkl");
        assert_eq!(stored.unwrap().access, "renewed-1");
    }

    #[tokio::test]
    async fn a_rejected_token_with_nothing_to_renew_it_says_so() {
        let tracker = Held::new(
            "simkl",
            "simkl",
            Some(TokenPair { access: "pin".into(), refresh: None, expires_at: None }),
        );
        let e = tracker.renew(&Endpoint::default(), NOW, Some("pin")).await.unwrap_err();
        assert_eq!(
            e.to_string(),
            anistream_core::Error::Auth("simkl rejected the token".into()).to_string()
        );
    }

    #[tokio::test]
    async fn a_sign_out_elsewhere_is_not_undone_by_a_renewal_here() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = process(dir.path(), pair("first", NOW + 3_600));
        // Nothing stored: the app signed out.
        let endpoint = Endpoint::default();
        let e = terminal.access(&endpoint, NOW).await.unwrap_err();
        assert!(matches!(e, anistream_core::Error::Auth(_)), "{e:?}");
        assert_eq!(endpoint.renewals.load(Ordering::SeqCst), 0);
        assert!(!terminal.is_held());
        assert!(TokenStore::with_backend(dir.path(), Backend::File).get_pair("simkl").is_err());
    }

    #[tokio::test]
    async fn without_a_store_a_renewed_pair_is_left_for_the_caller() {
        let tracker = Held::new("mal", "myanimelist", Some(pair("first", NOW + 3_600)));
        assert_eq!(tracker.access(&Endpoint::default(), NOW).await.unwrap(), "renewed-1");
        assert_eq!(tracker.take_renewed().await.unwrap().access, "renewed-1");
        assert!(tracker.take_renewed().await.is_none());
    }

    #[test]
    fn a_rejection_is_a_401_or_an_invalid_token_challenge() {
        let answer = |status: u16, challenge: Option<&str>| -> reqwest::Response {
            let mut response = http::Response::builder().status(status);
            if let Some(challenge) = challenge {
                response = response.header("www-authenticate", challenge);
            }
            response.body("").unwrap().into()
        };
        assert!(is_rejection(&answer(401, None)));
        assert!(is_rejection(&answer(
            400,
            Some(r#"Bearer error="invalid_token", error_description="expired""#)
        )));
        assert!(!is_rejection(&answer(403, Some(r#"Bearer error="insufficient_scope""#))));
        assert!(!is_rejection(&answer(200, None)));
    }
}
