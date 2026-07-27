//! A minimal loopback HTTP server that streams one torrent file to a player.
//!
//! mpv needs a URL; librqbit gives an `AsyncRead + AsyncSeek`. This bridges them.
//!
//! librqbit ships an `http-api` feature that would do this, but it exposes the whole
//! torrent *management* surface — add, delete, pause — on the same port. Even on loopback
//! that lets any local process manipulate the session, which sits badly beside the
//! fail-closed VPN guard. So this serves exactly one thing: `GET` on a single unguessable
//! path, read-only, bound to `127.0.0.1` on an ephemeral port.
//!
//! The risky part of writing this by hand is range handling, so that is a pure function
//! with its own tests. Getting it wrong means mpv cannot seek.

use std::{
    io::SeekFrom,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use tokio::{
    io::{
        AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader,
    },
    net::{TcpListener, TcpStream},
};

/// A byte range resolved against a known content length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// First byte, inclusive.
    pub start: u64,
    /// Last byte, inclusive.
    pub end: u64,
}

impl Range {
    pub fn len(&self) -> u64 {
        self.end.saturating_sub(self.start) + 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What a `Range` header asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeRequest {
    /// No `Range` header: the whole file.
    Full,
    /// A satisfiable range.
    Partial(Range),
    /// Syntactically valid but outside the file. Must produce a `416`, not a clamp —
    /// silently returning different bytes than asked for corrupts a seek.
    Unsatisfiable,
}

/// Parse a `Range` header value against a content length.
///
/// Handles the three forms players actually send: `bytes=0-`, `bytes=500-999`, and the
/// suffix form `bytes=-500` meaning the last 500 bytes.
pub fn parse_range(header: Option<&str>, content_length: u64) -> RangeRequest {
    let Some(raw) = header else {
        return RangeRequest::Full;
    };
    let Some(spec) = raw.trim().strip_prefix("bytes=") else {
        // A unit we do not understand. Serving the whole file is the safe reading.
        return RangeRequest::Full;
    };
    // Multi-range requests are legal but rare from players; serving the first is a
    // reasonable simplification, and serving the whole file would break seeking.
    let spec = spec.split(',').next().unwrap_or("").trim();

    let Some((from, to)) = spec.split_once('-') else {
        return RangeRequest::Full;
    };

    if content_length == 0 {
        return RangeRequest::Unsatisfiable;
    }
    let last = content_length - 1;

    match (from.trim(), to.trim()) {
        // `bytes=-500`: the final 500 bytes.
        ("", suffix) => match suffix.parse::<u64>() {
            Ok(0) => RangeRequest::Unsatisfiable,
            Ok(n) => RangeRequest::Partial(Range {
                start: content_length.saturating_sub(n),
                end: last,
            }),
            Err(_) => RangeRequest::Full,
        },
        // `bytes=500-`: from 500 to the end.
        (start, "") => match start.parse::<u64>() {
            Ok(s) if s <= last => RangeRequest::Partial(Range { start: s, end: last }),
            Ok(_) => RangeRequest::Unsatisfiable,
            Err(_) => RangeRequest::Full,
        },
        // `bytes=500-999`.
        (start, end) => match (start.parse::<u64>(), end.parse::<u64>()) {
            (Ok(s), Ok(e)) if s <= e && s <= last => {
                // A player may ask past the end; clamping the *end* is correct here, unlike
                // clamping the start.
                RangeRequest::Partial(Range { start: s, end: e.min(last) })
            }
            (Ok(_), Ok(_)) => RangeRequest::Unsatisfiable,
            _ => RangeRequest::Full,
        },
    }
}

/// Build the response head for a request.
pub fn response_head(request: RangeRequest, content_length: u64, content_type: &str) -> String {
    match request {
        RangeRequest::Full => format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {content_type}\r\n\
             Content-Length: {content_length}\r\n\
             Accept-Ranges: bytes\r\n\
             Connection: close\r\n\r\n"
        ),
        RangeRequest::Partial(range) => format!(
            "HTTP/1.1 206 Partial Content\r\n\
             Content-Type: {content_type}\r\n\
             Content-Length: {}\r\n\
             Content-Range: bytes {}-{}/{content_length}\r\n\
             Accept-Ranges: bytes\r\n\
             Connection: close\r\n\r\n",
            range.len(),
            range.start,
            range.end
        ),
        RangeRequest::Unsatisfiable => format!(
            "HTTP/1.1 416 Range Not Satisfiable\r\n\
             Content-Range: bytes */{content_length}\r\n\
             Content-Length: 0\r\n\
             Connection: close\r\n\r\n"
        ),
    }
}

/// Guess a content type from a filename. mpv does not need it, but a browser does.
pub fn content_type_for(name: &str) -> &'static str {
    let lowered = name.to_ascii_lowercase();
    if lowered.ends_with(".mp4") || lowered.ends_with(".m4v") {
        "video/mp4"
    } else if lowered.ends_with(".webm") {
        "video/webm"
    } else if lowered.ends_with(".avi") {
        "video/x-msvideo"
    } else {
        // Matroska is what anime releases overwhelmingly use.
        "video/x-matroska"
    }
}

/// Extract the request target from an HTTP request line.
pub fn request_target(line: &str) -> Option<&str> {
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    // Read-only by construction: nothing else is even parsed.
    if !method.eq_ignore_ascii_case("GET") && !method.eq_ignore_ascii_case("HEAD") {
        return None;
    }
    parts.next()
}

/// A reader that can also seek.
///
/// Exists so [`StreamSource`] can hand back a boxed reader instead of naming a concrete
/// type. That matters practically: librqbit's `FileStream` lives in a private module and
/// cannot be named from outside the crate, even though the method returning it is public.
/// Boxing also keeps this module free of any torrent dependency, so it is testable against
/// an in-memory cursor.
pub trait SeekableRead: AsyncRead + AsyncSeek + Send + Unpin {}
impl<T: AsyncRead + AsyncSeek + Send + Unpin> SeekableRead for T {}

/// Opens a fresh reader over the same file for each connection.
pub trait StreamSource: Send + Sync + 'static {
    fn open(&self) -> std::io::Result<Box<dyn SeekableRead>>;
    fn len(&self) -> u64;
    fn name(&self) -> String;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A running stream server.
pub struct StreamServer {
    url: String,
    handle: tokio::task::JoinHandle<()>,
}

impl StreamServer {
    /// The URL to hand to a player.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Stop serving. Playback in progress will end.
    pub fn shutdown(self) {
        self.handle.abort();
    }
}

impl Drop for StreamServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// Serve `source` on loopback until dropped.
///
/// The path carries a random token. It is not a security boundary on its own — loopback
/// binding is — but it stops an unrelated local process from stumbling onto the stream by
/// guessing a predictable path.
pub async fn serve<S: StreamSource>(source: S, token: &str) -> std::io::Result<StreamServer> {
    // Loopback only. Never bind a torrent stream to a routable interface.
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
    let port = listener.local_addr()?.port();
    let path = format!("/s/{token}");
    let url = format!("http://127.0.0.1:{port}{path}");

    let source = Arc::new(source);
    let expected_path = path.clone();

    let handle = tokio::spawn(async move {
        loop {
            let Ok((socket, _)) = listener.accept().await else {
                break;
            };
            let source = Arc::clone(&source);
            let expected = expected_path.clone();
            // One task per connection: mpv opens several, and a stalled read on one must
            // not block the others.
            tokio::spawn(async move {
                if let Err(e) = handle_connection(socket, source, &expected).await {
                    tracing::debug!(error = %e, "stream connection ended");
                }
            });
        }
    });

    tracing::info!(%url, "torrent stream server listening on loopback");
    Ok(StreamServer { url, handle })
}

async fn handle_connection<S: StreamSource>(
    socket: TcpStream,
    source: Arc<S>,
    expected_path: &str,
) -> std::io::Result<()> {
    let (read_half, mut write_half) = socket.into_split();
    let mut reader = BufReader::new(read_half);

    let mut request_line = String::new();
    read_line(&mut reader, &mut request_line).await?;

    let target = request_target(&request_line);
    if target.map(|t| t.split('?').next().unwrap_or(t)) != Some(expected_path) {
        write_half.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n").await?;
        return Ok(());
    }
    let is_head = request_line.trim_start().to_ascii_uppercase().starts_with("HEAD");

    // Read headers, keeping only Range.
    let mut range_header: Option<String> = None;
    loop {
        let mut line = String::new();
        let n = read_line(&mut reader, &mut line).await?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("range")
        {
            range_header = Some(value.trim().to_owned());
        }
    }

    let length = source.len();
    let request = parse_range(range_header.as_deref(), length);
    let head = response_head(request, length, content_type_for(&source.name()));
    write_half.write_all(head.as_bytes()).await?;

    if matches!(request, RangeRequest::Unsatisfiable) {
        return Ok(());
    }
    if is_head {
        return Ok(());
    }

    let (start, remaining) = match request {
        RangeRequest::Partial(r) => (r.start, r.len()),
        _ => (0, length),
    };

    let mut file = source.open()?;
    if start > 0 {
        file.seek(SeekFrom::Start(start)).await?;
    }
    copy_exact(&mut file, &mut write_half, remaining).await
}

/// Read one CRLF-terminated line, with a cap so a hostile client cannot exhaust memory.
async fn read_line<R: AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
    out: &mut String,
) -> std::io::Result<usize> {
    const MAX: usize = 8 * 1024;
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

/// Copy exactly `amount` bytes, streaming as they become available.
async fn copy_exact<R, W>(reader: &mut R, writer: &mut W, amount: u64) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    // 64 KiB balances syscall overhead against how long a stalled piece blocks the write.
    let mut buffer = vec![0u8; 64 * 1024];
    let mut left = amount;

    while left > 0 {
        let want = buffer.len().min(left as usize);
        let read = reader.read(&mut buffer[..want]).await?;
        if read == 0 {
            // Short read: the torrent ended or the stream was dropped. Closing the
            // connection is the honest signal.
            break;
        }
        writer.write_all(&buffer[..read]).await?;
        left -= read as u64;
    }
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const LEN: u64 = 1000;

    #[test]
    fn no_range_header_means_the_whole_file() {
        assert_eq!(parse_range(None, LEN), RangeRequest::Full);
    }

    #[test]
    fn an_open_ended_range_runs_to_the_last_byte() {
        // What mpv sends first, every time.
        assert_eq!(
            parse_range(Some("bytes=0-"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 999 })
        );
        assert_eq!(
            parse_range(Some("bytes=500-"), LEN),
            RangeRequest::Partial(Range { start: 500, end: 999 })
        );
    }

    #[test]
    fn a_closed_range_is_inclusive_at_both_ends() {
        let r = parse_range(Some("bytes=500-999"), LEN);
        assert_eq!(r, RangeRequest::Partial(Range { start: 500, end: 999 }));
        if let RangeRequest::Partial(range) = r {
            assert_eq!(range.len(), 500, "inclusive: 500..=999 is 500 bytes");
        }
    }

    #[test]
    fn a_suffix_range_returns_the_tail() {
        // mpv uses this to read the moov atom / trailing index.
        assert_eq!(
            parse_range(Some("bytes=-100"), LEN),
            RangeRequest::Partial(Range { start: 900, end: 999 })
        );
        // A suffix longer than the file is the whole file, not an error.
        assert_eq!(
            parse_range(Some("bytes=-5000"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 999 })
        );
    }

    #[test]
    fn a_start_past_the_end_is_unsatisfiable_rather_than_clamped() {
        // Clamping would silently return different bytes than the player asked for, which
        // corrupts a seek instead of failing it.
        assert_eq!(parse_range(Some("bytes=1000-"), LEN), RangeRequest::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=5000-6000"), LEN), RangeRequest::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=-0"), LEN), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn an_end_past_the_last_byte_is_clamped() {
        // Clamping the *end* is correct and expected; clamping the start is not.
        assert_eq!(
            parse_range(Some("bytes=900-99999"), LEN),
            RangeRequest::Partial(Range { start: 900, end: 999 })
        );
    }

    #[test]
    fn a_backwards_range_is_unsatisfiable() {
        assert_eq!(parse_range(Some("bytes=900-100"), LEN), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn a_malformed_or_unknown_range_falls_back_to_the_whole_file() {
        for header in ["bytes=abc-def", "bytes=", "items=0-10", "nonsense", "bytes=x-"] {
            assert_eq!(
                parse_range(Some(header), LEN),
                RangeRequest::Full,
                "unexpected handling of {header:?}"
            );
        }
    }

    #[test]
    fn only_the_first_of_a_multi_range_request_is_served() {
        assert_eq!(
            parse_range(Some("bytes=0-99,200-299"), LEN),
            RangeRequest::Partial(Range { start: 0, end: 99 })
        );
    }

    #[test]
    fn a_zero_length_file_cannot_satisfy_a_range() {
        assert_eq!(parse_range(Some("bytes=0-"), 0), RangeRequest::Unsatisfiable);
    }

    #[test]
    fn a_full_response_advertises_range_support() {
        // Without Accept-Ranges mpv will not attempt to seek at all.
        let head = response_head(RangeRequest::Full, LEN, "video/x-matroska");
        assert!(head.starts_with("HTTP/1.1 200 OK"));
        assert!(head.contains("Accept-Ranges: bytes"));
        assert!(head.contains("Content-Length: 1000"));
    }

    #[test]
    fn a_partial_response_reports_the_exact_range_and_length() {
        let head = response_head(
            RangeRequest::Partial(Range { start: 500, end: 999 }),
            LEN,
            "video/x-matroska",
        );
        assert!(head.starts_with("HTTP/1.1 206 Partial Content"));
        assert!(head.contains("Content-Range: bytes 500-999/1000"));
        assert!(head.contains("Content-Length: 500"), "must be the range length, not the file");
    }

    #[test]
    fn an_unsatisfiable_response_is_a_416_with_the_real_length() {
        let head = response_head(RangeRequest::Unsatisfiable, LEN, "video/x-matroska");
        assert!(head.starts_with("HTTP/1.1 416"));
        assert!(head.contains("Content-Range: bytes */1000"));
    }

    #[test]
    fn content_types_cover_what_releases_actually_use() {
        assert_eq!(content_type_for("ep01.mkv"), "video/x-matroska");
        assert_eq!(content_type_for("EP01.MP4"), "video/mp4");
        assert_eq!(content_type_for("x.webm"), "video/webm");
        // Matroska is the sane default for anime.
        assert_eq!(content_type_for("no-extension"), "video/x-matroska");
    }

    #[test]
    fn only_read_methods_are_accepted() {
        // Read-only by construction: nothing else is even parsed.
        assert_eq!(request_target("GET /s/abc HTTP/1.1"), Some("/s/abc"));
        assert_eq!(request_target("HEAD /s/abc HTTP/1.1"), Some("/s/abc"));
        assert_eq!(request_target("POST /s/abc HTTP/1.1"), None);
        assert_eq!(request_target("DELETE /s/abc HTTP/1.1"), None);
        assert_eq!(request_target("garbage"), None);
    }

    /// An in-memory source, so the server can be exercised without a torrent session.
    struct MemorySource(Vec<u8>);

    impl StreamSource for MemorySource {
        fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
            Ok(Box::new(Cursor::new(self.0.clone())))
        }
        fn len(&self) -> u64 {
            self.0.len() as u64
        }
        fn name(&self) -> String {
            "test.mkv".into()
        }
    }

    async fn request(url: &str, range: Option<&str>) -> (String, Vec<u8>) {
        use tokio::io::AsyncWriteExt;
        let without_scheme = url.trim_start_matches("http://");
        let (authority, path) = without_scheme.split_once('/').unwrap();
        let mut socket = TcpStream::connect(authority).await.unwrap();

        let mut req = format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\n");
        if let Some(r) = range {
            req.push_str(&format!("Range: {r}\r\n"));
        }
        req.push_str("\r\n");
        socket.write_all(req.as_bytes()).await.unwrap();

        let mut raw = Vec::new();
        socket.read_to_end(&mut raw).await.unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(raw.len());
        (
            String::from_utf8_lossy(&raw[..split]).to_string(),
            raw.get(split + 4..).unwrap_or_default().to_vec(),
        )
    }

    #[tokio::test]
    async fn the_server_returns_the_whole_file_by_default() {
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let server = serve(MemorySource(data.clone()), "tok").await.unwrap();

        let (head, body) = request(server.url(), None).await;
        assert!(head.starts_with("HTTP/1.1 200 OK"), "got {head}");
        assert_eq!(body.len(), 4096);
        assert_eq!(body, data);
    }

    #[tokio::test]
    async fn the_server_honours_a_range_so_seeking_works() {
        // This is the property mpv depends on for seeking to work at all.
        let data: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let server = serve(MemorySource(data.clone()), "tok").await.unwrap();

        let (head, body) = request(server.url(), Some("bytes=1000-1099")).await;
        assert!(head.starts_with("HTTP/1.1 206"), "got {head}");
        assert!(head.contains("Content-Range: bytes 1000-1099/4096"));
        assert_eq!(body.len(), 100);
        assert_eq!(body, &data[1000..1100], "wrong bytes returned for the range");
    }

    #[tokio::test]
    async fn the_server_rejects_an_unsatisfiable_range() {
        let server = serve(MemorySource(vec![0; 100]), "tok").await.unwrap();
        let (head, body) = request(server.url(), Some("bytes=500-")).await;
        assert!(head.starts_with("HTTP/1.1 416"));
        assert!(body.is_empty());
    }

    #[tokio::test]
    async fn the_server_serves_only_its_own_path() {
        // The token is not a security boundary — loopback is — but it stops an unrelated
        // local process from stumbling onto the stream.
        let server = serve(MemorySource(vec![1, 2, 3]), "secret").await.unwrap();
        let wrong = server.url().replace("secret", "guessed");
        let (head, _) = request(&wrong, None).await;
        assert!(head.starts_with("HTTP/1.1 404"), "got {head}");
    }

    #[tokio::test]
    async fn the_server_binds_loopback_only() {
        // Never expose a torrent stream on a routable interface.
        let server = serve(MemorySource(vec![0; 10]), "tok").await.unwrap();
        assert!(server.url().starts_with("http://127.0.0.1:"), "got {}", server.url());
    }

    #[tokio::test]
    async fn dropping_the_server_stops_it_listening() {
        let server = serve(MemorySource(vec![0; 10]), "tok").await.unwrap();
        let url = server.url().to_owned();
        drop(server);
        // Give the abort a moment to take effect.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let authority = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
        let mut socket = match TcpStream::connect(&authority).await {
            Ok(s) => s,
            // Refused outright is the ideal outcome.
            Err(_) => return,
        };
        // Otherwise the accept loop is gone, so no response ever arrives.
        socket.write_all(b"GET / HTTP/1.1\r\n\r\n").await.ok();
        let mut buf = Vec::new();
        let read = tokio::time::timeout(
            std::time::Duration::from_millis(200),
            socket.read_to_end(&mut buf),
        )
        .await;
        assert!(read.is_err() || buf.is_empty(), "server still responding after drop");
    }
}
