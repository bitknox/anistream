//! A member of a room on a Syncplay server: the connection, the members, and one player kept with
//! the room.
//!
//! [`Party::join`] connects and runs on its own task. The front end reports its player as it
//! plays, and hears back through [`PartyEvent`]s: who is in the room, what the player is to do,
//! and the notices to show. A connection that drops is made again a few times, and the player is
//! brought back to the room; a server that refuses the party, or one that cannot be reached in
//! the first place, ends it with a message that says why.

use std::{collections::BTreeMap, fmt, io, sync::Arc, time::Duration};

use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
    sync::mpsc,
    time::{Instant, sleep_until, timeout},
};
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};

use crate::{
    notice::Notice,
    ping::Latency,
    sync::{Change, Decision, PlayerAction, RoomState, Sample, Sync},
    wire::{self, FileInfo, MAX_LINE, OutState, ServerMessage, SetItem, UserEvent},
};

/// The port a Syncplay server listens on when none is given.
pub const DEFAULT_PORT: u16 = 8999;

/// How long a server has to take the connection, and to answer the first messages.
const ANSWER: Duration = Duration::from_secs(10);

/// A Syncplay server, as `host:port`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

/// A server written in a way that can't be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Write the server as a host and a port, like syncplay.example:8999")]
pub struct BadServer;

impl Server {
    /// Read `host:port`, or a bare host on the default port. An IPv6 address takes brackets
    /// when it has a port: `[::1]:8999`.
    pub fn parse(text: &str) -> Result<Self, BadServer> {
        let text = text.trim();
        let (host, port) = if let Some(rest) = text.strip_prefix('[') {
            let (host, after) = rest.split_once(']').ok_or(BadServer)?;
            match after {
                "" => (host, None),
                _ => (host, Some(after.strip_prefix(':').ok_or(BadServer)?)),
            }
        } else {
            match text.matches(':').count() {
                0 => (text, None),
                1 => text.split_once(':').map(|(h, p)| (h, Some(p))).ok_or(BadServer)?,
                // A bare IPv6 address.
                _ => (text, None),
            }
        };
        let host_ok = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '_'));
        if !host_ok {
            return Err(BadServer);
        }
        let port = match port {
            None => DEFAULT_PORT,
            Some(port) => port.parse::<u16>().ok().filter(|p| *p != 0).ok_or(BadServer)?,
        };
        Ok(Self { host: host.to_ascii_lowercase(), port })
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.host.contains(':') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// Whether to ask a server to continue over TLS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tls {
    /// Ask, and continue in the clear when the server has no TLS. A server that says it has TLS
    /// and then fails the handshake is not used in the clear.
    Prefer,
    /// Never ask.
    Off,
}

/// How a lost connection is made again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retry {
    /// How many tries before the party ends.
    pub attempts: u32,
    /// The wait before the first try; each one after waits twice as long.
    pub first: Duration,
    /// The longest wait between two tries.
    pub longest: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self { attempts: 6, first: Duration::from_secs(1), longest: Duration::from_secs(15) }
    }
}

impl Retry {
    fn delay(&self, attempt: u32) -> Duration {
        let doubled = self.first.saturating_mul(1u32 << attempt.saturating_sub(1).min(16));
        doubled.min(self.longest)
    }
}

/// What a party is joined with.
#[derive(Debug, Clone)]
pub struct PartyOptions {
    pub server: Server,
    pub room: String,
    /// The name this member asks for. A server gives another when it is taken.
    pub name: String,
    pub tls: Tls,
    pub retry: Retry,
    /// How long the server can say nothing before the connection counts as lost. Servers send
    /// the room's state every second or so.
    pub silence: Duration,
}

impl PartyOptions {
    pub fn new(server: Server, room: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            server,
            room: room.into(),
            name: name.into(),
            tls: Tls::Prefer,
            retry: Retry::default(),
            silence: Duration::from_secs(20),
        }
    }
}

/// A member of the room, as the front end shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: String,
    /// `None` for a client that has no ready state.
    pub ready: Option<bool>,
    pub file: Option<FileInfo>,
    /// Whether their file looks like this member's: the same name, and lengths within a few
    /// seconds of each other. `None` when either has none open.
    pub file_matches: Option<bool>,
    /// This member.
    pub you: bool,
}

/// What a party tells the front end.
#[derive(Debug, Clone, PartialEq)]
pub enum PartyEvent {
    /// In the room. `name` is the one the server gave, which may differ from the one asked for.
    Joined { name: String, room: String, encrypted: bool, motd: Option<String> },
    /// Who is in the room now, this member first.
    Members(Vec<Member>),
    /// Something for the player to do.
    Player(PlayerAction),
    /// Something to show on the player.
    Notice(Notice),
    /// The smoothed round trip to the server, in milliseconds, when it moves by more than a
    /// little.
    Latency(u32),
    /// The connection was lost, and is being made again: this is try `attempt` of `of`.
    Reconnecting { attempt: u32, of: u32 },
    /// Back in the room after a lost connection.
    Rejoined { name: String, encrypted: bool },
    /// The party is over: left on purpose with `None`, or for the reason given.
    Left(Option<String>),
}

enum Input {
    Sample(Sample),
    Ready(bool),
    File(Option<FileInfo>),
    Attach,
    Detach,
    Leave,
}

/// A party being joined or in. Dropping it leaves.
#[derive(Debug, Clone)]
pub struct Party {
    inputs: mpsc::UnboundedSender<Input>,
}

impl Party {
    /// Join the room, on a task of the current Tokio runtime. Events come on the receiver,
    /// starting with [`PartyEvent::Joined`] or, when the server can't be reached or refuses,
    /// [`PartyEvent::Left`] with why.
    pub fn join(options: PartyOptions) -> (Self, mpsc::UnboundedReceiver<PartyEvent>) {
        let (inputs, input_rx) = mpsc::unbounded_channel();
        let (events, event_rx) = mpsc::unbounded_channel();
        tokio::spawn(run(options, input_rx, events));
        (Self { inputs }, event_rx)
    }

    /// What the player reports of itself.
    pub fn sample(&self, sample: Sample) {
        let _ = self.inputs.send(Input::Sample(sample));
    }

    /// Say whether this member is ready.
    pub fn ready(&self, ready: bool) {
        let _ = self.inputs.send(Input::Ready(ready));
    }

    /// Say which file is open, or that none is.
    pub fn file(&self, file: Option<FileInfo>) {
        let _ = self.inputs.send(Input::File(file));
    }

    /// A player is playing for this member: what it reports from now on is kept with the room.
    pub fn attach(&self) {
        let _ = self.inputs.send(Input::Attach);
    }

    /// The player is gone; the member stays in the room.
    pub fn detach(&self) {
        let _ = self.inputs.send(Input::Detach);
    }

    pub fn leave(&self) {
        let _ = self.inputs.send(Input::Leave);
    }

    /// Whether the party is still going.
    pub fn is_open(&self) -> bool {
        !self.inputs.is_closed()
    }
}

/// How one connection ended.
enum End {
    /// The member left.
    Left,
    /// The server refused the party; trying again would not help.
    Refused(String),
    /// The connection was lost, or never made.
    Lost(String),
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

async fn run(
    options: PartyOptions,
    mut inputs: mpsc::UnboundedReceiver<Input>,
    events: mpsc::UnboundedSender<PartyEvent>,
) {
    let mut member = Membership::new(options, events);
    let mut attempt = 0;
    loop {
        let (end, joined) = member.connection(&mut inputs).await;
        let reason = match end {
            End::Left => return member.emit(PartyEvent::Left(None)),
            End::Refused(reason) => return member.emit(PartyEvent::Left(Some(reason))),
            End::Lost(reason) => reason,
        };
        tracing::info!(server = %member.options.server, %reason, "party connection lost");
        if !member.joined_once {
            return member.emit(PartyEvent::Left(Some(reason)));
        }
        if joined {
            attempt = 0;
        }
        attempt += 1;
        let retry = member.options.retry;
        if attempt > retry.attempts {
            let server = &member.options.server;
            let reason = format!("Lost the connection to {server}, and could not get it back");
            return member.emit(PartyEvent::Left(Some(reason)));
        }
        member.emit(PartyEvent::Reconnecting { attempt, of: retry.attempts });
        let deadline = Instant::now() + retry.delay(attempt);
        loop {
            tokio::select! {
                () = sleep_until(deadline) => break,
                input = inputs.recv() => match input {
                    None | Some(Input::Leave) => return member.emit(PartyEvent::Left(None)),
                    Some(input) => member.offline(input),
                },
            }
        }
    }
}

/// A member's side of the party, across connections.
struct Membership {
    options: PartyOptions,
    events: mpsc::UnboundedSender<PartyEvent>,
    started: Instant,
    /// Whether any connection reached the room: until one has, failing ends the party.
    joined_once: bool,
    /// The server answered the request for TLS with something other than an answer: it is
    /// spoken to in the clear from then on.
    plain_only: bool,

    // What this member says of itself, kept across connections.
    ready: bool,
    file: Option<FileInfo>,
    sync: Sync,
    latency: Latency,
    shown_latency: Option<u32>,

    // This connection.
    me: String,
    room: String,
    members: BTreeMap<String, Seen>,
    /// The members of the room are known: the room's state is not acted on until they are, as
    /// who else is watching decides whose position wins.
    listed: bool,
    /// Every member said they are ready, as last seen.
    all_ready: bool,
    /// Changes this member made that the server has not taken in yet.
    ignoring_client: u32,
    /// Changes the server made that this member has to acknowledge, once.
    ignoring_server: u32,
    /// A change made while another was still on its way, sent once that one is taken in.
    held: Option<Change>,
    outbox: Vec<Value>,
}

#[derive(Debug, Clone, Default)]
struct Seen {
    ready: Option<bool>,
    file: Option<FileInfo>,
}

impl Membership {
    fn new(options: PartyOptions, events: mpsc::UnboundedSender<PartyEvent>) -> Self {
        Self {
            plain_only: options.tls == Tls::Off,
            me: options.name.clone(),
            room: options.room.clone(),
            options,
            events,
            started: Instant::now(),
            joined_once: false,
            ready: false,
            file: None,
            sync: Sync::default(),
            latency: Latency::default(),
            shown_latency: None,
            members: BTreeMap::new(),
            listed: false,
            all_ready: false,
            ignoring_client: 0,
            ignoring_server: 0,
            held: None,
            outbox: Vec::new(),
        }
    }

    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn emit(&self, event: PartyEvent) {
        let _ = self.events.send(event);
    }

    /// One connection, from the socket to its end. Returns how it ended, and whether it reached
    /// the room.
    async fn connection(&mut self, inputs: &mut mpsc::UnboundedReceiver<Input>) -> (End, bool) {
        let (stream, encrypted) = match self.open().await {
            Ok(opened) => opened,
            Err(end) => return (end, false),
        };
        let (read, mut write) = tokio::io::split(stream);
        let mut reader = BufReader::new(read);
        let mut partial = Vec::new();

        let hello = wire::hello(&self.options.name, &self.options.room);
        if let Err(error) = send(&mut write, &[hello]).await {
            return (self.lost(&error), false);
        }
        let server = self.options.server.clone();
        let answered = timeout(ANSWER, async {
            loop {
                match next_line(&mut reader, &mut partial).await {
                    Ok(Some(line)) => {
                        for message in wire::parse(&line).unwrap_or_default() {
                            match message {
                                ServerMessage::Hello(hello) => return Ok(hello),
                                ServerMessage::Error(reason) => {
                                    let reason = crate::notice::clean(&reason);
                                    return Err(End::Refused(format!(
                                        "{server} refused the party: {reason}"
                                    )));
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(None) => {
                        return Err(End::Lost(format!("{server} closed the connection")));
                    }
                    Err(error) => return Err(End::Lost(format!("Lost {server}: {error}"))),
                }
            }
        })
        .await;
        let hello = match answered {
            Ok(Ok(hello)) => hello,
            Ok(Err(end)) => return (end, false),
            Err(_) => return (End::Lost(format!("{server} did not answer")), false),
        };

        self.me =
            if hello.username.is_empty() { self.options.name.clone() } else { hello.username };
        self.room = hello.room.unwrap_or_else(|| self.options.room.clone());
        self.members.clear();
        self.listed = false;
        self.all_ready = false;
        self.ignoring_client = 0;
        self.ignoring_server = 0;
        self.held = None;
        self.outbox.clear();
        if self.joined_once {
            self.sync.rejoin();
            self.emit(PartyEvent::Rejoined { name: self.me.clone(), encrypted });
        } else {
            self.joined_once = true;
            self.emit(PartyEvent::Joined {
                name: self.me.clone(),
                room: self.room.clone(),
                encrypted,
                motd: hello.motd.map(|m| crate::notice::clean(&m)),
            });
        }
        self.members
            .insert(self.me.clone(), Seen { ready: Some(self.ready), file: self.file.clone() });
        if self.file.is_some() {
            self.outbox.push(wire::set_file(self.file.as_ref()));
        }
        if self.ready {
            self.outbox.push(wire::set_ready(true, true));
        }
        self.outbox.push(wire::list());

        let mut heard = Instant::now();
        loop {
            let end = tokio::select! {
                line = next_line(&mut reader, &mut partial) => match line {
                    Ok(Some(line)) => {
                        heard = Instant::now();
                        self.on_line(&line)
                    }
                    Ok(None) => Some(End::Lost(format!("Lost the connection to {server}"))),
                    Err(error) => Some(self.lost(&error)),
                },
                input = inputs.recv() => match input {
                    None | Some(Input::Leave) => Some(End::Left),
                    Some(input) => {
                        self.input(input);
                        None
                    }
                },
                () = sleep_until(heard + self.options.silence) => {
                    Some(End::Lost(format!("{server} stopped answering")))
                }
            };
            let outbox = std::mem::take(&mut self.outbox);
            if !outbox.is_empty()
                && let Err(error) = send(&mut write, &outbox).await
            {
                return (self.lost(&error), true);
            }
            if let Some(end) = end {
                if matches!(end, End::Left) {
                    let _ = write.shutdown().await;
                }
                return (end, true);
            }
        }
    }

    /// The socket, over TLS when the server takes it.
    async fn open(&mut self) -> Result<(Box<dyn Stream>, bool), End> {
        let server = self.options.server.clone();
        loop {
            let address = (server.host.as_str(), server.port);
            let socket = match timeout(ANSWER, TcpStream::connect(address)).await {
                Ok(Ok(socket)) => socket,
                Ok(Err(error)) => {
                    return Err(End::Lost(format!("Could not reach {server}: {error}")));
                }
                Err(_) => {
                    return Err(End::Lost(format!("Could not reach {server}: no answer")));
                }
            };
            let _ = socket.set_nodelay(true);
            if self.plain_only {
                return Ok((Box::new(socket), false));
            }
            let mut socket = socket;
            let asked = async {
                socket.write_all(wire::line(&wire::start_tls()).as_bytes()).await?;
                read_raw_line(&mut socket).await
            };
            let answer = match timeout(ANSWER, asked).await {
                Ok(Ok(Some(line))) => wire::parse(&line).ok().and_then(|messages| {
                    messages.into_iter().find_map(|m| match m {
                        ServerMessage::Tls { accepted } => Some(accepted),
                        _ => None,
                    })
                }),
                _ => None,
            };
            match answer {
                Some(true) => {
                    let name = ServerName::try_from(server.host.clone()).map_err(|_| {
                        End::Lost(format!("Could not make a secure connection to {server}"))
                    })?;
                    let connected =
                        timeout(ANSWER, TlsConnector::from(tls_config()).connect(name, socket));
                    return match connected.await {
                        Ok(Ok(tls)) => Ok((Box::new(tls), true)),
                        Ok(Err(error)) => Err(End::Lost(format!(
                            "Could not make a secure connection to {server}: {error}"
                        ))),
                        Err(_) => Err(End::Lost(format!(
                            "Could not make a secure connection to {server}: no answer"
                        ))),
                    };
                }
                Some(false) => return Ok((Box::new(socket), false)),
                // A server from before TLS, which does not know the question and may have hung
                // up on it: asked again in the clear.
                None => {
                    tracing::debug!(%server, "no TLS answer; speaking in the clear");
                    self.plain_only = true;
                }
            }
        }
    }

    fn lost(&self, error: &io::Error) -> End {
        End::Lost(format!("Lost the connection to {}: {error}", self.options.server))
    }

    /// One line from the server. Returns how the connection ends, when the line ends it.
    fn on_line(&mut self, line: &str) -> Option<End> {
        let messages = match wire::parse(line) {
            Ok(messages) => messages,
            Err(error) => {
                tracing::debug!(%error, "unreadable party line");
                return None;
            }
        };
        for message in messages {
            match message {
                ServerMessage::State(state) => self.on_state(state),
                ServerMessage::Set(items) => self.on_set(items),
                ServerMessage::List(mut rooms) => {
                    let listed = rooms.remove(&self.room).unwrap_or_default();
                    self.members = listed
                        .into_iter()
                        .map(|(name, listed)| {
                            (name, Seen { ready: listed.ready, file: listed.file })
                        })
                        .collect();
                    self.members.insert(
                        self.me.clone(),
                        Seen { ready: Some(self.ready), file: self.file.clone() },
                    );
                    self.listed = true;
                    // A list is how things stand, not someone getting ready: after a lost
                    // connection, a room that was all ready and paused stays paused.
                    self.members_changed(false);
                }
                ServerMessage::Chat { from, message } => {
                    if !message.trim().is_empty() {
                        self.emit(PartyEvent::Notice(Notice::chat(&from, &message)));
                    }
                }
                ServerMessage::Error(reason) => {
                    let server = &self.options.server;
                    let reason = crate::notice::clean(&reason);
                    return Some(End::Refused(format!("{server} ended the party: {reason}")));
                }
                ServerMessage::Hello(_)
                | ServerMessage::Tls { .. }
                | ServerMessage::Other(_) => {}
            }
        }
        None
    }

    fn on_state(&mut self, state: wire::State) {
        let now = self.now();
        if let Some(sent) = state.ping.client_latency_calculation {
            self.latency.receive(sent, state.ping.server_rtt.unwrap_or(0.0), now);
            self.show_latency();
        }
        match (state.ignoring_server, state.ignoring_client) {
            (Some(n), _) if n > 0 => {
                // The server made a change everyone has to take: this member's own changes on
                // their way are overtaken by it.
                self.ignoring_server = n;
                self.ignoring_client = 0;
                self.held = None;
            }
            (_, Some(n)) if n == self.ignoring_client => {
                self.ignoring_client = 0;
                if let Some(change) = self.held.take() {
                    self.send_change(change);
                }
            }
            _ => {}
        }
        if let Some(play) = state.playstate
            && self.listed
            && self.ignoring_client == 0
        {
            let room = RoomState {
                position: play.position,
                paused: play.paused,
                do_seek: play.do_seek,
                set_by: play.set_by,
            };
            let age = self.latency.forward_delay();
            let others = self.others_watching();
            let (change, decisions) = self.sync.room(room, age, &self.me, others, now);
            self.decide(decisions);
            if let Some(change) = change {
                self.send_change(change);
            }
        }
        let playstate = if self.ignoring_client == 0 || self.ignoring_server != 0 {
            self.sync.here(now)
        } else {
            None
        };
        let reply = OutState {
            playstate,
            seek: false,
            latency_calculation: state.ping.latency_calculation,
            client_time: now,
            client_rtt: self.latency.rtt(),
            ignoring_server: (self.ignoring_server != 0).then_some(self.ignoring_server),
            ignoring_client: (self.ignoring_client != 0).then_some(self.ignoring_client),
        };
        self.ignoring_server = 0;
        self.outbox.push(reply.to_json());
    }

    fn on_set(&mut self, items: Vec<SetItem>) {
        let mut changed = false;
        for item in items {
            match item {
                SetItem::Ready { username, ready, .. } => {
                    if username == self.me {
                        // The server's word for this member: ready is lost on a file change.
                        if let Some(ready) = ready {
                            self.ready = ready;
                        }
                    }
                    self.members.entry(username).or_default().ready = ready;
                    changed = true;
                }
                SetItem::User { name, room, file, event } => {
                    let elsewhere = room.as_deref().is_some_and(|r| r != self.room);
                    if name == self.me {
                        continue;
                    }
                    if event == Some(UserEvent::Left) || elsewhere {
                        if self.members.remove(&name).is_some() {
                            self.emit(PartyEvent::Notice(Notice::left(&name)));
                        }
                    } else {
                        let joined = !self.members.contains_key(&name);
                        let seen = self.members.entry(name.clone()).or_default();
                        if file.is_some() {
                            seen.file = file;
                        }
                        if joined && self.listed {
                            self.emit(PartyEvent::Notice(Notice::joined(&name)));
                            // Their ready state and file, as the server has them.
                            self.outbox.push(wire::list());
                        }
                    }
                    changed = true;
                }
                SetItem::Room(room) => {
                    self.room = room;
                    self.outbox.push(wire::list());
                }
                SetItem::Other(_) => {}
            }
        }
        if changed {
            self.members_changed(true);
        }
    }

    fn input(&mut self, input: Input) {
        let now = self.now();
        match input {
            Input::Sample(sample) => {
                let others = self.others_watching();
                let (change, decisions) = self.sync.sample(sample, others, now);
                self.decide(decisions);
                if let Some(change) = change {
                    self.send_change(change);
                }
            }
            Input::Ready(ready) => {
                self.ready = ready;
                self.outbox.push(wire::set_ready(ready, true));
                self.members.entry(self.me.clone()).or_default().ready = Some(ready);
                self.members_changed(true);
            }
            Input::File(file) => {
                self.outbox.push(wire::set_file(file.as_ref()));
                self.file = file.clone();
                self.members.entry(self.me.clone()).or_default().file = file;
                self.members_changed(false);
            }
            Input::Attach => self.sync.attach(),
            Input::Detach => self.sync.detach(),
            Input::Leave => {}
        }
    }

    /// What comes in while there is no connection: kept, to be said on the next one.
    fn offline(&mut self, input: Input) {
        let now = self.now();
        match input {
            // Kept with the player's state; the next connection meets the room afresh.
            Input::Sample(sample) => {
                let _ = self.sync.sample(sample, false, now);
            }
            Input::Ready(ready) => self.ready = ready,
            Input::File(file) => self.file = file,
            Input::Attach => self.sync.attach(),
            Input::Detach => self.sync.detach(),
            Input::Leave => {}
        }
    }

    fn send_change(&mut self, change: Change) {
        if self.ignoring_client != 0 {
            self.held = Some(change);
            return;
        }
        self.ignoring_client += 1;
        let state = OutState {
            playstate: Some((change.position, change.paused)),
            seek: change.seek,
            latency_calculation: None,
            client_time: self.now(),
            client_rtt: self.latency.rtt(),
            ignoring_server: (self.ignoring_server != 0).then_some(self.ignoring_server),
            ignoring_client: Some(self.ignoring_client),
        };
        self.ignoring_server = 0;
        self.outbox.push(state.to_json());
    }

    fn decide(&self, decisions: Vec<Decision>) {
        for decision in decisions {
            self.emit(match decision {
                Decision::Player(action) => PartyEvent::Player(action),
                Decision::Notice(notice) => PartyEvent::Notice(notice),
            });
        }
    }

    fn others_watching(&self) -> bool {
        self.members.iter().any(|(name, seen)| *name != self.me && seen.file.is_some())
    }

    /// Tell the front end who is in the room. `autoplay` when the change is someone getting
    /// ready, or leaving the room to those who are.
    fn members_changed(&mut self, autoplay: bool) {
        let mine = self.file.clone();
        let mut members: Vec<Member> = self
            .members
            .iter()
            .map(|(name, seen)| {
                let you = *name == self.me;
                let file_matches = match (&mine, &seen.file) {
                    (Some(a), Some(b)) if !you => Some(same_file(a, b)),
                    _ => None,
                };
                Member {
                    name: name.clone(),
                    ready: if you { Some(self.ready) } else { seen.ready },
                    file: if you { mine.clone() } else { seen.file.clone() },
                    file_matches,
                    you,
                }
            })
            .collect();
        members.sort_by_key(|m| !m.you);
        self.emit(PartyEvent::Members(members));
        self.autoplay(autoplay);
    }

    /// Everyone in the room ready, two or more, and newly so: play starts.
    fn autoplay(&mut self, allowed: bool) {
        let all_ready = self.members.len() >= 2
            && self.members.iter().all(|(name, seen)| {
                if *name == self.me { self.ready } else { seen.ready == Some(true) }
            });
        let rising = all_ready && !self.all_ready;
        self.all_ready = all_ready;
        if rising
            && allowed
            && let Some((change, decision)) = self.sync.play_now(self.now())
        {
            self.emit(PartyEvent::Notice(Notice::all_ready()));
            self.decide(vec![decision]);
            self.send_change(change);
        }
    }

    fn show_latency(&mut self) {
        let Some(millis) = self.latency.millis() else { return };
        let moved = self.shown_latency.is_none_or(|shown| shown.abs_diff(millis) >= 20);
        if moved {
            self.shown_latency = Some(millis);
            self.emit(PartyEvent::Latency(millis));
        }
    }
}

/// Two files are the same episode when their names match and their lengths are within a few
/// seconds; a length not known yet doesn't count against them.
fn same_file(a: &FileInfo, b: &FileInfo) -> bool {
    let lengths = match (a.duration, b.duration) {
        (Some(x), Some(y)) => (x - y).abs() <= 5.0,
        _ => true,
    };
    a.name == b.name && lengths
}

async fn send<W: AsyncWrite + Unpin>(write: &mut W, messages: &[Value]) -> io::Result<()> {
    let mut bytes = String::new();
    for message in messages {
        bytes.push_str(&wire::line(message));
    }
    write.write_all(bytes.as_bytes()).await?;
    write.flush().await
}

/// The next line from the server, without its end. Safe to cancel: what was read of a line is
/// kept in `partial` for the next call. `None` when the server hung up.
pub(crate) async fn next_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    partial: &mut Vec<u8>,
) -> io::Result<Option<String>> {
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(None);
        }
        let (taken, done) = match available.iter().position(|b| *b == b'\n') {
            Some(at) => (at + 1, true),
            None => (available.len(), false),
        };
        partial.extend_from_slice(&available[..taken]);
        reader.consume(taken);
        if partial.len() > MAX_LINE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the server sent too long a line",
            ));
        }
        if done {
            let line = String::from_utf8_lossy(partial).trim_end().to_owned();
            partial.clear();
            return Ok(Some(line));
        }
    }
}

/// One short line straight from the socket, a byte at a time, so nothing past it is read: the
/// answer to the TLS question, after which the socket may carry TLS.
async fn read_raw_line(socket: &mut TcpStream) -> io::Result<Option<String>> {
    let mut line = Vec::new();
    loop {
        let mut byte = [0u8; 1];
        if socket.read(&mut byte).await? == 0 {
            return Ok(None);
        }
        if byte[0] == b'\n' {
            return Ok(Some(String::from_utf8_lossy(&line).trim_end().to_owned()));
        }
        line.push(byte[0]);
        if line.len() > 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the server sent too long a line",
            ));
        }
    }
}

/// Servers are checked against the web's public roots, as a browser would.
fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: std::sync::OnceLock<Arc<ClientConfig>> = std::sync::OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots =
                RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("ring supports the default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_read_with_and_without_a_port() {
        assert_eq!(
            Server::parse("syncplay.example:8995"),
            Ok(Server { host: "syncplay.example".into(), port: 8995 })
        );
        assert_eq!(Server::parse(" Party.Example ").map(|s| s.port), Ok(DEFAULT_PORT));
        assert_eq!(Server::parse("party.example").unwrap().host, "party.example");
        assert_eq!(Server::parse("[::1]:9000"), Ok(Server { host: "::1".into(), port: 9000 }));
        assert_eq!(Server::parse("::1").map(|s| s.port), Ok(DEFAULT_PORT));
        assert_eq!(Server::parse("[::1]:9000").unwrap().to_string(), "[::1]:9000");
        assert_eq!(
            Server::parse("party.example:8999").unwrap().to_string(),
            "party.example:8999"
        );
        for bad in [
            "",
            ":8999",
            "party.example:",
            "party.example:0",
            "party.example:99999",
            "a b:1",
            "https://party.example",
        ] {
            assert_eq!(Server::parse(bad), Err(BadServer), "{bad}");
        }
    }

    #[test]
    fn retries_wait_longer_each_time_up_to_a_limit() {
        let retry = Retry::default();
        assert_eq!(retry.delay(1), Duration::from_secs(1));
        assert_eq!(retry.delay(2), Duration::from_secs(2));
        assert_eq!(retry.delay(4), Duration::from_secs(8));
        assert_eq!(retry.delay(9), Duration::from_secs(15));
    }

    #[test]
    fn files_match_by_name_and_close_lengths() {
        let file = |name: &str, duration| FileInfo { name: name.into(), duration };
        assert!(same_file(&file("Episode 3", Some(1440.0)), &file("Episode 3", Some(1442.0))));
        assert!(same_file(&file("Episode 3", None), &file("Episode 3", Some(1442.0))));
        assert!(!same_file(&file("Episode 3", Some(1440.0)), &file("Episode 3", Some(1300.0))));
        assert!(!same_file(&file("Episode 3", None), &file("Episode 4", None)));
    }
}
