//! Play on TV, end to end over loopback, as the apps hold it: one testing engine runs a TV's
//! remote, another is the phone. The phone's browse finds the TV, pairs with the code the TV is
//! asked to show, plays a fixture title on it and drives it from the card, and Forget unpairs it
//! on both sides. A second phone pairs from the TV's QR link without typing a code.
#![cfg(all(feature = "testing", feature = "remote"))]

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anistream_ffi::{
    FfiCastBrowser, FfiCastSink, FfiDirs, FfiDiscoverySink, FfiEngine, FfiFakeCast, FfiHost,
    FfiLinkPhase, FfiRemoteCard, FfiStart, FfiTvBrowser, FfiTvCommand, FfiTvLink, FfiTvProblem,
    FfiTvRowKind, FfiTvSender,
};
use anistream_model::{event::Event, remote::FrontIntent};
use anistream_testkit::fixtures;

const PATIENCE: Duration = Duration::from_secs(30);

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

struct Started {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    _root: tempfile::TempDir,
}

fn start() -> Started {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine =
        FfiEngine::start_testing(dirs, Arc::new(Keychain::default()), FfiStart::default())
            .expect("the engine starts");
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
    Started { engine, events, _root: root }
}

impl Started {
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

/// The platform's DNS-SD, as an app's browser reports to the sink: here the one TV, found at
/// 127.0.0.1 with its advert's TXT record, and a link-local address the sink drops.
struct Browser {
    instance: String,
    port: u16,
    txt: Vec<anistream_ffi::FfiTxtPair>,
    stopped: Mutex<bool>,
}

impl FfiTvBrowser for Browser {
    fn start(&self, sink: Arc<FfiDiscoverySink>) {
        sink.found(
            self.instance.clone(),
            vec!["fe80::1%en0".into(), "127.0.0.1".into()],
            self.port,
            self.txt.clone(),
        );
    }

    fn stop(&self) {
        *self.stopped.lock().unwrap() = true;
    }
}

fn until<T>(what: &str, mut check: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + PATIENCE;
    loop {
        if let Some(value) = check() {
            return value;
        }
        assert!(Instant::now() < deadline, "{what} in time");
        thread::sleep(Duration::from_millis(20));
    }
}

fn card_until(
    link: &FfiTvLink,
    what: &str,
    check: impl Fn(&FfiRemoteCard) -> bool,
) -> FfiRemoteCard {
    until(what, || Some(link.card()).filter(|card| check(card)))
}

fn block<F: std::future::Future>(future: F) -> F::Output {
    futures::executor::block_on(future)
}

#[test]
fn a_phone_finds_pairs_plays_and_forgets_a_tv() {
    let tv = start();
    let remote =
        block(tv.engine.start_remote("Living room".into(), 0)).expect("the TV listens");
    let advert = remote.advert();

    let phone = start();
    let sender: Arc<FfiTvSender> =
        phone.engine.tv_sender("Mika's phone".into(), "anistream-test/0".into()).unwrap();
    assert!(sender.rows().is_empty(), "nothing before a browse");
    let browser = Arc::new(Browser {
        instance: advert.instance.clone(),
        port: advert.port,
        txt: advert.txt.clone(),
        stopped: Mutex::new(false),
    });
    sender.browse(browser.clone());
    let row = until("the TV is found", || sender.rows().into_iter().next());
    assert_eq!(row.name, "Living room");
    assert_eq!(row.kind, FfiTvRowKind::New);
    assert_eq!(row.detail, "Not paired yet");
    assert_eq!(row.address.as_deref(), Some(format!("127.0.0.1:{}", advert.port).as_str()));

    // The six digits the TV shows for this phone.
    let pairing = block(sender.pair(row.key.clone())).expect("the TV answers");
    assert_eq!(pairing.tv_name(), "Living room");
    assert!(pairing.needs_code());
    block(pairing.request_code()).expect("the TV shows a code");
    let FrontIntent::ShowPairingCode { code, phone: name } = tv.intent() else {
        panic!("no code on the TV")
    };
    assert_eq!(name, "Mika's phone");
    let wrong = if code == "000000" { "111111" } else { "000000" };
    let refused = block(pairing.pair(Some(wrong.into()))).unwrap_err();
    let anistream_ffi::FfiTvError::Failed { problem, reason } = refused;
    assert_eq!(problem, FfiTvProblem::WrongCode);
    assert!(reason.starts_with("That code didn't match"), "{reason}");
    let spaced = format!("{} {}", &code[..3], &code[3..]);
    let paired = block(pairing.pair(Some(spaced))).expect("the right code pairs");
    assert_eq!(paired.name, "Living room");
    assert_eq!(tv.intent(), FrontIntent::HidePairingCode);
    assert_eq!(tv.intent(), FrontIntent::Paired { phone: "Mika's phone".into() });
    assert_eq!(sender.paired().len(), 1);
    let row = until("the row reads paired", || {
        sender.rows().into_iter().find(|row| row.kind == FfiTvRowKind::Paired)
    });
    assert_eq!(row.fingerprint.as_deref(), Some(paired.fingerprint.as_str()));
    sender.stop_browsing();
    assert!(*browser.stopped.lock().unwrap());
    let away = sender.rows();
    assert_eq!(away.len(), 1);
    assert_eq!(away[0].kind, FfiTvRowKind::PairedAway, "still listed at its last address");

    // Play on TV, then the card's controls.
    let link = sender.link(paired.fingerprint.clone()).expect("a link to a paired TV");
    card_until(&link, "connected", |card| card.phase == FfiLinkPhase::Connected);
    let id = fixtures::FINISHED;
    block(link.play(id, fixtures::title(id), "2".into(), Some(300.0)))
        .expect("the TV takes the episode");
    let card = card_until(&link, "the TV stages the episode", |card| card.now.is_some());
    let now = card.now.unwrap();
    assert_eq!(now.title, fixtures::title(id));
    assert_eq!(now.episode, "2");
    assert!(now.episode_line.starts_with("Episode 2"), "{}", now.episode_line);
    assert_eq!(card.tv_name, "Living room");

    block(link.send(FfiTvCommand::Volume { step: 5.0 })).unwrap();
    assert_eq!(tv.intent(), FrontIntent::Volume { step: 5.0 });
    block(link.send(FfiTvCommand::Text { text: "harbour".into(), submit: true })).unwrap();
    assert_eq!(tv.intent(), FrontIntent::Text { text: "harbour".into(), submit: true });
    remote.set_volume(35.0);
    card_until(&link, "the TV's volume", |card| {
        card.now.as_ref().and_then(|now| now.volume) == Some(35.0)
    });
    link.hang_up();
    assert!(block(link.send(FfiTvCommand::TogglePause)).is_err(), "nothing sent once closed");

    // Forget: the TV hears it and lets the phone go.
    assert!(block(sender.forget(paired.fingerprint.clone())));
    assert!(sender.paired().is_empty());
    until("the TV forgets the phone", || remote.paired().is_empty().then_some(()));

    // A second phone, from the QR code on the TV's Pair a phone screen: no code typed.
    let opened = remote.open_pairing().expect("a code");
    let qr = remote.pair_link("127.0.0.1".into(), Some(opened)).unwrap();
    let tablet = start();
    let sender =
        tablet.engine.tv_sender("Ren's tablet".into(), "anistream-test/0".into()).unwrap();
    let pairing = block(sender.pair(qr)).expect("the scanned link connects");
    assert!(!pairing.needs_code(), "the link carried the code");
    block(pairing.pair(None)).expect("pairs with the scanned code");
    assert_eq!(remote.paired().len(), 1);
    assert_eq!(sender.paired()[0].name, "Living room");

    tv.engine.shutdown();
    phone.engine.shutdown();
    tablet.engine.shutdown();
}

#[test]
fn the_fake_tv_plays_to_a_script_for_the_fixtures_apps() {
    let phone = start();
    let fake = phone.engine.start_fake_tv("Living room".into()).unwrap();
    let sender =
        phone.engine.tv_sender("Mika's phone".into(), "anistream-test/0".into()).unwrap();
    let code = fake.open_code();
    let pairing = block(sender.pair(fake.pair_link(Some(code)))).unwrap();
    let paired = block(pairing.pair(None)).unwrap();
    let link = sender.link(paired.fingerprint).unwrap();
    block(link.play(fixtures::FINISHED, String::new(), "3".into(), None)).unwrap();
    let card = card_until(&link, "playing", |card| {
        card.now.as_ref().is_some_and(|now| !now.loading && !now.subtitles.is_empty())
    });
    let now = card.now.unwrap();
    assert_eq!(now.audio.len(), 2);
    assert!(!now.subtitles_off);
    assert_eq!(now.skip.as_ref().map(|s| s.label.as_str()), Some("Skip opening"));
    block(
        link.send(FfiTvCommand::Track {
            kind: anistream_ffi::FfiTvTrackKind::Subtitle,
            id: None,
        }),
    )
    .unwrap();
    card_until(&link, "subtitles off", |card| {
        card.now.as_ref().is_some_and(|n| n.subtitles_off)
    });
    block(link.send(FfiTvCommand::TogglePause)).unwrap();
    card_until(&link, "paused", |card| card.now.as_ref().is_some_and(|n| n.paused));
    block(link.send(FfiTvCommand::Next)).unwrap();
    card_until(&link, "the next episode", |card| {
        card.now.as_ref().is_some_and(|n| n.episode == "4")
    });
    assert!(fake.commands().iter().any(|c| c.starts_with("play")));
    fake.revoke_all();
    card_until(&link, "unpaired", |card| card.phase == FfiLinkPhase::Unpaired);
    phone.engine.shutdown();
}

/// A `_googlecast._tcp` browse, as an app's browser reports to the sink: the fake TVs' adverts.
struct CastBrowse(Vec<Arc<FfiFakeCast>>);

impl FfiCastBrowser for CastBrowse {
    fn start(&self, sink: Arc<FfiCastSink>) {
        for fake in &self.0 {
            let advert = fake.advert();
            sink.found(advert.instance, vec![fake.host()], advert.port, advert.txt);
        }
    }

    fn stop(&self) {}
}

fn fake_cast_options() -> anistream_ffi::FfiFakeCastOptions {
    anistream_ffi::FfiFakeCastOptions {
        fixed_volume: true,
        fetches: true,
        refuses: false,
        launch_ms: 0,
    }
}

#[test]
fn a_fixtures_cast_plays_on_a_fake_tv_and_one_that_refuses_says_why() {
    let phone = start();
    let living =
        phone.engine.start_fake_cast("Living room".into(), fake_cast_options()).unwrap();
    let kitchen = phone
        .engine
        .start_fake_cast(
            "Kitchen display".into(),
            anistream_ffi::FfiFakeCastOptions { refuses: true, ..fake_cast_options() },
        )
        .unwrap();
    let sender =
        phone.engine.tv_sender("Mika's laptop".into(), "anistream-test/0".into()).unwrap();
    sender.browse_cast(Arc::new(CastBrowse(vec![living.clone(), kitchen.clone()])));
    let rows =
        until("both TVs listed", || Some(sender.cast_rows()).filter(|rows| rows.len() == 2));
    let key = |name: &str| rows.iter().find(|r| r.name == name).unwrap().key.clone();

    let cast = sender.cast(key("Living room"), fixtures::FINISHED, "3".into(), None).unwrap();
    let card = until("the cast playing", || {
        Some(cast.card()).filter(|c| c.card.now.as_ref().is_some_and(|n| !n.loading))
    });
    assert_eq!(card.volume_control, anistream_ffi::FfiVolumeControl::MuteOnly);
    assert!(card.notice.is_none());
    cast.command(FfiTvCommand::Mute { muted: true });
    until("muted", || Some(cast.card()).filter(|c| c.muted == Some(true)));
    cast.stop();
    until("stopped", || Some(cast.card()).filter(|c| c.card.over));

    let refused =
        sender.cast(key("Kitchen display"), fixtures::FINISHED, "3".into(), None).unwrap();
    let card = until("the refusal", || Some(refused.card()).filter(|c| c.card.over));
    assert!(matches!(card.notice, Some(anistream_model::Msg::NoticeCastRefused)), "{card:?}");
    living.stop();
    kitchen.stop();
    phone.engine.shutdown();
}
