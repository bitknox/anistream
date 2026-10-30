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
    serve_slowly(files, HashMap::new()).await
}

/// [`serve`], with each path in `delays` sending its body only after its delay, its head at
/// once: a torrent's pieces not there yet, or a slow CDN.
async fn serve_slowly(
    files: HashMap<String, Vec<u8>>,
    delays: HashMap<String, Duration>,
) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let files = files.clone();
            let delays = delays.clone();
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
                if let Some(delay) = delays.get(&path) {
                    tokio::time::sleep(*delay).await;
                }
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
        ..Timings::default()
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
        audio_track: None,
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

fn opus() -> FixtureAudio {
    let mut opus = FixtureAudio::aac();
    opus.codec = "A_OPUS";
    opus.private = b"OpusHead\x01\x02\x38\x01\x80\xbb\x00\x00\x00\x00\x00".to_vec();
    opus.frame_ms = 20;
    opus
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_refused_codec_falls_back_to_aac_and_the_next_cast_starts_there() {
    let file = matroska(&Fixture { audio: Some(opus()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    // However the TV says no: idle with an error, an `ERROR` message, or a stall.
    for how in [Refusal::IdleError, Refusal::Error, Refusal::Stall] {
        let receiver = FakeReceiver::start(FakeOptions {
            speed: 4.0,
            refuse: Refuse {
                bytes: vec![b"Opus".to_vec()],
                how: Some(how),
                ..Refuse::default()
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

        // Opus copied is refused, and the same load goes on with the Opus turned to AAC.
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        assert_eq!(receiver.heard("LOAD").len(), 2, "{how:?}");
        let plan = player.status().borrow().plan.clone().unwrap();
        assert_eq!(plan.audio, AudioPlan::ToAac { from: crate::formats::AudioCodec::Opus });
        assert!(memory.caps("tv-1").refused("opus/hls"), "{how:?}");
        assert!(!memory.caps("tv-1").refused("hls-fmp4"));

        // The next cast turns the Opus to AAC from the start.
        player.load(torrent(format!("{base}/episode.mkv")));
        next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
        assert_eq!(receiver.heard("LOAD").len(), 3);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_stops_hls_at_720_lines_plays_a_1080_line_file_passed_through() {
    let tall = Fixture {
        audio: Some(FixtureAudio::aac()),
        width: 1920,
        height: 1080,
        ..Fixture::default()
    };
    let short = Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() };
    let base = serve(HashMap::from([
        ("/1080.mkv".to_owned(), matroska(&tall)),
        ("/360.mkv".to_owned(), matroska(&short)),
    ]))
    .await;
    let receiver = FakeReceiver::start(FakeOptions {
        speed: 4.0,
        refuse: Refuse { hls_taller_than: Some(720), ..Refuse::default() },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        CastPlayer::connect(setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();

    // The remux is refused for its height, and the file as it is plays.
    player.load(torrent(format!("{base}/1080.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = receiver.heard("LOAD");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1]["media"]["contentType"], "video/x-matroska");
    let caps = memory.caps("tv-1");
    assert!(caps.refused("h264/1080/hls"));
    assert!(!caps.refused("hls-fmp4"), "HLS itself still plays");
    assert!(caps.played("mkv"));

    // The next 1080-line file goes straight to the file; a smaller one is still remuxed.
    player.load(torrent(format!("{base}/1080.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD").len(), 3);
    assert_eq!(receiver.heard("LOAD")[2]["media"]["contentType"], "video/x-matroska");
    player.load(torrent(format!("{base}/360.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(receiver.heard("LOAD")[3]["media"]["contentType"], "application/x-mpegurl");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_file_whose_picture_a_tv_may_lack_is_asked_about_with_a_clip_first() {
    // A file the remuxer can't read, so it goes as it is: HEVC with sound.
    let hevc = Fixture {
        video: "V_MPEGH/ISO/HEVC",
        cues: false,
        audio: Some(FixtureAudio::aac()),
        ..Fixture::default()
    };
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&hevc))])).await;

    // A TV with no HEVC decoder plays such a file's sound alone and says nothing. The clip,
    // with no sound, is refused, and the file never reaches the TV.
    let hub = FakeReceiver::start(FakeOptions {
        speed: 4.0,
        refuse: Refuse {
            bytes: vec![b"hvc1".to_vec(), b"V_MPEGH".to_vec()],
            how: Some(Refusal::LoadFailed),
            sound_alone: true,
            ..Refuse::default()
        },
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        CastPlayer::connect(setup(&hub, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { .. } | PlaybackEvent::Loaded))
        .await;
    let sent = hub.heard("LOAD");
    assert_eq!(sent.len(), 1, "the clip alone");
    assert_eq!(sent[0]["media"]["contentType"], "video/mp4");
    assert_eq!(
        player.failure(),
        Some(CastFailure::Uncastable(Uncastable::VideoCodec("HEVC".into())))
    );
    assert!(memory.caps("tv-1").refused("hevc/720/progressive"));

    // A TV that plays the clip gets the file next, and is not asked again.
    let tv = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let (player, mut events) =
        CastPlayer::connect(setup(&tv, Arc::clone(&memory), Arc::new(NoEncoder)))
            .await
            .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let sent = tv.heard("LOAD");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0]["media"]["contentType"], "video/mp4");
    assert_eq!(sent[1]["media"]["contentType"], "video/x-matroska");
    assert!(memory.caps("tv-1").played("hevc/720/progressive"));
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    assert_eq!(tv.heard("LOAD").len(), 3, "no clip the second time");
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
async fn a_slow_first_segment_is_waited_for_and_teaches_nothing() {
    let playlist = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n#EXTINF:4.0,\nseg1.ts\n#EXT-X-ENDLIST\n";
    let mut segment = vec![0u8; 188 * 4];
    for packet in segment.chunks_mut(188) {
        packet[0] = 0x47;
    }
    // The first segment takes twice the movement window and more than the fetch window, as a
    // torrent that has only just started does.
    let base = serve_slowly(
        HashMap::from([
            ("/a/index.m3u8".to_owned(), playlist.as_bytes().to_vec()),
            ("/a/seg0.ts".to_owned(), segment.clone()),
            ("/a/seg1.ts".to_owned(), segment),
        ]),
        HashMap::from([("/a/seg0.ts".to_owned(), Duration::from_secs(3))]),
    )
    .await;
    let receiver = FakeReceiver::start(FakeOptions { speed: 4.0, ..FakeOptions::default() })
        .await
        .unwrap();
    let memory = Arc::new(CastMemory::memory());
    let mut setup = setup(&receiver, Arc::clone(&memory), Arc::new(NoEncoder));
    setup.timings.slow_source = Duration::from_secs(1);
    let (player, mut events) = CastPlayer::connect(setup).await.unwrap();
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
    assert_eq!(receiver.heard("LOAD").len(), 1, "the first plan played, once it could");
    let caps = memory.caps("tv-1");
    assert!(
        caps.formats.values().all(|answer| answer.played),
        "a slow source is not a refusal: {:?}",
        caps.formats
    );
    assert!(caps.played("hls-ts"));
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
async fn stop_while_the_tv_starts_the_receiver_closes_it_once_up() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions {
        launch: Duration::from_millis(800),
        ..FakeOptions::default()
    })
    .await
    .unwrap();
    let (player, _events) = CastPlayer::connect(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(NoEncoder),
    ))
    .await
    .unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while receiver.heard("LAUNCH").is_empty() {
        assert!(tokio::time::Instant::now() < deadline, "the load launched the receiver");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    // Stop on the card while the card still says "Starting".
    player.quit().await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while !receiver.heard("STOP").iter().any(|m| m["sessionId"].is_string()) {
        assert!(tokio::time::Instant::now() < deadline, "the receiver was left up on the TV");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(receiver.heard("LOAD").is_empty(), "nothing was loaded after the stop");
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
    // The relay closed with the cast, though the player is still held.
    let served = receiver.fetched()[0].url.clone();
    assert!(PlainHttp.get(&served, &[]).await.is_err(), "the relay still answers");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_speed_asked_for_before_a_load_is_set_once_it_plays() {
    let file = matroska(&Fixture::default());
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) = CastPlayer::connect(setup(
        &receiver,
        Arc::new(CastMemory::memory()),
        Arc::new(FakeEncoder::default()),
    ))
    .await
    .unwrap();
    // The session sets the speed from its options before it loads the first file.
    player.set_rate(1.5).await.unwrap();
    assert!(receiver.heard("SET_PLAYBACK_RATE").is_empty());
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    next(&mut events, |e| *e == PlaybackEvent::Speed(1.5)).await;
    assert_eq!(receiver.heard("SET_PLAYBACK_RATE")[0]["playbackRate"], 1.5);
    assert_eq!(player.rate(), 1.5);
    player.quit().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_tv_that_goes_silent_ends_the_cast_and_never_holds_a_control_up() {
    let file = matroska(&Fixture { audio: Some(FixtureAudio::aac()), ..Fixture::default() });
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), file)])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let mut setup = setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder));
    setup.timings.silence = Duration::from_secs(2);
    let (player, mut events) = CastPlayer::connect(setup).await.unwrap();
    player.load(torrent(format!("{base}/episode.mkv")));
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    // Long enough on a live TV that the heartbeat and polls keep the channel up.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(player.is_alive(), "a TV that answers is kept");

    // Switched off at the wall: the socket stays open, and nothing comes back.
    receiver.go_silent();
    let began = tokio::time::Instant::now();
    for control in 0..3 {
        let asked = tokio::time::Instant::now();
        let sent = match control {
            0 => player.set_paused(true).await,
            1 => player.seek(10.0, true).await,
            _ => player.set_volume(50.0).await,
        };
        assert!(sent.is_ok() || !player.is_alive(), "{sent:?}");
        assert!(asked.elapsed() < Duration::from_secs(3), "a control waited on a silent TV");
    }
    next(&mut events, |e| matches!(e, PlaybackEvent::Ended { complete: false })).await;
    assert!(began.elapsed() < Duration::from_secs(8), "noticed in {:?}", began.elapsed());
    assert!(!player.is_alive());
    assert!(matches!(player.failure(), Some(CastFailure::Lost(_))), "{:?}", player.failure());
    assert_eq!(player.status().borrow().state, CastState::Ended);

    // Stop on the card doesn't wait out the silence either.
    let asked = tokio::time::Instant::now();
    let _ = player.quit().await;
    assert!(asked.elapsed() < Duration::from_secs(3));
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_audio_track_loads_the_file_again_where_it_was_without_a_new_start() {
    let dub = FixtureAudio { language: "eng", ..FixtureAudio::aac() };
    let file = Fixture {
        seconds: 60,
        audio: Some(FixtureAudio::aac()),
        more_audio: vec![dub],
        ..Fixture::default()
    };
    let base = serve(HashMap::from([("/episode.mkv".to_owned(), matroska(&file))])).await;
    let receiver = FakeReceiver::start(FakeOptions::default()).await.unwrap();
    let (player, mut events) =
        CastPlayer::connect(setup(&receiver, Arc::new(CastMemory::memory()), Arc::new(NoEncoder)))
            .await
            .unwrap();
    let mut status = player.status();
    player.load(torrent(format!("{base}/episode.mkv")));
    assert!(status.borrow_and_update().step.is_some(), "a load not yet playing says so");
    next(&mut events, |e| *e == PlaybackEvent::Loaded).await;
    let playing = player.status().borrow().clone();
    assert_eq!(playing.step, None);
    let languages: Vec<_> =
        playing.audio.iter().map(|a| (a.index, a.language.as_deref(), a.selected)).collect();
    assert_eq!(languages, [(0, Some("jpn"), true), (1, Some("eng"), false)]);

    // Played a little, the dub is picked.
    next(&mut events, |e| matches!(e, PlaybackEvent::Progress { position, .. } if *position >= 2.0))
        .await;
    player.set_audio(1);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while receiver.heard("LOAD").len() < 2
        || !player.status().borrow().audio.iter().any(|a| a.index == 1 && a.selected)
        || player.status().borrow().step.is_some()
    {
        assert!(tokio::time::Instant::now() < deadline, "the dub loaded");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let loads = receiver.heard("LOAD");
    assert!(loads[1]["currentTime"].as_f64().unwrap_or(0.0) >= 2.0, "{:?}", loads[1]);
    // The session heard nothing start again: no second `Loaded` before the file plays on.
    next(&mut events, |e| match e {
        PlaybackEvent::Loaded => panic!("the reload was taken for a new file"),
        PlaybackEvent::Progress { position, .. } => *position >= 3.0,
        _ => false,
    })
    .await;
    // The track playing isn't loaded again.
    player.set_audio(1);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(receiver.heard("LOAD").len(), 2);
}
