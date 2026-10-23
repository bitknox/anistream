//! A TV and phones on the loopback interface, end to end: discovery through the in-memory
//! network, pairing, refusal of a phone the TV does not know, commands reaching a fake engine,
//! state reaching the phones, revocation and reconnects. Nothing here touches HOME or the
//! config directory: trust lives in memory, or in a temporary directory.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use anistream_core::ids::AnilistId;
use anistream_model::{
    command::{Command, PlayerCommand},
    event::Event,
};
use anistream_remote::{
    Error,
    client::{Hello, Link, LinkOptions, LinkStatus, RemoteClient},
    command::{FrontIntent, Key, RemoteCommand, RemoteKey},
    discovery::{Advertiser, Browser, Discovery, DiscoverySink, MemoryNetwork},
    identity::Identity,
    server::{CommandSink, RemoteServer, ServerOptions},
    state::{RemoteState, Screen},
    trust::{FileTrust, MemoryTrust, TrustStore},
    wire::{PairFailure, Rejection},
};
use tokio::{
    sync::{mpsc, watch},
    time::timeout,
};

const WAIT: Duration = Duration::from_secs(5);

/// The engine and the TV front end, as the server sees them: what reached each.
struct FakeTv {
    engine: mpsc::UnboundedSender<Command>,
    front: mpsc::UnboundedSender<FrontIntent>,
}

impl CommandSink for FakeTv {
    fn engine(&self, command: Command) {
        let _ = self.engine.send(command);
    }

    fn front(&self, intent: FrontIntent) {
        let _ = self.front.send(intent);
    }
}

struct Tv {
    server: RemoteServer,
    identity: Identity,
    trust: Arc<dyn TrustStore>,
    engine: mpsc::UnboundedReceiver<Command>,
    front: mpsc::UnboundedReceiver<FrontIntent>,
}

impl Tv {
    async fn start(addr: SocketAddr, identity: Identity, trust: Arc<dyn TrustStore>) -> Self {
        let (engine_tx, engine) = mpsc::unbounded_channel();
        let (front_tx, front) = mpsc::unbounded_channel();
        let sink = Arc::new(FakeTv { engine: engine_tx, front: front_tx });
        let options =
            ServerOptions { tv_name: "Living room".into(), ..ServerOptions::default() };
        let server =
            RemoteServer::bind(addr, identity.clone(), Arc::clone(&trust), sink, options)
                .await
                .unwrap();
        Self { server, identity, trust, engine, front }
    }

    async fn new() -> Self {
        Self::start(local(0), Identity::generate().unwrap(), Arc::new(MemoryTrust::new())).await
    }

    fn addr(&self) -> SocketAddr {
        self.server.local_addr()
    }

    /// The code the TV front end was asked to show.
    async fn shown_code(&mut self) -> String {
        loop {
            match timeout(WAIT, self.front.recv()).await.unwrap().unwrap() {
                FrontIntent::ShowPairingCode { code, .. } => return code,
                _ => continue,
            }
        }
    }

    fn playing(&self) {
        self.server.observe(&Event::PlaybackStaged {
            id: AnilistId::new(4242),
            title: "Harbour Lights".into(),
            episode: "3".into(),
            episode_title: Some("Low Tide".into()),
            speed: 1.0,
        });
    }
}

fn local(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn hello(name: &str) -> Hello {
    Hello { name: name.into(), app: "anistream-test/0".into() }
}

/// A phone paired with `tv`: its identity and the TV key it pinned.
async fn paired_phone(tv: &mut Tv, name: &str) -> (Identity, RemoteClient) {
    let phone = Identity::generate().unwrap();
    let mut client =
        RemoteClient::connect(tv.addr(), &phone, None, &hello(name)).await.unwrap();
    client.request_code().await.unwrap();
    let code = tv.shown_code().await;
    let pinned = client.pair(&code).await.unwrap();
    assert_eq!(pinned, tv.identity.fingerprint());
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::HidePairingCode)
    );
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::Paired { phone: name.into() })
    );
    (phone, client)
}

async fn next_state(
    state: &mut watch::Receiver<Option<RemoteState>>,
    until: impl Fn(&RemoteState) -> bool,
) -> RemoteState {
    timeout(WAIT, async {
        loop {
            if let Some(current) = state.borrow_and_update().clone()
                && until(&current)
            {
                return current;
            }
            state.changed().await.unwrap();
        }
    })
    .await
    .expect("the state never came")
}

#[tokio::test]
async fn a_phone_finds_the_tv_pairs_with_the_code_on_screen_and_is_told_the_state() {
    let mut tv = Tv::new().await;
    let network = MemoryNetwork::new([127, 0, 0, 1].into());
    network.advertise(&tv.server.advert());

    let (sink, mut found) = DiscoverySink::channel();
    network.start(sink);
    let Some(Discovery::Found(seen)) = timeout(WAIT, found.recv()).await.unwrap() else {
        panic!("no TV found")
    };
    network.stop();
    assert_eq!(seen.instance, "Living room");
    assert!(seen.is(&tv.identity.fingerprint()));

    let phone = Identity::generate().unwrap();
    let mut client =
        RemoteClient::connect(seen.addresses[0], &phone, None, &hello("Hall phone"))
            .await
            .unwrap();
    assert!(!client.is_paired());
    assert_eq!(client.tv_name(), "Living room");
    client.request_code().await.unwrap();
    let code = tv.shown_code().await;
    assert_eq!(code.len(), 6);
    let pinned = client.pair(&code).await.unwrap();

    assert_eq!(pinned, tv.identity.fingerprint());
    let kept = tv.trust.get(&phone.fingerprint()).expect("the TV keeps the phone");
    assert_eq!(kept.name, "Hall phone");
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::HidePairingCode)
    );
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::Paired { phone: "Hall phone".into() })
    );

    let mut state = client.state();
    next_state(&mut state, |s| s.screen == Screen::Home).await;
}

#[tokio::test]
async fn a_phone_pairs_with_a_code_the_tv_opened_itself_without_asking() {
    let mut tv = Tv::new().await;
    let code = tv.server.open_code();
    assert_eq!(code.len(), 6);
    let phone = Identity::generate().unwrap();
    // As a scanned QR code has it: the TV's key pinned before connecting.
    let pin = Some(tv.identity.fingerprint());
    let mut client =
        RemoteClient::connect(tv.addr(), &phone, pin, &hello("Den tablet")).await.unwrap();
    client.pair(&code).await.expect("the TV's own code pairs");
    assert!(tv.trust.get(&phone.fingerprint()).is_some());
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::HidePairingCode)
    );
    assert_eq!(
        timeout(WAIT, tv.front.recv()).await.unwrap(),
        Some(FrontIntent::Paired { phone: "Den tablet".into() })
    );

    // Closed with its screen, it pairs nobody.
    tv.server.open_code();
    let late = tv.server.open_code();
    tv.server.close_code();
    let other = Identity::generate().unwrap();
    let mut client =
        RemoteClient::connect(tv.addr(), &other, None, &hello("Late phone")).await.unwrap();
    assert!(client.pair(&late).await.is_err());

    // A code a phone asked for is abandoned when the TV's dialog is cancelled.
    client.request_code().await.unwrap();
    let asked = tv.shown_code().await;
    tv.server.close_code();
    assert!(client.pair(&asked).await.is_err());
}

#[tokio::test]
async fn a_wrong_code_does_not_pair_and_the_code_is_dropped_after_three_tries() {
    let mut tv = Tv::new().await;
    let phone = Identity::generate().unwrap();
    let mut client =
        RemoteClient::connect(tv.addr(), &phone, None, &hello("Guess")).await.unwrap();
    client.request_code().await.unwrap();
    let code = tv.shown_code().await;
    let wrong = if code == "000000" { "000001" } else { "000000" };

    for _ in 0..3 {
        assert!(matches!(client.pair(wrong).await, Err(Error::Pair(_))));
    }
    assert!(matches!(client.pair(&code).await, Err(Error::PairFailed(PairFailure::NoCode))));
    assert!(tv.trust.get(&phone.fingerprint()).is_none());
}

#[tokio::test]
async fn an_unpaired_phone_is_refused_commands_and_told_nothing() {
    let mut tv = Tv::new().await;
    tv.playing();
    let stranger = Identity::generate().unwrap();
    let mut client =
        RemoteClient::connect(tv.addr(), &stranger, None, &hello("Stranger")).await.unwrap();

    let refused = client.send(RemoteCommand::TogglePause).await;
    assert!(matches!(refused, Err(Error::Rejected(Rejection::NotPaired))));
    let refused = client.send(RemoteCommand::Text { text: "x".into(), submit: true }).await;
    assert!(matches!(refused, Err(Error::Rejected(Rejection::NotPaired))));

    client.ping().await.unwrap();
    assert!(client.state().borrow().is_none(), "no state for a phone the TV does not know");
    assert!(tv.engine.try_recv().is_err(), "nothing reached the engine");
}

#[tokio::test]
async fn commands_reach_the_engine_and_the_front_end_in_order() {
    let mut tv = Tv::new().await;
    let (_, mut client) = paired_phone(&mut tv, "Hall phone").await;

    let refused = client.send(RemoteCommand::TogglePause).await;
    assert!(
        matches!(refused, Err(Error::Rejected(Rejection::NotNow))),
        "nothing is playing yet"
    );

    client
        .send(RemoteCommand::Play { id: AnilistId::new(4242), episode: "3".into() })
        .await
        .unwrap();
    tv.playing();
    client.send(RemoteCommand::TogglePause).await.unwrap();
    client.send(RemoteCommand::Seek { by: 30.0 }).await.unwrap();
    client.send(RemoteCommand::Next).await.unwrap();

    let mut got = Vec::new();
    for _ in 0..4 {
        got.push(timeout(WAIT, tv.engine.recv()).await.unwrap().unwrap());
    }
    assert!(
        matches!(&got[0], Command::Play { id, episode } if id.get() == 4242 && episode == "3")
    );
    assert!(matches!(got[1], Command::Player(PlayerCommand::TogglePause)));
    assert!(matches!(got[2], Command::Player(PlayerCommand::Seek(s)) if s == 30.0));
    assert!(matches!(got[3], Command::Player(PlayerCommand::Step(1))));

    client.send(RemoteCommand::Text { text: "orchard".into(), submit: false }).await.unwrap();
    client.send(RemoteCommand::Navigate { key: Key::Down }).await.unwrap();
    let text = timeout(WAIT, tv.front.recv()).await.unwrap().unwrap();
    assert_eq!(text, FrontIntent::Text { text: "orchard".into(), submit: false });
    let key = timeout(WAIT, tv.front.recv()).await.unwrap().unwrap();
    assert_eq!(key, FrontIntent::Navigate { key: RemoteKey::Down });
}

#[tokio::test]
async fn every_paired_phone_is_told_what_plays_and_where() {
    let mut tv = Tv::new().await;
    let (_, first) = paired_phone(&mut tv, "Phone one").await;
    let (_, second) = paired_phone(&mut tv, "Phone two").await;
    assert_eq!(tv.server.connected(), 2);

    tv.playing();
    tv.server.observe(&Event::Playback {
        position: 61.0,
        duration: Some(1440.0),
        paused: true,
    });
    tv.server.set_screen(Screen::Player);

    for client in [&first, &second] {
        let mut state = client.state();
        let seen = next_state(&mut state, |s| {
            s.screen == Screen::Player && s.now_playing.as_ref().is_some_and(|n| n.paused)
        })
        .await;
        let now = seen.now_playing.unwrap();
        assert_eq!(now.title, "Harbour Lights");
        assert_eq!(now.episode_title.as_deref(), Some("Low Tide"));
        assert_eq!(now.position, 61.0);
        assert!(!now.loading);
    }
}

#[tokio::test]
async fn a_link_reconnects_to_a_tv_that_came_back_and_hears_its_state_again() {
    let dir = tempfile::tempdir().unwrap();
    let tv_identity = Identity::generate().unwrap();
    let trust: Arc<dyn TrustStore> = Arc::new(FileTrust::open(dir.path()).unwrap());
    let mut tv = Tv::start(local(0), tv_identity.clone(), Arc::clone(&trust)).await;
    let addr = tv.addr();
    let (phone, client) = paired_phone(&mut tv, "Hall phone").await;
    drop(client);

    let options = LinkOptions {
        retry_min: Duration::from_millis(20),
        retry_max: Duration::from_millis(100),
        ping: Duration::from_secs(30),
    };
    let link = Link::open(addr, phone, tv_identity.fingerprint(), hello("Hall phone"), options);
    let mut status = link.status();
    let mut state = link.state();
    tv.server.set_screen(Screen::Settings);
    next_state(&mut state, |s| s.screen == Screen::Settings).await;

    // The TV app restarts: the same key, the same paired phones read back from disk.
    tv.server.stop().await;
    timeout(WAIT, status.wait_for(|s| matches!(s, LinkStatus::Retrying { .. })))
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(link.send(RemoteCommand::Skip).await, Err(Error::NotConnected)));

    let reopened: Arc<dyn TrustStore> = Arc::new(FileTrust::open(dir.path()).unwrap());
    let mut tv = Tv::start(addr, tv_identity, reopened).await;
    tv.server.set_screen(Screen::Search { query: "lights".into() });
    timeout(WAIT, status.wait_for(|s| matches!(s, LinkStatus::Connected { .. })))
        .await
        .unwrap()
        .unwrap();
    next_state(
        &mut state,
        |s| matches!(&s.screen, Screen::Search { query } if query == "lights"),
    )
    .await;

    link.send(RemoteCommand::Text { text: "lights at".into(), submit: false }).await.unwrap();
    let typed = timeout(WAIT, tv.front.recv()).await.unwrap().unwrap();
    assert_eq!(typed, FrontIntent::Text { text: "lights at".into(), submit: false });
}

#[tokio::test]
async fn a_link_will_not_talk_to_another_tv_at_the_same_address() {
    let mut tv = Tv::new().await;
    let addr = tv.addr();
    let (phone, client) = paired_phone(&mut tv, "Hall phone").await;
    let pinned = client.tv();
    drop(client);
    tv.server.stop().await;

    let impostor =
        Tv::start(addr, Identity::generate().unwrap(), Arc::new(MemoryTrust::new())).await;
    let refused = RemoteClient::connect(addr, &phone, Some(pinned), &hello("Hall phone")).await;
    assert!(refused.is_err(), "a key other than the pinned one is refused");
    assert_eq!(impostor.server.connected(), 0);
}

#[tokio::test]
async fn a_revoked_phone_is_cut_off_at_once_and_refused_after() {
    let mut tv = Tv::new().await;
    let addr = tv.addr();
    let (phone, client) = paired_phone(&mut tv, "Lost phone").await;
    let pinned = client.tv();
    drop(client);

    let link =
        Link::open(addr, phone.clone(), pinned, hello("Lost phone"), LinkOptions::default());
    let mut status = link.status();
    timeout(WAIT, status.wait_for(|s| matches!(s, LinkStatus::Connected { .. })))
        .await
        .unwrap()
        .unwrap();

    assert!(tv.server.revoke(&phone.fingerprint()).unwrap());
    timeout(WAIT, status.wait_for(|s| *s == LinkStatus::Unpaired)).await.unwrap().unwrap();
    assert!(tv.server.paired().is_empty());

    let mut again =
        RemoteClient::connect(addr, &phone, Some(pinned), &hello("Lost phone")).await.unwrap();
    assert!(!again.is_paired());
    assert!(matches!(
        again.send(RemoteCommand::Stop).await,
        Err(Error::Rejected(Rejection::NotPaired))
    ));
}

#[tokio::test]
async fn a_phone_that_unpairs_itself_is_forgotten() {
    let mut tv = Tv::new().await;
    let (phone, mut client) = paired_phone(&mut tv, "Old phone").await;
    client.unpair().await.unwrap();
    let mut ended = client.ended();
    timeout(WAIT, ended.wait_for(|e| e.is_some())).await.unwrap().unwrap();
    assert!(tv.trust.get(&phone.fingerprint()).is_none());
}
