//! The LAN proxy a cast device fetches from: the mender's job, on the interface that faces the
//! device instead of loopback, for as long as one cast lasts.
//!
//! **What it guards with.**
//! - It binds the one local address that routes to the device, never `0.0.0.0`, on a fresh port.
//! - Every URL carries a 128-bit token made for this cast alone; any other path is a `404`.
//! - With `allow` set, it accepts connections from the device's address only, and drops the rest
//!   before reading a byte.
//! - It lives as long as the [`LanServer`]: dropping it closes the listener, and the session
//!   drops it when the cast ends.
//!
//! **What it serves.** A directory's files with byte ranges (a torrent's stream or a downloaded
//! episode looks like this), an upstream URL fetched with the stream's own headers with its HLS
//! playlists rewritten to come back through here (the mender's role), and whatever a
//! [`Dynamic`] source makes on request: the remuxer's playlist and segments, a WebVTT track.
//!
//! **What a cast receiver needs that mpv did not.** The receiver is a web page, so adaptive
//! streams are fetched by script and need CORS: `Access-Control-Allow-Origin` on every answer and
//! an answer to `OPTIONS`. Chromium's Private Network Access may ask for
//! `Access-Control-Allow-Private-Network` too; both are answered, and `cors: false` turns them
//! off to see what the receiver does without.

use std::{
    collections::HashMap,
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use tokio::{
    fs::File,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream, tcp::OwnedWriteHalf},
    task::JoinHandle,
};

/// A resource made on request: its content type and body, or `None` for a `404`.
pub type Made<'a> = Pin<Box<dyn Future<Output = Option<(String, Vec<u8>)>> + Send + 'a>>;

/// A source of resources made on request, under `/x/` in the proxy's URLs.
pub trait Dynamic: Send + Sync {
    /// The resource at `path`.
    fn get<'a>(&'a self, path: &'a str) -> Made<'a>;
}

/// How the proxy is set up for one cast.
pub struct Options {
    /// The local address to bind: the one facing the device.
    pub bind: Ipv4Addr,
    /// The only peer accepted, when set.
    pub allow: Option<IpAddr>,
    /// Files under `/f/`.
    pub root: Option<PathBuf>,
    /// The headers each upstream request under `/u/` carries.
    pub upstream_headers: Vec<(String, String)>,
    pub dynamic: Option<Arc<dyn Dynamic>>,
    pub cors: bool,
}

/// One request as the proxy answered it, for the measurements.
#[derive(Debug, Clone)]
pub struct Logged {
    pub at: Duration,
    pub peer: IpAddr,
    pub method: String,
    pub path: String,
    pub range: Option<String>,
    pub origin: Option<String>,
    pub private_network: bool,
    pub status: u16,
    pub bytes: u64,
    pub took: Duration,
}

struct Shared {
    options: Options,
    /// The upstream's headers, which change with each file a cast plays.
    headers: Mutex<Vec<(String, String)>>,
    /// Single files shared under `/s/<id>/`, as a download or a torrent's file is.
    files: Mutex<HashMap<String, PathBuf>>,
    token: String,
    base: String,
    started: Instant,
    log: Mutex<Vec<Logged>>,
    refused_peers: Mutex<Vec<IpAddr>>,
}

/// A running LAN proxy. Dropping it stops it.
pub struct LanServer {
    shared: Arc<Shared>,
    task: JoinHandle<()>,
}

impl Drop for LanServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl LanServer {
    pub async fn start(options: Options) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((options.bind, 0))).await?;
        let port = listener.local_addr()?.port();
        let token: String = (0..16).map(|_| format!("{:02x}", rand::random::<u8>())).collect();
        let base = format!("http://{}:{port}/{token}", options.bind);
        let shared = Arc::new(Shared {
            headers: Mutex::new(options.upstream_headers.clone()),
            files: Mutex::default(),
            options,
            token,
            base,
            started: Instant::now(),
            log: Mutex::default(),
            refused_peers: Mutex::default(),
        });
        let task = {
            let shared = Arc::clone(&shared);
            tokio::spawn(async move {
                while let Ok((socket, peer)) = listener.accept().await {
                    if shared.options.allow.is_some_and(|allow| allow != peer.ip()) {
                        lock(&shared.refused_peers).push(peer.ip());
                        continue;
                    }
                    let shared = Arc::clone(&shared);
                    tokio::spawn(async move {
                        if let Err(e) = connection(socket, peer.ip(), &shared).await {
                            tracing::debug!(error = %e, "LAN proxy connection ended");
                        }
                    });
                }
            })
        };
        tracing::info!(base = %shared.base, "LAN proxy listening");
        Ok(Self { shared, task })
    }

    /// The URL of a file under the root.
    pub fn file_url(&self, relative: &str) -> String {
        format!("{}/f/{}", self.shared.base, relative.trim_start_matches('/'))
    }

    /// The URL that fetches `upstream` with the stream's headers.
    pub fn upstream_url(&self, upstream: &str) -> String {
        format!("{}/u/{}", self.shared.base, encode(upstream))
    }

    /// Share one file, with byte ranges, and return its URL.
    pub fn share_file(&self, path: &Path) -> String {
        let id = format!("{:08x}", rand::random::<u32>());
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        lock(&self.shared.files).insert(id.clone(), path.to_owned());
        format!("{}/s/{id}/{}", self.shared.base, encode(&name))
    }

    /// The headers every upstream request carries from now on.
    pub fn set_upstream_headers(&self, headers: Vec<(String, String)>) {
        *lock(&self.shared.headers) = headers;
    }

    pub fn dynamic_url(&self, path: &str) -> String {
        format!("{}/x/{}", self.shared.base, path.trim_start_matches('/'))
    }

    pub fn base(&self) -> &str {
        &self.shared.base
    }

    pub fn log(&self) -> Vec<Logged> {
        lock(&self.shared.log).clone()
    }

    pub fn clear_log(&self) {
        lock(&self.shared.log).clear();
    }

    pub fn refused_peers(&self) -> Vec<IpAddr> {
        lock(&self.shared.refused_peers).clone()
    }

    pub fn since_start(&self) -> Duration {
        self.shared.started.elapsed()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

struct Request {
    method: String,
    target: String,
    range: Option<String>,
    origin: Option<String>,
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
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut request = Request {
        method,
        target,
        range: None,
        origin: None,
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
            "origin" => request.origin = Some(value),
            "access-control-request-private-network" => request.private_network = true,
            "connection" => request.close = value.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    Ok(Some(request))
}

async fn connection(socket: TcpStream, peer: IpAddr, shared: &Shared) -> std::io::Result<()> {
    socket.set_nodelay(true)?;
    let (read, mut write) = socket.into_split();
    let mut reader = BufReader::new(read);
    while let Some(request) =
        tokio::time::timeout(Duration::from_secs(30), read_request(&mut reader))
            .await
            .unwrap_or(Ok(None))?
    {
        let began = Instant::now();
        let at = shared.started.elapsed();
        let (status, bytes) = answer(&request, shared, &mut write).await?;
        let path = request.target.replacen(&shared.token, "<token>", 1);
        tracing::info!(
            %peer, method = %request.method, %path, range = ?request.range, status, bytes,
            ms = began.elapsed().as_millis() as u64, "served"
        );
        lock(&shared.log).push(Logged {
            at,
            peer,
            method: request.method.clone(),
            path,
            range: request.range.clone(),
            origin: request.origin.clone(),
            private_network: request.private_network,
            status,
            bytes,
            took: began.elapsed(),
        });
        if request.close {
            break;
        }
    }
    Ok(())
}

fn cors_headers(shared: &Shared, request: &Request) -> String {
    if !shared.options.cors {
        return String::new();
    }
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
    reason: &str,
    extra: &str,
    length: u64,
) -> std::io::Result<()> {
    let head = format!("HTTP/1.1 {status} {reason}\r\n{extra}Content-Length: {length}\r\n\r\n");
    write.write_all(head.as_bytes()).await
}

async fn answer(
    request: &Request,
    shared: &Shared,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, u64)> {
    let cors = cors_headers(shared, request);
    let Some(rest) = request
        .target
        .split('?')
        .next()
        .and_then(|t| t.strip_prefix('/'))
        .and_then(|t| t.strip_prefix(shared.token.as_str()))
        .and_then(|t| t.strip_prefix('/'))
    else {
        head(write, 404, "Not Found", "", 0).await?;
        return Ok((404, 0));
    };
    if request.method == "OPTIONS" {
        head(write, 204, "No Content", &cors, 0).await?;
        return Ok((204, 0));
    }
    let head_only = request.method == "HEAD";
    if request.method != "GET" && !head_only {
        head(write, 405, "Method Not Allowed", &cors, 0).await?;
        return Ok((405, 0));
    }
    if let Some(relative) = rest.strip_prefix("f/") {
        let Some(root) = &shared.options.root else {
            head(write, 404, "Not Found", &cors, 0).await?;
            return Ok((404, 0));
        };
        return file(root, &decode(relative), request, &cors, head_only, write).await;
    }
    if let Some(shared_file) = rest.strip_prefix("s/") {
        let id = shared_file.split('/').next().unwrap_or_default();
        let path = lock(&shared.files).get(id).cloned();
        if let Some(path) = path
            && let (Some(dir), Some(name)) = (path.parent(), path.file_name())
        {
            let name = name.to_string_lossy().into_owned();
            return file(dir, &name, request, &cors, head_only, write).await;
        }
        head(write, 404, "Not Found", &cors, 0).await?;
        return Ok((404, 0));
    }
    if let Some(encoded) = rest.strip_prefix("u/") {
        return upstream(shared, &decode(encoded), request, &cors, head_only, write).await;
    }
    if let Some(path) = rest.strip_prefix("x/")
        && let Some(dynamic) = &shared.options.dynamic
        && let Some((content_type, body)) = dynamic.get(path).await
    {
        let extra = format!("{cors}Content-Type: {content_type}\r\n");
        head(write, 200, "OK", &extra, body.len() as u64).await?;
        if !head_only {
            write.write_all(&body).await?;
        }
        return Ok((200, body.len() as u64));
    }
    head(write, 404, "Not Found", &cors, 0).await?;
    Ok((404, 0))
}

pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default().to_ascii_lowercase().as_str() {
        "m3u8" => "application/vnd.apple.mpegurl",
        "ts" => "video/mp2t",
        "m4s" | "mp4" | "m4v" => "video/mp4",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "vtt" => "text/vtt",
        _ => "application/octet-stream",
    }
}

/// Parse a single `bytes=a-b` range against a resource of `size` bytes.
fn parse_range(range: &str, size: u64) -> Option<(u64, u64)> {
    let spec = range.trim().strip_prefix("bytes=")?;
    let (start, end) = spec.split_once('-')?;
    let (start, end) = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().ok()?;
            (size.saturating_sub(suffix), size.checked_sub(1)?)
        }
        (start, "") => (start.parse().ok()?, size.checked_sub(1)?),
        (start, end) => (start.parse().ok()?, end.parse::<u64>().ok()?.min(size.checked_sub(1)?)),
    };
    (start <= end && start < size).then_some((start, end))
}

async fn file(
    root: &Path,
    relative: &str,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, u64)> {
    // Under the root, and nowhere else.
    if relative.split('/').any(|part| part == ".." || part.is_empty()) {
        head(write, 404, "Not Found", cors, 0).await?;
        return Ok((404, 0));
    }
    let path = root.join(relative);
    let Ok(mut file) = File::open(&path).await else {
        head(write, 404, "Not Found", cors, 0).await?;
        return Ok((404, 0));
    };
    let size = file.metadata().await?.len();
    let kind = content_type(relative);
    let mut extra = format!("{cors}Content-Type: {kind}\r\nAccept-Ranges: bytes\r\n");
    let (status, start, len) = match request.range.as_deref().map(|r| parse_range(r, size)) {
        Some(Some((start, end))) => {
            extra.push_str(&format!("Content-Range: bytes {start}-{end}/{size}\r\n"));
            (206, start, end - start + 1)
        }
        Some(None) => {
            let extra = format!("{cors}Content-Range: bytes */{size}\r\n");
            head(write, 416, "Range Not Satisfiable", &extra, 0).await?;
            return Ok((416, 0));
        }
        None => (200, 0, size),
    };
    let reason = if status == 206 { "Partial Content" } else { "OK" };
    head(write, status, reason, &extra, len).await?;
    if head_only {
        return Ok((status, 0));
    }
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let sent = tokio::io::copy(&mut file.take(len), write).await?;
    Ok((status, sent))
}

/// Fetch `url` over plain HTTP with the stream's headers. The engine's mender does this with
/// its emulated client and TLS; the spike's upstream is a local stand-in for a locked CDN.
async fn fetch(
    url: &str,
    headers: &[(String, String)],
    range: Option<&str>,
) -> std::io::Result<(u16, Vec<(String, String)>, Vec<u8>)> {
    let rest = url
        .strip_prefix("http://")
        .ok_or_else(|| std::io::Error::other("the spike's upstream speaks plain HTTP only"))?;
    let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
    let path = if path.is_empty() { "/" } else { path };
    let mut socket = TcpStream::connect(authority).await?;
    let mut ask = format!("GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n");
    for (name, value) in headers {
        ask.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(range) = range {
        ask.push_str(&format!("Range: {range}\r\n"));
    }
    ask.push_str("\r\n");
    socket.write_all(ask.as_bytes()).await?;
    let mut raw = Vec::new();
    socket.read_to_end(&mut raw).await?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| std::io::Error::other("no response head"))?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let body = raw[split + 4..].to_vec();
    let mut lines = head.lines();
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(502);
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    Ok((status, headers, body))
}

async fn upstream(
    shared: &Shared,
    url: &str,
    request: &Request,
    cors: &str,
    head_only: bool,
    write: &mut OwnedWriteHalf,
) -> std::io::Result<(u16, u64)> {
    let whole = request.range.as_deref().is_none_or(|r| r.trim() == "bytes=0-");
    let range = if whole { None } else { request.range.as_deref() };
    let upstream_headers = lock(&shared.headers).clone();
    let Ok((status, headers, body)) = fetch(url, &upstream_headers, range).await else {
        head(write, 502, "Bad Gateway", cors, 0).await?;
        return Ok((502, 0));
    };
    let header = |name: &str| headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
    let kind = header("content-type").unwrap_or_else(|| content_type(url).to_owned());
    let playlist = kind.contains("mpegurl") || body.starts_with(b"#EXTM3U");
    let body = if playlist && status == 200 {
        let text = String::from_utf8_lossy(&body);
        rewrite_playlist(&text, url, &|u| format!("{}/u/{}", shared.base, encode(u))).into_bytes()
    } else {
        body
    };
    let mut extra = format!("{cors}Content-Type: {kind}\r\n");
    if let Some(content_range) = header("content-range") {
        extra.push_str(&format!("Content-Range: {content_range}\r\n"));
    }
    let reason = if status == 206 { "Partial Content" } else { "Relayed" };
    head(write, status, reason, &extra, body.len() as u64).await?;
    if !head_only {
        write.write_all(&body).await?;
    }
    Ok((status, body.len() as u64))
}

/// Point every URI in an HLS playlist back through the proxy.
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
                out.push(byte as char)
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

/// The proxy's own address is never 0.0.0.0: a guard for callers that build `Options` by hand.
pub fn is_lan_only(bind: Ipv4Addr) -> bool {
    !bind.is_unspecified() && !bind.is_loopback()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_read_as_http_says() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=-10", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=90-200", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=100-", 100), None);
    }

    #[test]
    fn playlist_uris_come_back_through_the_proxy() {
        let text = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:4,\nseg0.m4s\n/abs/seg1.m4s\n";
        let out = rewrite_playlist(text, "http://cdn.test/a/b.m3u8", &|u| format!("P[{u}]"));
        assert!(out.contains("#EXT-X-MAP:URI=\"P[http://cdn.test/a/init.mp4]\""), "{out}");
        assert!(out.contains("\nP[http://cdn.test/a/seg0.m4s]\n"));
        assert!(out.contains("\nP[http://cdn.test/abs/seg1.m4s]\n"));
    }

    #[test]
    fn a_wildcard_address_is_not_lan_only() {
        assert!(!is_lan_only(Ipv4Addr::UNSPECIFIED));
        assert!(!is_lan_only(Ipv4Addr::LOCALHOST));
        assert!(is_lan_only(Ipv4Addr::new(192, 168, 0, 2)));
    }
}
