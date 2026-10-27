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
use anistream_model::{Msg, event::Event, notice::Notice};
use anistream_track::{TokenPair, auth, mal};

use super::{Sync, activate};
use crate::{options::AppClients, outbox::Outbox};

/// The URL scheme the app's redirects come back on, unless its client registered another.
pub const CALLBACK_SCHEME: &str = "anistream";

/// Where `tracker`'s sign-in redirects to, on the app's own scheme: what its client registers
/// unless the app says otherwise ([`AppClients::anilist_redirect`]).
pub fn redirect_uri(tracker: &str) -> String {
    format!("{CALLBACK_SCHEME}://auth/{tracker}")
}

impl AppClients {
    /// Where `tracker`'s sign-in redirects to: the one the app's client registered, or the app's
    /// own scheme.
    pub fn redirect(&self, tracker: &str) -> String {
        let registered = match tracker {
            "anilist" => self.anilist_redirect.as_deref(),
            "mal" => self.mal_redirect.as_deref(),
            _ => None,
        };
        registered
            .map(str::trim)
            .filter(|uri| !uri.is_empty())
            .map_or_else(|| redirect_uri(tracker), str::to_owned)
    }

    /// The scheme `tracker`'s redirect comes back on, for the sheet to watch for: `https` for an
    /// App Link, the app's own otherwise.
    pub fn callback_scheme(&self, tracker: &str) -> String {
        let redirect = self.redirect(tracker);
        redirect.split_once("://").map_or(CALLBACK_SCHEME, |(scheme, _)| scheme).to_owned()
    }
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

/// `config`, as the trackers are built from it for an app: each tracker turned on where the app
/// ships a client for it, and MyAnimeList's, Simkl's and Trakt's clients among its settings, which
/// their trackers sign in, call and renew tokens with, where the file names none of its own. The
/// file is never given any of it.
pub fn with_app_clients(config: &Config, clients: &AppClients) -> Config {
    let mut config = config.clone();
    let trackers = &mut config.trackers;
    for (id, client) in [
        ("anilist", &clients.anilist),
        ("mal", &clients.mal),
        ("simkl", &clients.simkl),
        ("trakt", &clients.trakt),
    ] {
        if client.is_some() && !trackers.is_enabled(id) {
            trackers.enabled.push(id.to_owned());
        }
    }
    let unset = |id: &Option<String>| id.as_deref().is_none_or(|id| id.trim().is_empty());
    if let Some(client) = &clients.mal
        && unset(&trackers.mal.client_id)
    {
        trackers.mal.client_id = Some(client.clone());
    }
    if let Some(client) = &clients.simkl
        && unset(&trackers.simkl.client_id)
    {
        trackers.simkl.client_id = Some(client.clone());
    }
    // The secret goes with the app's own client and no other: a file's client keeps its own.
    if let Some(client) = &clients.trakt
        && unset(&trackers.trakt.client_id)
    {
        trackers.trakt.client_id = Some(client.clone());
        trackers.trakt.client_secret.clone_from(&clients.trakt_secret);
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
                let _ = tx.send(Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
                return true;
            }
        },
        ("mal", _, Some(client)) => {
            let pkce = mal::Pkce::generate();
            let url =
                mal::authorize_url_with_state(client, &pkce, &clients.redirect("mal"), &state);
            (url, Some(pkce.verifier))
        }
        _ => return false,
    };
    sync.pending.put(tracker, Pending { state, verifier });
    let _ = tx.send(Event::Authorize {
        tracker: tracker.to_owned(),
        url,
        callback_scheme: clients.callback_scheme(tracker),
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
    let name = super::tracker_name(&tracker);
    let alert = |msg: Msg| {
        let _ = tx.send(Event::Toast(Notice::alert(msg)));
    };
    let Some(pending) = sync.pending.take(&tracker) else {
        return alert(Msg::NoticeSignInNotWaiting { tracker: name });
    };
    let params = auth::callback_params(&callback);
    let param = |name: &str| params.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str());
    // A redirect for some other request, or one replayed: not this sign-in's to finish, and
    // not one to end it either, so the sheet's own redirect can still arrive. Every request
    // asks with a state, so a redirect without one is refused too, AniList's implicit grant
    // with a token in it included, which carries the state back in the fragment beside the
    // token: taken, a token anyone's page sent the app to would sign it in to their account.
    if param("state") != Some(pending.state.as_str()) {
        tracing::warn!(
            %tracker,
            stated = param("state").is_some(),
            "a sign-in redirect that does not carry this request's state, refused"
        );
        sync.pending.put(&tracker, pending);
        return alert(Msg::NoticeSignInWrongRequest { tracker: name });
    }
    if let Some(error) = param("error") {
        let reason = param("error_description").unwrap_or(error);
        return alert(Msg::NoticeSignInFailed { tracker: name, reason: reason.to_owned() });
    }
    let pair = match (tracker.as_str(), param("access_token"), param("code")) {
        ("anilist", Some(token), _) => TokenPair {
            access: token.to_owned(),
            refresh: None,
            expires_at: auth::token_expiry(token),
        },
        ("mal", _, Some(code)) => {
            let (Some(client), Some(verifier)) = (&clients.mal, &pending.verifier) else {
                return alert(Msg::NoticeMalClientMissingBuild);
            };
            let exchanged = mal::exchange_code(
                sync.http.plain(),
                client,
                code,
                verifier,
                &clients.redirect("mal"),
                anistream_store::now(),
            )
            .await;
            match exchanged {
                Ok(pair) => pair,
                Err(e) => {
                    return alert(Msg::NoticeTokenExchangeFailed {
                        tracker: name,
                        reason: e.to_string(),
                    });
                }
            }
        }
        _ => return alert(Msg::NoticeSignInNoToken { tracker: name }),
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
            let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeSignedIn { tracker: name })));
            activate(&sync, &tracker, &pair, &tx).await;
        }
        Err(e) => alert(Msg::NoticeTokenNotSaved { reason: e.to_string() }),
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
        let clients = AppClients {
            anilist: Some("4242".into()),
            mal: Some("a-mal-client".into()),
            ..AppClients::default()
        };
        let built = with_app_clients(&config, &clients);
        assert!(built.trackers.is_enabled("anilist") && built.trackers.is_enabled("mal"));
        assert_eq!(built.trackers.mal.client_id.as_deref(), Some("a-mal-client"));
        assert_eq!(built.trackers.anilist.client_id, None, "AniList's is never put in");
        assert!(!built.trackers.is_enabled("simkl") && !built.trackers.is_enabled("trakt"));
        assert_eq!(with_app_clients(&config, &AppClients::default()), config);
        assert_eq!(redirect_uri("mal"), "anistream://auth/mal");
    }

    #[test]
    fn an_apps_simkl_and_trakt_clients_turn_them_on_and_a_files_own_client_stands() {
        let clients = AppClients {
            simkl: Some("a-simkl-client".into()),
            trakt: Some("a-trakt-client".into()),
            ..AppClients::default()
        };
        let built = with_app_clients(&Config::default(), &clients);
        assert!(built.trackers.is_enabled("simkl") && built.trackers.is_enabled("trakt"));
        assert_eq!(built.trackers.simkl.client_id.as_deref(), Some("a-simkl-client"));
        assert_eq!(built.trackers.trakt.client_id.as_deref(), Some("a-trakt-client"));
        assert_eq!(built.trackers.trakt.client_secret, None, "Trakt's new client has none");

        let with_secret =
            AppClients { trakt_secret: Some("a-secret".into()), ..clients.clone() };
        let built = with_app_clients(&Config::default(), &with_secret);
        assert_eq!(built.trackers.trakt.client_secret.as_deref(), Some("a-secret"));

        let mut own = Config::default();
        own.trackers.trakt.client_id = Some("mine".into());
        own.trackers.trakt.client_secret = Some("my-secret".into());
        let built = with_app_clients(&own, &with_secret);
        assert_eq!(built.trackers.trakt.client_id.as_deref(), Some("mine"));
        assert_eq!(built.trackers.trakt.client_secret.as_deref(), Some("my-secret"));
    }

    #[test]
    fn a_registered_redirect_replaces_the_scheme_and_says_its_own() {
        let scheme = AppClients::default();
        assert_eq!(scheme.redirect("anilist"), "anistream://auth/anilist");
        assert_eq!(scheme.callback_scheme("anilist"), "anistream");
        let linked = AppClients {
            anilist_redirect: Some("https://anistream.tv/auth/anilist".into()),
            mal_redirect: Some("  ".into()),
            ..AppClients::default()
        };
        assert_eq!(linked.redirect("anilist"), "https://anistream.tv/auth/anilist");
        assert_eq!(linked.callback_scheme("anilist"), "https");
        // A blank one is none: MyAnimeList keeps the app's scheme.
        assert_eq!(linked.redirect("mal"), "anistream://auth/mal");
        assert_eq!(linked.callback_scheme("mal"), "anistream");
    }
}
