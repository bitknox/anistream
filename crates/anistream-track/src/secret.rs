//! Token storage.
//!
//! An AniList token is a year-long bearer credential for someone's whole account, so it does
//! not belong in `config.toml` next to their quality preference. The OS credential store is
//! first choice; a `0600` file under the data directory is the fallback for machines without
//! one (a headless Linux box with no D-Bus session, typically).
//!
//! The fallback is deliberately *reported* rather than silent — see [`Storage`]. Somebody
//! running this on a shared machine deserves to know their token is on disk.
//!
//! Two layers, so that where a token is kept can be someone else's to say. A [`SecretStore`]
//! keeps values by name and nothing more: [`LocalSecrets`] is the desktop's, the keychain or the
//! file, and a front end on a phone offers the platform's keychain through its host instead. A
//! [`TokenStore`] over either gives the values their shape, a bare token or a pair, and reads
//! each one once per process.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// The keyring service name. Stable, because changing it would orphan every stored token.
#[cfg_attr(
    any(test, not(feature = "keychain"), target_os = "ios", target_os = "android"),
    allow(dead_code)
)]
const SERVICE: &str = "anistream";

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("no token stored for {0}")]
    Missing(String),
    #[error("keychain: {0}")]
    Keychain(String),
    #[error("token file {path}: {source}")]
    File {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Where a token actually ended up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Storage {
    /// The OS credential store — encrypted at rest, unlocked with the login session.
    Keychain,
    /// A `0600` file. Readable by this user, and by root.
    File,
}

impl Storage {
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Keychain => "OS keychain",
            Self::File => "0600 file",
        }
    }

    /// Whether the user should be told about this rather than it being an implementation
    /// detail.
    pub const fn is_degraded(self) -> bool {
        matches!(self, Self::File)
    }
}

/// An access token, and what is needed to renew it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenPair {
    pub access: String,
    /// `None` for a tracker that does not issue one — AniList, for instance.
    pub refresh: Option<String>,
    /// When the access token stops working, if the tracker said.
    pub expires_at: Option<i64>,
}

impl TokenPair {
    /// Read either the JSON pair or a bare token.
    ///
    /// A bare string is not an error: it is what an earlier build stored, and what someone pasting
    /// a token by hand produces. Treating it as corrupt would sign them out for no reason.
    fn parse(raw: &str) -> Self {
        let trimmed = raw.trim();
        let parsed = serde_json::from_str::<serde_json::Value>(trimmed)
            .ok()
            .filter(|v| v.get("access").and_then(|a| a.as_str()).is_some());

        match parsed {
            Some(value) => Self {
                access: value["access"].as_str().unwrap_or_default().to_owned(),
                refresh: value["refresh"].as_str().map(str::to_owned),
                expires_at: value["expires_at"].as_i64(),
            },
            None => Self { access: trimmed.to_owned(), refresh: None, expires_at: None },
        }
    }

    /// Whether the access token should be renewed before use.
    ///
    /// Renews `margin` seconds early, because a token that expires mid-request fails in a way that
    /// looks like a rejected credential rather than an expired one.
    pub fn needs_refresh(&self, now: i64, margin: i64) -> bool {
        self.refresh.is_some() && self.expires_at.is_some_and(|at| now + margin >= at)
    }
}

/// Somewhere secrets are kept by name: what a [`TokenStore`] reads and writes through.
///
/// One value per id — a tracker's id, or the key a plugin store's token goes by — kept exactly as
/// it is given. What a value means, a bare token or a JSON pair, is the [`TokenStore`]'s business,
/// so a store only keeps it and hands it back.
///
/// Shared by every task that reads a token, on whichever thread runs it, and asked in the middle
/// of the work, which waits for the answer: so a call should be quick, a keychain's lookup at
/// most. The [`TokenStore`] in front reads each value once per process.
pub trait SecretStore: Send + Sync {
    /// What is kept under `id`, or `None` when nothing is.
    fn get(&self, id: &str) -> Result<Option<String>, SecretError>;

    /// Keep `value` under `id`, in place of whatever was kept there, and say where it went.
    fn set(&self, id: &str, value: &str) -> Result<Storage, SecretError>;

    /// Forget what is kept under `id`, wherever it is. Nothing kept is no error: signing out
    /// twice must not fail.
    fn delete(&self, id: &str) -> Result<(), SecretError>;

    /// Where a value under `id` is kept, or would be: what the Accounts screen says of it.
    fn storage_for(&self, id: &str) -> Storage;
}

/// Which store to use.
///
/// Selectable rather than automatic because of a problem with no clean workaround: on macOS the
/// keychain's access control is keyed on the *binary*, and every `cargo build` produces a different
/// one. "Always Allow" can therefore never stick during development, so anyone iterating on
/// anistream gets a password prompt per run. A file backend removes that entirely, at the cost of a
/// `0600` file instead of an encrypted store — a trade the user should get to make, not one this
/// code should make for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backend {
    /// Prefer the OS credential store, falling back to a file where there is none.
    #[default]
    Keychain,
    /// Use a `0600` file and never touch the keychain — so never prompt.
    File,
}

impl Backend {
    /// Parse a config value. Anything unrecognised is the safer option.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "file" => Self::File,
            _ => Self::Keychain,
        }
    }

    /// The environment override, for the development loop.
    ///
    /// `ANISTREAM_TOKEN_STORAGE=file` skips the keychain without editing config, which is what you
    /// want when the thing you are running is a fresh `cargo` build every time.
    pub const ENV: &'static str = "ANISTREAM_TOKEN_STORAGE";

    pub fn from_env_or(configured: &str) -> Self {
        Self::chosen(std::env::var(Self::ENV).ok().as_deref(), configured)
    }

    /// The backend `env`, the override's value, says when it is set, and `configured` says
    /// otherwise: the environment wins, so one run can keep the keychain out of it without the
    /// config being edited. [`Self::from_env_or`] with the environment read.
    pub fn chosen(env: Option<&str>, configured: &str) -> Self {
        Self::parse(env.unwrap_or(configured))
    }
}

/// The secrets anistream keeps on the machine by itself: in the OS keychain where one answers,
/// with a `0600` file under its directory as the fallback, or in the file alone for
/// [`Backend::File`]. What the desktop keeps tokens in.
///
/// A phone's keychain belongs to the app, not to a library reaching for it, so on iOS and Android
/// this has no keychain to ask and keeps the file alone: a front end there offers the platform's
/// own store through its host. So does a build without the `keychain` feature, a native app's on
/// macOS among them.
#[derive(Debug, Clone)]
pub struct LocalSecrets {
    dir: PathBuf,
    backend: Backend,
}

impl LocalSecrets {
    /// Secrets in `dir`, or in the keychain first as `backend` says.
    pub fn new(dir: impl Into<PathBuf>, backend: Backend) -> Self {
        Self { dir: dir.into(), backend }
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Whether this store will ever consult the OS keychain.
    fn uses_keychain(&self) -> bool {
        self.backend == Backend::Keychain
    }

    /// Move a token out of the keychain and into a file.
    ///
    /// Costs exactly one keychain read — the last one you will see. Exists so switching backends
    /// does not mean signing in again, which for a year-long token would be a silly thing to
    /// require.
    pub fn migrate_to_file(&self, tracker_id: &str) -> Result<Storage, SecretError> {
        // Read through the keychain regardless of this store's backend: the whole point is to
        // retrieve something the *other* backend holds.
        let Some(token) = keychain::get(tracker_id) else {
            // Nothing there. If a file already exists this is a no-op success, not a failure.
            return if self.path(tracker_id).exists() {
                Ok(Storage::File)
            } else {
                Err(SecretError::Missing(tracker_id.to_owned()))
            };
        };

        self.write_file(tracker_id, &token)?;
        // Removed from the keychain, so there is one copy rather than two diverging ones.
        keychain::delete(tracker_id);
        Ok(Storage::File)
    }

    fn path(&self, tracker_id: &str) -> PathBuf {
        // Sanitised: a tracker id reaches here from config, and `../` in a filename would be
        // a path traversal into someone's home directory.
        let safe: String = tracker_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.dir.join(format!("{safe}.token"))
    }

    fn write_file(&self, tracker_id: &str, token: &str) -> Result<(), SecretError> {
        let path = self.path(tracker_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|source| SecretError::File { path: path.clone(), source })?;
        }
        std::fs::write(&path, token)
            .map_err(|source| SecretError::File { path: path.clone(), source })?;
        restrict(&path).map_err(|source| SecretError::File { path, source })
    }
}

impl SecretStore for LocalSecrets {
    fn get(&self, tracker_id: &str) -> Result<Option<String>, SecretError> {
        if self.uses_keychain()
            && let Some(token) = keychain::get(tracker_id)
        {
            return Ok(Some(token));
        }

        let path = self.path(tracker_id);
        match std::fs::read_to_string(&path) {
            Ok(token) => Ok(Some(token.trim().to_owned())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(SecretError::File { path, source }),
        }
    }

    fn set(&self, tracker_id: &str, token: &str) -> Result<Storage, SecretError> {
        if self.uses_keychain() && keychain::set(tracker_id, token) {
            // Remove any earlier file copy, or a stale token would outlive the good one and
            // could be picked up after a keychain reset.
            let _ = std::fs::remove_file(self.path(tracker_id));
            return Ok(Storage::Keychain);
        }

        self.write_file(tracker_id, token)?;
        if self.uses_keychain() {
            // A fallback rather than a choice, which is worth saying.
            tracing::warn!(
                tracker = tracker_id,
                "no OS keychain available; token written to a 0600 file"
            );
        }
        Ok(Storage::File)
    }

    fn delete(&self, tracker_id: &str) -> Result<(), SecretError> {
        if self.uses_keychain() {
            keychain::delete(tracker_id);
        }
        match std::fs::remove_file(self.path(tracker_id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(SecretError::File { path: self.path(tracker_id), source }),
        }
    }

    fn storage_for(&self, tracker_id: &str) -> Storage {
        // The configured backend decides, not a guess from what happens to be on disk. This used to
        // report `Keychain` whenever no token file existed — so a store explicitly configured for
        // file mode, with nothing signed in yet, told the user its token was in the keychain. That
        // is the exact wrong answer for the one setting that exists to keep the keychain out of it,
        // and it surfaced on the Accounts screen as "file mode · OS keychain".
        match self.backend {
            Backend::File => Storage::File,
            // In keychain mode a file can still be present, from `--token-to-file` before the
            // setting was changed back. What is actually holding the token wins.
            Backend::Keychain => {
                if self.path(tracker_id).exists() {
                    Storage::File
                } else {
                    Storage::Keychain
                }
            }
        }
    }
}

/// The OS credential store, as `keyring` reaches it: the macOS keychain, Windows' credential
/// manager, or the Secret Service over D-Bus. Each call is best effort, and a keychain that does
/// not answer is one that is not there.
#[cfg(all(feature = "keychain", not(any(test, target_os = "ios", target_os = "android"))))]
mod keychain {
    use super::SERVICE;

    pub(super) fn get(id: &str) -> Option<String> {
        keyring::Entry::new(SERVICE, id).ok()?.get_password().ok()
    }

    /// Whether the keychain took it.
    pub(super) fn set(id: &str, value: &str) -> bool {
        keyring::Entry::new(SERVICE, id).is_ok_and(|entry| entry.set_password(value).is_ok())
    }

    pub(super) fn delete(id: &str) {
        if let Ok(entry) = keyring::Entry::new(SERVICE, id) {
            let _ = entry.delete_credential();
        }
    }
}

/// No keychain on a phone: there the system's belongs to the app, which offers it through the
/// host it runs the engine on, so [`LocalSecrets`] keeps the file alone. The same without the
/// `keychain` feature, which a native app's build leaves off on the desktop too.
#[cfg(all(
    not(test),
    any(not(feature = "keychain"), target_os = "ios", target_os = "android")
))]
mod keychain {
    pub(super) fn get(_: &str) -> Option<String> {
        None
    }

    pub(super) fn set(_: &str, _: &str) -> bool {
        false
    }

    pub(super) fn delete(_: &str) {}
}

/// This crate's tests get a keychain of their own, one per thread, so that none of them reads,
/// writes or deletes anything in the keychain of whoever runs them — the keychain path tested all
/// the same. [`keychain::refuse`] makes it one that does not answer.
#[cfg(test)]
mod keychain {
    use std::{cell::RefCell, collections::HashMap};

    thread_local! {
        static KEPT: RefCell<Option<HashMap<String, String>>> = RefCell::new(Some(HashMap::new()));
    }

    pub(super) fn get(id: &str) -> Option<String> {
        KEPT.with_borrow(|kept| kept.as_ref()?.get(id).cloned())
    }

    pub(super) fn set(id: &str, value: &str) -> bool {
        KEPT.with_borrow_mut(|kept| {
            kept.as_mut().map(|kept| kept.insert(id.to_owned(), value.to_owned())).is_some()
        })
    }

    pub(super) fn delete(id: &str) {
        KEPT.with_borrow_mut(|kept| kept.as_mut().map(|kept| kept.remove(id)));
    }

    /// From now on, on this thread, a keychain that does not answer: a headless box's.
    pub(super) fn refuse() {
        KEPT.set(None);
    }
}

/// Reads and writes tracker tokens, through whichever [`SecretStore`] keeps them.
#[derive(Clone)]
pub struct TokenStore {
    secrets: Arc<dyn SecretStore>,
    /// Tokens already read this process, so the keychain is touched once per tracker.
    ///
    /// Not an optimisation — a correctness-of-experience fix. On macOS every keychain read by an
    /// unsigned binary can prompt for the login password, and this type is consulted from several
    /// places per run (building the tracker, rendering the Accounts overlay, reporting expiry).
    /// Without the cache that is one password prompt each.
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl std::fmt::Debug for TokenStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never the cache: it holds the tokens.
        f.debug_struct("TokenStore").finish_non_exhaustive()
    }
}

impl TokenStore {
    /// A store using the OS keychain where available.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_backend(dir, Backend::Keychain)
    }

    /// A store in `dir`, through the keychain first or not as `backend` says: see
    /// [`LocalSecrets`]. [`Backend::File`] is what a test wants: a suite that wrote to the real
    /// login keychain would prompt for permission, leak state between runs, and leave
    /// credentials behind.
    pub fn with_backend(dir: impl Into<PathBuf>, backend: Backend) -> Self {
        Self::over(Arc::new(LocalSecrets::new(dir, backend)))
    }

    /// A store over `secrets`, wherever they are kept: a phone's keychain, as the host the
    /// engine runs on offers it.
    pub fn over(secrets: Arc<dyn SecretStore>) -> Self {
        Self { secrets, cache: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Store a token, returning where it went.
    pub fn set(&self, tracker_id: &str, token: &str) -> Result<Storage, SecretError> {
        // Cached immediately: a sign-in is followed by a read, and going back to the keychain
        // for something we just wrote would prompt again for no reason.
        self.remember(tracker_id, token.trim());
        self.secrets.set(tracker_id, token)
    }

    /// Store an access token together with the refresh token that renews it.
    ///
    /// Needed because trackers disagree about token lifetime in a way that changes the design.
    /// AniList issues a token good for 364 days and no refresh token — sign in once a year and
    /// forget it. MAL issues one good for about a month *with* a refresh token, so a client that
    /// only stored the access token would silently stop syncing every 31 days and blame the user.
    ///
    /// Stored as one JSON value under the tracker's key rather than two keyring entries: the pair
    /// is meaningless apart, and two entries could get out of step.
    pub fn set_pair(
        &self,
        tracker_id: &str,
        access: &str,
        refresh: Option<&str>,
        expires_at: Option<i64>,
    ) -> Result<Storage, SecretError> {
        let payload = serde_json::json!({
            "v": 1,
            "access": access.trim(),
            "refresh": refresh.map(str::trim),
            "expires_at": expires_at,
        });
        self.set(tracker_id, &payload.to_string())
    }

    /// The stored credential, whichever shape it is in.
    ///
    /// Tolerates a bare token as well as the JSON pair, so a token stored by an earlier build — or
    /// pasted by hand — keeps working rather than being read as a corrupt credential.
    pub fn get_pair(&self, tracker_id: &str) -> Result<TokenPair, SecretError> {
        // `get_raw`, not `get` — `get` returns only the *access* token, so parsing its output as a
        // pair silently produced `refresh: None`. That is the exact failure the pair exists to
        // prevent: MAL's token lapses after 30 days, and without the refresh token sync would have
        // died then with no way back but signing in again. Caught by `--example mal_probe`.
        let raw = self.get_raw(tracker_id)?;
        Ok(TokenPair::parse(&raw))
    }

    /// Fetch a token, reading the keychain at most once per tracker per process.
    ///
    /// Returns the *access* token for a stored pair, so callers that only need to authorise a
    /// request are unaffected by the pair format.
    pub fn get(&self, tracker_id: &str) -> Result<String, SecretError> {
        self.get_raw(tracker_id).map(|raw| TokenPair::parse(&raw).access)
    }

    /// The stored value verbatim.
    fn get_raw(&self, tracker_id: &str) -> Result<String, SecretError> {
        if let Some(cached) = self.cached(tracker_id) {
            return Ok(cached);
        }
        match self.secrets.get(tracker_id)? {
            Some(token) => {
                self.remember(tracker_id, &token);
                Ok(token)
            }
            None => Err(SecretError::Missing(tracker_id.to_owned())),
        }
    }

    fn cached(&self, tracker_id: &str) -> Option<String> {
        // A poisoned lock means another thread panicked mid-read. Falling through to a real read
        // is strictly better than propagating a panic out of a credential lookup.
        self.cache.lock().ok()?.get(tracker_id).cloned()
    }

    fn remember(&self, tracker_id: &str, token: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(tracker_id.to_owned(), token.to_owned());
        }
    }

    fn forget(&self, tracker_id: &str) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.remove(tracker_id);
        }
    }

    /// Whether a token is present, without reading it.
    ///
    /// Never returns an error: "is this account connected?" is a UI question, and a keychain
    /// hiccup should render as "not connected" rather than failing a frame.
    pub fn has(&self, tracker_id: &str) -> bool {
        self.get(tracker_id).is_ok()
    }

    /// Forget a token, from both locations.
    pub fn clear(&self, tracker_id: &str) -> Result<(), SecretError> {
        // Before touching storage: a cached token surviving a sign-out would keep the account
        // apparently connected for the rest of the process.
        self.forget(tracker_id);
        self.secrets.delete(tracker_id)
    }

    /// Where the token *would* be stored, for the Accounts overlay.
    pub fn storage_for(&self, tracker_id: &str) -> Storage {
        self.secrets.storage_for(tracker_id)
    }
}

/// Make a file readable only by its owner.
#[cfg(unix)]
fn restrict(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

/// On Windows the file inherits the user profile's ACL, which is already owner-only.
#[cfg(not(unix))]
fn restrict(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, TokenStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = TokenStore::with_backend(dir.path(), Backend::File);
        (dir, store)
    }

    #[test]
    fn a_token_round_trips() {
        let (_dir, store) = store();
        assert!(!store.has("anilist"));
        store.set("anilist", "tok-123").unwrap();
        assert_eq!(store.get("anilist").unwrap(), "tok-123");
        assert!(store.has("anilist"));
    }

    #[test]
    fn the_file_fallback_is_owner_only() {
        // A token readable by other users on the machine would be worse than useless.
        let (dir, store) = store();
        assert_eq!(store.set("anilist", "tok").unwrap(), Storage::File);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.path().join("anilist.token"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "token file is not 0600");
        }
        let _ = dir;
    }

    #[test]
    fn clearing_removes_the_token() {
        let (_dir, store) = store();
        store.set("anilist", "tok").unwrap();
        store.clear("anilist").unwrap();
        assert!(!store.has("anilist"));
        // Idempotent: logging out twice must not error.
        store.clear("anilist").unwrap();
    }

    #[test]
    fn a_missing_token_is_missing_rather_than_an_error() {
        let (_dir, store) = store();
        assert!(matches!(store.get("nope"), Err(SecretError::Missing(_))));
    }

    #[test]
    fn a_tracker_id_cannot_escape_the_token_directory() {
        // Tracker ids come from config, so `../../.ssh/id_rsa` is reachable input.
        let (dir, store) = store();
        store.set("../../evil", "tok").unwrap();
        let written: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(written, vec!["______evil.token"], "path traversal was not neutralised");
    }

    #[test]
    fn trailing_whitespace_from_a_pasted_token_is_trimmed() {
        // Copy-pasting a token out of a browser reliably brings a newline with it, and a
        // header value with a stray newline is rejected outright.
        let (_dir, store) = store();
        store.set("anilist", "tok-123\n").unwrap();
        assert_eq!(store.get("anilist").unwrap(), "tok-123");
    }

    #[test]
    fn a_stored_pair_reads_back_with_its_refresh_token() {
        // This regressed once and it matters more than it looks: `get_pair` was reading through
        // `get`, which returns only the access token, so the refresh token silently vanished. MAL's
        // access token lapses after 30 days, so the symptom would have been sync dying a month
        // later with no way back but signing in again. Caught by the live probe.
        let (_dir, store) = store();
        store.set_pair("mal", "acc-123", Some("ref-456"), Some(1_800)).unwrap();

        let pair = store.get_pair("mal").unwrap();
        assert_eq!(pair.access, "acc-123");
        assert_eq!(pair.refresh.as_deref(), Some("ref-456"), "the refresh token was lost");
        assert_eq!(pair.expires_at, Some(1_800));

        // And `get` still hands back just the access token, so every existing caller is unaffected
        // by the pair format.
        assert_eq!(store.get("mal").unwrap(), "acc-123");
    }

    #[test]
    fn a_pair_survives_a_fresh_store_reading_the_same_file() {
        // The real path: written by `--login`, read by the next process.
        let dir = tempfile::tempdir().unwrap();
        TokenStore::with_backend(dir.path(), Backend::File)
            .set_pair("mal", "acc", Some("ref"), Some(99))
            .unwrap();

        // A new store, so nothing is served from the in-process cache.
        let reopened = TokenStore::with_backend(dir.path(), Backend::File);
        let pair = reopened.get_pair("mal").unwrap();
        assert_eq!(pair.refresh.as_deref(), Some("ref"));
        assert_eq!(pair.expires_at, Some(99));
    }

    #[test]
    fn a_bare_token_still_reads_as_a_pair_with_no_refresh() {
        // AniList's shape, and what an earlier build stored. Treating it as corrupt would sign
        // someone out for no reason.
        let (_dir, store) = store();
        store.set("anilist", "just-a-token").unwrap();

        let pair = store.get_pair("anilist").unwrap();
        assert_eq!(pair.access, "just-a-token");
        assert_eq!(pair.refresh, None);
        assert_eq!(pair.expires_at, None);
        assert!(!pair.needs_refresh(i64::MAX, 0), "nothing to refresh with");
    }

    #[test]
    fn the_file_backend_never_consults_the_keychain() {
        // The whole reason the backend is selectable: on macOS every keychain read by an unsigned
        // binary can prompt, and a fresh `cargo build` is a new binary every time. A store in file
        // mode must not touch it even to check.
        let dir = tempfile::tempdir().unwrap();
        keychain::set("anilist", "in-the-keychain");
        let secrets = LocalSecrets::new(dir.path(), Backend::File);
        assert_eq!(secrets.backend(), Backend::File);
        assert!(!secrets.uses_keychain());
        assert_eq!(secrets.get("anilist").unwrap(), None, "the keychain's token was read");
        assert_eq!(secrets.set("anilist", "tok").unwrap(), Storage::File);
        secrets.delete("anilist").unwrap();
        assert_eq!(keychain::get("anilist").as_deref(), Some("in-the-keychain"), "deleted");
    }

    #[test]
    fn a_keychain_that_answers_keeps_the_token_and_the_file_copy_goes() {
        // The default backend, as a desktop with a keychain has it. A file left from before must
        // not outlive the token that replaced it.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("anilist.token"), "stale").unwrap();
        let secrets = LocalSecrets::new(dir.path(), Backend::Keychain);
        assert_eq!(secrets.storage_for("anilist"), Storage::File, "the file holds it, for now");

        assert_eq!(secrets.set("anilist", "fresh").unwrap(), Storage::Keychain);
        assert!(!dir.path().join("anilist.token").exists(), "the stale copy outlived it");
        assert_eq!(secrets.storage_for("anilist"), Storage::Keychain);
        assert_eq!(secrets.get("anilist").unwrap().as_deref(), Some("fresh"));

        secrets.delete("anilist").unwrap();
        assert_eq!(keychain::get("anilist"), None);
    }

    #[test]
    fn a_keychain_that_does_not_answer_falls_back_to_the_file() {
        // A headless Linux box with no D-Bus session: still signed in, and told it is a file.
        let dir = tempfile::tempdir().unwrap();
        keychain::refuse();
        let secrets = LocalSecrets::new(dir.path(), Backend::Keychain);
        assert_eq!(secrets.set("anilist", "tok").unwrap(), Storage::File);
        assert_eq!(secrets.get("anilist").unwrap().as_deref(), Some("tok"));
        assert_eq!(secrets.storage_for("anilist"), Storage::File);
    }

    #[test]
    fn a_backend_is_parsed_leniently_but_defaults_to_the_safer_one() {
        assert_eq!(Backend::parse("file"), Backend::File);
        assert_eq!(Backend::parse("FILE"), Backend::File);
        assert_eq!(Backend::parse("  file  "), Backend::File);
        assert_eq!(Backend::parse("keychain"), Backend::Keychain);
        // A typo must not silently downgrade someone's credential storage.
        assert_eq!(Backend::parse("keychian"), Backend::Keychain);
        assert_eq!(Backend::parse(""), Backend::Keychain);
    }

    #[test]
    fn the_environment_overrides_the_configured_backend() {
        // The development-loop escape hatch: one run, no config edit. Through `chosen`, with the
        // variable's value handed in: setting a process-wide env var inside a test would race
        // every other test.
        assert_eq!(Backend::chosen(Some("file"), "keychain"), Backend::File);
        assert_eq!(Backend::chosen(Some("keychain"), "file"), Backend::Keychain);
        // Set to something else, it still wins, and means the safer store.
        assert_eq!(Backend::chosen(Some("vault"), "file"), Backend::Keychain);
        assert_eq!(Backend::chosen(None, "file"), Backend::File);
        assert_eq!(Backend::chosen(None, "keychain"), Backend::Keychain);
        assert_eq!(Backend::ENV, "ANISTREAM_TOKEN_STORAGE");
    }

    #[test]
    fn migrating_with_nothing_stored_reports_missing_rather_than_succeeding() {
        // A migration that silently "succeeded" with no token would leave the user thinking they
        // were signed in.
        let dir = tempfile::tempdir().unwrap();
        let secrets = LocalSecrets::new(dir.path(), Backend::File);
        assert!(matches!(
            secrets.migrate_to_file("nope-no-such"),
            Err(SecretError::Missing(_))
        ));
    }

    #[test]
    fn migrating_when_a_file_already_exists_is_a_no_op_success() {
        // Running it twice must not fail; the second run has nothing to move but the outcome the
        // user asked for is already true.
        let (dir, store) = store();
        store.set("anilist", "tok").unwrap();
        let secrets = LocalSecrets::new(dir.path(), Backend::File);
        assert_eq!(secrets.migrate_to_file("anilist").unwrap(), Storage::File);
        assert_eq!(store.get("anilist").unwrap(), "tok");
    }

    #[test]
    fn migrating_moves_the_keychain_token_into_the_file_and_out_of_the_keychain() {
        // One copy, not two diverging ones, whichever backend the store was set to.
        let dir = tempfile::tempdir().unwrap();
        keychain::set("mal", "from-the-keychain");
        let secrets = LocalSecrets::new(dir.path(), Backend::Keychain);
        assert_eq!(secrets.migrate_to_file("mal").unwrap(), Storage::File);
        assert_eq!(keychain::get("mal"), None);
        let reopened = TokenStore::with_backend(dir.path(), Backend::File);
        assert_eq!(reopened.get("mal").unwrap(), "from-the-keychain");
    }

    /// A store kept in memory, counting what it is asked: what a host's own store looks like
    /// from here.
    #[derive(Default)]
    struct Counted {
        kept: Mutex<HashMap<String, String>>,
        reads: std::sync::atomic::AtomicUsize,
    }

    impl SecretStore for Counted {
        fn get(&self, id: &str) -> Result<Option<String>, SecretError> {
            self.reads.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self.kept.lock().unwrap().get(id).cloned())
        }

        fn set(&self, id: &str, value: &str) -> Result<Storage, SecretError> {
            self.kept.lock().unwrap().insert(id.to_owned(), value.to_owned());
            Ok(Storage::Keychain)
        }

        fn delete(&self, id: &str) -> Result<(), SecretError> {
            self.kept.lock().unwrap().remove(id);
            Ok(())
        }

        fn storage_for(&self, _: &str) -> Storage {
            Storage::Keychain
        }
    }

    #[test]
    fn a_token_store_over_another_store_shapes_its_values_and_reads_each_once() {
        // A phone's keychain behind the host: the pair format and the one read per process are
        // the token store's, whichever store keeps the bytes.
        let secrets = Arc::new(Counted::default());
        secrets.kept.lock().unwrap().insert("kitsu".into(), "bare".into());
        let store = TokenStore::over(secrets.clone());

        assert_eq!(store.get_pair("kitsu").unwrap().access, "bare");
        assert_eq!(store.get("kitsu").unwrap(), "bare");
        assert_eq!(secrets.reads.load(std::sync::atomic::Ordering::Relaxed), 1, "read twice");

        assert_eq!(store.set_pair("mal", "acc", Some("ref"), None).unwrap(), Storage::Keychain);
        let kept = secrets.kept.lock().unwrap().get("mal").cloned().unwrap();
        assert!(kept.contains("\"refresh\":\"ref\""), "{kept}");
        assert_eq!(store.storage_for("mal"), Storage::Keychain);

        store.clear("mal").unwrap();
        assert!(!store.has("mal"), "the cached copy outlived the sign-out");
        assert!(!secrets.kept.lock().unwrap().contains_key("mal"));
    }

    #[test]
    fn the_degraded_storage_is_reported_as_such() {
        // Someone on a shared machine deserves to know the token is on disk rather than in a
        // keychain, so this is surfaced in the Accounts overlay.
        assert!(Storage::File.is_degraded());
        assert!(!Storage::Keychain.is_degraded());
    }
}
