//! The app's own start, as far as a test can take it without the network: up to the files it
//! opens, which it refuses to go past when they are wrong.

use std::sync::Arc;

use anistream_ffi::{FfiBackground, FfiDirs, FfiEngine, FfiError, FfiHost, FfiStart};

struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

fn dirs(root: &std::path::Path) -> FfiDirs {
    let dir = |name: &str| root.join(name).display().to_string();
    FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") }
}

#[test]
fn a_config_that_does_not_load_is_refused_with_the_file_and_the_line() {
    let root = tempfile::tempdir().expect("a temporary directory");
    std::fs::create_dir_all(root.path().join("config")).unwrap();
    std::fs::write(root.path().join("config/config.toml"), "[playback]\nbogus = 1\n").unwrap();

    let Err(FfiError::Config { reason }) = FfiEngine::start(
        dirs(root.path()),
        Arc::new(Quiet),
        FfiBackground::default(),
        FfiStart::default(),
    ) else {
        panic!("a config that does not load is refused");
    };
    assert!(reason.contains("config.toml, line 2:"), "{reason}");
    assert!(reason.contains("bogus"), "{reason}");
}

#[test]
fn a_directory_that_cannot_be_made_is_refused() {
    let root = tempfile::tempdir().expect("a temporary directory");
    // A file where the data directory should go.
    std::fs::write(root.path().join("data"), "").unwrap();

    let Err(FfiError::Dirs { reason }) = FfiEngine::start(
        dirs(root.path()),
        Arc::new(Quiet),
        FfiBackground::default(),
        FfiStart::default(),
    ) else {
        panic!("a data directory that cannot be made is refused");
    };
    assert!(reason.contains("data"), "{reason}");
}
