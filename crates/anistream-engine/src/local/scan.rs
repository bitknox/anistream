//! Reading a folder: which video files are in it, and which series they belong to.
//!
//! Folder first: a file inside a folder belongs to the series the folder is named for, and a
//! file loose in the root is named by its own file name. Nothing is opened, moved or renamed.

use std::path::{Path, PathBuf};

use anistream_providers::torrent::release;

/// The most levels below a folder that are read.
const DEPTH: usize = 6;

const VIDEO: &[&str] = &["mkv", "mp4", "m4v", "avi", "webm", "mov", "ts", "wmv"];

/// A video file and what its name says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: PathBuf,
    pub episode: Option<u32>,
}

/// The files of one series found in a folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Series {
    pub name: String,
    pub files: Vec<File>,
}

/// Read `root`. `Err` when it cannot be read at all, as a drive that is not plugged in.
pub fn scan(root: &Path) -> std::io::Result<Vec<Series>> {
    std::fs::read_dir(root)?;
    let mut found: Vec<(String, File)> = Vec::new();
    walk(root, root, 0, &mut found);
    let mut series: Vec<Series> = Vec::new();
    for (name, file) in found {
        match series.iter_mut().find(|s| s.name.eq_ignore_ascii_case(&name)) {
            Some(existing) => existing.files.push(file),
            None => series.push(Series { name, files: vec![file] }),
        }
    }
    for s in &mut series {
        s.files.sort_by(|a, b| (a.episode, &a.path).cmp(&(b.episode, &b.path)));
    }
    series.sort_by_key(|s| s.name.to_lowercase());
    Ok(series)
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<(String, File)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < DEPTH {
                walk(root, &path, depth + 1, out);
            }
        } else if is_video(&path) {
            let stem =
                path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let parsed = release::parse(&stem);
            let series = series_folder(root, &path)
                .map(|folder| clean_folder(&folder))
                .filter(|folder| !folder.is_empty())
                .unwrap_or_else(|| parsed.title.clone());
            out.push((series, File { path, episode: parsed.episode }));
        }
    }
}

/// The first folder below `root` that holds `file`, when it is not loose in the root.
fn series_folder(root: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(root).ok()?;
    let mut parts = rel.components();
    let first = parts.next()?;
    parts.next()?;
    Some(first.as_os_str().to_string_lossy().into_owned())
}

/// A folder's name without release tags: `[Group] Show Name (2019) [1080p]` is `Show Name`.
pub fn clean_folder(name: &str) -> String {
    let mut out = String::new();
    let mut depth = 0u32;
    for c in name.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(if c == '_' || c == '.' { ' ' } else { c }),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_video(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| VIDEO.iter().any(|v| v.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, rel: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    #[test]
    fn a_folder_names_its_series_and_loose_files_name_themselves() {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "Lantern Harbour [Release] (1080p)/Lantern Harbour - 02.mkv");
        touch(dir.path(), "Lantern Harbour [Release] (1080p)/Lantern Harbour - 01.mkv");
        touch(dir.path(), "Quiet Orchard Days - 03.mp4");
        touch(dir.path(), "Quiet Orchard Days - 04.mp4");
        touch(dir.path(), "notes.txt");
        touch(dir.path(), ".hidden/Ghost - 01.mkv");
        let series = scan(dir.path()).unwrap();
        let names: Vec<_> = series.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Lantern Harbour", "Quiet Orchard Days"]);
        let episodes: Vec<_> = series[0].files.iter().map(|f| f.episode).collect();
        assert_eq!(episodes, [Some(1), Some(2)], "in episode order");
        assert_eq!(series[1].files.len(), 2);
    }

    #[test]
    fn a_folder_that_is_not_there_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        assert!(scan(&dir.path().join("unplugged")).is_err());
    }

    #[test]
    fn folder_names_lose_their_tags() {
        assert_eq!(clean_folder("[Group] Show_Name (2019) [1080p]"), "Show Name");
        assert_eq!(clean_folder("Plain Name"), "Plain Name");
    }
}
