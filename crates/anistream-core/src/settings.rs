//! Writing individual settings back to `config.toml`.
//!
//! Deliberately *not* `toml::to_string(&config)`. Serialising the whole struct would be four
//! lines, and it would silently destroy the file it was asked to update: every comment gone,
//! every key reordered into declaration order, and every value the user left unset written out
//! explicitly — so a future change to a default would no longer reach them. A config file is a
//! document the user owns, not a serialisation of our struct.
//!
//! So a write touches exactly the key it was asked to touch, and `toml_edit` preserves the rest
//! of the document byte for byte. It also gets table placement right, which hand-rolled patching
//! does not: inserting a key just before an existing `[table]` header puts it in the *previous*
//! table, which is a silent corruption rather than a parse error.

use std::path::Path;

use crate::config::Paths;

/// A value being written to the config file.
///
/// A small closed set rather than `toml_edit::Value`, so the crate's public surface does not
/// depend on which TOML library is underneath.
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl SettingValue {
    fn into_toml(self) -> toml_edit::Value {
        match self {
            Self::Str(s) => toml_edit::Value::from(s),
            Self::Int(i) => toml_edit::Value::from(i),
            Self::Float(f) => toml_edit::Value::from(f),
            Self::Bool(b) => toml_edit::Value::from(b),
        }
    }
}

/// Write one key, creating any tables it needs, and leave the rest of the document alone.
///
/// `table` is the dotted path of the containing table — `["providers", "torrent"]` for
/// `[providers.torrent]`. An empty path writes at the document root.
pub fn write_key(
    paths: &Paths,
    table: &[&str],
    key: &str,
    value: SettingValue,
) -> Result<(), crate::Error> {
    let path = &paths.config_file;
    let existing = match std::fs::read_to_string(path) {
        Ok(src) => src,
        // No config file yet is the common case on a fresh install, and changing a setting is
        // a perfectly reasonable way to create one.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => {
            return Err(crate::Error::Config(format!("cannot read {}: {e}", path.display())));
        }
    };

    let mut doc = existing.parse::<toml_edit::DocumentMut>().map_err(|e| {
        crate::Error::Config(format!("{} is not valid TOML: {e}", path.display()))
    })?;

    // Walk down to the containing table, creating implicit tables on the way. `set_implicit`
    // keeps `[providers]` from being written as an empty header when only `[providers.torrent]`
    // has any keys in it.
    let mut node = doc.as_table_mut();
    for segment in table {
        let entry = node
            .entry(segment)
            .or_insert_with(|| toml_edit::Item::Table(toml_edit::Table::new()));
        node = entry.as_table_mut().ok_or_else(|| {
            crate::Error::Config(format!(
                "cannot set {}.{key}: {segment} is not a table in {}",
                table.join("."),
                path.display()
            ))
        })?;
        node.set_implicit(true);
    }
    // The table we actually write into must not be implicit, or its header is omitted.
    if !table.is_empty() {
        node.set_implicit(false);
    }

    // Assign through the existing item where there is one, so a value's own trailing comment
    // and surrounding whitespace survive being changed.
    match node.entry(key) {
        toml_edit::Entry::Occupied(mut slot) => {
            let decor = slot.get().as_value().map(|v| v.decor().clone());
            let mut new_value = value.into_toml();
            if let Some(decor) = decor {
                *new_value.decor_mut() = decor;
            }
            *slot.get_mut() = toml_edit::Item::Value(new_value);
        }
        toml_edit::Entry::Vacant(slot) => {
            slot.insert(toml_edit::Item::Value(value.into_toml()));
        }
    }

    let rendered = doc.to_string();

    // Validate before writing rather than after. A config the app refuses to load is much worse
    // than a setting that did not stick, and this is the last point where it can be caught.
    crate::config::Config::from_toml(&rendered)?;

    write_atomically(path, rendered.as_bytes())
}

/// Write via a temporary file in the same directory, then rename.
///
/// Truncating the real file first would leave the user with an empty config if the process died
/// mid-write — and this file can carry a client secret that is not recoverable from anywhere else.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), crate::Error> {
    let parent = path.parent().ok_or_else(|| {
        crate::Error::Config(format!("{} has no parent directory", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        crate::Error::Config(format!("cannot create {}: {e}", parent.display()))
    })?;

    let temporary = path.with_extension("toml.new");
    std::fs::write(&temporary, bytes).map_err(|e| {
        crate::Error::Config(format!("cannot write {}: {e}", temporary.display()))
    })?;

    // Carry the original's permissions across. The file may hold an OAuth client secret, and a
    // rename would otherwise silently widen it from 0600 to whatever the umask allows.
    #[cfg(unix)]
    if let Ok(meta) = std::fs::metadata(path) {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        let _ = std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(mode));
    }

    std::fs::rename(&temporary, path)
        .map_err(|e| crate::Error::Config(format!("cannot replace {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.to_path_buf(),
            cache_dir: dir.to_path_buf(),
        }
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("anistream-settings-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn comments_and_unrelated_keys_survive_a_write() {
        // The property the whole module exists for. Round-tripping the struct would pass a
        // "value changed" test and still destroy the file.
        let dir = temp_dir("comments");
        let paths = paths_in(&dir);
        std::fs::write(
            &paths.config_file,
            "# my notes\n[playback]\ntranslation = \"sub\"  # I prefer subs\nquality = 1080\n",
        )
        .unwrap();

        write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();

        let after = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(after.contains("# my notes"), "leading comment lost:\n{after}");
        assert!(after.contains("# I prefer subs"), "inline comment lost:\n{after}");
        assert!(after.contains("quality = 720"), "value not written:\n{after}");
        assert!(after.contains("translation = \"sub\""), "sibling key lost:\n{after}");
    }

    #[test]
    fn a_nested_table_is_created_in_the_right_place() {
        // The failure this guards against is subtle: writing a key immediately before an
        // existing `[table]` header silently files it under the *previous* table.
        let dir = temp_dir("nested");
        let paths = paths_in(&dir);
        std::fs::write(&paths.config_file, "[trackers]\nenabled = []\n").unwrap();

        write_key(
            &paths,
            &["providers", "torrent", "vpn"],
            "socks_url",
            SettingValue::Str("socks5://10.64.0.1:1080".into()),
        )
        .unwrap();

        let config = crate::config::Config::load(&paths).unwrap();
        assert_eq!(
            config.providers.torrent.vpn.socks_url.as_deref(),
            Some("socks5://10.64.0.1:1080"),
            "value did not land in the right table"
        );
        assert!(config.trackers.enabled.is_empty(), "the other table was disturbed");
    }

    #[test]
    fn enabling_torrents_without_the_required_setup_is_refused() {
        // The validate-before-write rule earning its place: whatever `Config::validate` requires
        // before torrenting can start now gates the settings screen too, so no UI toggle can write
        // a config the app would refuse to load. Found by this module's own test, not by design.
        //
        // Deliberately not asserting *which* requirement fires. There are several — a VPN mode, a
        // proxy URL, an indexer — and pinning this to one couples a test about the write path to
        // the order `validate` happens to check them in.
        let dir = temp_dir("guard");
        let paths = paths_in(&dir);
        std::fs::write(&paths.config_file, "[providers.torrent]\nenabled = false\n").unwrap();

        let err =
            write_key(&paths, &["providers", "torrent"], "enabled", SettingValue::Bool(true))
                .unwrap_err();
        assert!(
            err.to_string().contains("providers.torrent"),
            "the refusal has to name the setting at fault: {err}"
        );
        assert!(
            !crate::config::Config::load(&paths).unwrap().providers.torrent.enabled,
            "a refused write must change nothing"
        );
    }

    #[test]
    fn a_missing_config_file_is_created() {
        let dir = temp_dir("fresh");
        let paths = paths_in(&dir);
        write_key(&paths, &["playback"], "quality", SettingValue::Int(2160)).unwrap();
        assert_eq!(crate::config::Config::load(&paths).unwrap().playback.quality, 2160);
    }

    #[test]
    fn a_write_that_would_not_load_is_refused_and_changes_nothing() {
        // `commit_threshold` is validated, so this is a real path rather than a contrived one.
        let dir = temp_dir("invalid");
        let paths = paths_in(&dir);
        std::fs::write(&paths.config_file, "[playback]\ncommit_threshold = 0.85\n").unwrap();

        let err =
            write_key(&paths, &["playback"], "commit_threshold", SettingValue::Float(9.0));
        assert!(err.is_err(), "an out-of-range value must be refused");

        let after = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(after.contains("0.85"), "the file was modified anyway:\n{after}");
    }

    #[test]
    fn writing_the_same_key_twice_does_not_duplicate_it() {
        let dir = temp_dir("twice");
        let paths = paths_in(&dir);
        write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();
        write_key(&paths, &["playback"], "quality", SettingValue::Int(480)).unwrap();
        let after = std::fs::read_to_string(&paths.config_file).unwrap();
        assert_eq!(after.matches("quality").count(), 1, "key duplicated:\n{after}");
        assert_eq!(crate::config::Config::load(&paths).unwrap().playback.quality, 480);
    }

    #[test]
    fn an_unparseable_file_is_reported_rather_than_overwritten() {
        let dir = temp_dir("broken");
        let paths = paths_in(&dir);
        std::fs::write(&paths.config_file, "this is not = = toml\n").unwrap();
        let err = write_key(&paths, &["playback"], "quality", SettingValue::Int(720));
        assert!(err.is_err());
        assert_eq!(
            std::fs::read_to_string(&paths.config_file).unwrap(),
            "this is not = = toml\n"
        );
    }
}
