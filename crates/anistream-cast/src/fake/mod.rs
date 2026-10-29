//! A Cast receiver on loopback, for tests: the platform receiver, the Default Media Receiver and
//! its media session, with playback simulated at a chosen speed.
//!
//! It speaks the same TLS and framing as a TV, with a self-signed test certificate (the files
//! beside this one, made for this alone). A `LOAD` fetches what it is given as the receiver does,
//! with the receiver's `Origin`: for HLS the playlist, the init segment and the first segment,
//! each needing CORS; otherwise the first bytes. What it fetched decides what it refuses, as
//! [`Refuse`] says. Then the position advances at `speed` seconds a second until the duration,
//! where it goes idle with `FINISHED`. Like the real receiver it reports on changes only, so a
//! sender that doesn't poll sees the position stand still.
//!
//! What it does not do: decode anything or answer `deviceauth`.

pub mod matroska;

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};

use crate::{
    channel::{NS_CONNECTION, NS_HEARTBEAT, NS_MEDIA, NS_RECEIVER},
    client::DEFAULT_MEDIA_RECEIVER,
    fetch::{PlainHttp, Upstream},
    probe::RECEIVER_ORIGIN,
    proto::{CastMessage, Payload},
    relay::resolve,
};

const CERT: &[u8] = include_bytes!("cert.der");
const KEY: &[u8] = include_bytes!("key.der");

/// How the fake turns down what it can't play.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// `LOAD_FAILED`.
    LoadFailed,
    /// Accepts the load, and the position never moves.
    Stall,
    /// Accepts the load, then goes idle with `ERROR`.
    IdleError,
    /// Accepts the load, then sends an `ERROR` message, as the Nest Hub does for FLAC.
    Error,
}

/// What the fake refuses: a load whose fetched bytes contain one of `bytes` (a sample entry's
/// type, `hvc1` or `Opus`), or whose content type is one of `content_types`.
#[derive(Debug, Clone, Default)]
pub struct Refuse {
    pub bytes: Vec<Vec<u8>>,
    pub content_types: Vec<String>,
    pub how: Option<Refusal>,
    /// A picture it refuses beside sound it plays goes unremarked: the sound plays, the position
    /// moves, and nothing is reported, as the Nest Hub does with HEVC in a file.
    pub sound_alone: bool,
    /// HLS whose picture is taller than this is `LOAD_FAILED`: the Nest Hub stops at 720 lines.
    pub hls_taller_than: Option<u32>,
}

/// How the fake is set up.
#[derive(Debug, Clone)]
pub struct FakeOptions {
    /// Seconds of playback a second.
    pub speed: f64,
    /// Every file's length unless its HLS playlist says.
    pub duration: f64,
    pub refuse: Refuse,
    /// The volume's `controlType`: `attenuation` or `fixed`.
    pub control: &'static str,
    /// Whether it fetches at all: a TV behind a firewall doesn't, and a `LOAD` then hangs.
    pub fetches: bool,
}

impl Default for FakeOptions {
    fn default() -> Self {
        Self {
            speed: 1.0,
            duration: 20.0,
            refuse: Refuse::default(),
            control: "attenuation",
            fetches: true,
        }
    }
}

/// One fetch the fake made: the URL, its status, the body's length, and whether the answer
/// carried CORS.
#[derive(Debug, Clone)]
pub struct Fetched {
    pub url: String,
    pub status: u16,
    pub bytes: usize,
    pub cors: bool,
}

/// A running fake receiver.
pub struct FakeReceiver {
    pub address: SocketAddr,
    /// Every message the receiver was sent, as JSON with its namespace.
    pub heard: Arc<Mutex<Vec<(String, Value)>>>,
    pub fetched: Arc<Mutex<Vec<Fetched>>>,
    control: mpsc::UnboundedSender<Control>,
    task: JoinHandle<()>,
}

enum Control {
    TakeOver,
    Silence,
}

impl Drop for FakeReceiver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone)]
struct Playing {
    session: i64,
    url: String,
    duration: f64,
    position: f64,
    state: &'static str,
    rate: f64,
    tracks: Vec<u64>,
    since: Instant,
    stalled: bool,
}

impl Playing {
    fn advance(&mut self, speed: f64) {
        if self.state == "PLAYING" && !self.stalled {
            self.position += self.since.elapsed().as_secs_f64() * self.rate * speed;
        }
        self.since = Instant::now();
    }

    fn status(&self, idle_reason: Option<&str>, volume: &Value) -> Value {
        let mut status = json!({
            "mediaSessionId": self.session,
            "playbackRate": self.rate,
            "playerState": if idle_reason.is_some() { "IDLE" } else { self.state },
            "currentTime": self.position.min(self.duration),
            "supportedMediaCommands": 12303,
            "volume": volume,
            "activeTrackIds": self.tracks,
            "media": {"contentId": self.url, "duration": self.duration, "streamType": "BUFFERED"},
        });
        if let Some(reason) = idle_reason {
            status["idleReason"] = json!(reason);
        }
        status
    }
}

impl FakeReceiver {
    pub async fn start(options: FakeOptions) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
        let config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(std::io::Error::other)?
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(CERT.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec())),
            )
            .map_err(std::io::Error::other)?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let heard: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let fetched: Arc<Mutex<Vec<Fetched>>> = Arc::default();
        let (control, mut controls) = mpsc::unbounded_channel();
        let task = {
            let (heard, fetched) = (Arc::clone(&heard), Arc::clone(&fetched));
            tokio::spawn(async move {
                // One sender at a time, as the tests have it; the next connection takes over.
                let mut current: Option<JoinHandle<()>> = None;
                let mut forward: Option<mpsc::UnboundedSender<Control>> = None;
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            let Ok((socket, _)) = accepted else { return };
                            let acceptor = acceptor.clone();
                            let (heard, fetched, options) =
                                (Arc::clone(&heard), Arc::clone(&fetched), options.clone());
                            let (tx, rx) = mpsc::unbounded_channel();
                            forward = Some(tx);
                            if let Some(previous) = current.take() {
                                previous.abort();
                            }
                            current = Some(tokio::spawn(async move {
                                if let Ok(tls) = acceptor.accept(socket).await {
                                    let _ = serve(tls, heard, fetched, options, rx).await;
                                }
                            }));
                        }
                        Some(control) = controls.recv() => {
                            if let Some(forward) = &forward {
                                let _ = forward.send(control);
                            }
                        }
                    }
                }
            })
        };
        Ok(Self { address, heard, fetched, control, task })
    }

    /// The messages heard so far of `kind`, in any namespace.
    pub fn heard(&self, kind: &str) -> Vec<Value> {
        lock(&self.heard)
            .iter()
            .filter(|(_, v)| v["type"] == kind)
            .map(|(_, v)| v.clone())
            .collect()
    }

    pub fn fetched(&self) -> Vec<Fetched> {
        lock(&self.fetched).clone()
    }

    /// Another sender launches its own app, as when someone casts from their phone.
    pub fn take_over(&self) {
        let _ = self.control.send(Control::TakeOver);
    }

    /// Say nothing more and read nothing more, with the connection left open: a TV switched
    /// off at the wall, or gone from the Wi-Fi, which sends no FIN.
    pub fn go_silent(&self) {
        let _ = self.control.send(Control::Silence);
    }
}

#[allow(clippy::too_many_lines)]
async fn serve(
    tls: tokio_rustls::server::TlsStream<TcpStream>,
    heard: Arc<Mutex<Vec<(String, Value)>>>,
    fetched: Arc<Mutex<Vec<Fetched>>>,
    options: FakeOptions,
    mut controls: mpsc::UnboundedReceiver<Control>,
) -> std::io::Result<()> {
    let (mut read, mut write) = tokio::io::split(tls);
    let (tx, mut rx) = mpsc::unbounded_channel::<CastMessage>();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if write.write_all(&message.frame()).await.is_err() {
                break;
            }
        }
    });
    let transport = "web-1".to_owned();
    let mut app: Option<(String, &'static str)> = None;
    let mut playing: Option<Playing> = None;
    let mut next_session = 1;
    let mut level = 0.4;
    let mut muted = false;
    let mut ticker = tokio::time::interval(Duration::from_millis(50));
    let send = |source: &str, destination: &str, namespace: &str, payload: Value| {
        let _ = tx.send(CastMessage::text(source, destination, namespace, payload.to_string()));
    };
    let volume = |level: f64, muted: bool| json!({"level": level, "muted": muted, "controlType": options.control, "stepInterval": 0.05});
    let receiver_status = |app: &Option<(String, &'static str)>, request: u64, level, muted| {
        let applications: Vec<Value> = app
            .iter()
            .map(|(session, id)| {
                json!({"appId": id, "displayName": "Receiver", "sessionId": session,
                       "transportId": if *id == DEFAULT_MEDIA_RECEIVER { "web-1" } else { "web-9" },
                       "isIdleScreen": false, "namespaces": [{"name": NS_MEDIA}]})
            })
            .collect();
        json!({"type": "RECEIVER_STATUS", "requestId": request, "status": {
            "applications": applications, "volume": volume(level, muted)}})
    };
    let mut sender = String::from("*");
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Some(p) = playing.as_mut() {
                    p.advance(options.speed);
                    if p.position >= p.duration {
                        let done = p.status(Some("FINISHED"), &volume(level, muted));
                        playing = None;
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0, "status": [done]}));
                    }
                }
            }
            Some(control) = controls.recv() => match control {
                Control::TakeOver => {
                    app = Some(("someone-else".into(), "E0000000"));
                    playing = None;
                    send("receiver-0", "*", NS_RECEIVER, receiver_status(&app, 0, level, muted));
                }
                Control::Silence => {
                    // Holding the socket, the writer and all, and never touching it again.
                    let _held = (&read, &tx);
                    std::future::pending::<()>().await;
                }
            },
            frame = read_frame(&mut read) => {
                let Some(message) = frame? else { return Ok(()) };
                let Payload::Text(text) = &message.payload else { continue };
                let Ok(payload) = serde_json::from_str::<Value>(text) else { continue };
                sender.clone_from(&message.source);
                lock(&heard).push((message.namespace.clone(), payload.clone()));
                let request = payload["requestId"].as_u64().unwrap_or(0);
                let kind = payload["type"].as_str().unwrap_or_default().to_owned();
                match (message.namespace.as_str(), kind.as_str()) {
                    (NS_HEARTBEAT, "PING") => send(&message.destination, &sender, NS_HEARTBEAT, json!({"type": "PONG"})),
                    (NS_RECEIVER, "GET_STATUS") => {
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_RECEIVER, "SET_VOLUME") => {
                        if options.control != "fixed" && let Some(l) = payload["volume"]["level"].as_f64() {
                            level = l;
                        }
                        if let Some(m) = payload["volume"]["muted"].as_bool() {
                            muted = m;
                        }
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_RECEIVER, "LAUNCH") => {
                        app = Some((format!("session-{request}"), DEFAULT_MEDIA_RECEIVER));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_RECEIVER, "STOP") => {
                        app = None;
                        playing = None;
                        send(&transport, &sender, NS_CONNECTION, json!({"type": "CLOSE"}));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request, level, muted));
                    }
                    (NS_MEDIA, "LOAD") => {
                        if !options.fetches {
                            // A TV that can't reach the sender: the load hangs.
                            continue;
                        }
                        let url = payload["media"]["contentId"].as_str().unwrap_or_default().to_owned();
                        let content_type = payload["media"]["contentType"].as_str().unwrap_or_default().to_owned();
                        let hls = content_type.contains("mpegurl");
                        let result = fetch_like_a_receiver(&url, hls, &fetched).await;
                        let refused = match &result {
                            Ok((_, bytes)) => {
                                let mut info = crate::formats::MediaInfo::default();
                                crate::probe::read_sample_entries(bytes, &mut info);
                                let tall = hls && options.refuse.hls_taller_than.zip(info.height).is_some_and(|(limit, h)| h > limit);
                                let matched = options.refuse.content_types.contains(&content_type)
                                    || options.refuse.bytes.iter().any(|needle| contains(bytes, needle));
                                let sound = SOUND.iter().any(|needle| contains(bytes, needle));
                                if tall {
                                    Some(Refusal::LoadFailed)
                                } else {
                                    options.refuse.how.filter(|_| matched && !(options.refuse.sound_alone && sound && !hls))
                                }
                            }
                            Err(_) => Some(Refusal::LoadFailed),
                        };
                        if refused == Some(Refusal::LoadFailed) {
                            send(&transport, &sender, NS_MEDIA, json!({
                                "type": "LOAD_FAILED", "requestId": request, "detailedErrorCode": 104}));
                            continue;
                        }
                        let duration = result.ok().and_then(|(d, _)| d).unwrap_or(options.duration);
                        let session = next_session;
                        next_session += 1;
                        if let Some(old) = playing.take() {
                            send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                                "status": [old.status(Some("INTERRUPTED"), &volume(level, muted))]}));
                        }
                        let p = Playing {
                            session,
                            url,
                            duration,
                            position: payload["currentTime"].as_f64().unwrap_or(0.0),
                            state: if payload["autoplay"] == false { "PAUSED" } else { "PLAYING" },
                            rate: 1.0,
                            tracks: payload["activeTrackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
                            since: Instant::now(),
                            stalled: refused == Some(Refusal::Stall),
                        };
                        send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": [p.status(None, &volume(level, muted))]}));
                        if refused == Some(Refusal::Error) {
                            send(&transport, "*", NS_MEDIA, json!({"type": "ERROR", "requestId": 0, "detailedErrorCode": 100}));
                            continue;
                        }
                        if refused == Some(Refusal::IdleError) {
                            send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0,
                                "status": [p.status(Some("ERROR"), &volume(level, muted))]}));
                        } else {
                            playing = Some(p);
                        }
                    }
                    (NS_MEDIA, kind) => {
                        let Some(p) = playing.as_mut() else {
                            send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": []}));
                            continue;
                        };
                        p.advance(options.speed);
                        let mut idle = None;
                        match kind {
                            "PLAY" => p.state = "PLAYING",
                            "PAUSE" => p.state = "PAUSED",
                            "SEEK" => p.position = payload["currentTime"].as_f64().unwrap_or(p.position),
                            "SET_PLAYBACK_RATE" => p.rate = payload["playbackRate"].as_f64().unwrap_or(1.0),
                            "EDIT_TRACKS_INFO" => p.tracks = payload["activeTrackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
                            "STOP" => idle = Some("CANCELLED"),
                            _ => {}
                        }
                        let status = p.status(idle, &volume(level, muted));
                        if idle.is_some() {
                            playing = None;
                        }
                        send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": [status]}));
                    }
                    _ => {}
                }
            }
        }
    }
}

/// What says a stream has sound: an MP4 audio sample entry, or a Matroska audio codec id.
const SOUND: [&[u8]; 6] = [b"mp4a", b"Opus", b"fLaC", b"A_AAC", b"A_OPUS", b"A_FLAC"];

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}

async fn read_frame<R: AsyncReadExt + Unpin>(
    read: &mut R,
) -> std::io::Result<Option<CastMessage>> {
    let mut len = [0u8; 4];
    if read.read_exact(&mut len).await.is_err() {
        return Ok(None);
    }
    let mut body = vec![0u8; u32::from_be_bytes(len) as usize];
    read.read_exact(&mut body).await?;
    CastMessage::decode(&body).map(Some).map_err(std::io::Error::other)
}

/// GET `url` as the receiver does; for HLS the playlist, its init segment and its first
/// segment, each refused without CORS. The duration is the playlist's, when there is one, and
/// the bytes are everything fetched, for the refusal rules.
async fn fetch_like_a_receiver(
    url: &str,
    hls: bool,
    fetched: &Mutex<Vec<Fetched>>,
) -> Result<(Option<f64>, Vec<u8>), u16> {
    let get = |url: String| async move {
        let headers = [
            ("Origin".to_owned(), RECEIVER_ORIGIN.to_owned()),
            ("Range".to_owned(), "bytes=0-".to_owned()),
        ];
        let response = PlainHttp.get(&url, &headers).await.map_err(|_| 502u16)?;
        let (status, cors) =
            (response.status, response.header("access-control-allow-origin").is_some());
        let body = response.prefix(4 << 20).await.map_err(|_| 502u16)?;
        Ok::<_, u16>((url, status, cors, body))
    };
    let mut all = Vec::new();
    let mut note = |(url, status, cors, body): (String, u16, bool, Vec<u8>)| {
        lock(fetched).push(Fetched { url, status, bytes: body.len(), cors });
        if !(200..300).contains(&status) || (hls && !cors) {
            return Err(status);
        }
        all.extend_from_slice(&body);
        Ok(body)
    };
    let body = note(get(url.to_owned()).await?)?;
    if !hls {
        return Ok((None, all));
    }
    let mut url = url.to_owned();
    let mut text = String::from_utf8_lossy(&body).into_owned();
    if let Some(variant) = text
        .lines()
        .skip_while(|l| !l.starts_with("#EXT-X-STREAM-INF"))
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        url = resolve(&url, variant.trim());
        let body = note(get(url.clone()).await?)?;
        text = String::from_utf8_lossy(&body).into_owned();
    }
    let duration: f64 = text
        .lines()
        .filter_map(|l| l.strip_prefix("#EXTINF:"))
        .filter_map(|l| l.split(',').next()?.parse::<f64>().ok())
        .sum();
    if let Some(map) = text.lines().find_map(|l| {
        let at = l.find("URI=\"")? + 5;
        l.starts_with("#EXT-X-MAP").then(|| l[at..].split('"').next().map(str::to_owned))?
    }) {
        note(get(resolve(&url, &map)).await?)?;
    }
    if let Some(first) = text.lines().find(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        note(get(resolve(&url, first.trim())).await?)?;
    }
    Ok(((duration > 0.0).then_some(duration), all))
}
