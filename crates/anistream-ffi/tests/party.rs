//! Watch parties as the bindings drive them: an invite written and read, and a party joined on
//! the fake server, steered by a member it plays, and left, with no network.
#![cfg(all(feature = "testing", feature = "party"))]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use anistream_core::ids::AnilistId;
use anistream_ffi::{
    FfiDirs, FfiEngine, FfiHost, FfiStart, parse_party_invite, party_invite_link,
};
use anistream_model::{
    command::Command,
    event::Event,
    party::{PartyInvite, PartyMessageKind},
};

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

fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&Event) -> bool + Send + 'static,
) -> Vec<Event> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        let found = loop {
            let batch = futures::executor::block_on(engine.next_events());
            if batch.is_empty() {
                break false;
            }
            let found = batch.iter().any(&until);
            read.extend(batch);
            if found {
                break true;
            }
        };
        let _ = tx.send((read, found));
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

#[test]
fn an_invite_reads_back_as_it_was_written() {
    let invite = PartyInvite {
        server: "syncplay.example:8999".into(),
        room: "amber anchor".into(),
        id: AnilistId::new(21),
        episode: "3".into(),
    };
    let link = party_invite_link(invite.clone());
    assert!(link.starts_with("anistream://party?"), "{link}");
    assert_eq!(parse_party_invite(link), Some(invite));
    assert_eq!(parse_party_invite("anistream://party?room=amber".into()), None);
}

#[test]
fn a_party_is_joined_hears_its_members_and_is_left() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet), FfiStart::default())
        .expect("the engine starts");
    let party = engine.start_fake_party().expect("the fake server starts");

    engine.send(Command::PartyJoin {
        server: party.server(),
        room: "amber-anchor".into(),
        name: Some("Ada".into()),
    });
    let read = read_until(&engine, |e| matches!(e, Event::PartyMembers(_)));
    assert!(read.iter().any(|e| matches!(e, Event::PartyJoined(view) if view.name == "Ada")));

    party.add_member("amber-anchor".into(), "Mika".into());
    party.member_says("Mika".into(), "starting soon".into());
    let read = read_until(
        &engine,
        |e| matches!(e, Event::PartyMessage(m) if matches!(&m.kind, PartyMessageKind::Chat { .. })),
    );
    let said = read.iter().find_map(|e| match e {
        Event::PartyMessage(m) if matches!(&m.kind, PartyMessageKind::Chat { from } if from == "Mika") => {
            Some(m.text.clone())
        }
        _ => None,
    });
    assert!(said.is_some_and(|text| text.contains("starting soon")), "{read:?}");

    engine.send(Command::PartyLeave);
    read_until(&engine, |e| matches!(e, Event::PartyLeft { reason: None }));
    party.stop();
    engine.shutdown();
}
