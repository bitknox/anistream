//! A fixture scene never touches the real network, and never shows a real network's name: the
//! connection check answers from the fixtures, and what else could reach out is held to
//! loopback.
#![cfg(feature = "testing")]

use std::{sync::Arc, thread, time::Duration};

use anistream_ffi::{FfiDirs, FfiEngine, FfiHost, FfiScene};
use anistream_model::{command::Command, event::Event, sources::GuardView};

/// How long a test waits for what it is waiting on before failing: everything is on loopback.
const PATIENCE: Duration = Duration::from_secs(30);

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

/// The engine's events until one `until` accepts, polled on a thread of its own.
fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        loop {
            let batch = futures::executor::block_on(engine.next_events());
            let found = batch.iter().any(&until);
            let empty = batch.is_empty();
            read.extend(batch);
            if found || empty {
                let _ = tx.send((read, found));
                break;
            }
        }
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

#[test]
fn the_connection_check_in_device_mode_is_answered_from_the_fixtures() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    // Torrents on, protected by the VPN already running on the device: the setting whose check
    // looked the exit up on the real network.
    std::fs::create_dir_all(&dirs.config).unwrap();
    std::fs::write(
        std::path::Path::new(&dirs.config).join("config.toml"),
        "[providers.torrent]\nenabled = true\n[providers.torrent.vpn]\nmode = \"external\"\n",
    )
    .unwrap();
    // The engine's own clients resolve no name, and the fixture guard makes no request, so a
    // lookup that went out anyway would come back unreachable rather than protected.
    let engine = FfiEngine::start_scene_on(
        dirs,
        Arc::new(Quiet),
        FfiScene::Watching,
        anistream_engine::clock::Clock::default(),
    )
    .expect("the engine starts");

    assert_eq!(engine.send(Command::CheckConnection), None, "a check is not a job");
    let read = read_until(
        &engine,
        |event| matches!(event, Event::Vpn(Some(view)) if *view != GuardView::Checking),
    );
    let verdict = read.into_iter().rev().find_map(|event| match event {
        Event::Vpn(view) => view,
        _ => None,
    });
    let Some(GuardView::Protected { exit }) = verdict else {
        panic!("the fixtures' exit was not what the check found: {verdict:?}");
    };
    assert!(exit.contains("fixture") && exit.contains("Exampleland"), "{exit}");
}
