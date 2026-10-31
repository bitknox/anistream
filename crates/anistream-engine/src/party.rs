//! Watch parties: a room on a Syncplay server that every playback here plays with, through
//! `anistream-party`'s client. Built with the `party` feature; without it every party command
//! is answered with [`NO_PARTY`](crate::playback::NO_PARTY).
//!
//! A party is one task beside the engine. It holds the connection, opens the
//! [`PartyHub`](crate::playback::seat::PartyHub) that playbacks take their seats from, carries
//! what the room asks to the player and what the player does to the room, and tells the front
//! end the party as it stands. The engine keeps a handle to steer it: ready, and leave.

use std::time::Duration;

use anistream_core::{config::PlaybackConfig, ids::AnilistId};
use anistream_meta::{Media, Meta};
use anistream_model::Msg;
use anistream_model::{
    event::Event,
    party::{
        NoSourceWhy, PartyCommand, PartyConnection, PartyEpisode, PartyForm, PartyGlimpse,
        PartyGuess, PartyMember, PartyMessage, PartyMessageKind, PartyRecentRoom,
        PartyRoomEpisode, PartyShare, PartySources, PartyStartKind, PartyView,
        SUGGESTED_SERVER, SUGGESTED_SERVER_RUN_BY, clean_party_code, room_name,
    },
};
use anistream_party::{
    FileInfo, Member, NoticeKind, Party, PartyEvent, PartyOptions, PlayerAction, Playlist,
    Sample, Server, file,
};
use anistream_providers::ProviderRegistry;
use anistream_store::{PartyRoom, Store};
use tokio::sync::mpsc;

use crate::{
    jobs::JobGuard,
    now_epoch,
    outbox::Outbox,
    party_identify::{Identified, identify},
    playback::{
        prefetch::Prefetch,
        resolve::{not_aired, resolve_for_playback},
        seat::{SeatCue, SeatMessage, SeatReport},
    },
    services::Services,
    state::EngineState,
};

/// The count before a room that is all ready plays, in seconds.
const COUNT: u32 = 3;
/// How long a look into a room waits for its members to be listed.
const LOOK_SETTLE: Duration = Duration::from_millis(1500);

/// The party going, as the engine steers it.
pub struct Running {
    control: mpsc::UnboundedSender<Control>,
    /// `host:port`, as the server was read.
    server: String,
    room: String,
}

enum Control {
    Ready(bool),
    /// Set what the room watches: Change episode, and Watch something else.
    Change(AnilistId, String),
    /// Use this title for the plain file the room plays, or, with `None`, reject the match.
    Choose(Option<AnilistId>),
    /// Start the room now.
    StartAnyway,
    /// Open the player in the room's episode again.
    OpenPlayer,
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
        room: room_name(rand::random::<u64>()),
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
    join_with(server, room, name, None, engine, services, tx)
}

/// [`join`], setting what the room watches to `pick` once in: a party this device starts.
pub fn join_with(
    server: &str,
    room: &str,
    name: Option<&str>,
    pick: Option<PartyEpisode>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> bool {
    let server = match Server::parse(server) {
        Ok(server) => server,
        Err(bad) => return refuse(Msg::verbatim(bad.to_string()), tx),
    };
    let room = room.trim();
    if room.is_empty() {
        return refuse(Msg::PartyNeedsRoom, tx);
    }
    let address = server.to_string();
    if let Some(running) = going(engine) {
        if running.server == address && running.room == room {
            if let Some(pick) = pick {
                let _ = running.control.send(Control::Change(pick.id, pick.episode));
            }
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
    let (internal, internals) = mpsc::unbounded_channel();
    let env = Env {
        meta: services.meta.clone(),
        store: services.store.clone(),
        registry: services.registry.clone(),
        prefetch: services.prefetch.clone(),
        playback: engine.config.effective().playback.clone(),
        tx: tx.clone(),
        internal,
    };
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
        room_episode: None,
        next_episode: None,
        starter: None,
        starter_present: false,
        can_start_anyway: false,
        code: None,
        link: None,
        playing: false,
        starting_in: None,
        sources: None,
        guesses: Vec::new(),
        looking: true,
        moves_on: false,
    };
    let task = Task::new(party, env, view, pick);
    tokio::spawn(task.run(events, seats, controls, internals));
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
        return refuse(needs_server(services.hosted_player), tx);
    };
    let same_server = current.as_ref().is_some_and(|(going, _)| {
        Server::parse(&server).is_ok_and(|asked| asked.to_string() == *going)
    });
    let room = room
        .or_else(|| current.filter(|_| same_server).map(|(_, room)| room))
        .or_else(|| named(syncplay.room.clone()))
        .unwrap_or_else(|| room_name(rand::random::<u64>()));
    join(&server, &room, name.as_deref(), engine, services, tx)
}

/// Join the party the settings name before a play nobody asked a party of, when they send
/// every play to one (`syncplay.enabled`) and none is going. The terminal's way in alone
/// ([`EngineOptions::every_play_a_party`]): the apps start parties when asked, and the setting
/// does nothing there. A play goes on alone when it can't be joined: the reason is said, and
/// nothing else waits on it.
///
/// [`EngineOptions::every_play_a_party`]: crate::options::EngineOptions::every_play_a_party
pub fn join_by_default(engine: &mut EngineState, services: &Services, tx: &Outbox) {
    if services.every_play_a_party
        && engine.config.effective().syncplay.enabled
        && going(engine).is_none()
    {
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

/// Why a party with no server cannot start: in the terminal's words, which name the setting to
/// set, or an app's, which has a field for it. `hosted` is whether the front end hosts its
/// player, as every app does and the terminal does not.
fn needs_server(hosted: bool) -> Msg {
    let (server, operator) = (SUGGESTED_SERVER.to_owned(), SUGGESTED_SERVER_RUN_BY.to_owned());
    if hosted {
        Msg::PartyChooseServer { server, operator }
    } else {
        Msg::PartyNeedsServer { server, operator }
    }
}

/// A party that can't be joined: the reason, as the party's end.
fn refuse(reason: Msg, tx: &Outbox) -> bool {
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

/// A lobby command: see [`PartyCommand`].
pub fn command(
    command: PartyCommand,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match command {
        PartyCommand::Lobby { server, room, name, pick } => {
            lobby((&server, &room, name.as_deref()), pick, engine, services, tx);
        }
        PartyCommand::ChangeEpisode { id, episode } => change_episode(id, episode, engine, tx),
        PartyCommand::ChooseTitle { id } => choose_title(id, engine, tx),
        PartyCommand::StartAnyway => start_anyway(engine, tx),
        PartyCommand::OpenPlayer => open_player(engine, tx),
        PartyCommand::LookIn { server, room, name } => {
            look_in((server, room, name), engine, services, tx);
        }
        PartyCommand::LoadRecent => load_recent(services, tx),
        PartyCommand::ForgetRoom { server, room } => forget_room(&server, &room, services, tx),
    }
}

/// Join a room for its lobby: see [`PartyCommand::Lobby`].
pub fn lobby(
    (server, room, name): (&str, &str, Option<&str>),
    pick: Option<PartyEpisode>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    join_with(server, room, name, pick, engine, services, tx);
}

/// Set what the room watches.
pub fn change_episode(id: AnilistId, episode: String, engine: &EngineState, tx: &Outbox) {
    steer(Control::Change(id, episode), engine, tx);
}

/// Use this title for the room's plain file, or reject the match made.
pub fn choose_title(id: Option<AnilistId>, engine: &EngineState, tx: &Outbox) {
    steer(Control::Choose(id), engine, tx);
}

/// Start the room without waiting for those who are not ready.
pub fn start_anyway(engine: &EngineState, tx: &Outbox) {
    steer(Control::StartAnyway, engine, tx);
}

/// Open the player in the room's episode once more.
pub fn open_player(engine: &EngineState, tx: &Outbox) {
    steer(Control::OpenPlayer, engine, tx);
}

fn steer(control: Control, engine: &EngineState, tx: &Outbox) {
    match going(engine) {
        Some(running) => {
            let _ = running.control.send(control);
        }
        None => not_in_one(tx),
    }
}

/// Look into a room for a moment: who is there and what it watches. Only when the room is
/// opened; nothing keeps looking.
pub fn look_in(
    (server, room, name): (String, String, Option<String>),
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let name = name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| {
            let saved = engine.config.effective().syncplay.name.clone();
            if saved.trim().is_empty() { "anistream".to_owned() } else { saved }
        });
    let (meta, tx) = (services.meta.clone(), tx.clone());
    tokio::spawn(async move {
        let failed = |reason| Event::PartyLookFailed {
            server: server.clone(),
            room: room.clone(),
            reason,
        };
        let parsed = match Server::parse(&server) {
            Ok(parsed) => parsed,
            Err(bad) => return drop(tx.send(failed(Msg::verbatim(bad.to_string())))),
        };
        let address = parsed.to_string();
        let options = PartyOptions::new(parsed, room.clone(), name);
        let glimpse = match anistream_party::look(options, LOOK_SETTLE).await {
            Ok(glimpse) => glimpse,
            Err(why) => {
                tracing::info!(%why, "could not look into a room");
                return drop(tx.send(failed(Msg::PartyServerUnreachable { server: address })));
            }
        };
        let by = glimpse.playlist.by.clone();
        let (current, next) = (glimpse.playlist.current(), glimpse.playlist.next());
        let (current, next) = tokio::join!(
            async {
                match current {
                    Some(name) => Some(identify(&meta, name, by.as_deref()).await.room),
                    None => None,
                }
            },
            async {
                match next {
                    Some(name) => Some(identify(&meta, name, by.as_deref()).await.room),
                    None => None,
                }
            }
        );
        let members =
            glimpse.members.into_iter().filter(|member| !member.you).map(member).collect();
        let _ = tx.send(Event::PartyGlimpse(PartyGlimpse {
            server: address,
            room,
            members,
            room_episode: current,
            next_episode: next,
        }));
    });
}

/// The last ten rooms, as the front end shows them.
pub fn recent(store: &Store) -> Vec<PartyRecentRoom> {
    store
        .party_rooms()
        .unwrap_or_default()
        .into_iter()
        .map(|kept| PartyRecentRoom {
            server: kept.server,
            room: kept.room,
            id: kept.anilist_id,
            title: kept.title,
            episode: kept.episode,
            names: kept.names,
            last_at: kept.last_at,
            cover_url: kept.cover_url,
        })
        .collect()
}

/// Say the last ten rooms.
pub fn load_recent(services: &Services, tx: &Outbox) {
    let _ = tx.send(Event::PartyRecent(recent(&services.store)));
}

/// Forget a room, and say the rooms left.
pub fn forget_room(server: &str, room: &str, services: &Services, tx: &Outbox) {
    let address = Server::parse(server).map_or_else(|_| server.to_owned(), |s| s.to_string());
    if let Err(e) = services.store.forget_party_room(&address, room) {
        tracing::warn!(error = %e, "could not forget a room");
    }
    load_recent(services, tx);
}

/// What a party's task needs to look things up for the room, and to say what it finds.
#[derive(Clone)]
struct Env {
    meta: Meta,
    store: Store,
    registry: ProviderRegistry,
    prefetch: Prefetch,
    playback: PlaybackConfig,
    tx: Outbox,
    /// Where lookups running beside the task leave their answers.
    internal: mpsc::UnboundedSender<Internal>,
}

/// The two episodes a room is set to: the one it watches and the one after it.
struct Pair {
    files: Vec<String>,
    current: PartyRoomEpisode,
    next: Option<PartyRoomEpisode>,
    media: Option<Media>,
}

/// What a lookup running beside the task found.
enum Internal {
    /// What the room's playlist is, for lookup `seq`.
    Room { seq: u64, found: Option<Identified>, next: Option<PartyRoomEpisode> },
    /// What this device set the room to.
    Picked(Pair),
    /// Whether this device's sources have the room's episode.
    Sources { seq: u64, sources: PartySources },
    /// The title chosen for a plain file.
    Chosen { seq: u64, media: Option<Media> },
    /// The episode after the one that ended, for the room to move on to.
    Advance { seq: u64, pair: Pair },
}

fn episode_of(
    id: AnilistId,
    media: Option<&Media>,
    episode: &str,
    fallback: &str,
) -> PartyRoomEpisode {
    let title = media.map_or_else(|| fallback.to_owned(), |m| m.title.display().to_owned());
    PartyRoomEpisode {
        id: Some(id),
        episode: episode.to_owned(),
        file: file::file_name(&title, id.get(), episode),
        title,
        found: anistream_model::party::PartyFound::Tag,
        plain_by: None,
    }
}

/// `episode` of `id` and the one after it, when there is one: none past the title's last, nor
/// before it airs.
async fn pair(meta: &Meta, id: AnilistId, episode: &str) -> Pair {
    let media = meta.media(id).await.ok();
    let fallback = format!("Title {id}");
    let current = episode_of(id, media.as_ref(), episode, &fallback);
    let next = episode.trim().parse::<u32>().ok().and_then(|n| {
        let after = n.checked_add(1)?;
        let label = after.to_string();
        if let Some(media) = &media
            && (media.episodes.is_some_and(|total| n >= total)
                || not_aired(media, &label).is_some())
        {
            return None;
        }
        Some(episode_of(id, media.as_ref(), &label, &fallback))
    });
    let mut files = vec![current.file.clone()];
    files.extend(next.iter().map(|next| next.file.clone()));
    Pair { files, current, next, media }
}

/// Whether this device's sources have `episode` of `id`. Streams found are kept for the play
/// that follows, so the room starts without a wait.
async fn check_sources(
    env: &Env,
    id: AnilistId,
    episode: &str,
    media: Option<&Media>,
) -> PartySources {
    if let Some(reason) = media.and_then(|media| not_aired(media, episode)) {
        return PartySources::Missing { why: NoSourceWhy::NotAired, reason };
    }
    let found = resolve_for_playback(
        &env.meta,
        &env.store,
        &env.registry,
        id,
        episode,
        None,
        &env.playback,
        None,
    )
    .await;
    match found {
        Ok(resolved) => {
            env.prefetch.keep(id, episode, env.playback.translation, resolved);
            PartySources::Found
        }
        Err(unplayable) => {
            PartySources::Missing { why: unplayable.why, reason: unplayable.text }
        }
    }
}

/// The party's task: the room's events to the front end and the player, the player's reports
/// and the engine's asks to the room, until the party ends.
struct Task {
    party: Party,
    env: Env,
    view: PartyView,
    members: Vec<Member>,
    playlist: Playlist,
    /// What the room is set to is looked up again when this changes; older answers are dropped.
    seq: u64,
    /// What this device is to set the room to once in.
    pick: Option<PartyEpisode>,
    joined: bool,
    /// Where the room's asks of the player go: the playback seated last.
    cues: Option<mpsc::UnboundedSender<SeatCue>>,
    /// The title the room's episode is, and the guesses at a plain file's.
    media: Option<Media>,
    guesses: Vec<PartyGuess>,
    cover: Option<String>,
    /// The episode this device plays for the room.
    playing: Option<(AnilistId, String)>,
    /// The room moved on and plays when everyone has the new episode open.
    moving_on: bool,
    countdown: Option<u32>,
    tick: tokio::time::Interval,
    /// Left for another party, or by an engine shutting down: the end goes unsaid.
    quiet: bool,
}

impl Task {
    fn new(party: Party, env: Env, view: PartyView, pick: Option<PartyEpisode>) -> Self {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        Self {
            party,
            env,
            view,
            members: Vec::new(),
            playlist: Playlist::default(),
            seq: 0,
            pick,
            joined: false,
            cues: None,
            media: None,
            guesses: Vec::new(),
            cover: None,
            playing: None,
            moving_on: false,
            countdown: None,
            tick,
            quiet: false,
        }
    }

    fn cue(&self, cue: SeatCue) {
        if let Some(cues) = &self.cues {
            let _ = cues.send(cue);
        }
    }

    fn say(&self, event: Event) {
        let _ = self.env.tx.send(event);
    }

    /// The members who are there beside this one.
    fn others(&self) -> Vec<String> {
        self.members.iter().filter(|m| !m.you).map(|m| m.name.clone()).collect()
    }

    fn all_ready(&self) -> bool {
        self.members.len() >= 2 && self.members.iter().all(|m| m.ready.unwrap_or(true))
    }

    /// Work out what the view derives from the rest, and say it.
    fn refresh(&mut self) {
        let starter = self.playlist.by.clone();
        let present =
            starter.as_ref().is_some_and(|by| self.members.iter().any(|m| &m.name == by));
        let mine = starter.as_ref().is_some_and(|by| *by == self.view.name);
        self.view.starter = starter;
        self.view.starter_present = present;
        self.view.can_start_anyway = !self.view.playing
            && self.view.room_episode.as_ref().is_some_and(|r| r.id.is_some())
            && (mine || (self.view.starter.is_some() && !present && self.members.len() >= 2));
        self.view.starting_in = self.countdown;
        self.view.moves_on = self.view.next_episode.is_some();
        self.view.guesses = match &self.view.room_episode {
            Some(room) if room.id.is_none() => self.guesses.clone(),
            _ => Vec::new(),
        };
        self.view.code = clean_party_code(&self.view.room).filter(|_| {
            anistream_party::Server::parse(&self.view.server)
                .is_ok_and(|s| s.to_string() == SUGGESTED_SERVER)
        });
        let share = PartyShare {
            server: self.view.server.clone(),
            room: self.view.room.clone(),
            by: Some(self.view.name.clone()),
            title: self.view.room_episode.as_ref().map(|r| r.title.clone()),
            episode: self.view.room_episode.as_ref().map(|r| r.episode.clone()),
        };
        self.view.link = Some(share.https_link());
        if self.joined {
            self.say(Event::PartyState(self.view.clone()));
        }
    }

    /// Start or stop the count, and move a room on that has everyone ready.
    fn evaluate(&mut self) {
        let can = self.joined
            && !self.view.playing
            && self.view.room_episode.as_ref().is_some_and(|r| r.id.is_some())
            && self.all_ready();
        if can && self.countdown.is_none() {
            self.countdown = Some(COUNT);
            self.tick.reset();
        } else if !can {
            self.countdown = None;
        }
        if self.moving_on && self.view.playing && self.all_ready() {
            self.moving_on = false;
            self.party.play();
        }
        self.refresh();
    }

    /// The room plays now (`kind`), or moves on: every app opens its player.
    fn start(&mut self, kind: PartyStartKind) {
        let Some(room) = self.view.room_episode.clone() else { return };
        let Some(id) = room.id else { return };
        self.countdown = None;
        self.view.playing = true;
        self.playing = Some((id, room.episode.clone()));
        match kind {
            PartyStartKind::Start => self.party.play(),
            PartyStartKind::MoveOn => self.moving_on = true,
            PartyStartKind::Rejoin => {}
        }
        self.say(Event::PartyStarted { id, episode: room.episode, kind });
        self.evaluate();
    }

    /// Look the room's playlist up again: the title, the next, and the sources.
    fn look_up(&mut self) {
        self.seq += 1;
        let seq = self.seq;
        let env = self.env.clone();
        let (current, next, by) = (
            self.playlist.current().map(str::to_owned),
            self.playlist.next().map(str::to_owned),
            self.playlist.by.clone(),
        );
        tokio::spawn(async move {
            let Some(current) = current else {
                let _ = env.internal.send(Internal::Room { seq, found: None, next: None });
                return;
            };
            let (found, next) =
                tokio::join!(identify(&env.meta, &current, by.as_deref()), async {
                    match &next {
                        Some(name) => Some(identify(&env.meta, name, by.as_deref()).await.room),
                        None => None,
                    }
                });
            let _ = env.internal.send(Internal::Room { seq, found: Some(found), next });
        });
    }

    fn look_for_sources(&mut self) {
        let Some(room) = self.view.room_episode.clone() else { return };
        let Some(id) = room.id else {
            self.view.sources = None;
            return;
        };
        self.view.sources = Some(PartySources::Checking);
        let (env, seq, media) = (self.env.clone(), self.seq, self.media.clone());
        tokio::spawn(async move {
            let sources = check_sources(&env, id, &room.episode, media.as_ref()).await;
            let _ = env.internal.send(Internal::Sources { seq, sources });
        });
    }

    /// Keep the room among the last ten, and say them.
    fn remember(&self) {
        let Some(room) = &self.view.room_episode else { return };
        let Some(id) = room.id else { return };
        let kept = PartyRoom {
            server: self.view.server.clone(),
            room: self.view.room.clone(),
            anilist_id: id,
            title: room.title.clone(),
            episode: room.episode.clone(),
            names: self.others(),
            cover_url: self.cover.clone(),
            last_at: now_epoch(),
        };
        if let Err(e) = self.env.store.remember_party_room(&kept) {
            tracing::warn!(error = %e, "could not keep the room");
            return;
        }
        self.say(Event::PartyRecent(recent(&self.env.store)));
    }

    fn pick_episode(&self, id: AnilistId, episode: String) {
        let env = self.env.clone();
        tokio::spawn(async move {
            let pair = pair(&env.meta, id, &episode).await;
            let _ = env.internal.send(Internal::Picked(pair));
        });
    }

    fn set_room(
        &mut self,
        room: Option<PartyRoomEpisode>,
        next: Option<PartyRoomEpisode>,
        media: Option<Media>,
    ) {
        self.cover = media.as_ref().and_then(|m| m.cover_image.best().map(str::to_owned));
        self.media = media;
        self.view.room_episode = room;
        self.view.next_episode = next;
        self.view.looking = false;
    }

    fn on_party(&mut self, event: PartyEvent) -> bool {
        match event {
            PartyEvent::Joined { name, room, encrypted, motd } => {
                (self.view.name, self.view.room, self.view.encrypted, self.view.motd) =
                    (name, room, encrypted, motd);
                self.refresh();
                self.joined = true;
                self.say(Event::PartyJoined(self.view.clone()));
                self.say(Event::PartyRecent(recent(&self.env.store)));
                if let Some(pick) = self.pick.take() {
                    self.pick_episode(pick.id, pick.episode);
                }
            }
            PartyEvent::Listed => {
                // Nothing in the playlist by now: the room has nothing open.
                if self.view.looking && self.playlist.current().is_none() && self.pick.is_none()
                {
                    self.view.looking = false;
                    self.refresh();
                }
            }
            PartyEvent::Members(members) => {
                self.members = members;
                self.say(Event::PartyMembers(
                    self.members.iter().cloned().map(member).collect(),
                ));
                self.evaluate();
            }
            PartyEvent::Playlist(playlist) => {
                let changed = playlist.files != self.playlist.files
                    || playlist.index != self.playlist.index;
                self.playlist = playlist;
                if changed {
                    self.look_up();
                }
                self.refresh();
            }
            PartyEvent::Player(action) => self.cue(match action {
                PlayerAction::Pause => SeatCue::Pause(true),
                PlayerAction::Play => SeatCue::Pause(false),
                PlayerAction::Seek(to) => SeatCue::Seek(to),
                PlayerAction::Speed(speed) => SeatCue::Speed(speed),
            }),
            PartyEvent::Notice(notice) => {
                self.cue(SeatCue::Notice(notice.text.clone()));
                let kind = message_kind(notice.kind);
                self.say(Event::PartyMessage(PartyMessage { kind, text: notice.text }));
            }
            PartyEvent::Latency(millis) => {
                self.view.latency_ms = Some(millis);
                if self.joined {
                    self.say(Event::PartyState(self.view.clone()));
                }
            }
            PartyEvent::Reconnecting { attempt, of } => {
                if attempt == 1 {
                    self.cue(SeatCue::Notice(Msg::PartyServerLost.english()));
                }
                self.view.connection = PartyConnection::Reconnecting { attempt, of };
                self.say(Event::PartyState(self.view.clone()));
            }
            PartyEvent::Rejoined { name, encrypted } => {
                self.cue(SeatCue::Notice(Msg::PartyRejoined.english()));
                (self.view.name, self.view.encrypted) = (name, encrypted);
                self.view.connection = PartyConnection::Connected;
                self.say(Event::PartyState(self.view.clone()));
            }
            PartyEvent::Left(reason) => {
                tracing::info!(?reason, "the watch party ended");
                self.remember();
                if !self.quiet {
                    let said = reason.clone().unwrap_or_else(|| Msg::PartyLeft.english());
                    let reason = reason.map(Msg::verbatim);
                    self.cue(SeatCue::Notice(said));
                    self.say(Event::PartyLeft { reason });
                    self.say(Event::PartyRecent(recent(&self.env.store)));
                }
                return true;
            }
        }
        false
    }

    fn on_report(&mut self, report: SeatReport) {
        match report {
            SeatReport::Playing { id, episode, name, duration } => {
                let tagged = file::file_name(&name, id.get(), &episode);
                self.party.file(Some(FileInfo { name: tagged, duration }));
                let share = PartyShare {
                    server: self.view.server.clone(),
                    room: self.view.room.clone(),
                    by: Some(self.view.name.clone()),
                    title: Some(name),
                    episode: Some(episode.clone()),
                };
                self.view.invite = Some(share.https_link());
                let changed = self
                    .view
                    .episode
                    .as_ref()
                    .is_none_or(|e| e.id != id || e.episode != episode);
                self.view.episode = Some(PartyEpisode { id, episode });
                // After a move-on the new file is open: this member is ready to play it.
                if self.moving_on {
                    self.party.ready(true);
                    self.view.ready = true;
                }
                if changed && self.joined {
                    self.say(Event::PartyState(self.view.clone()));
                }
                if self.moving_on {
                    self.evaluate();
                }
            }
            SeatReport::Sample { position, paused, speed } => {
                self.party.sample(Sample { position, paused, speed });
            }
            SeatReport::Ended { leave, finished } => {
                self.party.detach();
                self.party.file(None);
                self.cues = None;
                if leave {
                    self.party.leave();
                } else if finished {
                    self.finished();
                }
            }
        }
    }

    /// The episode played to its end: the room moves on to the next, or says it is the last
    /// that is out.
    fn finished(&mut self) {
        let Some((id, episode)) = self.playing.clone() else { return };
        let current = self
            .view
            .room_episode
            .as_ref()
            .is_some_and(|r| r.id == Some(id) && r.episode == episode);
        if !current {
            return;
        }
        let Some(next) = self.view.next_episode.clone().filter(|n| n.id.is_some()) else {
            self.view.playing = false;
            self.playing = None;
            let after = episode.trim().parse::<u32>().ok().map(|n| (n + 1).to_string());
            let airs =
                self.media.as_ref().zip(after).and_then(|(m, after)| not_aired(m, &after));
            self.say(Event::PartyLastOut { episode, airs });
            self.refresh();
            return;
        };
        let (env, seq) = (self.env.clone(), self.seq);
        tokio::spawn(async move {
            let pair = pair(&env.meta, next.id.unwrap_or(id), &next.episode).await;
            let _ = env.internal.send(Internal::Advance { seq, pair });
        });
    }

    fn on_internal(&mut self, message: Internal) {
        match message {
            Internal::Room { seq, found, next } if seq == self.seq => {
                let (room, guesses, media) = match found {
                    Some(found) => (Some(found.room), found.guesses, found.media),
                    None => (None, Vec::new(), None),
                };
                self.guesses = guesses;
                self.set_room(room, next, media);
                self.remember();
                let target = self
                    .view
                    .room_episode
                    .as_ref()
                    .and_then(|r| r.id.map(|id| (id, r.episode.clone())));
                match (target, self.playing.clone()) {
                    // Somebody moved the room on: so does this device.
                    (Some(target), Some(playing)) if self.view.playing && playing != target => {
                        self.start(PartyStartKind::MoveOn);
                        self.look_for_sources();
                    }
                    (Some(_), _) if !self.view.playing => self.look_for_sources(),
                    _ => {}
                }
                self.evaluate();
            }
            Internal::Picked(pair) => {
                self.seq += 1;
                let name = self.view.name.clone();
                self.party.set_playlist(pair.files.clone(), 0);
                self.playlist = Playlist { files: pair.files, index: Some(0), by: Some(name) };
                self.guesses.clear();
                self.view.playing = false;
                self.playing = None;
                self.moving_on = false;
                self.countdown = None;
                self.set_room(Some(pair.current), pair.next, pair.media);
                self.remember();
                self.look_for_sources();
                self.evaluate();
            }
            Internal::Sources { seq, sources } if seq == self.seq => {
                self.view.sources = Some(sources);
                self.refresh();
            }
            Internal::Chosen { seq, media } if seq == self.seq => {
                if let (Some(media), Some(room)) = (media, self.view.room_episode.as_mut()) {
                    room.id = Some(media.id);
                    room.title = media.title.display().to_owned();
                    room.found = anistream_model::party::PartyFound::FileName;
                    self.cover = media.cover_image.best().map(str::to_owned);
                    self.media = Some(media);
                    self.remember();
                    self.look_for_sources();
                    self.evaluate();
                }
            }
            Internal::Advance { seq, pair } if seq == self.seq => {
                let Some(index) = self.playlist.index else { return };
                let mut files: Vec<String> =
                    self.playlist.files.iter().take(index + 2).cloned().collect();
                if let Some(next) = &pair.next {
                    files.truncate(index + 2);
                    files.push(next.file.clone());
                }
                self.party.move_on(files.clone(), index + 1);
                self.playlist =
                    Playlist { files, index: Some(index + 1), by: self.playlist.by.clone() };
                self.seq += 1;
                self.guesses.clear();
                let room = PartyRoomEpisode {
                    found: self
                        .view
                        .room_episode
                        .as_ref()
                        .map_or(pair.current.found, |r| r.found),
                    ..pair.current
                };
                let next = pair.next;
                self.set_room(Some(room), next, pair.media);
                self.remember();
                self.start(PartyStartKind::MoveOn);
                self.look_for_sources();
            }
            // Answers to a lookup the room has since moved past.
            Internal::Room { .. }
            | Internal::Sources { .. }
            | Internal::Chosen { .. }
            | Internal::Advance { .. } => {}
        }
    }

    fn on_control(&mut self, control: Control) {
        match control {
            Control::Ready(ready) => {
                self.party.ready(ready);
                self.view.ready = ready;
                self.evaluate();
            }
            Control::Change(id, episode) => self.pick_episode(id, episode),
            Control::Choose(Some(id)) => {
                let (env, seq) = (self.env.clone(), self.seq);
                tokio::spawn(async move {
                    let media = env.meta.media(id).await.ok();
                    let _ = env.internal.send(Internal::Chosen { seq, media });
                });
            }
            Control::Choose(None) => {
                if let Some(room) = self.view.room_episode.as_mut() {
                    room.id = None;
                    room.found = anistream_model::party::PartyFound::Unmatched;
                }
                self.view.sources = None;
                self.evaluate();
            }
            Control::StartAnyway => {
                if self.view.can_start_anyway {
                    self.party.ready(true);
                    self.view.ready = true;
                    self.start(PartyStartKind::Start);
                }
            }
            Control::OpenPlayer => self.start(PartyStartKind::Rejoin),
            Control::Leave => self.party.leave(),
            Control::Switch => {
                self.quiet = true;
                self.party.leave();
            }
        }
    }

    async fn run(
        mut self,
        mut events: mpsc::UnboundedReceiver<PartyEvent>,
        mut seats: mpsc::UnboundedReceiver<SeatMessage>,
        mut controls: mpsc::UnboundedReceiver<Control>,
        mut internals: mpsc::UnboundedReceiver<Internal>,
    ) {
        let mut controls_open = true;
        let mut seats_open = true;
        loop {
            tokio::select! {
                event = events.recv() => {
                    let Some(event) = event else { break };
                    if self.on_party(event) {
                        return;
                    }
                }
                message = seats.recv(), if seats_open => match message {
                    // No playback can take a seat any more: the hub was opened for another
                    // party, or the engine is gone.
                    None => {
                        seats_open = false;
                        self.quiet = true;
                        self.party.leave();
                    }
                    Some(SeatMessage::Seated(seat)) => {
                        self.cues = Some(seat);
                        self.party.attach();
                    }
                    Some(SeatMessage::Report(report)) => self.on_report(report),
                },
                control = controls.recv(), if controls_open => match control {
                    Some(control) => self.on_control(control),
                    None => {
                        controls_open = false;
                        self.party.leave();
                    }
                },
                Some(message) = internals.recv() => self.on_internal(message),
                _ = self.tick.tick(), if self.countdown.is_some() => {
                    match self.countdown {
                        Some(left) if left > 1 => {
                            self.countdown = Some(left - 1);
                            self.refresh();
                        }
                        Some(_) => self.start(PartyStartKind::Start),
                        None => {}
                    }
                }
            }
        }
        if !self.quiet {
            self.say(Event::PartyLeft { reason: None });
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_terminal_is_told_to_set_the_server_in_the_config_file() {
        assert!(matches!(needs_server(false), Msg::PartyNeedsServer { .. }));
        let app = needs_server(true);
        assert!(matches!(app, Msg::PartyChooseServer { .. }), "{app:?}");
        assert!(!app.english().contains("syncplay.server"), "{}", app.english());
    }
}
