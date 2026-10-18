//! MERGE-STUB: replaced by the bundle agent's implementation at merge time.
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Bundle {
    root: PathBuf,
}

impl Bundle {
    pub fn materialize(cache_dir: &Path) -> std::io::Result<Self> {
        Ok(Self { root: cache_dir.join("mpv").join("stub") })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn companion(&self) -> PathBuf {
        self.root.join("scripts/anistream")
    }
    pub fn uosc(&self) -> PathBuf {
        self.root.join("scripts/uosc")
    }
    pub fn thumbfast(&self) -> PathBuf {
        self.root.join("scripts/thumbfast.lua")
    }
    pub fn fonts(&self) -> PathBuf {
        self.root.join("fonts")
    }
    pub fn uosc_opts(&self) -> Vec<(String, String)> {
        Vec::new()
    }
    pub fn thumbfast_opts(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}
