//! A Syncplay server on the loopback interface, for tests and the apps' Fixtures builds: rooms,
//! members who connect, and members it plays itself, who join, get ready, pause, seek, fall
//! behind and chat when told to. Never in an app's release build.
//!
//! It keeps to what the client relies on, not to everything a real server does. The room's
//! position is its own clock, moved by changes; a member it plays who falls behind holds the
//! room back by as much, as a real server's room follows its slowest member. Positions that
//! members who connect report outside a change are recorded, not followed.

use std::{
    collections::BTreeMap,
    net::SocketAddr,
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use serde_json::{Map, Value, json};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::Instant,
};

use crate::{
    client::next_line,
    wire::{self, FileInfo},
};

/// How a [`FakeServer`] behaves.
#[derive(Debug, Clone)]
pub struct FakeOptions {
    /// How often the room's state is sent to every member who connected.
    pub tick: Duration,
    pub motd: Option<String>,
    /// Refuse every member, with this reason.
    pub refuse: Option<String>,
    pub tls: FakeTls,
}

impl Default for FakeOptions {
    fn default() -> Self {
        Self {
            tick: Duration::from_millis(100),
            motd: None,
            refuse: None,
            tls: FakeTls::Decline,
        }
    }
}

/// How a [`FakeServer`] answers a request for TLS. It has none to give either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeTls {
    /// Says it has none, as a current server without a certificate does.
    Decline,
    /// Hangs up on the question, as a server from before TLS may.
    Unknown,
}

/// A room as the server has it, for a test to look at.
#[derive(Debug, Clone, PartialEq)]
pub struct FakeRoom {
    pub position: f64,
    pub paused: bool,
    pub set_by: Option<String>,
    pub members: Vec<FakeOccupant>,
    /// The room's shared playlist, and the entry it is at.
    pub playlist: Vec<String>,
    pub index: Option<usize>,
    /// Who last wrote the playlist or its index.
    pub playlist_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FakeOccupant {
    pub name: String,
    pub ready: Option<bool>,
    pub file: Option<FileInfo>,
    /// The last position they reported outside a change; `None` for a member the server plays.
    pub reported: Option<f64>,
}

/// See the module's documentation. Stops when dropped.
pub struct FakeServer {
    shared: Arc<Shared>,
    address: SocketAddr,
    tasks: Vec<JoinHandle<()>>,
}

struct Shared {
    options: FakeOptions,
    started: Instant,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    rooms: BTreeMap<String, Room>,
    conns: BTreeMap<u64, Conn>,
    next_conn: u64,
    /// Connections are taken and closed at once, as by a server that is down.
    down: bool,
}

struct Room {
    position: f64,
    paused: bool,
    set_by: Option<String>,
    /// When `position` was the room's position, by the server's clock.
    at: f64,
    members: BTreeMap<String, Occupant>,
    playlist: Vec<String>,
    index: Option<usize>,
    playlist_by: Option<String>,
}

#[derive(Default)]
struct Occupant {
    file: Option<FileInfo>,
    ready: Option<bool>,
    /// How far behind the room a member the server plays is.
    behind: f64,
    reported: Option<f64>,
}

struct Conn {
    name: String,
    room: String,
    lines: mpsc::UnboundedSender<String>,
    /// Changes the server forced on this member that it has not acknowledged.
    server_ignoring: u32,
    /// A change of this member's taken in, to say so in the next state.
    client_ack: Option<u32>,
    /// The member's timestamp, to echo in the next state.
    echo: Option<f64>,
    kill: Option<oneshot::Sender<()>>,
}

impl Room {
    fn new(now: f64) -> Self {
        Self {
            position: 0.0,
            paused: true,
            set_by: None,
            at: now,
            members: BTreeMap::new(),
            playlist: Vec::new(),
            index: None,
            playlist_by: None,
        }
    }

    fn position(&self, now: f64) -> f64 {
        let played = if self.paused { self.position } else { self.position + (now - self.at) };
        let behind = self.members.values().map(|m| m.behind).fold(0.0, f64::max);
        (played - behind).max(0.0)
    }

    fn set(&mut self, position: f64, paused: bool, by: &str, now: f64) {
        let behind = self.members.values().map(|m| m.behind).fold(0.0, f64::max);
        self.position = position + behind;
        self.paused = paused;
        self.set_by = Some(by.to_owned());
        self.at = now;
    }
}

impl FakeServer {
    pub async fn start(options: FakeOptions) -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let address = listener.local_addr()?;
        let tick = options.tick;
        let shared = Arc::new(Shared {
            options,
            started: Instant::now(),
            inner: Mutex::new(Inner::default()),
        });
        let accepting = {
            let shared = shared.clone();
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    if shared.lock().down {
                        drop(socket);
                        continue;
                    }
                    tokio::spawn(serve(shared.clone(), socket));
                }
            })
        };
        let ticking = {
            let shared = shared.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tick);
                loop {
                    interval.tick().await;
                    let now = shared.now();
                    let mut inner = shared.lock();
                    let ids: Vec<u64> = inner.conns.keys().copied().collect();
                    for id in ids {
                        inner.send_state(id, now, false, false);
                    }
                }
            })
        };
        Ok(Self { shared, address, tasks: vec![accepting, ticking] })
    }

    /// Where to connect, as `host:port`.
    pub fn address(&self) -> String {
        self.address.to_string()
    }

    /// A member the server plays, joined to `room` now.
    pub fn member(&self, room: &str, name: &str) -> FakeMember {
        let now = self.shared.now();
        let mut inner = self.shared.lock();
        let occupants =
            &mut inner.rooms.entry(room.to_owned()).or_insert_with(|| Room::new(now)).members;
        occupants.insert(name.to_owned(), Occupant::default());
        inner.broadcast(room, None, &user(name, room, Some(json!({ "joined": {} })), None));
        FakeMember { shared: self.shared.clone(), room: room.to_owned(), name: name.to_owned() }
    }

    /// How a room stands, when anyone is in it.
    pub fn room(&self, room: &str) -> Option<FakeRoom> {
        let now = self.shared.now();
        let inner = self.shared.lock();
        let found = inner.rooms.get(room)?;
        Some(FakeRoom {
            position: found.position(now),
            paused: found.paused,
            set_by: found.set_by.clone(),
            members: found
                .members
                .iter()
                .map(|(name, m)| FakeOccupant {
                    name: name.clone(),
                    ready: m.ready,
                    file: m.file.clone(),
                    reported: m.reported,
                })
                .collect(),
            playlist: found.playlist.clone(),
            index: found.index,
            playlist_by: found.playlist_by.clone(),
        })
    }

    /// Close every connection, as a server restarting does.
    pub fn drop_connections(&self) {
        let mut inner = self.shared.lock();
        for conn in inner.conns.values_mut() {
            if let Some(kill) = conn.kill.take() {
                let _ = kill.send(());
            }
        }
    }

    /// Take no connections while `down`.
    pub fn set_down(&self, down: bool) {
        self.shared.lock().down = down;
    }

    /// Send a member who connected an error, which ends their party.
    pub fn kick(&self, name: &str, reason: &str) {
        let inner = self.shared.lock();
        for conn in inner.conns.values().filter(|c| c.name == name) {
            let _ = conn.lines.send(wire::line(&json!({ "Error": { "message": reason } })));
        }
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
        self.drop_connections();
    }
}

/// A member the server plays. See [`FakeServer::member`].
#[derive(Clone)]
pub struct FakeMember {
    shared: Arc<Shared>,
    room: String,
    name: String,
}

impl FakeMember {
    /// Open a file.
    pub fn open(&self, name: &str, duration: f64) {
        let file = FileInfo { name: name.to_owned(), duration: Some(duration) };
        self.with(|occupant| occupant.file = Some(file.clone()));
        let message = user(&self.name, &self.room, None, Some(&file));
        self.shared.lock().broadcast(&self.room, None, &message);
    }

    pub fn ready(&self, ready: bool) {
        self.with(|occupant| occupant.ready = Some(ready));
        let message = json!({
            "Set": { "ready": { "username": self.name, "isReady": ready, "manuallyInitiated": true } }
        });
        self.shared.lock().broadcast(&self.room, None, &message);
    }

    pub fn pause(&self) {
        self.change(|position, _| (position, true), false);
    }

    pub fn play(&self) {
        self.change(|position, _| (position, false), false);
    }

    pub fn seek(&self, to: f64) {
        self.change(|_, paused| (to, paused), true);
    }

    /// Fall `seconds` behind the room, which then follows: its position drops by as much.
    pub fn fall_behind(&self, seconds: f64) {
        self.with(|occupant| occupant.behind = seconds.max(0.0));
    }

    /// Replace the room's shared playlist, as a member's client does.
    pub fn set_playlist(&self, files: &[&str]) {
        let mut inner = self.shared.lock();
        let Some(room) = inner.rooms.get_mut(&self.room) else { return };
        room.playlist = files.iter().map(|f| (*f).to_owned()).collect();
        room.playlist_by = Some(self.name.clone());
        let message = playlist_json(&self.name, room);
        inner.broadcast(&self.room, None, &message);
    }

    /// Move the room to an entry of its playlist.
    pub fn set_index(&self, index: usize) {
        let mut inner = self.shared.lock();
        let Some(room) = inner.rooms.get_mut(&self.room) else { return };
        room.index = Some(index);
        room.playlist_by = Some(self.name.clone());
        let message = index_json(&self.name, room);
        inner.broadcast(&self.room, None, &message);
    }

    pub fn chat(&self, message: &str) {
        let message = json!({ "Chat": { "username": self.name, "message": message } });
        self.shared.lock().broadcast(&self.room, None, &message);
    }

    pub fn leave(&self) {
        let mut inner = self.shared.lock();
        if let Some(room) = inner.rooms.get_mut(&self.room) {
            room.members.remove(&self.name);
        }
        let message = user(&self.name, &self.room, Some(json!({ "left": true })), None);
        inner.broadcast(&self.room, None, &message);
    }

    fn with(&self, apply: impl FnOnce(&mut Occupant)) {
        let mut inner = self.shared.lock();
        if let Some(occupant) =
            inner.rooms.get_mut(&self.room).and_then(|room| room.members.get_mut(&self.name))
        {
            apply(occupant);
        }
    }

    fn change(&self, to: impl FnOnce(f64, bool) -> (f64, bool), seek: bool) {
        let now = self.shared.now();
        let mut inner = self.shared.lock();
        let Some(room) = inner.rooms.get_mut(&self.room) else { return };
        let (position, paused) = to(room.position(now), room.paused);
        room.set(position, paused, &self.name, now);
        inner.force(&self.room, now, seek);
    }
}

impl Shared {
    fn now(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Inner {
    /// Send `message` to every member of `room` who connected, but `except` one.
    fn broadcast(&self, room: &str, except: Option<u64>, message: &Value) {
        let line = wire::line(message);
        for (id, conn) in &self.conns {
            if conn.room == room && Some(*id) != except {
                let _ = conn.lines.send(line.clone());
            }
        }
    }

    /// The room changed: every member who connected takes its new state.
    fn force(&mut self, room: &str, now: f64, seek: bool) {
        let ids: Vec<u64> =
            self.conns.iter().filter(|(_, c)| c.room == room).map(|(id, _)| *id).collect();
        for id in ids {
            self.send_state(id, now, true, seek);
        }
    }

    fn send_state(&mut self, id: u64, now: f64, forced: bool, seek: bool) {
        let Some(conn) = self.conns.get(&id) else { return };
        let Some(room) = self.rooms.get(&conn.room) else { return };
        let playstate = json!({
            "position": room.position(now),
            "paused": room.paused,
            "doSeek": seek,
            "setBy": room.set_by.clone().map_or(Value::Null, Value::String),
        });
        let conn = self.conns.get_mut(&id).expect("looked up above");
        let mut ping = json!({ "latencyCalculation": now, "serverRtt": 0.0 });
        if let Some(echo) = conn.echo.take() {
            ping["clientLatencyCalculation"] = json!(echo);
        }
        let mut state = Map::new();
        state.insert("playstate".into(), playstate);
        state.insert("ping".into(), ping);
        let mut ignoring = Map::new();
        if forced {
            conn.server_ignoring += 1;
            ignoring.insert("server".into(), json!(conn.server_ignoring));
        }
        if let Some(n) = conn.client_ack.take() {
            ignoring.insert("client".into(), json!(n));
        }
        if !ignoring.is_empty() {
            state.insert("ignoringOnTheFly".into(), Value::Object(ignoring));
        }
        let _ = conn.lines.send(wire::line(&json!({ "State": state })));
    }

    fn list(&self) -> Value {
        let rooms: Map<String, Value> = self
            .rooms
            .iter()
            .map(|(name, room)| {
                let members: Map<String, Value> = room
                    .members
                    .iter()
                    .map(|(name, m)| {
                        let member = json!({
                            "position": m.reported.unwrap_or(0.0),
                            "file": file_json(m.file.as_ref()),
                            "isReady": m.ready,
                        });
                        (name.clone(), member)
                    })
                    .collect();
                (name.clone(), Value::Object(members))
            })
            .collect();
        json!({ "List": rooms })
    }
}

fn playlist_json(by: &str, room: &Room) -> Value {
    json!({ "Set": { "playlistChange": { "user": by, "files": room.playlist } } })
}

fn index_json(by: &str, room: &Room) -> Value {
    json!({ "Set": { "playlistIndex": { "user": by, "index": room.index } } })
}

fn file_json(file: Option<&FileInfo>) -> Value {
    match file {
        Some(file) => {
            json!({ "name": file.name, "duration": file.duration.unwrap_or(0.0), "size": 0 })
        }
        None => json!({}),
    }
}

fn user(name: &str, room: &str, event: Option<Value>, file: Option<&FileInfo>) -> Value {
    let mut about = json!({ "room": { "name": room } });
    if let Some(event) = event {
        about["event"] = event;
    }
    if let Some(file) = file {
        about["file"] = file_json(Some(file));
    }
    json!({ "Set": { "user": { name: about } } })
}

/// One member who connected, from the socket to its end.
async fn serve(shared: Arc<Shared>, socket: TcpStream) {
    let _ = socket.set_nodelay(true);
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let mut partial = Vec::new();
    let (lines, mut outgoing) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(line) = outgoing.recv().await {
            if write.write_all(line.as_bytes()).await.is_err() {
                break;
            }
        }
        let _ = write.shutdown().await;
    });
    let (kill, mut killed) = oneshot::channel();
    let mut kill = Some(kill);
    let mut id = None;

    loop {
        let line = tokio::select! {
            line = next_line(&mut reader, &mut partial) => line,
            _ = &mut killed => break,
        };
        let Ok(Some(line)) = line else { break };
        let Ok(Value::Object(messages)) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let mut close = false;
        for (key, body) in messages {
            match key.as_str() {
                "TLS" => match shared.options.tls {
                    FakeTls::Decline => {
                        let _ =
                            lines.send(wire::line(&json!({ "TLS": { "startTLS": "false" } })));
                    }
                    FakeTls::Unknown => close = true,
                },
                "Hello" => {
                    if let Some(reason) = &shared.options.refuse {
                        let _ =
                            lines.send(wire::line(&json!({ "Error": { "message": reason } })));
                        close = true;
                        continue;
                    }
                    id = Some(hello(&shared, &body, &lines, kill.take()));
                }
                _ => {
                    if let Some(id) = id {
                        message(&shared, id, &key, &body);
                    }
                }
            }
        }
        if close {
            break;
        }
    }

    if let Some(id) = id {
        let mut inner = shared.lock();
        if let Some(conn) = inner.conns.remove(&id) {
            if let Some(room) = inner.rooms.get_mut(&conn.room) {
                room.members.remove(&conn.name);
            }
            let left = user(&conn.name, &conn.room, Some(json!({ "left": true })), None);
            inner.broadcast(&conn.room, None, &left);
        }
    }
    drop(lines);
    let _ = writer.await;
}

fn hello(
    shared: &Shared,
    body: &Value,
    lines: &mpsc::UnboundedSender<String>,
    kill: Option<oneshot::Sender<()>>,
) -> u64 {
    let asked = body.get("username").and_then(Value::as_str).unwrap_or("guest");
    let room = body
        .get("room")
        .and_then(|r| r.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("default")
        .to_owned();
    let now = shared.now();
    let mut inner = shared.lock();
    let id = inner.next_conn;
    inner.next_conn += 1;
    let occupants =
        &mut inner.rooms.entry(room.clone()).or_insert_with(|| Room::new(now)).members;
    let mut name = asked.to_owned();
    while occupants.contains_key(&name) {
        name.push('_');
    }
    occupants.insert(name.clone(), Occupant::default());
    let welcome = json!({
        "Hello": {
            "username": name,
            "room": { "name": room },
            "version": wire::PROTOCOL_VERSION,
            "realversion": wire::REAL_VERSION,
            "motd": shared.options.motd.clone().unwrap_or_default(),
            "features": { "readiness": true, "chat": true },
        }
    });
    let _ = lines.send(wire::line(&welcome));
    if let Some(found) = inner.rooms.get(&room).filter(|r| !r.playlist.is_empty()) {
        let by = found.playlist_by.clone().unwrap_or_default();
        let _ = lines.send(wire::line(&playlist_json(&by, found)));
        let _ = lines.send(wire::line(&index_json(&by, found)));
    }
    inner.broadcast(&room, None, &user(&name, &room, Some(json!({ "joined": {} })), None));
    inner.conns.insert(
        id,
        Conn {
            name,
            room,
            lines: lines.clone(),
            server_ignoring: 0,
            client_ack: None,
            echo: None,
            kill,
        },
    );
    id
}

fn message(shared: &Shared, id: u64, key: &str, body: &Value) {
    let now = shared.now();
    let mut inner = shared.lock();
    let Some(conn) = inner.conns.get(&id) else { return };
    let (name, room) = (conn.name.clone(), conn.room.clone());
    match key {
        "Set" => {
            if let Some(file) = body.get("file") {
                let name_of = file.get("name").and_then(Value::as_str).unwrap_or_default();
                let file = (!name_of.is_empty()).then(|| FileInfo {
                    name: name_of.to_owned(),
                    duration: file.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0),
                });
                if let Some(occupant) =
                    inner.rooms.get_mut(&room).and_then(|r| r.members.get_mut(&name))
                {
                    occupant.file = file.clone();
                }
                let message = user(&name, &room, None, file.as_ref());
                inner.broadcast(&room, Some(id), &message);
            }
            if let Some(files) = body.get("playlistChange").and_then(|p| p.get("files")) {
                let files: Vec<String> = files
                    .as_array()
                    .map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default();
                if let Some(found) = inner.rooms.get_mut(&room) {
                    found.playlist = files;
                    found.playlist_by = Some(name.clone());
                    let message = playlist_json(&name, found);
                    inner.broadcast(&room, None, &message);
                }
            }
            if let Some(index) = body.get("playlistIndex").and_then(|p| p.get("index"))
                && let Some(found) = inner.rooms.get_mut(&room)
            {
                found.index = index.as_u64().map(|i| i as usize);
                found.playlist_by = Some(name.clone());
                let message = index_json(&name, found);
                inner.broadcast(&room, None, &message);
            }
            if let Some(ready) = body.get("ready") {
                let is_ready = ready.get("isReady").and_then(Value::as_bool);
                let manual =
                    ready.get("manuallyInitiated").and_then(Value::as_bool).unwrap_or(false);
                if let Some(occupant) =
                    inner.rooms.get_mut(&room).and_then(|r| r.members.get_mut(&name))
                {
                    occupant.ready = is_ready;
                }
                let message = json!({
                    "Set": { "ready": { "username": name, "isReady": is_ready, "manuallyInitiated": manual } }
                });
                inner.broadcast(&room, None, &message);
            }
        }
        "List" => {
            let list = inner.list();
            if let Some(conn) = inner.conns.get(&id) {
                let _ = conn.lines.send(wire::line(&list));
            }
        }
        "Chat" => {
            let text = body.as_str().or_else(|| body.get("message").and_then(Value::as_str));
            if let Some(text) = text {
                let message = json!({ "Chat": { "username": name, "message": text } });
                inner.broadcast(&room, None, &message);
            }
        }
        "State" => {
            let conn = inner.conns.get_mut(&id).expect("looked up above");
            if let Some(stamp) =
                body.get("ping").and_then(|p| p.get("clientLatencyCalculation"))
            {
                conn.echo = stamp.as_f64();
            }
            let ignoring = body.get("ignoringOnTheFly");
            if let Some(n) = ignoring.and_then(|i| i.get("server")).and_then(Value::as_u64)
                && n as u32 == conn.server_ignoring
            {
                conn.server_ignoring = 0;
            }
            let client = ignoring.and_then(|i| i.get("client")).and_then(Value::as_u64);
            if let Some(n) = client {
                conn.client_ack = Some(n as u32);
            }
            let listening = conn.server_ignoring == 0;
            let Some(play) = body.get("playstate") else { return };
            let (Some(position), Some(paused)) = (
                play.get("position").and_then(Value::as_f64),
                play.get("paused").and_then(Value::as_bool),
            ) else {
                return;
            };
            if !listening {
                return;
            }
            let seek = play.get("doSeek").and_then(Value::as_bool).unwrap_or(false);
            let Some(found) = inner.rooms.get_mut(&room) else { return };
            if seek || paused != found.paused {
                found.set(position, paused, &name, now);
                inner.force(&room, now, seek);
            } else if let Some(occupant) = found.members.get_mut(&name) {
                occupant.reported = Some(position);
            }
        }
        _ => {}
    }
}
