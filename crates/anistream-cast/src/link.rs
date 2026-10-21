//! A cast device as the player a playback runs in: what the engine's `CastLink` would wrap.
//!
//! The session drives every player in mpv's vocabulary through its `PlayerLink`: load a URL with
//! per-file options, set and cycle properties, seek, stop, quit. [`CastPlayer`] takes those
//! calls and turns them into Cast commands, and turns what the receiver reports into the same
//! [`PlaybackEvent`]s mpv's reports become, so resume, skips, Up Next and the history the
//! session writes every 10 s work unchanged:
//!
//! | The session says | The receiver is told |
//! | --- | --- |
//! | `load(url, options)` | `LOAD` of the URL as the LAN proxy serves it, at `start`, titled `force-media-title`; a local MKV is remuxed, `http-header-fields` go on the proxy's upstream requests |
//! | `add_subtitle(path)` | the file as WebVTT (ASS converted), side-loaded as a text track: by a reload at the current position, since `tracks` are fixed at `LOAD` |
//! | `set pause`, `cycle pause` | `PAUSE`, `PLAY` |
//! | `seek` | `SEEK` (relative ones from the last known position) |
//! | `set speed`, `add speed` | `SET_PLAYBACK_RATE` |
//! | `set volume`, `set mute` | the receiver's `SET_VOLUME` |
//! | `set sid` | `EDIT_TRACKS_INFO` |
//! | `stop`; `quit` | `STOP`; `STOP` and the receiver app stopped |
//! | shaders, audio filters, chapters | nothing: the TV scales the picture, and there is no filter |
//!
//! | The receiver reports | The session hears |
//! | --- | --- |
//! | the first status of a new media session | `Loaded` |
//! | `currentTime`, `media.duration` (polled every second) | `Progress` |
//! | `PAUSED` / `PLAYING` | `Paused` |
//! | `BUFFERING` | `Buffering` |
//! | `playbackRate`, volume | `Speed`, `Volume` |
//! | `IDLE` + `FINISHED` | `Ended { complete: true }` |
//! | `IDLE` + `CANCELLED` or `ERROR`, the app closed | `Ended { complete: false }` |
//! | `IDLE` + `INTERRUPTED` (our own next `LOAD`) | `Replaced` |

use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anistream_playback::PlaybackEvent;
use serde_json::{Value, json};
use tokio::{
    sync::{Mutex as AsyncMutex, mpsc},
    task::JoinHandle,
};

use crate::{
    App, CastError, DEFAULT_MEDIA_RECEIVER, Device, Incoming, LanServer, Load, Media,
    MediaStatus, Options, TextTrack,
    channel::{NS_CONNECTION, NS_MEDIA},
    discovery::local_address_towards,
    remux::Remuxer,
    serve::{Dynamic, Made},
    vtt,
};

/// What the proxy makes on request for one cast: the remuxed stream and the text tracks.
#[derive(Default)]
struct Resources {
    remuxer: Mutex<Option<Arc<Remuxer>>>,
    texts: Mutex<HashMap<String, Arc<Vec<u8>>>>,
}

impl Dynamic for Resources {
    fn get<'a>(&'a self, path: &'a str) -> Made<'a> {
        Box::pin(async move {
            if let Some(text) = lock(&self.texts).get(path).cloned() {
                return Some(("text/vtt".into(), text.to_vec()));
            }
            let remuxer = lock(&self.remuxer).clone()?;
            match path {
                "r/index.m3u8" => Some((
                    "application/vnd.apple.mpegurl".into(),
                    remuxer.playlist("seg", "init.mp4").into_bytes(),
                )),
                "r/init.mp4" => remuxer.init().ok().map(|b| ("video/mp4".into(), b)),
                _ => {
                    let index =
                        path.strip_prefix("r/seg")?.strip_suffix(".m4s")?.parse().ok()?;
                    let segment = tokio::task::spawn_blocking(move || remuxer.segment(index))
                        .await
                        .ok()?
                        .ok()?;
                    Some(("video/mp4".into(), segment))
                }
            }
        })
    }
}

#[derive(Default)]
struct State {
    /// The media session the session last heard `Loaded` for.
    loaded: Option<i64>,
    last: Option<MediaStatus>,
    paused: Option<bool>,
    buffering: bool,
    rate: Option<f64>,
    volume: Option<f64>,
    ended: bool,
    closed: bool,
    quit: bool,
    /// A SEEK is out, and its answer ends the seek.
    seeking: bool,
    /// The receiver's last words on a failure.
    error: Option<String>,
    /// The load in force, with its tracks, for a reload when a subtitle comes later.
    load: Option<Load>,
}

/// A Cast device playing what the session tells it.
pub struct CastPlayer {
    device: Device,
    media: AsyncMutex<Media>,
    server: LanServer,
    resources: Arc<Resources>,
    state: Arc<Mutex<State>>,
    events: mpsc::UnboundedSender<PlaybackEvent>,
    tasks: Vec<JoinHandle<()>>,
}

impl Drop for CastPlayer {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl CastPlayer {
    /// Connect to the device at `address`, start the Default Media Receiver on it, and open the
    /// LAN proxy it will fetch from, on the interface facing it and for it alone.
    pub async fn open(
        address: SocketAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<PlaybackEvent>), CastError> {
        let bind = match address.ip() {
            IpAddr::V4(ip) if ip.is_loopback() => ip,
            IpAddr::V4(ip) => local_address_towards(ip)?,
            IpAddr::V6(_) => {
                return Err(CastError::Protocol("IPv6 devices are not tried".into()));
            }
        };
        let resources = Arc::new(Resources::default());
        let server = LanServer::start(Options {
            bind,
            allow: Some(address.ip()),
            root: None,
            upstream_headers: Vec::new(),
            dynamic: Some(Arc::clone(&resources) as Arc<dyn Dynamic>),
            cors: true,
        })
        .await?;
        let (device, incoming) = Device::connect(address).await?;
        let app = device.launch(DEFAULT_MEDIA_RECEIVER).await?;
        let media = Media::new(device.channel.clone(), app.clone());
        let (events, rx) = mpsc::unbounded_channel();
        let state = Arc::new(Mutex::new(State::default()));
        let pump =
            tokio::spawn(pump(incoming, app.clone(), Arc::clone(&state), events.clone()));
        let poll = {
            let (channel, state) = (device.channel.clone(), Arc::clone(&state));
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tick.tick().await;
                    let active = {
                        let state = lock(&state);
                        state.loaded.is_some() && !state.ended && !state.closed
                    };
                    // The reply comes back as an event, with every other status.
                    if active
                        && channel
                            .send(
                                &app.transport_id,
                                NS_MEDIA,
                                &json!({"type": "GET_STATUS", "requestId": 1_000_000}),
                            )
                            .is_err()
                    {
                        break;
                    }
                }
            })
        };
        let player = Self {
            device,
            media: AsyncMutex::new(media),
            server,
            resources,
            state,
            events,
            tasks: vec![pump, poll],
        };
        Ok((player, rx))
    }

    /// Where the receiver fetches from, for the report.
    pub fn proxy_base(&self) -> &str {
        self.server.base()
    }

    /// Load `url` with mpv's per-file `options`, in place of whatever plays.
    pub async fn load(&self, url: &str, options: &[(String, String)]) -> Result<(), CastError> {
        let option =
            |name: &str| options.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
        let headers = option("http-header-fields").map(|v| header_list(&v)).unwrap_or_default();
        let (cast_url, content_type) = self.serve(url, headers)?;
        let load = Load {
            hls_segment_format: content_type.contains("mpegurl").then(|| "fmp4".to_owned()),
            hls_video_segment_format: None,
            url: cast_url,
            content_type,
            title: option("force-media-title").unwrap_or_default(),
            start: option("start").and_then(|s| s.parse().ok()),
            tracks: Vec::new(),
            active_tracks: Vec::new(),
            autoplay: true,
        };
        self.send_load(load).await
    }

    async fn send_load(&self, load: Load) -> Result<(), CastError> {
        {
            let mut state = lock(&self.state);
            state.ended = false;
            state.error = None;
            state.load = Some(load.clone());
        }
        let mut media = self.media.lock().await;
        media.load(&load).await.map(|_| ())
    }

    /// The URL the receiver fetches `url` by, and its content type.
    fn serve(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
    ) -> Result<(String, String), CastError> {
        let path = url.strip_prefix("file://").unwrap_or(url);
        let kind = |p: &str| crate::serve::content_type(p).to_owned();
        if !url.contains("://") || url.starts_with("file://") {
            let path = PathBuf::from(path);
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mkv")) {
                let remuxer =
                    Remuxer::open(&path).map_err(|e| CastError::Protocol(e.to_string()))?;
                *lock(&self.resources.remuxer) = Some(Arc::new(remuxer));
                return Ok((
                    self.server.dynamic_url("r/index.m3u8"),
                    "application/x-mpegurl".into(),
                ));
            }
            return Ok((
                self.server.share_file(&path),
                kind(path.to_str().unwrap_or_default()),
            ));
        }
        let bare = url.split('?').next().unwrap_or(url);
        let content_type = match kind(bare).as_str() {
            "application/octet-stream" => "application/x-mpegurl".to_owned(),
            "application/vnd.apple.mpegurl" => "application/x-mpegurl".to_owned(),
            other => other.to_owned(),
        };
        // A public stream that needs no headers is fetched by the receiver itself; anything on
        // this machine's loopback, or locked to a referer, comes through the proxy.
        let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
        if headers.is_empty() && !local {
            return Ok((url.to_owned(), content_type));
        }
        self.server.set_upstream_headers(headers);
        Ok((self.server.upstream_url(url), content_type))
    }

    /// Side-load a subtitle file as a WebVTT text track, converted from ASS or SRT.
    pub async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), CastError> {
        let path = Path::new(url.strip_prefix("file://").unwrap_or(url));
        let text = std::fs::read_to_string(path)?;
        let extension = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
        let webvtt = match extension.as_deref() {
            Some("ass" | "ssa") => vtt::convert(&text).0,
            Some("srt") => srt_to_vtt(&text),
            _ => text,
        };
        let id = lock(&self.resources.texts).len() as u32 + 1;
        let name = format!("t/{id}.vtt");
        lock(&self.resources.texts).insert(name.clone(), Arc::new(webvtt.into_bytes()));
        let track = TextTrack {
            id,
            url: self.server.dynamic_url(&name),
            name: title.unwrap_or_else(|| format!("Subtitles {id}")),
            language: language.unwrap_or_else(|| "und".into()),
        };
        // Tracks are fixed at LOAD: reload where it is, with the track on.
        let reload = {
            let state = lock(&self.state);
            state.load.clone().map(|mut load| {
                load.start = state.last.as_ref().map(|s| s.position).or(load.start);
                load.tracks.push(track.clone());
                load.active_tracks = vec![track.id];
                load
            })
        };
        match reload {
            Some(load) => self.send_load(load).await,
            None => Ok(()),
        }
    }

    /// Set one of mpv's properties, as far as a receiver has it.
    pub async fn set(&self, property: &str, value: Value) -> Result<(), CastError> {
        let media = self.media.lock().await;
        match property {
            "pause" => {
                if value.as_bool().unwrap_or(false) {
                    media.pause().await?;
                } else {
                    media.play().await?;
                }
            }
            "speed" => {
                media.set_rate(value.as_f64().unwrap_or(1.0)).await?;
            }
            "volume" => {
                let level = (value.as_f64().unwrap_or(100.0) / 100.0).clamp(0.0, 1.0);
                self.device.set_volume(Some(level), None).await?;
            }
            "mute" => {
                self.device.set_volume(None, Some(value.as_bool().unwrap_or(false))).await?;
            }
            "sid" => {
                let ids: Vec<u32> =
                    value.as_u64().map(|id| vec![id as u32]).unwrap_or_default();
                media.active_tracks(&ids).await?;
            }
            // Shaders, tone mapping, audio filters and the companion's user data mean nothing to
            // a receiver.
            _ => {}
        }
        Ok(())
    }

    /// Flip a boolean property without reading it, as mpv's `cycle` does.
    pub async fn cycle(&self, property: &str) -> Result<(), CastError> {
        if property == "pause" {
            let paused = lock(&self.state).paused.unwrap_or(false);
            return self.set("pause", json!(!paused)).await;
        }
        Ok(())
    }

    /// Step a numeric property by `delta`.
    pub async fn add(&self, property: &str, delta: f64) -> Result<(), CastError> {
        match property {
            "speed" => {
                let rate = lock(&self.state).rate.unwrap_or(1.0);
                self.set("speed", json!((rate + delta).clamp(0.5, 2.0))).await
            }
            "volume" => {
                let volume = lock(&self.state).volume.unwrap_or(100.0);
                self.set("volume", json!(volume + delta)).await
            }
            _ => Ok(()),
        }
    }

    /// Seek to `seconds`, or by them when not `absolute`.
    pub async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), CastError> {
        let target = if absolute {
            seconds
        } else {
            lock(&self.state).last.as_ref().map_or(0.0, |s| s.position) + seconds
        };
        lock(&self.state).seeking = true;
        let _ = self.events.send(PlaybackEvent::Seeking(true));
        let media = self.media.lock().await;
        media.seek(target.max(0.0)).await.map(|_| ())
    }

    /// Stop the file, keeping the receiver for the next.
    pub async fn stop(&self) -> Result<(), CastError> {
        self.media.lock().await.stop().await.map(|_| ())
    }

    /// Stop playing and close the receiver app, so the TV goes back to what it showed.
    pub async fn quit(&self) -> Result<(), CastError> {
        lock(&self.state).quit = true;
        let media = self.media.lock().await;
        let _ = media.stop().await;
        self.device.stop(&media.app).await
    }

    pub fn is_alive(&self) -> bool {
        self.device.channel.is_open() && !lock(&self.state).closed
    }

    pub fn has_quit(&self) -> bool {
        lock(&self.state).quit
    }

    /// The receiver's words on why nothing played, if it said any.
    pub fn diagnostics(&self) -> Option<String> {
        lock(&self.state).error.clone()
    }
}

/// Read what the receiver says into the session's events.
async fn pump(
    mut incoming: mpsc::UnboundedReceiver<Incoming>,
    app: App,
    state: Arc<Mutex<State>>,
    events: mpsc::UnboundedSender<PlaybackEvent>,
) {
    let say = |event| {
        let _ = events.send(event);
    };
    while let Some(message) = incoming.recv().await {
        let closed = (message.namespace == NS_CONNECTION
            && message.payload["type"] == "CLOSE"
            && message.source == app.transport_id)
            || (message.payload["type"] == "RECEIVER_STATUS"
                && !message.payload["status"]["applications"].as_array().is_some_and(|apps| {
                    apps.iter().any(|a| a["sessionId"] == app.session_id.as_str())
                }));
        if closed {
            let mut state = lock(&state);
            if !state.closed {
                state.closed = true;
                if state.loaded.is_some() && !state.ended {
                    state.ended = true;
                    say(PlaybackEvent::Ended { complete: false });
                }
            }
            continue;
        }
        if message.namespace != NS_MEDIA {
            continue;
        }
        if matches!(message.payload["type"].as_str(), Some("LOAD_FAILED" | "LOAD_CANCELLED")) {
            let mut state = lock(&state);
            state.error = Some(format!(
                "the receiver could not load it ({})",
                message.payload["detailedErrorCode"].as_i64().map_or_else(
                    || message.payload["type"].as_str().unwrap_or_default().to_owned(),
                    |c| c.to_string()
                )
            ));
            state.ended = true;
            say(PlaybackEvent::Ended { complete: false });
            continue;
        }
        let Some(status) = MediaStatus::first(&message.payload) else { continue };
        let mut state = lock(&state);
        let new_session = state.loaded != Some(status.media_session_id);
        if let Some(reason) = status.idle_reason.as_deref() {
            if new_session || state.ended {
                continue;
            }
            state.ended = true;
            match reason {
                "FINISHED" => say(PlaybackEvent::Ended { complete: true }),
                "INTERRUPTED" => {
                    // Our own next LOAD took its place.
                    state.ended = false;
                    say(PlaybackEvent::Replaced);
                }
                other => {
                    if other == "ERROR" {
                        state.error = Some("the receiver stopped with an error".into());
                    }
                    say(PlaybackEvent::Ended { complete: false });
                }
            }
            continue;
        }
        if status.state == "IDLE" {
            continue;
        }
        if new_session {
            state.loaded = Some(status.media_session_id);
            state.ended = false;
            state.paused = None;
            say(PlaybackEvent::Loaded);
        }
        say(PlaybackEvent::Progress { position: status.position, duration: status.duration });
        let paused = status.state == "PAUSED";
        if state.paused != Some(paused) {
            state.paused = Some(paused);
            say(PlaybackEvent::Paused(paused));
        }
        let buffering = status.state == "BUFFERING" || status.state == "LOADING";
        if state.buffering != buffering {
            state.buffering = buffering;
            say(PlaybackEvent::Buffering(buffering));
        }
        if state.rate != Some(status.rate) {
            state.rate = Some(status.rate);
            say(PlaybackEvent::Speed(status.rate));
        }
        if let Some(level) = status.volume.map(|v| (v * 100.0).round())
            && state.volume != Some(level)
        {
            state.volume = Some(level);
            say(PlaybackEvent::Volume(level));
        }
        if state.seeking
            && message.payload["requestId"].as_u64().is_some_and(|id| id != 1_000_000)
        {
            // The answer to the SEEK, or the first status after it.
            state.seeking = false;
            say(PlaybackEvent::Seeking(false));
        }
        state.last = Some(status);
    }
}

/// mpv's `http-header-fields` list back into pairs: items split on unescaped commas.
fn header_list(value: &str) -> Vec<(String, String)> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => current.extend(chars.next()),
            ',' => items.push(std::mem::take(&mut current)),
            c => current.push(c),
        }
    }
    items.push(current);
    items
        .into_iter()
        .filter_map(|item| {
            let (name, value) = item.split_once(':')?;
            Some((name.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// SRT to WebVTT: a header, and full stops for the decimal commas.
fn srt_to_vtt(srt: &str) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for line in srt.trim_start_matches('\u{feff}').lines() {
        if line.contains("-->") {
            out.push_str(&line.replace(',', "."));
        } else if line.trim().chars().all(|c| c.is_ascii_digit()) && !line.trim().is_empty() {
            continue;
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeDevice;

    async fn next(
        rx: &mut mpsc::UnboundedReceiver<PlaybackEvent>,
        want: impl Fn(&PlaybackEvent) -> bool,
    ) -> PlaybackEvent {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let event = tokio::time::timeout_at(deadline, rx.recv())
                .await
                .expect("the event in time")
                .expect("the player still there");
            if want(&event) {
                return event;
            }
        }
    }

    #[test]
    fn header_lists_read_back_as_mpv_was_given_them() {
        assert_eq!(
            header_list("Referer: https://player.example/,User-Agent: A\\, B"),
            [
                ("Referer".to_owned(), "https://player.example/".to_owned()),
                ("User-Agent".to_owned(), "A, B".to_owned())
            ]
        );
    }

    #[tokio::test]
    async fn the_session_drives_a_cast_as_it_drives_mpv() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("episode.mp4");
        std::fs::write(&file, vec![0u8; 4096]).unwrap();
        let subtitles = dir.path().join("episode.ass");
        std::fs::write(
            &subtitles,
            "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
             Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n",
        )
        .unwrap();
        // Five times real time, 20 s long: about four seconds of test.
        let device = FakeDevice::start(5.0, 20.0).await.unwrap();
        let (player, mut events) = CastPlayer::open(device.address).await.unwrap();

        let options = anistream_playback::protocol::file_options(
            &[("Referer".into(), "https://player.example/".into())],
            Some(4.0),
            "A Show · Episode 1",
            Some("en"),
            false,
        );
        player.load(file.to_str().unwrap(), &options).await.unwrap();
        assert_eq!(
            next(&mut events, |e| *e == PlaybackEvent::Loaded).await,
            PlaybackEvent::Loaded
        );
        let PlaybackEvent::Progress { position, duration } =
            next(&mut events, |e| matches!(e, PlaybackEvent::Progress { .. })).await
        else {
            unreachable!()
        };
        assert!(position >= 4.0, "resumed at the start option, got {position}");
        assert_eq!(duration, Some(20.0));
        let load = &device.heard("LOAD")[0];
        assert_eq!(load["media"]["metadata"]["title"], "A Show · Episode 1");
        assert_eq!(load["media"]["contentType"], "video/mp4");

        // A subtitle after the file loaded reloads it where it is, with the track on.
        player
            .add_subtitle(
                subtitles.to_str().unwrap(),
                Some("English".into()),
                Some("en".into()),
            )
            .await
            .unwrap();
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        // A new media session says it plays.
        next(&mut events, |e| *e == PlaybackEvent::Paused(false)).await;
        let reload = &device.heard("LOAD")[1];
        assert_eq!(reload["activeTrackIds"], json!([1]));
        assert!(reload["currentTime"].as_f64().unwrap() >= 4.0);
        assert_eq!(reload["media"]["tracks"][0]["trackContentType"], "text/vtt");

        player.cycle("pause").await.unwrap();
        assert_eq!(
            next(&mut events, |e| matches!(e, PlaybackEvent::Paused(_))).await,
            PlaybackEvent::Paused(true)
        );
        player.set("pause", json!(false)).await.unwrap();
        assert_eq!(
            next(&mut events, |e| matches!(e, PlaybackEvent::Paused(_))).await,
            PlaybackEvent::Paused(false)
        );
        player.add("speed", 0.5).await.unwrap();
        assert_eq!(
            next(&mut events, |e| matches!(e, PlaybackEvent::Speed(_))).await,
            PlaybackEvent::Speed(1.5)
        );
        player.seek(15.0, true).await.unwrap();
        next(&mut events, |e| *e == PlaybackEvent::Seeking(false)).await;

        // On to the end: the session hears a complete ending, as from mpv at EOF.
        assert_eq!(
            next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. })).await,
            PlaybackEvent::Ended { complete: true }
        );
        player.quit().await.unwrap();
        assert!(player.has_quit());
        assert!(
            device.heard("STOP").iter().any(|m| m["sessionId"].is_string()),
            "the app stopped"
        );
    }
}
