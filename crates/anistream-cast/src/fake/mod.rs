//! A Cast device on loopback, for tests: the platform receiver, the Default Media Receiver and
//! its media session, with playback simulated at a chosen speed.
//!
//! It speaks the same TLS and framing as a device, with a self-signed test certificate (the
//! files beside this one, made for this alone). A `LOAD` fetches the URL it is given, as a
//! receiver does: the playlist and its first segment for HLS, the first bytes otherwise, so the
//! proxy in front of it is exercised. Then the position advances at `speed` seconds a second,
//! with `MEDIA_STATUS` broadcast every half second and on every change, until it reaches the
//! duration and goes idle with `FINISHED`.
//!
//! What it does not do: decode anything, check codecs, or answer `deviceauth`.

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
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
    channel::{NS_HEARTBEAT, NS_MEDIA, NS_RECEIVER},
    client::DEFAULT_MEDIA_RECEIVER,
    proto::{CastMessage, Payload},
};

const CERT: &[u8] = include_bytes!("cert.der");
const KEY: &[u8] = include_bytes!("key.der");

/// A running fake device.
pub struct FakeDevice {
    pub address: SocketAddr,
    /// Every message the device was sent, as JSON with its namespace, for assertions.
    pub heard: Arc<Mutex<Vec<(String, Value)>>>,
    /// What each `LOAD`'s fetch through the proxy returned: the status of each request.
    pub fetched: Arc<Mutex<Vec<(String, u16, usize)>>>,
    task: JoinHandle<()>,
}

impl Drop for FakeDevice {
    fn drop(&mut self) {
        self.task.abort();
    }
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
}

impl Playing {
    fn advance(&mut self, speed: f64) {
        if self.state == "PLAYING" {
            self.position += self.since.elapsed().as_secs_f64() * self.rate * speed;
        }
        self.since = Instant::now();
    }

    fn status(&self, idle_reason: Option<&str>) -> Value {
        let mut status = json!({
            "mediaSessionId": self.session,
            "playbackRate": self.rate,
            "playerState": if idle_reason.is_some() { "IDLE" } else { self.state },
            "currentTime": self.position.min(self.duration),
            "supportedMediaCommands": 12303,
            "volume": {"level": 1.0, "muted": false},
            "activeTrackIds": self.tracks,
            "media": {"contentId": self.url, "duration": self.duration, "streamType": "BUFFERED"},
        });
        if let Some(reason) = idle_reason {
            status["idleReason"] = json!(reason);
        }
        status
    }
}

impl FakeDevice {
    /// Listen on loopback, with playback running `speed` times real time, and every loaded
    /// file `duration` seconds long unless its HLS playlist says otherwise.
    pub async fn start(speed: f64, duration: f64) -> std::io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(CERT.to_vec())],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY.to_vec())),
            )
            .map_err(std::io::Error::other)?;
        let acceptor = TlsAcceptor::from(Arc::new(config));
        let heard: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let fetched: Arc<Mutex<Vec<(String, u16, usize)>>> = Arc::default();
        let task = {
            let (heard, fetched) = (Arc::clone(&heard), Arc::clone(&fetched));
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let acceptor = acceptor.clone();
                    let (heard, fetched) = (Arc::clone(&heard), Arc::clone(&fetched));
                    tokio::spawn(async move {
                        if let Ok(tls) = acceptor.accept(socket).await {
                            let _ = serve(tls, heard, fetched, speed, duration).await;
                        }
                    });
                }
            })
        };
        Ok(Self { address, heard, fetched, task })
    }

    /// The messages heard so far of `kind`, in any namespace.
    pub fn heard(&self, kind: &str) -> Vec<Value> {
        lock(&self.heard)
            .iter()
            .filter(|(_, v)| v["type"] == kind)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

async fn serve(
    tls: tokio_rustls::server::TlsStream<TcpStream>,
    heard: Arc<Mutex<Vec<(String, Value)>>>,
    fetched: Arc<Mutex<Vec<(String, u16, usize)>>>,
    speed: f64,
    default_duration: f64,
) -> std::io::Result<()> {
    let (mut read, mut write) = tokio::io::split(tls);
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<CastMessage>();
    tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if write.write_all(&message.frame()).await.is_err() {
                break;
            }
        }
    });
    let transport = "web-1".to_owned();
    let mut app: Option<String> = None;
    let mut playing: Option<Playing> = None;
    let mut next_session = 1;
    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    let send = |source: &str, destination: &str, namespace: &str, payload: Value| {
        let _ = tx.send(CastMessage::text(source, destination, namespace, payload.to_string()));
    };
    let receiver_status = |app: &Option<String>, request: u64| {
        let applications: Vec<Value> = app
            .iter()
            .map(|session| {
                json!({"appId": DEFAULT_MEDIA_RECEIVER, "displayName": "Default Media Receiver",
                       "sessionId": session, "transportId": "web-1", "isIdleScreen": false,
                       "namespaces": [{"name": NS_MEDIA}]})
            })
            .collect();
        json!({"type": "RECEIVER_STATUS", "requestId": request, "status": {
            "applications": applications, "volume": {"level": 0.4, "muted": false}}})
    };
    let mut sender = String::new();
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if let Some(p) = playing.as_mut() {
                    p.advance(speed);
                    if p.position >= p.duration {
                        let done = p.status(Some("FINISHED"));
                        playing = None;
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0, "status": [done]}));
                    } else if p.state == "PLAYING" {
                        send(&transport, "*", NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": 0, "status": [p.status(None)]}));
                    }
                }
            }
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
                    (NS_RECEIVER, "GET_STATUS" | "SET_VOLUME") => {
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request));
                    }
                    (NS_RECEIVER, "LAUNCH") => {
                        app = Some(format!("session-{request}"));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request));
                    }
                    (NS_RECEIVER, "STOP") => {
                        app = None;
                        playing = None;
                        send(&transport, &sender, "urn:x-cast:com.google.cast.tp.connection", json!({"type": "CLOSE"}));
                        send("receiver-0", &sender, NS_RECEIVER, receiver_status(&app, request));
                    }
                    (NS_MEDIA, "LOAD") => {
                        let url = payload["media"]["contentId"].as_str().unwrap_or_default().to_owned();
                        match fetch_like_a_receiver(&url, &fetched).await {
                            Ok(playlist_duration) => {
                                let session = next_session;
                                next_session += 1;
                                let p = Playing {
                                    session,
                                    url,
                                    duration: playlist_duration.unwrap_or(default_duration),
                                    position: payload["currentTime"].as_f64().unwrap_or(0.0),
                                    state: if payload["autoplay"] == false { "PAUSED" } else { "PLAYING" },
                                    rate: 1.0,
                                    tracks: payload["activeTrackIds"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default(),
                                    since: Instant::now(),
                                };
                                send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": [p.status(None)]}));
                                playing = Some(p);
                            }
                            Err(status) => send(&transport, &sender, NS_MEDIA, json!({
                                "type": "LOAD_FAILED", "requestId": request, "detailedErrorCode": 104,
                                "reason": format!("the media answered {status}")})),
                        }
                    }
                    (NS_MEDIA, kind) => {
                        let Some(p) = playing.as_mut() else {
                            send(&transport, &sender, NS_MEDIA, json!({"type": "MEDIA_STATUS", "requestId": request, "status": []}));
                            continue;
                        };
                        p.advance(speed);
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
                        let status = p.status(idle);
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

/// GET `url` over plain HTTP; the playlist's first segment too, for HLS. The duration is the
/// playlist's, when there is one.
async fn fetch_like_a_receiver(
    url: &str,
    fetched: &Mutex<Vec<(String, u16, usize)>>,
) -> Result<Option<f64>, u16> {
    let (status, body) = get(url).await.map_err(|_| 502u16)?;
    lock(fetched).push((url.to_owned(), status, body.len()));
    if !(200..300).contains(&status) {
        return Err(status);
    }
    let text = String::from_utf8_lossy(&body);
    if !text.starts_with("#EXTM3U") {
        return Ok(None);
    }
    let duration: f64 = text
        .lines()
        .filter_map(|l| l.strip_prefix("#EXTINF:"))
        .filter_map(|l| l.split(',').next()?.parse::<f64>().ok())
        .sum();
    if let Some(first) = text.lines().find(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let segment = crate::serve::resolve(url, first.trim());
        let (status, body) = get(&segment).await.map_err(|_| 502u16)?;
        lock(fetched).push((segment, status, body.len()));
        if !(200..300).contains(&status) {
            return Err(status);
        }
    }
    Ok((duration > 0.0).then_some(duration))
}

async fn get(url: &str) -> std::io::Result<(u16, Vec<u8>)> {
    let rest = url.strip_prefix("http://").ok_or_else(|| std::io::Error::other("not http"))?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let mut socket = TcpStream::connect(authority).await?;
    let ask = format!(
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nOrigin: https://www.gstatic.com\r\n\
         Range: bytes=0-\r\nConnection: close\r\n\r\n"
    );
    socket.write_all(ask.as_bytes()).await?;
    let mut raw = Vec::new();
    socket.read_to_end(&mut raw).await?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(raw.len());
    let status = String::from_utf8_lossy(&raw[..split])
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Ok((status, raw.get(split + 4..).unwrap_or_default().to_vec()))
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    use super::*;
    use crate::{Device, LanServer, Load, Media, MediaStatus, Options, TextTrack};

    /// Read `events` until a media status matches `want`, or a few seconds pass.
    async fn until(
        events: &mut tokio::sync::mpsc::UnboundedReceiver<crate::Incoming>,
        want: impl Fn(&MediaStatus) -> bool,
    ) -> Option<MediaStatus> {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while let Ok(Some(event)) = tokio::time::timeout_at(deadline, events.recv()).await {
            if let Some(status) = MediaStatus::first(&event.payload)
                && want(&status)
            {
                return Some(status);
            }
        }
        None
    }

    #[tokio::test]
    async fn a_cast_runs_from_launch_to_finished_through_the_proxy() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("index.m3u8"),
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\nseg1.ts\n#EXT-X-ENDLIST\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("seg0.ts"), vec![0x47u8; 188 * 10]).unwrap();
        // Ten times real time: the 8 s playlist plays in under a second.
        let device = FakeDevice::start(10.0, 20.0).await.unwrap();
        let server = LanServer::start(Options {
            bind: Ipv4Addr::LOCALHOST,
            allow: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            root: Some(dir.path().to_owned()),
            upstream_headers: Vec::new(),
            dynamic: None,
            cors: true,
        })
        .await
        .unwrap();

        let (cast, mut events) = Device::connect(device.address).await.unwrap();
        let app = cast.launch(DEFAULT_MEDIA_RECEIVER).await.unwrap();
        assert_eq!(app.transport_id, "web-1");
        let mut media = Media::new(cast.channel.clone(), app.clone());
        let load = Load {
            url: server.file_url("index.m3u8"),
            content_type: "application/x-mpegurl".into(),
            title: "test".into(),
            start: Some(2.0),
            tracks: vec![TextTrack {
                id: 1,
                url: server.file_url("en.vtt"),
                name: "English".into(),
                language: "en".into(),
            }],
            active_tracks: vec![1],
            autoplay: true,
            ..Load::default()
        };
        let first = media.load(&load).await.unwrap();
        assert_eq!((first.state.as_str(), first.duration), ("PLAYING", Some(8.0)));
        assert!(first.position >= 2.0, "resumed where it was told");
        assert_eq!(first.active_tracks, [1]);

        // The receiver fetched the playlist and its first segment through the proxy.
        let fetched = lock(&device.fetched).clone();
        // `Range: bytes=0-`, as a receiver opens with, is answered 206 with the whole file.
        assert_eq!(fetched.iter().map(|f| f.1).collect::<Vec<_>>(), [206, 206]);
        assert_eq!(fetched[1].2, 1880);
        assert!(server.log().iter().all(|l| l.status == 206));

        let paused = media.pause().await.unwrap().unwrap();
        assert_eq!(paused.state, "PAUSED");
        let sought = media.seek(6.0).await.unwrap().unwrap();
        assert_eq!(sought.position, 6.0);
        media.play().await.unwrap();
        let off = media.active_tracks(&[]).await.unwrap().unwrap();
        assert!(off.active_tracks.is_empty());

        let done = until(&mut events, |s| s.idle_reason.is_some()).await.expect("an ending");
        assert_eq!(done.idle_reason.as_deref(), Some("FINISHED"));

        cast.stop(&app).await.unwrap();
        assert!(cast.running().await.unwrap().is_none());
        assert_eq!(device.heard("LOAD")[0]["currentTime"], 2.0);
    }

    #[tokio::test]
    async fn a_load_the_proxy_refuses_fails_as_the_receiver_says() {
        let dir = tempfile::tempdir().unwrap();
        let device = FakeDevice::start(1.0, 20.0).await.unwrap();
        let server = LanServer::start(Options {
            bind: Ipv4Addr::LOCALHOST,
            allow: None,
            root: Some(dir.path().to_owned()),
            upstream_headers: Vec::new(),
            dynamic: None,
            cors: true,
        })
        .await
        .unwrap();
        let (cast, _events) = Device::connect(device.address).await.unwrap();
        let app = cast.launch(DEFAULT_MEDIA_RECEIVER).await.unwrap();
        let mut media = Media::new(cast.channel.clone(), app);
        let missing = Load {
            url: server.file_url("missing.mp4"),
            content_type: "video/mp4".into(),
            autoplay: true,
            ..Load::default()
        };
        let refused = media.load(&missing).await.unwrap_err();
        assert!(refused.to_string().contains("LOAD_FAILED"), "{refused}");

        // A guessed token is a 404, whatever the path.
        let wrong = server
            .file_url("x")
            .replace(&server.base()[server.base().len() - 32..], &"0".repeat(32));
        assert_eq!(get(&wrong).await.unwrap().0, 404);
    }
}
