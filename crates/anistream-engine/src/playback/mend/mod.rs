//! A loopback proxy that repairs disguised media on its way to the player.
//!
//! **The problem.** Some hosts serve video segments dressed as something else. The case that
//! prompted this: every segment of an episode was a valid 1×1 PNG with the real MPEG-TS
//! appended after `IEND`, served from an image CDN — an image host will carry an image, so the
//! video is smuggled behind one. The site's own player strips the prefix in JavaScript before
//! handing bytes to MediaSource. mpv has no such hook: libavformat probes the first bytes, sees
//! PNG, decodes a 1×1 image and finds no video. The stream is not encrypted and there is no DRM
//! here — the bytes are simply mislabelled, and this puts the label right.
//!
//! **Why it is generic.** Nothing here knows about PNG, or about any particular source. It
//! searches the head of each segment for the first offset where a *container signature* appears
//! — MPEG-TS's repeating sync byte, or an ISO-BMFF box — and serves from there. A segment that
//! is already valid matches at offset zero and passes through byte-for-byte, so routing a
//! healthy stream through this changes nothing. Anything unrecognised is also passed through
//! untouched: the player is the better judge, and refusing to serve bytes we merely do not
//! recognise would break more than it fixed.
//!
//! **What it is not.** Not a rewriter of media, and not a way around encryption: an
//! `#EXT-X-KEY` is proxied verbatim so encrypted HLS keeps working exactly as it did, and
//! nothing attempts to decrypt anything.
//!
//! **Two players read through it.** The seek bar's thumbnailer is a second mpv opening the
//! same stream, and left to itself it fetched whole segments at every hover — twice, since
//! thumbfast seeks to a keyframe and then exactly — until the host answered `429` and the
//! rate-limit hold below parked the thumbnailer, and with it the thumbnail, for ten seconds at
//! a time. So the thumbnailer gets its own entry (see [`MendServer::thumbnail_url`]) and is
//! served by different rules: from what playback already fetched when it can be, never held,
//! and never the reason playback is held. See [`Class`].
//!
//! # Layout
//!
//! The server is here: the loopback listener, each connection read, routed by its entry and
//! answered, and what every part of the proxy shares. Finding where a segment's media starts,
//! rewriting a playlist, fetching from the host, and the thumbnailer's rules each have a file
//! of their own.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicU64},
    time::Instant,
};

use anistream_net::HttpClient;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};

mod playlist;
mod segment;
mod thumbnail;
mod upstream;

pub use playlist::{resolve_url, rewrite_playlist};
pub use segment::payload_start;

use playlist::looks_like_playlist;
use upstream::{Cache, send, send_once};

/// The largest response body the proxy will hold in memory.
///
/// Segments are seconds of video; a playlist is text. Anything far past that is not something
/// this proxy should be buffering, so it is refused rather than swallowing the process.
const MAX_BODY: usize = 96 * 1024 * 1024;

/// How many bytes of fetched resources the proxy keeps for a second read.
///
/// mpv's default cache holds 150 MiB ahead of the playhead and 50 MiB behind it, and what it
/// holds is what the seek bar shows as loaded. Keeping as much here means a thumbnail over any
/// loaded part of the bar is answered from memory, with nothing sent upstream.
const CACHE_BYTES: usize = 200 * 1024 * 1024;

/// A running mender.
pub struct MendServer {
    url: String,
    thumbnail_url: String,
    upstream: Arc<Upstream>,
    /// The listener and its accept loop, which a phone's app binds again on the same port when
    /// it comes back from the background: see `anistream_net::loopback`.
    _listener: Arc<anistream_net::loopback::Loopback>,
}

impl MendServer {
    /// The URL to hand to the player, in place of the stream's own.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The URL to hand to the seek bar's thumbnailer: the same stream, read under
    /// [`Class::Thumbnail`]'s rules.
    pub fn thumbnail_url(&self) -> &str {
        &self.thumbnail_url
    }

    /// The last error status the host answered playback's requests with, if it has: what a
    /// stream that never showed a frame failed of, for the server's record.
    pub fn refused(&self) -> Option<u16> {
        *lock(&self.upstream.refused)
    }
}

/// Which player a request comes from, told apart by the entry it came in through.
///
/// Both read the same stream and share one cache, but they want different things when the
/// upstream pushes back. Playback must not lose a segment — ffmpeg skips one it cannot open,
/// and skipping is what ran an episode to its end minutes in — so its requests are held through
/// a rate limit. A thumbnail is worth nothing a second later: the pointer has moved on, and a
/// thumbnailer parked on a held request shows the last picture it drew for as long as the hold
/// lasts. So thumbnail requests are answered from the cache when they can be, go upstream one
/// at a time, never wait out a rate limit, and a `429` they earn holds back only further
/// thumbnail requests, never playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Playback,
    Thumbnail,
}

impl Class {
    /// The path segment that names the class in a proxied URL.
    fn segment(self) -> &'static str {
        match self {
            Self::Playback => "m",
            Self::Thumbnail => "t",
        }
    }
}

/// A resource as the upstream sent it, before any mending or rewriting.
#[derive(Debug)]
struct Fetched {
    content_type: String,
    body: Vec<u8>,
}

/// One fetch, awaited by every request that wants the same resource while it runs.
type InFlight = Arc<tokio::sync::OnceCell<Result<Arc<Fetched>, String>>>;

/// Everything one proxied stream needs: the client to fetch with, and the headers to fetch with.
struct Upstream {
    http: HttpClient,
    headers: Vec<(String, String)>,
    token: String,
    port: u16,
    /// No upstream request before this instant. Shared, because a rate limit is per client
    /// rather than per segment: one request learning to wait means every other one should too,
    /// not discover it again by spending another request.
    cooldown: Mutex<Option<Instant>>,
    /// No thumbnail request upstream before this instant: set by a `429` a thumbnail request
    /// earned, and read by thumbnail requests alone. Playback's own `cooldown` holds back
    /// thumbnails too, but this never holds back playback.
    thumbnail_cooldown: Mutex<Option<Instant>>,
    /// Whole resources already fetched, for whichever player asks next.
    cache: Mutex<Cache>,
    cache_limit: usize,
    /// Fetches under way, so a second request for the same resource waits for the first
    /// rather than asking the upstream again — thumbfast's exact seek right behind its
    /// keyframe seek, or a thumbnail of the segment playback is fetching this moment.
    in_flight: Mutex<HashMap<String, InFlight>>,
    /// Held by the one thumbnail request allowed upstream at a time, with when it went.
    thumbnail_turn: tokio::sync::Mutex<Option<Instant>>,
    /// The ticket of the newest thumbnail request waiting to go upstream.
    thumbnail_newest: AtomicU64,
    /// Each segment's successor in the playlists served so far, and the last thumbnail
    /// request refused, with when. See [`Upstream::refused_successor`].
    successors: Mutex<HashMap<String, String>>,
    last_refused: Mutex<Option<(String, Instant)>>,
    /// The last `4xx` or `5xx` the host answered a playback request with. Only playback's: a
    /// thumbnail refused says nothing of whether the stream plays.
    refused: Mutex<Option<u16>>,
}

impl Upstream {
    fn new(http: HttpClient, headers: Vec<(String, String)>, token: &str, port: u16) -> Self {
        Self {
            http,
            headers,
            token: token.to_owned(),
            port,
            cooldown: Mutex::new(None),
            thumbnail_cooldown: Mutex::new(None),
            cache: Mutex::new(Cache::default()),
            cache_limit: CACHE_BYTES,
            in_flight: Mutex::new(HashMap::new()),
            thumbnail_turn: tokio::sync::Mutex::new(None),
            thumbnail_newest: AtomicU64::new(0),
            successors: Mutex::new(HashMap::new()),
            last_refused: Mutex::new(None),
            refused: Mutex::new(None),
        }
    }

    /// Note the status the host answered a playback request with, when it was an error.
    fn answered(&self, status: u16) {
        if (400..600).contains(&status) {
            *lock(&self.refused) = Some(status);
        }
    }

    /// The loopback URL that will fetch and mend `url` for a player of `class`.
    fn proxied(&self, url: &str, class: Class) -> String {
        format!(
            "http://127.0.0.1:{}/{}/{}/{}",
            self.port,
            class.segment(),
            self.token,
            encode_path(url)
        )
    }
}

/// Lock a mutex, recovering it if a panic poisoned it: every value guarded here is valid at
/// any instant, so carrying on beats taking the proxy down with the panic.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Percent-encode a URL so it can ride inside a path segment.
fn encode_path(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 16);
    for byte in url.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Reverse [`encode_path`].
fn decode_path(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&encoded[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Serve `url` through a mending proxy on loopback, until the returned server is dropped.
///
/// `headers` are the stream's own — the referer a locked CDN insists on travels with every
/// upstream request, which is a second thing this fixes: mpv would otherwise have to be told
/// them, and it cannot be told different headers per host.
pub async fn serve(
    http: HttpClient,
    url: &str,
    headers: Vec<(String, String)>,
    token: &str,
) -> std::io::Result<MendServer> {
    // Loopback only, like every other server this app runs. The proxy is told its port once it
    // has one, for the URLs it rewrites a playlist with.
    let upstream: Arc<std::sync::OnceLock<Arc<Upstream>>> = Arc::default();
    let listener = {
        let upstream = Arc::clone(&upstream);
        anistream_net::loopback::Loopback::bind(move |listener| {
            let upstream = Arc::clone(&upstream);
            tokio::spawn(async move {
                loop {
                    let Ok((socket, _)) = listener.accept().await else {
                        break;
                    };
                    let Some(upstream) = upstream.get().cloned() else { continue };
                    // One task per connection: mpv opens several at once for a playlist and
                    // its segments, and a slow segment must not block the rest.
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(socket, upstream).await {
                            tracing::debug!(error = %e, "mender connection ended");
                        }
                    });
                }
            })
        })
        .await?
    };
    let proxy = Arc::new(Upstream::new(http, headers, token, listener.port()));
    let entry = proxy.proxied(url, Class::Playback);
    let thumbnail_entry = proxy.proxied(url, Class::Thumbnail);
    let _ = upstream.set(Arc::clone(&proxy));

    tracing::info!(%entry, "mending proxy listening on loopback");
    Ok(MendServer {
        url: entry,
        thumbnail_url: thumbnail_entry,
        upstream: proxy,
        _listener: listener,
    })
}

async fn handle_connection(socket: TcpStream, upstream: Arc<Upstream>) -> std::io::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    read_line(&mut reader, &mut request_line).await?;

    // The `Range` header is the one thing carried through. A mended body has different offsets
    // from the upstream one, so a range cannot be answered *and* mended — but ignoring it and
    // replying `200` with the whole resource is worse than either: a playlist using
    // `#EXT-X-BYTERANGE` addresses many segments into one large file, so every segment request
    // returned the entire file, blew the body ceiling, and killed a stream that plays fine
    // without the proxy in the way. Such a resource is passed through untouched instead.
    let mut range: Option<String> = None;
    loop {
        let mut line = String::new();
        let n = read_line(&mut reader, &mut line).await?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range = Some(value.trim().to_owned());
        }
    }

    let Some(target) = request_target(&request_line) else {
        write_half
            .write_all(b"HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\n\r\n")
            .await?;
        return Ok(());
    };

    let routed = [Class::Playback, Class::Thumbnail].into_iter().find_map(|class| {
        let prefix = format!("/{}/{}/", class.segment(), upstream.token);
        target.strip_prefix(&prefix).map(|encoded| (class, decode_path(encoded)))
    });
    let Some((class, url)) = routed else {
        write_half.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    };

    // A ranged request asks for a slice of a resource, which is never the start of a segment,
    // so there is nothing to mend and the upstream's own answer is the right one to relay.
    //
    // Except the one mpv opens every resource with. `bytes=0-` asks for the whole thing and is
    // sent on the very first request, playlists included — relaying that meant the master
    // playlist came back verbatim, its segment URIs never rewritten, and mpv resolved them
    // against the proxy's own address and got nothing. An open range from zero is not a slice.
    if let Some(range) = range.filter(|r| !is_whole_resource(r)) {
        return relay_range(&upstream, class, &url, &range, &mut write_half).await;
    }

    let fetched = match class {
        Class::Playback => upstream.for_playback(&url).await,
        Class::Thumbnail => upstream.for_thumbnail(&url).await,
    };
    match fetched {
        Ok(fetched) => {
            let content_type = &fetched.content_type;
            let body = if looks_like_playlist(content_type, &fetched.body) {
                let text = String::from_utf8_lossy(&fetched.body);
                upstream.learn_order(&text, &url);
                let proxy = |u: &str| upstream.proxied(u, class);
                rewrite_playlist(&text, &url, &proxy).into_bytes()
            } else {
                let start = payload_start(&fetched.body);
                if start > 0 {
                    tracing::debug!(url = %url, stripped = start, "mended a disguised segment");
                }
                fetched.body[start..].to_vec()
            };
            // The thumbnailer's refusals are logged and its answers were not, so a seek bar
            // showing no thumbnails could not be told from one that never asked.
            if class == Class::Thumbnail {
                tracing::debug!(url = %url, bytes = body.len(), "thumbnail request answered");
            }

            let head = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {content_type}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );
            write_half.write_all(head.as_bytes()).await?;
            write_half.write_all(&body).await?;
            write_half.flush().await
        }
        Err(e) if class == Class::Thumbnail => {
            tracing::debug!(url = %url, reason = %e, "thumbnail request refused");
            // At once, and as a refusal: the thumbnailer moves on to the newest position,
            // where waiting would have frozen it on this one.
            write_half
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .await?;
            Ok(())
        }
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "mender upstream failed");
            // The upstream's failure, reported as one. mpv treats 502 as a dead segment and
            // says so, which is more honest than an empty 200.
            write_half
                .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                .await?;
            Ok(())
        }
    }
}

/// Whether a `Range` header asks for the entire resource.
///
/// `bytes=0-` is what a player sends to open a file it intends to read through, so it carries no
/// intent to slice and must not turn off mending.
fn is_whole_resource(range: &str) -> bool {
    range
        .trim()
        .strip_prefix("bytes=")
        .map(str::trim)
        .is_some_and(|spec| spec == "0-" || spec.is_empty())
}

/// Relay a byte range from upstream, unmended.
///
/// Mending is a whole-resource operation — it decides where a payload *starts* — so a request
/// for bytes 800..1599 of a file has nothing to mend and must not be answered with the file.
/// The upstream's status, content range and body are passed through as they came.
async fn relay_range(
    upstream: &Upstream,
    class: Class,
    url: &str,
    range: &str,
    write_half: &mut tokio::net::tcp::OwnedWriteHalf,
) -> std::io::Result<()> {
    let relayed = async {
        let response = match class {
            Class::Playback => {
                let response = send(upstream, url, Some(range)).await?;
                upstream.answered(response.status().as_u16());
                response
            }
            // Never held, as for a whole thumbnail segment; see `Class`.
            Class::Thumbnail => {
                if upstream.cooling_down() {
                    return Err("upstream is rate-limiting".to_owned());
                }
                send_once(upstream, url, Some(range)).await?
            }
        };
        let status = response.status().as_u16();
        let content_range = response
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = response.bytes().await.map_err(|e| e.to_string())?;
        if body.len() > MAX_BODY {
            return Err(format!("a ranged body of {} bytes is too large", body.len()));
        }
        Ok::<_, String>((status, content_range, body))
    }
    .await;

    match relayed {
        Ok((status, content_range, body)) => {
            let reason = if status == 206 { "Partial Content" } else { "OK" };
            let mut head = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n",
                body.len()
            );
            if let Some(content_range) = content_range {
                head.push_str(&format!("Content-Range: {content_range}\r\n"));
            }
            head.push_str("Connection: close\r\n\r\n");
            write_half.write_all(head.as_bytes()).await?;
            write_half.write_all(&body).await?;
            write_half.flush().await
        }
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "mender range relay failed");
            write_half.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n").await
        }
    }
}

/// The request target, for read methods only.
fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }
    parts.next().map(|t| t.split('?').next().unwrap_or(t))
}

/// Read one CRLF-terminated line, capped so a hostile client cannot exhaust memory.
async fn read_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut String,
) -> std::io::Result<usize> {
    const MAX: usize = 16 * 1024;
    let mut total = 0;
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            break;
        }
        total += n;
        if byte[0] == b'\n' {
            break;
        }
        if byte[0] != b'\r' {
            out.push(byte[0] as char);
        }
        if total > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "request line too long",
            ));
        }
    }
    Ok(total)
}

/// Registry of every mender kept alive for a playback, keyed by the URL it fronts.
///
/// A playback holds one; dropping it stops every proxy it started.
#[derive(Default)]
pub struct Menders(HashMap<String, MendServer>);

impl Menders {
    /// Route `url` through a mender, returning the URL to play instead.
    ///
    /// Failure to start one is not fatal — the original URL is returned and playback proceeds
    /// exactly as it would have. A proxy that cannot start must not cost you the episode.
    pub async fn route(
        &mut self,
        http: &HttpClient,
        url: &str,
        headers: &[(String, String)],
        token: &str,
    ) -> String {
        match serve(http.clone(), url, headers.to_vec(), token).await {
            Ok(server) => {
                let mended = server.url().to_owned();
                self.0.insert(url.to_owned(), server);
                mended
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not start the mender; playing direct");
                url.to_owned()
            }
        }
    }

    /// The thumbnailer's entry to the mender fronting `url`, when one is running.
    pub fn thumbnail_url(&self, url: &str) -> Option<String> {
        self.0.get(url).map(|server| server.thumbnail_url().to_owned())
    }

    /// The error status the host answered the mender fronting `url` with, if it did: see
    /// [`MendServer::refused`].
    pub fn refused(&self, url: &str) -> Option<u16> {
        self.0.get(url).and_then(MendServer::refused)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::playback::mend::segment::TS_PACKET;

    /// Several TS packets, the shape a real segment has: a PAT first, payload-unit-start set.
    pub(super) fn transport_stream() -> Vec<u8> {
        let mut data = Vec::new();
        for packet in 0..6 {
            // First packet opens the segment (PUSI, PID 0); the rest carry an ordinary PID.
            let (flags, pid_low) = if packet == 0 { (0x40, 0x00) } else { (0x00, 0x64) };
            data.extend([0x47, flags, pid_low, 0x10]);
            data.extend(std::iter::repeat_n(0xAB, TS_PACKET - 4));
        }
        data
    }

    #[test]
    fn the_range_a_player_opens_with_does_not_turn_mending_off() {
        // mpv sends `Range: bytes=0-` on its first request for every resource, playlists
        // included. Treating that as a slice relayed the master playlist verbatim, so its
        // segment URIs were never rewritten and mpv resolved them against the proxy's own
        // address — the stream died on a change meant to fix a different one.
        assert!(is_whole_resource("bytes=0-"));
        assert!(is_whole_resource(" bytes=0- "));

        // A genuine slice, which has nothing to mend and must be relayed.
        assert!(!is_whole_resource("bytes=800-1599"));
        assert!(!is_whole_resource("bytes=-500"));
        assert!(!is_whole_resource("bytes=1000-"));
    }

    #[test]
    fn a_url_survives_the_round_trip_through_a_path() {
        for url in [
            "https://cdn.test/a/b.ts?x=1&y=2",
            // The opaque, extensionless form a segment takes on a CDN that is not serving
            // it as video — no path to parse, so the round trip has to be exact.
            "https://images.test/obj/site-i18n/202604045d0d1d30d738c249",
            "https://cdn.test/spaced%20name.ts",
        ] {
            assert_eq!(decode_path(&encode_path(url)), url);
        }
    }

    #[tokio::test]
    async fn the_proxy_serves_only_its_own_token() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server =
            serve(http, "https://example.test/x.m3u8", Vec::new(), "secret").await.unwrap();
        let wrong = server.url().replace("/m/secret/", "/m/guessed/");

        let authority =
            wrong.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = &wrong[wrong.find("/m/").unwrap()..];
        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        assert!(String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 404"));
    }

    /// Ask `server`'s playback entry for what it fronts, and return the status line it answered.
    async fn ask(server: &MendServer) -> String {
        let authority =
            server.url().trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let path = &server.url()[server.url().find("/m/").unwrap()..];
        let mut socket = TcpStream::connect(&authority).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        String::from_utf8_lossy(&raw).lines().next().unwrap_or_default().to_owned()
    }

    #[tokio::test]
    async fn the_status_a_host_refused_playback_with_is_kept_for_its_record() {
        // A host answering `502` reaches mpv as the mender's own `502`, which says nothing of
        // the host; the server's record needs the host's status, so the mender keeps it.
        let host = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = host.local_addr().unwrap();
        tokio::spawn(async move {
            for answer in ["502 Bad Gateway", "200 OK"] {
                let Ok((mut socket, _)) = host.accept().await else { break };
                let mut request = [0u8; 4096];
                let _ = socket.read(&mut request).await;
                let head = format!(
                    "HTTP/1.1 {answer}\r\nContent-Type: video/mp2t\r\nContent-Length: 0\r\n\
                     Connection: close\r\n\r\n"
                );
                let _ = socket.write_all(head.as_bytes()).await;
            }
        });

        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let failing = serve(http.clone(), &format!("http://{address}/a.ts"), Vec::new(), "tok")
            .await
            .unwrap();
        assert_eq!(failing.refused(), None, "nothing asked yet");
        assert!(ask(&failing).await.starts_with("HTTP/1.1 502"));
        assert_eq!(failing.refused(), Some(502));

        let fine =
            serve(http, &format!("http://{address}/b.ts"), Vec::new(), "tok").await.unwrap();
        assert!(ask(&fine).await.starts_with("HTTP/1.1 200"));
        assert_eq!(fine.refused(), None);
    }

    #[tokio::test]
    async fn the_proxy_binds_loopback_only() {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        let server =
            serve(http, "https://example.test/x.m3u8", Vec::new(), "tok").await.unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "got {}", server.url());
    }
}
