//! A cast against the fake receiver on loopback, from connect to the end of the file.

use std::{collections::HashMap, time::Duration};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
};

use super::*;
use crate::{
    aac::{FakeEncoder, NoEncoder},
    fake::{
        FakeOptions, FakeReceiver, Refusal, Refuse,
        matroska::{Fixture, FixtureAudio, matroska},
    },
    fetch::PlainHttp,
};

/// A loopback server answering each path with its body, ranges and all, without CORS: what the
/// mender and the torrent server look like from here.
async fn serve(files: HashMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let files = files.clone();
            tokio::spawn(async move {
                let mut reader = BufReader::new(socket);
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    return;
                }
                let path = line.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let mut range = None;
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).await.unwrap_or(0) == 0
                        || header.trim().is_empty()
                    {
                        break;
                    }
                    if let Some(value) =
                        header.to_ascii_lowercase().strip_prefix("range: bytes=")
                    {
                        let (a, b) = value.trim().split_once('-').unwrap();
                        range = Some((a.parse::<usize>().unwrap(), b.parse::<usize>().ok()));
                    }
                }
                let mut socket = reader.into_inner();
                let Some(body) = files.get(&path) else {
                    let _ = socket
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                        .await;
                    return;
                };
                let kind = if path.ends_with(".m3u8") {
                    "application/vnd.apple.mpegurl"
                } else {
                    "video/x-matroska"
                };
                let (status, part, extra) = match range {
                    Some((a, b)) => {
                        let b = b.unwrap_or(body.len() - 1).min(body.len() - 1);
                        (
                            "206 Partial Content",
                            &body[a..=b],
                            format!("Content-Range: bytes {a}-{b}/{}\r\n", body.len()),
                        )
                    }
                    None => ("200 OK", &body[..], String::new()),
                };
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {kind}\r\nAccept-Ranges: bytes\r\n{extra}Content-Length: {}\r\n\r\n",
                    part.len()
                );
                let _ = socket.write_all(head.as_bytes()).await;
                let _ = socket.write_all(part).await;
            });
        }
    });
    format!("http://{address}")
}

fn timings() -> Timings {
    Timings {
        poll: Duration::from_millis(100),
        movement: Duration::from_millis(1500),
        fetch: Duration::from_secs(2),
        idle_close: Duration::from_secs(600),
    }
}

fn setup(
    receiver: &FakeReceiver,
    memory: Arc<CastMemory>,
    encoder: Arc<dyn AacEncoder>,
) -> CastSetup {
    CastSetup {
        device: CastDevice {
            instance: "Chromecast-test".into(),
            id: "tv-1".into(),
            name: "Living room".into(),
            model: "Chromecast".into(),
            address: receiver.address,
            video: true,
            group: false,
        },
        memory,
        upstream: Arc::new(PlainHttp),
        encoder,
        timings: timings(),
    }
}

fn torrent(url: String) -> CastLoad {
    CastLoad {
        source: CastSource { url: url.clone(), kind: SourceKind::Torrent, headers: Vec::new() },
        url,
        headers: Vec::new(),
        start: None,
        metadata: Metadata {
            series: "A Made-up Show".into(),
            title: "Episode 1".into(),
            image: None,
        },
        subtitles: Vec::new(),
        subtitle: None,
        audio_language: Some("ja".into()),
    }
}

async fn next(
    rx: &mut mpsc::UnboundedReceiver<PlaybackEvent>,
    want: impl Fn(&PlaybackEvent) -> bool,
) -> PlaybackEvent {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_torrents_matroska_is_remuxed_with_aac_and_driven_as_mpv_is() {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 8.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let encoder = FakeEncoder::default();
    let (player, mut events) =
        CastPlayer::connect(setup(&receiver, Arc::clone(&memory), Arc::new(encoder)))
            .await
            .unwrap();
    assert!(receiver.heard("LAUNCH").is_empty(), "nothing starts on the TV before a load");

    let dir = tempfile::tempdir().unwrap();
    let subtitles = dir.path().join("episode.ass");
    std::fs::write(
        &subtitles,
        "[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
         Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n",
    )
    .unwrap();
    let mut load = torrent(format!("{base}/episode.mkv"));
    load.start = Some(4.0);
    load.subtitles = vec![CastSubtitle {
        path: subtitles,
        title: Some("English".into()),
        language: Some("en".into()),
    }];
    load.subtitle = Some(0);
    player.load(load);
    assert_eq!(next(&mut events, |e| *e == PlaybackEvent::Loaded).await, PlaybackEvent::Loaded);

    let sent = &receiver.heard("LOAD")[0];
    assert_eq!(sent["media"]["contentType"], "application/x-mpegurl");
    assert_eq!(sent["media"]["hlsSegmentFormat"], "fmp4");
    assert_eq!(sent["media"]["metadata"]["seriesTitle"], "A Made-up Show");
    assert_eq!(sent["media"]["tracks"][0]["trackContentType"], "text/vtt");
    assert_eq!(sent["activeTrackIds"], serde_json::json!([1]));
    assert_eq!(sent["currentTime"], 4.0);
    let fetched = receiver.fetched();
    assert!(fetched.iter().all(|f| f.cors && f.status == 200), "{fetched:?}");
    assert_eq!(fetched.len(), 3, "playlist, init, first segment");
    let plan = player.status().borrow().plan.clone().unwrap();
    assert_eq!(plan.audio, AudioPlan::ToAac { from: crate::formats::AudioCodec::Ac3 });
    assert!(memory.caps("tv-1").played("hls-fmp4"));

    player.toggle_pause().await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;
    player.set_paused(false).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(false)).await;
    player.set_rate(1.5).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;
    player.seek(15.0, true).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Seeking(false)).await;
    // The position keeps moving between changes because it is asked for.
    let PlaybackEvent::Progress { position, .. } = next(
        &mut events,
        |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position > 16.0),
    )
    .await
    else {
        unreachable!()
    };
    assert!(position <= 20.0);
    assert_eq!(
        next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. })).await,
        PlaybackEvent::Ended { complete: true }
    );
    player.quit().await.unwrap();
    assert!(player.has_quit() && !player.is_alive());
    assert!(
        receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()),
        "the app stopped"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_codec_falls_back_and_the_next_cast_starts_with_what_plays() {
    let mut opus = FixtureAudio::aac();
    opus.codec = "A_OPUS";
    opus.private = b"OpusHead\x01\x02\x38\x01\x80\xbb\x00\x00\x00\x00\x00".to_vec();
    opus.frame_ms = 20;
    let file = matroska(&Fixture { audio: Some(opus), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions {
        speed: 4.0,
        refuse: Refuse {
            bytes: vec![b"Opus".to_vec()],
            content_types: Vec::new(),
            how: Some(Refusal::IdleError),
        },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) = CastPlayer::connect(setup(
        &receiver,
        Arc::clone(&memory),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();

    // Opus copied, remuxed and then as the file is: both refused, and Opus is to blame.
    player.load(torrent(format!("{base}/episode.mkv")));
    assert_eq!(
        next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. } | PlaybackEvent::Loaded))
            .await,
        PlaybackEvent::Ended { complete: false }
    );
    assert_eq!(player.failure(), Some(CastFailure::Refused));
    assert_eq!(receiver.heard("LOAD").len(), 2);
    assert!(memory.caps("tv-1").refused("opus"));
    assert!(!memory.caps("tv-1").refused("hls-fmp4"));

    // The next cast turns the Opus to AAC from the start.
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD").len(), 3);
    let plan = player.status().borrow().plan.clone().unwrap();
    assert_eq!(plan.audio, AudioPlan::ToAac { from: crate::formats::AudioCodec::Opus });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_file_the_remuxer_cant_read_is_passed_through() {
    let fixture =
        Fixture { cues: false, audio: Some(FixtureAudio::aac()), ..Fixture::default() };
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("episode.mkv");
    std::fs::write(&path, matroska(&fixture)).unwrap();
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        CastPlayer::connect(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let url = path.to_string_lossy().into_owned();
    let mut load = torrent(url.clone());
    load.source = CastSource { url, kind: SourceKind::File, headers: Vec::new() };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = receiver.heard("LOAD");
    assert_eq!(sent.len(), 1, "the remux plan never reached the TV");
    assert_eq!(sent[0]["media"]["contentType"], "video/x-matroska");
    assert!(memory.caps("tv-1").played("mkv"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hls_behind_the_mender_is_relayed_with_cors() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\nseg1.ts\n#EXT-X-ENDLIST\n";
    let mut segment = vec![0u8; 188 * 4];
    for packet in segment.chunks_mut(188) {
        packet[0] = 0x47;
    }
    let base = serve(HashMap::from([
        ("/a/index.m3u8".to_owned(), playlist.as_bytes().to_vec()),
        ("/a/seg0.ts".to_owned(), segment.clone()),
        ("/a/seg1.ts".to_owned(), segment),
    ]))
    .await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        CastPlayer::connect(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let url = format!("{base}/a/index.m3u8");
    let load = CastLoad {
        source: CastSource {
            url: url.clone(),
            kind: SourceKind::Hls,
            headers: vec![("Referer".into(), "https://player.example/".into())],
        },
        ..torrent(url)
    };
    player.load(load);
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let fetched = receiver.fetched();
    assert_eq!(fetched.len(), 2);
    assert!(fetched.iter().all(|f| f.cors && f.url.contains("/u/")), "{fetched:?}");
    assert_eq!(receiver.heard("LOAD")[0]["media"]["hlsVideoSegmentFormat"], "mpeg2_ts");
    assert_eq!(
        next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. })).await,
        PlaybackEvent::Ended { complete: true }
    );
    assert!(memory.caps("tv-1").played("hls-ts"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_uncastable_stream_never_takes_the_tv_over() {
    let base =
        serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&Fixture::default()))]))
            .await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) = CastPlayer::connect(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(NoEncoder),
    ))
    .await
    .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::AudioCodec("AC-3".into())))
    );
    assert!(receiver.heard("LAUNCH").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_never_fetches_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver =
        FakeReceiver::start(FakeOptions { fetches: false, ..FakeOptions::default() })
            .await
            .unwrap();
    let (player, mut events) = CastPlayer::connect(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(NoEncoder),
    ))
    .await
    .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::NeverFetches));
    assert_eq!(receiver.heard("LOAD").len(), 1, "no second plan: none would fare better");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_sender_taking_over_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) = CastPlayer::connect(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(NoEncoder),
    ))
    .await
    .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    receiver.take_over();
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert_eq!(player.failure(), Some(CastFailure::TakenOver));
    assert!(!player.is_alive());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_with_fixed_volume_only_mutes_and_a_long_pause_ends_the_cast() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver =
        FakeReceiver::start(FakeOptions { control: "fixed", ..FakeOptions::default() })
            .await
            .unwrap();
    let mut setup = setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder));
    setup.timings.idle_close = Duration::from_millis(800);
    let (player, mut events) = CastPlayer::connect(setup).await.unwrap();
    assert_eq!(player.volume_control(), VolumeControl::MuteOnly);
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;

    player.set_volume(30.0).await.unwrap();
    assert!(receiver.heard("SET_VOLUME").is_empty(), "the TV's own volume is in charge");
    player.set_muted(true).await.unwrap();
    assert_eq!(receiver.heard("SET_VOLUME")[0]["volume"]["muted"], true);

    player.set_paused(true).await.unwrap();
    next(&mut events, |e| *e == PlaybackEvent::Paused(true)).await;
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    // The media's own stop ends the file a moment before the app is closed.
    let closed = async {
        while !(receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string())
            && !player.is_alive())
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(5), closed).await.expect("the app closed");
}

#[test]
fn subtitles_become_webvtt() {
    let srt =
        "1\n00:00:01,000 --> 00:00:02,500\nHello\n\n2\n00:00:03,000 --> 00:00:04,000\nAgain\n";
    let vtt = to_webvtt(srt, Some("srt"));
    assert!(vtt.starts_with("WEBVTT\n\n"));
    assert!(vtt.contains("00:00:01.000 --> 00:00:02.500\nHello"));
    assert!(!vtt.contains("\n1\n"));
    assert_eq!(to_webvtt("WEBVTT\n\nx", None), "WEBVTT\n\nx");
}
