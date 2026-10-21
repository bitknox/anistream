//! The thumbnailer's entry, by [`Class::Thumbnail`]'s rules: answered from what playback
//! already fetched when it can be, sent upstream one at a time and only for the newest
//! position, never held, and refused at once when the host is pushing back.
//!
//! [`Class::Thumbnail`]: super::Class::Thumbnail

use std::{
    sync::{Arc, atomic::Ordering},
    time::{Duration, Instant},
};

use super::{Fetched, InFlight, Upstream, lock, playlist::segment_urls, upstream::fetch_once};

/// The longest a thumbnail request waits for anything: a fetch already under way, or its turn
/// to go upstream. The thumbnailer reads one segment at a time, so while it waits here it
/// cannot get to the newer position the pointer has moved on to.
const THUMBNAIL_WAIT: Duration = Duration::from_secs(3);

/// The least time between two thumbnail fetches from upstream.
///
/// Sweeping across the bar asks for a segment at every position the thumbnailer reaches. The
/// thumbnailer coalesces positions while it is busy, but against a fast upstream it would still
/// fetch several segments a second, every one of them counted by a host that rate-limits.
const THUMBNAIL_SPACING: Duration = Duration::from_millis(250);

/// How soon after a refused thumbnail request the next segment counts as ffmpeg skipping on.
///
/// ffmpeg's HLS demuxer answers a segment it cannot open by opening the next one at once, so a
/// refusal is followed within milliseconds by a request for its successor. See
/// [`Upstream::refused_successor`].
const SKIP_WINDOW: Duration = Duration::from_secs(1);

impl Upstream {
    /// Fetch for the thumbnailer, by [`Class::Thumbnail`]'s rules. `Err` is a refusal, to be
    /// answered at once.
    ///
    /// [`Class::Thumbnail`]: crate::playback::mend::Class::Thumbnail
    pub(super) async fn for_thumbnail(&self, url: &str) -> Result<Arc<Fetched>, String> {
        if self.refused_successor(url) {
            return Err(self.refuse(url, "the segment after a refused one"));
        }
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        if self.cooling_down() {
            return Err(self.refuse(url, "upstream is rate-limiting"));
        }
        let joined = lock(&self.in_flight).get(url).cloned();
        if let Some(cell) = joined {
            return self.await_other(url, cell).await;
        }

        // One thumbnail fetch upstream at a time, spaced, and only the newest request that
        // wants one: a request overtaken while it waited gives way to the one that overtook
        // it, so a sweep sends its last position upstream rather than every one it crossed.
        let ticket = self.thumbnail_newest.fetch_add(1, Ordering::SeqCst) + 1;
        let Ok(mut turn) =
            tokio::time::timeout(THUMBNAIL_WAIT, self.thumbnail_turn.lock()).await
        else {
            return Err(self.refuse(url, "another thumbnail fetch is still running"));
        };
        if let Some(last) = *turn {
            tokio::time::sleep_until((last + THUMBNAIL_SPACING).into()).await;
        }
        if self.thumbnail_newest.load(Ordering::SeqCst) != ticket {
            // Not `refuse`: whoever asked has moved on to the newer request, so nothing will
            // skip on from this one.
            return Err("a newer thumbnail request overtook it".to_owned());
        }
        // The wait may have been long enough for someone else to fetch it.
        if let Some(hit) = self.cached(url) {
            return Ok(hit);
        }
        if self.cooling_down() {
            return Err(self.refuse(url, "upstream is rate-limiting"));
        }
        let joined = lock(&self.in_flight).get(url).cloned();
        if let Some(cell) = joined {
            drop(turn);
            return self.await_other(url, cell).await;
        }
        *turn = Some(Instant::now());
        let cell = self.begin(url);
        self.lead(url, cell, fetch_once(self, url)).await.map_err(|e| self.refuse(url, &e))
    }

    /// Wait, briefly, on a fetch another request started.
    async fn await_other(&self, url: &str, cell: InFlight) -> Result<Arc<Fetched>, String> {
        let waited = tokio::time::timeout(
            THUMBNAIL_WAIT,
            // Only reached if that fetch was abandoned midway: nothing to wait for any more.
            cell.get_or_init(|| async {
                Err("the fetch it was waiting on stopped".to_owned())
            }),
        )
        .await;
        match waited {
            Ok(Ok(fetched)) => Ok(Arc::clone(fetched)),
            Ok(Err(e)) => Err(self.refuse(url, e)),
            Err(_) => Err(self.refuse(url, "the fetch under way is taking too long")),
        }
    }

    /// Whether either class is waiting out a rate limit.
    pub(super) fn cooling_down(&self) -> bool {
        let now = Instant::now();
        [&self.cooldown, &self.thumbnail_cooldown]
            .iter()
            .any(|cooldown| lock(cooldown).is_some_and(|until| until > now))
    }

    /// Note a refused thumbnail request, and say why.
    fn refuse(&self, url: &str, why: &str) -> String {
        *lock(&self.last_refused) = Some((url.to_owned(), Instant::now()));
        why.to_owned()
    }

    /// Whether `url` is the segment after one just refused to the thumbnailer.
    ///
    /// A refusal makes ffmpeg's HLS demuxer open the next segment at once, and the next, until
    /// one opens. Left alone that walks forward to the first segment the cache holds — often
    /// the part around the playhead, minutes from where the pointer is — and the thumbnail
    /// shows that, under the wrong time. Refusing the walk as well runs it to the end of the
    /// playlist instead, which draws nothing: no thumbnail rather than a wrong one.
    fn refused_successor(&self, url: &str) -> bool {
        let Some((refused, at)) = lock(&self.last_refused).clone() else { return false };
        at.elapsed() < SKIP_WINDOW
            && lock(&self.successors).get(&refused).is_some_and(|next| next == url)
    }

    /// Remember the order of a media playlist's segments.
    pub(super) fn learn_order(&self, body: &str, base: &str) {
        // A master playlist lists variants, not a sequence.
        if !body.contains("#EXTINF") {
            return;
        }
        let urls = segment_urls(body, base);
        let mut successors = lock(&self.successors);
        for pair in urls.windows(2) {
            successors.insert(pair[0].clone(), pair[1].clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use tokio::net::TcpListener;

    use super::*;
    use crate::playback::mend::{tests::transport_stream, *};

    // Two players: playback and the seek bar's thumbnailer.

    /// A stand-in segment host on loopback: serves `files` by name after `delay`, logs every
    /// request, and answers `429` with `Retry-After: 10` whenever `limited(name, recent)`
    /// says so, `recent` being the segment requests in the last two seconds, this one
    /// included.
    struct Origin {
        base: String,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl Origin {
        fn asked(&self, name: &str) -> usize {
            lock(&self.log).iter().filter(|n| *n == name).count()
        }

        fn segments_asked(&self) -> usize {
            lock(&self.log).iter().filter(|n| n.ends_with(".ts")).count()
        }
    }

    async fn origin(
        files: Vec<(String, Vec<u8>)>,
        delay: Duration,
        limited: impl Fn(&str, usize) -> bool + Send + Sync + 'static,
    ) -> Origin {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let log = Arc::new(Mutex::new(Vec::new()));
        let files: Arc<HashMap<String, Vec<u8>>> = Arc::new(files.into_iter().collect());
        let recent = Arc::new(Mutex::new(Vec::<Instant>::new()));
        let limited = Arc::new(limited);
        {
            let log = Arc::clone(&log);
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    let (log, files, recent, limited) = (
                        Arc::clone(&log),
                        Arc::clone(&files),
                        Arc::clone(&recent),
                        Arc::clone(&limited),
                    );
                    tokio::spawn(async move {
                        let (read, mut write) = socket.into_split();
                        let mut reader = BufReader::new(read);
                        let mut line = String::new();
                        if read_line(&mut reader, &mut line).await.is_err() {
                            return;
                        }
                        loop {
                            let mut header = String::new();
                            match read_line(&mut reader, &mut header).await {
                                Ok(n) if n > 0 && !header.trim().is_empty() => {}
                                _ => break,
                            }
                        }
                        let path = line.split_whitespace().nth(1).unwrap_or("/");
                        let name = path.rsplit('/').next().unwrap_or("").to_owned();
                        lock(&log).push(name.clone());
                        let is_limited = name.ends_with(".ts") && {
                            let mut recent = lock(&recent);
                            recent.retain(|at| at.elapsed() < Duration::from_secs(2));
                            recent.push(Instant::now());
                            limited(&name, recent.len())
                        };
                        if is_limited {
                            let _ = write
                                .write_all(
                                    b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 10\r\n\
                                      Content-Length: 0\r\nConnection: close\r\n\r\n",
                                )
                                .await;
                            return;
                        }
                        tokio::time::sleep(delay).await;
                        let Some(body) = files.get(&name) else {
                            let _ = write
                                .write_all(
                                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
                                )
                                .await;
                            return;
                        };
                        let kind = if name.ends_with(".m3u8") {
                            "application/vnd.apple.mpegurl"
                        } else {
                            "video/mp2t"
                        };
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\n\
                             Connection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = write.write_all(head.as_bytes()).await;
                        let _ = write.write_all(body).await;
                    });
                }
            });
        }
        Origin { base, log }
    }

    /// A VOD playlist of `count` segments named `seg<n>.ts`, each a distinct transport stream.
    fn vod(count: usize) -> Vec<(String, Vec<u8>)> {
        let mut playlist =
            "#EXTM3U\n#EXT-X-VERSION:3\n#EXT-X-TARGETDURATION:4\n#EXT-X-PLAYLIST-TYPE:VOD\n"
                .to_owned();
        let mut files = Vec::new();
        for n in 0..count {
            playlist.push_str(&format!("#EXTINF:4.0,\nseg{n}.ts\n"));
            let mut segment = transport_stream();
            segment.push(n as u8);
            files.push((format!("seg{n}.ts"), segment));
        }
        playlist.push_str("#EXT-X-ENDLIST\n");
        files.push(("index.m3u8".to_owned(), playlist.into_bytes()));
        files
    }

    async fn mender_for(origin: &Origin) -> MendServer {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        serve(http, &format!("{}/index.m3u8", origin.base), Vec::new(), "tok").await.unwrap()
    }

    /// GET through the mender: status and body.
    async fn get(url: impl AsRef<str>) -> (u16, Vec<u8>) {
        let rest = url.as_ref().trim_start_matches("http://");
        let (authority, path) = rest.split_once('/').unwrap();
        let mut socket = TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(raw.len());
        let status = String::from_utf8_lossy(&raw[..split])
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        (status, raw.get(split + 4..).unwrap_or_default().to_vec())
    }

    /// The proxied URL of segment `n`, as a player of `class` would find it in its playlist.
    fn segment(server: &MendServer, origin: &Origin, class: Class, n: usize) -> String {
        let entry = match class {
            Class::Playback => server.url(),
            Class::Thumbnail => server.thumbnail_url(),
        };
        let port_and_token = &entry[..entry.rfind('/').unwrap()];
        format!("{port_and_token}/{}", encode_path(&format!("{}/seg{n}.ts", origin.base)))
    }

    /// GET a thumbnail request, insisting it is answered within a second: the property under
    /// test everywhere below. A held request would take the host's ten.
    async fn get_promptly(url: &str) -> (u16, Vec<u8>) {
        tokio::time::timeout(Duration::from_secs(1), get(url))
            .await
            .expect("a thumbnail request was held instead of answered")
    }

    #[tokio::test]
    async fn the_thumbnail_entry_rewrites_its_playlist_onto_itself() {
        let origin = origin(vod(2), Duration::ZERO, |_, _| false).await;
        let server = mender_for(&origin).await;

        let (_, playback) = get(server.url()).await;
        let (_, thumbnails) = get(server.thumbnail_url()).await;
        let (playback, thumbnails) =
            (String::from_utf8_lossy(&playback), String::from_utf8_lossy(&thumbnails));
        // Segments found through one entry come back through the same one, so the
        // thumbnailer's requests stay recognisable all the way down.
        assert!(playback.contains("/m/tok/") && !playback.contains("/t/tok/"), "{playback}");
        assert!(
            thumbnails.contains("/t/tok/") && !thumbnails.contains("/m/tok/"),
            "{thumbnails}"
        );
        // An ended playlist is as fixed as a segment: the second player costs no request.
        assert_eq!(origin.asked("index.m3u8"), 1);
    }

    #[tokio::test]
    async fn thumbnails_are_answered_from_what_playback_already_fetched() {
        let origin = origin(vod(3), Duration::ZERO, |_, _| false).await;
        let server = mender_for(&origin).await;

        let (status, played) = get(&segment(&server, &origin, Class::Playback, 1)).await;
        assert_eq!(status, 200);
        let (status, thumbnail) =
            get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 200);
        assert_eq!(thumbnail, played);
        assert_eq!(origin.asked("seg1.ts"), 1, "a loaded part of the bar went upstream again");
    }

    #[tokio::test]
    async fn a_rate_limited_thumbnail_is_refused_at_once_and_never_holds_playback() {
        // The reported failure: a 429 parked the thumbnailer for ten seconds, and the hold it
        // set was shared, so playback's own requests waited behind it too.
        let origin = origin(vod(4), Duration::ZERO, |name, _| name == "seg1.ts").await;
        let server = mender_for(&origin).await;

        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 503);

        let started = Instant::now();
        let (status, _) = get(&segment(&server, &origin, Class::Playback, 2)).await;
        assert_eq!(status, 200);
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "playback waited out a rate limit only a thumbnail had earned"
        );

        // While the thumbnail hold lasts, what playback fetched is still a thumbnail away,
        // and nothing else is asked for.
        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 2)).await;
        assert_eq!(status, 200);
        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 3)).await;
        assert_eq!(status, 503);
        assert_eq!(origin.asked("seg3.ts"), 0);
    }

    #[tokio::test]
    async fn while_playback_is_held_thumbnails_are_refused_without_asking_upstream() {
        let origin = origin(vod(3), Duration::ZERO, |name, _| name == "seg0.ts").await;
        let server = mender_for(&origin).await;

        // Playback meets the 429 and holds, as it must.
        let held = tokio::spawn(get(segment(&server, &origin, Class::Playback, 0)));
        let started = Instant::now();
        while origin.asked("seg0.ts") == 0 {
            assert!(started.elapsed() < Duration::from_secs(5), "playback never asked");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;

        let (status, _) = get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await;
        assert_eq!(status, 503);
        assert_eq!(origin.asked("seg1.ts"), 0, "a thumbnail spent a request during the hold");
        held.abort();
    }

    #[tokio::test]
    async fn one_segment_asked_for_at_once_is_fetched_once() {
        // thumbfast's exact seek lands right behind its keyframe seek, on the same segment.
        let origin = origin(vod(1), Duration::from_millis(300), |_, _| false).await;
        let server = mender_for(&origin).await;
        let url = segment(&server, &origin, Class::Thumbnail, 0);

        let asks: Vec<_> = (0..4).map(|_| tokio::spawn(get(url.clone()))).collect();
        for ask in asks {
            assert_eq!(ask.await.unwrap().0, 200);
        }
        assert_eq!(origin.asked("seg0.ts"), 1);
    }

    #[tokio::test]
    async fn only_the_newest_waiting_thumbnail_goes_upstream() {
        // A sweep crosses positions faster than they can be fetched; the ones it crossed are
        // not worth a request each.
        let origin = origin(vod(3), Duration::from_millis(500), |_, _| false).await;
        let server = mender_for(&origin).await;

        let first = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 0)));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let crossed = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 1)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        let newest = tokio::spawn(get(segment(&server, &origin, Class::Thumbnail, 2)));

        assert_eq!(first.await.unwrap().0, 200);
        assert_eq!(crossed.await.unwrap().0, 503);
        assert_eq!(newest.await.unwrap().0, 200);
        assert_eq!((origin.asked("seg1.ts"), origin.asked("seg2.ts")), (0, 1));
    }

    #[tokio::test]
    async fn the_segments_after_a_refused_one_are_refused_too() {
        // ffmpeg opens the next segment when one is refused, and the next, until one opens:
        // left alone it would land on the first cached one and draw that, minutes off.
        let origin = origin(vod(4), Duration::ZERO, |name, _| name == "seg1.ts").await;
        let server = mender_for(&origin).await;
        get(server.thumbnail_url()).await;
        for n in [0, 2, 3] {
            get(&segment(&server, &origin, Class::Playback, n)).await;
        }

        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 1)).await.0, 503);
        // Cached, and refused all the same: these are ffmpeg walking on from the refusal.
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 2)).await.0, 503);
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 3)).await.0, 503);
        // A seek elsewhere is not part of the walk.
        assert_eq!(get_promptly(&segment(&server, &origin, Class::Thumbnail, 0)).await.0, 200);
    }

    #[tokio::test]
    async fn a_sweep_through_a_host_that_starts_refusing_never_stalls_a_thumbnail() {
        // The regression as reported, without mpv: a host that answers 429 to a burst of
        // segment requests, playback having loaded the start, and a thumbnailer sweeping the
        // whole bar one segment at a time. Every thumbnail request is answered within a
        // second, no thumbnail request goes upstream once the host has said to stop, and the
        // loaded part of the bar keeps its thumbnails throughout.
        let tripped = Arc::new(Mutex::new(None::<Instant>));
        let origin = origin(vod(12), Duration::from_millis(20), {
            let tripped = Arc::clone(&tripped);
            move |_, recent| {
                let mut tripped = lock(&tripped);
                if tripped.is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                    return true;
                }
                if recent > 6 {
                    *tripped = Some(Instant::now());
                    return true;
                }
                false
            }
        })
        .await;
        let server = mender_for(&origin).await;
        get(server.url()).await;
        for n in 0..3 {
            assert_eq!(get(&segment(&server, &origin, Class::Playback, n)).await.0, 200);
        }
        get(server.thumbnail_url()).await;

        let mut answered = Vec::new();
        for n in (3..12).chain((0..12).rev()) {
            answered
                .push(get_promptly(&segment(&server, &origin, Class::Thumbnail, n)).await.0);
        }
        assert!(answered.contains(&503), "the host never pushed back: {answered:?}");
        let at_trip = origin.segments_asked();
        assert!(lock(&tripped).is_some());

        // Resting on the loaded start: answered from memory while the host still refuses.
        for n in [1, 0, 2] {
            assert_eq!(
                get_promptly(&segment(&server, &origin, Class::Thumbnail, n)).await.0,
                200
            );
        }
        assert_eq!(
            origin.segments_asked(),
            at_trip,
            "a thumbnail went upstream while the host was refusing"
        );
    }

    /// mpv's JSON IPC, enough to drive a thumbnailer.
    #[cfg(unix)]
    struct Ipc {
        lines: tokio::io::Lines<BufReader<tokio::net::unix::OwnedReadHalf>>,
        writer: tokio::net::unix::OwnedWriteHalf,
        next_id: u64,
    }

    #[cfg(unix)]
    impl Ipc {
        async fn connect(path: &std::path::Path) -> Self {
            use tokio::io::AsyncBufReadExt;
            for _ in 0..400 {
                if let Ok(socket) = tokio::net::UnixStream::connect(path).await {
                    let (read, writer) = socket.into_split();
                    return Self { lines: BufReader::new(read).lines(), writer, next_id: 0 };
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            panic!("the thumbnailer never opened its IPC socket");
        }

        /// Run a command; a reply slower than two seconds is the freeze under test.
        async fn command(&mut self, args: serde_json::Value) -> serde_json::Value {
            self.next_id += 1;
            let id = self.next_id;
            let line = serde_json::json!({ "command": args, "request_id": id }).to_string();
            self.writer.write_all(format!("{line}\n").as_bytes()).await.unwrap();
            let reply = tokio::time::timeout(Duration::from_secs(2), async {
                while let Some(line) = self.lines.next_line().await.unwrap() {
                    let value: serde_json::Value = serde_json::from_str(&line).unwrap();
                    if value["request_id"] == id {
                        return value;
                    }
                }
                panic!("the thumbnailer closed its IPC socket");
            })
            .await;
            reply.unwrap_or_else(|_| panic!("the thumbnailer stopped answering: {args}"))
        }
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    async fn a_flooded_thumbnailer_draws_where_the_pointer_rests() {
        // thumbfast's own child, with thumbfast's arguments, on the thumbnail entry of an HLS
        // stream whose host starts answering 429 to the burst a fast sweep makes. Playback has
        // loaded the first part. After a flood of seeks across the whole bar, a seek into the
        // loaded part must draw within a bound — before, a held segment froze the child on
        // its last picture for ten seconds at a time.
        let dir = tempfile::tempdir().unwrap();
        let encoded = tokio::process::Command::new("mpv")
            .args(["--no-config", "--really-quiet", "--no-audio"])
            .arg("av://lavfi:testsrc2=size=320x180:rate=24:duration=60")
            .args(["--of=hls", "--ofopts=hls_time=4,hls_playlist_type=vod"])
            // MPEG-2 repeats its sequence header at every keyframe, so each segment decodes
            // on its own; MPEG-4 part 2 puts it in the first segment only.
            .args(["--ovc=mpeg2video", "--ovcopts=g=48,qscale=4"])
            .arg(format!("--o={}", dir.path().join("index.m3u8").display()))
            .status()
            .await;
        match encoded {
            Err(_) => {
                // CI's mpv job installs mpv and sets this, so a missing mpv fails there
                // instead of passing as a skip.
                assert!(
                    std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                    "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
                );
                eprintln!("skipping: mpv is not on PATH");
                return;
            }
            Ok(status) if !status.success() => {
                eprintln!("skipping: this mpv cannot write an HLS fixture");
                return;
            }
            Ok(_) => {}
        }
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir.path()).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            files.push((name, std::fs::read(&path).unwrap()));
        }
        let segments = files.iter().filter(|(name, _)| name.ends_with(".ts")).count();
        let origin = origin(files, Duration::from_millis(30), |_, recent| recent > 6).await;
        let server = mender_for(&origin).await;

        // Playback loads the first 16 seconds.
        get(server.url()).await;
        for n in 0..4 {
            let url = format!("{}/index{n}.ts", origin.base);
            let entry = server.url();
            let proxied =
                format!("{}/{}", &entry[..entry.rfind('/').unwrap()], encode_path(&url));
            assert_eq!(get(&proxied).await.0, 200);
        }

        let ipc_path = dir.path().join("ipc");
        let thumb = dir.path().join("thumb.bgra");
        let frame = 320 * 180 * 4;
        let mut child = tokio::process::Command::new("mpv")
            .args(["--no-config", "--msg-level=all=no", "--idle", "--pause"])
            .args(["--keep-open=always", "--really-quiet", "--no-terminal"])
            .args(["--load-scripts=no", "--osc=no", "--ytdl=no", "--no-sub", "--no-audio"])
            .args(["--start=8", "--hr-seek=no", "--demuxer-readahead-secs=0"])
            .args(["--demuxer-max-bytes=128KiB", "--vd-lavc-fast", "--hwdec=no"])
            .args(["--vf=scale=320:-2,format=bgra", "--ovc=rawvideo", "--of=image2"])
            .args(["--ofopts=update=1", "--demuxer-lavf-o-append=http_multiple=0"])
            .arg(format!("--o={}", thumb.display()))
            .arg(format!("--input-ipc-server={}", ipc_path.display()))
            .args(["--", server.thumbnail_url()])
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let mut ipc = Ipc::connect(&ipc_path).await;
        // As thumbfast's child does once it is up: mpv before 0.40 applies its encoding profile
        // after the command line, which turns keep-open off, and the first seek to meet an end
        // of file then ended the file and left the child idle.
        ipc.command(serde_json::json!(["set_property", "keep-open", "always"])).await;
        let drawn = |wait: Duration| {
            let thumb = thumb.clone();
            async move {
                let deadline = Instant::now() + wait;
                while Instant::now() < deadline {
                    if std::fs::metadata(&thumb).is_ok_and(|m| m.len() == frame) {
                        return true;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                false
            }
        };
        assert!(drawn(Duration::from_secs(5)).await, "no first thumbnail");

        // The flood: a new position every 15 ms for a second and a half, back and forth
        // across the bar, as thumbfast sends them while the pointer races.
        let started = Instant::now();
        let mut step = 0_u32;
        while started.elapsed() < Duration::from_millis(1500) {
            let sweep = f64::from(step % 40) / 40.0;
            let target = if (step / 40).is_multiple_of(2) { sweep } else { 1.0 - sweep } * 58.0;
            ipc.command(serde_json::json!(["seek", target, "absolute+keyframes"])).await;
            step += 1;
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        assert!(origin.segments_asked() > 4, "the flood never reached the host");

        // Rest on a loaded position, the way thumbfast does: keyframe, then exact.
        std::fs::remove_file(&thumb).ok();
        let rested = Instant::now();
        ipc.command(serde_json::json!(["seek", 10.0, "absolute+keyframes"])).await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        ipc.command(serde_json::json!(["seek", 10.0, "absolute+exact"])).await;
        assert!(
            drawn(Duration::from_secs(3)).await,
            "the thumbnailer drew nothing where the pointer rested ({segments} segments, host \
             asked {} times)",
            origin.segments_asked()
        );
        // The first picture can be the keyframe seek's, from the keyframe at 8s; the exact
        // seek lands a moment later. Reading the position once raced it on a loaded runner, so
        // it is read until it lands, within the same bound.
        let mut position = f64::NAN;
        while rested.elapsed() < Duration::from_secs(3) {
            let reply = ipc.command(serde_json::json!(["get_property", "time-pos"])).await;
            position = reply["data"].as_f64().unwrap_or(f64::NAN);
            if (8.5..=10.5).contains(&position) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!((8.5..=10.5).contains(&position), "drew {position}s for a rest at 10s");

        ipc.command(serde_json::json!(["quit"])).await;
        let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
    }
}
