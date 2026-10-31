//! Syncplay's messages: JSON objects, one per line, each key a message of its own.
//!
//! What a client sends is built here as JSON, and what a server sends is read leniently: servers
//! differ by release in which fields they include, so a field that is missing or of another type
//! reads as unknown rather than failing the whole line. A line may carry several messages, as
//! `{"Set": {...}, "State": {...}}`. They are read in a fixed order, as a JSON object's keys have
//! none: who is in the room before where the room is, and an error last.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

/// The protocol version a client says it speaks. Every server since Syncplay 1.2 takes it.
pub const PROTOCOL_VERSION: &str = "1.2.255";

/// The release a client says it is. Servers decide by it which messages a client understands:
/// readiness from 1.3 and chat from 1.5. This is a release that has both and no shared
/// playlists, which a party uses for the episode the room watches and the one after it.
pub const REAL_VERSION: &str = "1.7.3";

/// The longest line read from a server. Real messages are well under a kilobyte; the room list
/// of a busy server is the longest, at a few hundred bytes a member.
pub const MAX_LINE: usize = 256 * 1024;

/// The end of every line a client sends. Servers split on it, and a bare newline is not one.
pub const LINE_END: &str = "\r\n";

/// A file as a room sees it: its name and length. Its size is never sent (see
/// `docs/party-protocol.md`).
#[derive(Debug, Clone, PartialEq)]
pub struct FileInfo {
    pub name: String,
    /// In seconds; `None` until the player knows it.
    pub duration: Option<f64>,
}

/// One message a client sends, as the line to write.
pub fn line(message: &Value) -> String {
    let mut line = message.to_string();
    line.push_str(LINE_END);
    line
}

/// Ask to continue over TLS, before anything else is said.
pub fn start_tls() -> Value {
    json!({ "TLS": { "startTLS": "send" } })
}

/// Say who this is and which room it joins.
pub fn hello(name: &str, room: &str) -> Value {
    json!({
        "Hello": {
            "username": name,
            "room": { "name": room },
            "version": PROTOCOL_VERSION,
            "realversion": REAL_VERSION,
            "features": {
                "sharedPlaylists": true,
                "chat": true,
                "featureList": true,
                "readiness": true,
                "managedRooms": false,
                "persistentRooms": false,
                "uiMode": "GUI",
            },
        }
    })
}

/// Say which file is open, or that none is. A file with no length yet is sent with nought, as
/// servers expect a number.
pub fn set_file(file: Option<&FileInfo>) -> Value {
    let file = match file {
        Some(file) => json!({
            "name": file.name,
            "duration": file.duration.unwrap_or(0.0),
            "size": 0,
        }),
        None => json!({ "name": "", "duration": 0, "size": 0 }),
    };
    json!({ "Set": { "file": file } })
}

/// Say whether this member is ready, and whether they said so themselves.
pub fn set_ready(ready: bool, manual: bool) -> Value {
    json!({ "Set": { "ready": { "isReady": ready, "manuallyInitiated": manual } } })
}

/// Replace the room's shared playlist with `files`, in this form: a name each.
pub fn set_playlist(files: &[String]) -> Value {
    json!({ "Set": { "playlistChange": { "files": files } } })
}

/// Make the entry at `index` the one the room watches.
pub fn set_playlist_index(index: usize) -> Value {
    json!({ "Set": { "playlistIndex": { "index": index } } })
}

/// Ask for the members of the room.
pub fn list() -> Value {
    json!({ "List": null })
}

/// Where this member's player is, as a reply to the server's state or as a change of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct OutState {
    /// Position and pause, when there is a player to say them of.
    pub playstate: Option<(f64, bool)>,
    /// The position was moved on purpose, rather than played to.
    pub seek: bool,
    /// The server's timestamp, echoed so it can measure its round trip.
    pub latency_calculation: Option<f64>,
    /// This client's clock as it sends, which the server echoes back.
    pub client_time: f64,
    /// The round trip this client last measured.
    pub client_rtt: f64,
    /// The server's count of changes it waits to have acknowledged, echoed once.
    pub ignoring_server: Option<u32>,
    /// This client's count of changes it waits for the server to take in.
    pub ignoring_client: Option<u32>,
}

impl OutState {
    pub fn to_json(&self) -> Value {
        let mut state = Map::new();
        if let Some((position, paused)) = self.playstate {
            let mut playstate = json!({ "position": position, "paused": paused });
            if self.seek {
                playstate["doSeek"] = Value::Bool(true);
            }
            state.insert("playstate".into(), playstate);
        }
        let mut ping = json!({
            "clientLatencyCalculation": self.client_time,
            "clientRtt": self.client_rtt,
        });
        if let Some(stamp) = self.latency_calculation {
            ping["latencyCalculation"] = json!(stamp);
        }
        state.insert("ping".into(), ping);
        if self.ignoring_server.is_some() || self.ignoring_client.is_some() {
            let mut ignoring = Map::new();
            if let Some(n) = self.ignoring_server {
                ignoring.insert("server".into(), json!(n));
            }
            if let Some(n) = self.ignoring_client {
                ignoring.insert("client".into(), json!(n));
            }
            state.insert("ignoringOnTheFly".into(), Value::Object(ignoring));
        }
        json!({ "State": state })
    }
}

/// What a server said.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerMessage {
    /// The answer to [`start_tls`]: whether the server continues over TLS.
    Tls {
        accepted: bool,
    },
    Hello(Hello),
    Set(Vec<SetItem>),
    State(State),
    /// Who is in which room, as rooms of members.
    List(BTreeMap<String, BTreeMap<String, Listed>>),
    Chat {
        from: String,
        message: String,
    },
    /// The server refuses something, and closes the connection after it.
    Error(String),
    /// A message this client does not take part in, by its key.
    Other(String),
}

/// The server's welcome.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hello {
    /// The name the server gave this member, which may differ from the one asked for when it
    /// was taken.
    pub username: String,
    pub room: Option<String>,
    pub version: Option<String>,
    /// The server's message of the day, when it has one.
    pub motd: Option<String>,
}

/// One part of a `Set`.
#[derive(Debug, Clone, PartialEq)]
pub enum SetItem {
    /// A member said whether they are ready; `None` for a client that has no ready state.
    Ready { username: String, ready: Option<bool>, manual: bool },
    /// A member joined, left, moved room or opened a file.
    User {
        name: String,
        room: Option<String>,
        file: Option<FileInfo>,
        event: Option<UserEvent>,
    },
    /// This member is now in this room.
    Room(String),
    /// The room's shared playlist was replaced, by `user` when the server says who.
    Playlist { user: Option<String>, files: Vec<String> },
    /// The room moved to this entry of the playlist; `None` when it has none.
    PlaylistIndex { user: Option<String>, index: Option<usize> },
    /// A part this client does not take part in.
    Other(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserEvent {
    Joined,
    Left,
}

/// The room's state, and the server's timestamps.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub playstate: Option<PlayState>,
    pub ping: Ping,
    /// The server waits for this client to acknowledge this many changes it made.
    pub ignoring_server: Option<u32>,
    /// The server has taken in this many of this client's changes.
    pub ignoring_client: Option<u32>,
}

/// Where the room is.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayState {
    pub position: f64,
    pub paused: bool,
    /// Someone moved the position on purpose.
    pub do_seek: bool,
    /// Who changed it last, when anyone has.
    pub set_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ping {
    /// The server's timestamp, for this client to echo.
    pub latency_calculation: Option<f64>,
    /// This client's own timestamp, echoed by the server.
    pub client_latency_calculation: Option<f64>,
    /// The server's round trip to this client.
    pub server_rtt: Option<f64>,
}

/// One member in a room list.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Listed {
    pub position: Option<f64>,
    pub file: Option<FileInfo>,
    pub ready: Option<bool>,
}

/// Why a line could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("the server sent something that is not JSON")]
    NotJson,
    #[error("the server sent a message that is not an object")]
    NotAnObject,
}

/// Read one line from a server: every message on it, in the order they are best taken in. An
/// empty line reads as none.
pub fn parse(line: &str) -> Result<Vec<ServerMessage>, WireError> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(line).map_err(|_| WireError::NotJson)?;
    let Value::Object(map) = value else { return Err(WireError::NotAnObject) };
    let mut messages: Vec<ServerMessage> =
        map.into_iter().map(|(key, body)| message(&key, &body)).collect();
    messages.sort_by_key(rank);
    Ok(messages)
}

fn rank(message: &ServerMessage) -> u8 {
    match message {
        ServerMessage::Tls { .. } => 0,
        ServerMessage::Hello(_) => 1,
        ServerMessage::Set(_) => 2,
        ServerMessage::List(_) => 3,
        ServerMessage::State(_) => 4,
        ServerMessage::Chat { .. } => 5,
        ServerMessage::Other(_) => 6,
        ServerMessage::Error(_) => 7,
    }
}

fn message(key: &str, body: &Value) -> ServerMessage {
    match key {
        "TLS" => {
            let answer = body.get("startTLS");
            let accepted = match answer {
                Some(Value::String(s)) => s == "true",
                Some(Value::Bool(b)) => *b,
                _ => false,
            };
            ServerMessage::Tls { accepted }
        }
        "Hello" => ServerMessage::Hello(Hello {
            username: string(body.get("username")).unwrap_or_default(),
            room: body.get("room").and_then(|r| string(r.get("name"))),
            version: string(body.get("realversion")).or_else(|| string(body.get("version"))),
            motd: string(body.get("motd")).filter(|m| !m.trim().is_empty()),
        }),
        "Set" => ServerMessage::Set(set_items(body)),
        "State" => ServerMessage::State(state(body)),
        "List" => ServerMessage::List(list_of(body)),
        "Chat" => ServerMessage::Chat {
            from: string(body.get("username")).unwrap_or_default(),
            message: string(body.get("message")).unwrap_or_default(),
        },
        "Error" => {
            let message = string(body.get("message"))
                .or_else(|| string(Some(body)))
                .unwrap_or_else(|| "no reason given".into());
            ServerMessage::Error(message)
        }
        other => ServerMessage::Other(other.to_owned()),
    }
}

fn set_items(body: &Value) -> Vec<SetItem> {
    let Some(map) = body.as_object() else { return Vec::new() };
    let mut items = Vec::new();
    for (key, value) in map {
        match key.as_str() {
            "ready" => items.push(SetItem::Ready {
                username: string(value.get("username")).unwrap_or_default(),
                ready: value.get("isReady").and_then(Value::as_bool),
                manual: value
                    .get("manuallyInitiated")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }),
            "user" => {
                let Some(users) = value.as_object() else { continue };
                for (name, user) in users {
                    let event = user.get("event").and_then(|event| {
                        if event.get("joined").is_some() {
                            Some(UserEvent::Joined)
                        } else if event.get("left").is_some() {
                            Some(UserEvent::Left)
                        } else {
                            None
                        }
                    });
                    items.push(SetItem::User {
                        name: name.clone(),
                        room: user.get("room").and_then(|r| string(r.get("name"))),
                        file: user.get("file").and_then(file_info),
                        event,
                    });
                }
            }
            "room" => {
                if let Some(name) = string(value.get("name")) {
                    items.push(SetItem::Room(name));
                }
            }
            "playlistChange" => items.push(SetItem::Playlist {
                user: string(value.get("user")).filter(|u| !u.is_empty()),
                files: value
                    .get("files")
                    .and_then(Value::as_array)
                    .map(|files| files.iter().filter_map(|f| string(Some(f))).collect())
                    .unwrap_or_default(),
            }),
            "playlistIndex" => items.push(SetItem::PlaylistIndex {
                user: string(value.get("user")).filter(|u| !u.is_empty()),
                index: value.get("index").and_then(Value::as_u64).map(|i| i as usize),
            }),
            other => items.push(SetItem::Other(other.to_owned())),
        }
    }
    items
}

fn state(body: &Value) -> State {
    let playstate = body.get("playstate").and_then(|p| {
        Some(PlayState {
            position: number(p.get("position"))?,
            paused: p.get("paused").and_then(Value::as_bool)?,
            do_seek: p.get("doSeek").and_then(Value::as_bool).unwrap_or(false),
            set_by: string(p.get("setBy")).filter(|s| !s.is_empty()),
        })
    });
    let ping = body
        .get("ping")
        .map(|p| Ping {
            latency_calculation: number(p.get("latencyCalculation")),
            client_latency_calculation: number(p.get("clientLatencyCalculation")),
            server_rtt: number(p.get("serverRtt")),
        })
        .unwrap_or_default();
    let ignoring = body.get("ignoringOnTheFly");
    let count = |side: &str| {
        ignoring.and_then(|i| i.get(side)).and_then(Value::as_u64).map(|n| n as u32)
    };
    State {
        playstate,
        ping,
        ignoring_server: count("server"),
        ignoring_client: count("client"),
    }
}

fn list_of(body: &Value) -> BTreeMap<String, BTreeMap<String, Listed>> {
    let mut rooms = BTreeMap::new();
    let Some(map) = body.as_object() else { return rooms };
    for (room, members) in map {
        let Some(members) = members.as_object() else { continue };
        let listed = members
            .iter()
            .map(|(name, member)| {
                let listed = Listed {
                    position: number(member.get("position")),
                    file: member.get("file").and_then(file_info),
                    ready: member.get("isReady").and_then(Value::as_bool),
                };
                (name.clone(), listed)
            })
            .collect();
        rooms.insert(room.clone(), listed);
    }
    rooms
}

/// A file as a server describes it; `None` for no file, which servers say with an empty object
/// or an empty name.
fn file_info(value: &Value) -> Option<FileInfo> {
    let name = string(value.get("name")).filter(|n| !n.is_empty())?;
    let duration = number(value.get("duration")).filter(|d| *d > 0.0);
    Some(FileInfo { name, duration })
}

fn string(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

fn number(value: Option<&Value>) -> Option<f64> {
    value?.as_f64().filter(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_ends_as_servers_split_lines() {
        let sent = line(&list());
        assert_eq!(sent, "{\"List\":null}\r\n");
    }

    #[test]
    fn hello_names_the_room_and_the_releases_servers_decide_by() {
        let hello = hello("Mika", "quiet-harbor");
        assert_eq!(hello["Hello"]["username"], "Mika");
        assert_eq!(hello["Hello"]["room"]["name"], "quiet-harbor");
        assert_eq!(hello["Hello"]["version"], PROTOCOL_VERSION);
        assert_eq!(hello["Hello"]["realversion"], REAL_VERSION);
        assert_eq!(hello["Hello"]["features"]["readiness"], true);
        assert_eq!(hello["Hello"]["features"]["sharedPlaylists"], true);
    }

    #[test]
    fn a_file_is_sent_with_its_name_and_length_and_never_its_size() {
        let file =
            FileInfo { name: "Starlit Harbor · Episode 3".into(), duration: Some(1420.5) };
        let set = set_file(Some(&file));
        assert_eq!(set["Set"]["file"]["name"], "Starlit Harbor · Episode 3");
        assert_eq!(set["Set"]["file"]["duration"], 1420.5);
        assert_eq!(set["Set"]["file"]["size"], 0);
    }

    #[test]
    fn a_state_reply_carries_only_what_it_has() {
        let reply = OutState {
            playstate: None,
            seek: false,
            latency_calculation: Some(100.5),
            client_time: 200.25,
            client_rtt: 0.04,
            ignoring_server: None,
            ignoring_client: None,
        }
        .to_json();
        assert!(reply["State"].get("playstate").is_none(), "no player, no position");
        assert_eq!(reply["State"]["ping"]["latencyCalculation"], 100.5);
        assert!(reply["State"].get("ignoringOnTheFly").is_none());

        let change = OutState {
            playstate: Some((61.0, true)),
            seek: true,
            latency_calculation: None,
            client_time: 1.0,
            client_rtt: 0.0,
            ignoring_server: Some(1),
            ignoring_client: Some(2),
        }
        .to_json();
        assert_eq!(change["State"]["playstate"]["position"], 61.0);
        assert_eq!(change["State"]["playstate"]["paused"], true);
        assert_eq!(change["State"]["playstate"]["doSeek"], true);
        assert_eq!(change["State"]["ignoringOnTheFly"]["server"], 1);
        assert_eq!(change["State"]["ignoringOnTheFly"]["client"], 2);
    }

    #[test]
    fn every_message_on_a_line_is_read_in_a_fixed_order() {
        let read = parse(
            r#"{"State": {"playstate": {"position": 12.5, "paused": false, "doSeek": true, "setBy": "Mika"}, "ping": {"latencyCalculation": 5.5, "serverRtt": 0.02, "clientLatencyCalculation": 4.0}, "ignoringOnTheFly": {"server": 1}}, "Chat": {"username": "Mika", "message": "hi"}}"#,
        )
        .unwrap();
        assert_eq!(read.len(), 2);
        let ServerMessage::State(state) = &read[0] else { panic!("{read:?}") };
        let playstate = state.playstate.as_ref().unwrap();
        assert_eq!(playstate.position, 12.5);
        assert!(playstate.do_seek);
        assert_eq!(playstate.set_by.as_deref(), Some("Mika"));
        assert_eq!(state.ping.latency_calculation, Some(5.5));
        assert_eq!(state.ping.client_latency_calculation, Some(4.0));
        assert_eq!(state.ignoring_server, Some(1));
        assert_eq!(read[1], ServerMessage::Chat { from: "Mika".into(), message: "hi".into() });
    }

    #[test]
    fn members_joining_leaving_and_opening_files_are_read() {
        let read = parse(
            r#"{"Set": {"user": {"Mika": {"room": {"name": "r"}, "event": {"joined": {"version": "1.7.0"}}}, "Ren": {"room": {"name": "r"}, "event": {"left": true}}, "Ada": {"room": {"name": "r"}, "file": {"name": "Episode 3", "duration": 1440, "size": "abc"}}}}}"#,
        )
        .unwrap();
        let ServerMessage::Set(items) = &read[0] else { panic!("{read:?}") };
        let event_of = |who: &str| {
            items.iter().find_map(|item| match item {
                SetItem::User { name, event, .. } if name == who => Some(*event),
                _ => None,
            })
        };
        assert_eq!(event_of("Mika"), Some(Some(UserEvent::Joined)));
        assert_eq!(event_of("Ren"), Some(Some(UserEvent::Left)));
        assert_eq!(event_of("Ada"), Some(None));
        let file = items.iter().find_map(|item| match item {
            SetItem::User { file: Some(file), .. } => Some(file.clone()),
            _ => None,
        });
        assert_eq!(file, Some(FileInfo { name: "Episode 3".into(), duration: Some(1440.0) }));
    }

    #[test]
    fn ready_states_and_the_room_list_are_read() {
        let read = parse(
            r#"{"Set": {"ready": {"username": "Mika", "isReady": true, "manuallyInitiated": true}}}"#,
        )
        .unwrap();
        assert_eq!(
            read,
            vec![ServerMessage::Set(vec![SetItem::Ready {
                username: "Mika".into(),
                ready: Some(true),
                manual: true
            }])]
        );

        let read = parse(
            r#"{"List": {"r": {"Mika": {"position": 3.0, "file": {}, "isReady": null}, "Ren": {"position": 0, "file": {"name": "Episode 3", "duration": 1440.0}, "isReady": false}}}}"#,
        )
        .unwrap();
        let ServerMessage::List(rooms) = &read[0] else { panic!("{read:?}") };
        let room = &rooms["r"];
        assert_eq!(room["Mika"].file, None, "an empty file object is no file");
        assert_eq!(room["Mika"].ready, None);
        assert_eq!(room["Ren"].ready, Some(false));
        assert_eq!(room["Ren"].file.as_ref().unwrap().duration, Some(1440.0));
    }

    #[test]
    fn hello_errors_and_tls_answers_are_read() {
        let read = parse(r#"{"Hello": {"username": "Mika_", "room": {"name": "r"}, "realversion": "1.7.3", "motd": "Be kind"}}"#).unwrap();
        let ServerMessage::Hello(hello) = &read[0] else { panic!("{read:?}") };
        assert_eq!(hello.username, "Mika_");
        assert_eq!(hello.room.as_deref(), Some("r"));
        assert_eq!(hello.motd.as_deref(), Some("Be kind"));

        assert_eq!(
            parse(r#"{"Error": {"message": "Wrong password supplied"}}"#).unwrap(),
            vec![ServerMessage::Error("Wrong password supplied".into())]
        );
        assert_eq!(
            parse(r#"{"TLS": {"startTLS": "true"}}"#).unwrap(),
            vec![ServerMessage::Tls { accepted: true }]
        );
        assert_eq!(
            parse(r#"{"TLS": {"startTLS": "false"}}"#).unwrap(),
            vec![ServerMessage::Tls { accepted: false }]
        );
    }

    #[test]
    fn a_broken_line_is_an_error_and_a_blank_one_is_nothing() {
        assert_eq!(parse("not json"), Err(WireError::NotJson));
        assert_eq!(parse("[1, 2]"), Err(WireError::NotAnObject));
        assert_eq!(parse("  \r"), Ok(Vec::new()));
        assert_eq!(
            parse(r#"{"NewThing": {}}"#).unwrap(),
            vec![ServerMessage::Other("NewThing".into())]
        );
    }
}
