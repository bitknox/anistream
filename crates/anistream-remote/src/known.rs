//! The TVs a phone has paired with: each TV's key fingerprint, its name and the address it was
//! last reached at. The phone's side of [`crate::trust`], and as public: fingerprints are hashes
//! of public keys, not secrets.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Mutex, PoisonError},
};

use serde::{Deserialize, Serialize};

use crate::{Error, identity::Fingerprint};

/// A TV this phone paired with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownTv {
    pub fingerprint: Fingerprint,
    pub name: String,
    /// Where it was last reached: tried when a browse does not find it, as on a network that
    /// drops multicast.
    pub address: SocketAddr,
    /// Unix seconds.
    pub paired_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
}

/// The phone's paired TVs, in `remote-tvs.json` in the directory given: the engine's data
/// directory. Written whole to a temporary file and renamed over the old one.
#[derive(Debug)]
pub struct KnownTvs {
    path: Option<PathBuf>,
    tvs: Mutex<BTreeMap<Fingerprint, KnownTv>>,
}

impl KnownTvs {
    /// Kept in memory only, for tests.
    pub fn memory() -> Self {
        Self { path: None, tvs: Mutex::new(BTreeMap::new()) }
    }

    pub fn open(dir: &Path) -> Result<Self, Error> {
        let path = dir.join("remote-tvs.json");
        let mut tvs = BTreeMap::new();
        match std::fs::read(&path) {
            Ok(bytes) => {
                let list: Vec<KnownTv> = serde_json::from_slice(&bytes)?;
                tvs.extend(list.into_iter().map(|tv| (tv.fingerprint, tv)));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        Ok(Self { path: Some(path), tvs: Mutex::new(tvs) })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<Fingerprint, KnownTv>> {
        self.tvs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The paired TVs, the most recently paired first.
    pub fn list(&self) -> Vec<KnownTv> {
        let mut tvs: Vec<KnownTv> = self.lock().values().cloned().collect();
        tvs.sort_by(|a, b| b.paired_at.cmp(&a.paired_at).then_with(|| a.name.cmp(&b.name)));
        tvs
    }

    pub fn get(&self, fingerprint: &Fingerprint) -> Option<KnownTv> {
        self.lock().get(fingerprint).cloned()
    }

    /// The TV whose short fingerprint a browse found: see [`crate::discovery::FoundTv::is`].
    pub fn by_short(&self, short: &str) -> Option<KnownTv> {
        self.lock().values().find(|tv| tv.fingerprint.short() == short).cloned()
    }

    /// Keep a TV just paired, or one paired again.
    pub fn add(&self, tv: KnownTv) -> Result<(), Error> {
        self.lock().insert(tv.fingerprint, tv);
        self.save()
    }

    /// The TV was reached at `address`, calling itself `name`. Saved only when either changed,
    /// so a reconnect costs no write.
    pub fn seen(
        &self,
        fingerprint: &Fingerprint,
        name: Option<&str>,
        address: SocketAddr,
        at: i64,
    ) -> Result<(), Error> {
        let changed = {
            let mut tvs = self.lock();
            let Some(tv) = tvs.get_mut(fingerprint) else { return Ok(()) };
            tv.last_seen = Some(at);
            let renamed = name.is_some_and(|name| !name.is_empty() && name != tv.name);
            if let Some(name) = name.filter(|_| renamed) {
                tv.name = name.to_owned();
            }
            let moved = tv.address != address;
            tv.address = address;
            renamed || moved
        };
        if changed { self.save() } else { Ok(()) }
    }

    /// Forget a TV. True when it was there.
    pub fn remove(&self, fingerprint: &Fingerprint) -> Result<bool, Error> {
        let removed = self.lock().remove(fingerprint).is_some();
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    fn save(&self) -> Result<(), Error> {
        let Some(path) = &self.path else { return Ok(()) };
        let body = serde_json::to_vec_pretty(&self.list())?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, body)?;
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    fn tv(name: &str, paired_at: i64) -> KnownTv {
        KnownTv {
            fingerprint: Identity::generate().unwrap().fingerprint(),
            name: name.into(),
            address: "192.168.1.20:47811".parse().unwrap(),
            paired_at,
            last_seen: None,
        }
    }

    #[test]
    fn paired_tvs_are_kept_across_opens_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let den = tv("Den", 1_000);
        {
            let known = KnownTvs::open(dir.path()).unwrap();
            known.add(den.clone()).unwrap();
            known.add(tv("Living room", 2_000)).unwrap();
        }
        let known = KnownTvs::open(dir.path()).unwrap();
        let names: Vec<String> = known.list().into_iter().map(|tv| tv.name).collect();
        assert_eq!(names, ["Living room", "Den"]);
        assert_eq!(
            known.by_short(&den.fingerprint.short()).map(|tv| tv.name),
            Some("Den".into())
        );
        assert!(known.remove(&den.fingerprint).unwrap());
        assert!(KnownTvs::open(dir.path()).unwrap().get(&den.fingerprint).is_none());
    }

    #[test]
    fn a_tv_seen_at_a_new_address_or_name_is_written_down() {
        let dir = tempfile::tempdir().unwrap();
        let known = KnownTvs::open(dir.path()).unwrap();
        let den = tv("Den", 1_000);
        known.add(den.clone()).unwrap();
        let moved: SocketAddr = "192.168.1.31:47811".parse().unwrap();
        known.seen(&den.fingerprint, Some("Den TV"), moved, 5_000).unwrap();
        let read = KnownTvs::open(dir.path()).unwrap().get(&den.fingerprint).unwrap();
        assert_eq!((read.name.as_str(), read.address), ("Den TV", moved));
    }
}
