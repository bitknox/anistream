//! The receiver and media namespaces over a [`Channel`]: launch the Default Media Receiver, load
//! a URL with its text tracks, play, pause, seek, stop, set the volume, and read what it reports.

use std::net::SocketAddr;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::channel::{CastError, Channel, Incoming, NS_MEDIA, NS_RECEIVER, RECEIVER};

/// Google's Default Media Receiver: needs no registration and plays HLS, DASH and progressive
/// MP4 through the Cast web player.
pub const DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";

/// A connected receiver, through its platform receiver.
pub struct Device {
    pub channel: Channel,
}

/// An app running on the receiver, as a sender talks to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    pub app_id: String,
    pub session_id: String,
    pub transport_id: String,
}

/// How the receiver's volume can be changed: its `controlType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VolumeControl {
    /// `attenuation` or `master`: the level can be set.
    #[default]
    Level,
    /// `fixed`: the TV's own volume is in charge, and only mute works.
    MuteOnly,
}

/// The parts of a `RECEIVER_STATUS` a sender needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceiverStatus {
    pub volume: VolumeControl,
    pub level: Option<f64>,
    pub muted: Option<bool>,
    /// The app in the foreground, other than the idle screen.
    pub app: Option<App>,
}

impl ReceiverStatus {
    pub fn read(payload: &Value) -> Option<Self> {
        let status = payload.get("status")?;
        let volume = &status["volume"];
        Some(Self {
            volume: match volume["controlType"].as_str() {
                Some("fixed") => VolumeControl::MuteOnly,
                _ => VolumeControl::Level,
            },
            level: volume["level"].as_f64(),
            muted: volume["muted"].as_bool(),
            app: running_app(payload, None),
        })
    }
}

impl Device {
    pub async fn connect(
        address: SocketAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Incoming>), CastError> {
        let (channel, events) = Channel::connect(address).await?;
        Ok((Self { channel }, events))
    }

    /// The receiver's `RECEIVER_STATUS`: volume, and the app running, if any.
    pub async fn status(&self) -> Result<ReceiverStatus, CastError> {
        let reply =
            self.channel.request(RECEIVER, NS_RECEIVER, json!({"type": "GET_STATUS"})).await?;
        ReceiverStatus::read(&reply)
            .ok_or_else(|| CastError::Protocol(format!("GET_STATUS answered with {reply}")))
    }

    /// Launch `app_id`, or join it if it already runs, and connect to its transport.
    pub async fn launch(&self, app_id: &str) -> Result<App, CastError> {
        let status = self
            .channel
            .request(RECEIVER, NS_RECEIVER, json!({"type": "LAUNCH", "appId": app_id}))
            .await?;
        let app = match running_app(&status, Some(app_id)) {
            Some(app) => app,
            // The first status can come before the app is up; ask once more.
            None => {
                let reply = self
                    .channel
                    .request(RECEIVER, NS_RECEIVER, json!({"type": "GET_STATUS"}))
                    .await?;
                running_app(&reply, Some(app_id))
                    .ok_or_else(|| CastError::Protocol(format!("{app_id} did not start")))?
            }
        };
        self.channel.connect_to(&app.transport_id)?;
        Ok(app)
    }

    /// Stop the app `app` names, which ends any media in it.
    pub async fn stop(&self, app: &App) -> Result<(), CastError> {
        self.channel
            .request(
                RECEIVER,
                NS_RECEIVER,
                json!({"type": "STOP", "sessionId": app.session_id}),
            )
            .await
            .map(|_| ())
    }

    pub async fn set_volume(
        &self,
        level: Option<f64>,
        muted: Option<bool>,
    ) -> Result<(), CastError> {
        let mut volume = json!({});
        if let Some(level) = level {
            volume["level"] = json!(level.clamp(0.0, 1.0));
        }
        if let Some(muted) = muted {
            volume["muted"] = json!(muted);
        }
        self.channel
            .request(RECEIVER, NS_RECEIVER, json!({"type": "SET_VOLUME", "volume": volume}))
            .await
            .map(|_| ())
    }
}

/// The app a status names: `want`, or else any that is not the idle screen.
pub fn running_app(status: &Value, want: Option<&str>) -> Option<App> {
    status["status"]["applications"].as_array()?.iter().find_map(|app| {
        let app_id = app["appId"].as_str()?;
        if want.is_some_and(|want| want != app_id) || app["isIdleScreen"] == true {
            return None;
        }
        Some(App {
            app_id: app_id.to_owned(),
            session_id: app["sessionId"].as_str()?.to_owned(),
            transport_id: app["transportId"].as_str()?.to_owned(),
        })
    })
}

/// A side-loaded text track: WebVTT the receiver fetches itself.
#[derive(Debug, Clone, PartialEq)]
pub struct TextTrack {
    pub id: u32,
    pub url: String,
    pub name: String,
    pub language: String,
}

/// How the receiver draws text tracks, for the whole track: the viewer's subtitle style, as the
/// `textTrackStyle` of a load.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// `#RRGGBBAA`.
    pub foreground: String,
    pub background: String,
    /// `NONE`, `OUTLINE`, `DROP_SHADOW`, `RAISED` or `DEPRESSED`.
    pub edge: String,
    pub edge_color: String,
    pub scale: f64,
}

impl Default for TextStyle {
    /// Plain white on a soft shadow, close to the apps' own subtitle default.
    fn default() -> Self {
        Self {
            foreground: "#FFFFFFFF".into(),
            background: "#00000000".into(),
            edge: "DROP_SHADOW".into(),
            edge_color: "#000000C0".into(),
            scale: 1.0,
        }
    }
}

/// What the receiver's loading screen and its own controls show.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata {
    /// The show.
    pub series: String,
    /// "Episode 7", or the episode's own name.
    pub title: String,
    pub image: Option<String>,
}

/// What to load, as the `LOAD` message's media.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Load {
    pub url: String,
    pub content_type: String,
    /// `fmp4` or `ts`, for HLS: the receiver otherwise guesses from the first segment.
    pub hls_segment_format: Option<String>,
    pub metadata: Metadata,
    pub start: Option<f64>,
    pub tracks: Vec<TextTrack>,
    pub active_tracks: Vec<u32>,
    pub style: Option<TextStyle>,
    pub autoplay: bool,
}

impl Load {
    pub fn message(&self, session_id: &str) -> Value {
        let mut metadata = json!({
            "metadataType": 2,
            "seriesTitle": self.metadata.series,
            "title": self.metadata.title,
        });
        if let Some(image) = &self.metadata.image {
            metadata["images"] = json!([{"url": image}]);
        }
        let mut media = json!({
            "contentId": self.url,
            "contentUrl": self.url,
            "streamType": "BUFFERED",
            "contentType": self.content_type,
            "metadata": metadata,
        });
        if let Some(format) = &self.hls_segment_format {
            media["hlsSegmentFormat"] = json!(format);
            media["hlsVideoSegmentFormat"] =
                json!(if format == "fmp4" { "fmp4" } else { "mpeg2_ts" });
        }
        if !self.tracks.is_empty() {
            media["tracks"] = self
                .tracks
                .iter()
                .map(|track| {
                    json!({
                        "trackId": track.id,
                        "type": "TEXT",
                        "subtype": "SUBTITLES",
                        "trackContentId": track.url,
                        "trackContentType": "text/vtt",
                        "name": track.name,
                        "language": track.language,
                    })
                })
                .collect();
            let style = self.style.clone().unwrap_or_default();
            media["textTrackStyle"] = json!({
                "backgroundColor": style.background,
                "foregroundColor": style.foreground,
                "edgeType": style.edge,
                "edgeColor": style.edge_color,
                "fontScale": style.scale,
                "fontFamily": "Sans-Serif",
            });
        }
        let mut load = json!({
            "type": "LOAD",
            "sessionId": session_id,
            "media": media,
            "autoplay": self.autoplay,
        });
        if let Some(start) = self.start {
            load["currentTime"] = json!(start);
        }
        if !self.active_tracks.is_empty() {
            load["activeTrackIds"] = json!(self.active_tracks);
        }
        load
    }
}

/// The media session inside a running receiver app.
#[derive(Clone)]
pub struct Media {
    pub channel: Channel,
    pub app: App,
    pub media_session_id: Option<i64>,
}

impl Media {
    pub fn new(channel: Channel, app: App) -> Self {
        Self { channel, app, media_session_id: None }
    }

    async fn ask(&self, payload: Value) -> Result<Value, CastError> {
        self.channel.request(&self.app.transport_id, NS_MEDIA, payload).await
    }

    fn session(&self) -> Result<i64, CastError> {
        self.media_session_id.ok_or_else(|| CastError::Protocol("nothing is loaded".into()))
    }

    /// Load `load`, and return the first status of it.
    pub async fn load(&mut self, load: &Load) -> Result<MediaStatus, CastError> {
        let reply = self.ask(load.message(&self.app.session_id)).await?;
        let status = MediaStatus::first(&reply)
            .ok_or_else(|| CastError::Protocol(format!("LOAD answered with {reply}")))?;
        self.media_session_id = Some(status.media_session_id);
        Ok(status)
    }

    pub async fn command(
        &self,
        kind: &str,
        extra: Value,
    ) -> Result<Option<MediaStatus>, CastError> {
        let mut payload = json!({"type": kind, "mediaSessionId": self.session()?});
        if let (Value::Object(payload), Value::Object(extra)) = (&mut payload, extra) {
            payload.extend(extra);
        }
        Ok(MediaStatus::first(&self.ask(payload).await?))
    }

    pub async fn play(&self) -> Result<Option<MediaStatus>, CastError> {
        self.command("PLAY", json!({})).await
    }

    pub async fn pause(&self) -> Result<Option<MediaStatus>, CastError> {
        self.command("PAUSE", json!({})).await
    }

    pub async fn seek(&self, seconds: f64) -> Result<Option<MediaStatus>, CastError> {
        self.command("SEEK", json!({"currentTime": seconds.max(0.0)})).await
    }

    pub async fn stop(&self) -> Result<Option<MediaStatus>, CastError> {
        self.command("STOP", json!({})).await
    }

    pub async fn set_rate(&self, rate: f64) -> Result<Option<MediaStatus>, CastError> {
        self.command("SET_PLAYBACK_RATE", json!({"playbackRate": rate})).await
    }

    pub async fn active_tracks(&self, ids: &[u32]) -> Result<Option<MediaStatus>, CastError> {
        self.command("EDIT_TRACKS_INFO", json!({"activeTrackIds": ids})).await
    }

    /// Ask for the media status without waiting for it: the answer comes back with every other
    /// status the receiver sends. The Default Media Receiver reports on changes only, so this is
    /// what keeps the position moving.
    pub fn poll(&self) -> Result<(), CastError> {
        let mut payload = json!({"type": "GET_STATUS", "requestId": POLL_REQUEST});
        if let Some(id) = self.media_session_id {
            payload["mediaSessionId"] = json!(id);
        }
        self.channel.send(&self.app.transport_id, NS_MEDIA, &payload)
    }
}

/// The request id every status poll goes out with: no request waits on it.
pub const POLL_REQUEST: u64 = 1 << 40;

/// The parts of a `MEDIA_STATUS` a player cares about.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaStatus {
    pub media_session_id: i64,
    /// `IDLE`, `LOADING`, `BUFFERING`, `PLAYING` or `PAUSED`.
    pub state: String,
    /// Why it is idle: `FINISHED`, `CANCELLED`, `INTERRUPTED` or `ERROR`.
    pub idle_reason: Option<String>,
    pub position: f64,
    pub duration: Option<f64>,
    pub rate: f64,
    pub active_tracks: Vec<u32>,
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    /// The media the receiver plays, by the URL it was loaded with.
    pub content: Option<String>,
}

impl MediaStatus {
    /// The first media status a message carries, when it carries one.
    pub fn first(payload: &Value) -> Option<Self> {
        let status = payload["status"].as_array()?.first()?;
        Some(Self {
            media_session_id: status["mediaSessionId"].as_i64()?,
            state: status["playerState"].as_str().unwrap_or("IDLE").to_owned(),
            idle_reason: status["idleReason"].as_str().map(str::to_owned),
            position: status["currentTime"].as_f64().unwrap_or(0.0),
            duration: status["media"]["duration"].as_f64(),
            rate: status["playbackRate"].as_f64().unwrap_or(1.0),
            active_tracks: status["activeTrackIds"]
                .as_array()
                .map(|ids| {
                    ids.iter()
                        .filter_map(|id| id.as_u64().and_then(|id| u32::try_from(id).ok()))
                        .collect()
                })
                .unwrap_or_default(),
            volume: status["volume"]["level"].as_f64(),
            muted: status["volume"]["muted"].as_bool(),
            content: status["media"]["contentId"].as_str().map(str::to_owned),
        })
    }

    /// Whether the message says the media session is gone without a status for it: an empty
    /// status list, as a receiver answers once its media has ended.
    pub fn none_in(payload: &Value) -> bool {
        payload["type"] == "MEDIA_STATUS"
            && payload["status"].as_array().is_some_and(std::vec::Vec::is_empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_load_names_its_tracks_start_and_loading_screen() {
        let load = Load {
            url: "http://192.0.2.1:1/t/x/r/index.m3u8".into(),
            content_type: "application/x-mpegurl".into(),
            hls_segment_format: Some("fmp4".into()),
            metadata: Metadata {
                series: "Harbour Lights".into(),
                title: "Episode 7".into(),
                image: Some("https://art.example/still.jpg".into()),
            },
            start: Some(42.0),
            tracks: vec![TextTrack {
                id: 1,
                url: "http://192.0.2.1:1/t/x/t/1.vtt".into(),
                name: "English".into(),
                language: "en".into(),
            }],
            active_tracks: vec![1],
            style: None,
            autoplay: true,
        };
        let message = load.message("s");
        assert_eq!(message["currentTime"], 42.0);
        assert_eq!(message["activeTrackIds"], json!([1]));
        assert_eq!(message["media"]["tracks"][0]["trackContentType"], "text/vtt");
        assert_eq!(message["media"]["hlsSegmentFormat"], "fmp4");
        assert_eq!(message["media"]["hlsVideoSegmentFormat"], "fmp4");
        assert_eq!(message["media"]["metadata"]["seriesTitle"], "Harbour Lights");
        assert_eq!(
            message["media"]["metadata"]["images"][0]["url"],
            "https://art.example/still.jpg"
        );
        assert_eq!(message["media"]["textTrackStyle"]["edgeType"], "DROP_SHADOW");
    }

    #[test]
    fn a_finished_status_reads_as_finished() {
        let payload = json!({"type": "MEDIA_STATUS", "status": [{
            "mediaSessionId": 3, "playerState": "IDLE", "idleReason": "FINISHED",
            "currentTime": 20.0, "playbackRate": 1, "media": {"duration": 20.02},
        }]});
        let status = MediaStatus::first(&payload).unwrap();
        assert_eq!(status.idle_reason.as_deref(), Some("FINISHED"));
        assert_eq!(status.duration, Some(20.02));
    }

    #[test]
    fn a_fixed_volume_receiver_offers_mute_alone() {
        let payload = json!({"type": "RECEIVER_STATUS", "status": {
            "volume": {"controlType": "fixed", "level": 1.0, "muted": false},
            "applications": [{"appId": "E8C28D3C", "isIdleScreen": true, "sessionId": "a",
                              "transportId": "a"}]}});
        let status = ReceiverStatus::read(&payload).unwrap();
        assert_eq!(status.volume, VolumeControl::MuteOnly);
        assert_eq!(status.app, None, "the idle screen is not an app");
        let master = json!({"status": {"volume": {"controlType": "master", "level": 0.3}}});
        assert_eq!(ReceiverStatus::read(&master).unwrap().volume, VolumeControl::Level);
    }
}
