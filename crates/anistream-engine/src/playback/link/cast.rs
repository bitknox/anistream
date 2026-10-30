//! A TV with Chromecast built in as the player a playback runs in.
//!
//! The session drives a cast as it drives mpv: it loads a file, sets `pause` and `speed`, seeks,
//! and reads what happened from [`PlaybackEvent`]s. [`CastLink`] turns that into the Cast
//! protocol through [`CastPlayer`], which chooses how the TV reads the stream, tries the ways in
//! turn, and remembers what each TV played. What a TV has no use for, the upscaling chain among
//! it, is taken and dropped. The overlays the sender's card draws in the TV's place, Up Next, the
//! skip on offer and the Undo note, go to the [`CastHub`].

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
};

use anistream_cast::{
    CastDevice, CastFailure, CastLoad, CastPlayer, CastSetup, CastSource, CastSubtitle,
    Metadata, SourceKind, Uncastable,
};
use anistream_core::{
    stream::{Stream, StreamKind},
    traits::PlaybackRequest,
};
use anistream_model::Msg;
use async_trait::async_trait;

use super::{FileFacts, LinkError, Opened, Overlay, PlayerLink, PlayerOpener, PlayerTarget};
use crate::cast::CastHub;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn failed(e: impl std::fmt::Display) -> LinkError {
    LinkError(e.to_string())
}

/// What opens a cast to one TV for a playback.
pub struct CastOpener {
    hub: Arc<CastHub>,
    device: CastDevice,
    scratch: PathBuf,
    /// Why the TV couldn't be reached, once it couldn't.
    unreached: Mutex<Option<CastFailure>>,
}

impl CastOpener {
    /// A cast to `device`, with its scratch files, the subtitles fetched ahead, in `scratch`.
    pub fn new(hub: Arc<CastHub>, device: CastDevice, scratch: PathBuf) -> Self {
        Self { hub, device, scratch, unreached: Mutex::new(None) }
    }

    pub fn device(&self) -> &CastDevice {
        &self.device
    }
}

#[async_trait]
impl PlayerOpener for CastOpener {
    /// Connect to the TV and open the relay it will read from. Nothing shows on the TV until the
    /// first file loads, so a stream it can't play never takes it over.
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError> {
        // A TV this cast couldn't reach isn't tried again for the play's next stream.
        if let Some(failure) = lock(&self.unreached).clone() {
            return Err(LinkError(failure.to_string()));
        }
        let setup = CastSetup {
            device: self.device.clone(),
            memory: self.hub.memory(),
            upstream: self.hub.upstream(),
            encoder: self.hub.encoder(),
            timings: self.hub.timings(),
        };
        let (player, events) = match CastPlayer::connect(setup).await {
            Ok(opened) => opened,
            Err(failure) => {
                // The card shows why, and the casting ends: no player ever ran to end it.
                tracing::warn!(%failure, "couldn't reach the TV");
                *lock(&self.unreached) = Some(failure.clone());
                self.hub.failed(failure.clone());
                return Err(LinkError(failure.to_string()));
            }
        };
        let player = Arc::new(player);
        self.hub.attach(&player);
        if let Some(speed) = request.speed.filter(|s| (*s - 1.0).abs() > f64::EPSILON) {
            let _ = player.set_rate(speed).await;
        }
        let link = CastLink {
            player,
            device: self.device.id.clone(),
            name: self.device.name.clone(),
            hub: Arc::clone(&self.hub),
            file: Mutex::new(None),
        };
        Ok((Box::new(link), events))
    }

    fn scratch_dir(&self) -> &Path {
        &self.scratch
    }

    fn target(&self) -> PlayerTarget {
        PlayerTarget::Cast(self.device.id.clone())
    }

    fn open_failure_notice(&self) -> Option<Msg> {
        lock(&self.unreached).as_ref().map(|failure| notice(failure, &self.device.name))
    }
}

/// The session's link to a cast.
pub struct CastLink {
    player: Arc<CastPlayer>,
    /// The TXT `id` of the TV.
    device: String,
    /// The TV's own name, which the notices say.
    name: String,
    hub: Arc<CastHub>,
    /// What the next load plays, from [`PlayerLink::prepare`].
    file: Mutex<Option<FileFacts>>,
}

impl CastLink {
    pub fn player(&self) -> &Arc<CastPlayer> {
        &self.player
    }
}

/// What a cast to the TV called `name` that failed tells the viewer.
pub fn notice(failure: &CastFailure, name: &str) -> Msg {
    let name = name.to_owned();
    match failure {
        CastFailure::Uncastable(Uncastable::VideoCodec(codec)) => {
            Msg::NoticeCastVideoCodec { name, codec: codec.clone() }
        }
        CastFailure::Uncastable(Uncastable::AudioCodec(codec)) => {
            Msg::NoticeCastAudioCodec { name, codec: codec.clone() }
        }
        CastFailure::Uncastable(Uncastable::Container(_)) => Msg::NoticeCastContainer { name },
        CastFailure::Uncastable(Uncastable::DeepLink) => Msg::NoticeCastDeepLink,
        CastFailure::NeverFetches => Msg::NoticeCastNeverFetches { name },
        CastFailure::Refused => Msg::NoticeCastRefused { name },
        CastFailure::TakenOver => Msg::NoticeCastTakenOver { name },
        CastFailure::NotSameNetwork => Msg::NoticeCastNotSameNetwork { name },
        CastFailure::Lost(_) => Msg::NoticeCastLost { name },
    }
}

/// How a stream reaches the TV, as the cast's plans need to know it.
pub fn source_of(stream: &Stream) -> CastSource {
    let kind = match stream.kind {
        StreamKind::Hls => SourceKind::Hls,
        StreamKind::TorrentHttp => SourceKind::Torrent,
        StreamKind::ExternalDeepLink => SourceKind::DeepLink,
        StreamKind::Mp4 if !stream.url.contains("://") => SourceKind::File,
        StreamKind::Mp4 => SourceKind::Progressive,
    };
    CastSource { url: stream.url.clone(), kind, headers: stream.headers.clone() }
}

/// The external subtitles of `stream` that are on disk, the preferred language first, and which
/// of them to show: the first, unless a dub watcher has the audio in their language.
fn subtitles(
    stream: &Stream,
    language: Option<&str>,
    dub: bool,
) -> (Vec<CastSubtitle>, Option<usize>) {
    let tracks: Vec<CastSubtitle> =
        anistream_playback::protocol::subtitle_order(&stream.subtitles, language)
            .into_iter()
            .filter_map(|s| {
                let path = Path::new(s.url.strip_prefix("file://").unwrap_or(&s.url));
                (!s.hard && path.is_file()).then(|| {
                    let language = (!s.language.is_empty()).then(|| s.language.clone());
                    CastSubtitle { path: path.to_owned(), title: language.clone(), language }
                })
            })
            .collect();
    let on = (!tracks.is_empty() && !dub).then_some(0);
    (tracks, on)
}

/// The episode line the TV shows under the show's name, in the front end's language.
fn episode_line(file: &FileFacts, language: Option<&str>) -> String {
    let line = match file.episode_title.as_deref().filter(|t| !t.is_empty()) {
        Some(title) => {
            Msg::CastEpisodeTitled { episode: file.episode.clone(), title: title.to_owned() }
        }
        None => Msg::CastEpisode { episode: file.episode.clone() },
    };
    match language {
        Some(tag) => line.text_in(tag),
        None => line.english(),
    }
}

/// The resume point from mpv's per-file options.
fn start_of(options: &[(String, String)]) -> Option<f64> {
    options.iter().find(|(k, _)| k == "start").and_then(|(_, v)| v.parse().ok())
}

fn number(value: &serde_json::Value) -> Option<f64> {
    value.as_f64().or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

#[async_trait]
impl PlayerLink for CastLink {
    fn target(&self) -> PlayerTarget {
        PlayerTarget::Cast(self.device.clone())
    }

    fn prepare(&self, file: &FileFacts) {
        *lock(&self.file) = Some(file.clone());
    }

    fn subtitles_with_load(&self) -> bool {
        true
    }

    fn failure_notice(&self) -> Option<Msg> {
        self.player.failure().as_ref().map(|failure| notice(failure, &self.name))
    }

    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        let file = lock(&self.file).take();
        let load = match file {
            Some(file) => {
                let (subtitles, subtitle) =
                    subtitles(&file.played, file.subtitle_language.as_deref(), file.dub);
                CastLoad {
                    source: source_of(&file.source),
                    url: url.to_owned(),
                    headers: file.played.headers.clone(),
                    start: start_of(&options),
                    metadata: Metadata {
                        series: file.title.clone(),
                        title: episode_line(&file, self.hub.language().as_deref()),
                        image: file.image.clone(),
                    },
                    subtitles,
                    subtitle,
                    // As mpv's `alang`: the original Japanese for a sub watcher, their own
                    // language for a dub watcher.
                    audio_language: if file.dub {
                        file.subtitle_language.clone()
                    } else {
                        Some("ja".to_owned())
                    },
                    audio_track: None,
                }
            }
            // Not told what it is: played as a stream from its address alone.
            None => CastLoad {
                source: source_of(&Stream::new(url, StreamKind::Mp4)),
                url: url.to_owned(),
                headers: Vec::new(),
                start: start_of(&options),
                metadata: Metadata::default(),
                subtitles: Vec::new(),
                subtitle: None,
                audio_language: None,
                audio_track: None,
            },
        };
        self.player.load(load);
        Ok(())
    }

    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError> {
        let path = Path::new(url.strip_prefix("file://").unwrap_or(url));
        if !path.is_file() {
            // A TV reads subtitles from the relay, which serves only files on this device.
            return Ok(());
        }
        self.player.add_subtitle(path, title, language).await.map_err(failed)
    }

    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError> {
        let player = &self.player;
        match property {
            "pause" => {
                if let Some(paused) = value.as_bool() {
                    player.set_paused(paused).await.map_err(failed)?;
                }
            }
            "speed" => {
                if let Some(rate) = number(&value) {
                    player.set_rate(rate).await.map_err(failed)?;
                }
            }
            "volume" => {
                // mpv's 0 to 100, which is the scale the player takes too.
                if let Some(volume) = number(&value) {
                    player.set_volume(volume.clamp(0.0, 100.0)).await.map_err(failed)?;
                }
            }
            "mute" => {
                if let Some(muted) = value.as_bool() {
                    player.set_muted(muted).await.map_err(failed)?;
                }
            }
            "sid" => {
                // mpv's track ids count from one, as the TV's text tracks do.
                let id = value.as_u64().and_then(|id| u32::try_from(id).ok());
                player.set_subtitle(id).await.map_err(failed)?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn cycle(&self, property: &'static str) -> Result<(), LinkError> {
        match property {
            "pause" => self.player.toggle_pause().await.map_err(failed),
            "mute" => self.player.set_muted(!self.player.muted()).await.map_err(failed),
            _ => Ok(()),
        }
    }

    async fn toggle_fullscreen(&self) -> Result<(), LinkError> {
        Ok(())
    }

    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError> {
        match property {
            "volume" => {
                let level = (self.player.volume() + delta).clamp(0.0, 100.0);
                self.player.set_volume(level).await.map_err(failed)
            }
            "speed" => {
                let rate = (self.player.rate() + delta).clamp(0.5, 2.0);
                self.player.set_rate(rate).await.map_err(failed)
            }
            _ => Ok(()),
        }
    }

    async fn multiply(&self, property: &'static str, factor: f64) -> Result<(), LinkError> {
        if property == "speed" {
            let rate = (self.player.rate() * factor).clamp(0.5, 2.0);
            return self.player.set_rate(rate).await.map_err(failed);
        }
        Ok(())
    }

    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError> {
        let target =
            if absolute { seconds } else { self.player.position().unwrap_or(0.0) + seconds };
        self.hub.seeked(target);
        self.player.seek(seconds, absolute).await.map_err(failed)
    }

    async fn shaders(&self, _paths: &[String]) -> Result<(), LinkError> {
        Ok(())
    }

    async fn audio_filter(&self, _add: bool, _filter: &str) -> Result<(), LinkError> {
        Ok(())
    }

    async fn restate_chapters(&self) -> Result<(), LinkError> {
        Ok(())
    }

    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        self.hub.overlay(&overlay);
        Ok(())
    }

    async fn stop(&self) -> Result<(), LinkError> {
        self.player.stop().await.map_err(failed)
    }

    async fn quit(&self) -> Result<(), LinkError> {
        self.player.quit().await.map_err(failed)
    }

    fn is_alive(&self) -> bool {
        self.player.is_alive()
    }

    fn has_quit(&self) -> bool {
        self.player.has_quit()
    }

    async fn diagnostics(&self) -> Option<String> {
        self.player.failure().map(|failure| failure.to_string())
    }

    async fn shutdown(self: Box<Self>) {
        if let Err(e) = self.player.quit().await {
            tracing::debug!(error = %e, "the TV didn't take the stop");
        }
        // A cast that ended for good ends the casting; one parked for the next episode is
        // not shut down.
        if self.player.failure().is_some_and(|f| {
            matches!(
                f,
                CastFailure::TakenOver | CastFailure::Lost(_) | CastFailure::NotSameNetwork
            )
        }) {
            self.hub.end();
        }
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::stream::Subtitle;

    use super::*;

    fn subtitle(language: &str, url: String, hard: bool) -> Subtitle {
        Subtitle { language: language.into(), url, hard, format: None }
    }

    #[test]
    fn a_stream_is_read_as_the_cast_plans_need() {
        let kind = |url: &str, kind| source_of(&Stream::new(url, kind)).kind;
        assert_eq!(kind("https://cdn.example/a.m3u8", StreamKind::Hls), SourceKind::Hls);
        assert_eq!(kind("http://127.0.0.1:9/a", StreamKind::TorrentHttp), SourceKind::Torrent);
        assert_eq!(kind("app://open", StreamKind::ExternalDeepLink), SourceKind::DeepLink);
        assert_eq!(kind("/downloads/1.mkv", StreamKind::Mp4), SourceKind::File);
        assert_eq!(kind("https://cdn.example/1.mp4", StreamKind::Mp4), SourceKind::Progressive);
        let mut stream = Stream::new("https://cdn.example/1.mp4", StreamKind::Mp4);
        stream.headers = vec![("Referer".into(), "https://site.example/".into())];
        assert_eq!(
            source_of(&stream).headers,
            stream.headers,
            "the relay sends what the source wants"
        );
    }

    #[test]
    fn only_subtitles_on_disk_go_to_the_tv_and_a_dub_watcher_starts_without() {
        let dir = tempfile::tempdir().unwrap();
        let on_disk = |name: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, "WEBVTT\n").unwrap();
            path.to_string_lossy().into_owned()
        };
        let mut stream = Stream::new("/downloads/1.mkv", StreamKind::Mp4);
        stream.subtitles = vec![
            subtitle("fr", on_disk("1.fr.ass"), false),
            subtitle("en", format!("file://{}", on_disk("1.en.ass")), false),
            subtitle("de", "https://subs.example/1.de.vtt".into(), false),
            subtitle("es", on_disk("1.es.ass"), true),
        ];

        let (tracks, on) = subtitles(&stream, Some("en"), false);
        let languages: Vec<_> = tracks.iter().map(|t| t.language.as_deref()).collect();
        assert_eq!(languages, [Some("en"), Some("fr")], "the preferred language first");
        assert!(tracks[0].path.is_file(), "file:// is a path on disk");
        assert_eq!(on, Some(0));

        let (tracks, on) = subtitles(&stream, Some("en"), true);
        assert_eq!(tracks.len(), 2);
        assert_eq!(on, None);
        assert_eq!(
            subtitles(&Stream::new("/x.mkv", StreamKind::Mp4), None, false),
            (vec![], None)
        );
    }

    #[test]
    fn the_tv_shows_the_episode_and_the_resume_point_is_read_from_mpvs_options() {
        let stream = Stream::new("/downloads/1.mkv", StreamKind::Mp4);
        let mut file = FileFacts {
            source: stream.clone(),
            played: stream,
            title: "Harbour Lights".into(),
            episode: "4".into(),
            episode_title: None,
            image: None,
            subtitle_language: None,
            dub: false,
        };
        assert_eq!(episode_line(&file), "Episode 4");
        file.episode_title = Some(String::new());
        assert_eq!(episode_line(&file), "Episode 4");
        file.episode_title = Some("Low Tide".into());
        assert_eq!(episode_line(&file), "Episode 4 · Low Tide");

        let options = |start: &str| {
            vec![("sid".to_owned(), "1".to_owned()), ("start".to_owned(), start.to_owned())]
        };
        assert_eq!(start_of(&options("312.5")), Some(312.5));
        assert_eq!(start_of(&options("later")), None);
        assert_eq!(start_of(&[]), None);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mpvs_volume_reaches_the_tv_on_its_own_scale() {
        use std::time::Duration;

        use anistream_cast::{
            CastMemory, NoEncoder, PlainHttp, Timings,
            fake::{FakeOptions, FakeReceiver},
        };

        let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        };
        let hub = Arc::new(CastHub::with(None, Arc::new(PlainHttp), Timings::default()));
        let (player, _events) = CastPlayer::connect(CastSetup {
            device,
            memory: Arc::new(CastMemory::memory()),
            upstream: Arc::new(PlainHttp),
            encoder: Arc::new(NoEncoder),
            timings: Timings::default(),
        })
        .await
        .unwrap();
        let link = CastLink {
            player: Arc::new(player),
            device: "tv-1".into(),
            hub,
            file: Mutex::new(None),
        };
        let levels = || -> Vec<f64> {
            receiver
                .heard("SET_VOLUME")
                .iter()
                .filter_map(|m| m["volume"]["level"].as_f64())
                .collect()
        };
        let settle = |count: usize| {
            let levels = &levels;
            let player = Arc::clone(&link.player);
            async move {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
                while levels().len() < count || player.volume() != levels()[count - 1] * 100.0 {
                    assert!(tokio::time::Instant::now() < deadline, "the TV took the volume");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        };

        // mpv's `volume` is 0 to 100, and so is what the TV is set to, in hundredths.
        link.set("volume", serde_json::json!(70)).await.unwrap();
        settle(1).await;
        link.add("volume", -20.0).await.unwrap();
        settle(2).await;
        let levels = levels();
        assert!((levels[0] - 0.7).abs() < 1e-9, "{levels:?}");
        assert!((levels[1] - 0.5).abs() < 1e-9, "{levels:?}");
    }

    #[tokio::test]
    async fn a_tv_that_cant_be_reached_ends_the_cast_and_says_why() {
        use anistream_cast::{PlainHttp, Timings};

        // A loopback port nothing listens on.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let device = CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: closed,
            video: true,
            group: false,
        };
        let hub = Arc::new(CastHub::with(None, Arc::new(PlainHttp), Timings::default()));
        hub.begin(device.clone());
        let dir = tempfile::tempdir().unwrap();
        let opener = CastOpener::new(Arc::clone(&hub), device, dir.path().to_owned());
        let request = PlaybackRequest::default();
        assert!(opener.open(&request).await.is_err());

        assert_eq!(
            opener.open_failure_notice(),
            Some(Msg::NoticeCastLost { name: "Living room".into() })
        );
        assert!(!hub.casting(), "the next play opens on this device");
        let view = hub.view();
        assert!(view.over, "the card isn't left starting");
        assert!(matches!(view.status.and_then(|s| s.failure), Some(CastFailure::Lost(_))));
        // The play's next stream doesn't wait on the TV again.
        let again = tokio::time::Instant::now();
        assert!(opener.open(&request).await.is_err());
        assert!(again.elapsed() < std::time::Duration::from_millis(100));
    }

    #[test]
    fn every_failure_says_why_naming_the_tv() {
        let codec = |c: &str| CastFailure::Uncastable(Uncastable::AudioCodec(c.into()));
        let name = || "Living room".to_owned();
        assert_eq!(
            notice(&codec("AC-3"), "Living room"),
            Msg::NoticeCastAudioCodec { name: name(), codec: "AC-3".into() }
        );
        assert_eq!(
            notice(&CastFailure::Lost("reset".into()), "Living room").english(),
            "Lost Living room, so casting stopped."
        );
        assert_eq!(
            notice(&CastFailure::NeverFetches, "Living room").english(),
            "Living room didn't start reading the stream. A VPN or a firewall on this device \
             may be in the way."
        );
        for failure in [
            CastFailure::Refused,
            CastFailure::TakenOver,
            CastFailure::NotSameNetwork,
            CastFailure::Uncastable(Uncastable::VideoCodec("HEVC".into())),
            CastFailure::Uncastable(Uncastable::Container("AVI".into())),
        ] {
            assert!(notice(&failure, "Living room").english().contains("Living room"));
        }
    }

    #[test]
    fn the_tv_shows_the_episode_line_in_the_front_end_s_language() {
        let file = FileFacts {
            source: Stream::new("https://cdn.example/1.m3u8", StreamKind::Hls),
            played: Stream::new("https://cdn.example/1.m3u8", StreamKind::Hls),
            title: "Stardust Wayfarer".into(),
            episode: "3".into(),
            episode_title: Some("Low Tide".into()),
            image: None,
            subtitle_language: None,
            dub: false,
        };
        assert_eq!(episode_line(&file, None), "Episode 3 · Low Tide");
        // A language the build lacks reads as English rather than failing.
        assert_eq!(episode_line(&file, Some("xx")), "Episode 3 · Low Tide");
    }
}
