//! The receiver and media namespaces over a [`Channel`]: launch the Default Media Receiver, load
//! a URL, play, pause, seek, stop, and read what it reports.

use std::net::SocketAddr;

use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::channel::{CastError, Channel, Incoming, NS_MEDIA, NS_RECEIVER, RECEIVER};

/// Google's Default Media Receiver: needs no registration and plays HLS, DASH and progressive
/// MP4 through the Cast web player.
pub const DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";

/// A connected device, through its platform receiver.
pub struct Device {
    pub channel: Channel,
}

/// An app running on the device, as a sender talks to it.
#[derive(Debug, Clone)]
pub struct App {
    pub app_id: String,
    pub session_id: String,
    pub transport_id: String,
}

impl Device {
    pub async fn connect(
        address: SocketAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Incoming>), CastError> {
        let (channel, events) = Channel::connect(address).await?;
        Ok((Self { channel }, events))
    }

    /// The device's `RECEIVER_STATUS`: volume, and the app running, if any.
    pub async fn status(&self) -> Result<Value, CastError> {
        self.channel.request(RECEIVER, NS_RECEIVER, json!({"type": "GET_STATUS"})).await
    }

    /// The app running on the device now, other than its idle screen.
    pub async fn running(&self) -> Result<Option<App>, CastError> {
        Ok(running_app(&self.status().await?, None))
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
            None => running_app(&self.status().await?, Some(app_id))
                .ok_or_else(|| CastError::Protocol(format!("{app_id} did not start")))?,
        };
        self.channel.connect_to(&app.transport_id)?;
        Ok(app)
    }

    /// Stop the app `session_id` names, which ends any media in it.
    pub async fn stop(&self, app: &App) -> Result<(), CastError> {
        self.channel
            .request(RECEIVER, NS_RECEIVER, json!({"type": "STOP", "sessionId": app.session_id}))
            .await
            .map(|_| ())
    }

    pub async fn set_volume(&self, level: Option<f64>, muted: Option<bool>) -> Result<(), CastError> {
        let mut volume = json!({});
        if let Some(level) = level {
            volume["level"] = json!(level);
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

fn running_app(status: &Value, want: Option<&str>) -> Option<App> {
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
#[derive(Debug, Clone)]
pub struct TextTrack {
    pub id: u32,
    pub url: String,
    pub name: String,
    pub language: String,
}

/// What to load, as the `LOAD` message's media.
#[derive(Debug, Clone, Default)]
pub struct Load {
    pub url: String,
    pub content_type: String,
    /// `ts`, `ts_aac` or `fmp4`, for HLS: the receiver otherwise guesses from the first segment.
    pub hls_segment_format: Option<String>,
    /// `mpeg2_ts` or `fmp4`.
    pub hls_video_segment_format: Option<String>,
    pub title: String,
    pub start: Option<f64>,
    pub tracks: Vec<TextTrack>,
    pub active_tracks: Vec<u32>,
    pub autoplay: bool,
}

impl Load {
    pub fn message(&self, session_id: &str) -> Value {
        let mut media = json!({
            "contentId": self.url,
            "contentUrl": self.url,
            "streamType": "BUFFERED",
            "contentType": self.content_type,
            "metadata": {"metadataType": 0, "title": self.title},
        });
        if let Some(format) = &self.hls_segment_format {
            media["hlsSegmentFormat"] = json!(format);
        }
        if let Some(format) = &self.hls_video_segment_format {
            media["hlsVideoSegmentFormat"] = json!(format);
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
            // Plain white on a soft shadow, close to the app's own subtitle default.
            media["textTrackStyle"] = json!({
                "backgroundColor": "#00000000",
                "foregroundColor": "#FFFFFFFF",
                "edgeType": "DROP_SHADOW",
                "edgeColor": "#000000C0",
                "fontScale": 1.0,
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

    pub async fn command(&self, kind: &str, extra: Value) -> Result<Option<MediaStatus>, CastError> {
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
        self.command("SEEK", json!({"currentTime": seconds})).await
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

    pub async fn status(&self) -> Result<Option<MediaStatus>, CastError> {
        let mut payload = json!({"type": "GET_STATUS"});
        if let Some(id) = self.media_session_id {
            payload["mediaSessionId"] = json!(id);
        }
        Ok(MediaStatus::first(&self.ask(payload).await?))
    }
}

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
    /// The receiver's own words for a failure, newer firmware only.
    pub error: Option<String>,
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
                .map(|ids| ids.iter().filter_map(|id| id.as_u64().map(|id| id as u32)).collect())
                .unwrap_or_default(),
            volume: status["volume"]["level"].as_f64(),
            muted: status["volume"]["muted"].as_bool(),
            error: status["extendedStatus"]["playerState"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| payload["detailedErrorCode"].as_i64().map(|c| c.to_string())),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_load_names_its_tracks_and_start() {
        let load = Load {
            url: "http://192.0.2.1:1/c/t/f/a.m3u8".into(),
            content_type: "application/x-mpegurl".into(),
            hls_segment_format: Some("fmp4".into()),
            hls_video_segment_format: Some("fmp4".into()),
            title: "Test".into(),
            start: Some(42.0),
            tracks: vec![TextTrack {
                id: 1,
                url: "http://192.0.2.1:1/c/t/f/a.vtt".into(),
                name: "English".into(),
                language: "en".into(),
            }],
            active_tracks: vec![1],
            autoplay: true,
        };
        let message = load.message("s");
        assert_eq!(message["currentTime"], 42.0);
        assert_eq!(message["activeTrackIds"], json!([1]));
        assert_eq!(message["media"]["tracks"][0]["trackContentType"], "text/vtt");
        assert_eq!(message["media"]["hlsSegmentFormat"], "fmp4");
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
}
