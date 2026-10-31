//! What this machine remembers about stores: which it has joined, what it last saw in each, and
//! which installed plugins came from where.
//!
//! One JSON file in the data directory. Tokens are not in it — they are credentials, and go to
//! the OS keychain like a tracker's — so the file can be read, backed up or deleted without
//! leaking access to anything.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    crypto::{self, PublicKey},
    protocol::{Index, IndexEntry, Member, Role, host_set},
};

/// The state file's format version.
const VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    #[error("store state {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("store state {path} is damaged: {message}")]
    Damaged { path: PathBuf, message: String },
}

/// One store this machine has joined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Joined {
    /// [`PublicKey::id`] of the store's key.
    pub id: String,
    pub base: String,
    pub name: String,
    pub key: String,
    pub member: Member,
    pub joined_at: i64,
    /// The highest index sequence accepted from this store.
    pub sequence: u64,
    /// The last verified index, so the store's page has something to show while offline.
    pub index: Option<Index>,
    pub checked_at: Option<i64>,
    /// The store refused this device's token. Kept rather than forgotten, so the user sees
    /// that access was removed instead of the store silently disappearing.
    #[serde(default)]
    pub revoked: bool,
    /// Joined with the store's open link, as a member with no name.
    #[serde(default)]
    pub anonymous: bool,
}

impl Joined {
    pub fn public_key(&self) -> Option<PublicKey> {
        PublicKey::parse(&self.key).ok()
    }

    pub fn is_owner(&self) -> bool {
        self.member.role == Role::Owner
    }

    /// Where this device's token lives in the credential store.
    pub fn token_key(&self) -> String {
        token_key(&self.id)
    }
}

/// The credential-store key for a store's token.
pub fn token_key(store_id: &str) -> String {
    format!("plugin-store-{store_id}")
}

/// Where an installed plugin came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Installed {
    pub store: String,
    pub version: String,
    pub sha256: String,
    /// What the user agreed it may contact, so an update that asks for more can say so.
    pub allowed_hosts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalState {
    pub version: u32,
    pub stores: Vec<Joined>,
    /// Keyed by plugin id.
    pub installed: BTreeMap<String, Installed>,
    /// The highest index sequence seen from each store ever joined, kept after leaving one. A
    /// store left and joined again must not be able to hand back a list older than one this
    /// machine has already seen.
    #[serde(default)]
    pub sequences: BTreeMap<String, u64>,
}

impl Default for LocalState {
    fn default() -> Self {
        Self {
            version: VERSION,
            stores: Vec::new(),
            installed: BTreeMap::new(),
            sequences: BTreeMap::new(),
        }
    }
}

impl LocalState {
    /// Read the state file. A missing file is an empty state: nothing joined yet.
    pub fn load(path: &Path) -> Result<Self, LocalError> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| LocalError::Damaged {
                path: path.to_owned(),
                message: e.to_string(),
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(source) => Err(LocalError::Io { path: path.to_owned(), source }),
        }
    }

    /// Write the state file, replacing it in one step.
    ///
    /// Through a temporary file and a rename, so a crash mid-write leaves the old state rather
    /// than half a JSON document that would read as no stores at all.
    pub fn save(&self, path: &Path) -> Result<(), LocalError> {
        let io = |source| LocalError::Io { path: path.to_owned(), source };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| LocalError::Damaged {
            path: path.to_owned(),
            message: e.to_string(),
        })?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, text).map_err(io)?;
        std::fs::rename(&partial, path).map_err(io)
    }

    pub fn store(&self, id: &str) -> Option<&Joined> {
        self.stores.iter().find(|s| s.id == id)
    }

    pub fn store_mut(&mut self, id: &str) -> Option<&mut Joined> {
        self.stores.iter_mut().find(|s| s.id == id)
    }

    /// Add a store, or refresh the record of one joined again.
    pub fn upsert(&mut self, joined: Joined) {
        let remembered = self.sequences.get(&joined.id).copied().unwrap_or(0);
        match self.store_mut(&joined.id) {
            Some(existing) => {
                // Joining again means a fresh token, so whatever was refused before is not now.
                let sequence = existing.sequence.max(joined.sequence).max(remembered);
                *existing = Joined { sequence, ..joined };
            }
            None => {
                let sequence = joined.sequence.max(remembered);
                self.stores.push(Joined { sequence, ..joined });
            }
        }
    }

    /// Forget a store. Plugins installed from it stay installed and simply stop updating.
    pub fn remove_store(&mut self, id: &str) -> Option<Joined> {
        let at = self.stores.iter().position(|s| s.id == id)?;
        let left = self.stores.remove(at);
        let seen = self.sequences.entry(left.id.clone()).or_insert(0);
        *seen = (*seen).max(left.sequence);
        Some(left)
    }

    /// Record a verified index.
    pub fn accept_index(&mut self, id: &str, index: Index, now: i64) {
        let seen = self.sequences.entry(id.to_owned()).or_insert(0);
        *seen = (*seen).max(index.sequence);
        if let Some(store) = self.store_mut(id) {
            store.sequence = store.sequence.max(index.sequence);
            store.name.clone_from(&index.store);
            store.index = Some(index);
            store.checked_at = Some(now);
            store.revoked = false;
        }
    }

    pub fn mark_revoked(&mut self, id: &str) {
        if let Some(store) = self.store_mut(id) {
            store.revoked = true;
        }
    }

    pub fn record_install(&mut self, store: &str, entry: &IndexEntry) {
        self.installed.insert(
            entry.id.clone(),
            Installed {
                store: store.to_owned(),
                version: entry.version.clone(),
                sha256: entry.sha256.clone(),
                allowed_hosts: entry.allowed_hosts.clone(),
            },
        );
    }

    pub fn forget_install(&mut self, id: &str) -> Option<Installed> {
        self.installed.remove(id)
    }

    /// Where a store's plugin stands on this machine.
    ///
    /// `plugin_dir` is consulted for a file of the same name that the store did not put there,
    /// because installing would replace it and the user should know that first.
    pub fn status(&self, store: &str, entry: &IndexEntry, plugin_dir: &Path) -> PluginStatus {
        match self.installed.get(&entry.id) {
            // Recorded, but the file is gone: deleted by hand. Offer it again.
            Some(_) if !plugin_file(plugin_dir, &entry.id).exists() => PluginStatus::Available,
            Some(installed) if installed.store != store => {
                PluginStatus::OtherStore(installed.store.clone())
            }
            Some(installed) if installed.version == entry.version => PluginStatus::Installed,
            Some(installed) => PluginStatus::Update {
                from: installed.version.clone(),
                new_hosts: new_hosts(&installed.allowed_hosts, &entry.allowed_hosts),
            },
            None if plugin_file(plugin_dir, &entry.id).exists() => PluginStatus::AddedByHand,
            None => PluginStatus::Available,
        }
    }

    /// Plugins with a newer version waiting, across every joined store.
    pub fn updates(&self, plugin_dir: &Path) -> Vec<(String, IndexEntry)> {
        self.stores
            .iter()
            .filter(|s| !s.revoked)
            .filter_map(|s| s.index.as_ref().map(|index| (s, index)))
            .flat_map(|(store, index)| {
                index
                    .plugins
                    .iter()
                    .filter(|entry| {
                        matches!(
                            self.status(&store.id, entry, plugin_dir),
                            PluginStatus::Update { .. }
                        )
                    })
                    .map(|entry| (store.id.clone(), entry.clone()))
            })
            .collect()
    }

    /// Whether an installed plugin's file is still the one the store sent.
    pub fn is_unchanged(&self, id: &str, plugin_dir: &Path) -> bool {
        let Some(installed) = self.installed.get(id) else { return false };
        std::fs::read(plugin_file(plugin_dir, id))
            .is_ok_and(|bytes| crypto::sha256_hex(&bytes) == installed.sha256)
    }
}

/// How a store's plugin relates to this machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Available,
    Installed,
    /// A newer version, and any hosts it adds to what was agreed.
    Update {
        from: String,
        new_hosts: Vec<String>,
    },
    /// A file with this id is in the plugin directory but did not come from a store.
    AddedByHand,
    /// Installed from a different store, which keeps updating it.
    OtherStore(String),
}

/// Hosts in `new` that were not in `old`.
pub fn new_hosts(old: &[String], new: &[String]) -> Vec<String> {
    let old = host_set(old);
    host_set(new).into_iter().filter(|h| !old.contains(h)).collect()
}

/// Where a plugin with this id lives.
///
/// Named after the id because that is what unlocks `[providers.plugins.settings.<id>]`: the host
/// only hands a plugin its settings when the file name agrees with the id it declares.
pub fn plugin_file(plugin_dir: &Path, id: &str) -> PathBuf {
    plugin_dir.join(format!("{id}.wasm"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, version: &str, hosts: &[&str]) -> IndexEntry {
        IndexEntry {
            id: id.into(),
            name: id.into(),
            version: version.into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
            sha256: "00".into(),
            size: 1,
            published_at: 0,
        }
    }

    fn joined(id: &str) -> Joined {
        Joined {
            id: id.into(),
            base: "https://plugins.example".into(),
            name: "Test".into(),
            key: String::new(),
            member: Member { id: "m".into(), name: "Alex".into(), role: Role::Member },
            joined_at: 0,
            sequence: 0,
            index: None,
            checked_at: None,
            revoked: false,
            anonymous: false,
        }
    }

    fn touch(dir: &Path, id: &str) {
        std::fs::write(plugin_file(dir, id), b"x").unwrap();
    }

    #[test]
    fn a_missing_file_is_an_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        let state = LocalState::load(&dir.path().join("stores.json")).unwrap();
        assert!(state.stores.is_empty());
    }

    #[test]
    fn state_round_trips_through_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("stores.json");
        let mut state = LocalState::default();
        state.upsert(joined("abc"));
        state.record_install("abc", &entry("example", "1.0.0", &["api.example"]));
        state.save(&path).unwrap();
        assert_eq!(LocalState::load(&path).unwrap(), state);
        assert!(!path.with_extension("json.partial").exists());
    }

    #[test]
    fn a_damaged_file_is_reported_rather_than_read_as_empty() {
        // Reading garbage as "no stores" would look like every store had been left.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stores.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert!(matches!(LocalState::load(&path), Err(LocalError::Damaged { .. })));
    }

    #[test]
    fn statuses_follow_what_is_on_disk_and_recorded() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = LocalState::default();
        let v1 = entry("example", "1.0.0", &["api.example"]);
        let v2 = entry("example", "2.0.0", &["api.example", "cdn.example"]);

        assert_eq!(state.status("abc", &v1, dir.path()), PluginStatus::Available);

        touch(dir.path(), "example");
        assert_eq!(state.status("abc", &v1, dir.path()), PluginStatus::AddedByHand);

        state.record_install("abc", &v1);
        assert_eq!(state.status("abc", &v1, dir.path()), PluginStatus::Installed);
        assert_eq!(
            state.status("abc", &v2, dir.path()),
            PluginStatus::Update {
                from: "1.0.0".into(),
                new_hosts: vec!["cdn.example".into()]
            }
        );
        assert_eq!(
            state.status("other", &v1, dir.path()),
            PluginStatus::OtherStore("abc".into())
        );

        // Deleted by hand: offered again rather than claimed as installed.
        std::fs::remove_file(plugin_file(dir.path(), "example")).unwrap();
        assert_eq!(state.status("abc", &v1, dir.path()), PluginStatus::Available);
    }

    #[test]
    fn updates_are_collected_across_stores_but_not_from_revoked_ones() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = LocalState::default();
        for id in ["a", "b"] {
            state.upsert(joined(id));
        }
        touch(dir.path(), "one");
        touch(dir.path(), "two");
        state.record_install("a", &entry("one", "1.0.0", &[]));
        state.record_install("b", &entry("two", "1.0.0", &[]));
        let index = |store: &str, plugins| Index {
            protocol: 1,
            store: store.into(),
            key: String::new(),
            sequence: 1,
            issued_at: 0,
            plugins,
            owners: Vec::new(),
            open: false,
            contact: None,
        };
        state.accept_index("a", index("a", vec![entry("one", "1.1.0", &[])]), 10);
        state.accept_index("b", index("b", vec![entry("two", "2.0.0", &[])]), 10);

        let found: Vec<String> =
            state.updates(dir.path()).into_iter().map(|(_, e)| e.id).collect();
        assert_eq!(found, vec!["one", "two"]);

        state.mark_revoked("b");
        let found: Vec<String> =
            state.updates(dir.path()).into_iter().map(|(_, e)| e.id).collect();
        assert_eq!(found, vec!["one"]);
    }

    #[test]
    fn joining_again_keeps_the_highest_sequence_seen() {
        // A fresh join must not reset rollback protection to zero.
        let mut state = LocalState::default();
        state.upsert(Joined { sequence: 9, ..joined("abc") });
        state.upsert(Joined { sequence: 0, revoked: false, ..joined("abc") });
        assert_eq!(state.store("abc").unwrap().sequence, 9);
        assert_eq!(state.stores.len(), 1);
    }

    #[test]
    fn leaving_and_joining_again_keeps_rollback_protection() {
        let mut state = LocalState::default();
        state.upsert(joined("abc"));
        let index = Index {
            protocol: 1,
            store: "Test".into(),
            key: String::new(),
            sequence: 12,
            issued_at: 0,
            plugins: Vec::new(),
            owners: Vec::new(),
            open: false,
            contact: None,
        };
        state.accept_index("abc", index, 0);
        state.remove_store("abc");
        state.upsert(joined("abc"));
        assert_eq!(state.store("abc").unwrap().sequence, 12);
    }

    #[test]
    fn new_hosts_ignores_case_and_order() {
        let old = vec!["API.example".to_owned()];
        let new = vec!["cdn.example".to_owned(), "api.example".to_owned()];
        assert_eq!(new_hosts(&old, &new), vec!["cdn.example"]);
        assert!(new_hosts(&new, &old).is_empty());
    }

    #[test]
    fn the_token_key_is_namespaced() {
        // Tracker tokens share the credential store, keyed by tracker id. A store id can never
        // collide with one.
        assert_eq!(token_key("9f3c1a2b"), "plugin-store-9f3c1a2b");
    }
}
