//! The client against the fake server, over the loopback interface: joining, members, a room
//! that pauses, seeks and drifts, getting ready, chat, and a connection lost and made again.

use std::time::Duration;

use tokio::{
    sync::mpsc,
    time::{Instant, timeout},
};

use crate::{
    client::{Party, PartyEvent, PartyOptions, Retry, Server, Tls},
    fake::{FakeOptions, FakeServer, FakeTls},
    notice::NoticeKind,
    sync::{PlayerAction, Sample},
    wire::FileInfo,
};

const ROOM: &str = "quiet-harbor";
const EPISODE: &str = "Starlit Harbor · Episode 3";
const LENGTH: f64 = 1440.0;

/// A player that does what it is told, and plays on by the clock.
struct FakePlayer {
    position: f64,
    paused: bool,
    speed: f64,
    at: Instant,
}

impl FakePlayer {
    fn new(position: f64, paused: bool) -> Self {
        Self { position, paused, speed: 1.0, at: Instant::now() }
    }

    fn position(&self) -> f64 {
        if self.paused {
            self.position
        } else {
            self.position + self.at.elapsed().as_secs_f64() * self.speed
        }
    }

    fn settle(&mut self) {
        self.position = self.position();
        self.at = Instant::now();
    }

    fn apply(&mut self, action: &PlayerAction) {
        self.settle();
        match action {
            PlayerAction::Pause => self.paused = true,
            PlayerAction::Play => self.paused = false,
            PlayerAction::Seek(to) => self.position = *to,
            PlayerAction::Speed(speed) => self.speed = *speed,
        }
    }

    fn sample(&self) -> Sample {
        Sample { position: self.position(), paused: self.paused, speed: self.speed }
    }
}

/// One member who connects, with a player, as a front end would run them.
struct Viewer {
    party: Party,
    events: mpsc::UnboundedReceiver<PartyEvent>,
    player: Option<FakePlayer>,
    seen: Vec<PartyEvent>,
}

impl Viewer {
    fn join(server: &FakeServer, name: &str) -> Self {
        Self::join_with(server, name, |_| {})
    }

    fn join_with(
        server: &FakeServer,
        name: &str,
        adjust: impl FnOnce(&mut PartyOptions),
    ) -> Self {
        let mut options =
            PartyOptions::new(Server::parse(&server.address()).unwrap(), ROOM, name);
        options.retry = Retry {
            attempts: 3,
            first: Duration::from_millis(50),
            longest: Duration::from_millis(100),
        };
        options.silence = Duration::from_secs(2);
        adjust(&mut options);
        let (party, events) = Party::join(options);
        Self { party, events, player: None, seen: Vec::new() }
    }

    /// Open the episode and start reporting a player at `position`.
    fn watch(&mut self, position: f64, paused: bool) {
        self.party.file(Some(FileInfo { name: EPISODE.into(), duration: Some(LENGTH) }));
        self.party.attach();
        self.player = Some(FakePlayer::new(position, paused));
    }

    fn player(&mut self) -> &mut FakePlayer {
        self.player.as_mut().expect("watching")
    }

    /// Run, reporting the player every 50 ms and doing what it is told, until an event matches.
    async fn until(&mut self, what: &str, matches: impl Fn(&PartyEvent) -> bool) -> PartyEvent {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut report = tokio::time::interval(Duration::from_millis(50));
        loop {
            let event = tokio::select! {
                event = self.events.recv() => event,
                _ = report.tick() => {
                    if let Some(player) = &self.player {
                        self.party.sample(player.sample());
                    }
                    continue;
                }
                () = tokio::time::sleep_until(deadline) => {
                    panic!("no {what} in time; saw {:#?}", self.seen)
                }
            };
            let Some(event) = event else {
                panic!("the party ended before {what}; saw {:#?}", self.seen)
            };
            if let (PartyEvent::Player(action), Some(player)) = (&event, self.player.as_mut()) {
                player.apply(action);
            }
            self.seen.push(event.clone());
            if matches(&event) {
                return event;
            }
        }
    }

    /// Run for a while, reporting and obeying, and hand back what came.
    async fn run_for(&mut self, time: Duration) -> Vec<PartyEvent> {
        let start = self.seen.len();
        let _ = timeout(time, self.until("nothing", |_| false)).await;
        self.seen[start..].to_vec()
    }

    async fn joined(&mut self) -> String {
        match self.until("joining", |e| matches!(e, PartyEvent::Joined { .. })).await {
            PartyEvent::Joined { name, .. } => name,
            _ => unreachable!(),
        }
    }

    async fn notice(&mut self, kind: impl Fn(&NoticeKind) -> bool) -> String {
        match self
            .until("a notice", |e| matches!(e, PartyEvent::Notice(n) if kind(&n.kind)))
            .await
        {
            PartyEvent::Notice(notice) => notice.text,
            _ => unreachable!(),
        }
    }
}

async fn server() -> FakeServer {
    FakeServer::start(FakeOptions { tick: Duration::from_millis(50), ..FakeOptions::default() })
        .await
        .unwrap()
}

/// Wait for the server's room to be in a state.
async fn room_until(
    server: &FakeServer,
    what: &str,
    ok: impl Fn(&crate::fake::FakeRoom) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if server.room(ROOM).is_some_and(|room| ok(&room)) {
            return;
        }
        assert!(Instant::now() < deadline, "the room never {what}: {:?}", server.room(ROOM));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn joining_names_this_member_and_lists_the_room() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    let mut viewer = Viewer::join(&server, "Ada");
    let name = viewer.joined().await;
    assert_eq!(name, "Ada");
    let members = viewer
        .until("both members", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    let PartyEvent::Members(members) = members else { unreachable!() };
    assert!(members[0].you, "this member comes first");
    assert_eq!(members[1].name, "Mika");
    assert_eq!(members[1].file.as_ref().map(|f| f.name.as_str()), Some(EPISODE));
    assert_eq!(members[1].file_matches, None, "nothing is open here to match");

    viewer.watch(0.0, true);
    let members = viewer
        .until("a matching file", |e| {
            matches!(e, PartyEvent::Members(m) if m.iter().any(|m| m.file_matches == Some(true)))
        })
        .await;
    assert!(matches!(members, PartyEvent::Members(_)));
}

#[tokio::test]
async fn a_name_that_is_taken_gets_another() {
    let server = server().await;
    let _mika = server.member(ROOM, "Ada");
    let mut viewer = Viewer::join(&server, "Ada");
    assert_eq!(viewer.joined().await, "Ada_");
}

#[tokio::test]
async fn a_refusal_and_an_unreachable_server_end_the_party_saying_why() {
    let server = FakeServer::start(FakeOptions {
        refuse: Some("This room is full".into()),
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let mut viewer = Viewer::join(&server, "Ada");
    let left = viewer.until("leaving", |e| matches!(e, PartyEvent::Left(_))).await;
    let PartyEvent::Left(Some(reason)) = left else { panic!("{left:?}") };
    assert!(reason.ends_with("refused the party: This room is full"), "{reason}");

    // A port nobody listens on.
    let closed = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = closed.local_addr().unwrap().to_string();
    drop(closed);
    let options = PartyOptions::new(Server::parse(&address).unwrap(), ROOM, "Ada");
    let (_party, mut events) = Party::join(options);
    let left = timeout(Duration::from_secs(5), events.recv()).await.unwrap();
    let Some(PartyEvent::Left(Some(reason))) = left else { panic!("{left:?}") };
    assert!(reason.starts_with(&format!("Could not reach {address}")), "{reason}");
}

#[tokio::test]
async fn a_server_without_tls_is_spoken_to_in_the_clear() {
    for tls in [FakeTls::Decline, FakeTls::Unknown] {
        let server =
            FakeServer::start(FakeOptions { tls, ..FakeOptions::default() }).await.unwrap();
        let mut viewer = Viewer::join_with(&server, "Ada", |o| o.tls = Tls::Prefer);
        let joined = viewer.until("joining", |e| matches!(e, PartyEvent::Joined { .. })).await;
        assert!(matches!(joined, PartyEvent::Joined { encrypted: false, .. }), "{tls:?}");
    }
}

#[tokio::test]
async fn the_first_to_watch_sets_the_room_and_the_next_is_brought_to_it() {
    let server = server().await;
    let mut ada = Viewer::join(&server, "Ada");
    ada.joined().await;
    ada.until("the member list", |e| matches!(e, PartyEvent::Members(_))).await;
    ada.watch(300.0, true);
    ada.run_for(Duration::from_millis(400)).await;
    room_until(&server, "took Ada's position", |r| (r.position - 300.0).abs() < 0.5).await;

    let mut ren = Viewer::join(&server, "Ren");
    ren.joined().await;
    ren.until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2)).await;
    ren.watch(0.0, false);
    ren.until(
        "being brought to the room",
        |e| matches!(e, PartyEvent::Player(PlayerAction::Seek(to)) if (to - 300.0).abs() < 0.5),
    )
    .await;
    ren.until("pausing with the room", |e| {
        matches!(e, PartyEvent::Player(PlayerAction::Pause))
    })
    .await;
    // The room is still Ada's: Ren arriving moved nobody.
    ren.run_for(Duration::from_millis(300)).await;
    let room = server.room(ROOM).unwrap();
    assert!((room.position - 300.0).abs() < 0.5 && room.paused, "{room:?}");
}

#[tokio::test]
async fn someone_else_pausing_seeking_and_resuming_moves_this_player() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    mika.play();
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer
        .until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    viewer.watch(0.0, false);
    viewer.run_for(Duration::from_millis(300)).await;

    mika.pause();
    viewer.until("pausing", |e| matches!(e, PartyEvent::Player(PlayerAction::Pause))).await;
    let text = viewer.notice(|k| matches!(k, NoticeKind::Paused { by } if by == "Mika")).await;
    assert!(text.starts_with("Mika paused at 0:0"), "{text}");

    mika.seek(600.0);
    viewer
        .until("seeking", |e| {
            matches!(e, PartyEvent::Player(PlayerAction::Seek(to)) if (to - 600.0).abs() < 0.5)
        })
        .await;
    let text = viewer.notice(|k| matches!(k, NoticeKind::Seeked { .. })).await;
    assert!(text.ends_with("to 10:00"), "{text}");

    mika.play();
    viewer.until("resuming", |e| matches!(e, PartyEvent::Player(PlayerAction::Play))).await;
    assert_eq!(
        viewer.notice(|k| matches!(k, NoticeKind::Resumed { .. })).await,
        "Mika resumed"
    );
    // Doing what the room said is not news for the room.
    viewer.run_for(Duration::from_millis(400)).await;
    assert_eq!(server.room(ROOM).unwrap().set_by.as_deref(), Some("Mika"));
}

#[tokio::test]
async fn pausing_and_seeking_here_reach_the_room() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    mika.play();
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer
        .until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    viewer.watch(0.0, false);
    viewer.run_for(Duration::from_millis(300)).await;

    viewer.player().apply(&PlayerAction::Pause);
    viewer.run_for(Duration::from_millis(200)).await;
    room_until(&server, "paused for Ada", |r| r.paused && r.set_by.as_deref() == Some("Ada"))
        .await;

    viewer.player().apply(&PlayerAction::Seek(500.0));
    viewer.run_for(Duration::from_millis(200)).await;
    room_until(&server, "moved for Ada", |r| (r.position - 500.0).abs() < 0.5).await;

    viewer.player().apply(&PlayerAction::Play);
    viewer.run_for(Duration::from_millis(200)).await;
    room_until(&server, "played for Ada", |r| !r.paused).await;
}

#[tokio::test]
async fn play_starts_when_everyone_is_ready() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer
        .until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    viewer.watch(0.0, true);
    viewer.run_for(Duration::from_millis(200)).await;

    viewer.party.ready(true);
    viewer.run_for(Duration::from_millis(200)).await;
    assert!(server.room(ROOM).unwrap().paused, "one of two ready starts nothing");
    room_until(&server, "had Ada ready", |r| {
        r.members.iter().any(|m| m.name == "Ada" && m.ready == Some(true))
    })
    .await;

    mika.ready(true);
    assert_eq!(viewer.notice(|k| *k == NoticeKind::AllReady).await, "Everyone is ready");
    viewer.until("playing", |e| matches!(e, PartyEvent::Player(PlayerAction::Play))).await;
    room_until(&server, "started", |r| !r.paused).await;
}

#[tokio::test]
async fn a_player_ahead_of_the_room_is_moved_back() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    mika.play();
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer
        .until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    viewer.watch(0.0, false);
    // Past the settling time after joining.
    viewer.run_for(Duration::from_millis(2300)).await;

    mika.fall_behind(10.0);
    let text = viewer.notice(|k| *k == NoticeKind::Corrected).await;
    assert!(text.starts_with("Back to "), "{text}");
    let seeks =
        viewer.seen.iter().filter(|e| matches!(e, PartyEvent::Player(PlayerAction::Seek(_))));
    assert!(seeks.count() >= 1);
    // The correction is this player's, not a change for the room.
    assert_eq!(server.room(ROOM).unwrap().set_by.as_deref(), Some("Mika"));
}

#[tokio::test]
async fn chat_and_comings_and_goings_are_notices() {
    let server = server().await;
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer.until("the member list", |e| matches!(e, PartyEvent::Members(_))).await;

    let mika = server.member(ROOM, "Mika");
    assert_eq!(viewer.notice(|k| matches!(k, NoticeKind::Joined { .. })).await, "Mika joined");
    mika.chat("starting in a minute\n");
    assert_eq!(
        viewer.notice(|k| matches!(k, NoticeKind::Chat { .. })).await,
        "Mika: starting in a minute"
    );
    mika.leave();
    assert_eq!(viewer.notice(|k| matches!(k, NoticeKind::Left { .. })).await, "Mika left");
    viewer
        .until(
            "the member list without Mika",
            |e| matches!(e, PartyEvent::Members(m) if m.len() == 1),
        )
        .await;
}

#[tokio::test]
async fn a_lost_connection_is_made_again_and_the_room_kept() {
    let server = server().await;
    let mika = server.member(ROOM, "Mika");
    mika.open(EPISODE, LENGTH);
    mika.seek(120.0);
    mika.play();
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer
        .until("the member list", |e| matches!(e, PartyEvent::Members(m) if m.len() == 2))
        .await;
    viewer.watch(0.0, false);
    viewer.party.ready(true);
    viewer.run_for(Duration::from_millis(300)).await;

    server.drop_connections();
    viewer
        .until("reconnecting", |e| matches!(e, PartyEvent::Reconnecting { attempt: 1, of: 3 }))
        .await;
    viewer.until("rejoining", |e| matches!(e, PartyEvent::Rejoined { .. })).await;
    // Its ready state is said again on the new connection.
    room_until(&server, "had Ada ready again", |r| {
        r.members.iter().any(|m| m.name == "Ada" && m.ready == Some(true))
    })
    .await;

    // Mika moving on while Ada is away brings Ada along on the way back.
    server.set_down(true);
    server.drop_connections();
    viewer.until("reconnecting", |e| matches!(e, PartyEvent::Reconnecting { .. })).await;
    mika.seek(900.0);
    server.set_down(false);
    viewer.until("rejoining", |e| matches!(e, PartyEvent::Rejoined { .. })).await;
    viewer
        .until("being brought back", |e| {
            matches!(e, PartyEvent::Player(PlayerAction::Seek(to)) if (to - 900.0).abs() < 2.0)
        })
        .await;
}

#[tokio::test]
async fn a_server_that_stays_away_or_sends_an_error_ends_the_party() {
    let server = server().await;
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    server.set_down(true);
    server.drop_connections();
    let left = viewer.until("leaving", |e| matches!(e, PartyEvent::Left(_))).await;
    let PartyEvent::Left(Some(reason)) = left else { panic!("{left:?}") };
    assert!(reason.contains("could not get it back"), "{reason}");
    let tries =
        viewer.seen.iter().filter(|e| matches!(e, PartyEvent::Reconnecting { .. })).count();
    assert_eq!(tries, 3);

    let server = self::server().await;
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    server.kick("Ada", "Your client is too old");
    let left = viewer.until("leaving", |e| matches!(e, PartyEvent::Left(_))).await;
    assert_eq!(
        left,
        PartyEvent::Left(Some(format!(
            "{} ended the party: Your client is too old",
            Server::parse(&server.address()).unwrap()
        )))
    );
}

#[tokio::test]
async fn leaving_says_so_and_frees_the_name() {
    let server = server().await;
    let mut viewer = Viewer::join(&server, "Ada");
    viewer.joined().await;
    viewer.party.leave();
    let left = viewer.until("leaving", |e| matches!(e, PartyEvent::Left(_))).await;
    assert_eq!(left, PartyEvent::Left(None));
    room_until(&server, "let Ada go", |r| r.members.is_empty()).await;
}
