//! The relay a TV fetches from while a cast lasts: the mender's and the torrent server's loopback
//! addresses, a file on this device, and what the remuxer and the subtitle converter make, on the
//! interface that faces the TV.
//!
//! What it guards with:
//! - It binds the one local address that routes to the TV, never `0.0.0.0`, on a fresh port.
//! - Every path carries a 128-bit token made for this cast alone; any other path is a `404`.
//! - It accepts connections from the TV's address alone and drops the rest unread.
//! - It fetches upstream only from the origins it was handed, and those a playlist it relayed
//!   names, so it is no open proxy.
//! - It closes when the [`Relay`] is dropped, which the cast does when it ends, and every
//!   connection open to it closes with it.
//!
//! What a receiver needs that mpv did not: the receiver is a web page, so HLS and WebVTT are
//! fetched by script and need CORS, `Access-Control-Allow-Origin` on every answer and an answer
//! to `OPTIONS`, and Chromium's Private Network Access may ask for
//! `Access-Control-Allow-Private-Network`.
//!
//! Bodies are streamed as they arrive. Playlists alone are read whole, to point their addresses
//! back here.

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use futures::StreamExt;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedWriteHalf},
    task::{JoinHandle, JoinSet},
};

use crate::fetch::Upstream;

/// A resource made on request: its content type and body, or `None` for a `404`.
pub type Made<'a> = Pin<Box<dyn Future<Output = Option<(String, Vec<u8>)>> + Send + 'a>>;

/// What the relay makes on request, under `/x/`: the remuxer's playlist and segments, a WebVTT
/// track.
pub trait Dynamic: Send + Sync {
    fn get<'a>(&'a self, path: &'a str) -> Made<'a>;
}

/// The largest playlist relayed.
const PLAYLIST_LIMIT: usize = 8 << 20;

struct Shared {
    token: String,
    base: String,
    allow: IpAddr,
    upstream: Arc<dyn Upstream>,
    /// The headers each upstream request carries: the stream's, which change with each file.
    headers: Mutex<Vec<(String, String)>>,
    /// The `scheme://authority` origins the relay may fetch from.
    origins: Mutex<HashSet<String>>,
    /// Single files shared under `/s/<id>/`.
    files: Mutex<HashMap<String, PathBuf>>,
    dynamic: Mutex<Option<Arc<dyn Dynamic>>>,
    /// Small bodies kept in memory under `/b/<id>/`: the capability clips.
    bytes: Mutex<HashMap<String, (&'static str, &'static [u8])>>,
    /// Requests that arrived, counted as they arrive: a TV that reads a header and refuses
    /// closes the connection before the body is done.
    requests: AtomicU64,
    /// Requests for the picture and sound (an init or media segment, a progressive range), as
    /// they arrive, playlists and subtitles aside.
    media_requests: AtomicU64,
    last_request: Mutex<Instant>,
}

/// A running relay. Dropping it closes it.
pub struct Relay {
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.close();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Relay {
    /// Listen on `bind` for `allow` alone, fetching upstream through `upstream`.
    pub async fn start(
        bind: Ipv4Addr,
        allow: IpAddr,
        upstream: Arc<dyn Upstream>,
    ) -> std::io::Result<Self> {
        if bind.is_unspecified() {
            return Err(std::io::Error::other("the relay never listens on every interface"));
        }
        let listener = TcpListener::bind(SocketAddr::from((bind, 0))).await?;
        let port = listener.local_addr()?.port();
        let token: String = (0..16).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
        let shared = Arc::new(Shared {
            base: format!("http://{bind}:{port}/{token}"),
            token,
            allow,
            upstream,
            headers: Mutex::default(),
            origins: Mutex::default(),
            files: Mutex::default(),
            dynamic: Mutex::default(),
            bytes: Mutex::default(),
            requests: AtomicU64::new(0),
            media_requests: AtomicU64::new(0),
            last_request: Mutex::new(Instant::now()),
        });
        let task = {
            let shared = Arc::clone(&shared);
            tokio::spawn(async move {
                // Every connection lives in this set, so aborting the accept loop drops them
                // all: a TV's kept-alive connection gets nothing once the cast has ended.
                let mut connections = JoinSet::new();
                while let Ok((socket, peer)) = listener.accept().await {
                    while connections.try_join_next().is_some() {}
                    if peer.ip() != shared.allow {
                        tracing::debug!(peer = %peer.ip(), "relay refused a connection");
                        continue;
                    }
                    let shared = Arc::clone(&shared);
                    connections.spawn(async move {
                        if let Err(e) = connection(socket, &shared).await {
                            tracing::debug!(error = %e, "relay connection ended");
                        }
                    });
                }
            })
        };
        Ok(Self { shared, task })
    }

    /// Stop serving: the listener and every open connection close, and what was shared is
    /// forgotten. The cast calls it the moment it ends; dropping the relay does it too.
    pub fn close(&self) {
        self.task.abort();
        // A request already being answered holds `Shared` until its task is dropped; what it
        // could reach goes now.
        lock(&self.shared.dynamic).take();
        lock(&self.shared.files).clear();
        lock(&self.shared.bytes).clear();
        lock(&self.shared.origins).clear();
    }

    pub fn base(&self) -> &str {
        &self.shared.base
    }

    /// Share one file, with byte ranges, and return its URL.
    pub fn share_file(&self, path: &std::path::Path) -> String {
        let id = format!("{:08x}", rand::random::<u32>());
        let name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        lock(&self.shared.files).insert(id.clone(), path.to_owned());
        format!("{}/s/{id}/{}", self.shared.base, encode(&name))
    }

    /// Share `body`, kept in memory, as `name` of `content_type`, and return its URL.
    pub fn share_bytes(
        &self,
        name: &str,
        content_type: &'static str,
        body: &'static [u8],
    ) -> String {
        let id = format!("{:08x}", rand::random::<u32>());
        lock(&self.shared.bytes).insert(id.clone(), (content_type, body));
        format!("{}/b/{id}/{}", self.shared.base, encode(name))
    }

    /// The URL that fetches `upstream` with the stream's headers; its origin is allowed from now.
    pub fn upstream_url(&self, upstream: &str) -> String {
        if let Some(origin) = origin(upstream) {
            lock(&self.shared.origins).insert(origin);
        }
        format!("{}/u/{}", self.shared.base, encode(upstream))
    }

    /// The headers every upstream request carries from now on.
    pub fn set_headers(&self, headers: Vec<(String, String)>) {
        *lock(&self.shared.headers) = headers;
    }

    pub fn set_dynamic(&self, dynamic: Option<Arc<dyn Dynamic>>) {
        *lock(&self.shared.dynamic) = dynamic;
    }

    pub fn dynamic_url(&self, path: &str) -> String {
        format!("{}/x/{}", self.shared.base, path.trim_start_matches('/'))
    }

    /// Requests that arrived since the last [`Relay::reset_stats`].
    pub fn requests(&self) -> u64 {
        self.shared.requests.load(Ordering::Relaxed)
    }

    /// Whether the TV asked for the picture or sound since the last reset: an init segment, a
    /// media segment or a progressive range, counted when the request arrives.
    pub fn media_served(&self) -> bool {
        self.shared.media_requests.load(Ordering::Relaxed) > 0
    }

    pub fn reset_stats(&self) {
        self.shared.requests.store(0, Ordering::Relaxed);
        self.shared.media_requests.store(0, Ordering::Relaxed);
    }

    /// How long since the TV last asked for anything.
    pub fn idle_for(&self) -> Duration {
        lock(&self.shared.last_request).elapsed()
    }
}

/// `scheme://authority` of a URL.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    Some(format!("{}://{}", scheme.to_ascii_lowercase(), authority.to_ascii_lowercase()))
}

struct Request {
    method: String,
    target: String,
    range: Option<String>,
    private_network: bool,
    close: bool,
}

async fn read_request(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> std::io::Result<Option<Request>> {
    let mut line = String::new();
    if reader.read_line(&mut line).await? == 0 {
        return Ok(None);
    }
    let mut parts = line.split_whitespace();
    let mut request = Request {
        method: parts.next().unwrap_or_default().to_owned(),
        target: parts.next().unwrap_or_default().to_owned(),
        range: None,
        private_network: false,
        close: false,
    };
    let mut total = line.len();
    loop {
        let mut header = String::new();
        let n = reader.read_line(&mut header).await?;
        total += n;
        if total > 32 * 1024 {
            return Err(std::io::Error::other("headers too long"));
        }
        let header = header.trim_end();
        if n == 0 || header.is_empty() {
            break;
        }
        let Some((name, value)) = header.split_once(':') else { continue };
        let value = value.trim().to_owned();
        match name.trim().to_ascii_lowercase().as_str() {
            "range" => request.range = Some(value),
            "access-control-request-private-network" => request.private_network = true,
            "connection" => request.close = value.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    Ok(Some(request))
}

async fn connection(socket: TcpStream, shared: &Shared) -> std::io::Result<()> {
    socket.set_nodelay(true)?;
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    while let Some(request) =
        tokio::time::timeout(Duration::from_secs(60), read_request(&mut reader))
            .await
            .unwrap_or(Ok(None))?
    {
        *lock(&shared.last_request) = Instant::now();
        shared.requests.fetch_add(1, Ordering::Relaxed);
        let (status, keep) = answer(&request, shared, &mut write).await?;
        tracing::debug!(method = %request.method, status, "relayed");
        if request.close || !keep {
            break;
        }
    }
    Ok(())
}

fn cors(request: &Request) -> String {
    let mut out = String::from(
        "Access-Control-Allow-Origin: *\r\n\
         Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n",
    );
    if request.method == "OPTIONS" {
        out.push_str(
            "Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
             Access-Control-Allow-Headers: *\r\n\
             Access-Control-Max-Age: 600\r\n",
        );
        if request.private_network {
            out.push_str("Access-Control-Allow-Private-Network: true\r\n");
        }
    }
    out
}

async fn head(
    write: &mut OwnedWriteHalf,
    status: u16,
    extra: &str,
    length: Option<u64>,
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        404 => "Not Found",
        405 => "Method Not Allowed",
        416 => "Range Not Satisfiable",
        502 => "Bad Gateway",
        _ => "Relayed",
    };
    let length = match length {
        Some(length) => format!("Content-Length: {length}\r\n"),
        None => "Connection: close\r\n".to_owned(),
    };
    let head = format!("HTTP/1.1 {status} {reason}\r\n{extra}{length}\r\n");
    write.write_all(head.as_bytes()).await
}

/// Answer one request: its status, and whether the connection may carry another.
async fn answer(
    request: &Request,
    shared: &Shared,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let cors = cors(request);
    let Some(rest) = request
        .target
        .split('?')
        .next()
        .and_then(|t| t.strip_prefix('/'))
        .and_then(|t| t.strip_prefix(shared.token.as_str()))
        .and_then(|t| t.strip_prefix('/'))
    else {
        head(write, 404, "", Some(0)).await?;
        return Ok((404, true));
    };
    if request.method == "OPTIONS" {
        head(write, 204, &cors, Some(0)).await?;
        return Ok((204, true));
    }
    let head_only = request.method == "HEAD";
    if request.method != "GET" && !head_only {
        head(write, 405, &cors, Some(0)).await?;
        return Ok((405, true));
    }
    if let Some(kept) = rest.strip_prefix("b/") {
        let id = kept.split('/').next().unwrap_or_default();
        let found = lock(&shared.bytes).get(id).copied();
        if let Some((content_type, body)) = found {
            if !head_only {
                shared.media_requests.fetch_add(1, Ordering::Relaxed);
            }
            let extra = format!("{cors}Content-Type: {content_type}\r\n");
            head(write, 200, &extra, Some(body.len() as u64)).await?;
            if !head_only {
                write.write_all(body).await?;
            }
            return Ok((200, true));
        }
    } else if let Some(shared_file) = rest.strip_prefix("s/") {
        let id = shared_file.split('/').next().unwrap_or_default();
        let path = lock(&shared.files).get(id).cloned();
        if let Some(path) = path {
            return file(shared, &path, request, &cors, head_only, write).await;
        }
    } else if let Some(encoded) = rest.strip_prefix("u/") {
        return upstream(shared, &decode(encoded), request, &cors, head_only, write).await;
    } else if let Some(path) = rest.strip_prefix("x/") {
        let dynamic = lock(&shared.dynamic).clone();
        if let Some(dynamic) = dynamic
            && let Some((content_type, body)) = dynamic.get(path).await
        {
            if is_media(&content_type) && !head_only {
                shared.media_requests.fetch_add(1, Ordering::Relaxed);
            }
            let extra = format!("{cors}Content-Type: {content_type}\r\n");
            head(write, 200, &extra, Some(body.len() as u64)).await?;
            if !head_only {
                write.write_all(&body).await?;
            }
            return Ok((200, true));
        }
    }
    head(write, 404, &cors, Some(0)).await?;
    Ok((404, true))
}

/// Whether bytes of this type are the picture and sound, not a playlist or subtitles.
fn is_media(content_type: &str) -> bool {
    let kind = content_type.to_ascii_lowercase();
    !(kind.contains("mpegurl") || kind.contains("vtt") || kind.starts_with("text/"))
}

pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "m3u8" => "application/vnd.apple.mpegurl",
        "ts" => "video/mp2t",
        "m4s" | "mp4" | "m4v" => "video/mp4",
        "m4a" => "audio/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "vtt" => "text/vtt",
        _ => "application/octet-stream",
    }
}

/// A single `bytes=a-b` range against a resource of `size` bytes.
fn parse_range(range: &str, size: u64) -> Option<(u64, u64)> {
    let spec = range.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().ok()?;
            (size.saturating_sub(suffix), size.checked_sub(1)?)
        }
        (start, "") => (start.parse().ok()?, size.checked_sub(1)?),
        (start, end) => {
            (start.parse().ok()?, end.parse::<u64>().ok()?.min(size.checked_sub(1)?))
        }
    };
    (start <= end && start < size).then_some((start, end))
}

async fn file(
    shared: &Shared,
    path: &std::path::Path,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let Ok(mut file) = File::open(path).await else {
        head(write, 404, cors, Some(0)).await?;
        return Ok((404, true));
    };
    let size = file.metadata().await?.len();
    let kind = content_type(&path.to_string_lossy());
    let mut extra = format!("{cors}Content-Type: {kind}\r\nAccept-Ranges: bytes\r\n");
    let (status, start, len) = match request.range.as_deref().map(|r| parse_range(r, size)) {
        Some(Some((start, end))) => {
            extra.push_str(&format!("Content-Range: bytes {start}-{end}/{size}\r\n"));
            (206, start, end - start + 1)
        }
        Some(None) => {
            let extra = format!("{cors}Content-Range: bytes */{size}\r\n");
            head(write, 416, &extra, Some(0)).await?;
            return Ok((416, true));
        }
        None => (200, 0, size),
    };
    if is_media(kind) && !head_only {
        shared.media_requests.fetch_add(1, Ordering::Relaxed);
    }
    head(write, status, &extra, Some(len)).await?;
    if head_only {
        return Ok((status, true));
    }
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut body = file.take(len);
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let n = body.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        write.write_all(&buffer[..n]).await?;
    }
    Ok((status, true))
}

async fn upstream(
    shared: &Shared,
    url: &str,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, bool)> {
    let allowed = origin(url).is_some_and(|o| lock(&shared.origins).contains(&o));
    if !allowed {
        head(write, 404, cors, Some(0)).await?;
        return Ok((404, true));
    }
    let mut headers = lock(&shared.headers).clone();
    if let Some(range) = &request.range {
        headers.push(("Range".into(), range.clone()));
    }
    let response = match shared.upstream.get(url, &headers).await {
        Ok(response) => response,
        Err(e) => {
            tracing::debug!(error = %e, "relay upstream failed");
            head(write, 502, cors, Some(0)).await?;
            return Ok((502, true));
        }
    };
    let status = response.status;
    let kind = response.header("content-type").map_or_else(
        || content_type(url.split('?').next().unwrap_or(url)).to_owned(),
        str::to_owned,
    );
    let content_range = response.header("content-range").map(str::to_owned);
    let length: Option<u64> =
        response.header("content-length").and_then(|l| l.trim().parse().ok());
    let mut extra = format!("{cors}Content-Type: {kind}\r\n");
    if let Some(content_range) = content_range {
        extra.push_str(&format!(
            "Content-Range: bytes {}\r\n",
            content_range.trim_start_matches("bytes ")
        ));
    }
    if response.header("accept-ranges").is_some() || status == 206 {
        extra.push_str("Accept-Ranges: bytes\r\n");
    }

    let playlist = kind.contains("mpegurl")
        || url.split('?').next().is_some_and(|p| p.to_ascii_lowercase().ends_with(".m3u8"));
    if playlist && matches!(status, 200 | 206) {
        // A playlist is rewritten whole, so it is fetched whole, whatever range was asked.
        let whole = status == 200
            || response.header("content-range").is_some_and(|r| {
                let r = r.trim_start_matches("bytes ").trim();
                let (span, total) = r.split_once('/').unwrap_or((r, ""));
                let (from, to) = span.split_once('-').unwrap_or(("", ""));
                from == "0" && to.parse::<u64>().ok().map(|t| t + 1) == total.parse().ok()
            });
        let body = if whole {
            response.bytes(PLAYLIST_LIMIT).await?
        } else {
            let headers = lock(&shared.headers).clone();
            match shared.upstream.get(url, &headers).await {
                Ok(again) if again.status == 200 => again.bytes(PLAYLIST_LIMIT).await?,
                _ => {
                    head(write, 502, cors, Some(0)).await?;
                    return Ok((502, true));
                }
            }
        };
        let text = String::from_utf8_lossy(&body);
        let origins = std::cell::RefCell::new(Vec::new());
        let rewritten = rewrite_playlist(&text, url, &|u| {
            if let Some(o) = origin(u) {
                origins.borrow_mut().push(o);
            }
            format!("{}/u/{}", shared.base, encode(u))
        });
        lock(&shared.origins).extend(origins.into_inner());
        let extra = format!("{cors}Content-Type: {kind}\r\n");
        head(write, 200, &extra, Some(rewritten.len() as u64)).await?;
        if !head_only {
            write.write_all(rewritten.as_bytes()).await?;
        }
        return Ok((200, true));
    }

    if (200..300).contains(&status) && is_media(&kind) && !head_only {
        shared.media_requests.fetch_add(1, Ordering::Relaxed);
    }
    head(write, status, &extra, length).await?;
    if head_only {
        return Ok((status, length.is_some()));
    }
    let mut body = response.body;
    let mut sent = 0u64;
    while let Some(chunk) = body.next().await {
        let chunk = chunk?;
        write.write_all(&chunk).await?;
        sent += chunk.len() as u64;
    }
    // A body shorter than it said can't leave the connection in step.
    Ok((status, length.is_some_and(|l| l == sent)))
}

/// Point every URI in an HLS playlist back through the relay.
pub fn rewrite_playlist(text: &str, base: &str, proxy: &dyn Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push('\n');
        } else if !trimmed.starts_with('#') {
            out.push_str(&proxy(&resolve(base, trimmed)));
            out.push('\n');
        } else if let Some(start) = trimmed.find("URI=\"") {
            let value_at = start + 5;
            let end = trimmed[value_at..].find('"').map_or(trimmed.len(), |e| value_at + e);
            out.push_str(&trimmed[..value_at]);
            out.push_str(&proxy(&resolve(base, &trimmed[value_at..end])));
            out.push_str(&trimmed[end..]);
            out.push('\n');
        } else {
            out.push_str(trimmed);
            out.push('\n');
        }
    }
    out
}

/// Resolve `reference` against `base`, enough for playlist URIs.
pub fn resolve(base: &str, reference: &str) -> String {
    if reference.contains("://") {
        return reference.to_owned();
    }
    let scheme_end = base.find("://").map_or(0, |i| i + 3);
    let authority_end = base[scheme_end..].find('/').map_or(base.len(), |i| scheme_end + i);
    if let Some(rooted) = reference.strip_prefix('/') {
        return format!("{}/{rooted}", &base[..authority_end]);
    }
    let path = base.split('?').next().unwrap_or(base);
    let dir_end = path.rfind('/').filter(|&i| i >= authority_end).map_or(path.len(), |i| i + 1);
    let mut joined = format!("{}{reference}", &path[..dir_end]);
    if dir_end == path.len() && !joined[..dir_end].ends_with('/') {
        joined.insert(dir_end, '/');
    }
    joined
}

fn encode(url: &str) -> String {
    let mut out = String::with_capacity(url.len() + 16);
    for byte in url.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(byte));
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode(encoded: &str) -> String {
    let bytes = encoded.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(byte) =
                encoded.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetch::{PlainHttp, UpstreamResponse};

    #[test]
    fn ranges_read_as_http_says() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=90-200", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
    }

    #[test]
    fn playlist_uris_come_back_through_the_relay() {
        let text =
            "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:4,\nseg0.m4s\n/abs/seg1.m4s\n";
        let out = rewrite_playlist(text, "http://cdn.example/a/b.m3u8", &|u| format!("P[{u}]"));
        assert!(out.contains("#EXT-X-MAP:URI=\"P[http://cdn.example/a/init.mp4]\""), "{out}");
        assert!(out.contains("\nP[http://cdn.example/a/seg0.m4s]\n"));
        assert!(out.contains("\nP[http://cdn.example/abs/seg1.m4s]\n"));
    }

    /// GET `url` as the receiver does, with its origin.
    async fn get(url: &str, range: Option<&str>) -> UpstreamResponse {
        let mut headers = vec![("Origin".to_owned(), crate::probe::RECEIVER_ORIGIN.to_owned())];
        if let Some(range) = range {
            headers.push(("Range".into(), range.into()));
        }
        PlainHttp.get(url, &headers).await.unwrap()
    }

    #[tokio::test]
    async fn the_relay_serves_its_tv_with_cors_and_only_what_it_was_handed() {
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let source = crate::fetch::tests::serve(body.clone(), true).await;
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();

        // Streamed through, chunked upstream and all, with CORS.
        let url = relay.upstream_url(&source);
        let response = get(&url, None).await;
        assert_eq!(response.status, 200);
        assert_eq!(response.header("access-control-allow-origin"), Some("*"));
        assert_eq!(response.bytes(1 << 20).await.unwrap(), body);
        assert!(relay.media_served());

        let ranged = get(&url, Some("bytes=100-199")).await;
        assert_eq!(ranged.status, 206);
        assert_eq!(ranged.bytes(1 << 20).await.unwrap(), body[100..200]);

        // Somewhere it was never handed is not fetched.
        let elsewhere = format!("{}/u/{}", relay.base(), encode("http://elsewhere.example/x"));
        assert_eq!(get(&elsewhere, None).await.status, 404);
        // A guessed token is a 404, whatever the path.
        let wrong = url.replace(&relay.shared.token, &"0".repeat(32));
        assert_eq!(get(&wrong, None).await.status, 404);

        // A file here, by ranges, and the preflight a receiver may send.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("episode.mp4");
        std::fs::write(&path, &body).unwrap();
        let shared = relay.share_file(&path);
        let part = get(&shared, Some("bytes=0-")).await;
        assert_eq!((part.status, part.header("content-type")), (206, Some("video/mp4")));
        assert_eq!(part.bytes(1 << 20).await.unwrap().len(), body.len());

        relay.reset_stats();
        assert_eq!(relay.requests(), 0);
        assert!(!relay.media_served());

        // A TV that asks, reads the head and hangs up has still asked for media.
        let (authority, path) = shared.trim_start_matches("http://").split_once('/').unwrap();
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        socket
            .write_all(format!("GET /{path} HTTP/1.1\r\nHost: tv\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut start = [0u8; 12];
        socket.read_exact(&mut start).await.unwrap();
        drop(socket);
        assert!(start.starts_with(b"HTTP/1.1 200"));
        assert_eq!(relay.requests(), 1);
        assert!(relay.media_served());

        // Bytes kept in memory, as the capability clips are.
        relay.reset_stats();
        let kept = relay.share_bytes("clip.webm", "video/webm", b"not a real clip");
        let response = get(&kept, None).await;
        assert_eq!(response.header("content-type"), Some("video/webm"));
        assert_eq!(response.bytes(1024).await.unwrap(), b"not a real clip");
        assert!(relay.media_served());
    }

    #[tokio::test]
    async fn a_connection_kept_alive_is_closed_when_the_cast_ends() {
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let kept = relay.share_bytes("clip.webm", "video/webm", b"twelve bytes");
        let (authority, path) = kept.trim_start_matches("http://").split_once('/').unwrap();
        let ask = format!("GET /{path} HTTP/1.1\r\nHost: tv\r\n\r\n");
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        socket.write_all(ask.as_bytes()).await.unwrap();
        let mut answer = vec![0u8; 512];
        let n = socket.read(&mut answer).await.unwrap();
        assert!(answer[..n].starts_with(b"HTTP/1.1 200"));

        drop(relay);
        tokio::time::sleep(Duration::from_millis(50)).await;
        // The same connection, asked again: nothing comes back but the close.
        let _ = socket.write_all(ask.as_bytes()).await;
        let mut rest = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(2), socket.read_to_end(&mut rest))
            .await
            .expect("the connection closed");
        assert!(read.is_err() || rest.is_empty(), "{}", String::from_utf8_lossy(&rest));
    }

    #[tokio::test]
    async fn the_relay_drops_anyone_but_its_tv() {
        let relay = Relay::start(
            Ipv4Addr::LOCALHOST,
            "192.0.2.7".parse().unwrap(),
            Arc::new(PlainHttp),
        )
        .await
        .unwrap();
        let url = relay.dynamic_url("t/1.vtt");
        assert!(PlainHttp.get(&url, &[]).await.is_err(), "closed before a byte");
        assert!(
            Relay::start(
                Ipv4Addr::UNSPECIFIED,
                "192.0.2.7".parse().unwrap(),
                Arc::new(PlainHttp)
            )
            .await
            .is_err()
        );
    }
}
