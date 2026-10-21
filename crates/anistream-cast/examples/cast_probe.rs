//! The casting spike's command line: find a Cast device, serve test media to it on the LAN, play
//! it there, drive it, and report what happened as JSON.
//!
//! ```text
//! cast_probe discover
//! cast_probe info <ip>
//! cast_probe play <ip> --root DIR --path REL [--vtt REL] [--type T] [--hls ts|fmp4]
//!            [--video fmp4|mpeg2_ts] [--seconds N] [--controls] [--no-cors] [--upstream]
//!            [--remux FILE.mkv] [--start S] [--title T] [--keep-volume]
//! cast_probe matrix <ip> --root DIR --cases FILE [--seconds N]
//! cast_probe vtt <in.ass> <out.vtt>
//! cast_probe remux <in.mkv> <out-dir>
//! cast_probe fake                      # a device on loopback; the others take CAST_PORT
//! cast_probe serve <dir>                # the proxy alone, on loopback
//! ```
//!
//! A cast refuses to start while another app is running on the device, mutes the device for its
//! run and puts the volume back after, and stops the receiver app when it ends, so the device
//! goes back to its idle screen.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use anistream_cast::{
    DEFAULT_MEDIA_RECEIVER, Device, Incoming, LanServer, Load, Media, MediaStatus, Options,
    TextTrack,
    channel::NS_MEDIA,
    discovery, remux,
    serve::{Dynamic, content_type},
    vtt,
};
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedReceiver;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "anistream_cast=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("discover") => {
            let found = discovery::discover(Duration::from_secs(3))?;
            for device in found {
                println!(
                    "{}",
                    json!({"instance": device.instance, "name": device.name, "model": device.model,
                           "address": device.address.to_string(), "txt": device.txt})
                );
            }
        }
        Some("info") => info(ip(&args)?).await?,
        // The LAN proxy alone, on loopback, serving a directory: for players on this machine.
        Some("serve") => {
            let root = PathBuf::from(args.get(1).ok_or("serve DIR")?);
            let server = LanServer::start(Options {
                bind: Ipv4Addr::LOCALHOST,
                allow: None,
                root: Some(root),
                upstream_headers: Vec::new(),
                dynamic: None,
                cors: true,
            })
            .await?;
            println!("{}/f/", server.base());
            std::future::pending::<()>().await;
        }
        // A Cast device on loopback, for a dry run of the other commands with CAST_PORT.
        Some("fake") => {
            let device = anistream_cast::fake::FakeDevice::start(4.0, 20.0).await?;
            println!("{}", device.address.port());
            std::future::pending::<()>().await;
        }
        Some("play") => {
            let report = play(ip(&args)?, &Flags::read(&args[2..])?).await?;
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
        Some("matrix") => matrix(ip(&args)?, &args[2..]).await?,
        Some("vtt") => {
            let ass = std::fs::read_to_string(&args[1])?;
            let (text, tally) = vtt::convert(&ass);
            std::fs::write(&args[2], text)?;
            eprintln!("{tally:?}");
        }
        Some("remux") => {
            let started = Instant::now();
            let remuxer = remux::Remuxer::open(&PathBuf::from(&args[1]))?;
            let out = PathBuf::from(&args[2]);
            std::fs::create_dir_all(&out)?;
            std::fs::write(out.join("index.m3u8"), remuxer.playlist("seg", "init.mp4"))?;
            std::fs::write(out.join("init.mp4"), remuxer.init()?)?;
            let opened = started.elapsed();
            for index in 0..remuxer.segments().len() {
                std::fs::write(out.join(format!("seg{index}.m4s")), remuxer.segment(index)?)?;
            }
            eprintln!(
                "{}",
                json!({"tracks": remuxer.describe(), "segments": remuxer.segments().len(),
                       "open_ms": opened.as_millis() as u64,
                       "total_ms": started.elapsed().as_millis() as u64})
            );
        }
        _ => {
            eprintln!("usage: cast_probe discover | info IP | play IP … | matrix IP … | vtt IN OUT | remux IN DIR");
            std::process::exit(2);
        }
    }
    Ok(())
}

/// The device's Cast port: 8009, or `CAST_PORT` for the fake device.
fn port() -> u16 {
    std::env::var("CAST_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8009)
}

fn ip(args: &[String]) -> Result<Ipv4Addr> {
    Ok(args.get(1).ok_or("give the device's address")?.parse()?)
}

async fn info(ip: Ipv4Addr) -> Result<()> {
    let (device, mut events) = Device::connect(SocketAddr::from((ip, port()))).await?;
    println!("receiver status: {}", device.status().await?);
    for (namespace, kind) in [
        ("urn:x-cast:com.google.cast.receiver.discovery", "GET_DEVICE_INFO"),
        ("urn:x-cast:com.google.cast.setup", "eureka_info"),
    ] {
        let mut payload = json!({"type": kind});
        if kind == "eureka_info" {
            payload["data"] = json!({"params": "version,name,build_info,device_info,net"});
        }
        match tokio::time::timeout(
            Duration::from_secs(4),
            device.channel.request("receiver-0", namespace, payload),
        )
        .await
        {
            Ok(Ok(reply)) => println!("{kind}: {reply}"),
            Ok(Err(e)) => println!("{kind}: {e}"),
            Err(_) => println!("{kind}: no answer"),
        }
    }
    while let Ok(event) = events.try_recv() {
        println!("event: {} {}", event.namespace, event.payload);
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct Flags {
    root: Option<PathBuf>,
    path: Option<String>,
    vtt: Option<String>,
    content_type: Option<String>,
    hls: Option<String>,
    video: Option<String>,
    seconds: Option<f64>,
    controls: bool,
    cors: bool,
    upstream: bool,
    remux: Option<PathBuf>,
    start: Option<f64>,
    title: Option<String>,
    keep_volume: bool,
    label: Option<String>,
}

impl Flags {
    fn read(args: &[String]) -> Result<Self> {
        let mut flags = Flags { cors: true, ..Flags::default() };
        let mut it = args.iter();
        while let Some(flag) = it.next() {
            let mut value = || it.next().cloned().ok_or(format!("{flag} needs a value"));
            match flag.as_str() {
                "--root" => flags.root = Some(PathBuf::from(value()?)),
                "--path" => flags.path = Some(value()?),
                "--vtt" => flags.vtt = Some(value()?),
                "--type" => flags.content_type = Some(value()?),
                "--hls" => flags.hls = Some(value()?),
                "--video" => flags.video = Some(value()?),
                "--seconds" => flags.seconds = Some(value()?.parse()?),
                "--start" => flags.start = Some(value()?.parse()?),
                "--title" => flags.title = Some(value()?),
                "--label" => flags.label = Some(value()?),
                "--remux" => flags.remux = Some(PathBuf::from(value()?)),
                "--controls" => flags.controls = true,
                "--no-cors" => flags.cors = false,
                "--upstream" => flags.upstream = true,
                "--keep-volume" => flags.keep_volume = true,
                other => return Err(format!("unknown flag {other}").into()),
            }
        }
        Ok(flags)
    }
}

/// A loopback stand-in for a CDN that refuses requests without the player's Referer, as the
/// locked hosts the mender exists for do.
async fn locked_cdn(root: PathBuf) -> Result<(u16, tokio::task::JoinHandle<()>)> {
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    let port = listener.local_addr()?.port();
    let task = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let root = root.clone();
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buffer = vec![0u8; 8192];
                let Ok(n) = socket.read(&mut buffer).await else { return };
                let request = String::from_utf8_lossy(&buffer[..n]).into_owned();
                let path = request.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let referred = request.lines().any(|l| {
                    l.to_ascii_lowercase().starts_with("referer:")
                        && l.contains("https://player.example/")
                });
                let file = root.join(path.trim_start_matches('/'));
                let answer = match (referred, std::fs::read(&file)) {
                    (true, Ok(body)) => {
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
                            content_type(&path),
                            body.len()
                        );
                        [head.into_bytes(), body].concat()
                    }
                    (false, _) => b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".to_vec(),
                    (true, Err(_)) => {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                    }
                };
                let _ = socket.write_all(&answer).await;
            });
        }
    });
    Ok((port, task))
}

/// The remuxer behind the proxy's `/x/` routes.
struct Remuxed(remux::Remuxer);

impl Dynamic for Remuxed {
    fn get<'a>(&'a self, path: &'a str) -> anistream_cast::serve::Made<'a> {
        Box::pin(async move {
            let remuxer = &self.0;
            match path {
                "index.m3u8" => Some((
                    "application/vnd.apple.mpegurl".into(),
                    remuxer.playlist("seg", "init.mp4").into_bytes(),
                )),
                "init.mp4" => remuxer.init().ok().map(|b| ("video/mp4".into(), b)),
                _ => {
                    let index: usize =
                        path.strip_prefix("seg")?.strip_suffix(".m4s")?.parse().ok()?;
                    let started = Instant::now();
                    let segment = tokio::task::block_in_place(|| remuxer.segment(index)).ok()?;
                    tracing::info!(index, ms = started.elapsed().as_millis() as u64,
                                   bytes = segment.len(), "remuxed a segment");
                    Some(("video/mp4".into(), segment))
                }
            }
        })
    }
}

/// One cast's timeline and measurements.
#[derive(Default)]
struct Timeline {
    entries: Vec<Value>,
    last: Option<MediaStatus>,
    playing_at: Option<Duration>,
    advancing_at: Option<Duration>,
    finished: bool,
    failed: Option<Value>,
}

impl Timeline {
    fn take(&mut self, t: Duration, event: &Incoming) {
        if event.namespace != NS_MEDIA {
            if event.payload["type"] == "CLOSE" {
                self.failed.get_or_insert(json!({"closed": event.source}));
            }
            return;
        }
        let kind = event.payload["type"].as_str().unwrap_or_default();
        if kind != "MEDIA_STATUS" {
            self.entries.push(json!({"t": secs(t), "message": event.payload}));
            if kind.contains("FAILED") || kind.contains("ERROR") || kind == "INVALID_REQUEST" {
                self.failed.get_or_insert(event.payload.clone());
            }
            return;
        }
        let Some(status) = MediaStatus::first(&event.payload) else {
            // An empty status list: the media session has gone.
            self.entries.push(json!({"t": secs(t), "status": "none"}));
            return;
        };
        let changed = self.last.as_ref().is_none_or(|last| {
            last.state != status.state
                || last.idle_reason != status.idle_reason
                || last.active_tracks != status.active_tracks
        });
        if status.state == "PLAYING" && self.playing_at.is_none() {
            self.playing_at = Some(t);
        }
        if status.state == "PLAYING" && status.position > 0.05 && self.advancing_at.is_none() {
            self.advancing_at = Some(t);
        }
        if status.idle_reason.as_deref() == Some("FINISHED") {
            self.finished = true;
        }
        if status.idle_reason.as_deref() == Some("ERROR") {
            self.failed.get_or_insert(event.payload.clone());
        }
        if changed {
            self.entries.push(json!({
                "t": secs(t), "state": status.state, "idle": status.idle_reason,
                "position": status.position, "duration": status.duration,
                "tracks": status.active_tracks,
                "extended": event.payload["status"][0]["extendedStatus"],
            }));
        }
        self.last = Some(status);
    }
}

fn secs(d: Duration) -> f64 {
    (d.as_secs_f64() * 1000.0).round() / 1000.0
}

/// Read events into the timeline until `until` passes or `done` says so.
async fn pump(
    events: &mut UnboundedReceiver<Incoming>,
    timeline: &mut Timeline,
    epoch: Instant,
    until: Instant,
    done: impl Fn(&Timeline) -> bool,
) {
    while Instant::now() < until && !done(timeline) {
        match tokio::time::timeout_at(until.into(), events.recv()).await {
            Ok(Some(event)) => timeline.take(epoch.elapsed(), &event),
            Ok(None) => break,
            Err(_) => break,
        }
    }
}

/// Stop the cast, put the volume back, and let the device return to idle.
async fn wind_down(device: &Device, media: &Media, volume: Option<(f64, bool)>) {
    let _ = media.stop().await;
    let _ = device.stop(&media.app).await;
    if let Some((level, muted)) = volume {
        let _ = device.set_volume(Some(level), Some(muted)).await;
    }
}

async fn prepare(ip: Ipv4Addr, keep_volume: bool) -> Result<(Device, UnboundedReceiver<Incoming>, Option<(f64, bool)>, Duration, Duration)> {
    let connecting = Instant::now();
    let (device, events) = Device::connect(SocketAddr::from((ip, port()))).await?;
    let connected = connecting.elapsed();
    let status = device.status().await?;
    if let Some(app) = device.running().await?
        && app.app_id != DEFAULT_MEDIA_RECEIVER
    {
        return Err(format!(
            "{} is running on the device ({}); not interrupting it",
            app.app_id, status["status"]["applications"][0]["displayName"]
        )
        .into());
    }
    let volume = status["status"]["volume"]["level"]
        .as_f64()
        .zip(status["status"]["volume"]["muted"].as_bool());
    let volume = if keep_volume { None } else { volume };
    if volume.is_some() {
        device.set_volume(None, Some(true)).await?;
    }
    let launching = Instant::now();
    let app = device.launch(DEFAULT_MEDIA_RECEIVER).await?;
    let _ = app;
    Ok((device, events, volume, connected, launching.elapsed()))
}

fn load_for(flags: &Flags, server: &LanServer, cdn: Option<u16>) -> Result<Load> {
    let (url, path) = if flags.remux.is_some() {
        (server.dynamic_url("index.m3u8"), "index.m3u8".to_owned())
    } else {
        let path = flags.path.clone().ok_or("--path is needed")?;
        let url = match cdn {
            Some(port) => server.upstream_url(&format!("http://127.0.0.1:{port}/{path}")),
            None => server.file_url(&path),
        };
        (url, path)
    };
    let content_type = flags.content_type.clone().unwrap_or_else(|| {
        match content_type(&path) {
            "application/vnd.apple.mpegurl" => "application/x-mpegurl",
            other => other,
        }
        .to_owned()
    });
    let tracks: Vec<TextTrack> = flags
        .vtt
        .iter()
        .map(|vtt| TextTrack {
            id: 1,
            url: server.file_url(vtt),
            name: "English".into(),
            language: "en".into(),
        })
        .collect();
    Ok(Load {
        url,
        content_type,
        hls_segment_format: flags.hls.clone(),
        hls_video_segment_format: flags.video.clone(),
        title: flags.title.clone().unwrap_or_else(|| format!("anistream spike · {path}")),
        start: flags.start,
        active_tracks: tracks.iter().map(|t| t.id).collect(),
        tracks,
        autoplay: true,
    })
}

async fn play(ip: Ipv4Addr, flags: &Flags) -> Result<Value> {
    let bind = discovery::local_address_towards(ip)?;
    let remuxer = match &flags.remux {
        Some(file) => {
            let opening = Instant::now();
            let remuxer = remux::Remuxer::open(file)?;
            tracing::info!(ms = opening.elapsed().as_millis() as u64, "remuxer opened");
            Some(Arc::new(Remuxed(remuxer)) as Arc<dyn Dynamic>)
        }
        None => None,
    };
    let (cdn, cdn_task) = match (&flags.root, flags.upstream) {
        (Some(root), true) => {
            let (port, task) = locked_cdn(root.clone()).await?;
            (Some(port), Some(task))
        }
        _ => (None, None),
    };
    let server = LanServer::start(Options {
        bind,
        allow: Some(IpAddr::V4(ip)),
        root: flags.root.clone(),
        upstream_headers: if cdn.is_some() {
            vec![("Referer".into(), "https://player.example/".into())]
        } else {
            Vec::new()
        },
        dynamic: remuxer,
        cors: flags.cors,
    })
    .await?;
    let (device, mut events, volume, connected, launched) =
        prepare(ip, flags.keep_volume).await?;
    let app = device.running().await?.ok_or("the receiver did not start")?;
    let mut media = Media::new(device.channel.clone(), app);
    let load = load_for(flags, &server, cdn)?;

    let report = run(&device, &mut media, &mut events, &server, &load, flags).await;
    wind_down(&device, &media, volume).await;
    drop(server);
    if let Some(task) = cdn_task {
        task.abort();
    }
    let mut report = report?;
    report["connect_ms"] = json!(connected.as_millis() as u64);
    report["launch_ms"] = json!(launched.as_millis() as u64);
    Ok(report)
}

async fn run(
    device: &Device,
    media: &mut Media,
    events: &mut UnboundedReceiver<Incoming>,
    server: &LanServer,
    load: &Load,
    flags: &Flags,
) -> Result<Value> {
    let _ = device;
    server.clear_log();
    let epoch = Instant::now();
    let since_server = server.since_start();
    let mut timeline = Timeline::default();
    let loaded = media.load(load).await;
    let load_ms = epoch.elapsed().as_millis() as u64;
    let load_reply = match loaded {
        Ok(status) => json!({"state": status.state, "duration": status.duration}),
        Err(e) => {
            timeline.failed.get_or_insert(json!({"load": e.to_string()}));
            json!({"error": e.to_string()})
        }
    };
    let seconds = flags.seconds.unwrap_or(12.0);
    let mut controls = Vec::new();
    if timeline.failed.is_none() {
        // Until it plays, or fails, or 25 s pass.
        pump(events, &mut timeline, epoch, Instant::now() + Duration::from_secs(25), |t| {
            t.advancing_at.is_some() || t.failed.is_some() || t.finished
        })
        .await;
        if flags.controls && timeline.advancing_at.is_some() {
            let step = |kind: &'static str| (kind, Instant::now());
            for (kind, wait) in [("PAUSE", 3.0), ("PLAY", 2.0), ("SEEK", 3.0), ("TRACKS_OFF", 2.0),
                                 ("TRACKS_ON", 2.0), ("RATE", 3.0)] {
                pump(events, &mut timeline, epoch, Instant::now() + Duration::from_secs_f64(wait), |_| false).await;
                let (kind, asked) = step(kind);
                let answer = match kind {
                    "PAUSE" => media.pause().await,
                    "PLAY" => media.play().await,
                    "SEEK" => {
                        let target = timeline
                            .last
                            .as_ref()
                            .and_then(|s| s.duration)
                            .map_or(20.0, |d| (d - 8.0).max(1.0));
                        media.seek(target).await
                    }
                    "TRACKS_OFF" if !load.tracks.is_empty() => media.active_tracks(&[]).await,
                    "TRACKS_ON" if !load.tracks.is_empty() => media.active_tracks(&[1]).await,
                    "RATE" => media.set_rate(1.0).await,
                    _ => continue,
                };
                controls.push(json!({
                    "command": kind,
                    "t": secs(epoch.elapsed()),
                    "round_trip_ms": asked.elapsed().as_millis() as u64,
                    "answer": match answer {
                        Ok(Some(s)) => json!({"state": s.state, "position": s.position, "tracks": s.active_tracks}),
                        Ok(None) => json!(null),
                        Err(e) => json!({"error": e.to_string()}),
                    },
                }));
            }
            // To the end, to see FINISHED.
            pump(events, &mut timeline, epoch, Instant::now() + Duration::from_secs(20), |t| {
                t.finished || t.failed.is_some()
            })
            .await;
        } else {
            pump(events, &mut timeline, epoch, epoch + Duration::from_secs_f64(seconds), |t| {
                t.finished || t.failed.is_some()
            })
            .await;
        }
    }
    let final_status = media.status().await.ok().flatten();
    // The proxy's guard, from a peer that is not the device (this machine, on the LAN address):
    // the connection should be dropped before a byte is read.
    let self_fetch = from_here(&load.url).await;
    let log = server.log();
    let first_request = log.first().map(|l| l.at.saturating_sub(since_server));
    let media_bytes: u64 = log.iter().map(|l| l.bytes).sum();
    let busy: f64 = log.iter().map(|l| l.took.as_secs_f64()).sum();
    let biggest = log.iter().max_by_key(|l| l.bytes);
    Ok(json!({
        "label": flags.label,
        "url_kind": load.content_type,
        "load_reply_ms": load_ms,
        "load": load_reply,
        "first_request_s": first_request.map(secs),
        "playing_s": timeline.playing_at.map(secs),
        "first_frame_s": timeline.advancing_at.map(secs),
        "played": timeline.advancing_at.is_some(),
        "finished": timeline.finished,
        "failed": timeline.failed,
        "final": final_status.map(|s| json!({"state": s.state, "position": s.position,
            "idle": s.idle_reason, "tracks": s.active_tracks, "duration": s.duration})),
        "controls": controls,
        "from_another_peer": self_fetch,
        "timeline": timeline.entries,
        "proxy": {
            "requests": log.len(),
            "bytes": media_bytes,
            "options": log.iter().filter(|l| l.method == "OPTIONS").count(),
            "origins": log.iter().filter_map(|l| l.origin.clone()).collect::<std::collections::BTreeSet<_>>(),
            "private_network_preflights": log.iter().filter(|l| l.private_network).count(),
            "statuses": log.iter().map(|l| l.status).collect::<std::collections::BTreeSet<_>>(),
            "biggest": biggest.map(|l| json!({"path": l.path, "bytes": l.bytes,
                "ms": l.took.as_millis() as u64,
                "mbit_s": if l.took.as_secs_f64() > 0.0 { (l.bytes as f64 * 8.0 / 1e6 / l.took.as_secs_f64() * 10.0).round() / 10.0 } else { 0.0 }})),
            "busy_s": secs(Duration::from_secs_f64(busy)),
            "refused_peers": server.refused_peers(),
            "log": log.iter().take(40).map(|l| json!({"t": secs(l.at.saturating_sub(since_server)), "m": l.method,
                "path": l.path.rsplit('/').next().unwrap_or_default(), "range": l.range, "status": l.status,
                "bytes": l.bytes, "ms": l.took.as_millis() as u64})).collect::<Vec<_>>(),
        },
    }))
}

/// Ask the proxy for `url` from this machine, which the allow-list should turn away.
async fn from_here(url: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let Some(rest) = url.strip_prefix("http://") else { return "not proxied".into() };
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let Ok(Ok(mut socket)) =
        tokio::time::timeout(Duration::from_secs(2), tokio::net::TcpStream::connect(authority)).await
    else {
        return "no connection".into();
    };
    let ask = format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n");
    let _ = socket.write_all(ask.as_bytes()).await;
    let mut head = [0u8; 12];
    match tokio::time::timeout(Duration::from_secs(2), socket.read(&mut head)).await {
        Ok(Ok(0)) | Ok(Err(_)) => "dropped".into(),
        Ok(Ok(n)) => String::from_utf8_lossy(&head[..n]).into_owned(),
        Err(_) => "no answer".into(),
    }
}

/// Every case in a file, one per line: `label | flags…`, through one receiver session.
async fn matrix(ip: Ipv4Addr, args: &[String]) -> Result<()> {
    let base = Flags::read(&args.iter().take_while(|a| *a != "--cases").cloned().collect::<Vec<_>>())?;
    let cases = args
        .iter()
        .skip_while(|a| *a != "--cases")
        .nth(1)
        .ok_or("--cases FILE")?;
    let bind = discovery::local_address_towards(ip)?;
    let server = LanServer::start(Options {
        bind,
        allow: Some(IpAddr::V4(ip)),
        root: base.root.clone(),
        upstream_headers: Vec::new(),
        dynamic: None,
        cors: true,
    })
    .await?;
    let (device, mut events, volume, _, _) = prepare(ip, base.keep_volume).await?;
    let app = device.running().await?.ok_or("the receiver did not start")?;
    let mut media = Media::new(device.channel.clone(), app);
    let text = std::fs::read_to_string(cases)?;
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let (label, rest) = line.split_once('|').ok_or("label | flags")?;
        let mut words: Vec<String> = rest.split_whitespace().map(str::to_owned).collect();
        words.extend(["--label".into(), label.trim().into()]);
        let mut flags = Flags::read(&words)?;
        flags.root = base.root.clone();
        flags.seconds = flags.seconds.or(base.seconds).or(Some(10.0));
        let load = load_for(&flags, &server, None)?;
        let report = run(&device, &mut media, &mut events, &server, &load, &flags).await;
        let _ = media.stop().await;
        // Drain what the stop said, so it is not read as the next case's.
        tokio::time::sleep(Duration::from_millis(800)).await;
        while events.try_recv().is_ok() {}
        match report {
            Ok(mut report) => {
                report.as_object_mut().map(|r| r.remove("timeline"));
                if let Some(proxy) = report["proxy"].as_object_mut() {
                    proxy.remove("log");
                }
                println!("{}", serde_json::to_string(&report)?);
            }
            Err(e) => println!("{}", json!({"label": label.trim(), "error": e.to_string()})),
        }
        if !media.channel.is_open() {
            break;
        }
    }
    wind_down(&device, &media, volume).await;
    Ok(())
}
