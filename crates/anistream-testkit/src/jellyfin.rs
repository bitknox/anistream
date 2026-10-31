//! A stand-in for a Jellyfin server, on loopback.
//!
//! It speaks the little of the API the client uses: the public info, password and Quick Connect
//! sign-in, views, series with provider ids, episodes, playback info, the stream itself and the
//! progress reports. It keeps what it was told, so a test can read back what the app reported.
//! The server, its people and its titles are made up. HTTP is spoken by hand, one request per
//! connection, as the fake AniList does.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex, MutexGuard},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};

pub const USER: &str = "mika";
pub const PASSWORD: &str = "lantern-tea";
pub const USER_ID: &str = "u-mika";
pub const TOKEN: &str = "fixture-jellyfin-token";
pub const SERVER_ID: &str = "0f1e2d3c";
pub const SERVER_NAME: &str = "Mika's Jellyfin";
/// The code a Quick Connect shows.
pub const QUICK_CODE: &str = "482913";
const QUICK_SECRET: &str = "quick-secret";

/// A series the fake holds. `ids` are the provider ids its metadata carries.
#[derive(Debug, Clone)]
pub struct Show {
    pub id: String,
    pub library: String,
    pub name: String,
    pub ids: Vec<(String, String)>,
    /// `(season, number)` of each episode.
    pub episodes: Vec<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub body: Value,
}

struct State {
    version: String,
    jellyfin: bool,
    quick_connect: bool,
    quick_approved: bool,
    libraries: Vec<(String, String, Option<String>)>,
    shows: Vec<Show>,
    played: Vec<String>,
    requests: Vec<Seen>,
}

/// The running fake. Dropping it stops it.
pub struct FakeJellyfin {
    addr: SocketAddr,
    state: Arc<Mutex<State>>,
    task: JoinHandle<()>,
}

impl Drop for FakeJellyfin {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl FakeJellyfin {
    /// A server of version 10.9.11 with two libraries: Anime Shows, whose series carry AniList or
    /// AniDB ids, and Films, which is not a shows library.
    pub async fn start() -> Self {
        let listener =
            TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.expect("bind loopback");
        let addr = listener.local_addr().expect("local address");
        let state = Arc::new(Mutex::new(State {
            version: "10.9.11".into(),
            jellyfin: true,
            quick_connect: true,
            quick_approved: false,
            libraries: vec![
                ("lib-anime".into(), "Anime Shows".into(), Some("tvshows".into())),
                ("lib-films".into(), "Films".into(), Some("movies".into())),
            ],
            shows: vec![
                Show {
                    id: "s-1".into(),
                    library: "lib-anime".into(),
                    name: "Stardust Wayfarer".into(),
                    ids: vec![("AniList".into(), "900001".into())],
                    episodes: vec![(1, 1), (1, 2), (1, 3)],
                },
                Show {
                    id: "s-2".into(),
                    library: "lib-anime".into(),
                    name: "Lantern Harbour".into(),
                    ids: vec![("AniDB".into(), "424242".into())],
                    episodes: vec![(1, 1), (1, 2)],
                },
                Show {
                    id: "s-3".into(),
                    library: "lib-anime".into(),
                    name: "Quiet Orchard Days".into(),
                    ids: vec![],
                    episodes: vec![(1, 1)],
                },
            ],
            played: Vec::new(),
            requests: Vec::new(),
        }));
        let task = tokio::spawn({
            let state = state.clone();
            async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(serve(socket, state.clone()));
                }
            }
        });
        Self { addr, state, task }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// What a person types: `127.0.0.1:port`.
    pub fn address(&self) -> String {
        self.addr.to_string()
    }

    pub fn set_version(&self, version: &str) {
        self.state().version = version.to_owned();
    }

    /// Answer as something that is not Jellyfin.
    pub fn set_not_jellyfin(&self) {
        self.state().jellyfin = false;
    }

    pub fn set_quick_connect(&self, on: bool) {
        self.state().quick_connect = on;
    }

    /// The person approved the code on a signed-in device.
    pub fn approve_quick_connect(&self) {
        self.state().quick_approved = true;
    }

    pub fn set_libraries(&self, libraries: Vec<(String, String, Option<String>)>) {
        self.state().libraries = libraries;
    }

    pub fn set_shows(&self, shows: Vec<Show>) {
        self.state().shows = shows;
    }

    /// Every request received so far.
    pub fn requests(&self) -> Vec<Seen> {
        self.state().requests.clone()
    }

    /// Items marked played.
    pub fn played(&self) -> Vec<String> {
        self.state().played.clone()
    }
}

struct Request {
    method: String,
    path: String,
    query: Vec<(String, String)>,
    authorization: String,
    body: Value,
}

async fn serve(socket: TcpStream, state: Arc<Mutex<State>>) {
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    let mut line = String::new();
    if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let query = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_owned(), v.to_owned())
        })
        .collect();
    let (mut length, mut authorization) = (0usize, String::new());
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).await.unwrap_or(0) == 0 || header.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            match name.trim().to_ascii_lowercase().as_str() {
                "content-length" => length = value.trim().parse().unwrap_or(0).min(1 << 20),
                "authorization" => authorization = value.trim().to_owned(),
                _ => {}
            }
        }
    }
    let mut raw = vec![0u8; length];
    let _ = reader.read_exact(&mut raw).await;
    let body = serde_json::from_slice(&raw).unwrap_or(Value::Null);
    let request = Request { method, path: path.to_owned(), query, authorization, body };
    let (status, reply) = answer(&state, &request);
    let text = reply.to_string();
    let head = format!(
        "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
        text.len()
    );
    let _ = write.write_all(head.as_bytes()).await;
    let _ = write.write_all(text.as_bytes()).await;
    let _ = write.shutdown().await;
}

fn answer(state: &Mutex<State>, request: &Request) -> (u16, Value) {
    let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
    state.requests.push(Seen {
        method: request.method.clone(),
        path: request.path.clone(),
        body: request.body.clone(),
    });
    let query = |name: &str| {
        request
            .query
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    };
    let signed_in = request.authorization.contains(&format!("Token=\"{TOKEN}\""));
    let path = request.path.as_str();
    let session = || json!({ "AccessToken": TOKEN, "User": { "Id": USER_ID, "Name": USER } });
    match (request.method.as_str(), path) {
        ("GET", "/System/Info/Public") => {
            if !state.jellyfin {
                return (200, json!({ "hello": "world" }));
            }
            (
                200,
                json!({
                    "ServerName": SERVER_NAME,
                    "Version": state.version,
                    "ProductName": "Jellyfin Server",
                    "Id": SERVER_ID,
                }),
            )
        }
        ("POST", "/Users/AuthenticateByName") => {
            if request.body["Username"] == USER && request.body["Pw"] == PASSWORD {
                (200, session())
            } else {
                (401, json!({}))
            }
        }
        ("GET", "/QuickConnect/Enabled") => (200, json!(state.quick_connect)),
        ("POST", "/QuickConnect/Initiate") => {
            if !state.quick_connect {
                return (401, json!({}));
            }
            (200, json!({ "Code": QUICK_CODE, "Secret": QUICK_SECRET, "Authenticated": false }))
        }
        ("GET", "/QuickConnect/Connect") => {
            if query("secret") != QUICK_SECRET {
                return (404, json!({}));
            }
            (
                200,
                json!({ "Code": QUICK_CODE, "Secret": QUICK_SECRET,
                        "Authenticated": state.quick_approved }),
            )
        }
        ("POST", "/Users/AuthenticateWithQuickConnect") => {
            if request.body["Secret"] == QUICK_SECRET && state.quick_approved {
                (200, session())
            } else {
                (401, json!({}))
            }
        }
        _ if !signed_in => (401, json!({})),
        ("GET", p) if p == format!("/Users/{USER_ID}/Views") => {
            let items: Vec<Value> = state
                .libraries
                .iter()
                .map(|(id, name, kind)| json!({ "Id": id, "Name": name, "CollectionType": kind }))
                .collect();
            (200, json!({ "Items": items }))
        }
        ("GET", "/Items") => {
            let items: Vec<Value> = state
                .shows
                .iter()
                .filter(|s| s.library == query("parentId"))
                .map(|s| {
                    let ids: serde_json::Map<String, Value> =
                        s.ids.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
                    json!({ "Id": s.id, "Name": s.name, "ProviderIds": ids,
                            "ChildCount": s.episodes.len() })
                })
                .collect();
            (200, json!({ "Items": items }))
        }
        ("GET", p) if p.starts_with("/Shows/") && p.ends_with("/Episodes") => {
            let series = p.trim_start_matches("/Shows/").trim_end_matches("/Episodes");
            let Some(show) = state.shows.iter().find(|s| s.id == series) else {
                return (404, json!({}));
            };
            let items: Vec<Value> = show
                .episodes
                .iter()
                .map(|(season, number)| {
                    json!({
                        "Id": format!("{series}-{season}-{number}"),
                        "Name": format!("Episode {number}"),
                        "ParentIndexNumber": season,
                        "IndexNumber": number,
                        "RunTimeTicks": 14_400_000_000i64,
                        "UserData": { "PlaybackPositionTicks": 0, "Played": false },
                    })
                })
                .collect();
            (200, json!({ "Items": items }))
        }
        ("POST", p) if p.starts_with("/Items/") && p.ends_with("/PlaybackInfo") => (
            200,
            json!({
                "PlaySessionId": "play-1",
                "MediaSources": [{
                    "Id": "src-1",
                    "Container": "mkv",
                    "SupportsDirectPlay": true,
                    "MediaStreams": [
                        { "Index": 0, "Type": "Video", "Codec": "hevc" },
                        { "Index": 2, "Type": "Subtitle", "IsExternal": true, "Codec": "srt",
                          "Language": "eng",
                          "DeliveryUrl": "/Videos/x/src-1/Subtitles/2/0/Stream.srt" },
                    ],
                }],
            }),
        ),
        ("POST", p) if p.starts_with("/Users/") && p.contains("/PlayedItems/") => {
            let item = p.rsplit('/').next().unwrap_or_default().to_owned();
            state.played.push(item);
            (200, json!({}))
        }
        (
            "POST",
            "/Sessions/Playing" | "/Sessions/Playing/Progress" | "/Sessions/Playing/Stopped",
        ) => (204, Value::Null),
        _ => (404, json!({})),
    }
}
