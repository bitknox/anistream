//! The phones a TV has paired with: their key fingerprints, what to call them, and when they were
//! last seen. Removing one is how a TV revokes a phone.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use serde::{Deserialize, Serialize};

use crate::{Error, identity::Fingerprint};

/// Phones a TV keeps at most. Pairing another is refused until one is removed.
pub const MAX_PAIRED: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PairedDevice {
    pub fingerprint: Fingerprint,
    pub name: String,
    /// Unix seconds.
    pub paired_at: i64,
    #[serde(default)]
    pub last_seen: Option<i64>,
}

/// Where a TV keeps its paired phones.
pub trait TrustStore: Send + Sync + 'static {
    fn get(&self, fingerprint: &Fingerprint) -> Option<PairedDevice>;
    fn list(&self) -> Vec<PairedDevice>;
    /// Keep a newly paired phone, or refresh one paired again. False when the store is full.
    fn add(&self, device: PairedDevice) -> Result<bool, Error>;
    /// Forget a phone. True when it was there.
    fn remove(&self, fingerprint: &Fingerprint) -> Result<bool, Error>;
    fn seen(&self, fingerprint: &Fingerprint, at: i64);
}

/// Kept in memory only: for tests, and a TV that pairs afresh on every launch.
#[derive(Debug, Default)]
pub struct MemoryTrust {
    devices: Mutex<BTreeMap<Fingerprint, PairedDevice>>,
}

impl MemoryTrust {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<Fingerprint, PairedDevice>> {
        self.devices.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl TrustStore for MemoryTrust {
    fn get(&self, fingerprint: &Fingerprint) -> Option<PairedDevice> {
        self.lock().get(fingerprint).cloned()
    }

    fn list(&self) -> Vec<PairedDevice> {
        self.lock().values().cloned().collect()
    }

    fn add(&self, device: PairedDevice) -> Result<bool, Error> {
        let mut devices = self.lock();
        if devices.len() >= MAX_PAIRED && !devices.contains_key(&device.fingerprint) {
            return Ok(false);
        }
        devices.insert(device.fingerprint, device);
        Ok(true)
    }

    fn remove(&self, fingerprint: &Fingerprint) -> Result<bool, Error> {
        Ok(self.lock().remove(fingerprint).is_some())
    }

    fn seen(&self, fingerprint: &Fingerprint, at: i64) {
        if let Some(device) = self.lock().get_mut(fingerprint) {
            device.last_seen = Some(at);
        }
    }
}

/// Kept in a JSON file, `remote-devices.json` in the directory given: the engine's data
/// directory on a TV. Written whole to a temporary file and renamed over the old one, so a
/// crash leaves one or the other. Fingerprints are public keys' hashes, not secrets.
#[derive(Debug)]
pub struct FileTrust {
    path: PathBuf,
    memory: MemoryTrust,
}

impl FileTrust {
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let path = dir.join("remote-devices.json");
        let memory = MemoryTrust::new();
        match std::fs::read(&path) {
            Ok(bytes) => {
                let devices: Vec<PairedDevice> = serde_json::from_slice(&bytes)?;
                for device in devices {
                    memory.add(device)?;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(Self { path, memory })
    }

    fn save(&self) -> Result<(), Error> {
        let body = serde_json::to_vec_pretty(&self.memory.list())?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, body)?;
        std::fs::rename(&temp, &self.path)?;
        Ok(())
    }
}

impl TrustStore for FileTrust {
    fn get(&self, fingerprint: &Fingerprint) -> Option<PairedDevice> {
        self.memory.get(fingerprint)
    }

    fn list(&self) -> Vec<PairedDevice> {
        self.memory.list()
    }

    fn add(&self, device: PairedDevice) -> Result<bool, Error> {
        if !self.memory.add(device)? {
            return Ok(false);
        }
        self.save()?;
        Ok(true)
    }

    fn remove(&self, fingerprint: &Fingerprint) -> Result<bool, Error> {
        let removed = self.memory.remove(fingerprint)?;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    /// In memory only: a write per connection would wear a TV box's flash for a detail.
    /// It is saved with the next pairing or removal.
    fn seen(&self, fingerprint: &Fingerprint, at: i64) {
        self.memory.seen(fingerprint, at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    fn device(name: &str) -> PairedDevice {
        PairedDevice {
            fingerprint: Identity::generate().unwrap().fingerprint(),
            name: name.into(),
            paired_at: 1_000,
            last_seen: None,
        }
    }

    #[test]
    fn a_file_store_keeps_its_phones_across_opens() {
        let dir = tempfile::tempdir().unwrap();
        let phone = device("Hall phone");
        {
            let store = FileTrust::open(dir.path()).unwrap();
            assert!(store.add(phone.clone()).unwrap());
            assert!(store.add(device("Spare tablet")).unwrap());
        }
        let store = FileTrust::open(dir.path()).unwrap();
        assert_eq!(store.list().len(), 2);
        assert!(store.remove(&phone.fingerprint).unwrap());
        assert!(FileTrust::open(dir.path()).unwrap().get(&phone.fingerprint).is_none());
    }

    #[test]
    fn a_full_store_refuses_a_new_phone_but_not_a_known_one() {
        let store = MemoryTrust::new();
        let first = device("first");
        store.add(first.clone()).unwrap();
        for n in 1..MAX_PAIRED {
            assert!(store.add(device(&n.to_string())).unwrap());
        }
        assert!(!store.add(device("one too many")).unwrap());
        assert!(store.add(first).unwrap());
    }
}
