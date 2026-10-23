//! A TV's remote as the app holds it: a phone on loopback pairs with the code the TV is asked to
//! show, and what it asks of the front end comes out of `next_events` among the engine's events.
//! The TV's key is kept in the host's keychain, here a map, so a second start is the same TV.
#![cfg(all(feature = "testing", feature = "remote"))]

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anistream_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use anistream_model::{
    event::Event,
    remote::{FrontIntent, RemoteKey},
};
use anistream_remote::{
    client::{Hello, RemoteClient},
    command::{Key, RemoteCommand},
    identity::Identity,
};

const PATIENCE: Duration = Duration::from_secs(30);

/// A host whose keychain is a map.
#[derive(Default)]
struct Keychain(Mutex<HashMap<String, String>>);

impl FfiHost for Keychain {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, id: String) -> Option<String> {
        self.0.lock().unwrap().get(&id).cloned()
    }

    fn secret_set(&self, id: String, value: String) -> bool {
        self.0.lock().unwrap().insert(id, value);
        true
    }

    fn secret_delete(&self, id: String) -> bool {
        self.0.lock().unwrap().remove(&id);
        true
    }
}

struct Tv {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    _root: tempfile::TempDir,
}

impl Tv {
    fn start(host: Arc<Keychain>) -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let engine =
            FfiEngine::start_testing(dirs, host, FfiStart::default()).expect("the engine starts");
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self { engine, events, _root: root }
    }

    /// The next intent the remote sent the front end, past the engine's own events.
    fn intent(&self) -> FrontIntent {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.events.recv_timeout(left) {
                Ok(Event::Remote(intent)) => return intent,
                Ok(_) => {}
                Err(_) => panic!("no intent from the remote in time"),
            }
        }
    }
}

#[test]
fn a_phone_pairs_and_its_keys_and_volume_reach_the_front_end() {
    let host = Arc::new(Keychain::default());
    let tv = Tv::start(Arc::clone(&host));
    let remote = futures::executor::block_on(tv.engine.start_remote("Den".into(), 0))
        .expect("the remote listens");
    assert!(remote.is_running());
    assert_eq!(remote.advert().instance, "Den");
    let link = remote.pair_link("192.168.1.20".into(), Some("042917".into())).unwrap();
    assert!(link.starts_with("anistream-remote://pair?addr=192.168.1.20:"), "{link}");
    assert!(link.ends_with("&code=042917"), "{link}");

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let addr = SocketAddr::from(([127, 0, 0, 1], remote.port()));
    let phone = Identity::generate().unwrap();
    let hello = Hello { name: "Mika's phone".into(), app: "anistream-test/0".into() };
    let mut client = runtime.block_on(async {
        let mut client = RemoteClient::connect(addr, &phone, None, &hello).await.unwrap();
        client.request_code().await.unwrap();
        client
    });
    let FrontIntent::ShowPairingCode { code, phone: name } = tv.intent() else {
        panic!("no pairing code shown")
    };
    assert_eq!(name, "Mika's phone");
    runtime.block_on(client.pair(&code)).expect("the code pairs");
    assert_eq!(tv.intent(), FrontIntent::HidePairingCode);
    assert_eq!(remote.paired().len(), 1);

    runtime.block_on(client.send(RemoteCommand::Navigate { key: Key::Down })).unwrap();
    assert_eq!(tv.intent(), FrontIntent::Navigate { key: RemoteKey::Down });
    // The TV's own volume, with nothing playing.
    runtime.block_on(client.send(RemoteCommand::Volume { step: 5.0 })).unwrap();
    assert_eq!(tv.intent(), FrontIntent::Volume { step: 5.0 });

    let short = remote.short_fingerprint();
    futures::executor::block_on(remote.stop());
    assert!(!remote.is_running());
    // The key was kept: started again, it is the same TV.
    let again = futures::executor::block_on(tv.engine.start_remote("Den".into(), 0)).unwrap();
    assert_eq!(again.short_fingerprint(), short);
    assert_eq!(again.paired().len(), 1, "the paired phone is remembered");
    let phone = again.paired().remove(0).fingerprint;
    assert_eq!(again.revoke(phone), Ok(true));
    assert!(again.paired().is_empty());
    tv.engine.shutdown();
}
