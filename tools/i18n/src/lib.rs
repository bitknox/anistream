//! The translation tool: reads the Fluent catalogue in `i18n/` and writes every platform's strings
//! from it. See docs/translations.md.

pub mod android;
pub mod apple;
pub mod catalogue;
pub mod codelint;
pub mod draft;
pub mod lint;
pub mod msg;
pub mod plural;
pub mod pseudo;
pub mod rust;
pub mod ship;
pub mod util;
pub mod windows;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::catalogue::Catalogue;
use crate::ship::Mode;
use crate::util::Files;

/// Every generated file, by path relative to the repository.
pub fn generate_all(cat: &Catalogue, mode: Mode) -> Result<Files> {
    let mut files = Files::new();
    files.extend(apple::generate(cat, mode)?);
    files.extend(android::generate(cat, mode)?);
    files.extend(windows::generate(cat, mode)?);
    files.extend(rust::generate(cat, mode)?);
    files.extend(msg::generate(cat)?);
    Ok(files)
}

/// Generated files already on disk that no longer belong to the catalogue (a removed language's
/// files), found by where the generators put things.
pub fn owned_on_disk(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut visit = |dir: &str, keep: &dyn Fn(&Path) -> bool| {
        fn walk(dir: &Path, keep: &dyn Fn(&Path) -> bool, out: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, keep, out);
                } else if keep(&p) {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(&root.join(dir), keep, &mut out);
        found.extend(out.into_iter().filter_map(|p| {
            p.strip_prefix(root).ok().map(|r| r.to_string_lossy().replace('\\', "/"))
        }));
    };
    visit("android/i18n/src/main/res", &|p| {
        p.file_name().is_some_and(|n| n == "strings.xml" || n == "locales_config.xml")
    });
    visit("desktop/windows/Anistream.Windows/Strings", &|p| {
        p.file_name().is_some_and(|n| n == "Resources.resw")
    });
    visit("crates/anistream-i18n/ftl", &|p| p.extension().is_some_and(|e| e == "ftl"));
    found
}

/// What differs between the catalogue's output and the files on disk: a line per problem.
pub fn stale(root: &Path, expected: &Files) -> Vec<String> {
    let mut out = Vec::new();
    for (path, want) in expected {
        match fs::read_to_string(root.join(path)) {
            Err(_) => out.push(format!("{path}: missing")),
            // A Windows editor may save CRLF; that is not a stale file.
            Ok(have) if have.replace("\r\n", "\n") != *want => {
                let have = have.replace("\r\n", "\n");
                let line =
                    have.lines().zip(want.lines()).position(|(a, b)| a != b).map_or_else(
                        || have.lines().count().min(want.lines().count()) + 1,
                        |i| i + 1,
                    );
                out.push(format!("{path}: differs from the catalogue (first at line {line})"));
            }
            Ok(_) => {}
        }
    }
    for path in owned_on_disk(root) {
        if !expected.contains_key(&path) {
            out.push(format!("{path}: no longer generated; delete it"));
        }
    }
    out
}

pub fn write(root: &Path, files: &Files) -> Result<usize> {
    let mut changed = 0;
    for path in owned_on_disk(root) {
        if !files.contains_key(&path) {
            fs::remove_file(root.join(&path))?;
            changed += 1;
        }
    }
    for (path, text) in files {
        let full = root.join(path);
        if fs::read_to_string(&full).ok().as_ref() == Some(text) {
            continue;
        }
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, text)?;
        changed += 1;
    }
    Ok(changed)
}

/// The repository root: the nearest folder above `start` that holds `i18n/languages.toml`.
pub fn find_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|d| d.join("i18n/languages.toml").is_file()).map(Path::to_path_buf)
}
