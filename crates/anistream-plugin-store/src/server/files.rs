//! Plugin files in a directory, beside the SQLite database.

use std::path::{Path, PathBuf};

use anistream_store_core::blobs::{BlobError, Blobs};

pub struct LocalBlobs {
    dir: PathBuf,
}

impl LocalBlobs {
    pub fn new(dir: &Path) -> Self {
        Self { dir: dir.to_owned() }
    }

    /// Where a key lives. Keys are made by the store from validated ids, versions and digests,
    /// but a path is checked here anyway: this is the one place a key becomes a file name.
    fn path(&self, key: &str) -> Result<PathBuf, BlobError> {
        let safe = !key.is_empty()
            && key.split('/').all(|part| {
                !part.is_empty()
                    && part != "."
                    && part != ".."
                    && part.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-+".contains(&b))
            });
        if !safe {
            return Err(BlobError(format!("unusable key {key:?}")));
        }
        Ok(self.dir.join(key))
    }
}

#[async_trait::async_trait]
impl Blobs for LocalBlobs {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), BlobError> {
        let path = self.path(key)?;
        let io = |e: std::io::Error| BlobError(format!("{}: {e}", path.display()));
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(io)?;
        }
        // Written beside, then renamed: a reader never sees half a file.
        let partial = path.with_extension(format!(
            "part-{}",
            anistream_store_core::crypto::new_id().map_err(|e| BlobError(e.to_string()))?
        ));
        tokio::fs::write(&partial, bytes).await.map_err(io)?;
        if let Err(e) = tokio::fs::rename(&partial, &path).await {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(io(e));
        }
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BlobError> {
        let path = self.path(key)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError(format!("{}: {e}", path.display()))),
        }
    }

    async fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, BlobError> {
        use tokio::io::{AsyncReadExt, AsyncSeekExt};
        let path = self.path(key)?;
        let io = |e: std::io::Error| BlobError(format!("{}: {e}", path.display()));
        let mut file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io(e)),
        };
        file.seek(std::io::SeekFrom::Start(start)).await.map_err(io)?;
        let mut bytes = Vec::new();
        file.take(len).read_to_end(&mut bytes).await.map_err(io)?;
        Ok(Some(bytes))
    }

    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        let path = self.path(key)?;
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(BlobError(format!("{}: {e}", path.display()))),
        }
        // Empty directories go with their last file, so a store's directory does not fill with
        // the outlines of plugins it no longer offers.
        let mut dir = path.parent();
        while let Some(parent) = dir
            && parent != self.dir
            && tokio::fs::remove_dir(parent).await.is_ok()
        {
            dir = parent.parent();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_file_round_trips_and_its_directories_go_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let blobs = LocalBlobs::new(dir.path());
        blobs.put("plugins/example/1.0.0/abc.wasm", b"xyz".to_vec()).await.unwrap();
        assert_eq!(
            blobs.get("plugins/example/1.0.0/abc.wasm").await.unwrap(),
            Some(b"xyz".to_vec())
        );
        assert_eq!(
            blobs.get_range("plugins/example/1.0.0/abc.wasm", 1, 5).await.unwrap(),
            Some(b"yz".to_vec())
        );
        assert_eq!(blobs.get_range("plugins/missing", 0, 1).await.unwrap(), None);
        blobs.delete("plugins/example/1.0.0/abc.wasm").await.unwrap();
        assert_eq!(blobs.get("plugins/example/1.0.0/abc.wasm").await.unwrap(), None);
        assert!(!dir.path().join("plugins").exists());
        blobs.delete("plugins/missing").await.unwrap();
    }

    #[tokio::test]
    async fn a_key_cannot_leave_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        let blobs = LocalBlobs::new(dir.path());
        for key in ["../escape", "a/../../b", "/abs", "a//b", "", "a/./b"] {
            assert!(blobs.put(key, Vec::new()).await.is_err(), "{key}");
        }
    }
}
