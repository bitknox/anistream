//! Keeping the stream cache to a size.
//!
//! librqbit writes every streamed episode under the session's directory and never removes
//! anything on its own, so without this the directory only grows: a binge leaves gigabytes behind
//! per evening, and in time the disk fills and librqbit fails mid-episode with "no space left".
//! The session deletes a stream's files when it lets the stream go; this is the backstop for what
//! that cannot reach — whatever earlier runs left, and whatever is still on disk when the cap is
//! hit.
//!
//! The unit is a **top-level entry** of the directory: a single-file torrent's file, or a batch's
//! folder. That is the granularity librqbit writes at, and the only one a sweep can remove without
//! leaving half a torrent behind for the next session to re-verify.

use std::{
    ffi::{OsStr, OsString},
    path::{Component, Path},
    time::SystemTime,
};

/// One top-level entry of the cache directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub name: OsString,
    /// Every byte under it.
    pub bytes: u64,
    /// The newest modification time under it: when librqbit last wrote to it, which for a
    /// stream is when it was last watched.
    pub touched: SystemTime,
}

/// What a sweep did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Swept {
    pub removed: usize,
    pub freed: u64,
    /// What the directory holds afterwards, kept entries included.
    pub left: u64,
}

/// The entries to remove so the cache fits in `limit` bytes.
///
/// Least recently written first, which keeps the episodes most likely to be rewatched or resumed.
/// An entry `keep` names is never chosen, but it does count against the limit: a download the
/// user asked for, or a stream that is playing, is still disk the cache is using, and leaving it
/// out of the total would let everything else grow back past the cap around it.
pub fn plan(
    entries: &[CacheEntry],
    limit: u64,
    keep: impl Fn(&OsStr) -> bool,
) -> Vec<OsString> {
    let mut total: u64 = entries.iter().map(|e| e.bytes).sum();
    let mut candidates: Vec<&CacheEntry> = entries.iter().filter(|e| !keep(&e.name)).collect();
    candidates.sort_by_key(|e| e.touched);
    let mut chosen = Vec::new();
    for entry in candidates {
        if total <= limit {
            break;
        }
        total = total.saturating_sub(entry.bytes);
        chosen.push(entry.name.clone());
    }
    chosen
}

/// The top-level entry of `dir` that `path` lies in, if it lies in `dir` at all.
///
/// How a download's file or a live torrent's file is turned into the name a sweep must leave
/// alone.
pub fn entry_of(dir: &Path, path: &Path) -> Option<OsString> {
    match path.strip_prefix(dir).ok()?.components().next()? {
        Component::Normal(name) => Some(name.to_owned()),
        _ => None,
    }
}

/// Read the cache directory. A directory that does not exist yet is an empty cache.
///
/// Symbolic links are skipped rather than followed or removed: librqbit never makes one, so a
/// link here is something the user put there, and following it could reach files that are not
/// the cache's to count or delete.
pub fn scan(dir: &Path) -> std::io::Result<Vec<CacheEntry>> {
    let listing = match std::fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut entries = Vec::new();
    for item in listing.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(item.path()) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        let (bytes, written) = measure(&item.path(), &meta);
        // An empty folder has no file to date it by, so its own time stands in.
        let touched =
            written.unwrap_or_else(|| meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        entries.push(CacheEntry { name: item.file_name(), bytes, touched });
    }
    Ok(entries)
}

/// Size and newest file modification time of a file, or of everything under a folder.
///
/// Files only: a folder's own time changes when a file is created in it, which for a torrent is
/// the moment it was added, not the last time it was watched.
fn measure(path: &Path, meta: &std::fs::Metadata) -> (u64, Option<SystemTime>) {
    if !meta.is_dir() {
        return (meta.len(), meta.modified().ok());
    }
    let mut bytes = 0;
    let mut touched: Option<SystemTime> = None;
    if let Ok(listing) = std::fs::read_dir(path) {
        for item in listing.flatten() {
            let Ok(meta) = std::fs::symlink_metadata(item.path()) else { continue };
            if meta.file_type().is_symlink() {
                continue;
            }
            let (size, at) = measure(&item.path(), &meta);
            bytes += size;
            touched = touched.max(at);
        }
    }
    (bytes, touched)
}

/// Bring the cache directory down to `limit` bytes, leaving alone every entry `keep` names.
///
/// Blocking, and on a large cache it walks thousands of files, so it belongs on a blocking
/// thread. An entry that cannot be removed is reported and skipped rather than ending the sweep:
/// one stuck folder must not leave the rest of the disk full.
pub fn sweep(dir: &Path, limit: u64, keep: impl Fn(&OsStr) -> bool) -> std::io::Result<Swept> {
    let entries = scan(dir)?;
    let mut left: u64 = entries.iter().map(|e| e.bytes).sum();
    let mut swept = Swept::default();
    for name in plan(&entries, limit, keep) {
        let path = dir.join(&name);
        let bytes = entries.iter().find(|e| e.name == name).map_or(0, |e| e.bytes);
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        match removed {
            Ok(()) => {
                swept.removed += 1;
                swept.freed += bytes;
                left = left.saturating_sub(bytes);
            }
            // Gone already: another sweep, or the session forgetting it, got there first.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                left = left.saturating_sub(bytes);
            }
            Err(e) => tracing::warn!(path = %path.display(), error = %e, "could not prune"),
        }
    }
    swept.left = left;
    Ok(swept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn entry(name: &str, bytes: u64, age_days: u64) -> CacheEntry {
        CacheEntry {
            name: name.into(),
            bytes,
            touched: SystemTime::UNIX_EPOCH
                + Duration::from_secs(100_000_000 - age_days * 86_400),
        }
    }

    #[test]
    fn the_oldest_go_first_until_the_cache_fits() {
        let entries = [entry("new", 4, 1), entry("old", 4, 9), entry("middle", 4, 5)];
        assert_eq!(plan(&entries, 8, |_| false), ["old"]);
        assert_eq!(plan(&entries, 4, |_| false), ["old", "middle"]);
        assert_eq!(plan(&entries, 0, |_| false), ["old", "middle", "new"]);
        assert!(plan(&entries, 12, |_| false).is_empty(), "under the limit, nothing goes");
    }

    #[test]
    fn a_kept_entry_is_never_removed_but_still_counts() {
        // A download is the user's, and a playing stream is in use. Both still take disk, so
        // the rest is pruned around them rather than allowed to grow past the cap beside them.
        let entries = [entry("download", 10, 30), entry("old", 4, 9), entry("new", 4, 1)];
        let keep = |name: &OsStr| name == "download";
        assert_eq!(plan(&entries, 12, keep), ["old", "new"]);
        assert_eq!(plan(&entries, 0, keep), ["old", "new"], "kept even when over the limit");
    }

    #[test]
    fn a_path_is_traced_to_its_entry_only_inside_the_cache() {
        let dir = Path::new("/cache/torrents");
        assert_eq!(
            entry_of(dir, Path::new("/cache/torrents/Batch/Episode 01.mkv")).as_deref(),
            Some(OsStr::new("Batch"))
        );
        assert_eq!(
            entry_of(dir, Path::new("/cache/torrents/Episode 02.mkv")).as_deref(),
            Some(OsStr::new("Episode 02.mkv"))
        );
        assert_eq!(entry_of(dir, Path::new("/home/me/Videos/Episode 02.mkv")), None);
        assert_eq!(entry_of(dir, dir), None);
    }

    #[test]
    fn a_sweep_removes_old_entries_and_leaves_kept_ones_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let write = |name: &str, bytes: usize, age_days: u64| {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, vec![0_u8; bytes]).unwrap();
            let at = SystemTime::now() - Duration::from_secs(age_days * 86_400);
            std::fs::File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
        };
        write("Old Batch/Episode 01.mkv", 400, 20);
        write("Old Batch/Episode 02.mkv", 400, 19);
        write("Downloaded Episode.mkv", 500, 40);
        write("Recent Episode.mkv", 300, 1);

        let keep = |name: &OsStr| name == "Downloaded Episode.mkv";
        let swept = sweep(dir.path(), 900, keep).unwrap();

        assert_eq!(swept, Swept { removed: 1, freed: 800, left: 800 });
        assert!(!dir.path().join("Old Batch").exists(), "the whole batch goes at once");
        assert!(dir.path().join("Downloaded Episode.mkv").exists());
        assert!(dir.path().join("Recent Episode.mkv").exists());
    }

    #[test]
    fn a_missing_cache_directory_is_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let swept = sweep(&dir.path().join("torrents"), 0, |_| false).unwrap();
        assert_eq!(swept, Swept::default());
    }
}
