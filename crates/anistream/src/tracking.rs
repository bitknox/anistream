//! Wiring sync to the UI.
//!
//! The engine in `anistream-track` is deliberately ignorant of the interface; this is the part
//! that builds trackers from config, runs the background loops, and turns their reports into
//! [`Update`]s. Two rules shape it:
//!
//! - **Sync never blocks anything.** Every loop lives on its own task, and a tracker being down
//!   produces one badge change rather than a stream of toasts.
//! - **Signing in is the only interactive part**, and it opens a browser, so it runs off the UI
//!   thread like everything else.

use std::sync::Arc;

use anistream_core::{
    config::Config,
    ids::AnilistId,
    traits::{TrackOp, Tracker},
};
use anistream_meta::anilist::AniList;
use anistream_store::Store;
use anistream_track::{AniListTracker, TokenStore, auth, sync};
use anistream_ui::{
    LibrarySegment,
    app::{ConflictRow, SyncState, Toast, Update},
};
use tokio::sync::mpsc;

/// Everything the sync loops need.
#[derive(Clone)]
pub struct Sync {
    pub trackers: Vec<Arc<dyn Tracker>>,
    /// The AniList tracker as its concrete type.
    ///
    /// The `Tracker` trait returns only the syncable projection — ids, progress, status, score —
    /// which is right for sync but not enough for the Library screen, which needs titles and
    /// cover art. Keeping the concrete handle is cheaper and clearer than putting `as_any` on a
    /// public trait for one caller's benefit.
    pub anilist: Option<Arc<AniListTracker>>,
    /// The MAL tracker as its concrete type, for the same reason as `anilist`: refresh is not on
    /// the `Tracker` trait — AniList has no such concept, and putting it there would make every
    /// implementation carry it.
    pub mal: Option<Arc<anistream_track::MalTracker>>,
    pub tokens: TokenStore,
    pub store: Store,
    pub config: Config,
    /// Kept so the OAuth token exchange goes through the same configured client — including its
    /// proxy settings — as every other request.
    pub http: anistream_net::HttpClient,
}

impl Sync {
    /// Build trackers from config.
    ///
    /// A tracker with no stored token is still constructed. That is deliberate: it makes the
    /// Accounts overlay able to offer a sign-in, and the outbox accumulates against its id in
    /// the meantime, so watching now and signing in later still syncs.
    pub fn build(config: &Config, store: &Store, http: &anistream_net::HttpClient) -> Self {
        Self::with_tokens(config, store, http, token_store(config))
    }

    /// Build against an existing token store.
    ///
    /// The store caches reads, so sharing one means the OS keychain is consulted once per process
    /// instead of once per caller — which on macOS is the difference between one password prompt
    /// and five.
    pub fn with_tokens(
        config: &Config,
        store: &Store,
        http: &anistream_net::HttpClient,
        tokens: TokenStore,
    ) -> Self {
        let mut trackers: Vec<Arc<dyn Tracker>> = Vec::new();
        let mut anilist = None;
        let mut mal = None;

        if config.trackers.is_enabled("anilist") {
            let token = tokens.get("anilist").ok();
            if token.is_none() {
                tracing::info!("anilist enabled but not signed in; queueing locally");
            }
            let client =
                AniList::new(http.clone(), config.network.anilist_rate_limit).with_token(token);
            let tracker = Arc::new(AniListTracker::new(client));
            trackers.push(tracker.clone());
            anilist = Some(tracker);
        }

        if config.trackers.is_enabled("mal")
            && let Some(client_id) = config.trackers.mal.client_id.clone()
        {
            // MAL keys on `mal_id`, so this is the first tracker that needs the mapping layer —
            // exactly what the plan said would make a second tracker cheap rather than a new
            // subsystem.
            let tokens_for_mal = tokens.get_pair("mal").ok();
            if tokens_for_mal.is_none() {
                tracing::info!("mal enabled but not signed in; queueing locally");
            }
            let tracker = Arc::new(anistream_track::MalTracker::new(
                client_id,
                http.plain().clone(),
                Arc::new(StoreMapping { store: store.clone() }),
                tokens_for_mal,
            ));
            trackers.push(tracker.clone());
            mal = Some(tracker);
        }

        // Simkl bridges through `mal_id`, which the mapping table already holds for MAL's sake —
        // so the third tracker cost an auth flow and a push call, exactly as the plan predicted a
        // second identity system would not be needed.
        if config.trackers.is_enabled("simkl")
            && let Some(client_id) = config.trackers.simkl.client_id.clone()
        {
            trackers.push(Arc::new(anistream_track::SimklTracker::new(
                client_id,
                http.plain().clone(),
                Arc::new(StoreMapping { store: store.clone() }),
                tokens.get_pair("simkl").ok(),
            )));
        }

        // Trakt keys on TVDB, which the datasets cover far less completely — see the module docs.
        if config.trackers.is_enabled("trakt")
            && let Some(client_id) = config.trackers.trakt.client_id.clone()
        {
            trackers.push(Arc::new(anistream_track::TraktTracker::new(
                client_id,
                http.plain().clone(),
                Arc::new(StoreMapping { store: store.clone() }),
                tokens.get_pair("trakt").ok(),
            )));
        }

        Self {
            trackers,
            anilist,
            mal,
            tokens,
            store: store.clone(),
            config: config.clone(),
            http: http.clone(),
        }
    }

    /// The initial state for each tracker, so the badge is right before any sync runs.
    pub fn initial_states(&self) -> Vec<SyncState> {
        self.trackers
            .iter()
            .map(|tracker| {
                let id = tracker.id();
                let storage = self.tokens.storage_for(id);
                SyncState {
                    tracker: id.to_owned(),
                    connected: tracker.is_authenticated(),
                    user: None,
                    storage: Some(storage.describe().to_owned()),
                    storage_degraded: storage.is_degraded(),
                    outbox: self.store.outbox_depth(Some(id)).unwrap_or(0),
                    needs_reauth: false,
                    last: None,
                }
            })
            .collect()
    }

    fn state_for(
        &self,
        tracker: &dyn Tracker,
        needs_reauth: bool,
        last: Option<String>,
    ) -> SyncState {
        let id = tracker.id();
        let storage = self.tokens.storage_for(id);
        SyncState {
            tracker: id.to_owned(),
            connected: tracker.is_authenticated(),
            user: None,
            storage: Some(storage.describe().to_owned()),
            storage_degraded: storage.is_degraded(),
            outbox: self.store.outbox_depth(Some(id)).unwrap_or(0),
            needs_reauth,
            last,
        }
    }

    /// The first tracker's state with a freshly-read queue depth.
    ///
    /// Used right after queueing something, so the `⇅` badge moves the instant you act rather
    /// than at the next drain tick.
    pub fn state_after_enqueue(&self) -> SyncState {
        match self.trackers.first() {
            Some(tracker) => self.state_for(tracker.as_ref(), false, Some("queued".into())),
            None => SyncState::default(),
        }
    }
}

/// The materialised ID mapping, as trackers that key on something other than AniList need it.
///
/// Reads both directions out of the `mapping` table the dataset refresh fills. A miss is `None`
/// rather than an error: the tracker turns that into a named failure, because a title silently not
/// syncing is the thing the mapping layer exists to prevent.
struct StoreMapping {
    store: Store,
}

impl StoreMapping {
    /// Episodes in the seasons before this AniList entry, as the datasets report it.
    ///
    /// Clamped at zero: `episode_offset` is stored signed and a negative value would mean "this
    /// entry starts before the series does", which is not a thing.
    fn offset_of(&self, anilist_id: AnilistId) -> u32 {
        self.store
            .mapping_for(anilist_id)
            .ok()
            .flatten()
            .and_then(|m| m.episode_offset)
            .unwrap_or(0)
            .max(0) as u32
    }
}

/// Trakt needs season-relative numbering, which is what the datasets' `episode_offset` is for.
///
/// This is the field Fribb's corpus carries and ThaUnknown's does not — the reason both are merged.
/// Where there is no offset the answer is season 1, which is right for the great majority of titles
/// and wrong in a knowable way for split cours.
impl anistream_track::SeasonMapping for StoreMapping {
    fn tvdb_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.tvdb_id)
    }

    fn season_episode(&self, anilist_id: AnilistId, absolute: u32) -> Option<(u32, u32)> {
        let offset = self.offset_of(anilist_id);
        if offset > 0 {
            // An offset *is* the statement "this entry starts partway into a TVDB series", so it
            // both identifies a later season and gives the number to subtract. Saturating, because
            // an offset larger than the episode number would otherwise wrap.
            return Some((2, absolute.saturating_sub(offset).max(1)));
        }

        // No offset. That only means season one if this AniList entry is the *whole* TVDB series —
        // and measured against the real table, for 3,606 titles it is not. Refusing there is the
        // difference between skipping a push and writing S3E02 onto S1E02.
        let tvdb_id = self.tvdb_id(anilist_id)?;
        match self.store.anilist_entries_for_tvdb(tvdb_id) {
            Ok(1) => Some((1, absolute.max(1))),
            Ok(_) => None,
            // A failed lookup is treated as ambiguous. Guessing on a read error would be the worst
            // of both: wrong data, and no idea why.
            Err(e) => {
                tracing::warn!(error = %e, "could not check tvdb season ambiguity");
                None
            }
        }
    }

    fn absolute_episode(&self, anilist_id: AnilistId, season: u32, episode: u32) -> u32 {
        if season <= 1 {
            return episode;
        }
        episode + self.offset_of(anilist_id)
    }

    fn anilist_for_tvdb(&self, tvdb_id: u32) -> Option<AnilistId> {
        self.store.anilist_id_for_tvdb(tvdb_id).ok().flatten()
    }
}

impl anistream_track::mal::IdMapping for StoreMapping {
    fn mal_id(&self, anilist_id: AnilistId) -> Option<u32> {
        self.store.mapping_for(anilist_id).ok().flatten().and_then(|m| m.mal_id)
    }

    fn anilist_id(&self, mal_id: u32) -> Option<AnilistId> {
        self.store.anilist_id_for_mal(mal_id).ok().flatten()
    }
}

/// Fetch one segment of the tracker's list for the Library screen.
///
/// Titles are remembered as they pass through, which is what later lets a sync conflict name the
/// show instead of printing a bare id.
pub async fn load_library(sync: &Sync, store: &Store, segment: LibrarySegment) -> Update {
    use anistream_ui::app::Content;

    let Some(tracker) = sync.trackers.first() else {
        return Update::Content(Content::Entries(Vec::new()));
    };
    if !tracker.is_authenticated() {
        // Not a failure. The Library screen renders its own "no account" state, which explains
        // that history works regardless.
        return Update::Content(Content::Entries(Vec::new()));
    }

    // The concrete handle, kept alongside the trait object rather than reached by downcasting:
    // the trait returns only the syncable projection, but this screen needs covers and titles
    // too, and fetching those per title would cost a request each against thirty a minute.
    let Some(anilist) = sync.anilist.as_ref() else {
        return Update::Content(Content::Entries(Vec::new()));
    };

    match anilist.library().await {
        Ok(entries) => {
            let wanted = segment.wire();
            let mut rows = Vec::new();
            for entry in entries.iter().filter(|e| e.status == wanted) {
                let _ = store.remember_title(entry.media.id, entry.media.title.display());
                let mut row = crate::data::entry_from(&entry.media, Some(store));
                // The tracker's progress, not the local projection: this screen is a view of
                // the remote list, and showing local numbers here would hide a divergence
                // rather than reveal it.
                let total = row.episodes.unwrap_or(0);
                row.progress = Some((entry.progress, (entry.progress + 1).min(total.max(1))));
                rows.push(row);
            }
            Update::Content(Content::Entries(rows))
        }
        Err(anistream_core::Error::Auth(_)) => {
            Update::Content(Content::Failed("sign in again — :accounts".into()))
        }
        Err(e) => Update::Content(Content::Failed(e.to_string())),
    }
}

/// Start the background drain and pull loops.
///
/// Both run forever. The drain is cheap when idle — one indexed count against SQLite — so it can
/// tick often; a pull costs a real request out of thirty a minute, so it does not.
pub fn spawn_loops(sync: Sync, tx: mpsc::UnboundedSender<Update>) {
    if sync.trackers.is_empty() {
        return;
    }

    let drain_every =
        std::time::Duration::from_secs(sync.config.trackers.drain_interval_secs.max(5));
    let pull_every =
        std::time::Duration::from_secs(sync.config.trackers.pull_interval_secs.max(60));

    {
        let sync = sync.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(drain_every);
            loop {
                ticker.tick().await;
                drain_once(&sync, &tx).await;
            }
        });
    }

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(pull_every);
        // The first tick is immediate: a pull at startup is what makes "watched two on my
        // phone" show up when you open the app.
        loop {
            ticker.tick().await;
            pull_once(&sync, &tx).await;
        }
    });
}

/// Write back any token a tracker renewed during a call.
///
/// MAL's tokens last about a month and refresh silently mid-call; without this the renewed pair
/// would live only in memory, and the next process would present the expired one — refreshing on
/// every single run until the *refresh* token itself expired.
async fn persist_renewed_tokens(sync: &Sync) {
    let Some(mal) = sync.mal.as_ref() else { return };

    // Taken rather than peeked, so a write failure does not silently discard it: the tracker keeps
    // the in-memory copy and offers it again on the next pass.
    if let Some(pair) = mal.take_renewed().await {
        match sync.tokens.set_pair(
            "mal",
            &pair.access,
            pair.refresh.as_deref(),
            pair.expires_at,
        ) {
            Ok(_) => tracing::info!("stored the renewed myanimelist token"),
            Err(e) => tracing::warn!(error = %e, "could not store the renewed token"),
        }
    }
}

/// One drain pass across every tracker.
pub async fn drain_once(sync: &Sync, tx: &mpsc::UnboundedSender<Update>) {
    let now = anistream_store::now();
    persist_renewed_tokens(sync).await;
    for tracker in &sync.trackers {
        match sync::drain(&sync.store, tracker.as_ref(), now).await {
            Ok(report) => {
                let last = if report.sent > 0 {
                    Some(format!("sent {}", report.sent))
                } else if report.failed > 0 {
                    Some("retrying".to_string())
                } else {
                    None
                };
                // Silent when nothing happened: a badge that changes on every idle tick is
                // noise, and there is no news to deliver.
                if !report.did_nothing() {
                    let _ = tx.send(Update::Sync(Box::new(sync.state_for(
                        tracker.as_ref(),
                        report.needs_reauth,
                        last,
                    ))));
                }
            }
            Err(e) => tracing::warn!(tracker = tracker.id(), error = %e, "drain failed"),
        }
    }
}

/// One library pull across every tracker.
pub async fn pull_once(sync: &Sync, tx: &mpsc::UnboundedSender<Update>) {
    let now = anistream_store::now();
    for tracker in &sync.trackers {
        if !tracker.is_authenticated() {
            continue;
        }
        let last_pull =
            sync.store.get_meta_i64(&pull_key(tracker.id())).unwrap_or(None).unwrap_or(0);

        match sync::pull(&sync.store, tracker.as_ref(), now, last_pull).await {
            Ok(report) => {
                let _ = sync.store.set_meta_i64(&pull_key(tracker.id()), now);
                let _ = tx.send(Update::Sync(Box::new(sync.state_for(
                    tracker.as_ref(),
                    false,
                    Some(format!("pulled {} titles", report.seen)),
                ))));

                // Conflicts are titles, so they need names — which means resolving each id
                // against what we already have locally rather than making more requests.
                if !report.conflicts.is_empty() {
                    let rows = report
                        .conflicts
                        .iter()
                        .map(|c| ConflictRow {
                            anilist_id: c.anilist_id,
                            title: sync
                                .store
                                .cached_title(c.anilist_id)
                                .unwrap_or(None)
                                .unwrap_or_else(|| format!("anilist {}", c.anilist_id.get())),
                            field: c.field.label().to_owned(),
                            local: c.local.clone(),
                            remote: c.remote.clone(),
                        })
                        .collect();
                    let _ = tx.send(Update::Conflicts(rows));
                }
            }
            Err(anistream_core::Error::Auth(_)) => {
                let _ = tx.send(Update::Sync(Box::new(sync.state_for(
                    tracker.as_ref(),
                    true,
                    Some("sign in again".into()),
                ))));
            }
            // A tracker outage is expected and must not produce a toast every interval; the
            // badge and the log carry it.
            Err(e) => tracing::warn!(tracker = tracker.id(), error = %e, "library pull failed"),
        }
    }
}

/// Run a tracker's sign-in flow, then report the result.
///
/// Opens a browser and waits. Everything here is best-effort and every failure is reported as a
/// toast rather than propagated — a failed sign-in must leave the app exactly as usable as it
/// was, because local history never needed an account.
pub async fn connect(sync: Sync, tracker_id: String, tx: mpsc::UnboundedSender<Update>) {
    // Simkl and Trakt use the device flow: no redirect URI to register, and it works over SSH.
    if let Some(endpoints) = device_endpoints(&tracker_id) {
        connect_device(sync, tracker_id, endpoints, tx).await;
        return;
    }
    if tracker_id == "mal" {
        connect_mal(sync, tx).await;
        return;
    }
    if tracker_id != "anilist" {
        let _ =
            tx.send(Update::Toast(Toast::alert(format!("{tracker_id}: no sign-in flow yet"))));
        return;
    }

    let auth_config = &sync.config.trackers.anilist;
    let blank = |field: &Option<String>| field.as_deref().unwrap_or("").trim().is_empty();
    if blank(&auth_config.client_id) || blank(&auth_config.client_secret) {
        // AniList supports only the authorization code grant, so both halves are required.
        let _ = tx.send(Update::Toast(Toast::alert(
            "set trackers.anilist.client_id and client_secret — run: anistream --login-url",
        )));
        return;
    }
    let (client_id, client_secret) = (
        auth_config.client_id.clone().unwrap_or_default(),
        auth_config.client_secret.clone().unwrap_or_default(),
    );
    let flow =
        if auth_config.flow == "paste" { auth::Flow::Paste } else { auth::Flow::Loopback };
    let port = auth_config.redirect_port;

    let url = match auth::authorize_url(&client_id, flow, port) {
        Ok(url) => url,
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(e.to_string())));
            return;
        }
    };

    // Start listening *before* opening the browser, or a fast redirect could arrive before the
    // socket exists.
    let listening = matches!(flow, auth::Flow::Loopback)
        .then(|| tokio::spawn(auth::wait_for_code_from(port, "AniList")));

    if let Err(e) = open::that_detached(&url) {
        let _ = tx.send(Update::Toast(Toast::alert(format!("could not open a browser: {e}"))));
        tracing::info!(%url, "open this to sign in");
    }

    let Some(listening) = listening else {
        // The paste flow needs a terminal to paste into, which the TUI is currently occupying.
        let _ = tx.send(Update::Toast(Toast::info(
            "paste flow: quit and run `anistream --login` instead",
        )));
        return;
    };

    let code = match listening.await {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("sign-in failed: {e}"))));
            return;
        }
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("sign-in task failed: {e}"))));
            return;
        }
    };

    // The code is single-use and short-lived, so it is exchanged immediately rather than stored.
    let token = match auth::exchange_code(
        sync.http.plain(),
        &client_id,
        &client_secret,
        &auth::redirect_uri(port),
        &code,
    )
    .await
    {
        Ok(token) => token,
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("token exchange failed: {e}"))));
            return;
        }
    };

    match sync.tokens.set(&tracker_id, &token) {
        Ok(storage) => {
            let mut message = format!("signed in to {tracker_id}");
            if storage.is_degraded() {
                // Not hidden: someone on a shared machine should know.
                message.push_str(" — token stored in a 0600 file, no keychain available");
            }
            let _ = tx.send(Update::Toast(Toast::info(message)));
            // This used to say "restart to start syncing", which was true and terrible: the token
            // was in the keychain and nothing running could see it. Handing it to the live tracker
            // is the whole fix.
            let pair = anistream_track::TokenPair {
                access: token.clone(),
                refresh: None,
                expires_at: auth::token_expiry(&token),
            };
            activate(&sync, &tracker_id, &pair, &tx).await;
        }
        Err(e) => {
            let _ =
                tx.send(Update::Toast(Toast::alert(format!("could not store the token: {e}"))));
        }
    }
}

/// Sign in to MyAnimeList from inside the app.
///
/// There was no interactive flow at all — the Accounts screen answered "mal: no sign-in flow yet"
/// while `--login --tracker mal` worked perfectly from the command line. The PKCE machinery already
/// existed; only this wiring was missing.
async fn connect_mal(sync: Sync, tx: mpsc::UnboundedSender<Update>) {
    use anistream_track::mal;

    let auth = &sync.config.trackers.mal;
    let Some(client_id) = auth.client_id.clone().filter(|id| !id.trim().is_empty()) else {
        let _ = tx.send(Update::Toast(Toast::alert(
            "set trackers.mal.client_id — register an app (type: other) at myanimelist.net/apiconfig",
        )));
        return;
    };

    // The verifier has to survive until the exchange, so it is made before the browser opens and
    // held across the wait. MAL supports only `plain`, so it doubles as the challenge.
    let pkce = mal::Pkce::generate();
    let redirect = auth::redirect_uri(auth.redirect_port);
    let url = mal::authorize_url(&client_id, &pkce, &redirect);

    // Listening starts before the browser opens, or a fast redirect arrives before the socket does.
    let waiting = tokio::spawn(auth::wait_for_code_from(auth.redirect_port, "MyAnimeList"));
    if let Err(e) = open::that_detached(&url) {
        let _ = tx.send(Update::Toast(Toast::alert(format!("could not open a browser: {e}"))));
        tracing::info!(%url, "open this to sign in");
    }
    let _ = tx.send(Update::Status("waiting for MyAnimeList to redirect back…".into()));

    let code = match waiting.await {
        Ok(Ok(code)) => code,
        Ok(Err(e)) => {
            let _ = tx.send(Update::Toast(Toast::alert(e.to_string())));
            let _ = tx.send(Update::Status(String::new()));
            return;
        }
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("sign-in cancelled: {e}"))));
            let _ = tx.send(Update::Status(String::new()));
            return;
        }
    };

    let pair = match mal::exchange_code(
        sync.http.plain(),
        &client_id,
        &code,
        &pkce.verifier,
        &redirect,
        anistream_store::now(),
    )
    .await
    {
        Ok(pair) => pair,
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("token exchange failed: {e}"))));
            let _ = tx.send(Update::Status(String::new()));
            return;
        }
    };

    match sync.tokens.set_pair("mal", &pair.access, pair.refresh.as_deref(), pair.expires_at) {
        Ok(storage) => {
            let _ = tx.send(Update::Toast(Toast::info(format!(
                "signed in to MyAnimeList — token in the {}",
                storage.describe()
            ))));
            if pair.refresh.is_none() {
                // MAL's tokens last 30 days, so no refresh token means sync stops in a month with
                // nothing but a re-sign-in to fix it. Worth saying at the time.
                let _ = tx.send(Update::Toast(Toast::alert(
                    "no refresh token issued — you will have to sign in again in about a month",
                )));
            }
            activate(&sync, "mal", &pair, &tx).await;
        }
        Err(e) => {
            let _ =
                tx.send(Update::Toast(Toast::alert(format!("could not store the token: {e}"))));
        }
    }
    let _ = tx.send(Update::Status(String::new()));
}

/// Make a sign-in take effect immediately, everywhere.
///
/// Three things have to happen and all three were missing from at least one flow: the *live* tracker
/// has to be handed the credential (it captured one at construction, which is why signing in used to
/// end with "restart to start syncing"), the badge and Accounts screen have to be rebuilt from what
/// is now true, and a first pull has to be kicked off so the Library is not empty until the next
/// interval.
async fn activate(
    sync: &Sync,
    tracker_id: &str,
    pair: &anistream_track::TokenPair,
    tx: &mpsc::UnboundedSender<Update>,
) {
    if let Some(tracker) = sync.trackers.iter().find(|t| t.id() == tracker_id) {
        tracker
            .accept_credentials(&pair.access, pair.refresh.as_deref(), pair.expires_at)
            .await;
    }
    for state in sync.initial_states() {
        let _ = tx.send(Update::Sync(Box::new(state)));
    }
    // A pull now rather than at the next interval: having just signed in, an empty Library reads as
    // a failed sign-in.
    pull_once(sync, tx).await;
}

/// The device-flow endpoints for a tracker, if it uses one.
fn device_endpoints(tracker_id: &str) -> Option<anistream_track::DeviceEndpoints> {
    match tracker_id {
        "simkl" => Some(anistream_track::device::SIMKL),
        "trakt" => Some(anistream_track::device::TRAKT),
        _ => None,
    }
}

/// Sign in with the OAuth device flow.
///
/// The user's part is typing a five-character code into a web page, so the code is the *only* thing
/// worth putting in front of them — it goes in the status line where it stays put, not in a toast
/// that fades after nine seconds while they are still reaching for the keyboard.
async fn connect_device(
    sync: Sync,
    tracker_id: String,
    endpoints: anistream_track::DeviceEndpoints,
    tx: mpsc::UnboundedSender<Update>,
) {
    let (client_id, client_secret) = match tracker_id.as_str() {
        "simkl" => (sync.config.trackers.simkl.client_id.clone(), None),
        _ => (
            sync.config.trackers.trakt.client_id.clone(),
            sync.config.trackers.trakt.client_secret.clone(),
        ),
    };
    let Some(client_id) = client_id.filter(|id| !id.trim().is_empty()) else {
        let _ = tx.send(Update::Toast(Toast::alert(format!(
            "set trackers.{tracker_id}.client_id first"
        ))));
        return;
    };

    let http = sync.http.plain().clone();
    let code = match anistream_track::device::request_code(&http, &endpoints, &client_id).await
    {
        Ok(code) => code,
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("{tracker_id}: {e}"))));
            return;
        }
    };

    // Its own persistent prompt on the Accounts screen, not the status line. The status line was
    // wrong twice over: it is drawn in the dimmest role in the palette, and any background task
    // setting a status — a rate-limit notice, a finished download — would wipe the one thing the
    // user has to read. Reported as the code never being displayed, which it effectively was not.
    let _ = tx.send(Update::DeviceCode(Some(anistream_ui::app::DeviceCodePrompt {
        tracker: tracker_id.clone(),
        code: code.user_code.clone(),
        url: code.verification_url.clone(),
    })));
    if let Err(e) = open::that_detached(&code.verification_url) {
        tracing::info!(url = %code.verification_url, error = %e, "open this to approve");
    }

    match anistream_track::device::poll_for_token(
        &http,
        &endpoints,
        &client_id,
        client_secret.as_deref(),
        &code,
    )
    .await
    {
        Ok(pair) => {
            let storage = sync.tokens.set_pair(
                &tracker_id,
                &pair.access,
                pair.refresh.as_deref(),
                pair.expires_at,
            );
            let message = match storage {
                Ok(storage) => {
                    format!("signed in to {tracker_id} — token in the {}", storage.describe())
                }
                // Signed in but unable to store it: the session works and the next launch will not,
                // which is worth saying rather than reporting a plain success.
                Err(e) => {
                    format!("signed in to {tracker_id}, but the token was not saved: {e}")
                }
            };
            let _ = tx.send(Update::Toast(Toast::info(message)));
            activate(&sync, &tracker_id, &pair, &tx).await;
        }
        Err(e) => {
            let _ = tx.send(Update::Toast(Toast::alert(format!("{tracker_id}: {e}"))));
        }
    }
    // Cleared however the flow ended — approved, refused or expired. A prompt left up after a
    // successful sign-in would be asking for a code that no longer does anything.
    let _ = tx.send(Update::DeviceCode(None));
    let _ = tx.send(Update::Status(String::new()));
}

/// Forget a tracker's token.
pub fn disconnect(sync: &Sync, tracker_id: &str, tx: &mpsc::UnboundedSender<Update>) {
    let update = match sync.tokens.clear(tracker_id) {
        // The queue is deliberately left alone. Signing out is not "discard my progress", and
        // signing back in should send what accumulated.
        Ok(()) => Update::Toast(Toast::info(format!(
            "signed out of {tracker_id} — {} queued op(s) kept",
            sync.store.outbox_depth(Some(tracker_id)).unwrap_or(0)
        ))),
        Err(e) => Update::Toast(Toast::alert(format!("could not sign out: {e}"))),
    };
    let _ = tx.send(update);

    // Then make every part of the app agree, immediately. Clearing the stored token used to be the
    // whole of signing out, so the header badge and the Accounts list went on showing a connected
    // account until the next restart — and the trackers themselves went on pushing with the
    // credential, because they had cached it at construction.
    let (sync, tx, tracker_id) = (sync.clone(), tx.clone(), tracker_id.to_owned());
    tokio::spawn(async move {
        if let Some(tracker) = sync.trackers.iter().find(|t| t.id() == tracker_id) {
            tracker.forget_credentials().await;
        }
        // Rebuilt from what is now true rather than patched, so there is one description of the
        // state and no way for the badge and the list to disagree.
        for state in sync.initial_states() {
            let _ = tx.send(Update::Sync(Box::new(state)));
        }
    });
}

/// Queue a status change for every tracker.
pub fn set_status(sync: &Sync, id: AnilistId, status: LibrarySegment) {
    let now = anistream_store::now();
    let op = TrackOp::SetStatus { anilist_id: id, status: segment_to_status(status), at: now };
    for tracker in &sync.trackers {
        if let Err(e) = sync.store.enqueue(tracker.id(), &op, now) {
            tracing::warn!(error = %e, "could not queue a status change");
        }
    }
}

/// Settle a divergence.
///
/// Keeping the local value queues a push; keeping the remote one is simply dropping the row,
/// because the tracker already holds it.
pub fn resolve_conflict(sync: &Sync, id: AnilistId, keep_local: bool) {
    if !keep_local {
        return;
    }
    let now = anistream_store::now();
    let episodes = sync.store.completed_episode_count(id).unwrap_or(0);
    if episodes == 0 {
        return;
    }
    let op = TrackOp::SetProgress { anilist_id: id, episode: episodes };
    for tracker in &sync.trackers {
        let _ = sync.store.enqueue(tracker.id(), &op, now);
    }
}

fn segment_to_status(segment: LibrarySegment) -> anistream_core::traits::WatchStatus {
    use anistream_core::traits::WatchStatus as W;
    match segment {
        LibrarySegment::Watching => W::Current,
        LibrarySegment::Planning => W::Planning,
        LibrarySegment::Completed => W::Completed,
        LibrarySegment::Paused => W::Paused,
        LibrarySegment::Dropped => W::Dropped,
    }
}

fn pull_key(tracker_id: &str) -> String {
    format!("last_pull:{tracker_id}")
}

/// Where the file-fallback token lives, if a keychain is unavailable.
/// A token store honouring `trackers.token_storage` and the `ANISTREAM_TOKEN_STORAGE` override.
///
/// One place, so every entry point — the app, `--login`, `--sync`, the probes — agrees about where
/// tokens live. Disagreeing would mean signing in through one path and being unauthenticated
/// through another.
pub fn token_store(config: &Config) -> TokenStore {
    let backend = anistream_track::secret::Backend::from_env_or(&config.trackers.token_storage);
    TokenStore::with_backend(token_dir(), backend)
}

/// Where the `0600` token file lives when the file backend is in use.
pub fn token_dir() -> std::path::PathBuf {
    anistream_core::config::Paths::resolve()
        .map(|p| p.data_dir.join("tokens"))
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_segment_maps_to_a_status() {
        // These go into someone's list, so a wrong mapping silently marks a show dropped.
        use anistream_core::traits::WatchStatus as W;
        assert_eq!(segment_to_status(LibrarySegment::Watching), W::Current);
        assert_eq!(segment_to_status(LibrarySegment::Planning), W::Planning);
        assert_eq!(segment_to_status(LibrarySegment::Completed), W::Completed);
        assert_eq!(segment_to_status(LibrarySegment::Paused), W::Paused);
        assert_eq!(segment_to_status(LibrarySegment::Dropped), W::Dropped);
    }

    #[test]
    fn a_tracker_with_no_token_is_still_built() {
        // So the Accounts overlay can offer a sign-in, and so the outbox accumulates in the
        // meantime — watch now, sign in later.
        let mut config = Config::default();
        config.trackers.enabled = vec!["anilist".into()];
        config.trackers.anilist.client_id = Some("12345".into());
        let store = Store::open_in_memory().unwrap();
        let http = anistream_net::HttpClient::new(&config.network).unwrap();

        let sync = Sync::build(&config, &store, &http);
        assert_eq!(sync.trackers.len(), 1);
        assert_eq!(sync.trackers[0].id(), "anilist");
    }

    #[test]
    fn no_trackers_are_built_when_none_are_enabled() {
        let config = Config::default();
        let store = Store::open_in_memory().unwrap();
        let http = anistream_net::HttpClient::new(&config.network).unwrap();
        assert!(Sync::build(&config, &store, &http).trackers.is_empty());
    }

    #[test]
    fn the_pull_cursor_key_is_per_tracker() {
        // Shared, one tracker's pull would reset the other's last-write-wins baseline.
        assert_ne!(pull_key("anilist"), pull_key("mal"));
    }

    #[tokio::test]
    async fn signing_out_keeps_the_queue() {
        // "Sign out" is not "throw away what I watched". Async because signing out now also tells
        // the trackers themselves to drop their cached credentials and republishes the state — the
        // token store alone was never the whole of it.
        let mut config = Config::default();
        config.trackers.enabled = vec!["anilist".into()];
        config.trackers.anilist.client_id = Some("1".into());
        let store = Store::open_in_memory().unwrap();
        let http = anistream_net::HttpClient::new(&config.network).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let sync = Sync {
            tokens: TokenStore::new(dir.path()).file_only(),
            ..Sync::build(&config, &store, &http)
        };

        store
            .enqueue(
                "anilist",
                &TrackOp::SetProgress { anilist_id: AnilistId::new(1), episode: 5 },
                0,
            )
            .unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        disconnect(&sync, "anilist", &tx);
        assert_eq!(store.outbox_depth(Some("anilist")).unwrap(), 1);
    }
}
