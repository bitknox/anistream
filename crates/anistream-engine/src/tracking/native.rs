//! Signing in from a native app, with the app's own clients: the system's sign-in sheet opens
//! the tracker's page and comes back on the app's own URL scheme.
//!
//! The terminal catches a redirect on a loopback port, which a phone has no use for. Here the
//! engine says what to open with [`Event::Authorize`], the app shows the sheet, and hands the
//! redirect back with `Command::FinishSignIn`. AniList's is the implicit grant, its token in the
//! redirect's fragment, with no secret, since an app's secret would not be one. MyAnimeList's is
//! the same PKCE the terminal uses, with the app's scheme for its redirect. Each asks with a
//! `state` of its own, and a redirect that does not carry it back is refused.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, PoisonError},
};

use anistream_core::config::Config;
use anistream_model::{event::Event, notice::Notice};
use anistream_track::{TokenPair, auth, mal};

use super::{Sync, activate};
use crate::{options::AppClients, outbox::Outbox};

/// The URL scheme the app's redirects come back on.
pub const CALLBACK_SCHEME: &str = "anistream";

/// Where `tracker`'s sign-in redirects to, on the app's own scheme: what its client registers.
pub fn redirect_uri(tracker: &str) -> String {
    format!("{CALLBACK_SCHEME}://auth/{tracker}")
}

/// The sign-ins under way, by tracker: what each asked with, for checking what comes back.
#[derive(Clone, Default)]
pub struct PendingSignIns(Arc<Mutex<HashMap<String, Pending>>>);

/// One sign-in under way.
struct Pending {
    /// What the redirect has to carry back.
    state: String,
    /// The PKCE verifier, for a code to be exchanged with.
    verifier: Option<String>,
}

impl PendingSignIns {
    fn put(&self, tracker: &str, pending: Pending) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(tracker.to_owned(), pending);
    }

    fn take(&self, tracker: &str) -> Option<Pending> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).remove(tracker)
    }
}

/// `config`, as the trackers are built from it for an app: AniList and MyAnimeList turned on
/// where the app ships a client for them, MyAnimeList's client among its settings, which its
/// tracker renews tokens with. The file is never given any of it.
pub fn with_app_clients(config: &Config, clients: &AppClients) -> Config {
    let mut config = config.clone();
    let trackers = &mut config.trackers;
    for (id, client) in [("anilist", &clients.anilist), ("mal", &clients.mal)] {
        if client.is_some() && !trackers.is_enabled(id) {
            trackers.enabled.push(id.to_owned());
        }
    }
    if let Some(client) = &clients.mal
        && trackers.mal.client_id.as_deref().is_none_or(|id| id.trim().is_empty())
    {
        trackers.mal.client_id = Some(client.clone());
    }
    config
}

/// Start signing in to `tracker` with the app's own client, when the app ships one: say what
/// the sign-in sheet opens. `false` when it ships none, for the terminal's own flows to run.
pub fn begin(sync: &Sync, clients: &AppClients, tracker: &str, tx: &Outbox) -> bool {
    let state = mal::Pkce::generate().verifier;
    let (url, verifier) = match (tracker, &clients.anilist, &clients.mal) {
        ("anilist", Some(client), _) => match auth::implicit_url(client, &state) {
            Ok(url) => (url, None),
            Err(e) => {
                let _ = tx.send(Event::Toast(Notice::alert(e.to_string())));
                return true;
            }
        },
        ("mal", _, Some(client)) => {
            let pkce = mal::Pkce::generate();
            let url =
                mal::authorize_url_with_state(client, &pkce, &redirect_uri("mal"), &state);
            (url, Some(pkce.verifier))
        }
        _ => return false,
    };
    sync.pending.put(tracker, Pending { state, verifier });
    let _ = tx.send(Event::Authorize {
        tracker: tracker.to_owned(),
        url,
        callback_scheme: CALLBACK_SCHEME.into(),
    });
    true
}

/// Finish signing in to `tracker` with the redirect the sheet came back with: checked against
/// the sign-in that asked for it, then the token kept and handed to the live tracker.
pub async fn finish(
    sync: Sync,
    clients: AppClients,
    tracker: String,
    callback: String,
    tx: Outbox,
) {
    let alert = |text: String| {
        let _ = tx.send(Event::Toast(Notice::alert(text)));
    };
    let Some(pending) = sync.pending.take(&tracker) else {
        return alert(format!("no sign-in to {tracker} is waiting for that"));
    };
    let params = auth::callback_params(&callback);
    let param = |name: &str| params.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
    // A redirect for some other request, or one replayed: not this sign-in's to finish, and
    // not one to end it either, so the sheet's own redirect can still arrive. One with no
    // state at all is taken only from AniList's implicit grant, with its token: its docs do not
    // promise to carry the state back, and refusing every real sign-in over that would be no
    // safety at all. It is said in the log, to be checked against the registered client.
    let implicit_token = tracker == "anilist" && param("access_token").is_some();
    match param("state") {
        Some(state) if state == pending.state => {}
        None if implicit_token => {
            tracing::warn!(%tracker, "a sign-in redirect with no state, taken for its token");
        }
        _ => {
            sync.pending.put(&tracker, pending);
            return alert(format!(
                "that sign-in to {tracker} did not come back from this request"
            ));
        }
    }
    if let Some(error) = param("error") {
        let reason = param("error_description").unwrap_or(error);
        return alert(format!("sign-in to {tracker} failed: {reason}"));
    }
    let pair = match (tracker.as_str(), param("access_token"), param("code")) {
        ("anilist", Some(token), _) => TokenPair {
            access: token.to_owned(),
            refresh: None,
            expires_at: auth::token_expiry(token),
        },
        ("mal", _, Some(code)) => {
            let (Some(client), Some(verifier)) = (&clients.mal, &pending.verifier) else {
                return alert("no MyAnimeList client to finish with".into());
            };
            let exchanged = mal::exchange_code(
                sync.http.plain(),
                client,
                code,
                verifier,
                &redirect_uri("mal"),
                anistream_store::now(),
            )
            .await;
            match exchanged {
                Ok(pair) => pair,
                Err(e) => return alert(format!("token exchange failed: {e}")),
            }
        }
        _ => return alert(format!("the sign-in to {tracker} came back with no token")),
    };
    // AniList's as a bare token, as the terminal keeps it; MyAnimeList's with its refresh.
    let kept = match tracker.as_str() {
        "anilist" => sync.tokens.set(&tracker, &pair.access),
        _ => sync.tokens.set_pair(
            &tracker,
            &pair.access,
            pair.refresh.as_deref(),
            pair.expires_at,
        ),
    };
    match kept {
        Ok(_) => {
            let name = if tracker == "mal" { "MyAnimeList" } else { "AniList" };
            let _ = tx.send(Event::Toast(Notice::info(format!("signed in to {name}"))));
            activate(&sync, &tracker, &pair, &tx).await;
        }
        Err(e) => alert(format!("could not store the token: {e}")),
    }
}

/// The sheet for `tracker` was closed: the sign-in it was for is over.
pub fn cancel(sync: &Sync, tracker: &str) {
    if sync.pending.take(tracker).is_some() {
        tracing::info!(tracker, "sign-in cancelled");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_apps_clients_turn_their_trackers_on_for_the_trackers_alone() {
        let config = Config::default();
        let clients =
            AppClients { anilist: Some("4242".into()), mal: Some("a-mal-client".into()) };
        let built = with_app_clients(&config, &clients);
        assert!(built.trackers.is_enabled("anilist") && built.trackers.is_enabled("mal"));
        assert_eq!(built.trackers.mal.client_id.as_deref(), Some("a-mal-client"));
        assert_eq!(built.trackers.anilist.client_id, None, "AniList's is never put in");
        assert_eq!(with_app_clients(&config, &AppClients::default()), config);
        assert_eq!(redirect_uri("mal"), "anistream://auth/mal");
    }
}
