//! Putting a downloaded plugin in the plugin directory, and taking one out.
//!
//! Nothing reaches the directory until it has passed every check. The directory is what the app
//! loads at startup, so a file written there first and verified afterwards would be one crash away
//! from running unverified.

use std::path::{Path, PathBuf};

use crate::{
    client,
    inspect::{self, Inspector},
    local::plugin_file,
    protocol::{IndexEntry, is_valid_id},
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("{0}")]
    Integrity(String),
    #[error("{id} does not load: {message}")]
    Unloadable { id: String, message: String },
    #[error("{0}")]
    Mismatch(String),
    #[error("could not write to the plugin directory: {0}")]
    Io(String),
}

/// Check `bytes` against `entry` and the plugin's own manifest, then write it as `{id}.wasm`.
///
/// Returns the path written. The rename at the end is the only moment the directory changes, so a
/// failure anywhere before it leaves the directory as it was.
pub async fn install(
    bytes: &[u8],
    entry: &IndexEntry,
    inspector: &dyn Inspector,
    plugin_dir: &Path,
) -> Result<PathBuf, InstallError> {
    if !is_valid_id(&entry.id) {
        return Err(InstallError::Integrity(format!("unusable plugin id {:?}", entry.id)));
    }
    // Repeated here even though the client checked: this function is the last gate, and it should
    // not depend on every caller having gone through the client first.
    client::verify_download(bytes, entry)
        .map_err(|e| InstallError::Integrity(e.to_string()))?;

    let info = inspector
        .inspect(bytes)
        .await
        .map_err(|message| InstallError::Unloadable { id: entry.id.clone(), message })?;
    if let Some(problem) = inspect::mismatch(&info, entry) {
        return Err(InstallError::Mismatch(problem));
    }

    let io = |e: std::io::Error| InstallError::Io(e.to_string());
    tokio::fs::create_dir_all(plugin_dir).await.map_err(io)?;
    let target = plugin_file(plugin_dir, &entry.id);
    // `.part`, not `.wasm`: the loader only picks up `*.wasm`, so a half-written file is never
    // mistaken for a plugin.
    let partial = plugin_dir.join(format!(".{}.wasm.part", entry.id));
    tokio::fs::write(&partial, bytes).await.map_err(io)?;
    if let Err(e) = tokio::fs::rename(&partial, &target).await {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(io(e));
    }
    Ok(target)
}

/// Remove an installed plugin's file. Already gone is not an error.
pub async fn uninstall(plugin_dir: &Path, id: &str) -> Result<(), InstallError> {
    if !is_valid_id(id) {
        return Err(InstallError::Integrity(format!("unusable plugin id {id:?}")));
    }
    match tokio::fs::remove_file(plugin_file(plugin_dir, id)).await {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(InstallError::Io(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto,
        inspect::fake::{FakeInspector, plugin},
    };

    fn entry_for(bytes: &[u8], id: &str, version: &str, hosts: &[&str]) -> IndexEntry {
        IndexEntry {
            id: id.into(),
            name: id.into(),
            version: version.into(),
            allowed_hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
            translations: Vec::new(),
            sha256: crypto::sha256_hex(bytes),
            size: bytes.len() as u64,
            published_at: 0,
        }
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    #[tokio::test]
    async fn a_matching_plugin_is_written_under_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = plugin("example", "1.0.0", &["api.example"]);
        let entry = entry_for(&bytes, "example", "1.0.0", &["api.example"]);

        let path = install(&bytes, &entry, &FakeInspector, dir.path()).await.unwrap();
        assert_eq!(path, dir.path().join("example.wasm"));
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(listing(dir.path()), vec!["example.wasm"], "no partial file left behind");
    }

    #[tokio::test]
    async fn a_plugin_asking_for_more_than_listed_never_reaches_the_directory() {
        let dir = tempfile::tempdir().unwrap();
        // The store lists one host; the plugin itself declares two.
        let bytes = plugin("example", "1.0.0", &["api.example", "else.example"]);
        let entry = entry_for(&bytes, "example", "1.0.0", &["api.example"]);

        let result = install(&bytes, &entry, &FakeInspector, dir.path()).await;
        assert!(matches!(result, Err(InstallError::Mismatch(_))));
        assert!(listing(dir.path()).is_empty());
    }

    #[tokio::test]
    async fn bytes_that_do_not_match_the_listing_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = plugin("example", "1.0.0", &[]);
        let entry = entry_for(&bytes, "example", "1.0.0", &[]);
        let tampered = plugin("example", "1.0.1", &[]);

        let result = install(&tampered, &entry, &FakeInspector, dir.path()).await;
        assert!(matches!(result, Err(InstallError::Integrity(_))));
        assert!(listing(dir.path()).is_empty());
    }

    #[tokio::test]
    async fn something_that_does_not_load_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"\0asm garbage".to_vec();
        let entry = entry_for(&bytes, "example", "1.0.0", &[]);
        let result = install(&bytes, &entry, &FakeInspector, dir.path()).await;
        assert!(matches!(result, Err(InstallError::Unloadable { .. })));
    }

    #[tokio::test]
    async fn an_update_replaces_the_file_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let old = plugin("example", "1.0.0", &[]);
        install(&old, &entry_for(&old, "example", "1.0.0", &[]), &FakeInspector, dir.path())
            .await
            .unwrap();
        let new = plugin("example", "1.1.0", &[]);
        install(&new, &entry_for(&new, "example", "1.1.0", &[]), &FakeInspector, dir.path())
            .await
            .unwrap();
        assert_eq!(std::fs::read(dir.path().join("example.wasm")).unwrap(), new);
        assert_eq!(listing(dir.path()), vec!["example.wasm"]);
    }

    #[tokio::test]
    async fn uninstalling_is_idempotent_and_refuses_traversal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("example.wasm"), b"x").unwrap();
        uninstall(dir.path(), "example").await.unwrap();
        uninstall(dir.path(), "example").await.unwrap();
        assert!(listing(dir.path()).is_empty());
        assert!(uninstall(dir.path(), "../example").await.is_err());
    }
}
