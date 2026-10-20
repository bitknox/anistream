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
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingValue {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    /// Written as a TOML array of strings: `providers.order`, reordered from the Providers screen.
    ///
    /// Not `List`: the Kotlin bindings nest each variant as a class inside `SettingValue`, where
    /// `List` would name the variant itself rather than Kotlin's list of the strings it carries.
    Strings(Vec<String>),
}

impl SettingValue {
    fn into_toml(self) -> toml_edit::Value {
        match self {
            Self::Str(s) => toml_edit::Value::from(s),
            Self::Int(i) => toml_edit::Value::from(i),
            Self::Float(f) => toml_edit::Value::from(f),
            Self::Bool(b) => toml_edit::Value::from(b),
            Self::Strings(items) => toml_edit::Value::Array(items.into_iter().collect()),
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

    // A file that does not load is left exactly as it is. The app is running on defaults
    // because of it, and a write on top would either fail for the same reason after the fact or,
    // worse, succeed — and leave the user to find their file changed while it was broken. Said
    // in their terms: which file, which line, and that nothing was touched.
    if let Err(problem) = crate::config::Config::parse(&existing) {
        let name = path
            .file_name()
            .map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        return Err(crate::Error::Config(format!(
            "{name} is left as it is until it loads — {problem}"
        )));
    }

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
///
/// The temporary file is private from the moment it exists, rather than created with whatever
/// the umask allows and narrowed afterwards — which left the secret world-readable in between.
/// And it is named for this write alone: with one fixed name, two instances saving at once wrote
/// into the same file, and whichever renamed second could install the two interleaved.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), crate::Error> {
    use std::io::Write;

    let path = &linked_file(path);
    let parent = path.parent().ok_or_else(|| {
        crate::Error::Config(format!("{} has no parent directory", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        crate::Error::Config(format!("cannot create {}: {e}", parent.display()))
    })?;

    // The original's permissions, so a rename never widens them; a new file starts private,
    // since it may be given a client secret at the next sign-in.
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).map_or(0o600, |meta| meta.permissions().mode() & 0o7777)
    };

    let (temporary, mut file) = create_temporary(path, |options| {
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        let _ = options;
    })
    .map_err(|e| {
        crate::Error::Config(format!("cannot write next to {}: {e}", path.display()))
    })?;

    let written = (|| {
        file.write_all(bytes)?;
        // The umask may have taken bits the original had; put back exactly what it was.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        }
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(crate::Error::Config(format!("cannot replace {}: {e}", path.display())));
    }
    Ok(())
}

/// The file a link at `path` leads to, or `path` itself when it is no link.
///
/// A config kept in a dotfiles repository and linked into place is common, and renaming over the
/// link replaced it with a regular file: the setting stuck, and the repository silently stopped
/// being the config. The write goes to the target instead, which may not exist yet. Followed by
/// hand rather than canonicalised, since a link to a file not created yet has no canonical form,
/// and bounded, since a loop of links leads nowhere.
fn linked_file(path: &Path) -> std::path::PathBuf {
    let mut at = path.to_path_buf();
    for _ in 0..40 {
        let Ok(target) = std::fs::read_link(&at) else { break };
        // A relative target is relative to the link's own directory; `join` keeps an absolute
        // one as it is.
        at = match at.parent() {
            Some(parent) => parent.join(target),
            None => target,
        };
    }
    at
}

/// Create a temporary file beside `path` under a name no other write is using.
///
/// `create_new`, so a name that exists — another instance's write in flight — is never opened
/// and truncated; the next name is tried instead.
fn create_temporary(
    path: &Path,
    configure: impl Fn(&mut std::fs::OpenOptions),
) -> std::io::Result<(std::path::PathBuf, std::fs::File)> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static WRITES: AtomicU32 = AtomicU32::new(0);

    let name = path.file_name().map_or_else(|| "config.toml".into(), |n| n.to_string_lossy());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos());
    let mut last = None;
    for _ in 0..16 {
        let write = WRITES.fetch_add(1, Ordering::Relaxed);
        let candidate = path
            .with_file_name(format!(".{name}.{}-{write}-{nanos:09}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        configure(&mut options);
        match options.open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| std::io::Error::other("no free temporary name")))
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
    fn a_list_is_written_as_an_array_and_the_rest_of_the_file_survives() {
        let dir = temp_dir("list");
        let paths = paths_in(&dir);
        std::fs::write(
            &paths.config_file,
            "# my notes\n[providers]\norder = [\"torrent\", \"plugins\"]  # tried first\n\
             disabled = [\"remote\"]\n\n[playback]\nquality = 1080\n",
        )
        .unwrap();

        let order = ["alpha", "plugins", "torrent"].map(String::from).to_vec();
        write_key(&paths, &["providers"], "order", SettingValue::Strings(order.clone()))
            .unwrap();

        let config = crate::config::Config::load(&paths).unwrap();
        assert_eq!(config.providers.order, order);
        assert_eq!(config.providers.disabled, ["remote"], "the sibling key was disturbed");
        assert_eq!(config.playback.quality, 1080, "the other table was disturbed");
        let after = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(after.contains("# my notes"), "leading comment lost:\n{after}");
        assert!(after.contains("# tried first"), "inline comment lost:\n{after}");
        assert!(after.contains(r#"order = ["alpha", "plugins", "torrent"]"#), "{after}");
        assert_eq!(after.matches("order").count(), 1, "key duplicated:\n{after}");
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

    #[cfg(unix)]
    #[test]
    fn a_linked_config_stays_a_link_and_its_target_is_written() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("linked");
        let paths = paths_in(&dir);
        let dotfiles = dir.join("dotfiles");
        std::fs::create_dir_all(&dotfiles).unwrap();
        let target = dotfiles.join("anistream.toml");
        std::fs::write(&target, "[playback]\nquality = 1080\n").unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640)).unwrap();
        // Relative, as `ln -s` is usually given.
        std::os::unix::fs::symlink("dotfiles/anistream.toml", &paths.config_file).unwrap();

        write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();

        let link = std::fs::symlink_metadata(&paths.config_file).unwrap();
        assert!(link.file_type().is_symlink(), "the link was replaced by a file");
        assert!(std::fs::read_to_string(&target).unwrap().contains("quality = 720"));
        let mode = std::fs::metadata(&target).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o640, "the target's mode was not kept");
        let strays: Vec<_> = std::fs::read_dir(&dotfiles).unwrap().flatten().collect();
        assert_eq!(strays.len(), 1, "a temporary file was left behind: {strays:?}");
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

    #[cfg(unix)]
    #[test]
    fn a_private_file_stays_private_and_a_new_one_starts_private() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("modes");
        let paths = paths_in(&dir);
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;

        write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();
        assert_eq!(mode(&paths.config_file), 0o600, "a new config may soon hold a secret");

        std::fs::set_permissions(&paths.config_file, std::fs::Permissions::from_mode(0o640))
            .unwrap();
        write_key(&paths, &["playback"], "quality", SettingValue::Int(480)).unwrap();
        assert_eq!(mode(&paths.config_file), 0o640, "the user's own choice is kept");
    }

    #[cfg(unix)]
    #[test]
    fn the_temporary_file_is_never_readable_by_others() {
        // The window this closes: the secret sat in a file created with the umask's 0644 until
        // a chmod after the write.
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("temporary");
        let file = dir.join("config.toml");
        let (temporary, _handle) = create_temporary(&file, |options| {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        })
        .unwrap();
        let mode = std::fs::metadata(&temporary).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode & 0o077, 0, "group or others could read it: {mode:o}");
        // And it is this write's own: a second one does not land on it.
        let (other, _) = create_temporary(&file, |_| {}).unwrap();
        assert_ne!(temporary, other);
    }

    #[test]
    fn writes_racing_each_other_never_leave_a_broken_file() {
        // Two instances saving at once used to write into one shared temporary file.
        let dir = temp_dir("racing");
        let paths = paths_in(&dir);
        write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap();
        let writers: Vec<_> = (0..8)
            .map(|n| {
                let paths = paths.clone();
                std::thread::spawn(move || {
                    for i in 0..20 {
                        let value = if (n + i) % 2 == 0 { "sub" } else { "dub" };
                        let _ = write_key(
                            &paths,
                            &["playback"],
                            "translation",
                            SettingValue::Str(value.into()),
                        );
                    }
                })
            })
            .collect();
        for writer in writers {
            writer.join().unwrap();
        }
        let config = crate::config::Config::load(&paths).expect("the file still loads");
        assert_eq!(config.playback.quality, 720);
        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temporary files left behind: {leftovers:?}");
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

    #[test]
    fn a_file_that_does_not_load_is_never_written_over_even_when_the_edit_would_fix_it() {
        // The app runs on defaults while the file is broken, so a setting changed then is a
        // change to the defaults on screen — not an instruction to rewrite the user's file.
        let dir = temp_dir("unloadable");
        let paths = paths_in(&dir);
        let broken = "[playback]\nquality = 1080\nqualty = 720\n";
        std::fs::write(&paths.config_file, broken).unwrap();

        let err =
            write_key(&paths, &["playback"], "quality", SettingValue::Int(720)).unwrap_err();

        let said = err.to_string();
        assert!(said.contains("config.toml") && said.contains("line 3"), "{said}");
        assert!(said.contains("left as it is"), "{said}");
        assert_eq!(std::fs::read_to_string(&paths.config_file).unwrap(), broken);
    }
}
