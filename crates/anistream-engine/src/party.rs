//! Watch parties: a room on a Syncplay server that every playback here plays with, through
//! `anistream-party`'s client. Built with the `party` feature; without it every party command
//! is answered with [`NO_PARTY`](crate::playback::NO_PARTY).
//!
//! A party is one task beside the engine. It holds the connection, opens the
//! [`PartyHub`](crate::playback::seat::PartyHub) that playbacks take their seats from, carries
//! what the room asks to the player and what the player does to the room, and tells the front
//! end the party as it stands. The engine keeps a handle to steer it: ready, and leave.

use anistream_core::ids::AnilistId;
use anistream_model::{
    event::Event,
    party::{
        PartyConnection, PartyEpisode, PartyForm, PartyInvite, PartyMember, PartyMessage,
        PartyMessageKind, PartyView, SUGGESTED_SERVER, SUGGESTED_SERVER_RUN_BY, room_name,
    },
};
use anistream_party::{
    FileInfo, Member, NoticeKind, Party, PartyEvent, PartyOptions, PlayerAction, Sample, Server,
};
use tokio::sync::mpsc;

use crate::{
    jobs::JobGuard,
    outbox::Outbox,
    playback::seat::{SeatCue, SeatMessage, SeatReport},
    services::Services,
    state::EngineState,
};

/// The party going, as the engine steers it.
pub struct Running {
    control: mpsc::UnboundedSender<Control>,
    /// `host:port`, as the server was read.
    server: String,
    room: String,
}

enum Control {
    Ready(bool),
    Leave,
    /// Leave for another party, which says itself: this one ends without a word.
    Switch,
}

impl Running {
    fn going(&self) -> bool {
        !self.control.is_closed()
    }
}

/// The party going, if one is.
fn going(engine: &EngineState) -> Option<&Running> {
    engine.party.as_ref().filter(|running| running.going())
}

/// What the form that starts a party opens with.
pub fn form(engine: &EngineState, tx: &Outbox) {
    let syncplay = &engine.config.effective().syncplay;
    let mut recent = syncplay.recent_servers.clone();
    recent.retain(|server| !server.trim().is_empty());
    let _ = tx.send(Event::PartyForm(PartyForm {
        recent_servers: recent,
        suggested_server: SUGGESTED_SERVER.into(),
        suggested_run_by: SUGGESTED_SERVER_RUN_BY.into(),
        room: room_name(rand::random()),
        name: syncplay.name.clone(),
    }));
}

/// Join the room `room` on `server`, unless this is the party already going; any other party
/// is left first. What plays now, and every play after, plays with the room. `false`, with the
/// reason said, when the server or the room can't be used.
pub fn join(
    server: &str,
    room: &str,
    name: Option<&str>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> bool {
    let server = match Server::parse(server) {
        Ok(server) => server,
        Err(bad) => return refuse(bad.to_string(), tx),
    };
    let room = room.trim();
    if room.is_empty() {
        return refuse("A party needs a room to meet in".into(), tx);
    }
    let address = server.to_string();
    if let Some(running) = going(engine) {
        if running.server == address && running.room == room {
            return true;
        }
        let _ = running.control.send(Control::Switch);
    }
    let name = name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map_or_else(|| engine.config.effective().syncplay.name.clone(), str::to_owned);
    let name = if name.trim().is_empty() { "anistream".to_owned() } else { name };

    tracing::info!(server = %address, room, "joining a watch party");
    let (party, events) = Party::join(PartyOptions::new(server, room, name.clone()));
    let seats = services.party.open();
    let (control, controls) = mpsc::unbounded_channel();
    let view = PartyView {
        server: address.clone(),
        room: room.to_owned(),
        name,
        connection: PartyConnection::Connected,
        encrypted: false,
        ready: false,
        episode: None,
        invite: None,
        motd: None,
        latency_ms: None,
    };
    tokio::spawn(run(party, events, seats, controls, view, tx.clone()));
    engine.party = Some(Running { control, server: address, room: room.to_owned() });
    true
}

/// Join the party a play asks for, from what it names and else the party going, then the
/// settings: their server or the last one used, their room or a new one, their name. `false`,
/// with the reason said, when there is no server to use.
pub fn join_for_play(
    server: Option<String>,
    room: Option<String>,
    name: Option<String>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> bool {
    let named =
        |value: Option<String>| value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
    let (server, room) = (named(server), named(room));
    let current = going(engine).map(|running| (running.server.clone(), running.room.clone()));
    let syncplay = &engine.config.effective().syncplay;
    let server = server
        .or_else(|| current.as_ref().map(|(server, _)| server.clone()))
        .or_else(|| named(Some(syncplay.server.clone())))
        .or_else(|| syncplay.recent_servers.iter().find_map(|s| named(Some(s.clone()))));
    let Some(server) = server else {
        return refuse(
            format!(
                "Choose a server for the party first, as syncplay.server. \
                 {SUGGESTED_SERVER} is run by {SUGGESTED_SERVER_RUN_BY}."
            ),
            tx,
        );
    };
    let same_server = current.as_ref().is_some_and(|(going, _)| {
        Server::parse(&server).is_ok_and(|asked| asked.to_string() == *going)
    });
    let room = room
        .or_else(|| current.filter(|_| same_server).map(|(_, room)| room))
        .or_else(|| named(syncplay.room.clone()))
        .unwrap_or_else(|| room_name(rand::random()));
    join(&server, &room, name.as_deref(), engine, services, tx)
}

/// Join the party the settings name before a play nobody asked a party of, when they send
/// every play to one (`syncplay.enabled`) and none is going. A play goes on alone when it
/// can't be joined: the reason is said, and nothing else waits on it.
pub fn join_by_default(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    if engine.config.effective().syncplay.enabled && going(engine).is_none() {
        join_for_play(None, None, None, engine, services, tx);
    }
}

/// Say whether this member is ready.
pub fn ready(ready: bool, engine: &EngineState, tx: &Outbox) {
    match going(engine) {
        Some(running) => {
            let _ = running.control.send(Control::Ready(ready));
        }
        None => not_in_one(tx),
    }
}

/// Leave the party. What plays goes on, as a private session.
pub fn leave(engine: &mut EngineState, tx: &Outbox) {
    match engine.party.take().filter(Running::going) {
        Some(running) => {
            let _ = running.control.send(Control::Leave);
        }
        None => not_in_one(tx),
    }
}

/// A party command with no party going: the front end is told there is none.
fn not_in_one(tx: &Outbox) {
    let _ = tx.send(Event::PartyLeft { reason: None });
}

/// A party that can't be joined: the reason, as the party's end.
fn refuse(reason: String, tx: &Outbox) -> bool {
    tracing::info!(%reason, "no watch party");
    let _ = tx.send(Event::PartyLeft { reason: Some(reason) });
    false
}

/// Play `episode` of `id` with the party a play names: see
/// [`Command::PlayParty`](anistream_model::command::Command::PlayParty).
#[allow(clippy::too_many_arguments)]
pub fn play(
    id: AnilistId,
    episode: String,
    (server, room, name): (Option<String>, Option<String>, Option<String>),
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if join_for_play(server, room, name, engine, services, tx) {
        crate::playback::start::play_party(id, episode, job, engine, services, tx);
    }
}

/// The party's task: the room's events to the front end and the player, the player's reports
/// and the engine's asks to the room, until the party ends.
async fn run(
    party: Party,
    mut events: mpsc::UnboundedReceiver<PartyEvent>,
    mut seats: mpsc::UnboundedReceiver<SeatMessage>,
    mut controls: mpsc::UnboundedReceiver<Control>,
    mut view: PartyView,
    tx: Outbox,
) {
    // Where the room's asks of the player go: the playback seated last.
    let mut cues: Option<mpsc::UnboundedSender<SeatCue>> = None;
    let mut joined = false;
    let mut controls_open = true;
    let mut seats_open = true;
    // Left for another party, or by an engine shutting down: the end goes unsaid.
    let mut quiet = false;
    let cue = |cues: &Option<mpsc::UnboundedSender<SeatCue>>, cue: SeatCue| {
        if let Some(cues) = cues {
            let _ = cues.send(cue);
        }
    };
    loop {
        tokio::select! {
            event = events.recv() => {
                let Some(event) = event else { break };
                match event {
                    PartyEvent::Joined { name, room, encrypted, motd } => {
                        joined = true;
                        (view.name, view.room, view.encrypted, view.motd) =
                            (name, room, encrypted, motd);
                        let _ = tx.send(Event::PartyJoined(view.clone()));
                    }
                    PartyEvent::Members(members) => {
                        let members = members.into_iter().map(member).collect();
                        let _ = tx.send(Event::PartyMembers(members));
                    }
                    PartyEvent::Player(action) => cue(&cues, match action {
                        PlayerAction::Pause => SeatCue::Pause(true),
                        PlayerAction::Play => SeatCue::Pause(false),
                        PlayerAction::Seek(to) => SeatCue::Seek(to),
                        PlayerAction::Speed(speed) => SeatCue::Speed(speed),
                    }),
                    PartyEvent::Notice(notice) => {
                        cue(&cues, SeatCue::Notice(notice.text.clone()));
                        let kind = message_kind(notice.kind);
                        let _ = tx.send(Event::PartyMessage(PartyMessage { kind, text: notice.text }));
                    }
                    PartyEvent::Latency(millis) => {
                        view.latency_ms = Some(millis);
                        if joined {
                            let _ = tx.send(Event::PartyState(view.clone()));
                        }
                    }
                    PartyEvent::Reconnecting { attempt, of } => {
                        if attempt == 1 {
                            cue(&cues, SeatCue::Notice("Lost the party's server, trying again".into()));
                        }
                        view.connection = PartyConnection::Reconnecting { attempt, of };
                        let _ = tx.send(Event::PartyState(view.clone()));
                    }
                    PartyEvent::Rejoined { name, encrypted } => {
                        cue(&cues, SeatCue::Notice("Back with the party".into()));
                        (view.name, view.encrypted) = (name, encrypted);
                        view.connection = PartyConnection::Connected;
                        let _ = tx.send(Event::PartyState(view.clone()));
                    }
                    PartyEvent::Left(reason) => {
                        tracing::info!(?reason, "the watch party ended");
                        if !quiet {
                            let said = reason.clone().unwrap_or_else(|| "Left the party".into());
                            cue(&cues, SeatCue::Notice(said));
                            let _ = tx.send(Event::PartyLeft { reason });
                        }
                        return;
                    }
                }
            }
            message = seats.recv(), if seats_open => match message {
                // No playback can take a seat any more: the hub was opened for another party,
                // or the engine is gone.
                None => {
                    seats_open = false;
                    quiet = true;
                    party.leave();
                }
                Some(SeatMessage::Seated(seat)) => {
                    cues = Some(seat);
                    party.attach();
                }
                Some(SeatMessage::Report(report)) => match report {
                    SeatReport::Playing { id, episode, name, duration } => {
                        party.file(Some(FileInfo { name, duration }));
                        let invite = PartyInvite {
                            server: view.server.clone(),
                            room: view.room.clone(),
                            id,
                            episode: episode.clone(),
                        };
                        view.invite = Some(invite.link());
                        let changed = view.episode.as_ref().is_none_or(|e| e.id != id || e.episode != episode);
                        view.episode = Some(PartyEpisode { id, episode });
                        if changed && joined {
                            let _ = tx.send(Event::PartyState(view.clone()));
                        }
                    }
                    SeatReport::Sample { position, paused, speed } => {
                        party.sample(Sample { position, paused, speed });
                    }
                    SeatReport::Ended { leave } => {
                        party.detach();
                        party.file(None);
                        cues = None;
                        if leave {
                            party.leave();
                        }
                    }
                },
            },
            control = controls.recv(), if controls_open => match control {
                Some(Control::Ready(ready)) => {
                    party.ready(ready);
                    view.ready = ready;
                    if joined {
                        let _ = tx.send(Event::PartyState(view.clone()));
                    }
                }
                Some(Control::Leave) => party.leave(),
                Some(Control::Switch) => {
                    quiet = true;
                    party.leave();
                }
                None => {
                    controls_open = false;
                    party.leave();
                }
            },
        }
    }
    if !quiet {
        let _ = tx.send(Event::PartyLeft { reason: None });
    }
}

fn member(member: Member) -> PartyMember {
    PartyMember {
        name: member.name,
        ready: member.ready,
        duration: member.file.as_ref().and_then(|f| f.duration),
        file: member.file.map(|f| f.name),
        file_matches: member.file_matches,
        you: member.you,
    }
}

fn message_kind(kind: NoticeKind) -> PartyMessageKind {
    match kind {
        NoticeKind::Chat { from } => PartyMessageKind::Chat { from },
        NoticeKind::Joined { name } => PartyMessageKind::Joined { name },
        NoticeKind::Left { name } => PartyMessageKind::Left { name },
        NoticeKind::Paused { by } => PartyMessageKind::Paused { by },
        NoticeKind::Resumed { by } => PartyMessageKind::Resumed { by },
        NoticeKind::Seeked { by } => PartyMessageKind::Seeked { by },
        NoticeKind::Corrected => PartyMessageKind::Corrected,
        NoticeKind::AllReady => PartyMessageKind::AllReady,
    }
}
